use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::Result;
use phyllotaxis::spec::{load_document, swagger2_to_openapi3};
use serde_json::{json, Value};

fn document() -> Value {
    json!({"swagger": "2.0", "info": {"title": "Test", "version": "1"}, "paths": {}})
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/swagger2")
        .join(name)
}

fn assert_parses(value: &Value) -> Result<()> {
    let _: openapiv3::OpenAPI = serde_json::from_value(value.clone())?;
    Ok(())
}

// Object keys, sorted: serde_json maps do not keep document order.
fn keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("expected an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    keys
}

#[test]
fn servers_and_top_level_fields() -> Result<()> {
    for (fields, servers) in [
        (json!({}), json!([])),
        (
            json!({"host": "example.test"}),
            json!([{"url": "https://example.test"}]),
        ),
        (json!({"basePath": "/v2"}), json!([{"url": "/v2"}])),
        (
            json!({"host": "example.test", "basePath": "/v2", "schemes": ["http", "https"]}),
            json!([{"url": "http://example.test/v2"}, {"url": "https://example.test/v2"}]),
        ),
    ] {
        let mut input = document();
        input
            .as_object_mut()
            .expect("document")
            .extend(fields.as_object().expect("fields").clone());
        input["tags"] = json!([{"name": "Pets", "x-tag": true}]);
        input["externalDocs"] = json!({"url": "https://example.test/docs"});
        input["security"] = json!([]);
        input["x-root"] = json!({"type": "file", "discriminator": "unchanged"});
        input["custom"] = json!({"keep": true});
        let output = swagger2_to_openapi3(input.clone())?;
        assert_eq!(output["openapi"], "3.0.3");
        assert_eq!(
            output.get("servers").cloned().unwrap_or_else(|| json!([])),
            servers
        );
        for key in [
            "info",
            "tags",
            "externalDocs",
            "security",
            "x-root",
            "custom",
        ] {
            assert_eq!(output[key], input[key], "changed {key}");
        }
        for key in [
            "swagger",
            "host",
            "basePath",
            "schemes",
            "consumes",
            "produces",
            "definitions",
            "parameters",
            "responses",
            "securityDefinitions",
        ] {
            assert!(output.get(key).is_none(), "left Swagger key {key}");
        }
        assert_parses(&output)?;
    }
    Ok(())
}

#[test]
fn unsupported_versions_are_errors() {
    for version in [
        json!("1.2"),
        json!("3.0"),
        json!(2),
        json!(null),
        json!(true),
        json!({"bad": "version"}),
    ] {
        let mut input = document();
        input["swagger"] = version.clone();
        let error = swagger2_to_openapi3(input)
            .expect_err("unsupported version")
            .to_string();
        assert!(error.contains("Unsupported Swagger version"), "{error}");
        assert!(
            error.contains(
                &version
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| version.to_string())
            ),
            "{error}"
        );
    }
    assert!(swagger2_to_openapi3(json!({})).is_err());
    assert!(swagger2_to_openapi3(json!([])).is_err());
}

#[test]
fn unquoted_yaml_version_is_accepted() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("api.yaml");
    std::fs::write(
        &path,
        "swagger: 2.0\ninfo:\n  title: Unquoted\n  version: \"1\"\npaths: {}\n",
    )?;
    let loaded = load_document(path.to_str(), dir.path(), false)?;
    assert_eq!(loaded.api.info.title, "Unquoted");
    assert_eq!(loaded.api.openapi, "3.0.3");
    Ok(())
}

#[test]
fn schema_conversion_visits_schemas_without_changing_property_names_or_examples() -> Result<()> {
    let mut input = document();
    input["definitions"] = json!({
        "Pet": {
            "type": "object", "discriminator": "kind", "x-nullable": true,
            "x-schema": {"type": "file", "discriminator": "leave alone"},
            "example": {"type": "file", "discriminator": "leave alone", "x-nullable": true},
            "properties": {
                "discriminator": {"type": "string", "x-nullable": false},
                "type": {"type": "file"},
                "x-nullable": {"type": "boolean"},
                "nested": {"type": "array", "items": {"allOf": [
                    {"$ref": "#/definitions/Owner"},
                    {"type": "object", "additionalProperties": {"type": "array", "items": {"type": "file", "x-nullable": true}}}
                ]}}
            },
            "additionalProperties": true
        },
        "Owner": {"type": "object", "properties": {"name": {"type": "string"}}}
    });
    let output = swagger2_to_openapi3(input.clone())?;
    let schema = &output["components"]["schemas"]["Pet"];
    assert_eq!(schema["discriminator"], json!({"propertyName": "kind"}));
    assert_eq!(schema["nullable"], true);
    assert!(schema.get("x-nullable").is_none());
    assert_eq!(
        schema["properties"]["discriminator"],
        json!({"type": "string", "nullable": false})
    );
    assert_eq!(
        schema["properties"]["type"],
        json!({"type": "string", "format": "binary"})
    );
    assert_eq!(
        schema["properties"]["x-nullable"],
        json!({"type": "boolean"})
    );
    let nested = &schema["properties"]["nested"]["items"]["allOf"];
    assert_eq!(nested[0], json!({"$ref": "#/components/schemas/Owner"}));
    assert_eq!(
        nested[1]["additionalProperties"]["items"],
        json!({"type": "string", "format": "binary", "nullable": true})
    );
    assert_eq!(schema["additionalProperties"], true);
    for key in ["example", "x-schema"] {
        assert_eq!(schema[key], input["definitions"]["Pet"][key]);
    }
    assert_parses(&output)
}

#[test]
fn parameters_move_all_type_keywords_and_preserve_metadata() -> Result<()> {
    let mut input = document();
    let keywords = json!({
        "type": "array", "format": "custom", "items": {"type": "string", "enum": ["a", "b"], "x-nullable": true},
        "default": ["a"], "enum": [["a"], ["b"]], "maximum": 10, "exclusiveMaximum": true,
        "minimum": 1, "exclusiveMinimum": false, "maxLength": 9, "minLength": 1,
        "pattern": "a", "maxItems": 5, "minItems": 1, "uniqueItems": true, "multipleOf": 2
    });
    let mut parameter = keywords.clone();
    parameter.as_object_mut().expect("parameter").extend(
        json!({
            "name": "tags", "in": "query", "description": "Filter", "required": false,
            "allowEmptyValue": true, "collectionFormat": "multi", "x-parameter": "keep", "custom": 7
        })
        .as_object()
        .expect("metadata")
        .clone(),
    );
    input["parameters"] = json!({"Tags": parameter});
    let output = swagger2_to_openapi3(input)?;
    let converted = &output["components"]["parameters"]["Tags"];
    let mut expected = keywords;
    expected["items"]
        .as_object_mut()
        .expect("items")
        .remove("x-nullable");
    expected["items"]["nullable"] = json!(true);
    assert_eq!(converted["schema"], expected);
    assert_eq!(converted["name"], "tags");
    assert_eq!(converted["description"], "Filter");
    assert_eq!(converted["required"], false);
    assert_eq!(converted["allowEmptyValue"], true);
    assert_eq!(converted["x-parameter"], "keep");
    assert_eq!(converted["custom"], 7);
    assert_eq!(converted["style"], "form");
    assert_eq!(converted["explode"], true);
    assert!(converted.get("collectionFormat").is_none());
    for key in keys(&expected) {
        assert!(converted.get(key).is_none(), "unmoved type keyword {key}");
    }
    assert_parses(&output)
}

#[test]
fn array_styles_and_scalar_parameters() -> Result<()> {
    for location in ["query", "path", "header"] {
        for (format, style, explode) in [
            (
                None,
                if location == "query" {
                    "form"
                } else {
                    "simple"
                },
                false,
            ),
            (
                Some("csv"),
                if location == "query" {
                    "form"
                } else {
                    "simple"
                },
                false,
            ),
            (
                Some("tsv"),
                if location == "query" {
                    "form"
                } else {
                    "simple"
                },
                false,
            ),
            (
                Some("multi"),
                if location == "query" {
                    "form"
                } else {
                    "simple"
                },
                location == "query",
            ),
            // spaceDelimited and pipeDelimited are query-only in OpenAPI 3.0.
            (
                Some("ssv"),
                if location == "query" {
                    "spaceDelimited"
                } else {
                    "simple"
                },
                false,
            ),
            (
                Some("pipes"),
                if location == "query" {
                    "pipeDelimited"
                } else {
                    "simple"
                },
                false,
            ),
        ] {
            let mut input = document();
            let mut array = json!({"name": "ids", "in": location, "type": "array", "items": {"type": "integer"}});
            if let Some(format) = format {
                array["collectionFormat"] = json!(format);
            }
            input["paths"] = json!({"/pets": {"get": {"parameters": [array, {"name": "scalar", "in": "query", "type": "string", "collectionFormat": "csv"}], "responses": {"200": {"description": "ok"}}}}});
            let output = swagger2_to_openapi3(input)?;
            let parameters = &output["paths"]["/pets"]["get"]["parameters"];
            assert_eq!(parameters[0]["style"], style, "{location} {format:?}");
            assert_eq!(parameters[0]["explode"], explode);
            assert_eq!(
                parameters[0]["schema"],
                json!({"type": "array", "items": {"type": "integer"}})
            );
            assert!(parameters[0].get("required").is_none());
            assert!(parameters[1].get("style").is_none());
            assert!(parameters[1].get("explode").is_none());
            assert!(parameters[1].get("collectionFormat").is_none());
            assert_parses(&output)?;
        }
    }
    Ok(())
}

#[test]
fn nested_parameter_items_drop_collection_formats() -> Result<()> {
    let mut input = document();
    let parameter_type = json!({"type": "array", "items": {
        "type": "array", "collectionFormat": "pipes", "items": {
            "type": "file", "collectionFormat": "csv", "x-nullable": true
        }
    }});
    let mut query = parameter_type.clone();
    query["name"] = json!("query");
    query["in"] = json!("query");
    let mut form = parameter_type.clone();
    form["name"] = json!("form");
    form["in"] = json!("formData");
    input["paths"] = json!({"/items": {"post": {
        "parameters": [query, form], "responses": {"200": {
            "description": "ok", "headers": {"X-Items": parameter_type}
        }}
    }}});
    let output = swagger2_to_openapi3(input)?;
    let op = &output["paths"]["/items"]["post"];
    let expected = json!({"type": "array", "items": {"type": "array", "items": {
        "type": "string", "format": "binary", "nullable": true
    }}});
    assert_eq!(op["parameters"][0]["schema"], expected);
    assert_eq!(
        op["responses"]["200"]["headers"]["X-Items"]["schema"],
        expected
    );
    assert_eq!(
        op["requestBody"]["content"]["application/x-www-form-urlencoded"]["schema"]["properties"]
            ["form"],
        expected
    );
    assert_parses(&output)
}

#[test]
fn global_parameters_are_split_and_refs_target_existing_components() -> Result<()> {
    let mut input = document();
    input["consumes"] = json!(["application/xml", "application/json"]);
    input["definitions"] =
        json!({"Pet/Input": {"type": "object", "properties": {"name": {"type": "string"}}}});
    input["parameters"] = json!({
        "Limit": {"name": "limit", "in": "query", "type": "integer"},
        "Body": {"name": "pet", "in": "body", "description": "Create", "required": true, "schema": {"$ref": "#/definitions/Pet~1Input"}},
        "File": {"name": "file", "in": "formData", "type": "file", "required": true}
    });
    input["responses"] = json!({"Error": {"description": "Error", "schema": {"type": "string"}}});
    input["paths"] = json!({
        "/pets": {"parameters": [{"$ref": "#/parameters/Limit"}, {"$ref": "#/parameters/Body"}],
            "post": {"consumes": ["text/plain"], "responses": {"400": {"$ref": "#/responses/Error"}}},
            "put": {"parameters": [{"$ref": "#/parameters/Body"}], "responses": {"200": {"description": "ok"}}}},
        "/files": {"post": {"parameters": [{"$ref": "#/parameters/File"}], "responses": {"204": {"description": "ok"}}}}
    });
    let output = swagger2_to_openapi3(input)?;
    assert_eq!(keys(&output["components"]["parameters"]), ["Limit"]);
    assert_eq!(keys(&output["components"]["requestBodies"]), ["Body"]);
    assert_eq!(
        output["paths"]["/pets"]["parameters"],
        json!([{"$ref": "#/components/parameters/Limit"}])
    );
    // put uses the global consumes, so it keeps the shared body. post declares its
    // own consumes, so the body is inlined with post's media type.
    let put = &output["paths"]["/pets"]["put"];
    assert_eq!(
        put["requestBody"],
        json!({"$ref": "#/components/requestBodies/Body"})
    );
    let post = &output["paths"]["/pets"]["post"];
    assert_eq!(
        post["requestBody"],
        json!({"description": "Create", "required": true, "content": {
            "text/plain": {"schema": {"$ref": "#/components/schemas/Pet~1Input"}}
        }})
    );
    for method in ["post", "put"] {
        assert!(output["paths"]["/pets"][method].get("parameters").is_none());
    }
    let body = &output["components"]["requestBodies"]["Body"];
    assert_eq!(body["description"], "Create");
    assert_eq!(body["required"], true);
    assert_eq!(
        keys(&body["content"]),
        ["application/json", "application/xml"]
    );
    assert_eq!(
        body["content"]["application/xml"]["schema"],
        json!({"$ref": "#/components/schemas/Pet~1Input"})
    );
    assert_eq!(
        output["paths"]["/pets"]["post"]["responses"]["400"],
        json!({"$ref": "#/components/responses/Error"})
    );
    assert_eq!(
        output["paths"]["/files"]["post"]["requestBody"]["content"]["multipart/form-data"]
            ["schema"]["properties"]["file"],
        json!({"type": "string", "format": "binary"})
    );
    assert_parses(&output)
}

#[test]
fn path_body_inheritance_and_operation_overrides() -> Result<()> {
    let mut input = document();
    input["consumes"] = json!(["application/xml", "application/json"]);
    input["paths"] = json!({"/pets/{id}": {
        "x-path": {"keep": true},
        "parameters": [
            {"name": "id", "in": "path", "required": true, "type": "string"},
            {"name": "inherited", "in": "body", "description": "Path body", "required": false, "schema": {"type": "file"}}
        ],
        "post": {"x-operation": [1, 2], "consumes": ["text/plain", "application/json"], "parameters": [{"name": "q", "in": "query", "type": "string"}], "responses": {}},
        "put": {"parameters": [{"name": "own", "in": "body", "required": true, "schema": {"type": "integer"}}], "responses": {}},
        "patch": {"parameters": [{"name": "ownForm", "in": "formData", "type": "string"}], "responses": {}},
        "delete": {"responses": {}}
    }});
    let output = swagger2_to_openapi3(input)?;
    let path = &output["paths"]["/pets/{id}"];
    assert_eq!(path["x-path"], json!({"keep": true}));
    assert_eq!(
        path["parameters"],
        json!([{"name": "id", "in": "path", "required": true, "schema": {"type": "string"}}])
    );
    assert_eq!(path["post"]["x-operation"], json!([1, 2]));
    assert_eq!(
        keys(&path["post"]["requestBody"]["content"]),
        ["application/json", "text/plain"]
    );
    assert_eq!(path["post"]["requestBody"]["description"], "Path body");
    assert_eq!(path["post"]["requestBody"]["required"], false);
    assert_eq!(
        path["post"]["requestBody"]["content"]["text/plain"]["schema"],
        json!({"type": "string", "format": "binary"})
    );
    assert_eq!(path["post"]["parameters"][0]["name"], "q");
    assert_eq!(
        path["put"]["requestBody"]["content"]["application/xml"]["schema"],
        json!({"type": "integer"})
    );
    assert_eq!(
        path["patch"]["requestBody"]["content"]["application/x-www-form-urlencoded"]["schema"]
            ["properties"],
        json!({"ownForm": {"type": "string"}})
    );
    assert_eq!(
        keys(&path["delete"]["requestBody"]["content"]),
        ["application/json", "application/xml"]
    );
    assert!(path["post"].get("consumes").is_none());
    assert_parses(&output)
}

#[test]
fn form_data_builds_properties_media_types_and_required_fields() -> Result<()> {
    let mut input = document();
    input["consumes"] = json!([
        "application/json",
        "multipart/form-data",
        "application/x-www-form-urlencoded"
    ]);
    input["parameters"] = json!({"Note": {"name": "aNote", "in": "formData", "type": "string", "description": "Note"}});
    input["paths"] = json!({"/upload": {"post": {"parameters": [
        {"name": "zFile", "in": "formData", "type": "file", "required": true, "description": "Upload", "x-field": 7},
        {"$ref": "#/parameters/Note"},
        {"name": "bTags", "in": "formData", "type": "array", "items": {"type": "string", "x-nullable": true}, "required": true, "collectionFormat": "multi"}
    ], "responses": {}}}});
    let output = swagger2_to_openapi3(input)?;
    let body = &output["paths"]["/upload"]["post"]["requestBody"];
    assert_eq!(body["required"], true);
    assert_eq!(
        keys(&body["content"]),
        ["application/x-www-form-urlencoded", "multipart/form-data"]
    );
    let schema = &body["content"]["multipart/form-data"]["schema"];
    assert_eq!(schema["type"], "object");
    assert_eq!(keys(&schema["properties"]), ["aNote", "bTags", "zFile"]);
    assert_eq!(schema["required"], json!(["zFile", "bTags"]));
    assert_eq!(
        schema["properties"]["zFile"],
        json!({"type": "string", "format": "binary", "description": "Upload", "x-field": 7})
    );
    assert_eq!(
        schema["properties"]["aNote"],
        json!({"type": "string", "description": "Note"})
    );
    assert_eq!(
        schema["properties"]["bTags"],
        json!({"type": "array", "items": {"type": "string", "nullable": true}})
    );
    assert!(output["paths"]["/upload"]["post"]
        .get("parameters")
        .is_none());
    assert_parses(&output)
}

#[test]
fn inherited_form_data_and_media_fallbacks() -> Result<()> {
    for (kind, media) in [
        ("file", "multipart/form-data"),
        ("string", "application/x-www-form-urlencoded"),
    ] {
        let mut input = document();
        input["paths"] = json!({"/form": {
            "parameters": [{"name": "inherited", "in": "formData", "type": kind}],
            "post": {"parameters": [{"name": "q", "in": "query", "type": "string"}], "responses": {}},
            "put": {"parameters": [{"name": "own", "in": "formData", "type": "integer"}], "responses": {}},
            "patch": {"parameters": [{"name": "body", "in": "body", "schema": {"type": "string"}}], "responses": {}}
        }});
        let output = swagger2_to_openapi3(input)?;
        let path = &output["paths"]["/form"];
        assert!(path.get("parameters").is_none());
        let body = &path["post"]["requestBody"];
        assert_eq!(keys(&body["content"]), [media]);
        assert!(body.get("required").is_none());
        assert!(body["content"][media]["schema"].get("required").is_none());
        assert_eq!(
            keys(&body["content"][media]["schema"]["properties"]),
            ["inherited"]
        );
        assert_eq!(
            keys(
                &path["put"]["requestBody"]["content"]["application/x-www-form-urlencoded"]
                    ["schema"]["properties"]
            ),
            ["own"]
        );
        assert_eq!(
            keys(&path["patch"]["requestBody"]["content"]),
            ["application/json"]
        );
        assert_parses(&output)?;
    }
    Ok(())
}

#[test]
fn responses_use_effective_media_types_examples_and_header_schemas() -> Result<()> {
    let mut input = document();
    input["produces"] = json!(["application/xml", "application/json"]);
    input["responses"] = json!({"Shared": {"description": "Shared", "schema": {"type": "file"}}});
    input["paths"] = json!({"/pets": {
        "get": {"produces": ["text/plain", "application/json"], "responses": {
            "200": {"description": "ok", "x-response": true, "custom": "keep", "schema": {"type": "string", "x-nullable": true},
                "examples": {"text/plain": "Fern", "application/json": {"name": "Fern"}},
                "headers": {"X-Ids": {"description": "Identifiers", "type": "array", "items": {"type": "integer"}, "collectionFormat": "csv", "minItems": 1, "x-header": true}}},
            "204": {"description": "empty"}, "default": {"$ref": "#/responses/Shared"}
        }},
        "post": {"responses": {"201": {"description": "created", "schema": {"type": "integer"}}}}
    }});
    let output = swagger2_to_openapi3(input)?;
    let response = &output["paths"]["/pets"]["get"]["responses"]["200"];
    assert_eq!(
        keys(&response["content"]),
        ["application/json", "text/plain"]
    );
    assert_eq!(
        response["content"]["text/plain"],
        json!({"schema": {"type": "string", "nullable": true}, "example": "Fern"})
    );
    assert_eq!(
        response["content"]["application/json"]["example"],
        json!({"name": "Fern"})
    );
    assert_eq!(
        response["headers"]["X-Ids"],
        json!({"description": "Identifiers", "schema": {"type": "array", "items": {"type": "integer"}, "minItems": 1}, "x-header": true})
    );
    assert_eq!(response["x-response"], true);
    assert_eq!(response["custom"], "keep");
    assert!(response.get("schema").is_none());
    assert!(response.get("examples").is_none());
    assert!(output["paths"]["/pets"]["get"]["responses"]["204"]
        .get("content")
        .is_none());
    assert!(output["paths"]["/pets"]["get"].get("produces").is_none());
    assert_eq!(
        keys(&output["components"]["responses"]["Shared"]["content"]),
        ["application/json", "application/xml"]
    );
    assert_eq!(
        keys(&output["paths"]["/pets"]["post"]["responses"]["201"]["content"]),
        ["application/json", "application/xml"]
    );
    assert_parses(&output)
}

#[test]
fn response_and_body_default_to_json() -> Result<()> {
    let mut input = document();
    input["paths"] = json!({"/pets": {"post": {
        "parameters": [{"name": "pet", "in": "body", "schema": {"type": "string"}}],
        "responses": {"200": {"description": "ok", "schema": {"type": "string"}}}
    }}});
    let output = swagger2_to_openapi3(input)?;
    let operation = &output["paths"]["/pets"]["post"];
    assert_eq!(
        keys(&operation["requestBody"]["content"]),
        ["application/json"]
    );
    assert!(operation["requestBody"].get("required").is_none());
    assert_eq!(
        keys(&operation["responses"]["200"]["content"]),
        ["application/json"]
    );
    assert_parses(&output)
}

#[test]
fn security_schemes_preserve_descriptions_and_convert_all_oauth_flows() -> Result<()> {
    let mut input = document();
    input["securityDefinitions"] = json!({
        "Basic": {"type": "basic", "description": "Basic auth", "x-security": 7},
        "Key": {"type": "apiKey", "name": "key", "in": "query", "description": "API key"}
    });
    for (flow, name, urls) in [
        (
            "implicit",
            "implicit",
            json!({"authorizationUrl": "https://example.test/auth"}),
        ),
        (
            "password",
            "password",
            json!({"tokenUrl": "https://example.test/token"}),
        ),
        (
            "application",
            "clientCredentials",
            json!({"tokenUrl": "https://example.test/token"}),
        ),
        (
            "accessCode",
            "authorizationCode",
            json!({"authorizationUrl": "https://example.test/auth", "tokenUrl": "https://example.test/token"}),
        ),
    ] {
        let mut scheme = json!({"type": "oauth2", "flow": flow, "description": name, "scopes": {"read": "Read pets"}, "x-oauth": true});
        scheme
            .as_object_mut()
            .expect("scheme")
            .extend(urls.as_object().expect("URLs").clone());
        input["securityDefinitions"][flow] = scheme;
    }
    let output = swagger2_to_openapi3(input.clone())?;
    let security = &output["components"]["securitySchemes"];
    assert_eq!(
        security["Basic"],
        json!({"type": "http", "scheme": "basic", "description": "Basic auth", "x-security": 7})
    );
    assert_eq!(security["Key"], input["securityDefinitions"]["Key"]);
    for (flow, name) in [
        ("implicit", "implicit"),
        ("password", "password"),
        ("application", "clientCredentials"),
        ("accessCode", "authorizationCode"),
    ] {
        let source = &input["securityDefinitions"][flow];
        let scheme = &security[flow];
        assert_eq!(scheme["type"], "oauth2");
        assert_eq!(scheme["description"], name);
        assert_eq!(scheme["x-oauth"], true);
        assert_eq!(keys(&scheme["flows"]), [name]);
        assert_eq!(scheme["flows"][name]["scopes"], source["scopes"]);
        for url in ["authorizationUrl", "tokenUrl"] {
            assert_eq!(scheme["flows"][name].get(url), source.get(url));
            assert!(scheme.get(url).is_none());
        }
        assert!(scheme.get("flow").is_none());
        assert!(scheme.get("scopes").is_none());
    }
    assert_parses(&output)
}

#[test]
fn circular_parameter_refs_return_errors() {
    let mut input = document();
    input["parameters"] = json!({
        "A": {"$ref": "#/parameters/B"},
        "B": {"$ref": "#/parameters/A"}
    });
    input["paths"] =
        json!({"/pets": {"get": {"parameters": [{"$ref": "#/parameters/A"}], "responses": {}}}});
    let error = swagger2_to_openapi3(input).expect_err("cycle").to_string();
    assert!(
        error.contains("Circular Swagger parameter reference"),
        "{error}"
    );
}

// A dangling reference loads in an OpenAPI 3 document, so it loads in Swagger 2.0 too.
#[test]
fn dangling_refs_are_rewritten_not_rejected() -> Result<()> {
    let mut input = document();
    input["definitions"] = json!({"Bad": {"$ref": "#/definitions/Absent"}});
    input["paths"] = json!({"/pets": {"get": {
        "parameters": [{"$ref": "#/parameters/Missing"}],
        "responses": {"404": {"$ref": "#/responses/Gone"}}
    }}});
    let output = swagger2_to_openapi3(input)?;
    assert_eq!(
        output["components"]["schemas"]["Bad"]["$ref"],
        "#/components/schemas/Absent"
    );
    let op = &output["paths"]["/pets"]["get"];
    assert_eq!(
        op["parameters"],
        json!([{"$ref": "#/components/parameters/Missing"}])
    );
    assert_eq!(
        op["responses"]["404"]["$ref"],
        "#/components/responses/Gone"
    );
    assert_parses(&output)
}

#[test]
fn load_swagger_retains_raw_structure_and_matches_typed_equivalent() -> Result<()> {
    let cwd = tempfile::tempdir()?;
    let swagger = load_document(fixture("petstore-small.yaml").to_str(), cwd.path(), false)?;
    let equivalent = load_document(
        fixture("petstore-small-oas3.yaml").to_str(),
        cwd.path(),
        false,
    )?;
    assert_eq!(
        serde_json::to_value(&swagger.api)?,
        serde_json::to_value(&equivalent.api)?
    );
    assert_eq!(swagger.raw_value["swagger"], "2.0");
    assert!(swagger.raw_value.get("openapi").is_none());
    assert!(swagger.raw_value.get("components").is_none());
    assert_eq!(
        swagger.raw_value["definitions"]["Pet"]["properties"]["owner"]["$ref"],
        "#/definitions/Owner"
    );
    assert_eq!(equivalent.raw_value["openapi"], "3.0.3");
    Ok(())
}

#[test]
fn external_refs_are_bundled_before_conversion() -> Result<()> {
    let cwd = tempfile::tempdir()?;
    std::fs::write(cwd.path().join("pet.yaml"), "type: object\nx-nullable: true\ndiscriminator: kind\nproperties:\n  photo:\n    type: file\n")?;
    std::fs::write(cwd.path().join("operation.yaml"), "post:\n  parameters:\n    - name: pet\n      in: body\n      schema:\n        $ref: '#/definitions/Pet'\n  responses:\n    '200':\n      description: ok\n      schema:\n        $ref: '#/definitions/Pet'\n")?;
    let mut input = document();
    input["definitions"] = json!({"Pet": {"$ref": "pet.yaml"}});
    input["paths"] = json!({"/pets": {"$ref": "operation.yaml"}});
    let path = cwd.path().join("contract.json");
    std::fs::write(&path, serde_json::to_vec(&input)?)?;
    let loaded = load_document(path.to_str(), cwd.path(), false)?;
    assert_eq!(loaded.raw_value["definitions"]["Pet"]["x-nullable"], true);
    assert_eq!(
        loaded.raw_value["definitions"]["Pet"]["properties"]["photo"]["type"],
        "file"
    );
    let typed = serde_json::to_value(&loaded.api)?;
    let pet = &typed["components"]["schemas"]["Pet"];
    assert_eq!(pet["nullable"], true);
    assert_eq!(pet["discriminator"], json!({"propertyName": "kind"}));
    assert_eq!(pet["properties"]["photo"]["type"], "string");
    assert_eq!(pet["properties"]["photo"]["format"], "binary");
    assert_eq!(
        typed["paths"]["/pets"]["post"]["requestBody"]["content"]["application/json"]["schema"]
            ["$ref"],
        "#/components/schemas/Pet"
    );
    Ok(())
}

fn run(cwd: &Path, doc: Option<&Path>, mode: &str, args: &[&str]) -> Result<Output> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_phyllotaxis"));
    command
        .current_dir(cwd)
        .env("HOME", cwd)
        .env("XDG_CONFIG_HOME", cwd)
        .env_remove("PHYLLOTAXIS_DOCUMENT")
        .arg(mode);
    if let Some(doc) = doc {
        command.arg("--doc").arg(doc);
    }
    Ok(command.args(args).output()?)
}

#[test]
fn every_view_matches_openapi_equivalent_in_text_and_json() -> Result<()> {
    let cwd = tempfile::tempdir()?;
    let cases: &[&[&str]] = &[
        &[],
        &["--resources"],
        &["--resources", "pets"],
        &["--endpoint", "GET", "/pets"],
        &["--endpoint", "GET", "/pets/{id}"],
        &["--endpoint", "POST", "/pets"],
        &["--endpoint", "POST", "/uploads"],
        &["--schemas"],
        &["--schemas", "Pet"],
        &["--auth"],
        &["search", "pet"],
        &["--schemas", "Pet", "--example"],
        &["--endpoint", "POST", "/pets", "--example"],
        &["--schemas", "Pet", "--used-by"],
        &["--schemas", "Pet", "--expand"],
        &["--endpoint", "POST", "/pets", "--expand"],
        &["--endpoint", "POST", "/pets", "--context"],
        &["--endpoint", "POST", "/pets", "--context", "--example"],
    ];
    for mode in ["--text", "--json"] {
        for args in cases {
            let swagger = run(
                cwd.path(),
                Some(&fixture("petstore-small.yaml")),
                mode,
                args,
            )?;
            let equivalent = run(
                cwd.path(),
                Some(&fixture("petstore-small-oas3.yaml")),
                mode,
                args,
            )?;
            assert!(
                swagger.status.success(),
                "{mode} {args:?}: {}",
                String::from_utf8_lossy(&swagger.stderr)
            );
            assert!(
                equivalent.status.success(),
                "{mode} {args:?}: {}",
                String::from_utf8_lossy(&equivalent.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&swagger.stdout),
                String::from_utf8_lossy(&equivalent.stdout),
                "{mode} {args:?}"
            );
            assert_eq!(swagger.stderr, equivalent.stderr, "{mode} {args:?}");
        }
    }
    Ok(())
}

#[test]
fn for_navigates_swagger_definitions_in_text_and_json() -> Result<()> {
    let cwd = tempfile::tempdir()?;
    for mode in ["--text", "--json"] {
        let output = run(
            cwd.path(),
            Some(&fixture("petstore-small.yaml")),
            mode,
            &["--for", "/definitions/Pet"],
        )?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let raw: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(raw["properties"]["owner"]["$ref"], "#/definitions/Owner");
        assert_eq!(raw["properties"]["tags"]["x-nullable"], true);
    }
    Ok(())
}

#[test]
fn swagger_is_detected_under_custom_yaml_yml_and_json_names() -> Result<()> {
    for extension in ["yaml", "yml", "json"] {
        let cwd = tempfile::tempdir()?;
        let path = cwd.path().join(format!("custom-contract.{extension}"));
        let input = document();
        let content = if extension == "json" {
            serde_json::to_string(&input)?
        } else {
            serde_yaml_ng::to_string(&input)?
        };
        std::fs::write(&path, content)?;
        let candidates = phyllotaxis::commands::init::find_document_candidates(cwd.path(), None);
        assert_eq!(candidates, [path]);
        for mode in ["--text", "--json"] {
            let output = run(cwd.path(), None, mode, &[])?;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("Test"));
        }
    }
    Ok(())
}

#[test]
fn detection_only_reads_the_first_200_bytes() -> Result<()> {
    let cwd = tempfile::tempdir()?;
    std::fs::write(
        cwd.path().join("late.yaml"),
        format!("# {}\nswagger: '2.0'\n", "padding".repeat(40)),
    )?;
    std::fs::write(
        cwd.path().join("ordinary.yaml"),
        "title: Ordinary document\n",
    )?;
    assert!(phyllotaxis::commands::init::find_document_candidates(cwd.path(), None).is_empty());
    let output = run(cwd.path(), None, "--json", &[])?;
    assert!(String::from_utf8_lossy(&output.stderr).contains("No OpenAPI document found"));
    Ok(())
}

#[test]
fn unsupported_swagger_version_loads_as_an_anyhow_error_and_cli_error() -> Result<()> {
    let cwd = tempfile::tempdir()?;
    let path = cwd.path().join("unsupported.yaml");
    std::fs::write(
        &path,
        "swagger: '1.2'\ninfo:\n  title: Unsupported\n  version: '1'\npaths: {}\n",
    )?;
    let error = load_document(path.to_str(), cwd.path(), false).expect_err("unsupported version");
    let chain = format!("{error:#}");
    assert!(chain.contains("Failed to convert Swagger document"));
    assert!(chain.contains("unsupported.yaml"));
    assert!(chain.contains("Unsupported Swagger version '1.2'"));
    for mode in ["--text", "--json"] {
        let output = run(cwd.path(), Some(&path), mode, &[])?;
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("Unsupported Swagger version '1.2'"),
            "{stderr}"
        );
        assert!(!stderr.contains("panicked"), "{stderr}");
    }
    Ok(())
}

#[test]
fn shared_responses_take_the_operation_produces() -> Result<()> {
    let mut input = document();
    input["responses"] =
        json!({"Gone": {"description": "Gone", "schema": {"$ref": "#/definitions/Err"}}});
    input["definitions"] = json!({"Err": {"type": "string"}});
    input["paths"] = json!({"/a": {
        "get": {"responses": {"404": {"$ref": "#/responses/Gone"}}},
        "put": {"produces": ["text/csv"], "responses": {"404": {"$ref": "#/responses/Gone"}}}
    }});
    let output = swagger2_to_openapi3(input)?;
    assert_eq!(
        output["paths"]["/a"]["get"]["responses"]["404"],
        json!({"$ref": "#/components/responses/Gone"})
    );
    assert_eq!(
        output["paths"]["/a"]["put"]["responses"]["404"],
        json!({"description": "Gone", "content": {
            "text/csv": {"schema": {"$ref": "#/components/schemas/Err"}}
        }})
    );
    assert_parses(&output)
}

#[test]
fn response_examples_without_a_schema_become_content() -> Result<()> {
    let mut input = document();
    input["paths"] = json!({"/a": {"get": {"responses": {"200": {
        "description": "ok",
        "examples": {"application/json": {"id": 1}}
    }}}}});
    let output = swagger2_to_openapi3(input)?;
    assert_eq!(
        output["paths"]["/a"]["get"]["responses"]["200"]["content"],
        json!({"application/json": {"example": {"id": 1}}})
    );
    assert_parses(&output)
}

#[test]
fn refs_inside_literal_data_are_left_as_written() -> Result<()> {
    let mut input = document();
    input["parameters"] = json!({"File": {"name": "file", "in": "formData", "type": "file"}});
    input["definitions"] = json!({
        "Note": {
            "type": "object",
            "example": {"$ref": "#/parameters/File"},
            "default": {"$ref": "#/definitions/Note"},
            "enum": [{"$ref": "#/responses/Gone"}],
            "x-link": {"$ref": "#/definitions/Note"},
            "properties": {
                "example": {"$ref": "#/definitions/Tag"},
                "x-tag": {"$ref": "#/definitions/Tag"}
            }
        },
        "Tag": {"type": "string"},
        "x-Label": {"$ref": "#/definitions/Tag"}
    });
    input["paths"] = json!({
        "/a": {"get": {"responses": {
            "200": {
                "description": "ok",
                "schema": {"$ref": "#/definitions/Note"},
                "examples": {"application/json": {"$ref": "#/definitions/Note"}}
            },
            "x-data": {"$ref": "#/definitions/Tag"}
        }}},
        "x-shared": {"$ref": "#/definitions/Tag"}
    });
    let output = swagger2_to_openapi3(input)?;
    assert_eq!(
        output["paths"]["x-shared"],
        json!({"$ref": "#/definitions/Tag"})
    );
    assert_eq!(
        output["paths"]["/a"]["get"]["responses"]["x-data"],
        json!({"$ref": "#/definitions/Tag"})
    );
    let note = &output["components"]["schemas"]["Note"];
    assert_eq!(note["example"], json!({"$ref": "#/parameters/File"}));
    assert_eq!(note["default"], json!({"$ref": "#/definitions/Note"}));
    assert_eq!(note["enum"], json!([{"$ref": "#/responses/Gone"}]));
    assert_eq!(note["x-link"], json!({"$ref": "#/definitions/Note"}));
    // Property and schema names that look like keywords are still structure.
    assert_eq!(
        note["properties"]["example"],
        json!({"$ref": "#/components/schemas/Tag"})
    );
    assert_eq!(
        note["properties"]["x-tag"],
        json!({"$ref": "#/components/schemas/Tag"})
    );
    assert_eq!(
        output["components"]["schemas"]["x-Label"],
        json!({"$ref": "#/components/schemas/Tag"})
    );
    let media = &output["paths"]["/a"]["get"]["responses"]["200"]["content"]["application/json"];
    assert_eq!(
        media["schema"],
        json!({"$ref": "#/components/schemas/Note"})
    );
    assert_eq!(media["example"], json!({"$ref": "#/definitions/Note"}));
    Ok(())
}

#[test]
fn global_responses_named_like_extensions_are_converted() -> Result<()> {
    let mut input = document();
    input["definitions"] = json!({"Err": {"type": "string"}});
    input["responses"] = json!({"x-Error": {
        "description": "Error",
        "schema": {"$ref": "#/definitions/Err"},
        "examples": {"application/json": "boom"}
    }});
    input["paths"] = json!({"/a": {"get": {"responses": {
        "500": {"$ref": "#/responses/x-Error"},
        "x-note": {"$ref": "#/responses/x-Error"}
    }}}});
    let output = swagger2_to_openapi3(input)?;
    assert_eq!(
        output["components"]["responses"]["x-Error"],
        json!({"description": "Error", "content": {"application/json": {
            "schema": {"$ref": "#/components/schemas/Err"},
            "example": "boom"
        }}})
    );
    let responses = &output["paths"]["/a"]["get"]["responses"];
    assert_eq!(
        responses["500"],
        json!({"$ref": "#/components/responses/x-Error"})
    );
    assert_parses(&output)
}

#[test]
fn response_extensions_are_not_inlined_when_produces_differ() -> Result<()> {
    let mut input = document();
    input["responses"] = json!({"Gone": {"description": "Gone"}});
    input["paths"] = json!({"/a": {"get": {
        "produces": ["text/csv"],
        "responses": {
            "410": {"$ref": "#/responses/Gone"},
            "x-note": {"$ref": "#/responses/Gone"}
        }
    }}});
    let output = swagger2_to_openapi3(input)?;
    let responses = &output["paths"]["/a"]["get"]["responses"];
    assert_eq!(responses["410"], json!({"description": "Gone"}));
    assert_eq!(responses["x-note"], json!({"$ref": "#/responses/Gone"}));
    Ok(())
}

#[test]
fn a_document_declaring_openapi_is_not_converted() -> Result<()> {
    let cwd = tempfile::tempdir()?;
    let path = cwd.path().join("api.yaml");
    std::fs::write(
        &path,
        "openapi: 3.0.1\nswagger: \"2.0\"\nhost: api.example.com\ninfo:\n  title: Both\n  version: \"1\"\npaths: {}\n",
    )?;
    let loaded = load_document(path.to_str(), cwd.path(), false)?;
    // Conversion would set 3.0.3 and turn `host` into a server.
    assert_eq!(loaded.api.openapi, "3.0.1");
    assert!(loaded.api.servers.is_empty());
    Ok(())
}

#[test]
fn files_that_merely_mention_swagger_are_not_detected() -> Result<()> {
    let cwd = tempfile::tempdir()?;
    std::fs::write(
        cwd.path().join("docker-compose.yml"),
        "services:\n  swagger:\n    image: swaggerapi/swagger-ui\n",
    )?;
    std::fs::write(
        cwd.path().join("package.json"),
        "{\"name\": \"app\", \"dependencies\": {\"swagger\": \"^0.7.5\", \"swagger-ui\": \"2.0.0\"}}",
    )?;
    std::fs::write(
        cwd.path().join("legacy.json"),
        "{\"name\": \"legacy\", \"dependencies\": {\"swagger\": \"2.0.0\"}}",
    )?;
    std::fs::write(
        cwd.path().join("ranges.json"),
        "{\"name\": \"ranges\", \"dependencies\": {\"swagger\": \"2.0 - 3.0\"}}",
    )?;
    std::fs::write(
        cwd.path().join("pinned.json"),
        "{\"name\": \"pinned\", \"dependencies\": {\"swagger\": \"2.0\"}}",
    )?;
    std::fs::write(cwd.path().join("range.yaml"), "swagger: 2.0 || 3.0\n")?;
    std::fs::write(
        cwd.path().join("quoted-range.yaml"),
        "swagger: '2.0 || 3.0'\n",
    )?;
    std::fs::write(cwd.path().join("patch.json"), "{\"swagger\": \"2.0.1\"}")?;
    std::fs::write(
        cwd.path().join("brace.json"),
        "{\"description\": \"}\", \"dependencies\": {\"swagger\": \"2.0\"}}",
    )?;
    std::fs::write(cwd.path().join("array.json"), "[{\"swagger\": \"2.0\"}]")?;
    std::fs::write(
        cwd.path().join("flow.yaml"),
        "cfg: {\"swagger\": \"2.0\"}\n",
    )?;
    std::fs::write(cwd.path().join("comma.yaml"), "swagger: 2.0,\n")?;
    std::fs::write(
        cwd.path().join("trailing.yaml"),
        "swagger: \"2.0\" trailing\n",
    )?;
    assert!(phyllotaxis::commands::init::find_document_candidates(cwd.path(), None).is_empty());
    let output = run(cwd.path(), None, "--text", &[])?;
    assert!(String::from_utf8_lossy(&output.stderr).contains("No OpenAPI document found"));
    Ok(())
}

#[test]
fn swagger_headers_in_any_valid_form_are_detected_and_load() -> Result<()> {
    for content in [
        "\"swagger\": \"2.0\"\ninfo:\n  title: Test\n  version: \"1\"\npaths: {}\n",
        "{\"info\": {\"title\": \"a{b\", \"version\": \"1\"}, \"swagger\": \"2.0\", \"paths\": {}}",
        "\u{feff}swagger: \"2.0\"\ninfo:\n  title: Test\n  version: \"1\"\npaths: {}\n",
        "'swagger': '2.0'\ninfo:\n  title: Test\n  version: \"1\"\npaths: {}\n",
    ] {
        let cwd = tempfile::tempdir()?;
        let path = cwd.path().join("contract.yaml");
        std::fs::write(&path, content)?;
        let candidates = phyllotaxis::commands::init::find_document_candidates(cwd.path(), None);
        assert_eq!(candidates, [path.clone()], "{content:?}");
        let loaded = load_document(path.to_str(), cwd.path(), false)?;
        assert_eq!(loaded.api.openapi, "3.0.3", "{content:?}");
        let output = run(cwd.path(), None, "--json", &[])?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

#[test]
fn scope_extensions_are_not_turned_into_scopes() -> Result<()> {
    let mut input = document();
    input["securityDefinitions"] = json!({"oauth": {
        "type": "oauth2",
        "flow": "implicit",
        "authorizationUrl": "https://example.com/auth",
        "scopes": {"read": "Read", "x-note": "vendor", "x-meta": {"a": 1}}
    }});
    let output = swagger2_to_openapi3(input)?;
    assert_eq!(
        output["components"]["securitySchemes"]["oauth"]["flows"]["implicit"]["scopes"],
        json!({"read": "Read"})
    );
    assert_parses(&output)
}
