/// Helper to run the phyllotaxis binary with given args.
/// Returns (stdout, stderr, exit_code).
fn run(args: &[&str]) -> (String, String, i32) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
        .args(args)
        .output()
        .expect("failed to run phyllotaxis binary");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// Helper to run with the petstore fixture as --doc
fn run_with_petstore(args: &[&str]) -> (String, String, i32) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let spec = format!("{}/tests/fixtures/petstore.yaml", manifest_dir);
    let mut full_args = vec!["--doc", &spec];
    full_args.extend_from_slice(args);
    run(&full_args)
}

// ─── Task 14.1: Overview ───

#[test]
fn test_overview_text() {
    let (stdout, _stderr, code) = run_with_petstore(&[]);
    assert_eq!(code, 0, "Expected exit code 0");
    assert!(
        stdout.contains("API: Petstore API"),
        "Missing API title. Got: {}",
        &stdout[..200.min(stdout.len())]
    );
    assert!(
        stdout.contains("phyllotaxis --resources"),
        "Missing resources command hint"
    );
    assert!(
        stdout.contains("phyllotaxis --schemas"),
        "Missing schemas command hint"
    );
    assert!(
        stdout.contains("phyllotaxis --auth"),
        "Missing auth command hint"
    );
}

#[test]
fn test_overview_json() {
    let (stdout, _stderr, code) = run_with_petstore(&["--json"]);
    assert_eq!(code, 0, "Expected exit code 0");
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!(
            "Expected valid JSON. Got: {}",
            &stdout[..200.min(stdout.len())]
        )
    });
    assert_eq!(json["title"], "Petstore API", "JSON missing title");
}

// ─── Task 14.2: Resources ───

#[test]
fn test_resources_list() {
    let (stdout, _stderr, code) = run_with_petstore(&["--resources"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("pets"), "Missing pets group");
    assert!(
        stdout.contains("deprecated-pets"),
        "Missing deprecated-pets group"
    );
    assert!(stdout.contains("[DEPRECATED]"), "Missing DEPRECATED marker");
    assert!(
        stdout.contains("[ALPHA]"),
        "Missing ALPHA marker for experimental group"
    );
}

#[test]
fn test_resources_detail() {
    let (stdout, _stderr, code) = run_with_petstore(&["--resources", "pets"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Resource: Pets"), "Missing resource header");
    assert!(stdout.contains("GET"), "Missing GET method");
    assert!(stdout.contains("POST"), "Missing POST method");
    assert!(stdout.contains("DELETE"), "Missing DELETE method");
    assert!(stdout.contains("/pets"), "Missing /pets path");
}

#[test]
fn test_resources_endpoint_get() {
    let (stdout, _stderr, code) = run_with_petstore(&["--endpoint", "GET", "/pets"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Query Parameters"),
        "Missing query parameters section"
    );
    assert!(stdout.contains("200"), "Missing 200 response");
}

#[test]
fn test_resources_endpoint_post() {
    let (stdout, _stderr, code) = run_with_petstore(&["--endpoint", "POST", "/pets"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Request Body"),
        "Missing request body section"
    );
    assert!(
        stdout.contains("Request Example"),
        "Missing request example"
    );
    assert!(stdout.contains("Errors:"), "Missing errors section");
    assert!(stdout.contains("400"), "Missing 400 error code");
}

#[test]
fn test_resources_not_found() {
    let (_stdout, stderr, code) = run_with_petstore(&["--resources", "notexist"]);
    assert_eq!(code, 1, "Expected exit code 1 for not found");
    assert!(
        stderr.contains("not found"),
        "Expected 'not found' in stderr. Got: {}",
        stderr
    );
}

// ─── Task 14.3: Schemas ───

#[test]
fn test_schemas_list() {
    let (stdout, _stderr, code) = run_with_petstore(&["--schemas"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Pet"), "Missing Pet schema");
    assert!(stdout.contains("Owner"), "Missing Owner schema");
    assert!(stdout.contains("PetList"), "Missing PetList schema");
}

#[test]
fn test_schema_detail_pet() {
    let (stdout, _stderr, code) = run_with_petstore(&["--schemas", "Pet"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Schema: Pet"), "Missing schema header");
    assert!(stdout.contains("string/uuid"), "Missing uuid type for id");
    assert!(stdout.contains("Enum:"), "Missing enum values for status");
    assert!(
        stdout.contains("read-only"),
        "Missing read-only flag for id"
    );
}

#[test]
fn test_schema_detail_expanded() {
    let (stdout, _stderr, code) = run_with_petstore(&["--schemas", "Pet", "--expand"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("(expanded)"), "Missing expanded label");
    assert!(
        stdout.contains("Owner:"),
        "Owner's nested fields should appear after expansion"
    );
}

#[test]
fn test_schema_allof() {
    let (stdout, _stderr, code) = run_with_petstore(&["--schemas", "PetList"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("id"),
        "Expected id field from Pet via allOf"
    );
    assert!(stdout.contains("tags"), "Expected tags field from PetList");
}

#[test]
fn test_schema_oneof() {
    let (stdout, _stderr, code) = run_with_petstore(&["--schemas", "PetOrOwner"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("oneOf"),
        "Expected oneOf composition marker"
    );
    assert!(stdout.contains("Pet"), "Expected Pet as a variant");
    assert!(stdout.contains("Owner"), "Expected Owner as a variant");
}

#[test]
fn test_schema_not_found() {
    let (_stdout, stderr, code) = run_with_petstore(&["--schemas", "NotReal"]);
    assert_eq!(code, 1, "Expected exit code 1 for not found");
    assert!(
        stderr.contains("not found"),
        "Expected 'not found' in stderr"
    );
}

// ─── Task 14.4: Auth and Search ───

#[test]
fn test_auth() {
    let (stdout, _stderr, code) = run_with_petstore(&["--auth"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("bearerAuth"),
        "Missing bearerAuth scheme name"
    );
    assert!(stdout.to_lowercase().contains("http"), "Missing http type");
    assert!(stdout.contains("bearer"), "Missing bearer detail");
}

#[test]
fn test_search_pet() {
    let (stdout, _stderr, code) = run_with_petstore(&["search", "pet"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("pets"),
        "Expected 'pets' in resources section"
    );
    assert!(
        stdout.contains("/pets"),
        "Expected /pets endpoints in results"
    );
    assert!(stdout.contains("Pet"), "Expected Pet in schemas section");
    assert!(
        stdout.contains("PetList"),
        "Expected PetList in schemas section"
    );
}

#[test]
fn test_search_no_results() {
    let (stdout, _stderr, code) = run_with_petstore(&["search", "xyzzy123nonexistent"]);
    assert_eq!(code, 0, "No-results search should exit 0");
    assert!(
        stdout.contains("No results"),
        "Expected no-results message. Got: {}",
        &stdout[..200.min(stdout.len())]
    );
}

// ─── Task 2.3: Error propagation characterization ───

#[test]
fn test_endpoint_not_found() {
    let (_stdout, stderr, code) = run_with_petstore(&["--endpoint", "DELETE", "/nonexistent"]);
    assert_eq!(code, 1, "Expected exit code 1 for endpoint not found");
    assert!(
        stderr.to_lowercase().contains("not found"),
        "Expected 'not found' in stderr. Got: {}",
        stderr
    );
}

// ─── Task 3.1: ANSI escape injection sanitization ───

#[test]
fn test_ansi_injection_sanitized_from_description() {
    use std::io::Write;

    // Build a minimal OpenAPI spec with an ANSI escape sequence in a description field.
    // YAML double-quoted strings support \e as the ESC character (0x1B); the parser will
    // decode it to a real ESC byte, which then flows into the renderer unsanitized unless
    // we explicitly strip it. Raw 0x1B bytes are invalid YAML and get rejected by the parser,
    // so the YAML \e escape is the realistic injection vector.
    let spec_content = "openapi: \"3.0.0\"\ninfo:\n  title: Injection Test\n  version: \"1.0.0\"\n  description: \"Normal \\e[31mRED\\e[0m text\"\npaths: {}\n";

    let mut tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
    tmp.write_all(spec_content.as_bytes())
        .expect("failed to write spec");
    let spec_path = tmp.path().to_str().unwrap().to_owned();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
        .args(["--doc", &spec_path])
        .output()
        .expect("failed to run phyllotaxis");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        !stdout.as_bytes().contains(&0x1B),
        "stdout must not contain ESC (0x1B) — ANSI injection not sanitized. Got: {:?}",
        &stdout[..stdout.len().min(300)]
    );
}

// ─── Task 14.5: Global flags and error cases ───

#[test]
fn test_document_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
        .current_dir(dir.path())
        .output()
        .expect("failed to run binary");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    assert_eq!(code, 1, "Expected exit code 1 when no document found");
    assert!(
        stderr.to_lowercase().contains("not found") || stderr.to_lowercase().contains("no openapi"),
        "Expected error about missing document. Got: {}",
        stderr
    );
}

#[test]
fn test_invalid_document() {
    let (_stdout, stderr, code) = run(&["--doc", "/dev/null"]);
    assert_eq!(code, 1, "Expected exit code 1 for invalid document");
    assert!(
        stderr.to_lowercase().contains("parse")
            || stderr.to_lowercase().contains("failed")
            || stderr.to_lowercase().contains("error"),
        "Expected parse error in stderr. Got: {}",
        stderr
    );
}

#[test]
fn test_json_flag_all_commands() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let spec = format!("{}/tests/fixtures/petstore.yaml", manifest_dir);

    let commands: Vec<Vec<&str>> = vec![
        vec!["--doc", &spec, "--json"],
        vec!["--doc", &spec, "--json", "--resources"],
        vec!["--doc", &spec, "--json", "--resources", "pets"],
        vec!["--doc", &spec, "--json", "--endpoint", "GET", "/pets"],
        vec!["--doc", &spec, "--json", "--schemas"],
        vec!["--doc", &spec, "--json", "--schemas", "Pet"],
        vec!["--doc", &spec, "--json", "--auth"],
        vec!["--doc", &spec, "--json", "search", "pet"],
    ];

    for cmd_args in &commands {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
            .args(cmd_args)
            .output()
            .expect("failed to run binary");
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let code = output.status.code().unwrap_or(-1);

        assert_eq!(
            code,
            0,
            "Expected exit code 0 for {:?}. Stderr: {}",
            cmd_args,
            String::from_utf8_lossy(&output.stderr)
        );

        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
        assert!(
            parsed.is_ok(),
            "Expected valid JSON for {:?}. Got: {}",
            cmd_args,
            &stdout[..200.min(stdout.len())]
        );
    }
}

// ─── Task 3.2: TTY detection / plain output ───

// ─── Task 3.3: JSON mode structured errors and compact output ───

#[test]
fn test_json_error_is_structured() {
    // When --json is passed and a resource is not found, stderr must be valid JSON
    // with an "error" key — not a plain "Error: ..." string.
    let (_stdout, stderr, code) =
        run_with_petstore(&["--json", "--resources", "nonexistentresource"]);
    assert_eq!(code, 1, "Expected exit code 1 for not found");

    let parsed: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap_or_else(|_| {
        panic!(
            "Expected stderr to be valid JSON when --json is set. Got: {:?}",
            &stderr[..200.min(stderr.len())]
        )
    });
    assert!(
        parsed.get("error").is_some(),
        "Expected JSON stderr to have an 'error' key. Got: {}",
        parsed
    );
    assert!(
        !stderr.starts_with("Error:"),
        "stderr must not start with plain 'Error:' prefix when --json is set. Got: {:?}",
        &stderr[..200.min(stderr.len())]
    );
}

#[test]
fn test_json_compact_when_piped() {
    // In test context, stdout is a pipe (non-TTY), so JSON must be compact (single line, no newlines).
    let (stdout, _stderr, code) = run_with_petstore(&["--json", "--resources"]);
    assert_eq!(code, 0, "Expected exit code 0");

    // Must be valid JSON
    let _parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!(
            "Expected valid JSON on stdout. Got: {}",
            &stdout[..200.min(stdout.len())]
        )
    });

    // Compact JSON is a single line — the only newline is the trailing one from println!
    // Pretty-printed JSON has many internal newlines.
    let trimmed = stdout.trim();
    assert!(
        !trimmed.contains('\n'),
        "Compact JSON (non-TTY) must have no internal newlines. Got {} lines. First 200 chars: {:?}",
        trimmed.lines().count(),
        &stdout[..200.min(stdout.len())]
    );
}

/// Run phyllotaxis with the petstore fixture, extra env vars, and captured stdout (non-TTY).
fn run_with_petstore_env(args: &[&str], env: &[(&str, &str)]) -> (String, String, i32) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let spec = format!("{}/tests/fixtures/petstore.yaml", manifest_dir);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
        .args(["--doc", &spec])
        .args(args)
        .envs(env.iter().copied())
        .output()
        .expect("failed to run phyllotaxis binary");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// Returns true if the string contains lines that look like box-drawing separator lines
/// (lines consisting entirely of ─ or = characters, possibly with spaces).
fn has_decorator_lines(s: &str) -> bool {
    s.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && trimmed.chars().all(|c| c == '─' || c == '=' || c == '━')
    })
}

#[test]
fn test_no_color_env_plain_output() {
    let (stdout, _stderr, code) = run_with_petstore_env(&["--resources"], &[("NO_COLOR", "1")]);
    assert_eq!(code, 0);
    // Resource names must still appear
    assert!(
        stdout.contains("pets"),
        "Resource names must appear with NO_COLOR. Got: {}",
        &stdout[..200.min(stdout.len())]
    );
    // No box-drawing separator lines
    assert!(
        !has_decorator_lines(&stdout),
        "NO_COLOR=1 must suppress decorative separator lines. Got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
    // No "Drill deeper:" footer
    assert!(
        !stdout.contains("Drill deeper:"),
        "NO_COLOR=1 must suppress 'Drill deeper:' footer. Got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

#[test]
fn test_term_dumb_plain_output() {
    let (stdout, _stderr, code) = run_with_petstore_env(&["--resources"], &[("TERM", "dumb")]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("pets"),
        "Resource names must appear with TERM=dumb. Got: {}",
        &stdout[..200.min(stdout.len())]
    );
    assert!(
        !has_decorator_lines(&stdout),
        "TERM=dumb must suppress decorative separator lines. Got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
    assert!(
        !stdout.contains("Drill deeper:"),
        "TERM=dumb must suppress 'Drill deeper:' footer. Got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

#[test]
fn test_piped_output_plain() {
    // assert_cmd captures stdout as a pipe, so is_terminal() returns false.
    // run_with_petstore already captures output (non-TTY), so we just assert plain format.
    let (stdout, _stderr, code) = run_with_petstore(&["--resources"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("pets"),
        "Resource names must appear in piped output. Got: {}",
        &stdout[..200.min(stdout.len())]
    );
    assert!(
        !has_decorator_lines(&stdout),
        "Piped (non-TTY) output must not contain decorative separator lines. Got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
    assert!(
        !stdout.contains("Drill deeper:"),
        "Piped output must not contain 'Drill deeper:' footer. Got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

// ─── Task 3.6: Non-interactive init ───

#[test]
fn test_init_non_interactive_creates_config() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let spec = format!("{}/tests/fixtures/petstore.yaml", manifest_dir);
    let dir = tempfile::tempdir().unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
        .current_dir(dir.path())
        .args(["init", "--doc-path", &spec])
        .stdin(std::process::Stdio::null())
        .output()
        .expect("failed to run phyllotaxis");

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    assert_eq!(
        code, 0,
        "Expected exit code 0 for non-interactive init. Stderr: {}",
        stderr
    );

    let config_path = dir.path().join(".phyllotaxis.yaml");
    assert!(
        config_path.exists(),
        ".phyllotaxis.yaml must be written in the working directory"
    );

    let written = std::fs::read_to_string(&config_path).unwrap();
    assert!(
        written.contains("petstore.yaml"),
        ".phyllotaxis.yaml must contain the document path. Got:\n{}",
        written
    );
}

#[test]
fn test_init_non_interactive_missing_doc_exits_1() {
    let dir = tempfile::tempdir().unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
        .current_dir(dir.path())
        .args(["init", "--doc-path", "nonexistent/path.yaml"])
        .stdin(std::process::Stdio::null())
        .output()
        .expect("failed to run phyllotaxis");

    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 1, "Expected exit code 1 when doc-path does not exist");
}

// ─── Task 3.7: Accessible arrow labels ───

#[test]
fn test_non_tty_no_unicode_arrow_in_response_schema() {
    // Non-TTY output (piped, which is what Command::output gives us) must not contain
    // the Unicode → character. It should use ASCII -> instead.
    // POST /pets returns 201 Created with schema ref Pet, so → appears in responses.
    let (stdout, _stderr, code) = run_with_petstore(&["--endpoint", "POST", "/pets"]);
    assert_eq!(code, 0);
    assert!(
        !stdout.contains('→'),
        "Non-TTY output must not contain Unicode → arrow. Got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
    assert!(
        stdout.contains("->"),
        "Non-TTY output must use ASCII -> instead of →. Got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

#[test]
fn test_non_tty_no_unicode_arrow_in_discriminator() {
    // schemas PetOrOwner has a discriminator with mappings, which also render with →.
    let (stdout, _stderr, code) = run_with_petstore(&["--schemas", "PetOrOwner"]);
    assert_eq!(code, 0);
    assert!(
        !stdout.contains('→'),
        "Non-TTY output must not contain Unicode → arrow in discriminator. Got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
    assert!(
        stdout.contains("->"),
        "Non-TTY output must use ASCII -> for discriminator mappings. Got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

// ─── Task 4.2: Shell Completions ───

#[test]
fn test_completions_bash() {
    let (stdout, _stderr, code) = run(&["completions", "bash"]);
    assert_eq!(code, 0, "Expected exit code 0 for bash completions");
    assert!(
        !stdout.is_empty(),
        "Bash completion output must not be empty"
    );
    assert!(
        stdout.contains("phyllotaxis"),
        "Bash completion script must reference the binary name. Got: {}",
        &stdout[..200.min(stdout.len())]
    );
}

#[test]
fn test_completions_zsh() {
    let (stdout, _stderr, code) = run(&["completions", "zsh"]);
    assert_eq!(code, 0, "Expected exit code 0 for zsh completions");
    assert!(
        !stdout.is_empty(),
        "Zsh completion output must not be empty"
    );
    assert!(
        stdout.contains("phyllotaxis"),
        "Zsh completion script must reference the binary name. Got: {}",
        &stdout[..200.min(stdout.len())]
    );
}

#[test]
fn test_completions_fish() {
    let (stdout, _stderr, code) = run(&["completions", "fish"]);
    assert_eq!(code, 0, "Expected exit code 0 for fish completions");
    assert!(
        !stdout.is_empty(),
        "Fish completion output must not be empty"
    );
    assert!(
        stdout.contains("phyllotaxis"),
        "Fish completion script must reference the binary name. Got: {}",
        &stdout[..200.min(stdout.len())]
    );
}

// ─── Task 23: Kitchen-sink coverage gap integration tests ───

/// Helper to run with the kitchen-sink fixture as --doc
fn run_with_kitchen_sink(args: &[&str]) -> (String, String, i32) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let spec = format!("{}/tests/fixtures/kitchen-sink.yaml", manifest_dir);
    let mut full_args = vec!["--doc", &spec];
    full_args.extend_from_slice(args);
    run(&full_args)
}

// Success criterion 1: Non-JSON request bodies (multipart/form-data)
#[test]
fn test_multipart_body_visible_in_upload_endpoint() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["--endpoint", "POST", "/files/upload"]);
    assert_eq!(code, 0, "Expected exit code 0");
    assert!(
        stdout.contains("multipart/form-data"),
        "Missing content type, got:\n{}",
        &stdout[..300.min(stdout.len())]
    );
    assert!(
        stdout.contains("file"),
        "Missing file field, got:\n{}",
        &stdout[..300.min(stdout.len())]
    );
}

// Success criterion 2: Response headers
#[test]
fn test_response_headers_visible() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["--endpoint", "GET", "/users"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("X-Total-Count"),
        "Missing response header, got:\n{}",
        &stdout[..300.min(stdout.len())]
    );
}

// Success criterion 3: Callbacks list
#[test]
fn test_callbacks_list_kitchen_sink() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["--callbacks"]);
    assert_eq!(code, 0, "Expected exit code 0");
    assert!(stdout.contains("onEvent"), "Missing onEvent callback");
    assert!(
        stdout.contains("onStatusChange"),
        "Missing onStatusChange callback"
    );
}

// Success criterion 4: Callback detail
#[test]
fn test_callbacks_detail_on_event() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["--callbacks", "onEvent"]);
    assert_eq!(code, 0, "Expected exit code 0");
    assert!(stdout.contains("Callback: onEvent"), "Missing header");
    assert!(stdout.contains("EventPayload"), "Missing body schema");
}

#[test]
fn test_callbacks_not_found() {
    let (_stdout, stderr, code) = run_with_kitchen_sink(&["--callbacks", "nonexistent"]);
    assert_eq!(code, 1, "Expected exit code 1");
    assert!(stderr.contains("not found"), "Missing not found message");
}

// Success criterion 5: Links
#[test]
fn test_links_visible_on_post_users() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["--endpoint", "POST", "/users"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("GetCreatedUser"),
        "Missing link, got:\n{}",
        &stdout[..300.min(stdout.len())]
    );
}

// Success criterion 6: Schema constraints
#[test]
fn test_schema_constraints_visible() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["--schemas", "User"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("min:3"),
        "Missing min constraint, got:\n{}",
        &stdout[..300.min(stdout.len())]
    );
    assert!(
        stdout.contains("max:32"),
        "Missing max constraint, got:\n{}",
        &stdout[..300.min(stdout.len())]
    );
}

// Success criterion 7: writeOnly
#[test]
fn test_write_only_visible_on_create_user_request() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["--schemas", "CreateUserRequest"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("write-only"),
        "Missing write-only on password field, got:\n{}",
        &stdout[..300.min(stdout.len())]
    );
}

// Success criterion 8: deprecated
#[test]
fn test_deprecated_visible_on_pet_base() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["--schemas", "PetBase"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("DEPRECATED"),
        "Missing DEPRECATED on legacy_code, got:\n{}",
        &stdout[..300.min(stdout.len())]
    );
}

// Success criterion 9: Schema title
#[test]
fn test_schema_title_visible() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["--schemas", "GeoLocation"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Geographic Location"),
        "Missing title, got:\n{}",
        &stdout[..300.min(stdout.len())]
    );
}

// Success criterion 10: Integer enums
#[test]
fn test_integer_enum_visible() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["--schemas", "Priority"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains('0'),
        "Missing integer enum value, got:\n{}",
        &stdout[..300.min(stdout.len())]
    );
    assert!(
        stdout.contains('4'),
        "Missing integer enum value 4, got:\n{}",
        &stdout[..300.min(stdout.len())]
    );
}

// ─── Review Fixes ───

#[test]
fn test_empty_request_body_shows_raw_body_message() {
    let (stdout, _stderr, code) =
        run_with_kitchen_sink(&["--endpoint", "POST", "/admin/bulk-import"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Raw body (no schema)"),
        "Empty request body should show 'Raw body (no schema)', got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

#[test]
fn test_exclusive_min_max_shows_operators() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["--schemas", "GeoLocation"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains(">0"),
        "exclusiveMinimum should display as '>0', got:\n{}",
        stdout
    );
    // Should NOT show the bare word "exclusiveMinimum"
    assert!(
        !stdout.contains("exclusiveMinimum"),
        "Should not show bare 'exclusiveMinimum' label, got:\n{}",
        stdout
    );
}

#[test]
fn test_array_item_type_propagation() {
    let (stdout, _stderr, code) =
        run_with_kitchen_sink(&["--endpoint", "POST", "/files/upload-batch"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("binary[]"),
        "Array of binary items should show 'binary[]', got:\n{}",
        stdout
    );
}

#[test]
fn test_no_trailing_whitespace_on_empty_header_description() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["--endpoint", "HEAD", "/health"]);
    assert_eq!(code, 0);
    // Find the X-Health-Status line and check for trailing whitespace
    for line in stdout.lines() {
        if line.contains("X-Health-Status") {
            assert_eq!(
                line,
                line.trim_end(),
                "Header line should not have trailing whitespace"
            );
        }
    }
}

#[test]
fn test_json_no_top_level_links() {
    let (stdout, _stderr, code) =
        run_with_kitchen_sink(&["--json", "--endpoint", "POST", "/users"]);
    assert_eq!(code, 0);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|_| panic!("Invalid JSON: {}", &stdout[..200.min(stdout.len())]));
    // Top-level "links" should not exist in JSON output
    assert!(
        json.get("links").is_none(),
        "JSON endpoint detail should not have top-level 'links' (they belong on individual responses)"
    );
    // But links should still exist on individual responses
    let responses = json.get("responses").expect("responses should exist");
    let has_response_links = responses.as_array().unwrap().iter().any(|r| {
        r.get("links")
            .map(|l| l.as_array().map(|a| !a.is_empty()).unwrap_or(false))
            .unwrap_or(false)
    });
    assert!(
        has_response_links,
        "Links should still appear on individual responses"
    );
}

#[test]
fn test_callback_list_shows_operation_count() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["--callbacks"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("(1 operation)") || stdout.contains("(2 operations)"),
        "Callback list should show operation count, got:\n{}",
        stdout
    );
}

#[test]
fn test_expand_on_endpoint_shows_nested_fields() {
    let (stdout, _stderr, code) = run_with_petstore(&["--endpoint", "POST", "/pets", "--expand"]);
    assert_eq!(code, 0);
    // With --expand, the owner field should show its nested fields (id, name)
    assert!(
        stdout.contains("owner") && (stdout.contains("Owner:")),
        "With --expand, owner field should show as 'Owner:' with nested fields, got:\n{}",
        stdout
    );
}

#[test]
fn test_search_finds_callbacks() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&["search", "onEvent"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Callbacks:"),
        "Search must include a Callbacks section, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
    assert!(
        stdout.contains("onEvent"),
        "Search for 'onEvent' must find the callback, got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

#[test]
fn test_callbacks_fuzzy_suggestion_on_typo() {
    let (_stdout, stderr, code) = run_with_kitchen_sink(&["--callbacks", "onEven"]);
    assert_eq!(code, 1, "Expected exit code 1 for not found");
    assert!(
        stderr.contains("onEvent"),
        "Expected suggestion 'onEvent' for typo 'onEven', got:\n{}",
        stderr
    );
}

#[test]
fn test_overview_shows_callback_count() {
    let (stdout, _stderr, code) = run_with_kitchen_sink(&[]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("2 available") && stdout.contains("callbacks"),
        "Overview must include callback count for kitchen-sink (2 callbacks), got:\n{}",
        &stdout[..500.min(stdout.len())]
    );
}

// ─── Item 7: Enhanced Overview — top resources ───

#[test]
fn test_overview_shows_top_resources() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let spec = format!("{}/tests/fixtures/kitchen-sink.yaml", manifest_dir);
    let (stdout, _stderr, code) = run(&["--doc", &spec]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Top Resources"),
        "Overview should show Top Resources section. Got: {}",
        stdout
    );
}

#[test]
fn test_overview_json_includes_top_resources() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let spec = format!("{}/tests/fixtures/kitchen-sink.yaml", manifest_dir);
    let (stdout, _stderr, code) = run(&["--doc", &spec, "--json"]);
    assert_eq!(code, 0);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        json["top_resources"].is_array(),
        "JSON overview should include top_resources array"
    );
    assert!(
        !json["top_resources"].as_array().unwrap().is_empty(),
        "top_resources should not be empty for kitchen-sink spec"
    );
}

// Success criterion 11: No regressions on petstore
#[test]
fn test_petstore_regression() {
    let (stdout, _stderr, code) = run_with_petstore(&["--endpoint", "POST", "/pets"]);
    assert_eq!(code, 0, "Petstore regression: POST /pets should still work");
    assert!(
        stdout.contains("Request Body"),
        "Regression: missing request body"
    );
}

// ─── Item 3: Drill-deeper uses detected binary name ───

#[test]
fn test_drill_deeper_uses_binary_name() {
    // Drill-deeper hints should use the detected binary name.
    // Use --json which always includes drill_deeper regardless of TTY.
    let (stdout, _stderr, code) = run_with_petstore(&["--json", "--resources", "pets"]);
    assert_eq!(code, 0);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!(
            "Expected valid JSON. Got: {}",
            &stdout[..200.min(stdout.len())]
        )
    });
    let drill_deeper = json["drill_deeper"]
        .as_array()
        .expect("drill_deeper should be array");
    assert!(
        !drill_deeper.is_empty(),
        "drill_deeper should be non-empty for pets resource with endpoints"
    );
    // Every drill-deeper entry should start with the binary name
    for entry in drill_deeper {
        let s = entry.as_str().unwrap_or("");
        assert!(
            s.starts_with("phyllotaxis ") || s.starts_with("phyll "),
            "Drill deeper entry should start with binary name, got: {}",
            s
        );
    }
}

#[test]
fn test_overview_alias_tip_shown_when_invoked_as_phyllotaxis() {
    let (stdout, _stderr, code) = run_with_petstore(&[]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("alias") || stdout.contains("phyll"),
        "Overview should mention phyll alias when invoked as phyllotaxis. Got: {}",
        stdout
    );
}

#[test]
fn test_schema_list_drill_deeper_uses_phyll() {
    // Use --json which always includes drill_deeper regardless of TTY
    let (stdout, _stderr, code) = run_with_petstore(&["--json", "--schemas"]);
    assert_eq!(code, 0);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!(
            "Expected valid JSON. Got: {}",
            &stdout[..200.min(stdout.len())]
        )
    });
    let drill_deeper = json["drill_deeper"]
        .as_str()
        .expect("drill_deeper should be a string");
    assert!(
        drill_deeper.starts_with("phyllotaxis ") || drill_deeper.starts_with("phyll "),
        "Schema list drill_deeper should start with binary name. Got: {}",
        drill_deeper
    );
}

// ─── Item 5: --example flag ───

#[test]
fn test_schemas_example_flag() {
    let (stdout, _stderr, code) = run_with_petstore(&["--schemas", "Pet", "--example"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Example"),
        "Should show example header. Got: {}",
        stdout
    );
    // Pet has 'name' as required field
    assert!(
        stdout.contains("\"name\"") || stdout.contains("name"),
        "Example should include Pet's name field. Got: {}",
        stdout
    );
}

#[test]
fn test_schemas_example_flag_json() {
    let (stdout, _stderr, code) = run_with_petstore(&["--json", "--schemas", "Pet", "--example"]);
    assert_eq!(code, 0);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|_| panic!("Expected valid JSON. Got: {}", stdout));
    assert_eq!(json["schema"], "Pet");
    assert_eq!(json["source"], "auto-generated");
    assert!(
        json["example"].is_object(),
        "Example should be a JSON object"
    );
}

// ─── CLI composition and migration ────────────────────────────────────────

#[test]
fn test_multi_flag_resources_and_schemas() {
    // Combining --resources and --schemas in one call should produce both outputs
    let (stdout, _stderr, code) = run_with_petstore(&["--resources", "--schemas"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("pets"),
        "Combined output should include resource list. Got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
    assert!(
        stdout.contains("Pet"),
        "Combined output should include schema list. Got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

#[test]
fn test_multi_flag_resources_detail_and_schemas_detail() {
    // Drill into both at once: --resources pets --schemas Pet
    let (stdout, _stderr, code) = run_with_petstore(&["--resources", "pets", "--schemas", "Pet"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Resource: Pets"),
        "Should contain resource detail header. Got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
    assert!(
        stdout.contains("Schema: Pet"),
        "Should contain schema detail header. Got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

#[test]
fn test_multi_flag_resources_and_auth() {
    let (stdout, _stderr, code) = run_with_petstore(&["--resources", "--auth"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("pets"), "Should include resource list");
    assert!(stdout.contains("bearerAuth"), "Should include auth details");
}

#[test]
fn test_multi_flag_json_composition() {
    // JSON mode with multiple flags produces a single valid JSON document with top-level keys
    let (stdout, _stderr, code) = run_with_petstore(&["--json", "--resources", "--auth"]);
    assert_eq!(code, 0);

    let doc: serde_json::Value =
        serde_json::from_str(&stdout).expect("Multi-flag JSON should be a single valid document");
    assert!(
        doc.get("resources").is_some(),
        "Should have 'resources' top-level key"
    );
    assert!(
        doc.get("auth").is_some(),
        "Should have 'auth' top-level key"
    );
    // Resources should contain the actual resource data
    assert!(
        doc["resources"]["resources"].is_array(),
        "resources.resources should be an array"
    );
    // Auth should contain schemes
    assert!(
        doc["auth"]["schemes"].is_array(),
        "auth.schemes should be an array"
    );
}

#[test]
fn test_single_flag_json_unchanged() {
    // Single-flag JSON preserves original shape (no wrapper key)
    let (stdout, _stderr, code) = run_with_petstore(&["--json", "--resources"]);
    assert_eq!(code, 0);

    let doc: serde_json::Value =
        serde_json::from_str(&stdout).expect("Single-flag JSON should be valid");
    // Should have direct keys (resources, drill_deeper), NOT wrapped under "resources"
    assert!(
        doc.get("resources").is_some(),
        "Should have 'resources' key directly (not wrapped)"
    );
    assert!(
        doc.get("drill_deeper").is_some(),
        "Should have 'drill_deeper' key directly"
    );
}

#[test]
fn test_multi_flag_text_unchanged() {
    // Text mode with multiple flags still concatenates output
    let (stdout, _stderr, code) = run_with_petstore(&["--resources", "--auth"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Resources:"),
        "Text output should contain Resources section"
    );
    assert!(
        stdout.contains("Authentication:"),
        "Text output should contain Authentication section"
    );
}

#[test]
fn test_context_implies_expand() {
    // --context should expand inline objects in request bodies without explicit --expand
    let (stdout, _stderr, code) = run_with_petstore(&["--endpoint", "POST", "/pets", "--context"]);
    assert_eq!(code, 0);
    // Owner should be expanded to show nested fields
    assert!(
        stdout.contains("Owner:"),
        "--context should expand Owner ref. Got:\n{}",
        &stdout[..500.min(stdout.len())]
    );
    // Existing --expand behavior should be unchanged
    let (expand_stdout, _, expand_code) =
        run_with_petstore(&["--endpoint", "POST", "/pets", "--expand"]);
    assert_eq!(expand_code, 0);
    assert!(
        expand_stdout.contains("Owner:"),
        "--expand should still work independently"
    );
}

#[test]
fn test_no_empty_resource_groups() {
    // Resource groups with 0 endpoints should be hidden
    let (stdout, _stderr, code) = run_with_petstore(&["--json", "--resources"]);
    assert_eq!(code, 0);
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    for group in doc["resources"].as_array().unwrap() {
        assert!(
            group["endpoint_count"].as_u64().unwrap() > 0,
            "Resource group '{}' has 0 endpoints and should be hidden",
            group["slug"]
        );
    }
}

#[test]
fn test_positional_document_arg() {
    // phyll <document> --resources should use document as the path
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let spec = format!("{}/tests/fixtures/petstore.yaml", manifest_dir);
    let (stdout, _stderr, code) = run(&[&spec, "--resources"]);
    assert_eq!(
        code, 0,
        "Positional document arg should work as document path"
    );
    assert!(
        stdout.contains("pets"),
        "Should list resources from positional doc arg. Got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

#[test]
fn test_positional_document_overrides_doc_flag() {
    // Positional document takes priority over --doc
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let petstore = format!("{}/tests/fixtures/petstore.yaml", manifest_dir);
    let kitchen_sink = format!("{}/tests/fixtures/kitchen-sink.yaml", manifest_dir);
    // Pass kitchen-sink as positional, petstore as --doc — should use kitchen-sink
    let (stdout, _stderr, code) = run(&[&kitchen_sink, "--doc", &petstore, "--resources"]);
    assert_eq!(code, 0);
    // kitchen-sink has /users, petstore does not
    assert!(
        stdout.contains("users"),
        "Positional doc should override --doc. Expected kitchen-sink resources. Got:\n{}",
        &stdout[..400.min(stdout.len())]
    );
}

#[test]
fn test_migration_guard_resources() {
    let (_stdout, _stderr, code) = run_with_petstore(&[]);
    // First confirm petstore loads fine
    assert_eq!(code, 0);

    // Now test the migration guard: "resources" as positional should be caught
    // Use a tmpdir with no document so the only arg is the old subcommand name
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
        .current_dir(dir.path())
        .arg("resources")
        .output()
        .expect("failed to run");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 1, "Old subcommand should fail with migration hint");
    assert!(
        stderr.contains("removed in v2.0"),
        "Should show migration hint. Got: {}",
        stderr
    );
    assert!(
        stderr.contains("--resources"),
        "Migration hint should suggest new syntax. Got: {}",
        stderr
    );
}

#[test]
fn test_migration_guard_schemas() {
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
        .current_dir(dir.path())
        .arg("schemas")
        .output()
        .expect("failed to run");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 1);
    assert!(stderr.contains("removed in v2.0"));
    assert!(stderr.contains("--schemas"));
}

#[test]
fn test_migration_guard_auth() {
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
        .current_dir(dir.path())
        .arg("auth")
        .output()
        .expect("failed to run");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 1);
    assert!(stderr.contains("removed in v2.0"));
    assert!(stderr.contains("--auth"));
}

#[test]
fn test_migration_guard_callbacks() {
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
        .current_dir(dir.path())
        .arg("callbacks")
        .output()
        .expect("failed to run");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 1);
    assert!(stderr.contains("removed in v2.0"));
    assert!(stderr.contains("--callbacks"));
}

#[test]
fn test_migration_guard_endpoints() {
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
        .current_dir(dir.path())
        .arg("endpoints")
        .output()
        .expect("failed to run");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 1);
    assert!(stderr.contains("removed in v2.0"));
    assert!(stderr.contains("--resources"));
}

#[test]
fn test_migration_guard_json_mode() {
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
        .current_dir(dir.path())
        .args(["schemas", "--json"])
        .output()
        .expect("failed to run");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 1);
    // JSON mode should produce structured error
    let parsed: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap_or_else(|_| {
        panic!(
            "Migration error in --json mode should be valid JSON. Got: {}",
            stderr
        )
    });
    assert!(parsed.get("error").is_some());
    assert!(
        parsed["error"]
            .as_str()
            .unwrap()
            .contains("removed in v2.0"),
        "JSON error should contain migration hint"
    );
}

#[test]
fn test_no_flags_shows_overview() {
    // No flags at all should show overview, not error
    let (stdout, _stderr, code) = run_with_petstore(&[]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("API: Petstore API"),
        "No flags should show overview"
    );
}

#[test]
fn test_used_by_flag() {
    let (stdout, _stderr, code) = run_with_petstore(&["--schemas", "Pet", "--used-by"]);
    assert_eq!(code, 0);
    // Pet is used in GET /pets and POST /pets
    assert!(
        stdout.contains("/pets"),
        "--used-by should show endpoints using Pet schema. Got:\n{}",
        stdout
    );
}

// ─── External $ref dereferencing ──────────────────────────────────────────

/// Helper to run with the multi-file fixture as --doc
fn run_with_multi_file(args: &[&str]) -> (String, String, i32) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let spec = format!("{}/tests/fixtures/multi-file/openapi.yaml", manifest_dir);
    let mut full_args = vec!["--doc", &spec];
    full_args.extend_from_slice(args);
    run(&full_args)
}

#[test]
fn test_multi_file_overview() {
    let (stdout, _stderr, code) = run_with_multi_file(&[]);
    assert_eq!(code, 0, "Expected exit code 0 for multi-file spec");
    assert!(
        stdout.contains("Multi-File API"),
        "Missing API title. Got: {}",
        &stdout[..200.min(stdout.len())]
    );
}

#[test]
fn test_multi_file_overview_json() {
    let (stdout, _stderr, code) = run_with_multi_file(&["--json"]);
    assert_eq!(code, 0);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        panic!(
            "Expected valid JSON. Got: {}",
            &stdout[..200.min(stdout.len())]
        )
    });
    assert_eq!(json["title"], "Multi-File API");
}

#[test]
fn test_multi_file_schemas_list() {
    let (stdout, _stderr, code) = run_with_multi_file(&["--schemas"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Pet"),
        "Pet schema (from schemas/pet.yaml) must appear. Got: {}",
        &stdout[..300.min(stdout.len())]
    );
    assert!(
        stdout.contains("Error"),
        "Error schema (from schemas/common.yaml#/...) must appear. Got: {}",
        &stdout[..300.min(stdout.len())]
    );
}

#[test]
fn test_multi_file_resources_list() {
    let (stdout, _stderr, code) = run_with_multi_file(&["--resources"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("pets") || stdout.contains("Pets"),
        "Pets resource (from paths/pets.yaml) must appear. Got: {}",
        &stdout[..300.min(stdout.len())]
    );
}

// ─── External $ref error cases ────────────────────────────────────────────

#[test]
fn test_external_ref_missing_file_error() {
    let tmp = tempfile::tempdir().unwrap();
    let spec_content = r#"openapi: "3.0.3"
info:
  title: Bad Ref API
  version: "1.0.0"
paths: {}
components:
  schemas:
    Pet:
      $ref: "./schemas/does-not-exist.yaml"
"#;
    let spec_path = tmp.path().join("openapi.yaml");
    std::fs::write(&spec_path, spec_content).unwrap();

    let (_stdout, stderr, code) = run(&["--doc", spec_path.to_str().unwrap(), "--schemas"]);

    assert_ne!(code, 0, "Expected non-zero exit for missing ref file");
    assert!(
        stderr.contains("does-not-exist.yaml") || stderr.contains("$ref"),
        "Error message should name the missing file. Got: {}",
        stderr
    );
}

#[test]
fn test_external_ref_invalid_fragment_error() {
    let tmp = tempfile::tempdir().unwrap();
    let shared_content = r#"components:
  schemas:
    RealSchema:
      type: object
"#;
    std::fs::write(tmp.path().join("shared.yaml"), shared_content).unwrap();

    let spec_content = r#"openapi: "3.0.3"
info:
  title: Bad Fragment API
  version: "1.0.0"
paths: {}
components:
  schemas:
    Missing:
      $ref: "./shared.yaml#/components/schemas/DoesNotExist"
"#;
    let spec_path = tmp.path().join("openapi.yaml");
    std::fs::write(&spec_path, spec_content).unwrap();

    let (_stdout, stderr, code) = run(&["--doc", spec_path.to_str().unwrap(), "--schemas"]);

    assert_ne!(
        code, 0,
        "Expected non-zero exit for invalid fragment pointer"
    );
    assert!(
        stderr.contains("DoesNotExist")
            || stderr.contains("fragment")
            || stderr.contains("pointer"),
        "Error message should mention the bad fragment. Got: {}",
        stderr
    );
}

#[test]
fn test_external_ref_circular_ref_handled() {
    // Circular refs should be converted to local refs, not error
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("a.yaml"),
        "type: object\nproperties:\n  b:\n    $ref: \"./b.yaml\"\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("b.yaml"),
        "type: object\nproperties:\n  a:\n    $ref: \"./a.yaml\"\n",
    )
    .unwrap();

    let spec_content = r#"openapi: "3.0.3"
info:
  title: Circular API
  version: "1.0.0"
paths: {}
components:
  schemas:
    A:
      $ref: "./a.yaml"
"#;
    let spec_path = tmp.path().join("openapi.yaml");
    std::fs::write(&spec_path, spec_content).unwrap();

    let (stdout, _stderr, code) = run(&["--doc", spec_path.to_str().unwrap(), "--schemas"]);

    assert_eq!(
        code, 0,
        "Circular refs should be handled gracefully, not error"
    );
    assert!(
        stdout.contains("A"),
        "Schema A should appear in output. Got: {}",
        &stdout[..300.min(stdout.len())]
    );
}
