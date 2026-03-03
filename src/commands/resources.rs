use std::collections::BTreeMap;

use crate::models::resource::{
    humanize_tag_name, is_alpha_tag, is_deprecated_tag, slugify, Endpoint, Field, ResourceGroup,
};
use crate::spec;

pub fn extract_resource_groups(api: &openapiv3::OpenAPI) -> Vec<ResourceGroup> {
    // Build initial groups from the global tags list
    let mut groups: BTreeMap<String, ResourceGroup> = BTreeMap::new();
    for tag in &api.tags {
        let slug = slugify(&tag.name);
        groups.insert(
            tag.name.clone(),
            ResourceGroup {
                slug,
                display_name: humanize_tag_name(&tag.name),
                description: tag.description.clone(),
                is_deprecated: is_deprecated_tag(&tag.name),
                is_alpha: is_alpha_tag(&tag.name),
                endpoints: Vec::new(),
            },
        );
    }

    // Iterate all paths and operations, assigning endpoints to groups by tag
    for (path_str, path_item_ref) in &api.paths.paths {
        let path_item = match path_item_ref {
            openapiv3::ReferenceOr::Item(item) => item,
            openapiv3::ReferenceOr::Reference { .. } => continue,
        };

        let methods: &[(&str, &Option<openapiv3::Operation>)] = &[
            ("GET", &path_item.get),
            ("POST", &path_item.post),
            ("PUT", &path_item.put),
            ("DELETE", &path_item.delete),
            ("PATCH", &path_item.patch),
            ("HEAD", &path_item.head),
            ("OPTIONS", &path_item.options),
            ("TRACE", &path_item.trace),
        ];

        for &(method, op_opt) in methods {
            let op = match op_opt {
                Some(op) => op,
                None => continue,
            };

            let is_deprecated = op.deprecated;
            let is_alpha = matches!(
                op.extensions.get("x-alpha"),
                Some(serde_json::Value::Bool(true))
            );

            let endpoint = Endpoint {
                method: method.to_string(),
                path: path_str.clone(),
                summary: op.summary.clone(),
                description: op.description.clone(),
                is_deprecated,
                is_alpha,
                external_docs: None,
                parameters: Vec::new(),
                request_body: None,
                responses: Vec::new(),
                security_schemes: Vec::new(),
                callbacks: Vec::new(),
                links: Vec::new(),
                drill_deeper: Vec::new(),
            };

            if op.tags.is_empty() {
                continue;
            }

            for tag_name in &op.tags {
                // Get or create the group for this tag
                let group = groups
                    .entry(tag_name.clone())
                    .or_insert_with(|| ResourceGroup {
                        slug: slugify(tag_name),
                        display_name: humanize_tag_name(tag_name),
                        description: None,
                        is_deprecated: is_deprecated_tag(tag_name),
                        is_alpha: is_alpha_tag(tag_name),
                        endpoints: Vec::new(),
                    });
                group.endpoints.push(endpoint.clone());
            }
        }
    }

    // Sort by slug (BTreeMap is already sorted by key, but key is tag_name not slug)
    let mut result: Vec<ResourceGroup> = groups.into_values().collect();
    result.sort_by(|a, b| a.slug.cmp(&b.slug));

    // Fallback: if no group has endpoints, use path-prefix grouping
    let has_endpoints = result.iter().any(|g| !g.endpoints.is_empty());
    if !has_endpoints {
        return extract_path_prefix_groups(api);
    }

    result
}

/// Extract the meaningful first path segment, stripping common API version prefixes.
/// e.g., "/v1/customers/{id}" → "customers", "/api/v2/orders" → "orders"
pub(crate) fn path_prefix_group_name(path: &str) -> Option<String> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    for seg in &segments {
        // Skip version-like segments and "api" prefix
        let lower = seg.to_lowercase();
        if lower == "api"
            || (lower.starts_with('v') && lower[1..].chars().all(|c| c.is_ascii_digit()))
        {
            continue;
        }
        // Skip path parameters
        if seg.starts_with('{') {
            continue;
        }
        return Some(seg.to_string());
    }
    None
}

fn extract_path_prefix_groups(api: &openapiv3::OpenAPI) -> Vec<ResourceGroup> {
    let mut groups: BTreeMap<String, ResourceGroup> = BTreeMap::new();

    for (path_str, path_item_ref) in &api.paths.paths {
        let path_item = match path_item_ref {
            openapiv3::ReferenceOr::Item(item) => item,
            openapiv3::ReferenceOr::Reference { .. } => continue,
        };

        let group_name = match path_prefix_group_name(path_str) {
            Some(name) => name,
            None => continue,
        };

        let methods: &[(&str, &Option<openapiv3::Operation>)] = &[
            ("GET", &path_item.get),
            ("POST", &path_item.post),
            ("PUT", &path_item.put),
            ("DELETE", &path_item.delete),
            ("PATCH", &path_item.patch),
            ("HEAD", &path_item.head),
            ("OPTIONS", &path_item.options),
            ("TRACE", &path_item.trace),
        ];

        for &(method, op_opt) in methods {
            let op = match op_opt {
                Some(op) => op,
                None => continue,
            };

            let is_deprecated = op.deprecated;
            let is_alpha = matches!(
                op.extensions.get("x-alpha"),
                Some(serde_json::Value::Bool(true))
            );

            let endpoint = Endpoint {
                method: method.to_string(),
                path: path_str.clone(),
                summary: op.summary.clone(),
                description: op.description.clone(),
                is_deprecated,
                is_alpha,
                external_docs: None,
                parameters: Vec::new(),
                request_body: None,
                responses: Vec::new(),
                security_schemes: Vec::new(),
                callbacks: Vec::new(),
                links: Vec::new(),
                drill_deeper: Vec::new(),
            };

            let slug = slugify(&group_name);
            let group = groups
                .entry(group_name.clone())
                .or_insert_with(|| ResourceGroup {
                    slug: slug.clone(),
                    display_name: humanize_tag_name(&group_name),
                    description: None,
                    is_deprecated: false,
                    is_alpha: false,
                    endpoints: Vec::new(),
                });
            group.endpoints.push(endpoint);
        }
    }

    let mut result: Vec<ResourceGroup> = groups.into_values().collect();
    result.sort_by(|a, b| a.slug.cmp(&b.slug));
    result
}

pub fn find_resource_group(groups: &[ResourceGroup], slug: &str) -> Option<usize> {
    let slug_lower = slug.to_lowercase();
    groups
        .iter()
        .position(|g| g.slug.to_lowercase() == slug_lower)
}

pub fn get_resource_detail(api: &openapiv3::OpenAPI, slug: &str) -> Option<ResourceGroup> {
    let groups = extract_resource_groups(api);
    let idx = find_resource_group(&groups, slug)?;
    Some(groups.into_iter().nth(idx).unwrap())
}

/// Extract object-like properties from a schema, handling both explicit `type: object`
/// and implicit objects (schemas with `properties` but no `type` field, which openapiv3
/// parses as `SchemaKind::Any`).
#[allow(clippy::type_complexity)]
pub fn extract_object_properties(
    schema: &openapiv3::Schema,
) -> Option<(
    &indexmap::IndexMap<String, openapiv3::ReferenceOr<Box<openapiv3::Schema>>>,
    Vec<String>,
)> {
    match &schema.schema_kind {
        openapiv3::SchemaKind::Type(openapiv3::Type::Object(obj)) => {
            Some((&obj.properties, obj.required.clone()))
        }
        openapiv3::SchemaKind::Any(any) if !any.properties.is_empty() => {
            Some((&any.properties, any.required.clone()))
        }
        _ => None,
    }
}

pub fn build_fields(
    api: &openapiv3::OpenAPI,
    schema: &openapiv3::Schema,
    required_fields: &[String],
) -> Vec<Field> {
    // Handle allOf: flatten all constituent schemas
    if let openapiv3::SchemaKind::AllOf { all_of } = &schema.schema_kind {
        let mut all_fields: Vec<Field> = Vec::new();
        let mut merged_required: Vec<String> = required_fields.to_vec();

        for ref_or in all_of {
            let resolved = match ref_or {
                openapiv3::ReferenceOr::Item(s) => Some(s as &openapiv3::Schema),
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
            };

            if let Some(resolved) = resolved {
                // Collect required fields from this constituent
                if let Some((_, req)) = extract_object_properties(resolved) {
                    for r in &req {
                        if !merged_required.contains(r) {
                            merged_required.push(r.clone());
                        }
                    }
                }

                let sub_fields = build_fields(api, resolved, &merged_required);
                for field in sub_fields {
                    // Dedup: later wins
                    all_fields.retain(|f| f.name != field.name);
                    all_fields.push(field);
                }
            }
        }

        return all_fields;
    }

    let properties = match extract_object_properties(schema) {
        Some((props, _)) => props,
        None => return Vec::new(),
    };

    let mut fields = Vec::new();

    for (name, ref_or) in properties {
        let (resolved_schema, schema_name): (Option<&openapiv3::Schema>, Option<&str>) =
            match ref_or {
                openapiv3::ReferenceOr::Item(boxed) => (Some(boxed), None),
                openapiv3::ReferenceOr::Reference { reference } => {
                    match spec::schema_name_from_ref(reference) {
                        Some(sname) => {
                            let resolved = api
                                .components
                                .as_ref()
                                .and_then(|c| c.schemas.get(sname))
                                .and_then(|s| match s {
                                    openapiv3::ReferenceOr::Item(schema) => {
                                        Some(schema as &openapiv3::Schema)
                                    }
                                    _ => None,
                                });
                            (resolved, Some(sname))
                        }
                        None => (None, None),
                    }
                }
            };

        let resolved = match resolved_schema {
            Some(s) => s,
            None => continue,
        };

        let type_display = if let Some(sname) = schema_name {
            sname.to_string()
        } else {
            format_type_display(&resolved.schema_kind)
        };

        let enum_values = extract_enum_values(&resolved.schema_kind);

        // For array-of-ref fields, capture the item schema name for --expand support.
        // Direct $ref properties already have schema_name set, but array wrappers
        // (type: array, items: { $ref }) are inline Items with schema_name = None.
        let effective_schema_name = schema_name.map(|s| s.to_string()).or_else(|| {
            if let openapiv3::SchemaKind::Type(openapiv3::Type::Array(arr)) = &resolved.schema_kind
            {
                if let Some(openapiv3::ReferenceOr::Reference { reference }) = arr.items.as_ref() {
                    return spec::schema_name_from_ref(reference.as_str()).map(|s| s.to_string());
                }
            }
            None
        });

        fields.push(Field {
            name: name.clone(),
            type_display,
            required: required_fields.contains(name),
            optional: !required_fields.contains(name),
            nullable: resolved.schema_data.nullable,
            read_only: resolved.schema_data.read_only,
            write_only: resolved.schema_data.write_only,
            deprecated: resolved.schema_data.deprecated,
            description: resolved.schema_data.description.clone(),
            enum_values,
            constraints: extract_constraints(&resolved.schema_kind),
            default_value: resolved.schema_data.default.clone(),
            example: resolved.schema_data.example.clone(),
            nested_schema_name: effective_schema_name,
            nested_fields: Vec::new(),
        });
    }

    fields
}

/// Extract the properties map and required list from a schema, handling both
/// explicit `Type::Object` and implicit `AnySchema` (schemas with properties but
/// no declared type, common in some generated specs).
/// Recursively populate `nested_fields` for inline object properties.
///
/// `expand_fields_pub` (in schemas.rs) handles expansion of fields that reference
/// named schemas via `nested_schema_name`. But inline objects — those defined directly
/// in the schema with `type: object` and `properties` rather than via `$ref` — have
/// no name to look up. This function fills that gap by walking the parent schema's
/// properties and recursively building nested fields for any inline object sub-schemas.
fn expand_inline_objects(
    api: &openapiv3::OpenAPI,
    schema: &openapiv3::Schema,
    fields: &mut [Field],
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        return;
    }

    let (properties, _) = match extract_object_properties(schema) {
        Some(pair) => pair,
        None => return,
    };

    for field in fields.iter_mut() {
        // Skip fields that already have a nested_schema_name — those are handled
        // by expand_fields_pub which looks up named schemas in components.
        if field.nested_schema_name.is_some() {
            continue;
        }

        // Skip fields that already have nested_fields populated (e.g. from a previous pass).
        if !field.nested_fields.is_empty() {
            continue;
        }

        // Look up this field's property in the parent schema
        let ref_or = match properties.get(&field.name) {
            Some(r) => r,
            None => continue,
        };

        // Resolve the property schema (follow $ref if needed)
        let resolved: Option<&openapiv3::Schema> = match ref_or {
            openapiv3::ReferenceOr::Item(boxed) => Some(boxed),
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
        };

        let resolved = match resolved {
            Some(s) => s,
            None => continue,
        };

        // Check if this property is an inline object with sub-properties
        let (sub_properties, sub_required) = match extract_object_properties(resolved) {
            Some(pair) => pair,
            None => continue,
        };
        if sub_properties.is_empty() {
            continue;
        }

        // Build nested fields from the inline object's properties
        let mut nested = build_fields(api, resolved, &sub_required);

        // Recursively expand any inline objects within the nested fields
        expand_inline_objects(api, resolved, &mut nested, depth + 1, max_depth);

        // Also let expand_fields_pub handle any $ref-based nested schemas within
        {
            use crate::commands::schemas::expand_fields_pub;
            let mut visited = std::collections::HashSet::new();
            nested = expand_fields_pub(api, nested, &mut visited, depth + 1, max_depth);
        }

        field.nested_fields = nested;
    }
}

fn extract_constraints(kind: &openapiv3::SchemaKind) -> Vec<String> {
    let mut c = Vec::new();
    match kind {
        openapiv3::SchemaKind::Type(openapiv3::Type::String(s)) => {
            if let Some(min) = s.min_length {
                c.push(format!("min:{}", min));
            }
            if let Some(max) = s.max_length {
                c.push(format!("max:{}", max));
            }
            if let Some(ref pat) = s.pattern {
                c.push(format!("pattern:{}", pat));
            }
        }
        openapiv3::SchemaKind::Type(openapiv3::Type::Integer(i)) => {
            if let Some(min) = i.minimum {
                if i.exclusive_minimum {
                    c.push(format!(">{}", min));
                } else {
                    c.push(format!("min:{}", min));
                }
            }
            if let Some(max) = i.maximum {
                if i.exclusive_maximum {
                    c.push(format!("<{}", max));
                } else {
                    c.push(format!("max:{}", max));
                }
            }
            if let Some(mo) = i.multiple_of {
                c.push(format!("multipleOf:{}", mo));
            }
        }
        openapiv3::SchemaKind::Type(openapiv3::Type::Number(n)) => {
            if let Some(min) = n.minimum {
                if n.exclusive_minimum {
                    c.push(format!(">{}", min));
                } else {
                    c.push(format!("min:{}", min));
                }
            }
            if let Some(max) = n.maximum {
                if n.exclusive_maximum {
                    c.push(format!("<{}", max));
                } else {
                    c.push(format!("max:{}", max));
                }
            }
            if let Some(mo) = n.multiple_of {
                c.push(format!("multipleOf:{}", mo));
            }
        }
        openapiv3::SchemaKind::Type(openapiv3::Type::Array(a)) => {
            if let Some(min) = a.min_items {
                c.push(format!("minItems:{}", min));
            }
            if let Some(max) = a.max_items {
                c.push(format!("maxItems:{}", max));
            }
            if a.unique_items {
                c.push("uniqueItems".to_string());
            }
        }
        openapiv3::SchemaKind::Type(openapiv3::Type::Object(o)) => {
            if let Some(min) = o.min_properties {
                c.push(format!("minProperties:{}", min));
            }
            if let Some(max) = o.max_properties {
                c.push(format!("maxProperties:{}", max));
            }
        }
        _ => {}
    }
    c
}

fn format_type_display(kind: &openapiv3::SchemaKind) -> String {
    match kind {
        openapiv3::SchemaKind::Type(t) => match t {
            openapiv3::Type::String(s) => match &s.format {
                openapiv3::VariantOrUnknownOrEmpty::Item(openapiv3::StringFormat::Binary) => {
                    "binary".to_string()
                }
                openapiv3::VariantOrUnknownOrEmpty::Item(fmt) => {
                    format!("string/{}", format_variant_name(fmt))
                }
                openapiv3::VariantOrUnknownOrEmpty::Unknown(s) => {
                    format!("string/{}", s)
                }
                openapiv3::VariantOrUnknownOrEmpty::Empty => "string".to_string(),
            },
            openapiv3::Type::Integer(_) => "integer".to_string(),
            openapiv3::Type::Number(_) => "number".to_string(),
            openapiv3::Type::Boolean(_) => "boolean".to_string(),
            openapiv3::Type::Array(arr) => match &arr.items {
                Some(openapiv3::ReferenceOr::Reference { reference }) => {
                    let name = spec::schema_name_from_ref(reference.as_str()).unwrap_or("object");
                    format!("{}[]", name)
                }
                Some(openapiv3::ReferenceOr::Item(boxed)) => {
                    let item_type = format_type_display(&boxed.schema_kind);
                    format!("{}[]", item_type)
                }
                None => "array".to_string(),
            },
            openapiv3::Type::Object(_) => "object".to_string(),
        },
        // AnySchema: no explicit type constraint. Recover from typ/format fields.
        // Covers cases like `format: boolean` without `type: boolean` in some generated specs.
        openapiv3::SchemaKind::Any(any) => {
            if let Some(ref typ) = any.typ {
                return typ.clone();
            }
            "object".to_string()
        }
        _ => "object".to_string(),
    }
}

fn format_variant_name(fmt: &openapiv3::StringFormat) -> String {
    match fmt {
        openapiv3::StringFormat::DateTime => "date-time".to_string(),
        openapiv3::StringFormat::Date => "date".to_string(),
        openapiv3::StringFormat::Password => "password".to_string(),
        openapiv3::StringFormat::Byte => "byte".to_string(),
        openapiv3::StringFormat::Binary => "binary".to_string(),
    }
}

fn extract_enum_values(kind: &openapiv3::SchemaKind) -> Vec<String> {
    match kind {
        openapiv3::SchemaKind::Type(openapiv3::Type::String(s)) => {
            s.enumeration.iter().filter_map(|v| v.clone()).collect()
        }
        openapiv3::SchemaKind::Type(openapiv3::Type::Integer(i)) => i
            .enumeration
            .iter()
            .filter_map(|v| v.map(|n| n.to_string()))
            .collect(),
        openapiv3::SchemaKind::Type(openapiv3::Type::Number(n)) => n
            .enumeration
            .iter()
            .filter_map(|v| {
                v.map(|f| {
                    if f.fract() == 0.0 {
                        format!("{}", f as i64)
                    } else {
                        format!("{}", f)
                    }
                })
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn resolve_path_item<'a>(
    api: &'a openapiv3::OpenAPI,
    path: &str,
) -> Option<&'a openapiv3::PathItem> {
    match api.paths.paths.get(path)? {
        openapiv3::ReferenceOr::Item(item) => Some(item),
        openapiv3::ReferenceOr::Reference { .. } => None,
    }
}

fn resolve_operation<'a>(
    path_item: &'a openapiv3::PathItem,
    method: &str,
) -> Option<&'a openapiv3::Operation> {
    match method.to_uppercase().as_str() {
        "GET" => path_item.get.as_ref(),
        "POST" => path_item.post.as_ref(),
        "PUT" => path_item.put.as_ref(),
        "DELETE" => path_item.delete.as_ref(),
        "PATCH" => path_item.patch.as_ref(),
        "HEAD" => path_item.head.as_ref(),
        "OPTIONS" => path_item.options.as_ref(),
        "TRACE" => path_item.trace.as_ref(),
        _ => None,
    }
}

fn extract_security(api: &openapiv3::OpenAPI, operation: &openapiv3::Operation) -> Vec<String> {
    operation
        .security
        .as_ref()
        .or(api.security.as_ref())
        .map(|reqs| {
            reqs.iter()
                .flat_map(|req| req.keys().cloned())
                .collect::<Vec<String>>()
        })
        .unwrap_or_default()
}

fn extract_links_from_response(
    api: &openapiv3::OpenAPI,
    resp: &openapiv3::Response,
    bin_name: &str,
) -> Vec<crate::models::resource::ResponseLink> {
    use crate::models::resource::ResponseLink;

    resp.links
        .iter()
        .filter_map(|(link_name, link_ref)| {
            let link = match link_ref {
                openapiv3::ReferenceOr::Item(l) => l,
                openapiv3::ReferenceOr::Reference { .. } => return None,
            };

            let operation_id = match &link.operation {
                openapiv3::LinkOperation::OperationId(id) => id.clone(),
                openapiv3::LinkOperation::OperationRef(_) => return None,
            };

            let parameters: Vec<String> = link
                .parameters
                .iter()
                .map(|(k, v)| {
                    let val_str = match v {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    format!("{} = {}", k, val_str)
                })
                .collect();

            let drill_command = build_link_drill_command(api, &operation_id, bin_name);

            Some(ResponseLink {
                name: link_name.clone(),
                operation_id,
                parameters,
                description: link.description.clone(),
                drill_command,
            })
        })
        .collect()
}

fn build_link_drill_command(
    api: &openapiv3::OpenAPI,
    operation_id: &str,
    bin_name: &str,
) -> Option<String> {
    for (path_str, path_item_ref) in &api.paths.paths {
        let path_item = match path_item_ref {
            openapiv3::ReferenceOr::Item(item) => item,
            _ => continue,
        };
        let methods: &[(&str, &Option<openapiv3::Operation>)] = &[
            ("GET", &path_item.get),
            ("POST", &path_item.post),
            ("PUT", &path_item.put),
            ("DELETE", &path_item.delete),
            ("PATCH", &path_item.patch),
        ];
        for &(method, op_opt) in methods {
            if let Some(op) = op_opt {
                if op.operation_id.as_deref() == Some(operation_id) {
                    let slug = op.tags.first().map(|t| crate::models::resource::slugify(t));
                    if let Some(slug) = slug {
                        return Some(format!(
                            "{} resources {} {} {}",
                            bin_name, slug, method, path_str
                        ));
                    }
                }
            }
        }
    }
    None
}

/// Extract a human-readable schema ref string from a response media type's schema.
///
/// Handles four patterns:
/// 1. Direct $ref → "SchemaName"
/// 2. Inline array with $ref items → "SchemaName[]"
/// 3. Inline anyOf/oneOf with $ref variants → "SchemaA | SchemaB"
/// 4. Inline object with pagination wrapper → "SchemaName[] (list)"
fn resolve_response_schema_ref(
    schema_ref: &openapiv3::ReferenceOr<openapiv3::Schema>,
) -> Option<String> {
    match schema_ref {
        openapiv3::ReferenceOr::Reference { reference } => {
            spec::schema_name_from_ref(reference).map(|s| s.to_string())
        }
        openapiv3::ReferenceOr::Item(schema) => {
            match &schema.schema_kind {
                openapiv3::SchemaKind::Type(openapiv3::Type::Array(arr)) => match &arr.items {
                    Some(openapiv3::ReferenceOr::Reference { reference }) => {
                        spec::schema_name_from_ref(reference).map(|s| format!("{}[]", s))
                    }
                    Some(openapiv3::ReferenceOr::Item(item_schema)) => Some(format!(
                        "{}[]",
                        format_type_display(&item_schema.schema_kind)
                    )),
                    None => Some("array".to_string()),
                },
                openapiv3::SchemaKind::AnyOf { any_of } => {
                    let names = extract_composition_variant_names(any_of);
                    if names.is_empty() {
                        None
                    } else {
                        Some(names.join(" | "))
                    }
                }
                openapiv3::SchemaKind::OneOf { one_of } => {
                    let names = extract_composition_variant_names(one_of);
                    if names.is_empty() {
                        None
                    } else {
                        Some(names.join(" | "))
                    }
                }
                // Inline object: check for pagination wrapper pattern (data array with $ref items)
                openapiv3::SchemaKind::Type(openapiv3::Type::Object(obj)) => {
                    if let Some(data_schema_ref) = obj.properties.get("data") {
                        match data_schema_ref {
                            openapiv3::ReferenceOr::Item(data_schema) => {
                                if let openapiv3::SchemaKind::Type(openapiv3::Type::Array(arr)) =
                                    &data_schema.schema_kind
                                {
                                    match &arr.items {
                                        Some(openapiv3::ReferenceOr::Reference { reference }) => {
                                            return spec::schema_name_from_ref(reference)
                                                .map(|s| format!("{}[] (list)", s));
                                        }
                                        Some(openapiv3::ReferenceOr::Item(item_schema)) => {
                                            return Some(format!(
                                                "{}[] (list)",
                                                format_type_display(&item_schema.schema_kind)
                                            ));
                                        }
                                        None => {
                                            return Some("array (list)".to_string());
                                        }
                                    }
                                }
                            }
                            openapiv3::ReferenceOr::Reference { reference } => {
                                return spec::schema_name_from_ref(reference)
                                    .map(|s| format!("{} (list)", s));
                            }
                        }
                    }
                    Some("object".to_string())
                }
                _ => None,
            }
        }
    }
}

/// Extract variant names from anyOf/oneOf schema references.
/// For $ref variants, returns the schema name; for inline types, returns the type name.
fn extract_composition_variant_names(
    variants: &[openapiv3::ReferenceOr<openapiv3::Schema>],
) -> Vec<String> {
    variants
        .iter()
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

fn extract_responses(
    api: &openapiv3::OpenAPI,
    operation: &openapiv3::Operation,
    expand: bool,
    bin_name: &str,
) -> Vec<crate::models::resource::Response> {
    use crate::models::resource::{Response, ResponseHeader};
    let mut responses = Vec::new();
    for (status, resp_ref) in &operation.responses.responses {
        let status_code = match status {
            openapiv3::StatusCode::Code(code) => code.to_string(),
            openapiv3::StatusCode::Range(range) => format!("{}XX", range),
        };

        let resp = match resp_ref {
            openapiv3::ReferenceOr::Item(r) => r,
            openapiv3::ReferenceOr::Reference { .. } => continue,
        };

        let (schema_ref_name, example) = resp
            .content
            .get("application/json")
            .map(|media| {
                let schema_ref = media.schema.as_ref().and_then(resolve_response_schema_ref);
                (schema_ref, media.example.clone())
            })
            .unwrap_or((None, None));

        let headers: Vec<ResponseHeader> = resp
            .headers
            .iter()
            .filter_map(|(name, href)| {
                let header = match href {
                    openapiv3::ReferenceOr::Item(h) => h,
                    openapiv3::ReferenceOr::Reference { .. } => return None,
                };
                let type_display = match &header.format {
                    openapiv3::ParameterSchemaOrContent::Schema(s) => match s {
                        openapiv3::ReferenceOr::Item(schema) => {
                            format_type_display(&schema.schema_kind)
                        }
                        openapiv3::ReferenceOr::Reference { reference } => {
                            spec::schema_name_from_ref(reference)
                                .unwrap_or("object")
                                .to_string()
                        }
                    },
                    _ => "string".to_string(),
                };
                Some(ResponseHeader {
                    name: name.clone(),
                    type_display,
                    description: header.description.clone(),
                })
            })
            .collect();

        let links = extract_links_from_response(api, resp, bin_name);

        let fields = if expand {
            expand_response_schema(api, &schema_ref_name)
        } else {
            Vec::new()
        };

        responses.push(Response {
            status_code,
            description: resp.description.clone(),
            schema_ref: schema_ref_name,
            example,
            headers,
            links,
            fields,
        });
    }

    // Handle the default response (not included in operation.responses.responses)
    if let Some(openapiv3::ReferenceOr::Item(resp)) = operation.responses.default.as_ref() {
        let (schema_ref_name, example) = resp
            .content
            .get("application/json")
            .map(|media| {
                let schema_ref = media.schema.as_ref().and_then(resolve_response_schema_ref);
                (schema_ref, media.example.clone())
            })
            .unwrap_or((None, None));

        let headers: Vec<ResponseHeader> = resp
            .headers
            .iter()
            .filter_map(|(name, href)| {
                let header = match href {
                    openapiv3::ReferenceOr::Item(h) => h,
                    openapiv3::ReferenceOr::Reference { .. } => return None,
                };
                let type_display = match &header.format {
                    openapiv3::ParameterSchemaOrContent::Schema(s) => match s {
                        openapiv3::ReferenceOr::Item(schema) => {
                            format_type_display(&schema.schema_kind)
                        }
                        openapiv3::ReferenceOr::Reference { reference } => {
                            spec::schema_name_from_ref(reference)
                                .unwrap_or("object")
                                .to_string()
                        }
                    },
                    _ => "string".to_string(),
                };
                Some(ResponseHeader {
                    name: name.clone(),
                    type_display,
                    description: header.description.clone(),
                })
            })
            .collect();

        let links = extract_links_from_response(api, resp, bin_name);

        let fields = if expand {
            expand_response_schema(api, &schema_ref_name)
        } else {
            Vec::new()
        };

        responses.push(Response {
            status_code: "default".to_string(),
            description: resp.description.clone(),
            schema_ref: schema_ref_name,
            example,
            headers,
            links,
            fields,
        });
    }

    responses
}

/// Resolve a response's schema_ref into expanded fields using the same
/// build_fields + expand_fields_pub infrastructure used for request bodies.
///
/// For anyOf/oneOf responses (schema_ref = "A | B"), expands all variants
/// and merges their fields (deduplicating by name, last variant wins).
fn expand_response_schema(
    api: &openapiv3::OpenAPI,
    schema_ref: &Option<String>,
) -> Vec<crate::models::resource::Field> {
    use crate::commands::schemas::{expand_fields_pub, find_schema};

    let name = match schema_ref {
        Some(n) => {
            // Strip suffixes like "[] (list)", "[]", " (list)" to get the bare schema name
            let n = n.strip_suffix(" (list)").unwrap_or(n);
            n.strip_suffix("[]").unwrap_or(n)
        }
        None => return Vec::new(),
    };

    // Handle anyOf/oneOf pipe-separated variants
    let variant_names: Vec<&str> = name.split(" | ").collect();

    let mut all_fields = Vec::new();
    for variant_name in &variant_names {
        let (_key, schema) = match find_schema(api, variant_name) {
            Some(pair) => pair,
            None => continue,
        };

        let required = extract_object_properties(schema)
            .map(|(_, req)| req)
            .unwrap_or_default();

        let mut fields = build_fields(api, schema, &required);
        expand_inline_objects(api, schema, &mut fields, 1, 5);
        let mut visited = std::collections::HashSet::new();
        visited.insert(variant_name.to_string());
        let expanded = expand_fields_pub(api, fields, &mut visited, 1, 5);

        // Merge: dedup by name (later variant wins)
        for field in expanded {
            all_fields.retain(|f: &Field| f.name != field.name);
            all_fields.push(field);
        }
    }

    all_fields
}

fn merge_parameters(
    api: &openapiv3::OpenAPI,
    path_item: &openapiv3::PathItem,
    operation: &openapiv3::Operation,
) -> Vec<crate::models::resource::Parameter> {
    use crate::models::resource::{Parameter, ParameterLocation};
    let mut params_map: std::collections::BTreeMap<String, Parameter> =
        std::collections::BTreeMap::new();

    let all_param_refs: Vec<&openapiv3::ReferenceOr<openapiv3::Parameter>> = path_item
        .parameters
        .iter()
        .chain(operation.parameters.iter())
        .collect();

    for param_ref in all_param_refs {
        let param = match param_ref {
            openapiv3::ReferenceOr::Item(p) => p,
            openapiv3::ReferenceOr::Reference { reference } => {
                // Extract param name from "#/components/parameters/foo"
                let param_name = match reference.strip_prefix("#/components/parameters/") {
                    Some(name) => name,
                    None => continue,
                };
                // Look up in components
                match api
                    .components
                    .as_ref()
                    .and_then(|c| c.parameters.get(param_name))
                    .and_then(|p| match p {
                        openapiv3::ReferenceOr::Item(p) => Some(p),
                        _ => None,
                    }) {
                    Some(p) => p,
                    None => continue,
                }
            }
        };

        let data = match param {
            openapiv3::Parameter::Query { parameter_data, .. } => {
                (parameter_data, ParameterLocation::Query)
            }
            openapiv3::Parameter::Path { parameter_data, .. } => {
                (parameter_data, ParameterLocation::Path)
            }
            openapiv3::Parameter::Header { parameter_data, .. } => {
                (parameter_data, ParameterLocation::Header)
            }
            _ => continue,
        };

        let (pdata, location) = data;

        // Extract schema type from parameter format
        let (schema_type, format, enum_values) = extract_param_schema_info(api, &pdata.format);

        params_map.insert(
            pdata.name.clone(),
            Parameter {
                name: pdata.name.clone(),
                location,
                required: pdata.required,
                schema_type,
                format,
                description: pdata.description.clone(),
                enum_values,
            },
        );
    }

    params_map.into_values().collect()
}

fn extract_request_body(
    api: &openapiv3::OpenAPI,
    operation: &openapiv3::Operation,
    expand: bool,
) -> Option<crate::models::resource::RequestBody> {
    use crate::models::resource::RequestBody;

    let rb_ref = operation.request_body.as_ref()?;
    let rb = match rb_ref {
        openapiv3::ReferenceOr::Item(rb) => rb,
        openapiv3::ReferenceOr::Reference { reference } => {
            let name = reference.strip_prefix("#/components/requestBodies/")?;
            let components = api.components.as_ref()?;
            match components.request_bodies.get(name)? {
                openapiv3::ReferenceOr::Item(rb) => rb,
                openapiv3::ReferenceOr::Reference { .. } => return None,
            }
        }
    };

    // Priority order: JSON first (existing behavior), then multipart, then form-encoded, then any
    let priority = [
        "application/json",
        "multipart/form-data",
        "application/x-www-form-urlencoded",
    ];

    let (content_type, media) = priority
        .iter()
        .find_map(|ct| rb.content.get(*ct).map(|m| (*ct, m)))
        .or_else(|| rb.content.iter().next().map(|(ct, m)| (ct.as_str(), m)))?;

    // NOTE: multipart/form-data per-field encoding overrides (media.encoding) are intentionally
    // not extracted. They describe wire-level content type, not the field's logical schema type.

    let schema_ref = media.schema.as_ref()?;

    let (schema, top_ref_name): (&openapiv3::Schema, Option<String>) = match schema_ref {
        openapiv3::ReferenceOr::Item(s) => (s, None),
        openapiv3::ReferenceOr::Reference { reference } => {
            let sname = spec::schema_name_from_ref(reference)?;
            let components = api.components.as_ref()?;
            match components.schemas.get(sname)? {
                openapiv3::ReferenceOr::Item(s) => (s, Some(sname.to_string())),
                _ => return None,
            }
        }
    };

    let example = media.example.clone();

    // OneOf/AnyOf request body: surface variant names instead of empty fields
    let oneof_variants: &[openapiv3::ReferenceOr<openapiv3::Schema>] = match &schema.schema_kind {
        openapiv3::SchemaKind::OneOf { one_of } => one_of.as_slice(),
        openapiv3::SchemaKind::AnyOf { any_of } => any_of.as_slice(),
        _ => &[],
    };
    let options: Vec<String> = oneof_variants
        .iter()
        .filter_map(|r| match r {
            openapiv3::ReferenceOr::Reference { reference } => {
                spec::schema_name_from_ref(reference).map(|s| s.to_string())
            }
            _ => None,
        })
        .collect();

    if !options.is_empty() {
        return Some(RequestBody {
            content_type: content_type.to_string(),
            fields: Vec::new(),
            options,
            schema_ref: None,
            example,
            array_item_type: None,
        });
    }

    // Handle array request bodies: type: array with items.$ref
    if let openapiv3::SchemaKind::Type(openapiv3::Type::Array(arr)) = &schema.schema_kind {
        if let Some(ref items_ref) = arr.items {
            let (item_schema, item_name): (Option<&openapiv3::Schema>, Option<String>) =
                match items_ref {
                    openapiv3::ReferenceOr::Reference { reference } => {
                        let sname = spec::schema_name_from_ref(reference);
                        let resolved = sname.and_then(|n| {
                            api.components
                                .as_ref()
                                .and_then(|c| c.schemas.get(n))
                                .and_then(|s| match s {
                                    openapiv3::ReferenceOr::Item(schema) => {
                                        Some(schema as &openapiv3::Schema)
                                    }
                                    _ => None,
                                })
                        });
                        (resolved, sname.map(|s| s.to_string()))
                    }
                    openapiv3::ReferenceOr::Item(boxed) => {
                        (Some(boxed as &openapiv3::Schema), None)
                    }
                };

            if let Some(item_schema) = item_schema {
                let required: Vec<String> = extract_object_properties(item_schema)
                    .map(|(_, req)| req)
                    .unwrap_or_default();

                let mut fields = build_fields(api, item_schema, &required);

                if expand {
                    expand_inline_objects(api, item_schema, &mut fields, 1, 5);
                    use crate::commands::schemas::expand_fields_pub;
                    let mut visited = std::collections::HashSet::new();
                    fields = expand_fields_pub(api, fields, &mut visited, 1, 5);
                }

                return Some(RequestBody {
                    content_type: content_type.to_string(),
                    fields,
                    options: Vec::new(),
                    schema_ref: item_name.clone(),
                    example,
                    array_item_type: item_name,
                });
            }
        }
    }

    let required: Vec<String> = extract_object_properties(schema)
        .map(|(_, req)| req)
        .unwrap_or_default();

    let mut fields = build_fields(api, schema, &required);

    if expand {
        expand_inline_objects(api, schema, &mut fields, 1, 5);
        use crate::commands::schemas::expand_fields_pub;
        let mut visited = std::collections::HashSet::new();
        fields = expand_fields_pub(api, fields, &mut visited, 1, 5);
    }

    Some(RequestBody {
        content_type: content_type.to_string(),
        fields,
        options: Vec::new(),
        schema_ref: top_ref_name,
        example,
        array_item_type: None,
    })
}

pub fn get_endpoint_detail(
    api: &openapiv3::OpenAPI,
    method: &str,
    path: &str,
    expand: bool,
    bin_name: &str,
) -> Option<Endpoint> {
    // 1. Find path item
    let path_item = resolve_path_item(api, path)?;

    // 2. Get operation by method
    let operation = resolve_operation(path_item, method)?;

    // 3. Merge parameters
    let parameters = merge_parameters(api, path_item, operation);

    // 4. Request body
    let request_body = extract_request_body(api, operation, expand);

    // 5. Responses
    let responses = extract_responses(api, operation, expand, bin_name);

    // 6. Security schemes
    let security_schemes = extract_security(api, operation);

    // 6b. Callbacks (extraction lives in commands/callbacks.rs)
    let callbacks =
        crate::commands::callbacks::extract_callbacks_from_operation(operation, method, path);

    // 7. Drill deeper hints
    let mut seen = std::collections::HashSet::new();
    let mut drill_deeper = Vec::new();

    // 2xx response schema refs: strip suffixes, split anyOf/oneOf pipes, skip "object"
    for resp in &responses {
        if resp.status_code.starts_with('2') {
            if let Some(ref name) = resp.schema_ref {
                let bare_name = name.strip_suffix(" (list)").unwrap_or(name);
                let bare_name = bare_name.strip_suffix("[]").unwrap_or(bare_name);
                // anyOf/oneOf responses produce "A | B" — split into individual drill hints
                for variant in bare_name.split(" | ") {
                    // Skip generic "object" — no schema to drill into
                    if variant != "object" && seen.insert(variant.to_string()) {
                        drill_deeper.push(format!("{} schemas {}", bin_name, variant));
                    }
                }
            }
        }
    }

    // Request body: oneOf/anyOf options OR concrete schema ref
    if let Some(ref body) = request_body {
        if !body.options.is_empty() {
            for name in &body.options {
                if seen.insert(name.clone()) {
                    drill_deeper.push(format!("{} schemas {}", bin_name, name));
                }
            }
        } else if let Some(ref name) = body.schema_ref {
            if seen.insert(name.clone()) {
                drill_deeper.push(format!("{} schemas {}", bin_name, name));
            }
        }
    }

    // 8. Aggregate links from all responses
    let endpoint_links: Vec<crate::models::resource::ResponseLink> = responses
        .iter()
        .flat_map(|r| r.links.iter().cloned())
        .collect();

    Some(Endpoint {
        method: method.to_uppercase(),
        path: path.to_string(),
        summary: operation.summary.clone(),
        description: operation.description.clone(),
        is_deprecated: operation.deprecated,
        is_alpha: matches!(
            operation.extensions.get("x-alpha"),
            Some(serde_json::Value::Bool(true))
        ),
        external_docs: None,
        parameters,
        request_body,
        responses,
        security_schemes,
        callbacks,
        links: endpoint_links,
        drill_deeper,
    })
}

fn extract_param_schema_info(
    api: &openapiv3::OpenAPI,
    format: &openapiv3::ParameterSchemaOrContent,
) -> (String, Option<String>, Vec<String>) {
    let schema_ref = match format {
        openapiv3::ParameterSchemaOrContent::Schema(s) => s,
        _ => return ("string".to_string(), None, Vec::new()),
    };

    let (schema, _name) = match schema_ref {
        openapiv3::ReferenceOr::Item(s) => (s as &openapiv3::Schema, None),
        openapiv3::ReferenceOr::Reference { reference } => {
            match spec::schema_name_from_ref(reference) {
                Some(sname) => match api.components.as_ref().and_then(|c| c.schemas.get(sname)) {
                    Some(openapiv3::ReferenceOr::Item(s)) => (s as &openapiv3::Schema, Some(sname)),
                    _ => return (sname.to_string(), None, Vec::new()),
                },
                None => return ("string".to_string(), None, Vec::new()),
            }
        }
    };

    let type_str = format_type_display(&schema.schema_kind);
    let enum_values = extract_enum_values(&schema.schema_kind);

    // Extract format
    let fmt = match &schema.schema_kind {
        openapiv3::SchemaKind::Type(openapiv3::Type::String(s)) => match &s.format {
            openapiv3::VariantOrUnknownOrEmpty::Item(f) => Some(format_variant_name(f)),
            openapiv3::VariantOrUnknownOrEmpty::Unknown(s) => Some(s.clone()),
            openapiv3::VariantOrUnknownOrEmpty::Empty => None,
        },
        _ => None,
    };

    (type_str, fmt, enum_values)
}

pub fn suggest_similar<'a>(groups: &'a [ResourceGroup], slug: &str) -> Vec<&'a str> {
    let slug_lower = slug.to_lowercase();
    groups
        .iter()
        .filter(|g| strsim::jaro_winkler(&slug_lower, &g.slug.to_lowercase()) > 0.8)
        .take(3)
        .map(|g| g.slug.as_str())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_petstore() -> openapiv3::OpenAPI {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let content =
            std::fs::read_to_string(manifest_dir.join("tests/fixtures/petstore.yaml")).unwrap();
        serde_yaml_ng::from_str(&content).unwrap()
    }

    fn load_kitchen_sink() -> openapiv3::OpenAPI {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let content =
            std::fs::read_to_string(manifest_dir.join("tests/fixtures/kitchen-sink.yaml")).unwrap();
        serde_yaml_ng::from_str(&content).unwrap()
    }

    #[test]
    fn test_extract_petstore_groups() {
        let api = load_petstore();
        let groups = extract_resource_groups(&api);

        let slugs: Vec<&str> = groups.iter().map(|g| g.slug.as_str()).collect();
        assert!(
            slugs.contains(&"pets"),
            "Expected 'pets' group, got: {:?}",
            slugs
        );
        assert!(
            slugs.contains(&"deprecated-pets"),
            "Expected 'deprecated-pets' group"
        );
        assert!(
            slugs.contains(&"experimental"),
            "Expected 'experimental' group"
        );

        // Check deprecated flag
        let deprecated = groups.iter().find(|g| g.slug == "deprecated-pets").unwrap();
        assert!(deprecated.is_deprecated);

        // Check alpha flag
        let alpha = groups.iter().find(|g| g.slug == "experimental").unwrap();
        assert!(alpha.is_alpha);

        // Pets group should have 5 endpoints (4 original + /animals POST)
        let pets = groups.iter().find(|g| g.slug == "pets").unwrap();
        assert_eq!(
            pets.endpoints.len(),
            6,
            "Expected 6 endpoints in pets group"
        );
    }

    #[test]
    fn test_get_endpoint_detail_post_pets() {
        let api = load_petstore();
        let endpoint = get_endpoint_detail(&api, "POST", "/pets", false, "phyllotaxis");
        assert!(endpoint.is_some(), "Expected POST /pets endpoint");
        let ep = endpoint.unwrap();

        assert_eq!(ep.method, "POST");
        assert_eq!(ep.path, "/pets");
        assert!(
            ep.request_body.is_some(),
            "POST /pets should have request body"
        );
        assert!(
            !ep.security_schemes.is_empty(),
            "POST /pets should have security"
        );
        assert!(
            ep.responses.iter().any(|r| r.status_code == "201"),
            "Expected 201 response"
        );
        assert!(
            ep.responses.iter().any(|r| r.status_code == "400"),
            "Expected 400 error response"
        );
    }

    #[test]
    fn test_get_endpoint_detail_not_found() {
        let api = load_petstore();
        let result = get_endpoint_detail(&api, "DELETE", "/nonexistent", false, "phyllotaxis");
        assert!(result.is_none());
    }

    #[test]
    fn test_build_fields_allof() {
        let api = load_petstore();

        let petlist_schema = api
            .components
            .as_ref()
            .unwrap()
            .schemas
            .get("PetList")
            .unwrap();
        let petlist_schema = match petlist_schema {
            openapiv3::ReferenceOr::Item(s) => s,
            _ => panic!("Expected item"),
        };

        let fields = build_fields(&api, petlist_schema, &[]);

        let field_names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
        assert!(
            field_names.contains(&"id"),
            "Expected id from Pet: {:?}",
            field_names
        );
        assert!(field_names.contains(&"name"), "Expected name from Pet");
        assert!(field_names.contains(&"tags"), "Expected tags from PetList");
    }

    fn make_group(slug: &str) -> ResourceGroup {
        ResourceGroup {
            slug: slug.to_string(),
            display_name: slug.to_string(),
            description: None,
            is_deprecated: false,
            is_alpha: false,
            endpoints: vec![],
        }
    }

    #[test]
    fn test_find_group_exact() {
        let groups = vec![make_group("pets"), make_group("owners")];
        assert_eq!(find_resource_group(&groups, "pets"), Some(0));
        assert_eq!(find_resource_group(&groups, "PETS"), Some(0));
        assert_eq!(find_resource_group(&groups, "owners"), Some(1));
    }

    #[test]
    fn test_get_resource_detail_pets() {
        let api = load_petstore();
        let group = get_resource_detail(&api, "pets");
        assert!(group.is_some(), "Expected to find 'pets' group");
        let group = group.unwrap();

        let get_pets = group
            .endpoints
            .iter()
            .find(|e| e.method == "GET" && e.path == "/pets");
        assert!(get_pets.is_some(), "Expected GET /pets endpoint");
        assert_eq!(
            get_pets.unwrap().summary.as_deref(),
            Some("List all pets"),
            "Expected summary to be populated"
        );
    }

    #[test]
    fn test_build_fields_pet() {
        let api = load_petstore();

        let pet_schema = api.components.as_ref().unwrap().schemas.get("Pet").unwrap();
        let pet_schema = match pet_schema {
            openapiv3::ReferenceOr::Item(s) => s,
            _ => panic!("Expected item"),
        };

        let required = vec!["id".to_string(), "name".to_string()];
        let fields = build_fields(&api, pet_schema, &required);

        // id: string/uuid, read_only, required
        let id_field = fields.iter().find(|f| f.name == "id").expect("id field");
        assert_eq!(id_field.type_display, "string/uuid");
        assert!(id_field.read_only, "id should be read_only");
        assert!(id_field.required, "id should be required");

        // name: string, required
        let name_field = fields
            .iter()
            .find(|f| f.name == "name")
            .expect("name field");
        assert_eq!(name_field.type_display, "string");
        assert!(name_field.required);

        // status: string with enum
        let status_field = fields
            .iter()
            .find(|f| f.name == "status")
            .expect("status field");
        assert!(
            !status_field.enum_values.is_empty(),
            "status should have enum values"
        );
        assert!(status_field.enum_values.contains(&"available".to_string()));

        // nickname: nullable
        let nickname_field = fields
            .iter()
            .find(|f| f.name == "nickname")
            .expect("nickname field");
        assert!(nickname_field.nullable, "nickname should be nullable");

        // owner: ref to Owner
        let owner_field = fields
            .iter()
            .find(|f| f.name == "owner")
            .expect("owner field");
        assert_eq!(owner_field.type_display, "Owner");
        assert_eq!(owner_field.nested_schema_name.as_deref(), Some("Owner"));
    }

    #[test]
    fn test_find_group_not_found() {
        let groups = vec![make_group("pets")];
        assert_eq!(find_resource_group(&groups, "widgets"), None);
    }

    // ─── Task 4.1: Jaro-Winkler suggestion tests ───

    #[test]
    fn test_suggest_similar_transposition() {
        // "ptes" is a transposition of "pets" — contains() misses this
        let groups = vec![make_group("pets"), make_group("owners")];
        let suggestions = suggest_similar(&groups, "ptes");
        assert!(
            suggestions.contains(&"pets"),
            "Jaro-Winkler should suggest 'pets' for transposition typo 'ptes', got: {:?}",
            suggestions
        );
    }

    #[test]
    fn test_suggest_similar_extra_char() {
        // "petss" has an extra char — contains() misses this because "pets".contains("petss") is false
        let groups = vec![make_group("pets"), make_group("owners")];
        let suggestions = suggest_similar(&groups, "petss");
        assert!(
            suggestions.contains(&"pets"),
            "Jaro-Winkler should suggest 'pets' for near-match 'petss', got: {:?}",
            suggestions
        );
    }

    #[test]
    fn test_suggest_similar_no_false_positive() {
        // "xyz" should not match "pets" — very different strings
        let groups = vec![make_group("pets"), make_group("owners")];
        let suggestions = suggest_similar(&groups, "xyz");
        assert!(
            !suggestions.contains(&"pets"),
            "Jaro-Winkler must not suggest 'pets' for completely different input 'xyz'"
        );
    }

    #[test]
    fn test_oneof_request_body_surfaces_options() {
        let api = load_petstore();
        let ep = get_endpoint_detail(&api, "POST", "/animals", false, "phyllotaxis")
            .expect("Expected POST /animals endpoint");
        let body = ep.request_body.expect("Expected request body");
        assert!(body.fields.is_empty(), "OneOf body should have no fields");
        assert!(!body.options.is_empty(), "OneOf body should have options");
        assert!(
            body.options.contains(&"Pet".to_string()),
            "Expected Pet option"
        );
        assert!(
            body.options.contains(&"Owner".to_string()),
            "Expected Owner option"
        );
    }

    #[test]
    fn test_expand_endpoint_request_body() {
        let api = load_petstore();
        let ep = get_endpoint_detail(&api, "POST", "/pets", true, "phyllotaxis")
            .expect("Expected POST /pets endpoint");
        let body = ep.request_body.expect("Expected request body");
        // With expand, the owner field should have nested fields inlined
        let owner_field = body.fields.iter().find(|f| f.name == "owner");
        assert!(owner_field.is_some(), "Expected owner field");
        assert!(
            !owner_field.unwrap().nested_fields.is_empty(),
            "With expand, owner should have nested fields"
        );
    }

    #[test]
    fn test_ref_request_body_resolves() {
        let api = load_petstore();
        let ep = get_endpoint_detail(&api, "PUT", "/pets/{id}", false, "phyllotaxis")
            .expect("Expected PUT /pets/{id} endpoint");
        let body = ep
            .request_body
            .expect("$ref request body should resolve, not be None");
        assert_eq!(body.content_type, "application/json");
        assert!(
            !body.fields.is_empty(),
            "$ref request body should have fields from the Pet schema"
        );
        let field_names: Vec<&str> = body.fields.iter().map(|f| f.name.as_str()).collect();
        assert!(
            field_names.contains(&"name"),
            "Expected 'name' field from Pet schema, got: {:?}",
            field_names
        );
    }

    #[test]
    fn test_expand_endpoint_response_fields() {
        let api = load_petstore();
        let ep = get_endpoint_detail(&api, "POST", "/pets", true, "phyllotaxis")
            .expect("Expected POST /pets endpoint");
        // POST /pets returns 201 -> Pet; with expand, response should have Pet's fields
        let resp_201 = ep
            .responses
            .iter()
            .find(|r| r.status_code == "201")
            .expect("Expected 201 response");
        assert_eq!(resp_201.schema_ref.as_deref(), Some("Pet"));
        assert!(
            !resp_201.fields.is_empty(),
            "With expand, 201 response should have inline fields from Pet schema"
        );
        // Check that the id field is present (Pet has required id and name)
        let id_field = resp_201.fields.iter().find(|f| f.name == "id");
        assert!(id_field.is_some(), "Pet schema should have an id field");

        // Error responses should not have fields (no schema_ref)
        let resp_400 = ep
            .responses
            .iter()
            .find(|r| r.status_code == "400")
            .expect("Expected 400 response");
        assert!(
            resp_400.fields.is_empty(),
            "Error response without schema should have no fields"
        );
    }

    #[test]
    fn test_expand_false_response_fields_empty() {
        let api = load_petstore();
        let ep = get_endpoint_detail(&api, "POST", "/pets", false, "phyllotaxis")
            .expect("Expected POST /pets endpoint");
        // Without expand, response fields should be empty
        let resp_201 = ep
            .responses
            .iter()
            .find(|r| r.status_code == "201")
            .expect("Expected 201 response");
        assert!(
            resp_201.fields.is_empty(),
            "Without expand, response fields should be empty"
        );
    }

    #[test]
    fn test_pascal_case_display_name() {
        let api = load_petstore();
        let groups = extract_resource_groups(&api);
        let pascal = groups
            .iter()
            .find(|g| g.slug == "pascal-case-resource")
            .expect("Expected pascal-case-resource group");
        assert_eq!(
            pascal.display_name, "Pascal Case Resource",
            "PascalCaseResource tag should humanize to 'Pascal Case Resource'"
        );
    }

    #[test]
    fn test_resolve_path_item_found() {
        let api = crate::spec::load_spec(
            Some("tests/fixtures/petstore.yaml"),
            &std::path::PathBuf::from("."),
        )
        .unwrap();
        let result = resolve_path_item(&api.api, "/pets");
        assert!(result.is_some(), "expected /pets to resolve to a path item");
    }

    #[test]
    fn test_resolve_path_item_missing() {
        let api = crate::spec::load_spec(
            Some("tests/fixtures/petstore.yaml"),
            &std::path::PathBuf::from("."),
        )
        .unwrap();
        let result = resolve_path_item(&api.api, "/does-not-exist");
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_operation_known_method() {
        let api = crate::spec::load_spec(
            Some("tests/fixtures/petstore.yaml"),
            &std::path::PathBuf::from("."),
        )
        .unwrap();
        let path_item = resolve_path_item(&api.api, "/pets").unwrap();
        assert!(resolve_operation(path_item, "GET").is_some());
        assert!(
            resolve_operation(path_item, "get").is_some(),
            "should be case-insensitive"
        );
    }

    #[test]
    fn test_resolve_operation_unknown_method() {
        let api = crate::spec::load_spec(
            Some("tests/fixtures/petstore.yaml"),
            &std::path::PathBuf::from("."),
        )
        .unwrap();
        let path_item = resolve_path_item(&api.api, "/pets").unwrap();
        assert!(resolve_operation(path_item, "CONNECT").is_none());
    }

    #[test]
    fn test_extract_security_returns_vec() {
        let api = crate::spec::load_spec(
            Some("tests/fixtures/petstore.yaml"),
            &std::path::PathBuf::from("."),
        )
        .unwrap();
        let path_item = resolve_path_item(&api.api, "/pets").unwrap();
        let operation = resolve_operation(path_item, "GET").unwrap();
        // Just verify it runs without panic and returns a Vec (content depends on fixture)
        let _security = extract_security(&api.api, operation);
    }

    #[test]
    fn test_resolve_operation_absent_method() {
        let api = crate::spec::load_spec(
            Some("tests/fixtures/petstore.yaml"),
            &std::path::PathBuf::from("."),
        )
        .unwrap();
        let path_item = resolve_path_item(&api.api, "/pets").unwrap();
        // /pets only defines GET and POST; DELETE exists only on /pets/{id}
        assert!(resolve_operation(path_item, "DELETE").is_none());
    }

    #[test]
    fn test_extract_responses_nonempty() {
        let api = crate::spec::load_spec(
            Some("tests/fixtures/petstore.yaml"),
            &std::path::PathBuf::from("."),
        )
        .unwrap();
        let path_item = resolve_path_item(&api.api, "/pets").unwrap();
        let operation = resolve_operation(path_item, "GET").unwrap();
        let responses = extract_responses(&api.api, operation, false, "phyllotaxis");
        assert!(
            !responses.is_empty(),
            "GET /pets should have at least one response"
        );
    }

    #[test]
    fn test_extract_responses_status_codes_are_strings() {
        let api = crate::spec::load_spec(
            Some("tests/fixtures/petstore.yaml"),
            &std::path::PathBuf::from("."),
        )
        .unwrap();
        let path_item = resolve_path_item(&api.api, "/pets").unwrap();
        let operation = resolve_operation(path_item, "GET").unwrap();
        let responses = extract_responses(&api.api, operation, false, "phyllotaxis");
        for r in &responses {
            assert!(!r.status_code.is_empty());
        }
    }

    #[test]
    fn test_merge_parameters_nonempty_for_parameterized_path() {
        let api = crate::spec::load_spec(
            Some("tests/fixtures/petstore.yaml"),
            &std::path::PathBuf::from("."),
        )
        .unwrap();
        // /pets/{id} has a path-level 'id' parameter with GET operation
        let path_item = resolve_path_item(&api.api, "/pets/{id}").unwrap();
        let operation = resolve_operation(path_item, "GET").unwrap();
        let params = merge_parameters(&api.api, path_item, operation);
        assert!(
            !params.is_empty(),
            "parameterized endpoint should have parameters"
        );
    }

    #[test]
    fn test_merge_parameters_runs_for_simple_path() {
        let api = crate::spec::load_spec(
            Some("tests/fixtures/petstore.yaml"),
            &std::path::PathBuf::from("."),
        )
        .unwrap();
        let path_item = resolve_path_item(&api.api, "/pets").unwrap();
        let operation = resolve_operation(path_item, "GET").unwrap();
        let params = merge_parameters(&api.api, path_item, operation);
        let _ = params; // just verify it doesn't panic
    }

    #[test]
    fn test_extract_request_body_post_has_body() {
        let api = crate::spec::load_spec(
            Some("tests/fixtures/petstore.yaml"),
            &std::path::PathBuf::from("."),
        )
        .unwrap();
        // POST /pets has a requestBody with application/json schema $ref: Pet
        let path_item = resolve_path_item(&api.api, "/pets").unwrap();
        let operation = resolve_operation(path_item, "POST").unwrap();
        let body = extract_request_body(&api.api, operation, false);
        assert!(body.is_some(), "POST /pets should have a request body");
    }

    #[test]
    fn test_extract_request_body_get_is_none() {
        let api = crate::spec::load_spec(
            Some("tests/fixtures/petstore.yaml"),
            &std::path::PathBuf::from("."),
        )
        .unwrap();
        let path_item = resolve_path_item(&api.api, "/pets").unwrap();
        let operation = resolve_operation(path_item, "GET").unwrap();
        let body = extract_request_body(&api.api, operation, false);
        assert!(body.is_none(), "GET should have no request body");
    }

    #[test]
    fn test_drill_deeper_2xx_response_schema() {
        let api = load_petstore();
        let ep = get_endpoint_detail(&api, "GET", "/pets/{id}", false, "phyllotaxis").unwrap();
        assert!(
            ep.drill_deeper
                .contains(&"phyllotaxis schemas Pet".to_string()),
            "GET /pets/{{petId}} 200 response should yield drill_deeper for Pet, got: {:?}",
            ep.drill_deeper
        );
    }

    #[test]
    fn test_drill_deeper_excludes_error_responses() {
        let api = load_petstore();
        let ep = get_endpoint_detail(&api, "POST", "/pets", false, "phyllotaxis").unwrap();
        // POST /pets has 201 → Pet, 400 → no schema, 409 → no schema
        assert!(
            ep.drill_deeper
                .contains(&"phyllotaxis schemas Pet".to_string()),
            "Should include Pet from 201 response"
        );
        // Error responses should not contribute extra entries
        assert_eq!(
            ep.drill_deeper.iter().filter(|d| d.contains("Pet")).count(),
            1,
            "Pet should appear exactly once"
        );
    }

    #[test]
    fn test_drill_deeper_empty_when_no_schemas() {
        let api = load_petstore();
        let ep = get_endpoint_detail(&api, "DELETE", "/pets/{id}", false, "phyllotaxis").unwrap();
        assert!(
            ep.drill_deeper.is_empty(),
            "DELETE /pets/{{petId}} (204 no content) should have empty drill_deeper, got: {:?}",
            ep.drill_deeper
        );
    }

    #[test]
    fn test_drill_deeper_deduplication() {
        // POST /pets: 201 response has Pet schema_ref, request body also refs Pet
        let api = load_petstore();
        let ep = get_endpoint_detail(&api, "POST", "/pets", false, "phyllotaxis").unwrap();
        let pet_count = ep.drill_deeper.iter().filter(|d| d.contains("Pet")).count();
        assert_eq!(
            pet_count, 1,
            "Pet should be deduplicated across response and request body, got: {:?}",
            ep.drill_deeper
        );
    }

    #[test]
    fn test_request_body_schema_ref_concrete() {
        let api = load_petstore();
        let ep = get_endpoint_detail(&api, "POST", "/pets", false, "phyllotaxis").unwrap();
        let body = ep.request_body.as_ref().unwrap();
        assert_eq!(
            body.schema_ref,
            Some("Pet".to_string()),
            "POST /pets request body should have schema_ref = Pet"
        );
    }

    #[test]
    fn test_write_only_field_extraction() {
        let api = load_kitchen_sink();
        let schema = api
            .components
            .as_ref()
            .unwrap()
            .schemas
            .get("CreateUserRequest")
            .unwrap();
        let schema = match schema {
            openapiv3::ReferenceOr::Item(s) => s,
            _ => panic!("expected item"),
        };
        let fields = build_fields(
            &api,
            schema,
            &[
                "username".to_string(),
                "email".to_string(),
                "password".to_string(),
            ],
        );
        let password = fields
            .iter()
            .find(|f| f.name == "password")
            .expect("password field");
        assert!(password.write_only, "password should be write_only");
    }

    #[test]
    fn test_deprecated_field_extraction() {
        let api = load_kitchen_sink();
        let schema = api
            .components
            .as_ref()
            .unwrap()
            .schemas
            .get("PetBase")
            .unwrap();
        let schema = match schema {
            openapiv3::ReferenceOr::Item(s) => s,
            _ => panic!("expected item"),
        };
        let fields = build_fields(&api, schema, &[]);
        let legacy = fields
            .iter()
            .find(|f| f.name == "legacy_code")
            .expect("legacy_code field");
        assert!(legacy.deprecated, "legacy_code should be deprecated");
    }

    #[test]
    fn test_constraints_string_minlength_maxlength_pattern() {
        let api = load_kitchen_sink();
        let schema = api
            .components
            .as_ref()
            .unwrap()
            .schemas
            .get("User")
            .unwrap();
        let schema = match schema {
            openapiv3::ReferenceOr::Item(s) => s,
            _ => panic!(),
        };
        let fields = build_fields(&api, schema, &[]);
        let username = fields
            .iter()
            .find(|f| f.name == "username")
            .expect("username");
        assert!(
            username.constraints.iter().any(|c| c.starts_with("min:")),
            "missing min: {:?}",
            username.constraints
        );
        assert!(
            username.constraints.iter().any(|c| c.starts_with("max:")),
            "missing max: {:?}",
            username.constraints
        );
        assert!(
            username
                .constraints
                .iter()
                .any(|c| c.starts_with("pattern:")),
            "missing pattern: {:?}",
            username.constraints
        );
    }

    #[test]
    fn test_constraints_integer() {
        let api = load_kitchen_sink();
        let schema = api
            .components
            .as_ref()
            .unwrap()
            .schemas
            .get("Settings")
            .unwrap();
        let schema = match schema {
            openapiv3::ReferenceOr::Item(s) => s,
            _ => panic!(),
        };
        let fields = build_fields(&api, schema, &[]);
        let field = fields
            .iter()
            .find(|f| f.name == "max_upload_size_mb")
            .expect("max_upload_size_mb");
        assert!(
            field.constraints.iter().any(|c| c.starts_with("min:")),
            "missing min: {:?}",
            field.constraints
        );
        assert!(
            field.constraints.iter().any(|c| c.starts_with("max:")),
            "missing max: {:?}",
            field.constraints
        );
        assert!(
            field
                .constraints
                .iter()
                .any(|c| c.starts_with("multipleOf:")),
            "missing multipleOf: {:?}",
            field.constraints
        );
    }

    #[test]
    fn test_constraints_array_unique_items() {
        let api = load_kitchen_sink();
        let schema = api
            .components
            .as_ref()
            .unwrap()
            .schemas
            .get("PetBase")
            .unwrap();
        let schema = match schema {
            openapiv3::ReferenceOr::Item(s) => s,
            _ => panic!(),
        };
        let fields = build_fields(&api, schema, &[]);
        let tags = fields.iter().find(|f| f.name == "tags").expect("tags");
        assert!(
            tags.constraints.iter().any(|c| c == "uniqueItems"),
            "missing uniqueItems: {:?}",
            tags.constraints
        );
    }

    #[test]
    fn test_links_extracted_from_post_users() {
        let api = load_kitchen_sink();
        let ep = get_endpoint_detail(&api, "POST", "/users", false, "phyllotaxis").unwrap();
        let link_names: Vec<&str> = ep.links.iter().map(|l| l.name.as_str()).collect();
        assert!(
            link_names.contains(&"GetCreatedUser"),
            "missing GetCreatedUser: {:?}",
            link_names
        );
        assert!(
            link_names.contains(&"ListUserPets"),
            "missing ListUserPets: {:?}",
            link_names
        );
        let get_user_link = ep
            .links
            .iter()
            .find(|l| l.name == "GetCreatedUser")
            .unwrap();
        assert!(
            !get_user_link.parameters.is_empty(),
            "GetCreatedUser should have parameter mappings"
        );
        assert!(get_user_link
            .parameters
            .iter()
            .any(|p| p.contains("userId")));
    }

    #[test]
    fn test_link_drill_command_built() {
        let api = load_kitchen_sink();
        let ep = get_endpoint_detail(&api, "POST", "/users", false, "phyllotaxis").unwrap();
        let link = ep
            .links
            .iter()
            .find(|l| l.name == "GetCreatedUser")
            .unwrap();
        assert!(
            link.drill_command
                .as_ref()
                .map(|c| c.contains("phyllotaxis resources"))
                .unwrap_or(false),
            "Expected drill command, got: {:?}",
            link.drill_command
        );
    }

    #[test]
    fn test_response_headers_extracted() {
        let api = load_kitchen_sink();
        let ep = get_endpoint_detail(&api, "GET", "/users", false, "phyllotaxis").unwrap();
        let ok_resp = ep
            .responses
            .iter()
            .find(|r| r.status_code == "200")
            .unwrap();
        let header_names: Vec<&str> = ok_resp.headers.iter().map(|h| h.name.as_str()).collect();
        assert!(
            header_names.contains(&"X-Total-Count"),
            "missing X-Total-Count: {:?}",
            header_names
        );
        assert!(
            header_names.contains(&"X-Rate-Limit-Remaining"),
            "missing X-Rate-Limit-Remaining: {:?}",
            header_names
        );
    }

    #[test]
    fn test_callbacks_extracted_inline() {
        let api = load_kitchen_sink();
        let ep = get_endpoint_detail(
            &api,
            "POST",
            "/notifications/subscribe",
            false,
            "phyllotaxis",
        )
        .unwrap();
        let cb_names: Vec<&str> = ep.callbacks.iter().map(|c| c.name.as_str()).collect();
        assert!(
            cb_names.contains(&"onEvent"),
            "missing onEvent: {:?}",
            cb_names
        );
        assert!(
            cb_names.contains(&"onStatusChange"),
            "missing onStatusChange: {:?}",
            cb_names
        );

        let on_event = ep.callbacks.iter().find(|c| c.name == "onEvent").unwrap();
        assert!(
            !on_event.operations.is_empty(),
            "onEvent should have operations"
        );
        let op = &on_event.operations[0];
        assert_eq!(op.method, "POST");
        assert!(
            op.url_expression.contains("callbackUrl"),
            "URL expression: {}",
            op.url_expression
        );
        assert_eq!(
            op.body_schema.as_deref(),
            Some("EventPayload"),
            "onEvent body should be EventPayload, got {:?}",
            op.body_schema
        );
    }

    #[test]
    fn test_callback_responses_extracted() {
        let api = load_kitchen_sink();
        let ep = get_endpoint_detail(
            &api,
            "POST",
            "/notifications/subscribe",
            false,
            "phyllotaxis",
        )
        .unwrap();
        let on_event = ep.callbacks.iter().find(|c| c.name == "onEvent").unwrap();
        let op = &on_event.operations[0];
        let status_codes: Vec<&str> = op
            .responses
            .iter()
            .map(|r| r.status_code.as_str())
            .collect();
        assert!(
            status_codes.contains(&"200"),
            "missing 200: {:?}",
            status_codes
        );
        assert!(
            status_codes.contains(&"410"),
            "missing 410: {:?}",
            status_codes
        );
    }

    // ─── Task 14: Multi-content-type request body ───

    #[test]
    fn test_multipart_request_body_extracted() {
        let api = load_kitchen_sink();
        let ep = get_endpoint_detail(&api, "POST", "/files/upload", false, "phyllotaxis").unwrap();
        let rb = ep.request_body.as_ref().expect("should have request body");
        assert_eq!(rb.content_type, "multipart/form-data");
        let field_names: Vec<&str> = rb.fields.iter().map(|f| f.name.as_str()).collect();
        assert!(
            field_names.contains(&"file"),
            "missing 'file': {:?}",
            field_names
        );
        assert!(
            field_names.contains(&"description"),
            "missing 'description': {:?}",
            field_names
        );
    }

    #[test]
    fn test_multipart_binary_field_type() {
        let api = load_kitchen_sink();
        let ep = get_endpoint_detail(&api, "POST", "/files/upload", false, "phyllotaxis").unwrap();
        let rb = ep.request_body.as_ref().unwrap();
        let file_field = rb.fields.iter().find(|f| f.name == "file").unwrap();
        assert_eq!(
            file_field.type_display, "binary",
            "binary format should display as 'binary', not 'string/binary'"
        );
    }

    #[test]
    fn test_form_urlencoded_request_body() {
        let api = load_kitchen_sink();
        let ep = get_endpoint_detail(
            &api,
            "PUT",
            "/files/{fileId}/metadata",
            false,
            "phyllotaxis",
        )
        .unwrap();
        let rb = ep.request_body.as_ref().expect("should have request body");
        assert_eq!(rb.content_type, "application/x-www-form-urlencoded");
        let field_names: Vec<&str> = rb.fields.iter().map(|f| f.name.as_str()).collect();
        assert!(
            field_names.contains(&"description"),
            "missing 'description': {:?}",
            field_names
        );
    }

    // ─── Bug fix: inline request body without explicit `type: object` ───

    #[test]
    fn test_implicit_object_request_body_extracted() {
        // Schemas with `properties` but no `type: object` (common in GitLab, auto-generated
        // specs) should still have their fields extracted — not show "Raw body (no schema)".
        let yaml = r#"
openapi: "3.0.0"
info:
  title: Test
  version: "1.0"
paths:
  /messages:
    post:
      operationId: createMessage
      requestBody:
        required: true
        content:
          application/json:
            schema:
              required:
                - message
              properties:
                message:
                  type: string
                  description: Message to display
                color:
                  type: string
                  description: Background color
                active:
                  type: boolean
                  description: Whether the message is active
      responses:
        '201':
          description: Created
"#;
        let api: openapiv3::OpenAPI = serde_yaml_ng::from_str(yaml).unwrap();
        let ep = get_endpoint_detail(&api, "POST", "/messages", false, "phyllotaxis")
            .expect("Expected POST /messages endpoint");
        let body = ep
            .request_body
            .expect("Implicit object request body should be extracted");
        assert_eq!(body.content_type, "application/json");
        assert!(
            !body.fields.is_empty(),
            "Implicit object should have fields, got empty (was 'Raw body' bug)"
        );
        let field_names: Vec<&str> = body.fields.iter().map(|f| f.name.as_str()).collect();
        assert!(
            field_names.contains(&"message"),
            "missing 'message': {:?}",
            field_names
        );
        assert!(
            field_names.contains(&"color"),
            "missing 'color': {:?}",
            field_names
        );
        assert!(
            field_names.contains(&"active"),
            "missing 'active': {:?}",
            field_names
        );
        // Verify required flag
        let msg_field = body.fields.iter().find(|f| f.name == "message").unwrap();
        assert!(msg_field.required, "'message' should be required");
        let color_field = body.fields.iter().find(|f| f.name == "color").unwrap();
        assert!(!color_field.required, "'color' should be optional");
    }

    // ─── anyOf/oneOf response schema resolution ───

    #[test]
    fn test_anyof_response_schema_ref() {
        // GET /users/{userId}/account returns anyOf [User, DeletedUser]
        let api = load_kitchen_sink();
        let ep = get_endpoint_detail(&api, "GET", "/users/{userId}/account", false, "phyllotaxis")
            .unwrap();

        let resp_200 = ep
            .responses
            .iter()
            .find(|r| r.status_code == "200")
            .unwrap();
        assert_eq!(
            resp_200.schema_ref.as_deref(),
            Some("User | DeletedUser"),
            "anyOf response should show pipe-separated variant names"
        );
    }

    #[test]
    fn test_anyof_response_drill_deeper_splits_variants() {
        // Drill deeper should list each variant separately, not "User | DeletedUser"
        let api = load_kitchen_sink();
        let ep = get_endpoint_detail(&api, "GET", "/users/{userId}/account", false, "phyllotaxis")
            .unwrap();

        assert!(
            ep.drill_deeper
                .contains(&"phyllotaxis schemas User".to_string()),
            "Missing drill_deeper for User: {:?}",
            ep.drill_deeper
        );
        assert!(
            ep.drill_deeper
                .contains(&"phyllotaxis schemas DeletedUser".to_string()),
            "Missing drill_deeper for DeletedUser: {:?}",
            ep.drill_deeper
        );
    }

    // ─── Inline pagination/object response schema resolution ───

    #[test]
    fn test_inline_pagination_wrapper_response_schema() {
        let api = load_kitchen_sink();
        let ep = get_endpoint_detail(&api, "GET", "/users/paginated", false, "phyllotaxis")
            .expect("Expected GET /users/paginated endpoint");
        let resp_200 = ep
            .responses
            .iter()
            .find(|r| r.status_code == "200")
            .expect("Expected 200 response");
        assert_eq!(
            resp_200.schema_ref.as_deref(),
            Some("User[] (list)"),
            "Inline pagination wrapper with data.$ref array should resolve to 'User[] (list)'"
        );
    }

    #[test]
    fn test_inline_object_response_schema() {
        let api = load_kitchen_sink();
        let ep = get_endpoint_detail(&api, "GET", "/users/summary", false, "phyllotaxis")
            .expect("Expected GET /users/summary endpoint");
        let resp_200 = ep
            .responses
            .iter()
            .find(|r| r.status_code == "200")
            .expect("Expected 200 response");
        assert_eq!(
            resp_200.schema_ref.as_deref(),
            Some("object"),
            "Inline object without data array should resolve to 'object'"
        );
    }

    #[test]
    fn test_inline_pagination_drill_deeper_uses_item_type() {
        let api = load_kitchen_sink();
        let ep = get_endpoint_detail(&api, "GET", "/users/paginated", false, "phyllotaxis")
            .expect("Expected GET /users/paginated endpoint");
        assert!(
            ep.drill_deeper.iter().any(|d| d.contains("User")),
            "Drill deeper should reference the User item type, not the wrapper. Got: {:?}",
            ep.drill_deeper
        );
    }

    #[test]
    fn test_inline_object_no_drill_deeper() {
        let api = load_kitchen_sink();
        let ep = get_endpoint_detail(&api, "GET", "/users/summary", false, "phyllotaxis")
            .expect("Expected GET /users/summary endpoint");
        let has_object_drill = ep.drill_deeper.iter().any(|d| d.contains("object"));
        assert!(
            !has_object_drill,
            "Generic inline object should not produce a drill-deeper hint. Got: {:?}",
            ep.drill_deeper
        );
    }

    #[test]
    fn test_direct_ref_response_still_works() {
        let api = load_petstore();
        let ep = get_endpoint_detail(&api, "GET", "/pets/{id}", false, "phyllotaxis")
            .expect("Expected GET /pets/{id} endpoint");
        let resp_200 = ep
            .responses
            .iter()
            .find(|r| r.status_code == "200")
            .expect("Expected 200 response");
        assert_eq!(
            resp_200.schema_ref.as_deref(),
            Some("Pet"),
            "Direct $ref response should still resolve to schema name"
        );
    }

    #[test]
    fn test_direct_array_response_still_works() {
        let api = load_petstore();
        let ep = get_endpoint_detail(&api, "GET", "/pets", false, "phyllotaxis")
            .expect("Expected GET /pets endpoint");
        let resp_200 = ep
            .responses
            .iter()
            .find(|r| r.status_code == "200")
            .expect("Expected 200 response");
        assert_eq!(
            resp_200.schema_ref.as_deref(),
            Some("Pet[]"),
            "Direct array with $ref items should still resolve to 'Type[]'"
        );
    }

    // ─── --expand for inline request body objects ───

    #[test]
    fn test_expand_inline_object_in_request_body() {
        let api = load_kitchen_sink();
        let ep = get_endpoint_detail(&api, "PUT", "/admin/settings", true, "phyllotaxis")
            .expect("Expected PUT /admin/settings endpoint");
        let body = ep.request_body.expect("Expected request body");

        let smtp = body
            .fields
            .iter()
            .find(|f| f.name == "smtp_config")
            .expect("Expected smtp_config field");
        assert!(
            !smtp.nested_fields.is_empty(),
            "With expand, smtp_config should have nested fields for its inline properties"
        );

        let nested_names: Vec<&str> = smtp.nested_fields.iter().map(|f| f.name.as_str()).collect();
        assert!(
            nested_names.contains(&"host"),
            "missing nested 'host': {:?}",
            nested_names
        );
        assert!(
            nested_names.contains(&"port"),
            "missing nested 'port': {:?}",
            nested_names
        );
        assert!(
            nested_names.contains(&"username"),
            "missing nested 'username': {:?}",
            nested_names
        );
        assert!(
            nested_names.contains(&"password"),
            "missing nested 'password': {:?}",
            nested_names
        );
    }

    #[test]
    fn test_no_expand_inline_object_stays_empty() {
        let api = load_kitchen_sink();
        let ep = get_endpoint_detail(&api, "PUT", "/admin/settings", false, "phyllotaxis")
            .expect("Expected PUT /admin/settings endpoint");
        let body = ep.request_body.expect("Expected request body");

        let smtp = body
            .fields
            .iter()
            .find(|f| f.name == "smtp_config")
            .expect("Expected smtp_config field");
        assert!(
            smtp.nested_fields.is_empty(),
            "Without expand, smtp_config should NOT have nested fields"
        );
    }
}
