use crate::commands::resources::{build_fields, extract_object_properties};
use crate::models::resource::Field;
use crate::models::schema::{Composition, DiscriminatorInfo, SchemaModel};
use crate::spec;
use std::collections::HashSet;

/// Returns all schema names sorted alphabetically.
pub fn list_schemas(api: &openapiv3::OpenAPI) -> Vec<String> {
    api.components
        .as_ref()
        .map(|c| {
            let mut names: Vec<String> = c.schemas.keys().cloned().collect();
            names.sort();
            names
        })
        .unwrap_or_default()
}

/// Looks up a schema by name (case-sensitive first, then case-insensitive fallback).
/// Returns the canonical key name from the spec alongside the schema.
pub fn find_schema<'a>(
    api: &'a openapiv3::OpenAPI,
    name: &str,
) -> Option<(&'a str, &'a openapiv3::Schema)> {
    let schemas = &api.components.as_ref()?.schemas;

    // Exact match first
    if let Some((key, ref_or)) = schemas.get_key_value(name) {
        return match ref_or {
            openapiv3::ReferenceOr::Item(s) => Some((key.as_str(), s)),
            openapiv3::ReferenceOr::Reference { reference } => {
                let sname = spec::schema_name_from_ref(reference)?;
                let (resolved_key, resolved_ref) = schemas.get_key_value(sname)?;
                match resolved_ref {
                    openapiv3::ReferenceOr::Item(s) => Some((resolved_key.as_str(), s)),
                    _ => None,
                }
            }
        };
    }

    // Case-insensitive fallback
    let lower = name.to_lowercase();
    for (key, ref_or) in schemas {
        if key.to_lowercase() == lower {
            return match ref_or {
                openapiv3::ReferenceOr::Item(s) => Some((key.as_str(), s)),
                _ => None,
            };
        }
    }

    None
}

/// Returns up to 3 schema names with Jaro-Winkler distance > 0.8 from `name`.
pub fn suggest_similar_schemas(api: &openapiv3::OpenAPI, name: &str) -> Vec<String> {
    let lower = name.to_lowercase();
    api.components
        .as_ref()
        .map(|c| {
            c.schemas
                .keys()
                .filter(|k| strsim::jaro_winkler(&lower, &k.to_lowercase()) > 0.8)
                .take(3)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// Build a SchemaModel from a named schema — with fields, composition, and optional expansion.
pub fn build_schema_model(
    api: &openapiv3::OpenAPI,
    name: &str,
    expand: bool,
    max_depth: usize,
) -> Option<SchemaModel> {
    let (canonical_name, schema) = find_schema(api, name)?;

    let description = schema.schema_data.description.clone();
    let title = schema.schema_data.title.clone();

    let (fields, composition) = match &schema.schema_kind {
        openapiv3::SchemaKind::Type(openapiv3::Type::Object(obj)) => {
            let fields = build_fields(api, schema, &obj.required);
            (fields, None)
        }
        // Implicit object: has properties but no `type: object` (parsed as AnySchema)
        openapiv3::SchemaKind::Any(any) if !any.properties.is_empty() => {
            let fields = build_fields(api, schema, &any.required);
            (fields, None)
        }
        openapiv3::SchemaKind::Type(openapiv3::Type::String(str_type))
            if !str_type.enumeration.is_empty() =>
        {
            let values: Vec<String> = str_type
                .enumeration
                .iter()
                .filter_map(|v| v.clone())
                .collect();
            (Vec::new(), Some(Composition::Enum(values)))
        }
        openapiv3::SchemaKind::Type(openapiv3::Type::Integer(int_type))
            if !int_type.enumeration.is_empty() =>
        {
            let values: Vec<String> = int_type
                .enumeration
                .iter()
                .filter_map(|v| v.map(|n| n.to_string()))
                .collect();
            (Vec::new(), Some(Composition::Enum(values)))
        }
        openapiv3::SchemaKind::AllOf { .. } => {
            let fields = build_fields(api, schema, &[]);
            (fields, Some(Composition::AllOf))
        }
        openapiv3::SchemaKind::OneOf { one_of } => {
            let variants = extract_variant_names(one_of);
            let fields = extract_composition_fields(api, one_of);
            (fields, Some(Composition::OneOf(variants)))
        }
        openapiv3::SchemaKind::AnyOf { any_of } => {
            let variants = extract_variant_names(any_of);
            let fields = extract_composition_fields(api, any_of);
            (fields, Some(Composition::AnyOf(variants)))
        }
        _ => (Vec::new(), None),
    };

    // Compute base_type for schemas that aren't Object/composition — gives the LLM
    // a type label for primitive, array, and `not` schemas that would otherwise be opaque.
    let base_type = match &schema.schema_kind {
        openapiv3::SchemaKind::Type(t) => match t {
            openapiv3::Type::String(s) if s.enumeration.is_empty() => Some("string"),
            openapiv3::Type::Integer(i) if i.enumeration.is_empty() => Some("integer"),
            openapiv3::Type::Number(_) => Some("number"),
            openapiv3::Type::Boolean(_) => Some("boolean"),
            openapiv3::Type::Array(_) => Some("array"),
            _ => None, // Object and enum variants are covered by fields/composition
        },
        openapiv3::SchemaKind::Not { .. } => Some("not"),
        _ => None, // AllOf/OneOf/AnyOf already have composition
    };

    // Extract discriminator (lives on schema_data, independent of schema_kind)
    let discriminator = schema.schema_data.discriminator.as_ref().map(|d| {
        let mapping = d
            .mapping
            .iter()
            .map(|(value, reference)| {
                let schema_name = spec::schema_name_from_ref(reference)
                    .unwrap_or(reference.as_str())
                    .to_string();
                (value.clone(), schema_name)
            })
            .collect();
        DiscriminatorInfo {
            property_name: d.property_name.clone(),
            mapping,
        }
    });

    // Expand nested schemas if requested
    let fields = if expand {
        let mut visited = HashSet::new();
        visited.insert(name.to_string());
        expand_fields(api, fields, &mut visited, 1, max_depth)
    } else {
        fields
    };

    Some(SchemaModel {
        name: canonical_name.to_string(),
        title,
        description,
        fields,
        composition,
        discriminator,
        external_docs: None,
        base_type: base_type.map(|s| s.to_string()),
    })
}

/// Collect fields from anyOf/oneOf variant schemas.
///
/// For $ref variants, resolves the referenced schema and extracts its object fields.
/// For inline object variants, extracts fields directly.
/// Fields are merged across all variants (deduplicating by name — last variant wins).
///
/// Why: anyOf/oneOf schemas would otherwise show zero fields, making the schema detail
/// completely empty. This gives the user visibility into what fields each variant provides.
fn extract_composition_fields(
    api: &openapiv3::OpenAPI,
    variants: &[openapiv3::ReferenceOr<openapiv3::Schema>],
) -> Vec<Field> {
    let mut all_fields: Vec<Field> = Vec::new();

    for variant in variants {
        let resolved: Option<&openapiv3::Schema> = match variant {
            openapiv3::ReferenceOr::Reference { reference } => {
                spec::schema_name_from_ref(reference).and_then(|sname| {
                    api.components
                        .as_ref()
                        .and_then(|c| c.schemas.get(sname))
                        .and_then(|s| match s {
                            openapiv3::ReferenceOr::Item(schema) => {
                                Some(schema as &openapiv3::Schema)
                            }
                            _ => None,
                        })
                })
            }
            openapiv3::ReferenceOr::Item(schema) => Some(schema),
        };

        let schema = match resolved {
            Some(s) => s,
            None => continue,
        };

        let required = match &schema.schema_kind {
            openapiv3::SchemaKind::Type(openapiv3::Type::Object(obj)) => obj.required.clone(),
            _ => vec![],
        };

        let fields = build_fields(api, schema, &required);
        for field in fields {
            // Dedup by name — later variant wins
            all_fields.retain(|f| f.name != field.name);
            all_fields.push(field);
        }
    }

    all_fields
}

fn extract_variant_names(refs: &[openapiv3::ReferenceOr<openapiv3::Schema>]) -> Vec<String> {
    refs.iter()
        .filter_map(|r| match r {
            openapiv3::ReferenceOr::Reference { reference } => {
                spec::schema_name_from_ref(reference).map(|s| s.to_string())
            }
            openapiv3::ReferenceOr::Item(schema) => match &schema.schema_kind {
                openapiv3::SchemaKind::Type(t) => Some(match t {
                    openapiv3::Type::String(_) => "string".to_string(),
                    openapiv3::Type::Number(_) => "number".to_string(),
                    openapiv3::Type::Integer(_) => "integer".to_string(),
                    openapiv3::Type::Boolean(_) => "boolean".to_string(),
                    openapiv3::Type::Array(_) => "array".to_string(),
                    openapiv3::Type::Object(_) => "object".to_string(),
                }),
                _ => None,
            },
        })
        .collect()
}

pub(crate) fn expand_fields_pub(
    api: &openapiv3::OpenAPI,
    fields: Vec<Field>,
    visited: &mut HashSet<String>,
    depth: usize,
    max_depth: usize,
) -> Vec<Field> {
    expand_fields(api, fields, visited, depth, max_depth)
}

fn expand_fields(
    api: &openapiv3::OpenAPI,
    fields: Vec<Field>,
    visited: &mut HashSet<String>,
    depth: usize,
    max_depth: usize,
) -> Vec<Field> {
    if depth > max_depth {
        return fields;
    }

    fields
        .into_iter()
        .map(|mut field| {
            if let Some(ref schema_name) = field.nested_schema_name {
                if !visited.contains(schema_name) {
                    visited.insert(schema_name.clone());

                    if let Some((_, nested_schema)) = find_schema(api, schema_name) {
                        let required = extract_object_properties(nested_schema)
                            .map(|(_, req)| req)
                            .unwrap_or_default();
                        let nested = build_fields(api, nested_schema, &required);
                        field.nested_fields =
                            expand_fields(api, nested, visited, depth + 1, max_depth);
                    }

                    visited.remove(schema_name);
                }
            }
            field
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_petstore_api() -> openapiv3::OpenAPI {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let content =
            std::fs::read_to_string(manifest_dir.join("tests/fixtures/petstore.yaml")).unwrap();
        serde_yaml_ng::from_str(&content).unwrap()
    }

    fn load_kitchen_sink_api() -> openapiv3::OpenAPI {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let content =
            std::fs::read_to_string(manifest_dir.join("tests/fixtures/kitchen-sink.yaml")).unwrap();
        serde_yaml_ng::from_str(&content).unwrap()
    }

    #[test]
    fn test_list_schemas() {
        let api = load_petstore_api();
        let names = list_schemas(&api);
        assert!(names.contains(&"Pet".to_string()));
        assert!(names.contains(&"Owner".to_string()));
        assert!(names.contains(&"PetList".to_string()));
        assert!(names.contains(&"PetOrOwner".to_string()));
    }

    #[test]
    fn test_find_schema_exact() {
        let api = load_petstore_api();
        assert!(find_schema(&api, "Pet").is_some());
        assert!(find_schema(&api, "NonExistent").is_none());
    }

    #[test]
    fn test_find_schema_case_insensitive() {
        let api = load_petstore_api();
        let result = find_schema(&api, "pet");
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().0,
            "Pet",
            "Should return canonical name 'Pet' for input 'pet'"
        );
    }

    #[test]
    fn test_build_schema_model_pet() {
        let api = load_petstore_api();
        let model = build_schema_model(&api, "Pet", false, 5).unwrap();
        assert_eq!(model.name, "Pet");
        assert!(model.composition.is_none());
        assert!(!model.fields.is_empty());
        let field_names: Vec<&str> = model.fields.iter().map(|f| f.name.as_str()).collect();
        assert!(field_names.contains(&"id"));
        assert!(field_names.contains(&"name"));
    }

    #[test]
    fn test_build_schema_model_oneof() {
        let api = load_petstore_api();
        let model = build_schema_model(&api, "PetOrOwner", false, 5).unwrap();
        match &model.composition {
            Some(Composition::OneOf(variants)) => {
                assert!(variants.contains(&"Pet".to_string()));
                assert!(variants.contains(&"Owner".to_string()));
            }
            other => panic!("Expected OneOf composition, got {:?}", other),
        }
    }

    #[test]
    fn test_build_schema_model_allof() {
        let api = load_petstore_api();
        let model = build_schema_model(&api, "PetList", false, 5).unwrap();
        match &model.composition {
            Some(Composition::AllOf) => {}
            other => panic!("Expected AllOf composition, got {:?}", other),
        }
        assert!(
            !model.fields.is_empty(),
            "AllOf should have flattened fields"
        );
    }

    #[test]
    fn test_expand_schema() {
        let api = load_petstore_api();
        let model = build_schema_model(&api, "Pet", true, 5).unwrap();
        let owner_field = model.fields.iter().find(|f| f.name == "owner");
        assert!(owner_field.is_some(), "Pet should have owner field");
        assert!(
            !owner_field.unwrap().nested_fields.is_empty(),
            "Expanded owner should have nested fields"
        );
    }

    #[test]
    fn test_build_schema_model_enum() {
        let api = load_petstore_api();
        let model = build_schema_model(&api, "PetStatus", false, 5).unwrap();
        assert_eq!(model.name, "PetStatus");
        match &model.composition {
            Some(Composition::Enum(values)) => {
                assert!(values.contains(&"available".to_string()));
                assert!(values.contains(&"pending".to_string()));
                assert!(values.contains(&"sold".to_string()));
                assert_eq!(values.len(), 3);
            }
            other => panic!("Expected Enum composition, got {:?}", other),
        }
        assert!(model.fields.is_empty(), "Enum schema should have no fields");
    }

    // ─── Task 4.1: Jaro-Winkler suggestion tests ───

    #[test]
    fn test_suggest_similar_schemas_transposition() {
        // "Ownre" is a transposition of "Owner" — contains() misses this
        let api = load_petstore_api();
        let suggestions = suggest_similar_schemas(&api, "Ownre");
        assert!(
            suggestions.contains(&"Owner".to_string()),
            "Jaro-Winkler should suggest 'Owner' for transposition typo 'Ownre', got: {:?}",
            suggestions
        );
    }

    #[test]
    fn test_suggest_similar_schemas_extra_char() {
        // "Pett" has an extra char — contains() misses this
        let api = load_petstore_api();
        let suggestions = suggest_similar_schemas(&api, "Pett");
        assert!(
            suggestions.contains(&"Pet".to_string()),
            "Jaro-Winkler should suggest 'Pet' for near-match 'Pett', got: {:?}",
            suggestions
        );
    }

    #[test]
    fn test_suggest_similar_schemas_no_false_positive() {
        // "Xyz" should not match any schema
        let api = load_petstore_api();
        let suggestions = suggest_similar_schemas(&api, "Xyz");
        assert!(
            suggestions.is_empty(),
            "Jaro-Winkler must not suggest schemas for completely different input 'Xyz', got: {:?}",
            suggestions
        );
    }

    #[test]
    fn test_build_schema_model_discriminator() {
        let api = load_petstore_api();
        let model = build_schema_model(&api, "PetOrOwner", false, 5).unwrap();
        let disc = model
            .discriminator
            .as_ref()
            .expect("PetOrOwner should have a discriminator");
        assert_eq!(disc.property_name, "type");
        assert!(
            disc.mapping.iter().any(|(k, v)| k == "pet" && v == "Pet"),
            "Expected pet→Pet mapping, got: {:?}",
            disc.mapping
        );
        assert!(
            disc.mapping
                .iter()
                .any(|(k, v)| k == "owner" && v == "Owner"),
            "Expected owner→Owner mapping"
        );
    }

    #[test]
    fn test_integer_enum_schema_model() {
        let api = load_kitchen_sink_api();
        let model = build_schema_model(&api, "Priority", false, 5).unwrap();
        match &model.composition {
            Some(Composition::Enum(values)) => {
                assert!(values.contains(&"0".to_string()), "missing 0: {:?}", values);
                assert!(values.contains(&"4".to_string()), "missing 4: {:?}", values);
                assert_eq!(values.len(), 5);
            }
            other => panic!("Expected Enum, got {:?}", other),
        }
    }

    #[test]
    fn test_schema_title_extracted() {
        let api = load_kitchen_sink_api();
        let model = build_schema_model(&api, "GeoLocation", false, 5).unwrap();
        assert_eq!(
            model.title.as_deref(),
            Some("Geographic Location"),
            "GeoLocation should have title 'Geographic Location', got: {:?}",
            model.title
        );
    }

    #[test]
    fn test_schema_no_title_is_none() {
        let api = load_kitchen_sink_api();
        let model = build_schema_model(&api, "User", false, 5).unwrap();
        assert!(
            model.title.is_none(),
            "User has no title, got: {:?}",
            model.title
        );
    }

    #[test]
    fn test_expand_array_of_ref() {
        let api = load_kitchen_sink_api();
        // Error.details is array of ErrorDetail — should inline ErrorDetail fields when expanded
        let model = build_schema_model(&api, "Error", true, 5).unwrap();
        let details_field = model.fields.iter().find(|f| f.name == "details");
        assert!(details_field.is_some(), "Error should have a details field");
        let details = details_field.unwrap();
        assert!(
            !details.nested_fields.is_empty(),
            "With --expand, details (ErrorDetail[]) should have nested_fields populated. \
             Got type_display={:?}, nested_schema_name={:?}",
            details.type_display,
            details.nested_schema_name
        );
        // Spot-check that ErrorDetail's fields appear
        let field_names: Vec<&str> = details
            .nested_fields
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        assert!(
            field_names.contains(&"field") || field_names.contains(&"reason"),
            "Expanded details should contain ErrorDetail fields (field, reason), got: {:?}",
            field_names
        );
    }

    #[test]
    fn test_non_admin_role_has_base_type() {
        let api = load_kitchen_sink_api();
        let model = build_schema_model(&api, "NonAdminRole", false, 5).unwrap();
        assert_eq!(
            model.base_type.as_deref(),
            Some("not"),
            "NonAdminRole (a `not` schema) should have base_type='not', got: {:?}",
            model.base_type
        );
        assert!(model.fields.is_empty(), "not schema should have no fields");
        assert!(
            model.composition.is_none(),
            "not schema should have no composition"
        );
    }

    #[test]
    fn test_object_schema_has_no_base_type() {
        let api = load_kitchen_sink_api();
        let model = build_schema_model(&api, "User", false, 5).unwrap();
        assert!(
            model.base_type.is_none(),
            "Object schemas should not have base_type, got: {:?}",
            model.base_type
        );
    }

    #[test]
    fn test_oneof_inline_types() {
        let api = load_kitchen_sink_api();
        let model = build_schema_model(&api, "InsecureSsl", false, 5).unwrap();
        match &model.composition {
            Some(Composition::OneOf(variants)) => {
                assert!(
                    variants.contains(&"boolean".to_string()),
                    "Expected 'boolean' variant, got: {:?}",
                    variants
                );
                assert!(
                    variants.contains(&"string".to_string()),
                    "Expected 'string' variant, got: {:?}",
                    variants
                );
                assert_eq!(
                    variants.len(),
                    2,
                    "Expected 2 variants, got: {:?}",
                    variants
                );
            }
            other => panic!("Expected OneOf composition, got {:?}", other),
        }
    }

    #[test]
    fn test_anyof_inline_types() {
        let api = load_kitchen_sink_api();
        let model = build_schema_model(&api, "FlexibleValue", false, 5).unwrap();
        match &model.composition {
            Some(Composition::AnyOf(variants)) => {
                assert!(
                    variants.contains(&"string".to_string()),
                    "Expected 'string' variant, got: {:?}",
                    variants
                );
                assert!(
                    variants.contains(&"number".to_string()),
                    "Expected 'number' variant, got: {:?}",
                    variants
                );
                assert!(
                    variants.contains(&"integer".to_string()),
                    "Expected 'integer' variant, got: {:?}",
                    variants
                );
                assert_eq!(
                    variants.len(),
                    3,
                    "Expected 3 variants, got: {:?}",
                    variants
                );
            }
            other => panic!("Expected AnyOf composition, got {:?}", other),
        }
    }

    #[test]
    fn test_anyof_ref_variants_have_fields() {
        // SearchResult is anyOf [User, PetBase, FileMetadata] — all $ref variants.
        // Previously returned zero fields; now should merge fields from all variants.
        let api = load_kitchen_sink_api();
        let model = build_schema_model(&api, "SearchResult", false, 5).unwrap();

        assert!(
            !model.fields.is_empty(),
            "anyOf with $ref variants should have merged fields"
        );

        let field_names: Vec<&str> = model.fields.iter().map(|f| f.name.as_str()).collect();
        // From User:
        assert!(
            field_names.contains(&"username"),
            "Missing User field 'username': {:?}",
            field_names
        );
        assert!(
            field_names.contains(&"email"),
            "Missing User field 'email': {:?}",
            field_names
        );
        // From PetBase:
        assert!(
            field_names.contains(&"species"),
            "Missing PetBase field 'species': {:?}",
            field_names
        );
        // From FileMetadata:
        assert!(
            field_names.contains(&"filename"),
            "Missing FileMetadata field 'filename': {:?}",
            field_names
        );

        // Composition should still be present
        match &model.composition {
            Some(Composition::AnyOf(variants)) => {
                assert_eq!(variants.len(), 3);
            }
            other => panic!("Expected AnyOf composition, got {:?}", other),
        }
    }

    #[test]
    fn test_oneof_ref_variants_have_fields() {
        // Pet is oneOf [Dog, Cat, Bird] — all $ref variants.
        // Should merge fields from all variants (plus their allOf bases).
        let api = load_kitchen_sink_api();
        let model = build_schema_model(&api, "Pet", false, 5).unwrap();

        assert!(
            !model.fields.is_empty(),
            "oneOf with $ref variants should have merged fields"
        );

        let field_names: Vec<&str> = model.fields.iter().map(|f| f.name.as_str()).collect();
        // From PetBase (shared via allOf):
        assert!(
            field_names.contains(&"name"),
            "Missing PetBase field 'name': {:?}",
            field_names
        );
        assert!(
            field_names.contains(&"species"),
            "Missing PetBase field 'species': {:?}",
            field_names
        );
        // From Dog:
        assert!(
            field_names.contains(&"breed"),
            "Missing Dog field 'breed': {:?}",
            field_names
        );
        // From Cat:
        assert!(
            field_names.contains(&"indoor"),
            "Missing Cat field 'indoor': {:?}",
            field_names
        );
        // From Bird:
        assert!(
            field_names.contains(&"can_fly"),
            "Missing Bird field 'can_fly': {:?}",
            field_names
        );
    }

    #[test]
    fn test_oneof_inline_primitives_no_fields() {
        // InsecureSsl is oneOf [boolean, string] — inline primitive types.
        // Should have no fields (primitives have no object properties).
        let api = load_kitchen_sink_api();
        let model = build_schema_model(&api, "InsecureSsl", false, 5).unwrap();
        assert!(
            model.fields.is_empty(),
            "oneOf with inline primitives should have no fields"
        );
    }
}
