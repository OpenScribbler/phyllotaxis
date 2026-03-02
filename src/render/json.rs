use crate::commands::overview::OverviewData;
use crate::models::resource::ResourceGroup;

#[derive(serde::Serialize)]
struct SchemaDetailJson<'a> {
    name: &'a str,
    title: Option<&'a str>,
    description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_type: Option<&'a str>,
    composition: Option<CompositionJson>,
    discriminator: Option<DiscriminatorJson<'a>>,
    fields: Vec<FieldJson<'a>>,
    external_docs: Option<ExternalDocJson<'a>>,
    drill_deeper: Vec<String>,
}

#[derive(serde::Serialize)]
struct CompositionJson {
    #[serde(rename = "type")]
    composition_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    variants: Option<Vec<String>>,
}

#[derive(serde::Serialize)]
struct DiscriminatorJson<'a> {
    property_name: &'a str,
    mapping: Vec<DiscriminatorMappingEntry<'a>>,
}

#[derive(serde::Serialize)]
struct DiscriminatorMappingEntry<'a> {
    value: &'a str,
    schema: &'a str,
}

#[derive(serde::Serialize)]
struct FieldJson<'a> {
    name: &'a str,
    #[serde(rename = "type")]
    type_display: &'a str,
    required: bool,
    optional: bool,
    nullable: bool,
    read_only: bool,
    write_only: bool,
    deprecated: bool,
    description: Option<&'a str>,
    enum_values: &'a [String],
    constraints: &'a [String],
    default: Option<&'a serde_json::Value>,
    nested_schema: Option<&'a str>,
    nested_fields: Vec<FieldJson<'a>>,
}

#[derive(serde::Serialize)]
struct ExternalDocJson<'a> {
    url: &'a str,
    description: Option<&'a str>,
}

fn convert_fields<'a>(fields: &'a [crate::models::resource::Field]) -> Vec<FieldJson<'a>> {
    fields
        .iter()
        .map(|f| FieldJson {
            name: &f.name,
            type_display: &f.type_display,
            required: f.required,
            optional: f.optional,
            nullable: f.nullable,
            read_only: f.read_only,
            write_only: f.write_only,
            deprecated: f.deprecated,
            description: f.description.as_deref(),
            enum_values: &f.enum_values,
            constraints: &f.constraints,
            default: f.default_value.as_ref(),
            nested_schema: f.nested_schema_name.as_deref(),
            nested_fields: convert_fields(&f.nested_fields),
        })
        .collect()
}

fn serialize<T: serde::Serialize>(value: &T, is_tty: bool) -> String {
    if is_tty {
        serde_json::to_string_pretty(value).expect("serialize to JSON")
    } else {
        serde_json::to_string(value).expect("serialize to JSON")
    }
}

pub fn render_overview(data: &OverviewData, bin_name: &str, is_tty: bool) -> String {
    #[derive(serde::Serialize)]
    struct OverviewJson<'a> {
        title: &'a str,
        description: Option<&'a str>,
        servers: Vec<ServerJson<'a>>,
        auth: &'a [String],
        resource_count: usize,
        endpoint_count: usize,
        path_count: usize,
        schema_count: usize,
        callback_count: usize,
        commands: CommandsJson,
    }

    #[derive(serde::Serialize)]
    struct ServerJson<'a> {
        url: &'a str,
        variables: Vec<&'a crate::commands::overview::ServerVar>,
    }

    #[derive(serde::Serialize)]
    struct CommandsJson {
        resources: String,
        schemas: String,
        auth: String,
        search: String,
    }

    // Group server variables by URL (for now, all variables go with the first server)
    let servers: Vec<ServerJson> = data
        .base_urls
        .iter()
        .enumerate()
        .map(|(i, url)| {
            let variables = if i == 0 {
                data.server_variables.iter().collect()
            } else {
                vec![]
            };
            ServerJson {
                url: url.as_str(),
                variables,
            }
        })
        .collect();

    let json = OverviewJson {
        title: &data.title,
        description: data.description.as_deref(),
        servers,
        auth: &data.auth_schemes,
        resource_count: data.resource_count,
        endpoint_count: data.endpoint_count,
        path_count: data.path_count,
        schema_count: data.schema_count,
        callback_count: data.callback_count,
        commands: CommandsJson {
            resources: format!("{} resources", bin_name),
            schemas: format!("{} schemas", bin_name),
            auth: format!("{} auth", bin_name),
            search: format!("{} search", bin_name),
        },
    };

    serialize(&json, is_tty)
}

pub fn render_resource_list(groups: &[ResourceGroup], bin_name: &str, is_tty: bool) -> String {
    #[derive(serde::Serialize)]
    struct ResourceListJson {
        resources: Vec<ResourceItemJson>,
        drill_deeper: String,
    }

    #[derive(serde::Serialize)]
    struct ResourceItemJson {
        slug: String,
        display_name: String,
        description: Option<String>,
        deprecated: bool,
        alpha: bool,
        endpoint_count: usize,
    }

    let resources = groups
        .iter()
        .map(|g| ResourceItemJson {
            slug: g.slug.clone(),
            display_name: g.display_name.clone(),
            description: g.description.clone(),
            deprecated: g.is_deprecated,
            alpha: g.is_alpha,
            endpoint_count: g.endpoints.len(),
        })
        .collect();

    let json = ResourceListJson {
        resources,
        drill_deeper: format!("{} resources <name>", bin_name),
    };

    serialize(&json, is_tty)
}

pub fn render_resource_detail(group: &ResourceGroup, bin_name: &str, is_tty: bool) -> String {
    #[derive(serde::Serialize)]
    struct ResourceDetailJson<'a> {
        slug: &'a str,
        display_name: &'a str,
        description: Option<&'a str>,
        deprecated: bool,
        alpha: bool,
        endpoints: Vec<EndpointSummaryJson<'a>>,
        drill_deeper: Vec<String>,
    }

    #[derive(serde::Serialize)]
    struct EndpointSummaryJson<'a> {
        method: &'a str,
        path: &'a str,
        summary: Option<&'a str>,
        deprecated: bool,
        alpha: bool,
    }

    let endpoints: Vec<EndpointSummaryJson> = group
        .endpoints
        .iter()
        .map(|e| EndpointSummaryJson {
            method: &e.method,
            path: &e.path,
            summary: e.summary.as_deref(),
            deprecated: e.is_deprecated,
            alpha: e.is_alpha,
        })
        .collect();

    let drill_deeper: Vec<String> = group
        .endpoints
        .iter()
        .map(|e| {
            format!(
                "{} resources {} {} {}",
                bin_name, group.slug, e.method, e.path
            )
        })
        .collect();

    let json = ResourceDetailJson {
        slug: &group.slug,
        display_name: &group.display_name,
        description: group.description.as_deref(),
        deprecated: group.is_deprecated,
        alpha: group.is_alpha,
        endpoints,
        drill_deeper,
    };

    serialize(&json, is_tty)
}

pub fn render_schema_list(names: &[String], bin_name: &str, is_tty: bool) -> String {
    #[derive(serde::Serialize)]
    struct SchemaListJson<'a> {
        schemas: &'a [String],
        total: usize,
        drill_deeper: String,
    }

    let json = SchemaListJson {
        total: names.len(),
        schemas: names,
        drill_deeper: format!("{} schemas <name>", bin_name),
    };

    serialize(&json, is_tty)
}

pub fn render_schema_detail(model: &crate::models::schema::SchemaModel, bin_name: &str, is_tty: bool) -> String {
    use crate::models::schema::Composition;

    let composition = model.composition.as_ref().map(|c| match c {
        Composition::AllOf => CompositionJson {
            composition_type: "allOf",
            variants: None,
        },
        Composition::OneOf(v) => CompositionJson {
            composition_type: "oneOf",
            variants: Some(v.clone()),
        },
        Composition::AnyOf(v) => CompositionJson {
            composition_type: "anyOf",
            variants: Some(v.clone()),
        },
        Composition::Enum(v) => CompositionJson {
            composition_type: "enum",
            variants: Some(v.clone()),
        },
    });

    let discriminator = model.discriminator.as_ref().map(|d| DiscriminatorJson {
        property_name: &d.property_name,
        mapping: d
            .mapping
            .iter()
            .map(|(k, v)| DiscriminatorMappingEntry {
                value: k.as_str(),
                schema: v.as_str(),
            })
            .collect(),
    });

    // Collect unique nested schema names for drill_deeper
    let drill_deeper: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        model
            .fields
            .iter()
            .filter_map(|f| f.nested_schema_name.as_ref())
            .filter(|name| seen.insert(name.to_string()))
            .map(|name| format!("{} schemas {}", bin_name, name))
            .collect()
    };

    let json = SchemaDetailJson {
        name: &model.name,
        title: model.title.as_deref(),
        description: model.description.as_deref(),
        base_type: model.base_type.as_deref(),
        composition,
        discriminator,
        fields: convert_fields(&model.fields),
        external_docs: model.external_docs.as_ref().map(|d| ExternalDocJson {
            url: &d.url,
            description: d.description.as_deref(),
        }),
        drill_deeper,
    };

    serialize(&json, is_tty)
}

pub fn render_callback_list(callbacks: &[crate::models::resource::CallbackEntry], bin_name: &str, is_tty: bool) -> String {
    #[derive(serde::Serialize)]
    struct CallbackListJson<'a> {
        total: usize,
        callbacks: Vec<CallbackSummaryJson<'a>>,
        drill_deeper: String,
    }
    #[derive(serde::Serialize)]
    struct CallbackSummaryJson<'a> {
        name: &'a str,
        defined_on_method: &'a str,
        defined_on_path: &'a str,
    }

    let items: Vec<_> = callbacks.iter().map(|cb| CallbackSummaryJson {
        name: &cb.name,
        defined_on_method: &cb.defined_on_method,
        defined_on_path: &cb.defined_on_path,
    }).collect();

    let json = CallbackListJson {
        total: items.len(),
        callbacks: items,
        drill_deeper: format!("{} callbacks <name>", bin_name),
    };
    serialize(&json, is_tty)
}

pub fn render_callback_detail(cb: &crate::models::resource::CallbackEntry, _bin_name: &str, is_tty: bool) -> String {
    serialize(cb, is_tty)
}

pub fn render_search(results: &crate::commands::search::SearchResults, _bin_name: &str, is_tty: bool) -> String {
    serialize(results, is_tty)
}

pub fn render_auth(model: &crate::commands::auth::AuthModel, _bin_name: &str, is_tty: bool) -> String {
    serialize(model, is_tty)
}

pub fn render_endpoint_detail(endpoint: &crate::models::resource::Endpoint, is_tty: bool) -> String {
    // Endpoint already derives Serialize, so we can use it directly
    serialize(endpoint, is_tty)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_json(s: &str) -> serde_json::Value {
        serde_json::from_str(s).unwrap_or_else(|_| {
            panic!("Invalid JSON: {}", &s[..200.min(s.len())])
        })
    }

    #[test]
    fn test_all_json_outputs_parse() {
        use crate::commands::overview::OverviewData;
        use crate::commands::auth::{AuthModel, SecuritySchemeInfo};
        use crate::commands::search::SearchResults;
        use crate::models::schema::SchemaModel;
        use crate::models::resource::Endpoint;

        // Overview
        let overview = OverviewData {
            title: "Test API".to_string(),
            description: None,
            base_urls: vec!["https://api.example.com".to_string()],
            server_variables: vec![],
            auth_schemes: vec![],
            resource_count: 0,
            endpoint_count: 0,
            path_count: 0,
            schema_count: 0,
            callback_count: 0,
        };
        let v = parse_json(&render_overview(&overview, "phyllotaxis", false));
        assert_eq!(v["title"], "Test API");
        assert!(v["description"].is_null());

        // Resource list
        parse_json(&render_resource_list(&[], "phyllotaxis", false));

        // Resource detail
        let group = ResourceGroup {
            slug: "test".to_string(),
            display_name: "Test".to_string(),
            description: None,
            is_deprecated: false,
            is_alpha: false,
            endpoints: vec![],
        };
        let v = parse_json(&render_resource_detail(&group, "phyllotaxis", false));
        assert_eq!(v["deprecated"], false);

        // Schema list
        parse_json(&render_schema_list(&["Pet".to_string()], "phyllotaxis", false));

        // Schema detail
        let model = SchemaModel {
            name: "Pet".to_string(),
            title: None,
            description: None,
            fields: vec![],
            composition: None,
            discriminator: None,
            external_docs: None,
            base_type: None,
        };
        let v = parse_json(&render_schema_detail(&model, "phyllotaxis", false));
        assert!(v["composition"].is_null());
        assert!(v["discriminator"].is_null());
        assert!(v["drill_deeper"].is_array());

        // Auth
        let auth = AuthModel {
            schemes: vec![SecuritySchemeInfo {
                name: "test".to_string(),
                scheme_type: "http".to_string(),
                detail: "bearer".to_string(),
                description: None,
                usage_count: 3,
                oauth2_flows: vec![],
            }],
            total_operations: 3,
        };
        let v = parse_json(&render_auth(&auth, "phyllotaxis", false));
        assert!(v["schemes"].is_array());

        // Search
        let results = SearchResults {
            term: "test".to_string(),
            resources: vec![],
            endpoints: vec![],
            schemas: vec![],
            callbacks: vec![],
            suggestions: vec![],
        };
        let v = parse_json(&render_search(&results, "phyllotaxis", false));
        assert_eq!(v["term"], "test");
        assert!(v["resources"].is_array());
        assert!(v["suggestions"].is_array());

        // Endpoint detail
        let endpoint = Endpoint {
            method: "GET".to_string(),
            path: "/test".to_string(),
            summary: None,
            description: None,
            is_deprecated: false,
            is_alpha: false,
            external_docs: None,
            parameters: vec![],
            request_body: None,
            responses: vec![],
            security_schemes: vec![],
            callbacks: vec![],
            links: vec![],
            drill_deeper: vec![],
        };
        let v = parse_json(&render_endpoint_detail(&endpoint, false));
        assert_eq!(v["method"], "GET");
        assert_eq!(v["is_deprecated"], false);
        assert!(v["drill_deeper"].is_array(), "drill_deeper should be present as array");

        // Schema with title (Task 20)
        let model_with_title = SchemaModel {
            name: "GeoLocation".to_string(),
            title: Some("Geographic Location".to_string()),
            description: None,
            fields: vec![],
            composition: None,
            discriminator: None,
            external_docs: None,
            base_type: None,
        };
        let v = parse_json(&render_schema_detail(&model_with_title, "phyllotaxis", false));
        assert_eq!(v["title"], "Geographic Location", "JSON should include title");

        // Field with new properties (Task 20)
        use crate::models::resource::{RequestBody, Field};
        let endpoint_with_new_fields = Endpoint {
            method: "POST".to_string(),
            path: "/test".to_string(),
            summary: None,
            description: None,
            is_deprecated: false,
            is_alpha: false,
            external_docs: None,
            parameters: vec![],
            request_body: Some(RequestBody {
                content_type: "application/json".to_string(),
                fields: vec![Field {
                    name: "password".to_string(),
                    type_display: "string".to_string(),
                    required: true,
                    optional: false,
                    nullable: false,
                    read_only: false,
                    write_only: true,
                    deprecated: false,
                    description: None,
                    enum_values: vec![],
                    constraints: vec!["min:8".to_string()],
                    default_value: None,
                    example: None,
                    nested_schema_name: None,
                    nested_fields: vec![],
                }],
                options: vec![],
                schema_ref: None,
                example: None,
                array_item_type: None,
            }),
            responses: vec![],
            security_schemes: vec![],
            callbacks: vec![],
            links: vec![],
            drill_deeper: vec![],
        };
        let v = parse_json(&render_endpoint_detail(&endpoint_with_new_fields, false));
        let fields = &v["request_body"]["fields"][0];
        assert_eq!(fields["write_only"], true);
        assert_eq!(fields["deprecated"], false);
        assert!(fields["constraints"].is_array());
    }

    #[test]
    fn test_pretty_vs_compact() {
        let group = ResourceGroup {
            slug: "test".to_string(),
            display_name: "Test".to_string(),
            description: None,
            is_deprecated: false,
            is_alpha: false,
            endpoints: vec![],
        };

        let pretty = render_resource_detail(&group, "phyllotaxis", true);
        let compact = render_resource_detail(&group, "phyllotaxis", false);

        // Pretty has newlines; compact is a single line
        assert!(pretty.contains('\n'), "TTY output should be pretty-printed");
        assert!(!compact.contains('\n'), "non-TTY output should be compact");

        // Both parse to the same value
        let v_pretty: serde_json::Value = serde_json::from_str(&pretty).unwrap();
        let v_compact: serde_json::Value = serde_json::from_str(&compact).unwrap();
        assert_eq!(v_pretty, v_compact);
    }

    #[test]
    fn test_endpoint_detail_json_includes_drill_deeper() {
        use crate::models::resource::Endpoint;

        let endpoint = Endpoint {
            method: "GET".to_string(),
            path: "/pets/{id}".to_string(),
            summary: None,
            description: None,
            is_deprecated: false,
            is_alpha: false,
            external_docs: None,
            parameters: vec![],
            request_body: None,
            responses: vec![],
            security_schemes: vec![],
            callbacks: vec![],
            links: vec![],
            drill_deeper: vec!["phyllotaxis schemas Pet".to_string()],
        };
        let v = parse_json(&render_endpoint_detail(&endpoint, false));
        assert_eq!(
            v["drill_deeper"],
            serde_json::json!(["phyllotaxis schemas Pet"]),
            "drill_deeper should contain the schema command"
        );
    }

    // ─── Task 21: Endpoint JSON includes headers, links, callbacks ───

    #[test]
    fn test_endpoint_json_includes_new_fields() {
        use crate::models::resource::*;

        let endpoint = Endpoint {
            method: "GET".to_string(),
            path: "/users".to_string(),
            summary: None,
            description: None,
            is_deprecated: false,
            is_alpha: false,
            external_docs: None,
            parameters: vec![],
            request_body: None,
            responses: vec![Response {
                status_code: "200".to_string(),
                description: "OK".to_string(),
                schema_ref: None,
                example: None,
                headers: vec![ResponseHeader {
                    name: "X-Total-Count".to_string(),
                    type_display: "integer".to_string(),
                    description: None,
                }],
                links: vec![],
                fields: vec![],
            }],
            security_schemes: vec![],
            callbacks: vec![],
            links: vec![],
            drill_deeper: vec![],
        };

        let v = parse_json(&render_endpoint_detail(&endpoint, false));
        assert!(v["responses"][0]["headers"].is_array(), "headers should be array in JSON");
        assert_eq!(v["responses"][0]["headers"][0]["name"], "X-Total-Count");
        assert!(v["callbacks"].is_array(), "callbacks should be present as array");
        assert!(v.get("links").is_none(), "top-level links should not be serialized in JSON");
    }
}
