use crate::commands::resources::extract_resource_groups;
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
}

#[derive(Debug, serde::Serialize)]
pub struct SchemaMatch {
    pub name: String,
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
                let desc_match = op
                    .description
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&term_lower);

                // Also match against parameter names and descriptions
                let param_match = op.parameters.iter().any(|p| {
                    if let openapiv3::ReferenceOr::Item(param) = p {
                        let pdata = match param {
                            openapiv3::Parameter::Query { parameter_data, .. } => parameter_data,
                            openapiv3::Parameter::Path { parameter_data, .. } => parameter_data,
                            openapiv3::Parameter::Header { parameter_data, .. } => parameter_data,
                            openapiv3::Parameter::Cookie { parameter_data, .. } => parameter_data,
                        };
                        let name_match = pdata.name.to_lowercase().contains(&term_lower);
                        let desc_match = pdata
                            .description
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&term_lower);
                        name_match || desc_match
                    } else {
                        false
                    }
                });

                if path_match || summary_match || desc_match || param_match {
                    let resource_slug = op
                        .tags
                        .first()
                        .map(|t| slugify(t))
                        .unwrap_or_default();

                    endpoints.push(EndpointMatch {
                        method: method.to_string(),
                        path: path_str.clone(),
                        summary: op.summary.clone(),
                        resource_slug,
                    });
                }
            }
        }
    }

    // Search schemas
    let schemas: Vec<SchemaMatch> = list_schemas(api)
        .into_iter()
        .filter(|name| name.to_lowercase().contains(&term_lower))
        .map(|name| SchemaMatch { name })
        .collect();

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

    SearchResults {
        term: term.to_string(),
        resources,
        endpoints,
        schemas,
        callbacks,
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
            results.endpoints.iter().map(|e| &e.path).collect::<Vec<_>>()
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
}
