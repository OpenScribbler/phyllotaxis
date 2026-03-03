use crate::commands::resources::{
    extract_object_properties, extract_resource_groups, path_prefix_group_name,
};
use crate::commands::schemas::list_schemas;
use crate::models::resource::slugify;

#[derive(Debug, serde::Serialize)]
pub struct CallbackMatch {
    pub name: String,
    pub defined_on_path: String,
}

#[derive(Debug, serde::Serialize)]
pub struct SearchResults {
    pub term: String,
    pub resources: Vec<ResourceMatch>,
    pub endpoints: Vec<EndpointMatch>,
    pub schemas: Vec<SchemaMatch>,
    pub callbacks: Vec<CallbackMatch>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct ResourceMatch {
    pub slug: String,
    pub description: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct EndpointMatch {
    pub method: String,
    pub path: String,
    pub summary: Option<String>,
    pub resource_slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_on: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct SchemaMatch {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_field: Option<String>,
}

pub fn search(api: &openapiv3::OpenAPI, term: &str) -> SearchResults {
    let term_lower = term.to_lowercase();

    // Search resources
    let groups = extract_resource_groups(api);
    let resources: Vec<ResourceMatch> = groups
        .iter()
        .filter(|g| {
            g.slug.to_lowercase().contains(&term_lower)
                || g.description
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&term_lower)
        })
        .map(|g| ResourceMatch {
            slug: g.slug.clone(),
            description: g.description.clone(),
        })
        .collect();

    // Search endpoints
    let mut endpoints = Vec::new();
    for (path_str, item) in api.paths.iter() {
        if let openapiv3::ReferenceOr::Item(item) = item {
            let ops: Vec<(&str, &openapiv3::Operation)> = [
                ("GET", &item.get),
                ("PUT", &item.put),
                ("POST", &item.post),
                ("DELETE", &item.delete),
                ("OPTIONS", &item.options),
                ("HEAD", &item.head),
                ("PATCH", &item.patch),
                ("TRACE", &item.trace),
            ]
            .into_iter()
            .filter_map(|(method, op)| op.as_ref().map(|o| (method, o)))
            .collect();

            for (method, op) in ops {
                let path_match = path_str.to_lowercase().contains(&term_lower);
                let summary_match = op
                    .summary
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&term_lower);
                let op_desc_match = op
                    .description
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&term_lower);

                // Also match against parameter names and descriptions,
                // capturing the first matching param name for match-reason display.
                let mut matched_param_name: Option<String> = None;
                for p in &op.parameters {
                    if let openapiv3::ReferenceOr::Item(param) = p {
                        let pdata = match param {
                            openapiv3::Parameter::Query { parameter_data, .. } => parameter_data,
                            openapiv3::Parameter::Path { parameter_data, .. } => parameter_data,
                            openapiv3::Parameter::Header { parameter_data, .. } => parameter_data,
                            openapiv3::Parameter::Cookie { parameter_data, .. } => parameter_data,
                        };
                        let name_match = pdata.name.to_lowercase().contains(&term_lower);
                        let pdesc_match = pdata
                            .description
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&term_lower);
                        if name_match || pdesc_match {
                            matched_param_name = Some(pdata.name.clone());
                            break;
                        }
                    }
                }
                let param_match = matched_param_name.is_some();

                if path_match || summary_match || op_desc_match || param_match {
                    let resource_slug = op
                        .tags
                        .first()
                        .map(|t| slugify(t))
                        .or_else(|| path_prefix_group_name(path_str))
                        .unwrap_or_default();

                    // Only annotate matched_on when the match came from a parameter
                    // and was NOT also a path/summary/description match.
                    let matched_on =
                        if param_match && !path_match && !summary_match && !op_desc_match {
                            matched_param_name.map(|n| format!("parameter: {}", n))
                        } else {
                            None
                        };

                    endpoints.push(EndpointMatch {
                        method: method.to_string(),
                        path: path_str.clone(),
                        summary: op.summary.clone(),
                        resource_slug,
                        matched_on,
                    });
                }
            }
        }
    }

    // Search schemas — by name OR by field name
    let mut schemas: Vec<SchemaMatch> = Vec::new();
    for name in list_schemas(api) {
        if name.to_lowercase().contains(&term_lower) {
            // Name match: no field annotation
            schemas.push(SchemaMatch {
                name,
                matched_field: None,
            });
            continue;
        }

        // Field name match: look inside the schema's properties
        if let Some((_, schema)) = crate::commands::schemas::find_schema(api, &name) {
            let field_match = if let Some((props, _)) = extract_object_properties(schema) {
                props
                    .keys()
                    .find(|k| k.to_lowercase().contains(&term_lower))
                    .cloned()
            } else if let openapiv3::SchemaKind::AllOf { all_of } = &schema.schema_kind {
                // Walk allOf subschemas for inline object properties
                all_of.iter().find_map(|sub| {
                    if let openapiv3::ReferenceOr::Item(sub_schema) = sub {
                        if let Some((props, _)) = extract_object_properties(sub_schema) {
                            return props
                                .keys()
                                .find(|k| k.to_lowercase().contains(&term_lower))
                                .cloned();
                        }
                    }
                    None
                })
            } else {
                None
            };

            if let Some(field_name) = field_match {
                schemas.push(SchemaMatch {
                    name,
                    matched_field: Some(field_name),
                });
            }
        }
    }

    // Search callbacks
    let all_callbacks = crate::commands::callbacks::list_all_callbacks(api);
    let callbacks: Vec<CallbackMatch> = all_callbacks
        .into_iter()
        .filter(|cb| {
            cb.name.to_lowercase().contains(&term_lower)
                || cb.defined_on_path.to_lowercase().contains(&term_lower)
        })
        .map(|cb| CallbackMatch {
            name: cb.name,
            defined_on_path: cb.defined_on_path,
        })
        .collect();

    let has_results = !resources.is_empty()
        || !endpoints.is_empty()
        || !schemas.is_empty()
        || !callbacks.is_empty();

    let suggestions = if has_results {
        Vec::new()
    } else {
        let mut suggs: Vec<String> = crate::commands::resources::suggest_similar(&groups, term)
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        suggs.extend(crate::commands::schemas::suggest_similar_schemas(api, term));
        suggs.dedup();
        suggs.truncate(5);
        suggs
    };

    SearchResults {
        term: term.to_string(),
        resources,
        endpoints,
        schemas,
        callbacks,
        suggestions,
    }
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

    #[test]
    fn test_search_pet() {
        let api = load_petstore_api();
        let results = search(&api, "pet");

        assert!(
            !results.resources.is_empty(),
            "Expected resource matches for 'pet'"
        );
        assert!(
            results.resources.iter().any(|r| r.slug == "pets"),
            "Expected 'pets' resource match"
        );

        assert!(
            !results.endpoints.is_empty(),
            "Expected endpoint matches for 'pet'"
        );

        assert!(
            !results.schemas.is_empty(),
            "Expected schema matches for 'pet'"
        );
        let schema_names: Vec<&str> = results.schemas.iter().map(|s| s.name.as_str()).collect();
        assert!(schema_names.contains(&"Pet"), "Expected Pet schema match");
        assert!(
            schema_names.contains(&"PetList"),
            "Expected PetList schema match"
        );
    }

    #[test]
    fn test_search_no_results() {
        let api = load_petstore_api();
        let results = search(&api, "xyzzy123nonexistent");
        assert!(results.resources.is_empty());
        assert!(results.endpoints.is_empty());
        assert!(results.schemas.is_empty());
    }

    #[test]
    fn test_search_finds_query_param_name() {
        let api = load_petstore_api();
        // "filter" is a query parameter name on /pets/search but not in path/summary/description
        let results = search(&api, "filter");
        assert!(
            !results.endpoints.is_empty(),
            "Search for 'filter' should find the endpoint with a filter query param"
        );
        assert!(
            results.endpoints.iter().any(|e| e.path == "/pets/search"),
            "Expected /pets/search in results, got: {:?}",
            results
                .endpoints
                .iter()
                .map(|e| &e.path)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_search_finds_param_description() {
        let api = load_petstore_api();
        // "narrowing" appears in the filter param description
        let results = search(&api, "narrowing");
        assert!(
            !results.endpoints.is_empty(),
            "Search for 'narrowing' should find the endpoint with that term in a param description"
        );
    }

    fn load_kitchen_sink_api() -> openapiv3::OpenAPI {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let content =
            std::fs::read_to_string(manifest_dir.join("tests/fixtures/kitchen-sink.yaml")).unwrap();
        serde_yaml_ng::from_str(&content).unwrap()
    }

    #[test]
    fn test_search_field_name_email() {
        let api = load_kitchen_sink_api();
        let results = search(&api, "email");

        // Should find schemas that have an "email" field
        assert!(
            !results.schemas.is_empty(),
            "Search for 'email' should find schemas with email fields"
        );
        let names: Vec<&str> = results.schemas.iter().map(|s| s.name.as_str()).collect();
        assert!(
            names.contains(&"User"),
            "Expected User (has email field), got: {:?}",
            names
        );
        assert!(
            names.contains(&"CreateUserRequest"),
            "Expected CreateUserRequest (has email field), got: {:?}",
            names
        );

        // Matches via field should have matched_field populated
        let user_match = results.schemas.iter().find(|s| s.name == "User").unwrap();
        assert_eq!(
            user_match.matched_field.as_deref(),
            Some("email"),
            "User match should annotate matched_field='email'"
        );
    }

    #[test]
    fn test_search_endpoint_match_reason_parameter() {
        let api = load_kitchen_sink_api();
        // session_token is a cookie parameter on GET /users — not in path/summary/description
        let results = search(&api, "session");
        assert!(
            !results.endpoints.is_empty(),
            "Search for 'session' should find GET /users via session_token param"
        );
        let users_get = results
            .endpoints
            .iter()
            .find(|e| e.path == "/users" && e.method == "GET");
        assert!(
            users_get.is_some(),
            "Expected GET /users in results, got: {:?}",
            results
                .endpoints
                .iter()
                .map(|e| (&e.method, &e.path))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            users_get.unwrap().matched_on.as_deref(),
            Some("parameter: session_token"),
            "matched_on should be 'parameter: session_token'"
        );
    }

    #[test]
    fn test_search_endpoint_match_reason_none_for_path_match() {
        let api = load_kitchen_sink_api();
        // /users matches by path for "users" — matched_on should be None
        let results = search(&api, "users");
        let path_match = results
            .endpoints
            .iter()
            .find(|e| e.path == "/users" && e.method == "GET");
        if let Some(m) = path_match {
            assert!(
                m.matched_on.is_none(),
                "Path-matched endpoint should have matched_on=None, got: {:?}",
                m.matched_on
            );
        }
    }

    #[test]
    fn test_search_field_name_does_not_shadow_name_match() {
        let api = load_kitchen_sink_api();
        // "User" matches by name — matched_field should be None
        let results = search(&api, "user");
        let user_match = results.schemas.iter().find(|s| s.name == "User");
        assert!(user_match.is_some(), "User should still match by name");
        assert!(
            user_match.unwrap().matched_field.is_none(),
            "Name-matched schema should not have matched_field set"
        );
    }

    #[test]
    fn test_search_misspelled_term_has_suggestions() {
        let api = load_petstore_api();
        // "ptes" is a typo for "pets" — should produce fuzzy suggestions
        let results = search(&api, "ptes");
        assert!(results.resources.is_empty(), "No exact matches expected");
        assert!(results.endpoints.is_empty(), "No exact matches expected");
        assert!(results.schemas.is_empty(), "No exact matches expected");
        assert!(
            !results.suggestions.is_empty(),
            "Expected fuzzy suggestions for misspelled 'ptes', got none"
        );
    }

    #[test]
    fn test_search_successful_has_empty_suggestions() {
        let api = load_petstore_api();
        let results = search(&api, "pet");
        assert!(
            !results.resources.is_empty() || !results.endpoints.is_empty(),
            "Expected actual results for 'pet'"
        );
        assert!(
            results.suggestions.is_empty(),
            "Suggestions should be empty when there are real results"
        );
    }
}
