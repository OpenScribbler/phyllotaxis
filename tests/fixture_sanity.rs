#[test]
fn test_petstore_fixture_parses() {
    let yaml = std::fs::read_to_string("tests/fixtures/petstore.yaml")
        .expect("fixture file should exist");
    let api: openapiv3::OpenAPI = serde_yaml_ng::from_str(&yaml)
        .expect("fixture should parse as valid OpenAPI 3.0");

    assert_eq!(api.info.title, "Petstore API");
    assert_eq!(api.info.version, "1.0.0");

    // Verify tags
    let tag_names: Vec<&str> = api.tags.iter().map(|t| t.name.as_str()).collect();
    assert!(tag_names.contains(&"Pets"));
    assert!(tag_names.contains(&"Deprecated Pets (Deprecated)"));
    assert!(tag_names.contains(&"Experimental (Alpha)"));

    // Verify schemas
    let schemas = &api.components.as_ref().unwrap().schemas;
    assert!(schemas.contains_key("Pet"));
    assert!(schemas.contains_key("Owner"));
    assert!(schemas.contains_key("PetList"));
    assert!(schemas.contains_key("PetOrOwner"));

    // Verify paths
    assert_eq!(api.paths.paths.len(), 6); // /pets, /pets/{id}, /old-pets, /pets/search, /animals, /pascal-resource

    // Verify security scheme
    let sec = &api.components.as_ref().unwrap().security_schemes;
    assert!(sec.contains_key("bearerAuth"));
}
