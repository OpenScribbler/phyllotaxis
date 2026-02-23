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

/// Helper to run with the petstore fixture as --spec
fn run_with_petstore(args: &[&str]) -> (String, String, i32) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let spec = format!("{}/tests/fixtures/petstore.yaml", manifest_dir);
    let mut full_args = vec!["--spec", &spec];
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
        stdout.contains("phyllotaxis resources"),
        "Missing resources command hint"
    );
    assert!(
        stdout.contains("phyllotaxis schemas"),
        "Missing schemas command hint"
    );
    assert!(
        stdout.contains("phyllotaxis auth"),
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
    let (stdout, _stderr, code) = run_with_petstore(&["resources"]);
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
    let (stdout, _stderr, code) = run_with_petstore(&["resources", "pets"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Resource: Pets"), "Missing resource header");
    assert!(stdout.contains("GET"), "Missing GET method");
    assert!(stdout.contains("POST"), "Missing POST method");
    assert!(stdout.contains("DELETE"), "Missing DELETE method");
    assert!(stdout.contains("/pets"), "Missing /pets path");
}

#[test]
fn test_resources_endpoint_get() {
    let (stdout, _stderr, code) = run_with_petstore(&["resources", "pets", "GET", "/pets"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Query Parameters"),
        "Missing query parameters section"
    );
    assert!(
        stdout.contains("200"),
        "Missing 200 response"
    );
}

#[test]
fn test_resources_endpoint_post() {
    let (stdout, _stderr, code) = run_with_petstore(&["resources", "pets", "POST", "/pets"]);
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
    let (_stdout, stderr, code) = run_with_petstore(&["resources", "notexist"]);
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
    let (stdout, _stderr, code) = run_with_petstore(&["schemas"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Pet"), "Missing Pet schema");
    assert!(stdout.contains("Owner"), "Missing Owner schema");
    assert!(stdout.contains("PetList"), "Missing PetList schema");
}

#[test]
fn test_schema_detail_pet() {
    let (stdout, _stderr, code) = run_with_petstore(&["schemas", "Pet"]);
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
    let (stdout, _stderr, code) = run_with_petstore(&["schemas", "Pet", "--expand"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("(expanded)"),
        "Missing expanded label"
    );
    assert!(
        stdout.contains("Owner:"),
        "Owner's nested fields should appear after expansion"
    );
}

#[test]
fn test_schema_allof() {
    let (stdout, _stderr, code) = run_with_petstore(&["schemas", "PetList"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("id"), "Expected id field from Pet via allOf");
    assert!(
        stdout.contains("tags"),
        "Expected tags field from PetList"
    );
}

#[test]
fn test_schema_oneof() {
    let (stdout, _stderr, code) = run_with_petstore(&["schemas", "PetOrOwner"]);
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
    let (_stdout, stderr, code) = run_with_petstore(&["schemas", "NotReal"]);
    assert_eq!(code, 1, "Expected exit code 1 for not found");
    assert!(
        stderr.contains("not found"),
        "Expected 'not found' in stderr"
    );
}

// ─── Task 14.4: Auth and Search ───

#[test]
fn test_auth() {
    let (stdout, _stderr, code) = run_with_petstore(&["auth"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("bearerAuth"),
        "Missing bearerAuth scheme name"
    );
    assert!(
        stdout.to_lowercase().contains("http"),
        "Missing http type"
    );
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
    let (_stdout, stderr, code) = run_with_petstore(&["resources", "pets", "DELETE", "/nonexistent"]);
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
        .args(["--spec", &spec_path])
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
fn test_spec_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
        .current_dir(dir.path())
        .output()
        .expect("failed to run binary");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    assert_eq!(code, 1, "Expected exit code 1 when no spec found");
    assert!(
        stderr.to_lowercase().contains("not found")
            || stderr.to_lowercase().contains("no spec")
            || stderr.to_lowercase().contains("no openapi"),
        "Expected error about missing spec. Got: {}",
        stderr
    );
}

#[test]
fn test_invalid_spec() {
    let (_stdout, stderr, code) = run(&["--spec", "/dev/null"]);
    assert_eq!(code, 1, "Expected exit code 1 for invalid spec");
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
        vec!["--spec", &spec, "--json"],
        vec!["--spec", &spec, "--json", "resources"],
        vec!["--spec", &spec, "--json", "resources", "pets"],
        vec!["--spec", &spec, "--json", "resources", "pets", "GET", "/pets"],
        vec!["--spec", &spec, "--json", "schemas"],
        vec!["--spec", &spec, "--json", "schemas", "Pet"],
        vec!["--spec", &spec, "--json", "auth"],
        vec!["--spec", &spec, "--json", "search", "pet"],
    ];

    for cmd_args in &commands {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
            .args(cmd_args)
            .output()
            .expect("failed to run binary");
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let code = output.status.code().unwrap_or(-1);

        assert_eq!(
            code, 0,
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
    let (_stdout, stderr, code) = run_with_petstore(&["--json", "resources", "nonexistentresource"]);
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
    let (stdout, _stderr, code) = run_with_petstore(&["--json", "resources"]);
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
        .args(["--spec", &spec])
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
    let (stdout, _stderr, code) = run_with_petstore_env(&["resources"], &[("NO_COLOR", "1")]);
    assert_eq!(code, 0);
    // Resource names must still appear
    assert!(stdout.contains("pets"), "Resource names must appear with NO_COLOR. Got: {}", &stdout[..200.min(stdout.len())]);
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
    let (stdout, _stderr, code) = run_with_petstore_env(&["resources"], &[("TERM", "dumb")]);
    assert_eq!(code, 0);
    assert!(stdout.contains("pets"), "Resource names must appear with TERM=dumb. Got: {}", &stdout[..200.min(stdout.len())]);
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
    let (stdout, _stderr, code) = run_with_petstore(&["resources"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("pets"), "Resource names must appear in piped output. Got: {}", &stdout[..200.min(stdout.len())]);
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
        .args(["init", "--spec-path", &spec])
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
        ".phyllotaxis.yaml must contain the spec path. Got:\n{}",
        written
    );
}

#[test]
fn test_init_non_interactive_missing_spec_exits_1() {
    let dir = tempfile::tempdir().unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_phyllotaxis"))
        .current_dir(dir.path())
        .args(["init", "--spec-path", "nonexistent/path.yaml"])
        .stdin(std::process::Stdio::null())
        .output()
        .expect("failed to run phyllotaxis");

    let code = output.status.code().unwrap_or(-1);
    assert_eq!(
        code, 1,
        "Expected exit code 1 when spec-path does not exist"
    );
}

// ─── Task 3.7: Accessible arrow labels ───

#[test]
fn test_non_tty_no_unicode_arrow_in_response_schema() {
    // Non-TTY output (piped, which is what Command::output gives us) must not contain
    // the Unicode → character. It should use ASCII -> instead.
    // POST /pets returns 201 Created with schema ref Pet, so → appears in responses.
    let (stdout, _stderr, code) = run_with_petstore(&["resources", "pets", "POST", "/pets"]);
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
    let (stdout, _stderr, code) = run_with_petstore(&["schemas", "PetOrOwner"]);
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
    assert!(!stdout.is_empty(), "Bash completion output must not be empty");
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
    assert!(!stdout.is_empty(), "Zsh completion output must not be empty");
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
    assert!(!stdout.is_empty(), "Fish completion output must not be empty");
    assert!(
        stdout.contains("phyllotaxis"),
        "Fish completion script must reference the binary name. Got: {}",
        &stdout[..200.min(stdout.len())]
    );
}
