//! Swagger 2.0 support: phyllotaxis converts a Swagger 2.0 document to OpenAPI 3.0
//! when it loads it, so every view works on either version.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture(name: &str) -> String {
    format!(
        "{}/tests/fixtures/swagger2/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    )
}

/// Runs the binary with HOME and XDG_CONFIG_HOME pointed at `home`, so no user
/// config or environment variable picks the document.
fn run_in(dir: &Path, home: &Path, args: &[&str]) -> (String, String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
        .args(args)
        .current_dir(dir)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env_remove("PHYLLOTAXIS_DOCUMENT")
        .output()
        .expect("failed to run phyllotaxis binary");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

fn run_doc(doc: &str, mode: &str, args: &[&str]) -> (String, String, i32) {
    let home = tempfile::tempdir().unwrap();
    let mut full = vec!["--doc", doc, mode];
    full.extend_from_slice(args);
    run_in(Path::new(env!("CARGO_MANIFEST_DIR")), home.path(), &full)
}

fn convert(yaml: &str) -> Value {
    let doc: Value = serde_yaml_ng::from_str(yaml).expect("test input must be valid YAML");
    phyllotaxis::spec::swagger2_to_openapi3(doc).expect("conversion should succeed")
}

/// The Swagger 2.0 header every converter test document shares.
const HEAD: &str = "swagger: \"2.0\"\ninfo:\n  title: T\n  version: \"1\"\n";

fn convert_with(rest: &str) -> Value {
    convert(&format!("{HEAD}{rest}"))
}

fn collect_refs(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            for (k, child) in map {
                if k == "$ref" {
                    if let Some(s) = child.as_str() {
                        out.push(s.to_string());
                    }
                }
                collect_refs(child, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|c| collect_refs(c, out)),
        _ => {}
    }
}

// ─── Parity: every view of the Swagger 2.0 fixture matches its OpenAPI 3.0 twin ───

const PARITY_VIEWS: &[&[&str]] = &[
    &[],
    &["--resources"],
    &["--resources", "pets"],
    &["--resources", "animals"],
    &["--resources", "deprecated-pets"],
    &["--schemas"],
    &["--schemas", "Pet"],
    &["--schemas", "Owner"],
    &["--schemas", "PetList"],
    &["--schemas", "PetStatus"],
    &["--schemas", "Animal"],
    &["--schemas", "Cat"],
    &["--schemas", "Labels"],
    &["--schemas", "Error"],
    &["--schemas", "Pet", "--used-by"],
    &["--schemas", "Error", "--used-by"],
    &["--schemas", "Pet", "--expand"],
    &["--schemas", "Pet", "--example"],
    &["--schemas", "Cat", "--example"],
    &["--auth"],
    &["--callbacks"],
    &["--endpoint", "GET", "/pets"],
    &["--endpoint", "POST", "/pets"],
    &["--endpoint", "GET", "/pets/{id}"],
    &["--endpoint", "PUT", "/pets/{id}"],
    &["--endpoint", "DELETE", "/pets/{id}"],
    &["--endpoint", "POST", "/pets/{id}/photo"],
    &["--endpoint", "POST", "/pets/{id}/status"],
    &["--endpoint", "GET", "/old-pets"],
    &["--endpoint", "GET", "/animals"],
    &["--endpoint", "GET", "/pets/search"],
    &["--endpoint", "GET", "/pets", "--context"],
    &["--endpoint", "POST", "/pets", "--example"],
    &["--endpoint", "PUT", "/pets/{id}", "--example"],
    &["--endpoint", "POST", "/pets/{id}/photo", "--example"],
    &["search", "pet"],
    &["search", "owner"],
    &["search", "status"],
    &["search", "oauth"],
];

fn assert_parity(mode: &str) {
    let swagger = fixture("petstore.yaml");
    let twin = fixture("petstore-oas3.yaml");
    for view in PARITY_VIEWS {
        let (got, got_err, got_code) = run_doc(&swagger, mode, view);
        let (want, _, want_code) = run_doc(&twin, mode, view);
        assert_eq!(want_code, 0, "twin fixture failed for {view:?}");
        assert_eq!(
            got_code, 0,
            "Swagger 2.0 fixture failed for {view:?}: {got_err}"
        );
        assert_eq!(got, want, "{mode} output differs for {view:?}");
    }
}

#[test]
fn test_parity_text_output() {
    assert_parity("--text");
}

#[test]
fn test_parity_json_output() {
    assert_parity("--json");
}

#[test]
fn test_overview_names_swagger_document() {
    let (out, err, code) = run_doc(&fixture("petstore.yaml"), "--json", &[]);
    assert_eq!(code, 0, "{err}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["title"], "Swagger Petstore");
    assert_eq!(v["endpoint_count"], 10);
    assert_eq!(v["schema_count"], 8);
}

// ─── Loading ───

#[test]
fn test_load_document_converts_swagger2() {
    let loaded = phyllotaxis::spec::load_document(
        Some(&fixture("petstore.yaml")),
        Path::new(env!("CARGO_MANIFEST_DIR")),
        false,
    )
    .expect("a Swagger 2.0 document should load");
    assert_eq!(loaded.api.info.title, "Swagger Petstore");
    assert!(
        loaded.api.openapi.starts_with("3.0"),
        "{}",
        loaded.api.openapi
    );
    let schemas = &loaded.api.components.as_ref().unwrap().schemas;
    assert!(schemas.contains_key("Pet") && schemas.contains_key("Cat"));
    assert_eq!(loaded.api.paths.paths.len(), 7);
}

#[test]
fn test_load_document_raw_value_is_document_as_written() {
    let loaded = phyllotaxis::spec::load_document(
        Some(&fixture("petstore.yaml")),
        Path::new(env!("CARGO_MANIFEST_DIR")),
        false,
    )
    .unwrap();
    assert_eq!(loaded.raw_value["swagger"], "2.0");
    assert!(loaded.raw_value["definitions"]["Pet"].is_object());
}

#[test]
fn test_for_pointer_addresses_swagger_document() {
    let (out, err, code) = run_doc(
        &fixture("petstore.yaml"),
        "--json",
        &["--for", "/definitions/Pet/properties/nickname"],
    );
    assert_eq!(code, 0, "{err}");
    let v: Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["x-nullable"], true);
}

#[test]
fn test_swagger2_json_document_loads() {
    let dir = tempfile::tempdir().unwrap();
    let yaml = std::fs::read_to_string(fixture("petstore.yaml")).unwrap();
    let doc: Value = serde_yaml_ng::from_str(&yaml).unwrap();
    let path = dir.path().join("petstore.json");
    std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

    let (got, err, code) = run_doc(
        path.to_str().unwrap(),
        "--json",
        &["--endpoint", "GET", "/pets"],
    );
    assert_eq!(code, 0, "{err}");
    let (want, _, _) = run_doc(
        &fixture("petstore-oas3.yaml"),
        "--json",
        &["--endpoint", "GET", "/pets"],
    );
    assert_eq!(got, want);
}

#[test]
fn test_swagger2_external_ref_is_bundled_then_converted() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pet.yaml"),
        "type: object\nproperties:\n  name:\n    type: string\n    description: External pet name\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("api.yaml"),
        format!(
            "{HEAD}paths:\n  /pets:\n    get:\n      tags: [Pets]\n      responses:\n        \"200\":\n          description: ok\n          schema:\n            $ref: '#/definitions/Pet'\ndefinitions:\n  Pet:\n    $ref: './pet.yaml'\n"
        ),
    )
    .unwrap();
    let doc = dir.path().join("api.yaml");
    let (out, err, code) = run_doc(doc.to_str().unwrap(), "--text", &["--schemas", "Pet"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("External pet name"), "{out}");
}

#[test]
fn test_auto_detects_swagger2_document_with_unusual_name() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    std::fs::copy(fixture("petstore.yaml"), dir.path().join("store-api.yaml")).unwrap();
    let (out, err, code) = run_in(dir.path(), home.path(), &["--json"]);
    assert_eq!(code, 0, "{err}");
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["title"], "Swagger Petstore");
}

#[test]
fn test_init_candidates_include_swagger2_documents() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(fixture("petstore.yaml"), dir.path().join("store-api.yaml")).unwrap();
    std::fs::write(dir.path().join("notes.yaml"), "title: not an API\n").unwrap();
    let found = phyllotaxis::commands::init::find_document_candidates(dir.path(), None);
    let names: Vec<String> = found
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert_eq!(names, vec!["store-api.yaml".to_string()]);
}

#[test]
fn test_unsupported_swagger_version_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.yaml");
    std::fs::write(
        &path,
        "swagger: \"1.2\"\ninfo:\n  title: Old\n  version: \"1\"\npaths: {}\n",
    )
    .unwrap();
    let (_out, err, code) = run_doc(path.to_str().unwrap(), "--text", &[]);
    assert_ne!(code, 0);
    assert!(err.contains("1.2"), "error should name the version: {err}");
    assert!(!err.contains("panicked"), "{err}");

    let doc: Value =
        serde_yaml_ng::from_str("swagger: \"1.2\"\ninfo: {title: Old, version: \"1\"}\n").unwrap();
    assert!(phyllotaxis::spec::swagger2_to_openapi3(doc).is_err());
}

#[test]
fn test_openapi3_documents_are_untouched() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = PathBuf::from(manifest).join("tests/fixtures/petstore.yaml");
    let loaded =
        phyllotaxis::spec::load_document(Some(path.to_str().unwrap()), Path::new(manifest), false)
            .unwrap();
    assert_eq!(loaded.api.openapi, "3.0.4");
    assert_eq!(loaded.raw_value["openapi"], "3.0.4");
}

// ─── Converter: document level ───

#[test]
fn test_convert_fixture_deserializes_as_openapi3() {
    let yaml = std::fs::read_to_string(fixture("petstore.yaml")).unwrap();
    let out = convert(&yaml);
    assert!(out["openapi"].as_str().unwrap().starts_with("3.0."));
    assert!(out.get("swagger").is_none());
    for gone in [
        "definitions",
        "parameters",
        "responses",
        "securityDefinitions",
        "host",
        "basePath",
        "schemes",
        "consumes",
        "produces",
    ] {
        assert!(out.get(gone).is_none(), "top-level {gone} should be gone");
    }
    let api: openapiv3::OpenAPI =
        serde_json::from_value(out).expect("converted document should parse as OpenAPI 3.0");
    assert_eq!(api.info.version, "2.1.0");
}

#[test]
fn test_convert_rewrites_every_ref() {
    let yaml = std::fs::read_to_string(fixture("petstore.yaml")).unwrap();
    let out = convert(&yaml);
    let mut refs = Vec::new();
    collect_refs(&out, &mut refs);
    assert!(!refs.is_empty());
    for r in &refs {
        assert!(r.starts_with("#/components/"), "unconverted $ref {r}");
        let pointer = r.trim_start_matches('#');
        assert!(out.pointer(pointer).is_some(), "dangling $ref {r}");
    }
    assert!(refs.contains(&"#/components/requestBodies/PetBody".to_string()));
    assert!(refs.contains(&"#/components/responses/NotFound".to_string()));
    assert!(refs.contains(&"#/components/parameters/PetId".to_string()));
}

#[test]
fn test_convert_keeps_info_tags_security_and_extensions() {
    let out = convert_with(
        "x-root: 1\nexternalDocs:\n  url: https://docs.example.com\ntags:\n  - name: A\n    description: tag a\nsecurity:\n  - k: []\nsecurityDefinitions:\n  k:\n    type: apiKey\n    name: key\n    in: query\npaths:\n  /a:\n    x-path: 2\n    get:\n      x-op: 3\n      operationId: getA\n      responses:\n        \"200\":\n          description: ok\n",
    );
    assert_eq!(out["info"], json!({"title": "T", "version": "1"}));
    assert_eq!(out["x-root"], 1);
    assert_eq!(out["externalDocs"]["url"], "https://docs.example.com");
    assert_eq!(out["tags"], json!([{"name": "A", "description": "tag a"}]));
    assert_eq!(out["security"], json!([{"k": []}]));
    assert_eq!(out["paths"]["/a"]["x-path"], 2);
    assert_eq!(out["paths"]["/a"]["get"]["x-op"], 3);
    assert_eq!(out["paths"]["/a"]["get"]["operationId"], "getA");
    assert_eq!(
        out["components"]["securitySchemes"]["k"],
        json!({"type": "apiKey", "name": "key", "in": "query"})
    );
}

#[test]
fn test_convert_paths_only_document() {
    let out = convert_with("paths: {}\n");
    assert_eq!(out["paths"], json!({}));
    let api: openapiv3::OpenAPI = serde_json::from_value(out).unwrap();
    assert!(api.paths.paths.is_empty());
}

// ─── Converter: servers ───

#[test]
fn test_servers_one_per_scheme() {
    let out = convert_with(
        "host: api.example.com:8443\nbasePath: /v1\nschemes: [http, https]\npaths: {}\n",
    );
    assert_eq!(
        out["servers"],
        json!([{"url": "http://api.example.com:8443/v1"}, {"url": "https://api.example.com:8443/v1"}])
    );
}

#[test]
fn test_servers_default_scheme_is_https() {
    let out = convert_with("host: api.example.com\npaths: {}\n");
    assert_eq!(out["servers"], json!([{"url": "https://api.example.com"}]));
}

#[test]
fn test_servers_base_path_without_host() {
    let out = convert_with("basePath: /api\npaths: {}\n");
    assert_eq!(out["servers"], json!([{"url": "/api"}]));
}

#[test]
fn test_servers_absent_without_host_or_base_path() {
    let out = convert_with("paths: {}\n");
    let empty = out.get("servers").is_none() || out["servers"] == json!([]);
    assert!(empty, "servers: {}", out["servers"]);
}

// ─── Converter: security schemes ───

#[test]
fn test_security_schemes_convert_every_kind() {
    let yaml = std::fs::read_to_string(fixture("petstore.yaml")).unwrap();
    let out = convert(&yaml);
    let twin: Value =
        serde_yaml_ng::from_str(&std::fs::read_to_string(fixture("petstore-oas3.yaml")).unwrap())
            .unwrap();
    assert_eq!(
        out["components"]["securitySchemes"],
        twin["components"]["securitySchemes"]
    );
}

// ─── Converter: parameters ───

#[test]
fn test_parameter_keywords_move_into_schema() {
    let out = convert_with(
        "paths:\n  /a:\n    get:\n      parameters:\n        - name: n\n          in: query\n          required: true\n          description: count\n          allowEmptyValue: true\n          type: integer\n          format: int64\n          default: 5\n          minimum: 1\n          maximum: 10\n          exclusiveMaximum: true\n          multipleOf: 1\n          enum: [1, 5, 10]\n        - name: s\n          in: query\n          type: string\n          minLength: 2\n          maxLength: 8\n          pattern: '^[a-z]+$'\n      responses:\n        \"200\":\n          description: ok\n",
    );
    let params = &out["paths"]["/a"]["get"]["parameters"];
    assert_eq!(
        params[0],
        json!({
            "name": "n", "in": "query", "required": true, "description": "count",
            "allowEmptyValue": true,
            "schema": {"type": "integer", "format": "int64", "default": 5, "minimum": 1,
                       "maximum": 10, "exclusiveMaximum": true, "multipleOf": 1, "enum": [1, 5, 10]}
        })
    );
    assert_eq!(
        params[1],
        json!({"name": "s", "in": "query",
               "schema": {"type": "string", "minLength": 2, "maxLength": 8, "pattern": "^[a-z]+$"}})
    );
}

fn array_param(location: &str, collection_format: Option<&str>) -> Value {
    let cf = collection_format
        .map(|c| format!("          collectionFormat: {c}\n"))
        .unwrap_or_default();
    let name = if location == "path" { "ids" } else { "list" };
    let path = if location == "path" { "/a/{ids}" } else { "/a" };
    let required = if location == "path" {
        "          required: true\n"
    } else {
        ""
    };
    let out = convert_with(&format!(
        "paths:\n  {path}:\n    get:\n      parameters:\n        - name: {name}\n          in: {location}\n{required}          type: array\n          minItems: 1\n          maxItems: 4\n          uniqueItems: true\n          items:\n            type: integer\n{cf}      responses:\n        \"200\":\n          description: ok\n"
    ));
    out["paths"][path]["get"]["parameters"][0].clone()
}

#[test]
fn test_collection_formats_map_to_style_and_explode() {
    let cases: &[(&str, Option<&str>, &str, bool)] = &[
        ("query", None, "form", false),
        ("query", Some("csv"), "form", false),
        ("query", Some("multi"), "form", true),
        ("query", Some("ssv"), "spaceDelimited", false),
        ("query", Some("pipes"), "pipeDelimited", false),
        ("path", Some("csv"), "simple", false),
        ("header", None, "simple", false),
    ];
    for (location, cf, style, explode) in cases {
        let p = array_param(location, *cf);
        assert_eq!(p["style"], *style, "{location} {cf:?}: {p}");
        assert_eq!(p["explode"], *explode, "{location} {cf:?}: {p}");
        assert_eq!(
            p["schema"],
            json!({"type": "array", "minItems": 1, "maxItems": 4, "uniqueItems": true,
                   "items": {"type": "integer"}}),
            "{location} {cf:?}"
        );
        assert!(p.get("collectionFormat").is_none());
        assert!(p.get("type").is_none() && p.get("items").is_none());
    }
}

#[test]
fn test_path_level_parameters_stay_on_path_item() {
    let out = convert_with(
        "paths:\n  /a/{id}:\n    parameters:\n      - name: id\n        in: path\n        required: true\n        type: string\n    get:\n      responses:\n        \"200\":\n          description: ok\n",
    );
    assert_eq!(
        out["paths"]["/a/{id}"]["parameters"],
        json!([{"name": "id", "in": "path", "required": true, "schema": {"type": "string"}}])
    );
}

#[test]
fn test_global_parameters_split_by_location() {
    let out = convert_with(
        "parameters:\n  Q:\n    name: q\n    in: query\n    type: string\n  B:\n    name: b\n    in: body\n    description: the body\n    schema:\n      type: object\npaths:\n  /a:\n    post:\n      parameters:\n        - $ref: '#/parameters/Q'\n        - $ref: '#/parameters/B'\n      responses:\n        \"200\":\n          description: ok\n",
    );
    assert_eq!(
        out["components"]["parameters"],
        json!({"Q": {"name": "q", "in": "query", "schema": {"type": "string"}}})
    );
    assert_eq!(
        out["components"]["requestBodies"]["B"],
        json!({"description": "the body",
               "content": {"application/json": {"schema": {"type": "object"}}}})
    );
    let op = &out["paths"]["/a"]["post"];
    assert_eq!(
        op["parameters"],
        json!([{"$ref": "#/components/parameters/Q"}])
    );
    assert_eq!(
        op["requestBody"],
        json!({"$ref": "#/components/requestBodies/B"})
    );
}

// ─── Converter: request bodies ───

#[test]
fn test_body_parameter_uses_operation_consumes() {
    let out = convert_with(
        "consumes: [application/json]\npaths:\n  /a:\n    post:\n      consumes: [application/json, application/xml]\n      parameters:\n        - name: b\n          in: body\n          required: true\n          description: payload\n          schema:\n            $ref: '#/definitions/D'\n      responses:\n        \"200\":\n          description: ok\ndefinitions:\n  D:\n    type: object\n",
    );
    let schema = json!({"$ref": "#/components/schemas/D"});
    assert_eq!(
        out["paths"]["/a"]["post"]["requestBody"],
        json!({"required": true, "description": "payload", "content": {
            "application/json": {"schema": schema},
            "application/xml": {"schema": schema}
        }})
    );
    assert!(
        out["paths"]["/a"]["post"].get("parameters").is_none()
            || out["paths"]["/a"]["post"]["parameters"] == json!([])
    );
}

#[test]
fn test_body_parameter_defaults_to_global_then_json() {
    let global = convert_with(
        "consumes: [text/plain]\npaths:\n  /a:\n    post:\n      parameters:\n        - name: b\n          in: body\n          schema:\n            type: string\n      responses:\n        \"200\":\n          description: ok\n",
    );
    assert_eq!(
        global["paths"]["/a"]["post"]["requestBody"]["content"],
        json!({"text/plain": {"schema": {"type": "string"}}})
    );
    let none = convert_with(
        "paths:\n  /a:\n    post:\n      parameters:\n        - name: b\n          in: body\n          schema:\n            type: string\n      responses:\n        \"200\":\n          description: ok\n",
    );
    assert_eq!(
        none["paths"]["/a"]["post"]["requestBody"]["content"],
        json!({"application/json": {"schema": {"type": "string"}}})
    );
}

#[test]
fn test_path_level_body_parameter_moves_into_operations() {
    let out = convert_with(
        "paths:\n  /a:\n    parameters:\n      - name: b\n        in: body\n        schema:\n          type: object\n      - name: trace\n        in: header\n        type: string\n    put:\n      responses:\n        \"200\":\n          description: ok\n    post:\n      parameters:\n        - name: own\n          in: body\n          schema:\n            type: string\n      responses:\n        \"200\":\n          description: ok\n",
    );
    let item = &out["paths"]["/a"];
    assert_eq!(
        item["parameters"],
        json!([{"name": "trace", "in": "header", "schema": {"type": "string"}}])
    );
    assert_eq!(
        item["put"]["requestBody"]["content"]["application/json"]["schema"],
        json!({"type": "object"})
    );
    assert_eq!(
        item["post"]["requestBody"]["content"]["application/json"]["schema"],
        json!({"type": "string"}),
        "an operation's own body parameter wins over the path's"
    );
}

#[test]
fn test_form_parameters_become_urlencoded_body() {
    let out = convert_with(
        "paths:\n  /a:\n    post:\n      parameters:\n        - name: user\n          in: formData\n          type: string\n          required: true\n          description: login\n        - name: tags\n          in: formData\n          type: array\n          items:\n            type: string\n        - name: q\n          in: query\n          type: string\n      responses:\n        \"200\":\n          description: ok\n",
    );
    let op = &out["paths"]["/a"]["post"];
    assert_eq!(
        op["requestBody"],
        json!({"required": true, "content": {"application/x-www-form-urlencoded": {"schema": {
            "type": "object",
            "required": ["user"],
            "properties": {
                "user": {"type": "string", "description": "login"},
                "tags": {"type": "array", "items": {"type": "string"}}
            }
        }}}})
    );
    assert_eq!(
        op["parameters"],
        json!([{"name": "q", "in": "query", "schema": {"type": "string"}}])
    );
}

#[test]
fn test_file_form_parameter_becomes_multipart_binary() {
    let out = convert_with(
        "paths:\n  /a:\n    post:\n      parameters:\n        - name: upload\n          in: formData\n          type: file\n      responses:\n        \"200\":\n          description: ok\n",
    );
    let body = &out["paths"]["/a"]["post"]["requestBody"];
    assert_eq!(
        body["content"],
        json!({"multipart/form-data": {"schema": {"type": "object", "properties": {
            "upload": {"type": "string", "format": "binary"}
        }}}})
    );
    assert!(body.get("required").is_none() || body["required"] == false);
}

#[test]
fn test_form_parameters_keep_declared_form_media_types() {
    let out = convert_with(
        "paths:\n  /a:\n    post:\n      consumes: [multipart/form-data, application/x-www-form-urlencoded, application/json]\n      parameters:\n        - name: f\n          in: formData\n          type: string\n      responses:\n        \"200\":\n          description: ok\n",
    );
    let content = out["paths"]["/a"]["post"]["requestBody"]["content"]
        .as_object()
        .unwrap();
    let mut keys: Vec<&String> = content.keys().collect();
    keys.sort();
    assert_eq!(
        keys,
        vec!["application/x-www-form-urlencoded", "multipart/form-data"]
    );
}

#[test]
fn test_global_form_parameter_is_inlined_into_the_form_body() {
    let out = convert_with(
        "parameters:\n  Token:\n    name: token\n    in: formData\n    type: string\n    required: true\npaths:\n  /a:\n    post:\n      parameters:\n        - $ref: '#/parameters/Token'\n        - name: extra\n          in: formData\n          type: integer\n      responses:\n        \"200\":\n          description: ok\n",
    );
    let schema = &out["paths"]["/a"]["post"]["requestBody"]["content"]
        ["application/x-www-form-urlencoded"]["schema"];
    assert_eq!(schema["properties"]["token"], json!({"type": "string"}));
    assert_eq!(schema["properties"]["extra"], json!({"type": "integer"}));
    assert_eq!(schema["required"], json!(["token"]));
    let params = &out["components"]["parameters"];
    assert!(
        params.get("Token").is_none(),
        "a form parameter has no OpenAPI 3.0 parameter form"
    );
}

// ─── Converter: responses ───

#[test]
fn test_response_schema_uses_operation_produces() {
    let out = convert_with(
        "produces: [application/json]\npaths:\n  /a:\n    get:\n      produces: [application/xml, text/csv]\n      responses:\n        \"200\":\n          description: ok\n          schema:\n            type: string\n        \"204\":\n          description: empty\n",
    );
    let responses = &out["paths"]["/a"]["get"]["responses"];
    assert_eq!(
        responses["200"],
        json!({"description": "ok", "content": {
            "application/xml": {"schema": {"type": "string"}},
            "text/csv": {"schema": {"type": "string"}}
        }})
    );
    assert_eq!(responses["204"], json!({"description": "empty"}));
}

#[test]
fn test_response_headers_and_examples() {
    let out = convert_with(
        "paths:\n  /a:\n    get:\n      responses:\n        \"200\":\n          description: ok\n          headers:\n            X-Rate-Limit:\n              type: integer\n              format: int32\n              description: calls per hour\n            X-Ids:\n              type: array\n              items:\n                type: string\n          schema:\n            type: object\n          examples:\n            application/json:\n              id: 7\n",
    );
    let r = &out["paths"]["/a"]["get"]["responses"]["200"];
    assert_eq!(
        r["headers"]["X-Rate-Limit"],
        json!({"description": "calls per hour", "schema": {"type": "integer", "format": "int32"}})
    );
    assert_eq!(
        r["headers"]["X-Ids"]["schema"],
        json!({"type": "array", "items": {"type": "string"}})
    );
    assert_eq!(
        r["content"]["application/json"],
        json!({"schema": {"type": "object"}, "example": {"id": 7}})
    );
}

#[test]
fn test_response_refs_and_global_responses() {
    let out = convert_with(
        "produces: [application/json]\nresponses:\n  Oops:\n    description: failure\n    schema:\n      $ref: '#/definitions/E'\npaths:\n  /a:\n    get:\n      responses:\n        default:\n          $ref: '#/responses/Oops'\ndefinitions:\n  E:\n    type: object\n",
    );
    assert_eq!(
        out["components"]["responses"]["Oops"],
        json!({"description": "failure", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/E"}}}})
    );
    assert_eq!(
        out["paths"]["/a"]["get"]["responses"]["default"],
        json!({"$ref": "#/components/responses/Oops"})
    );
}

// ─── Converter: schemas ───

#[test]
fn test_schema_keywords_convert_recursively() {
    let out = convert_with(
        "paths: {}\ndefinitions:\n  A:\n    type: object\n    discriminator: kind\n    x-keep: yes\n    properties:\n      kind:\n        type: string\n      maybe:\n        type: string\n        x-nullable: true\n      blob:\n        type: file\n      nested:\n        type: object\n        properties:\n          deep:\n            type: array\n            items:\n              type: string\n              x-nullable: true\n      map:\n        type: object\n        additionalProperties:\n          $ref: '#/definitions/B'\n  B:\n    allOf:\n      - $ref: '#/definitions/A'\n      - type: object\n        x-nullable: false\n",
    );
    let a = &out["components"]["schemas"]["A"];
    assert_eq!(a["discriminator"], json!({"propertyName": "kind"}));
    assert_eq!(a["x-keep"], "yes");
    assert_eq!(
        a["properties"]["maybe"],
        json!({"type": "string", "nullable": true})
    );
    assert_eq!(
        a["properties"]["blob"],
        json!({"type": "string", "format": "binary"})
    );
    assert_eq!(
        a["properties"]["nested"]["properties"]["deep"]["items"],
        json!({"type": "string", "nullable": true})
    );
    assert_eq!(
        a["properties"]["map"]["additionalProperties"],
        json!({"$ref": "#/components/schemas/B"})
    );
    let b = &out["components"]["schemas"]["B"];
    assert_eq!(b["allOf"][0], json!({"$ref": "#/components/schemas/A"}));
    assert_eq!(b["allOf"][1], json!({"type": "object", "nullable": false}));
}

#[test]
fn test_schema_property_named_like_a_keyword_is_left_alone() {
    let out = convert_with(
        "paths: {}\ndefinitions:\n  A:\n    type: object\n    properties:\n      discriminator:\n        type: string\n      type:\n        type: string\n        enum: [file]\n",
    );
    let props = &out["components"]["schemas"]["A"]["properties"];
    assert_eq!(props["discriminator"], json!({"type": "string"}));
    assert_eq!(props["type"], json!({"type": "string", "enum": ["file"]}));
}

#[test]
fn test_inline_schemas_in_operations_convert() {
    let out = convert_with(
        "paths:\n  /a:\n    post:\n      parameters:\n        - name: b\n          in: body\n          schema:\n            type: object\n            properties:\n              note:\n                type: string\n                x-nullable: true\n              ref:\n                $ref: '#/definitions/D'\n      responses:\n        \"200\":\n          description: ok\n          schema:\n            type: array\n            items:\n              $ref: '#/definitions/D'\ndefinitions:\n  D:\n    type: string\n",
    );
    let op = &out["paths"]["/a"]["post"];
    let body = &op["requestBody"]["content"]["application/json"]["schema"];
    assert_eq!(
        body["properties"]["note"],
        json!({"type": "string", "nullable": true})
    );
    assert_eq!(
        body["properties"]["ref"],
        json!({"$ref": "#/components/schemas/D"})
    );
    assert_eq!(
        op["responses"]["200"]["content"]["application/json"]["schema"]["items"],
        json!({"$ref": "#/components/schemas/D"})
    );
}
