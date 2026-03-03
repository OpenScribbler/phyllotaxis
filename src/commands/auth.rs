#[derive(Debug, serde::Serialize)]
pub struct AuthModel {
    pub schemes: Vec<SecuritySchemeInfo>,
    pub total_operations: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct SecuritySchemeInfo {
    pub name: String,
    pub scheme_type: String,
    pub detail: String,
    pub description: Option<String>,
    pub usage_count: usize,
    /// Non-empty only for OAuth2 schemes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub oauth2_flows: Vec<OAuth2FlowInfo>,
}

#[derive(Debug, serde::Serialize)]
pub struct OAuth2FlowInfo {
    pub flow_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_url: Option<String>,
    pub scopes: Vec<OAuth2ScopeInfo>,
}

#[derive(Debug, serde::Serialize)]
pub struct OAuth2ScopeInfo {
    pub name: String,
    pub description: String,
}

pub fn build_auth_model(api: &openapiv3::OpenAPI) -> AuthModel {
    let total_operations = count_operations(api);

    let schemes = api
        .components
        .as_ref()
        .map(|c| &c.security_schemes)
        .into_iter()
        .flat_map(|schemes| schemes.iter())
        .filter_map(|(name, ref_or)| {
            let scheme = match ref_or {
                openapiv3::ReferenceOr::Item(s) => s,
                _ => return None,
            };

            let (scheme_type, detail, description, oauth2_flows) = match scheme {
                openapiv3::SecurityScheme::HTTP {
                    scheme,
                    description,
                    ..
                } => (
                    "http".to_string(),
                    scheme.clone(),
                    description.clone(),
                    vec![],
                ),
                openapiv3::SecurityScheme::APIKey {
                    location,
                    name,
                    description,
                    ..
                } => {
                    let loc = match location {
                        openapiv3::APIKeyLocation::Query => "query",
                        openapiv3::APIKeyLocation::Header => "header",
                        openapiv3::APIKeyLocation::Cookie => "cookie",
                    };
                    (
                        "apiKey".to_string(),
                        format!("{} ({})", name, loc),
                        description.clone(),
                        vec![],
                    )
                }
                openapiv3::SecurityScheme::OAuth2 {
                    flows, description, ..
                } => {
                    let oauth2_flows = extract_oauth2_flows(flows);
                    let detail = oauth2_flows
                        .iter()
                        .map(|f| f.flow_type.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    (
                        "oauth2".to_string(),
                        detail,
                        description.clone(),
                        oauth2_flows,
                    )
                }
                openapiv3::SecurityScheme::OpenIDConnect { description, .. } => (
                    "openIdConnect".to_string(),
                    "openIdConnect".to_string(),
                    description.clone(),
                    vec![],
                ),
            };

            let usage_count = count_scheme_usage(api, name);

            Some(SecuritySchemeInfo {
                name: name.clone(),
                scheme_type,
                detail,
                description,
                usage_count,
                oauth2_flows,
            })
        })
        .collect();

    AuthModel {
        schemes,
        total_operations,
    }
}

fn scopes_from_map(map: &indexmap::IndexMap<String, String>) -> Vec<OAuth2ScopeInfo> {
    map.iter()
        .map(|(name, desc)| OAuth2ScopeInfo {
            name: name.clone(),
            description: desc.clone(),
        })
        .collect()
}

fn extract_oauth2_flows(flows: &openapiv3::OAuth2Flows) -> Vec<OAuth2FlowInfo> {
    let mut result = Vec::new();

    if let Some(ref f) = flows.implicit {
        result.push(OAuth2FlowInfo {
            flow_type: "implicit".to_string(),
            authorization_url: Some(f.authorization_url.clone()),
            token_url: None,
            refresh_url: f.refresh_url.clone(),
            scopes: scopes_from_map(&f.scopes),
        });
    }
    if let Some(ref f) = flows.authorization_code {
        result.push(OAuth2FlowInfo {
            flow_type: "authorizationCode".to_string(),
            authorization_url: Some(f.authorization_url.clone()),
            token_url: Some(f.token_url.clone()),
            refresh_url: f.refresh_url.clone(),
            scopes: scopes_from_map(&f.scopes),
        });
    }
    if let Some(ref f) = flows.client_credentials {
        result.push(OAuth2FlowInfo {
            flow_type: "clientCredentials".to_string(),
            authorization_url: None,
            token_url: Some(f.token_url.clone()),
            refresh_url: f.refresh_url.clone(),
            scopes: scopes_from_map(&f.scopes),
        });
    }
    if let Some(ref f) = flows.password {
        result.push(OAuth2FlowInfo {
            flow_type: "password".to_string(),
            authorization_url: None,
            token_url: Some(f.token_url.clone()),
            refresh_url: f.refresh_url.clone(),
            scopes: scopes_from_map(&f.scopes),
        });
    }

    result
}

fn count_operations(api: &openapiv3::OpenAPI) -> usize {
    let mut count = 0;
    for (_path, item) in api.paths.iter() {
        if let openapiv3::ReferenceOr::Item(item) = item {
            for op in [
                &item.get,
                &item.put,
                &item.post,
                &item.delete,
                &item.options,
                &item.head,
                &item.patch,
                &item.trace,
            ] {
                if op.is_some() {
                    count += 1;
                }
            }
        }
    }
    count
}

fn count_scheme_usage(api: &openapiv3::OpenAPI, scheme_name: &str) -> usize {
    // Check if used globally
    let global = api
        .security
        .as_ref()
        .map(|reqs| reqs.iter().any(|req| req.contains_key(scheme_name)))
        .unwrap_or(false);

    let mut count = 0;
    for (_path, item) in api.paths.iter() {
        if let openapiv3::ReferenceOr::Item(item) = item {
            for op in [
                &item.get,
                &item.put,
                &item.post,
                &item.delete,
                &item.options,
                &item.head,
                &item.patch,
                &item.trace,
            ]
            .into_iter()
            .flatten()
            {
                // If the operation has its own security, check that
                // If not, fall back to global
                if let Some(ref op_security) = op.security {
                    if op_security.iter().any(|req| req.contains_key(scheme_name)) {
                        count += 1;
                    }
                } else if global {
                    count += 1;
                }
            }
        }
    }
    count
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
    fn test_build_auth_petstore() {
        let api = load_petstore_api();
        let model = build_auth_model(&api);
        assert_eq!(model.schemes.len(), 1, "Petstore has one security scheme");
        let scheme = &model.schemes[0];
        assert_eq!(scheme.name, "bearerAuth");
        assert_eq!(scheme.scheme_type, "http");
        assert_eq!(scheme.detail, "bearer");
        assert!(
            scheme.oauth2_flows.is_empty(),
            "HTTP scheme should have no OAuth2 flows"
        );
        assert!(
            scheme.usage_count > 0,
            "bearerAuth should be used by operations"
        );
        assert_eq!(
            scheme.usage_count, model.total_operations,
            "Global security should apply to all operations"
        );
    }
}
