use crate::models::resource::{CallbackEntry, CallbackOperation, CallbackResponse};

/// Build CallbackEntry items for all named callbacks on a single operation.
pub fn extract_callbacks_from_operation(
    operation: &openapiv3::Operation,
    method: &str,
    path: &str,
) -> Vec<CallbackEntry> {
    operation
        .callbacks
        .iter()
        .filter_map(|(callback_name, callback)| {
            build_callback_entry(callback_name, callback, operation, method, path)
        })
        .collect()
}

fn build_callback_entry(
    callback_name: &str,
    callback: &openapiv3::Callback,
    operation: &openapiv3::Operation,
    method: &str,
    path: &str,
) -> Option<CallbackEntry> {
    use crate::spec;

    let operations: Vec<CallbackOperation> = callback
        .iter()
        .flat_map(|(url_expr, path_item)| {
            let cb_methods: &[(&str, &Option<openapiv3::Operation>)] = &[
                ("POST", &path_item.post), ("GET", &path_item.get),
                ("PUT", &path_item.put), ("DELETE", &path_item.delete),
                ("PATCH", &path_item.patch),
            ];
            cb_methods.iter().filter_map(|&(m, op_opt)| {
                let op = op_opt.as_ref()?;

                let body_schema = op.request_body.as_ref().and_then(|rb_ref| {
                    let rb = match rb_ref {
                        openapiv3::ReferenceOr::Item(rb) => rb,
                        _ => return None,
                    };
                    let media = rb.content.get("application/json")
                        .or_else(|| rb.content.values().next())?;
                    match media.schema.as_ref()? {
                        openapiv3::ReferenceOr::Reference { reference } => {
                            spec::schema_name_from_ref(reference).map(|s| s.to_string())
                        }
                        openapiv3::ReferenceOr::Item(_) => {
                            Some("inline object".to_string())
                        }
                    }
                });

                let responses: Vec<CallbackResponse> = op
                    .responses
                    .responses
                    .iter()
                    .map(|(status, resp_ref)| {
                        let code = match status {
                            openapiv3::StatusCode::Code(c) => c.to_string(),
                            openapiv3::StatusCode::Range(r) => format!("{}XX", r),
                        };
                        let desc = match resp_ref {
                            openapiv3::ReferenceOr::Item(r) => r.description.clone(),
                            _ => String::new(),
                        };
                        CallbackResponse { status_code: code, description: desc }
                    })
                    .collect();

                Some(CallbackOperation {
                    method: m.to_string(),
                    url_expression: url_expr.clone(),
                    summary: op.summary.clone(),
                    body_schema,
                    responses,
                })
            }).collect::<Vec<_>>()
        })
        .collect();

    if operations.is_empty() {
        return None;
    }

    Some(CallbackEntry {
        name: callback_name.to_string(),
        defined_on_operation_id: operation.operation_id.clone(),
        defined_on_method: method.to_uppercase(),
        defined_on_path: path.to_string(),
        operations,
    })
}

/// Returns all callbacks defined anywhere in the spec.
pub fn list_all_callbacks(api: &openapiv3::OpenAPI) -> Vec<CallbackEntry> {
    let mut entries: Vec<CallbackEntry> = Vec::new();

    for (path_str, path_item_ref) in &api.paths.paths {
        let path_item = match path_item_ref {
            openapiv3::ReferenceOr::Item(item) => item,
            _ => continue,
        };

        let methods: &[(&str, &Option<openapiv3::Operation>)] = &[
            ("GET", &path_item.get), ("POST", &path_item.post), ("PUT", &path_item.put),
            ("DELETE", &path_item.delete), ("PATCH", &path_item.patch),
            ("HEAD", &path_item.head), ("OPTIONS", &path_item.options), ("TRACE", &path_item.trace),
        ];

        for &(method, op_opt) in methods {
            if let Some(op) = op_opt {
                let mut found = extract_callbacks_from_operation(op, method, path_str);
                entries.append(&mut found);
            }
        }
    }

    entries
}

/// Find a specific callback by name across all operations.
/// Callback name matching is case-sensitive.
pub fn find_callback(api: &openapiv3::OpenAPI, name: &str) -> Option<CallbackEntry> {
    list_all_callbacks(api)
        .into_iter()
        .find(|e| e.name == name)
}

pub fn suggest_similar_callbacks<'a>(all: &'a [CallbackEntry], name: &str) -> Vec<&'a str> {
    let name_lower = name.to_lowercase();
    all.iter()
        .filter(|cb| strsim::jaro_winkler(&name_lower, &cb.name.to_lowercase()) > 0.8)
        .take(3)
        .map(|cb| cb.name.as_str())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_kitchen_sink() -> openapiv3::OpenAPI {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let content =
            std::fs::read_to_string(manifest_dir.join("tests/fixtures/kitchen-sink.yaml")).unwrap();
        serde_yaml_ng::from_str(&content).unwrap()
    }

    #[test]
    fn test_list_all_callbacks_finds_on_event() {
        let api = load_kitchen_sink();
        let callbacks = list_all_callbacks(&api);
        let names: Vec<&str> = callbacks.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"onEvent"), "missing onEvent: {:?}", names);
        assert!(names.contains(&"onStatusChange"), "missing onStatusChange: {:?}", names);
    }

    #[test]
    fn test_find_callback_by_name() {
        let api = load_kitchen_sink();
        let cb = find_callback(&api, "onEvent");
        assert!(cb.is_some(), "onEvent should be findable");
        let cb = cb.unwrap();
        assert_eq!(cb.defined_on_method, "POST");
        assert_eq!(cb.defined_on_path, "/notifications/subscribe");
    }

    #[test]
    fn test_find_callback_not_found() {
        let api = load_kitchen_sink();
        let cb = find_callback(&api, "nonexistent");
        assert!(cb.is_none());
    }
}
