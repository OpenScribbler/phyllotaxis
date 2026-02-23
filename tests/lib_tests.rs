/// Direct library function tests — call internal APIs without spawning a subprocess.
/// These require `src/lib.rs` to exist as a re-export hub (task 2.1).

use std::path::PathBuf;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn petstore_path() -> String {
    manifest_dir()
        .join("tests/fixtures/petstore.yaml")
        .to_str()
        .unwrap()
        .to_string()
}

// ─── 1. spec::load_spec with a valid fixture ───────────────────────────────

#[test]
fn test_load_spec_valid_fixture() {
    let spec_path = petstore_path();
    let cwd = manifest_dir();

    let loaded = phyllotaxis::spec::load_spec(Some(&spec_path), &cwd)
        .expect("load_spec should succeed for petstore fixture");

    assert_eq!(loaded.api.info.title, "Petstore API");
    assert_eq!(loaded.api.info.version, "1.0.0");
}

// ─── 2. spec::load_spec with a nonexistent path ────────────────────────────

#[test]
fn test_load_spec_nonexistent_path() {
    let cwd = manifest_dir();

    let result = phyllotaxis::spec::load_spec(
        Some("/absolutely/does/not/exist/spec.yaml"),
        &cwd,
    );

    assert!(result.is_err(), "load_spec should return Err for missing file");
}

// ─── 3. commands::resources::extract_resource_groups ──────────────────────

#[test]
fn test_extract_resource_groups_direct() {
    let spec_path = petstore_path();
    let cwd = manifest_dir();
    let loaded = phyllotaxis::spec::load_spec(Some(&spec_path), &cwd)
        .expect("load_spec failed");

    let groups = phyllotaxis::commands::resources::extract_resource_groups(&loaded.api);

    assert!(!groups.is_empty(), "extract_resource_groups should return at least one group");

    let slugs: Vec<&str> = groups.iter().map(|g| g.slug.as_str()).collect();
    assert!(
        slugs.contains(&"pets"),
        "Expected 'pets' resource group, got: {:?}",
        slugs
    );
}

// ─── 4. commands::schemas::list_schemas ────────────────────────────────────

#[test]
fn test_list_schemas_direct() {
    let spec_path = petstore_path();
    let cwd = manifest_dir();
    let loaded = phyllotaxis::spec::load_spec(Some(&spec_path), &cwd)
        .expect("load_spec failed");

    let names = phyllotaxis::commands::schemas::list_schemas(&loaded.api);

    assert!(!names.is_empty(), "list_schemas should return schema names");
    assert!(names.contains(&"Pet".to_string()), "Expected 'Pet' schema");
    assert!(names.contains(&"Owner".to_string()), "Expected 'Owner' schema");
}

// ─── 5. render::text::render_overview ─────────────────────────────────────

#[test]
fn test_render_overview_text_direct() {
    let spec_path = petstore_path();
    let cwd = manifest_dir();
    let loaded = phyllotaxis::spec::load_spec(Some(&spec_path), &cwd)
        .expect("load_spec failed");

    let data = phyllotaxis::commands::overview::build(&loaded);
    let output = phyllotaxis::render::text::render_overview(&data, true);

    assert!(
        output.contains("API: Petstore API"),
        "render_overview should contain API title. Got:\n{}",
        output
    );
    assert!(
        output.contains("phyllotaxis resources"),
        "render_overview should contain resources hint"
    );
    assert!(
        output.contains("phyllotaxis schemas"),
        "render_overview should contain schemas hint"
    );
}
