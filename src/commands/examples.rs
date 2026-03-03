use openapiv3::{ReferenceOr, Schema, SchemaKind, Type};
use crate::spec;
use std::collections::HashSet;

/// Generate a JSON example object for a named schema.
///
/// - `include_optional`: when false, only required fields are included.
/// - Returns None if the schema doesn't exist.
/// - Caps recursion at depth 3 to handle circular references.
pub fn generate_example(
    api: &openapiv3::OpenAPI,
    schema_name: &str,
    include_optional: bool,
) -> Option<serde_json::Value> {
    let mut visited = HashSet::new();
    let (_, schema) = crate::commands::schemas::find_schema(api, schema_name)?;
    let required: Vec<String> = extract_required(schema);
    let mut value = generate_from_schema(api, schema, &required, include_optional, &mut visited, 0);

    // BUG-2 fix: for allOf subtypes, apply discriminator values from parent schemas.
    // When a schema is allOf [ParentRef, ...] and the parent has a discriminator mapping
    // that includes this schema, override the discriminator field with the mapped key value.
    if let SchemaKind::AllOf { all_of } = &schema.schema_kind {
        if let serde_json::Value::Object(ref mut map) = value {
            for member in all_of {
                if let ReferenceOr::Reference { reference } = member {
                    let parent_schema = spec::schema_name_from_ref(reference).and_then(|pname| {
                        api.components
                            .as_ref()
                            .and_then(|c| c.schemas.get(pname))
                            .and_then(|s| match s {
                                ReferenceOr::Item(s) => Some(s as &Schema),
                                _ => None,
                            })
                    });
                    if let Some(parent) = parent_schema {
                        if let Some(disc) = &parent.schema_data.discriminator {
                            // Find the key that maps to this schema name
                            let disc_value = disc.mapping.iter().find_map(|(key, ref_val)| {
                                let mapped_name = spec::schema_name_from_ref(ref_val)
                                    .unwrap_or(ref_val.as_str());
                                if mapped_name == schema_name {
                                    Some(key.clone())
                                } else {
                                    None
                                }
                            });
                            if let Some(val) = disc_value {
                                map.insert(
                                    disc.property_name.clone(),
                                    serde_json::Value::String(val),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    Some(value)
}

/// Generate a JSON example value from a schema, with recursion guard.
fn generate_from_schema(
    api: &openapiv3::OpenAPI,
    schema: &Schema,
    required: &[String],
    include_optional: bool,
    visited: &mut HashSet<String>,
    depth: usize,
) -> serde_json::Value {
    if depth > 3 {
        return serde_json::Value::Null;
    }

    // Use spec-provided example if present
    if let Some(example) = &schema.schema_data.example {
        return example.clone();
    }

    match &schema.schema_kind {
        SchemaKind::Type(Type::Object(obj)) => {
            let mut map = serde_json::Map::new();
            for (field_name, field_ref) in &obj.properties {
                let is_required = required.contains(field_name);
                if !is_required && !include_optional {
                    continue;
                }
                let field_val = resolve_field_ref(api, field_ref, include_optional, visited, depth);
                map.insert(field_name.clone(), field_val);
            }
            serde_json::Value::Object(map)
        }
        // Implicit object: has properties but no `type: object` (parsed as AnySchema)
        SchemaKind::Any(any) if !any.properties.is_empty() => {
            let mut map = serde_json::Map::new();
            for (field_name, field_ref) in &any.properties {
                let is_required = required.contains(field_name);
                if !is_required && !include_optional {
                    continue;
                }
                let field_val = resolve_field_ref(api, field_ref, include_optional, visited, depth);
                map.insert(field_name.clone(), field_val);
            }
            serde_json::Value::Object(map)
        }
        // BUG-1 fix: type:boolean + format:boolean is parsed as AnySchema{typ:Some("boolean")}
        // by openapiv3 (non-standard format causes fallback to AnySchema). Detect and produce true.
        SchemaKind::Any(any) if any.typ.as_deref() == Some("boolean") => {
            serde_json::json!(true)
        }
        SchemaKind::Type(Type::String(str_type)) => {
            // Enum: use first value
            if let Some(first) = str_type.enumeration.iter().find_map(|v| v.clone()) {
                return serde_json::Value::String(first);
            }
            // Format-aware placeholder
            let placeholder = match &str_type.format {
                openapiv3::VariantOrUnknownOrEmpty::Item(f) => match f {
                    openapiv3::StringFormat::DateTime => "2024-01-15T10:30:00Z",
                    openapiv3::StringFormat::Date => "2024-01-15",
                    openapiv3::StringFormat::Password => "string",
                    openapiv3::StringFormat::Byte => "string",
                    openapiv3::StringFormat::Binary => "string",
                },
                openapiv3::VariantOrUnknownOrEmpty::Unknown(s) => match s.as_str() {
                    "uuid" => "550e8400-e29b-41d4-a716-446655440000",
                    "email" => "user@example.com",
                    "uri" | "url" => "https://example.com",
                    _ => "string",
                },
                openapiv3::VariantOrUnknownOrEmpty::Empty => "string",
            };
            serde_json::Value::String(placeholder.to_string())
        }
        SchemaKind::Type(Type::Integer(_)) => serde_json::json!(0),
        SchemaKind::Type(Type::Number(_)) => serde_json::json!(0.0),
        SchemaKind::Type(Type::Boolean(_)) => serde_json::json!(true),
        SchemaKind::Type(Type::Array(arr)) => {
            let item_val = arr.items.as_ref().map(|items_ref| match items_ref {
                ReferenceOr::Reference { reference } => {
                    if let Some(sname) = spec::schema_name_from_ref(reference) {
                        if visited.contains(sname) {
                            serde_json::Value::Null
                        } else {
                            visited.insert(sname.to_string());
                            let val = api
                                .components
                                .as_ref()
                                .and_then(|c| c.schemas.get(sname))
                                .and_then(|s| match s {
                                    ReferenceOr::Item(s) => Some(s as &Schema),
                                    _ => None,
                                })
                                .map(|s| {
                                    let req = extract_required(s);
                                    generate_from_schema(api, s, &req, include_optional, visited, depth + 1)
                                })
                                .unwrap_or(serde_json::Value::Null);
                            visited.remove(sname);
                            val
                        }
                    } else {
                        serde_json::Value::Null
                    }
                }
                ReferenceOr::Item(item_schema) => {
                    let req = extract_required(item_schema.as_ref());
                    generate_from_schema(api, item_schema.as_ref(), &req, include_optional, visited, depth + 1)
                }
            }).unwrap_or(serde_json::Value::Null);
            serde_json::Value::Array(vec![item_val])
        }
        SchemaKind::AllOf { all_of } => {
            // Merge fields from all allOf members
            let mut map = serde_json::Map::new();
            for member in all_of {
                let member_schema = match member {
                    ReferenceOr::Reference { reference } => {
                        spec::schema_name_from_ref(reference).and_then(|sname| {
                            api.components
                                .as_ref()
                                .and_then(|c| c.schemas.get(sname))
                                .and_then(|s| match s {
                                    ReferenceOr::Item(s) => Some(s as &Schema),
                                    _ => None,
                                })
                        })
                    }
                    ReferenceOr::Item(s) => Some(s as &Schema),
                };
                if let Some(s) = member_schema {
                    let req = extract_required(s);
                    let val = generate_from_schema(api, s, &req, include_optional, visited, depth + 1);
                    if let serde_json::Value::Object(fields) = val {
                        map.extend(fields);
                    }
                }
            }
            serde_json::Value::Object(map)
        }
        // For oneOf/anyOf: generate from the first variant
        SchemaKind::OneOf { one_of } => {
            generate_from_first_variant(api, one_of, include_optional, visited, depth)
        }
        SchemaKind::AnyOf { any_of } => {
            generate_from_first_variant(api, any_of, include_optional, visited, depth)
        }
        // Any schema type not covered: return null
        _ => serde_json::Value::Null,
    }
}

/// Generate an example from the first variant of a oneOf/anyOf list.
fn generate_from_first_variant(
    api: &openapiv3::OpenAPI,
    variants: &[ReferenceOr<Schema>],
    include_optional: bool,
    visited: &mut HashSet<String>,
    depth: usize,
) -> serde_json::Value {
    if let Some(first) = variants.first() {
        let first_schema = match first {
            ReferenceOr::Reference { reference } => {
                spec::schema_name_from_ref(reference).and_then(|sname| {
                    api.components
                        .as_ref()
                        .and_then(|c| c.schemas.get(sname))
                        .and_then(|s| match s {
                            ReferenceOr::Item(s) => Some(s as &Schema),
                            _ => None,
                        })
                })
            }
            ReferenceOr::Item(s) => Some(s as &Schema),
        };
        if let Some(s) = first_schema {
            let req = extract_required(s);
            return generate_from_schema(api, s, &req, include_optional, visited, depth + 1);
        }
    }
    serde_json::Value::Object(serde_json::Map::new())
}

/// Resolve a field reference (either $ref or inline schema) and generate an example value.
fn resolve_field_ref(
    api: &openapiv3::OpenAPI,
    field_ref: &ReferenceOr<Box<Schema>>,
    include_optional: bool,
    visited: &mut HashSet<String>,
    depth: usize,
) -> serde_json::Value {
    match field_ref {
        ReferenceOr::Reference { reference } => {
            if let Some(sname) = spec::schema_name_from_ref(reference) {
                if visited.contains(sname) {
                    return serde_json::Value::Null;
                }
                visited.insert(sname.to_string());
                let val = api
                    .components
                    .as_ref()
                    .and_then(|c| c.schemas.get(sname))
                    .and_then(|s| match s {
                        ReferenceOr::Item(s) => Some(s as &Schema),
                        _ => None,
                    })
                    .map(|s| {
                        let req = extract_required(s);
                        generate_from_schema(api, s, &req, include_optional, visited, depth + 1)
                    })
                    .unwrap_or(serde_json::Value::Null);
                visited.remove(sname);
                val
            } else {
                serde_json::Value::Null
            }
        }
        ReferenceOr::Item(field_schema) => {
            let req = extract_required(field_schema);
            generate_from_schema(api, field_schema, &req, include_optional, visited, depth + 1)
        }
    }
}

fn extract_required(schema: &Schema) -> Vec<String> {
    match &schema.schema_kind {
        SchemaKind::Type(Type::Object(obj)) => obj.required.clone(),
        SchemaKind::Any(any) => any.required.clone(),
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_petstore() -> openapiv3::OpenAPI {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let content = std::fs::read_to_string(
            manifest_dir.join("tests/fixtures/petstore.yaml")
        ).unwrap();
        serde_yaml_ng::from_str(&content).unwrap()
    }

    fn load_kitchen_sink() -> openapiv3::OpenAPI {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let content = std::fs::read_to_string(
            manifest_dir.join("tests/fixtures/kitchen-sink.yaml")
        ).unwrap();
        serde_yaml_ng::from_str(&content).unwrap()
    }

    #[test]
    fn test_example_required_fields_only() {
        let api = load_kitchen_sink();
        // CreateUserRequest has required: [username, email, password]
        let output = generate_example(&api, "CreateUserRequest", false);
        assert!(output.is_some(), "Should generate example for CreateUserRequest");
        let obj = output.unwrap();
        assert!(obj.get("username").is_some(), "Should include required 'username'");
        assert!(obj.get("email").is_some(), "Should include required 'email'");
        assert!(obj.get("password").is_some(), "Should include required 'password'");
        // 'role' is optional (has default but not required) — should not appear when required-only
        assert!(obj.get("role").is_none(), "Should not include optional 'role'");
    }

    #[test]
    fn test_example_format_aware_uuid() {
        let api = load_kitchen_sink();
        // User.id is string/uuid
        let output = generate_example(&api, "User", true).unwrap();
        assert_eq!(
            output.get("id").and_then(|v| v.as_str()),
            Some("550e8400-e29b-41d4-a716-446655440000"),
            "UUID format should produce standard UUID placeholder"
        );
    }

    #[test]
    fn test_example_format_aware_date_time() {
        let api = load_kitchen_sink();
        let output = generate_example(&api, "User", true).unwrap();
        // created_at is string/date-time
        assert_eq!(
            output.get("created_at").and_then(|v| v.as_str()),
            Some("2024-01-15T10:30:00Z"),
            "date-time format should produce ISO 8601 placeholder"
        );
    }

    #[test]
    fn test_example_enum_uses_first_value() {
        let api = load_kitchen_sink();
        let output = generate_example(&api, "CreateUserRequest", true).unwrap();
        // role is enum [admin, editor, viewer]
        let role_val = output.get("role").and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            ["admin", "editor", "viewer"].contains(&role_val),
            "Enum field should use one of the enum values, got: {}",
            role_val
        );
    }

    #[test]
    fn test_example_unknown_schema_returns_none() {
        let api = load_petstore();
        let output = generate_example(&api, "NonExistentSchema", false);
        assert!(output.is_none(), "Unknown schema should return None");
    }

    #[test]
    fn test_example_depth_cap_prevents_infinite_loop() {
        let api = load_kitchen_sink();
        // TreeNode is a self-referencing schema — should not loop infinitely
        let output = generate_example(&api, "TreeNode", false);
        // Should return Some (even if partial) within reasonable time
        assert!(
            output.is_some(),
            "Self-referencing schema should not panic or loop"
        );
    }

    // BUG-1 regression: type:boolean with format:boolean was parsed by openapiv3 as
    // AnySchema{typ:Some("boolean")} instead of Type::Boolean, causing null output.
    #[test]
    fn test_example_boolean_with_format_annotation() {
        let api = load_kitchen_sink();
        let output = generate_example(&api, "BooleanFormatEntity", false).unwrap();
        assert_eq!(
            output.get("isActive"),
            Some(&serde_json::json!(true)),
            "isActive (type:boolean format:boolean) should produce true, not null"
        );
    }

    // BUG-2 regression: allOf subtype should use discriminator-mapped key for the
    // discriminator property, not the generic type placeholder ("string").
    #[test]
    fn test_example_allof_subtype_discriminator_value() {
        let api = load_kitchen_sink();

        let widget = generate_example(&api, "WidgetEntity", false).unwrap();
        assert_eq!(
            widget.get("kind").and_then(|v| v.as_str()),
            Some("widget"),
            "WidgetEntity discriminator field 'kind' should be 'widget', got: {:?}",
            widget.get("kind")
        );

        let gadget = generate_example(&api, "GadgetEntity", false).unwrap();
        assert_eq!(
            gadget.get("kind").and_then(|v| v.as_str()),
            Some("gadget"),
            "GadgetEntity discriminator field 'kind' should be 'gadget', got: {:?}",
            gadget.get("kind")
        );
    }
}
