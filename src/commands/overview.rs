use crate::spec::LoadedSpec;

#[derive(Debug, serde::Serialize)]
pub struct OverviewData {
    pub title: String,
    pub description: Option<String>,
    pub base_urls: Vec<String>,
    pub server_variables: Vec<ServerVar>,
    pub auth_schemes: Vec<String>,
    pub resource_count: usize,
    pub schema_count: usize,
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

    let resource_count = crate::commands::resources::extract_resource_groups(&loaded.api).len();

    let schema_count = loaded
        .api
        .components
        .as_ref()
        .map(|c| c.schemas.len())
        .unwrap_or(0);

    OverviewData {
        title,
        description,
        base_urls,
        server_variables,
        auth_schemes,
        resource_count,
        schema_count,
    }
}
