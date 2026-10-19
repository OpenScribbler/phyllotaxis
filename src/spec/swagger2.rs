use std::collections::HashSet;

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};

const TYPE_KEYWORDS: &[&str] = &[
    "type",
    "format",
    "items",
    "default",
    "enum",
    "maximum",
    "exclusiveMaximum",
    "minimum",
    "exclusiveMinimum",
    "maxLength",
    "minLength",
    "pattern",
    "maxItems",
    "minItems",
    "uniqueItems",
    "multipleOf",
];

/// Keys whose values are maps keyed only by names. Every `components` section is one too.
const NAME_MAPS: &[&str] = &["properties", "headers", "content"];

/// Keys whose values are maps keyed by names that may also hold `x-` extensions.
const EXTENSIBLE_NAME_MAPS: &[&str] = &["paths", "responses"];

/// Keywords whose values are literal data, not document structure.
const LITERAL_KEYWORDS: &[&str] = &["example", "examples", "default", "enum"];

/// What the keys of a map are, which decides where `rewrite_refs` looks for `$ref`s.
#[derive(Clone, Copy)]
enum Keys {
    /// Keywords: literal values and `x-` extensions are data, not structure.
    Keywords,
    /// Names only, so a name such as `x-Error` or `example` is structure.
    Names,
    /// Names beside `x-` extensions, as in `paths` and an operation's `responses`.
    NamesAndExtensions,
}

struct Converter {
    parameters: Map<String, Value>,
    responses: Map<String, Value>,
    consumes: Vec<String>,
    produces: Vec<String>,
}

#[derive(Default)]
struct Parameters {
    regular: Vec<Value>,
    payload: Vec<Value>,
}

/// Whether `doc` declares `swagger: "2.0"`. An unquoted `swagger: 2.0` in YAML
/// parses as a number, so that counts too.
pub(super) fn declares_version_two(doc: &Value) -> bool {
    match doc.get("swagger") {
        Some(Value::String(version)) => version == "2.0",
        Some(Value::Number(version)) => version.is_f64() && version.as_f64() == Some(2.0),
        _ => false,
    }
}

pub(super) fn convert(doc: Value) -> Result<Value> {
    if !declares_version_two(&doc) {
        let version = doc
            .get("swagger")
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| v.to_string())
            })
            .unwrap_or_else(|| "<missing>".to_string());
        bail!("Unsupported Swagger version '{}'", version);
    }
    let mut root = into_object(doc, "Swagger document")?;
    let converter = Converter {
        parameters: take_object(&mut root, "parameters")?,
        responses: take_object(&mut root, "responses")?,
        consumes: media_types(root.get("consumes"), &["application/json".to_string()])?,
        produces: media_types(root.get("produces"), &["application/json".to_string()])?,
    };

    let host = root.remove("host");
    let base_path = root.remove("basePath");
    let schemes = root.remove("schemes");
    if let Some(host) = host {
        let host = host.as_str().context("Swagger host must be a string")?;
        let base_path = base_path
            .as_ref()
            .map(|v| v.as_str().context("Swagger basePath must be a string"))
            .transpose()?
            .unwrap_or("");
        let schemes = media_types(schemes.as_ref(), &["https".to_string()])?;
        root.insert(
            "servers".to_string(),
            Value::Array(
                schemes
                    .iter()
                    .map(|scheme| json!({"url": format!("{scheme}://{host}{base_path}")}))
                    .collect(),
            ),
        );
    } else if let Some(base_path) = base_path {
        root.insert("servers".to_string(), json!([{"url": base_path}]));
    }
    root.remove("swagger");
    root.remove("consumes");
    root.remove("produces");
    root.insert("openapi".to_string(), json!("3.0.3"));

    let mut components = take_object(&mut root, "components")?;
    if let Some(definitions) = root.remove("definitions") {
        let schemas = into_object(definitions, "Swagger definitions")?
            .into_iter()
            .map(|(name, schema)| (name, convert_schema(schema)))
            .collect();
        components.insert("schemas".to_string(), Value::Object(schemas));
    }
    let mut parameters = Map::new();
    let mut request_bodies = Map::new();
    for (name, parameter) in &converter.parameters {
        let resolved = converter.resolve_parameter(parameter)?;
        match resolved.get("in").and_then(Value::as_str) {
            Some("body") => {
                request_bodies.insert(name.clone(), body_parameter(resolved, &converter.consumes)?);
            }
            Some("formData") => {}
            _ => {
                parameters.insert(name.clone(), convert_parameter(resolved)?);
            }
        }
    }
    if !parameters.is_empty() {
        components.insert("parameters".to_string(), Value::Object(parameters));
    }
    if !request_bodies.is_empty() {
        components.insert("requestBodies".to_string(), Value::Object(request_bodies));
    }
    if !converter.responses.is_empty() {
        components.insert(
            "responses".to_string(),
            Value::Object(
                converter
                    .responses
                    .clone()
                    .into_iter()
                    .map(|(name, response)| {
                        Ok((name, convert_response(response, &converter.produces)?))
                    })
                    .collect::<Result<Map<_, _>>>()?,
            ),
        );
    }
    if let Some(security) = root.remove("securityDefinitions") {
        let schemes = into_object(security, "Swagger securityDefinitions")?
            .into_iter()
            .map(|(name, scheme)| Ok((name, convert_security_scheme(scheme)?)))
            .collect::<Result<Map<_, _>>>()?;
        components.insert("securitySchemes".to_string(), Value::Object(schemes));
    }
    if !components.is_empty() {
        root.insert("components".to_string(), Value::Object(components));
    }
    if let Some(paths) = root.get_mut("paths").and_then(Value::as_object_mut) {
        for (name, path) in paths {
            if !name.starts_with("x-") {
                converter.convert_path(path)?;
            }
        }
    }

    let mut output = Value::Object(root);
    converter.rewrite_refs(&mut output, Keys::Keywords)?;
    Ok(output)
}

fn into_object(value: Value, label: &str) -> Result<Map<String, Value>> {
    match value {
        Value::Object(map) => Ok(map),
        _ => bail!("{} must be an object", label),
    }
}

fn take_object(map: &mut Map<String, Value>, key: &str) -> Result<Map<String, Value>> {
    map.remove(key)
        .map(|value| into_object(value, key))
        .transpose()
        .map(Option::unwrap_or_default)
}

fn media_types(value: Option<&Value>, fallback: &[String]) -> Result<Vec<String>> {
    match value {
        None => Ok(fallback.to_vec()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .context("Swagger media types and schemes must be strings")
            })
            .collect(),
        Some(_) => bail!("Swagger media types and schemes must be arrays"),
    }
}

fn is_operation(key: &str) -> bool {
    matches!(
        key,
        "get" | "put" | "post" | "delete" | "options" | "head" | "patch" | "trace"
    )
}

impl Converter {
    // Resolve parameter references for classification, but retain the reference at
    // use sites unless formData must be inlined. A reference to a missing parameter
    // resolves to itself and stays a dangling reference, as it would in OpenAPI 3.
    // Detect aliases that form a cycle.
    fn resolve_parameter(&self, parameter: &Value) -> Result<Value> {
        let mut current = parameter;
        let mut visited = HashSet::new();
        while let Some(reference) = current.get("$ref").and_then(Value::as_str) {
            let Some(name) = reference.strip_prefix("#/parameters/") else {
                break;
            };
            if !visited.insert(name) {
                bail!("Circular Swagger parameter reference '{}'", reference);
            }
            let name = name.replace("~1", "/").replace("~0", "~");
            let Some(target) = self.parameters.get(&name) else {
                break;
            };
            current = target;
        }
        Ok(current.clone())
    }

    fn partition_parameters(&self, parameters: Option<Value>) -> Result<Parameters> {
        let mut result = Parameters::default();
        if let Some(parameters) = parameters {
            let values = parameters
                .as_array()
                .context("Swagger parameters must be an array")?;
            for parameter in values {
                let resolved = self.resolve_parameter(parameter)?;
                if matches!(
                    resolved.get("in").and_then(Value::as_str),
                    Some("body" | "formData")
                ) {
                    result.payload.push(parameter.clone());
                } else {
                    result.regular.push(convert_parameter(parameter.clone())?);
                }
            }
        }
        Ok(result)
    }

    fn convert_path(&self, path: &mut Value) -> Result<()> {
        let path = path
            .as_object_mut()
            .context("Swagger path must be an object")?;
        let inherited = self.partition_parameters(path.remove("parameters"))?;
        if !inherited.regular.is_empty() {
            path.insert("parameters".to_string(), json!(inherited.regular));
        }
        for (method, operation) in path {
            if !is_operation(method) {
                continue;
            }
            let op = operation
                .as_object_mut()
                .context("Swagger operation must be an object")?;
            let consumes = media_types(op.get("consumes"), &self.consumes)?;
            let produces = media_types(op.get("produces"), &self.produces)?;
            op.remove("consumes");
            op.remove("produces");
            let parameters = self.partition_parameters(op.remove("parameters"))?;
            if !parameters.regular.is_empty() {
                op.insert("parameters".to_string(), json!(parameters.regular));
            }
            let payload = if parameters.payload.is_empty() {
                &inherited.payload
            } else {
                &parameters.payload
            };
            if let Some(body) = self.request_body(payload, &consumes)? {
                op.insert("requestBody".to_string(), body);
            }
            if let Some(mut responses) = op.remove("responses") {
                // A shared response takes its media types from the operation that
                // uses it, so inline it where they differ from the global ones.
                if produces != self.produces {
                    self.inline_responses(&mut responses);
                }
                op.insert(
                    "responses".to_string(),
                    convert_responses(responses, &produces)?,
                );
            }
        }
        Ok(())
    }

    fn request_body(&self, parameters: &[Value], consumes: &[String]) -> Result<Option<Value>> {
        let mut forms = Vec::new();
        for parameter in parameters {
            let resolved = self.resolve_parameter(parameter)?;
            match resolved.get("in").and_then(Value::as_str) {
                Some("body") => {
                    // A shared body takes its media types from the operation that
                    // uses it, so keep the reference only when they match.
                    return Ok(Some(
                        if parameter.get("$ref").is_some() && consumes == self.consumes {
                            parameter.clone()
                        } else {
                            body_parameter(resolved, consumes)?
                        },
                    ));
                }
                Some("formData") => forms.push(resolved),
                _ => {}
            }
        }
        if forms.is_empty() {
            return Ok(None);
        }
        let mut properties = Map::new();
        let mut required = Vec::new();
        let mut has_file = false;
        for parameter in forms {
            let mut property = into_object(parameter, "Swagger formData parameter")?;
            let name = property
                .remove("name")
                .and_then(|v| v.as_str().map(str::to_owned))
                .context("Swagger formData parameter must have a name")?;
            has_file |= property.get("type").and_then(Value::as_str) == Some("file");
            if property.remove("required") == Some(Value::Bool(true)) {
                required.push(name.clone());
            }
            property.remove("in");
            property.remove("collectionFormat");
            property.remove("allowEmptyValue");
            properties.insert(name, convert_parameter_schema(Value::Object(property)));
        }
        let mut schema = json!({"type": "object", "properties": properties});
        let is_required = !required.is_empty();
        if is_required {
            schema["required"] = json!(required);
        }
        let mut types: Vec<String> = consumes
            .iter()
            .filter(|media| {
                matches!(
                    media.as_str(),
                    "application/x-www-form-urlencoded" | "multipart/form-data"
                )
            })
            .cloned()
            .collect();
        if types.is_empty() {
            types.push(
                if has_file {
                    "multipart/form-data"
                } else {
                    "application/x-www-form-urlencoded"
                }
                .to_string(),
            );
        }
        let mut body = json!({"content": content_for_schema(schema, &types)});
        if is_required {
            body["required"] = json!(true);
        }
        Ok(Some(body))
    }

    fn inline_responses(&self, responses: &mut Value) {
        let Some(responses) = responses.as_object_mut() else {
            return;
        };
        for (status, response) in responses.iter_mut() {
            if status.starts_with("x-") {
                continue;
            }
            let target = response
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|reference| reference.strip_prefix("#/responses/"))
                .map(|name| name.replace("~1", "/").replace("~0", "~"))
                .and_then(|name| self.responses.get(&name));
            if let Some(target) = target {
                *response = target.clone();
            }
        }
    }

    /// Rewrite Swagger `$ref`s to their OpenAPI 3 locations. Literal values such as
    /// examples and `x-` extensions keep any `$ref` key they contain unchanged.
    fn rewrite_refs(&self, value: &mut Value, keys: Keys) -> Result<()> {
        match value {
            Value::Object(map) => {
                if let Keys::Names | Keys::NamesAndExtensions = keys {
                    for (name, child) in map.iter_mut() {
                        if matches!(keys, Keys::NamesAndExtensions) && name.starts_with("x-") {
                            continue;
                        }
                        self.rewrite_refs(child, Keys::Keywords)?;
                    }
                    return Ok(());
                }
                if let Some(Value::String(reference)) = map.get_mut("$ref") {
                    for (old, new) in [
                        ("#/definitions/", "#/components/schemas/"),
                        ("#/responses/", "#/components/responses/"),
                        ("#/securityDefinitions/", "#/components/securitySchemes/"),
                    ] {
                        if let Some(name) = reference.strip_prefix(old) {
                            *reference = format!("{new}{name}");
                            break;
                        }
                    }
                    if let Some(name) = reference.strip_prefix("#/parameters/") {
                        let parameter = self.resolve_parameter(&json!({"$ref": reference}))?;
                        let component = match parameter.get("in").and_then(Value::as_str) {
                            Some("body") => "requestBodies",
                            _ => "parameters",
                        };
                        *reference = format!("#/components/{component}/{name}");
                    }
                }
                for (key, child) in map.iter_mut() {
                    if key.starts_with("x-") || LITERAL_KEYWORDS.contains(&key.as_str()) {
                        continue;
                    }
                    if key == "components" {
                        if let Some(sections) = child.as_object_mut() {
                            for section in sections.values_mut() {
                                self.rewrite_refs(section, Keys::Names)?;
                            }
                        }
                        continue;
                    }
                    let keys = if NAME_MAPS.contains(&key.as_str()) {
                        Keys::Names
                    } else if EXTENSIBLE_NAME_MAPS.contains(&key.as_str()) {
                        Keys::NamesAndExtensions
                    } else {
                        Keys::Keywords
                    };
                    self.rewrite_refs(child, keys)?;
                }
            }
            Value::Array(values) => {
                for child in values {
                    self.rewrite_refs(child, Keys::Keywords)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn convert_schema(mut schema: Value) -> Value {
    if let Some(map) = schema.as_object_mut() {
        if map.get("x-nullable").is_some_and(Value::is_boolean) {
            if let Some(nullable) = map.remove("x-nullable") {
                map.insert("nullable".to_string(), nullable);
            }
        }
        if let Some(Value::String(name)) = map.get("discriminator") {
            map.insert("discriminator".to_string(), json!({"propertyName": name}));
        }
        if map.get("type").and_then(Value::as_str) == Some("file") {
            map.insert("type".to_string(), json!("string"));
            map.insert("format".to_string(), json!("binary"));
        }
        if let Some(properties) = map.get_mut("properties").and_then(Value::as_object_mut) {
            for property in properties.values_mut() {
                *property = convert_schema(property.take());
            }
        }
        for keyword in ["items", "additionalProperties", "not"] {
            if let Some(child) = map.get_mut(keyword) {
                *child = convert_schema(child.take());
            }
        }
        for keyword in ["allOf", "oneOf", "anyOf"] {
            if let Some(children) = map.get_mut(keyword).and_then(Value::as_array_mut) {
                for child in children {
                    *child = convert_schema(child.take());
                }
            }
        }
    }
    schema
}

fn take_parameter_schema(parameter: &mut Map<String, Value>) -> Value {
    let mut schema = Map::new();
    for keyword in TYPE_KEYWORDS {
        if let Some(value) = parameter.remove(*keyword) {
            schema.insert((*keyword).to_string(), value);
        }
    }
    convert_parameter_schema(Value::Object(schema))
}

fn convert_parameter_schema(mut schema: Value) -> Value {
    // Swagger Items Objects can themselves carry collectionFormat. Their type
    // keywords stay directly on the item schema, without a nested `schema` key.
    let mut item = &mut schema;
    while let Some(map) = item.as_object_mut() {
        map.remove("collectionFormat");
        let Some(nested) = map.get_mut("items") else {
            break;
        };
        item = nested;
    }
    convert_schema(schema)
}

fn convert_parameter(parameter: Value) -> Result<Value> {
    let mut parameter = into_object(parameter, "Swagger parameter")?;
    if parameter.contains_key("$ref") {
        return Ok(Value::Object(parameter));
    }
    let is_array = parameter.get("type").and_then(Value::as_str) == Some("array");
    let format = parameter.remove("collectionFormat");
    let schema = take_parameter_schema(&mut parameter);
    parameter.insert("schema".to_string(), schema);
    if is_array {
        // OpenAPI 3.0 allows spaceDelimited and pipeDelimited only in query, so
        // ssv and pipes elsewhere fall back to the location's default style.
        let in_query = parameter.get("in").and_then(Value::as_str) == Some("query");
        let (style, explode) = match format.as_ref().and_then(Value::as_str) {
            Some("multi") if in_query => ("form", true),
            Some("ssv") if in_query => ("spaceDelimited", false),
            Some("pipes") if in_query => ("pipeDelimited", false),
            _ if in_query => ("form", false),
            _ => ("simple", false),
        };
        parameter.insert("style".to_string(), json!(style));
        parameter.insert("explode".to_string(), json!(explode));
    }
    Ok(Value::Object(parameter))
}

fn content_for_schema(schema: Value, media_types: &[String]) -> Value {
    Value::Object(
        media_types
            .iter()
            .map(|media| (media.clone(), json!({"schema": schema})))
            .collect(),
    )
}

fn body_parameter(parameter: Value, consumes: &[String]) -> Result<Value> {
    let mut body = into_object(parameter, "Swagger body parameter")?;
    body.remove("name");
    body.remove("in");
    let schema = convert_schema(body.remove("schema").unwrap_or_else(|| json!({})));
    body.insert("content".to_string(), content_for_schema(schema, consumes));
    Ok(Value::Object(body))
}

fn convert_responses(responses: Value, produces: &[String]) -> Result<Value> {
    let mut responses = into_object(responses, "Swagger responses")?;
    for (status, response) in &mut responses {
        // `x-` keys in an operation's responses are extensions, not status codes.
        if status.starts_with("x-") {
            continue;
        }
        *response = convert_response(response.take(), produces)?;
    }
    Ok(Value::Object(responses))
}

fn convert_response(mut response: Value, produces: &[String]) -> Result<Value> {
    let map = response
        .as_object_mut()
        .context("Swagger response must be an object")?;
    let examples = map.remove("examples");
    let schema = map.remove("schema").map(convert_schema);
    let mut content = match &schema {
        Some(schema) => into_object(content_for_schema(schema.clone(), produces), "content")?,
        None => Map::new(),
    };
    if let Some(examples) = examples {
        for (media, example) in into_object(examples, "Swagger response examples")? {
            let entry = content.entry(media).or_insert_with(|| match &schema {
                Some(schema) => json!({"schema": schema}),
                None => json!({}),
            });
            entry["example"] = example;
        }
    }
    if !content.is_empty() {
        map.insert("content".to_string(), Value::Object(content));
    }
    if let Some(headers) = map.get_mut("headers").and_then(Value::as_object_mut) {
        for header in headers.values_mut() {
            let map = header
                .as_object_mut()
                .context("Swagger header must be an object")?;
            map.remove("collectionFormat");
            let schema = take_parameter_schema(map);
            map.insert("schema".to_string(), schema);
        }
    }
    Ok(response)
}

fn convert_security_scheme(scheme: Value) -> Result<Value> {
    let mut scheme = into_object(scheme, "Swagger security scheme")?;
    match scheme.get("type").and_then(Value::as_str) {
        Some("basic") => {
            scheme.insert("type".to_string(), json!("http"));
            scheme.insert("scheme".to_string(), json!("basic"));
        }
        Some("oauth2") => {
            let flow = scheme
                .remove("flow")
                .context("Swagger oauth2 scheme must have a flow")?;
            let (name, authorization, token) = match flow.as_str() {
                Some("implicit") => ("implicit", true, false),
                Some("password") => ("password", false, true),
                Some("application") => ("clientCredentials", false, true),
                Some("accessCode") => ("authorizationCode", true, true),
                _ => bail!("Unsupported Swagger OAuth2 flow '{}'", flow),
            };
            let mut converted = Map::new();
            let authorization_url = scheme.remove("authorizationUrl");
            let token_url = scheme.remove("tokenUrl");
            if authorization {
                if let Some(url) = authorization_url {
                    converted.insert("authorizationUrl".to_string(), url);
                }
            }
            if token {
                if let Some(url) = token_url {
                    converted.insert("tokenUrl".to_string(), url);
                }
            }
            // Swagger allows `x-` extensions among scopes; OpenAPI 3 scopes are names only.
            let scopes = match scheme.remove("scopes") {
                Some(Value::Object(scopes)) => Value::Object(
                    scopes
                        .into_iter()
                        .filter(|(name, _)| !name.starts_with("x-"))
                        .collect(),
                ),
                Some(other) => other,
                None => json!({}),
            };
            converted.insert("scopes".to_string(), scopes);
            scheme.insert("flows".to_string(), json!({name: converted}));
        }
        _ => {}
    }
    Ok(Value::Object(scheme))
}
