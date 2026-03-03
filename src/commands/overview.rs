use crate::spec::LoadedSpec;

#[derive(Debug, serde::Serialize)]
pub struct OverviewData {
    pub title: String,
    pub description: Option<String>,
    pub base_urls: Vec<String>,
    pub server_variables: Vec<ServerVar>,
    pub auth_schemes: Vec<String>,
    pub resource_count: usize,
    pub endpoint_count: usize,
    pub path_count: usize,
    pub schema_count: usize,
    pub callback_count: usize,
    pub top_resources: Vec<(String, usize)>, // (slug, endpoint_count)
}

#[derive(Debug, serde::Serialize)]
pub struct ServerVar {
    pub name: String,
    pub required: bool,
    pub description: Option<String>,
    pub default: Option<String>,
}

pub fn build(loaded: &LoadedSpec) -> OverviewData {
    let title = loaded.api.info.title.clone();
    let description = loaded.api.info.description.as_ref().map(|d| {
        if d.chars().count() > 200 {
            let truncated: String = d.chars().take(200).collect();
            format!("{}...", truncated)
        } else {
            d.clone()
        }
    });

    let config_vars = loaded.config.variables.as_ref();

    let mut base_urls = Vec::new();
    let mut server_variables = Vec::new();

    for server in &loaded.api.servers {
        let mut url = server.url.clone();

        // Resolve variables in the URL template
        if let Some(ref vars) = server.variables {
            for (var_name, var) in vars {
                let value = config_vars
                    .and_then(|cv| cv.get(var_name))
                    .unwrap_or(&var.default);

                url = url.replace(&format!("{{{}}}", var_name), value);

                server_variables.push(ServerVar {
                    name: var_name.clone(),
                    required: false, // OpenAPI 3 server vars always have a default
                    description: var.description.clone(),
                    default: Some(var.default.clone()),
                });
            }
        }

        base_urls.push(url);
    }

    let auth_schemes = loaded
        .api
        .components
        .as_ref()
        .map(|c| c.security_schemes.keys().cloned().collect())
        .unwrap_or_default();

    let resource_groups = crate::commands::resources::extract_resource_groups(&loaded.api);
    let resource_count = resource_groups.len();
    let endpoint_count: usize = resource_groups.iter().map(|g| g.endpoints.len()).sum();
    let path_count = loaded.api.paths.paths.len();

    let schema_count = loaded
        .api
        .components
        .as_ref()
        .map(|c| c.schemas.len())
        .unwrap_or(0);

    let callback_count = crate::commands::callbacks::list_all_callbacks(&loaded.api).len();

    let mut top_resources: Vec<(String, usize)> = resource_groups
        .iter()
        .map(|g| (g.slug.clone(), g.endpoints.len()))
        .collect();
    top_resources.sort_by(|a, b| b.1.cmp(&a.1));
    top_resources.truncate(5);

    OverviewData {
        title,
        description,
        base_urls,
        server_variables,
        auth_schemes,
        resource_count,
        endpoint_count,
        path_count,
        schema_count,
        callback_count,
        top_resources,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec;

    fn load_kitchen_sink() -> crate::spec::LoadedSpec {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = manifest_dir.join("tests/fixtures/kitchen-sink.yaml");
        spec::load_spec(Some(path.to_str().unwrap()), manifest_dir).unwrap()
    }

    #[test]
    fn test_top_resources_present_and_sorted() {
        let loaded = load_kitchen_sink();
        let data = build(&loaded);
        assert!(
            !data.top_resources.is_empty(),
            "top_resources should be populated"
        );
        // Verify sorted descending by count
        for window in data.top_resources.windows(2) {
            assert!(
                window[0].1 >= window[1].1,
                "top_resources should be sorted descending: {:?}",
                data.top_resources
            );
        }
        // At most 5
        assert!(data.top_resources.len() <= 5);
    }
}
