use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

#[derive(Debug, serde::Deserialize, Default)]
pub struct Config {
    /// Single document path (backward compat). Ignored if `documents` is present.
    pub document: Option<String>,
    /// Named documents map: name → relative path
    #[serde(default)]
    pub documents: HashMap<String, String>,
    /// Default document name to use when `documents` is present and no --doc flag is given
    pub default: Option<String>,
    #[serde(default)]
    pub variables: Option<HashMap<String, String>>,
}

/// Walk up from `start_dir` looking for `.phyllotaxis.yaml`.
/// Returns `(config, directory_containing_config)` if found.
pub fn load_config(start_dir: &Path) -> Option<(Config, PathBuf)> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let config_path = dir.join(".phyllotaxis.yaml");
        if config_path.is_file() {
            let content = match std::fs::read_to_string(&config_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Warning: could not read {}: {}", config_path.display(), e);
                    return None;
                }
            };
            match serde_yaml_ng::from_str::<Config>(&content) {
                Ok(config) => return Some((config, dir)),
                Err(e) => {
                    eprintln!("Warning: could not parse {}: {}", config_path.display(), e);
                    return None;
                }
            }
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Resolve the document file path using the priority chain:
/// 1. `--doc <name>` — look up in `documents` map in config
/// 2. `--doc <path>` — treat as file path (resolve relative to cwd)
/// 3. Config `default` → resolve from `documents` map
/// 4. Config single `document` field (backward compat)
/// 5. Auto-detect in start_dir (files containing "openapi:" in first 200 bytes)
/// 6. Error with helpful message
pub fn resolve_doc_path(
    doc_flag: Option<&str>,
    config: &Option<(Config, PathBuf)>,
    start_dir: &Path,
) -> Result<PathBuf> {
    // Helper: resolve a document path string relative to config_dir
    let resolve_named = |name: &str, config_dir: &Path| -> Option<PathBuf> {
        let path = PathBuf::from(name);
        let resolved = if path.is_absolute() {
            path
        } else {
            config_dir.join(name)
        };
        if resolved.is_file() {
            Some(resolved)
        } else {
            None
        }
    };

    // 1 & 2. --doc flag
    if let Some(doc) = doc_flag {
        // Try as a named document in the documents map first
        if let Some((cfg, config_dir)) = config {
            if let Some(named_path) = cfg.documents.get(doc) {
                if let Some(resolved) = resolve_named(named_path, config_dir) {
                    return Ok(resolved);
                }
                bail!(
                    "Named document '{}' points to '{}' which was not found (resolved from {})",
                    doc,
                    named_path,
                    config_dir.display()
                );
            }
        }

        // Fall back to treating as a file path
        let path = PathBuf::from(doc);
        let resolved = if path.is_absolute() {
            path
        } else {
            start_dir.join(path)
        };
        if resolved.is_file() {
            return Ok(resolved);
        }
        bail!(
            "Document '{}' not found as a named document or file path.",
            doc
        );
    }

    // 2b. PHYLLOTAXIS_DOCUMENT env var
    if let Ok(env_doc) = std::env::var("PHYLLOTAXIS_DOCUMENT") {
        if !env_doc.is_empty() {
            let path = PathBuf::from(&env_doc);
            let resolved = if path.is_absolute() {
                path
            } else {
                start_dir.join(path)
            };
            if resolved.is_file() {
                return Ok(resolved);
            }
            bail!(
                "PHYLLOTAXIS_DOCUMENT='{}' was set but the file was not found.",
                env_doc
            );
        }
    }

    // 3. Config default from documents map
    if let Some((cfg, config_dir)) = config {
        if !cfg.documents.is_empty() {
            let default_name = cfg.default.as_deref().unwrap_or_default();
            if let Some(named_path) = cfg.documents.get(default_name) {
                if let Some(resolved) = resolve_named(named_path, config_dir) {
                    return Ok(resolved);
                }
            }
            // No default set or default not found — error if multiple documents exist
            if cfg.default.is_none() {
                let names: Vec<&str> = cfg.documents.keys().map(String::as_str).collect();
                bail!(
                    "Multiple documents configured but no default set. Use --doc <name>.\n\
                     Available: {}",
                    names.join(", ")
                );
            }
            bail!(
                "Default document '{}' not found in documents map.",
                default_name
            );
        }

        // 4. Backward compat: single `document` field
        if let Some(doc) = &cfg.document {
            let path = PathBuf::from(doc);
            let resolved = if path.is_absolute() {
                path
            } else {
                config_dir.join(doc)
            };
            if resolved.is_file() {
                return Ok(resolved);
            }
            bail!(
                "Document from config not found: {} (resolved from {})",
                resolved.display(),
                config_dir.display()
            );
        }
    }

    // 5. Auto-detect
    if let Some(found) = auto_detect_document(start_dir) {
        return Ok(found);
    }

    // 6. Error
    bail!(
        "No OpenAPI document found. Tried:\n\
         1. --doc flag (not provided)\n\
         2. .phyllotaxis.yaml config ({})\n\
         3. Auto-detect in {} (no openapi files found)\n\n\
         Run 'phyllotaxis init' to set up, or use --doc <path>.",
        if config.is_some() {
            "found, no document configured"
        } else {
            "not found"
        },
        start_dir.display(),
    )
}

#[derive(Debug)]
pub struct LoadedDocument {
    pub api: openapiv3::OpenAPI,
    pub config: Config,
}

/// Load and parse an OpenAPI document. Resolves the document path, reads the file,
/// and parses it as YAML (falling back to JSON).
pub fn load_document(doc_flag: Option<&str>, start_dir: &Path) -> Result<LoadedDocument> {
    let config_result = load_config(start_dir);
    let spec_path = resolve_doc_path(doc_flag, &config_result, start_dir)?;

    // Guard against accidentally huge document files (100 MB limit)
    let metadata = std::fs::metadata(&spec_path)
        .with_context(|| format!("Failed to stat {}", spec_path.display()))?;
    const MAX_DOC_SIZE: u64 = 100 * 1024 * 1024;
    if metadata.len() > MAX_DOC_SIZE {
        bail!(
            "Document {} is too large ({:.1} MB, max 100 MB).",
            spec_path.display(),
            metadata.len() as f64 / (1024.0 * 1024.0)
        );
    }

    let content = std::fs::read_to_string(&spec_path)
        .with_context(|| format!("Failed to read {}", spec_path.display()))?;

    // Pass 1: parse to untyped Value (YAML first, then JSON)
    let mut value: serde_json::Value = serde_yaml_ng::from_str(&content)
        .or_else(|_| serde_json::from_str::<serde_json::Value>(&content))
        .with_context(|| format!("Failed to parse {}", spec_path.display()))?;

    // Pass 2: resolve all external $ref pointers in-place
    let base_dir = spec_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine parent dir of {}", spec_path.display()))?;
    bundle_refs(&mut value, base_dir, &mut vec![])
        .with_context(|| format!("Failed to bundle $refs in {}", spec_path.display()))?;

    // Pass 3: convert fully-resolved Value into the typed OpenAPI struct
    let api: openapiv3::OpenAPI = serde_json::from_value(value)
        .with_context(|| format!("Failed to parse {}", spec_path.display()))?;

    let config = config_result.map(|(c, _)| c).unwrap_or_default();

    Ok(LoadedDocument { api, config })
}

/// Search for OpenAPI document files by peeking at file contents.
fn auto_detect_document(dir: &Path) -> Option<PathBuf> {
    let candidates = [
        "openapi.yaml",
        "openapi.yml",
        "openapi.json",
        "swagger.yaml",
        "swagger.yml",
        "swagger.json",
    ];

    // Check common names first
    for name in &candidates {
        let path = dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }

    // Broader search: check yaml/json files in dir for "openapi:" header
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return None,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "yaml" | "yml" | "json") {
            continue;
        }
        // Peek at first 200 bytes (bounded read — avoids loading multi-GB files)
        if let Ok(mut file) = std::fs::File::open(&path) {
            use std::io::Read;
            let mut buf = [0u8; 200];
            let n = file.read(&mut buf).unwrap_or(0);
            let peek = String::from_utf8_lossy(&buf[..n]);
            if peek.contains("openapi:") || peek.contains("\"openapi\"") {
                return Some(path);
            }
        }
    }

    None
}

/// Navigate a `serde_json::Value` using an RFC 6901 JSON Pointer.
///
/// An empty pointer returns the value itself. Each `/`-delimited segment
/// is decoded (`~1` → `/`, `~0` → `~`) before lookup. Returns `None`
/// if any segment is missing or if an intermediate value is not an object.
fn json_pointer_get<'a>(
    value: &'a serde_json::Value,
    pointer: &str,
) -> Option<&'a serde_json::Value> {
    if pointer.is_empty() {
        return Some(value);
    }
    let mut current = value;
    for segment in pointer.split('/').skip(1) {
        let key = segment.replace("~1", "/").replace("~0", "~");
        match current {
            serde_json::Value::Object(map) => {
                current = map.get(&key)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

/// Split an external `$ref` string into `(file_path, optional_fragment)`.
///
/// The fragment, if present, is an RFC 6901 JSON Pointer (starts with `/`).
/// A trailing `#` with no content returns `None` for the fragment.
fn parse_external_ref(ref_str: &str) -> (&str, Option<String>) {
    match ref_str.split_once('#') {
        None => (ref_str, None),
        Some((path, fragment)) => {
            let frag = if fragment.is_empty() {
                None
            } else {
                Some(fragment.to_string())
            };
            (path, frag)
        }
    }
}

/// Recursively walk `value`, resolving all external `$ref` pointers in-place.
///
/// - Local refs (`#/...`) are left untouched — openapiv3 handles them.
/// - External refs to YAML/JSON files are resolved and inlined.
/// - External refs to other file types (`.cs`, `.php`, etc.) are left as-is.
/// - Circular refs are converted to local `#/` refs pointing to where the
///   file was first inlined, rather than erroring. This handles recursive
///   schemas (e.g., a `User` schema with a self-referencing property).
pub fn bundle_refs(
    value: &mut serde_json::Value,
    base_dir: &Path,
    visited: &mut Vec<PathBuf>,
) -> Result<()> {
    bundle_refs_impl(value, base_dir, visited, &mut HashMap::new(), "")
}

/// Internal implementation that tracks document position and file locations
/// for cycle-to-local-ref conversion.
fn bundle_refs_impl(
    value: &mut serde_json::Value,
    base_dir: &Path,
    visited: &mut Vec<PathBuf>,
    file_locations: &mut HashMap<PathBuf, String>,
    current_pointer: &str,
) -> Result<()> {
    match value {
        serde_json::Value::Object(map) => {
            // Check if this object IS a $ref
            if let Some(serde_json::Value::String(ref_str)) = map.get("$ref").cloned() {
                // Local ref — leave it for openapiv3 to resolve
                if ref_str.starts_with('#') {
                    return Ok(());
                }

                // External ref — resolve it
                let (file_part, fragment) = parse_external_ref(&ref_str);

                // Only resolve refs to YAML/JSON files; leave others as-is
                // (e.g., code samples like .cs, .php in vendor extensions)
                let ext = Path::new(file_part)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                if !matches!(ext, "yaml" | "yml" | "json") {
                    return Ok(());
                }

                let file_path = base_dir.join(file_part);
                let canonical = file_path.canonicalize().with_context(|| {
                    format!(
                        "Failed to resolve $ref '{}': file not found at {} (referenced from {})",
                        ref_str,
                        file_path.display(),
                        base_dir.display()
                    )
                })?;

                // Cycle check — convert to local ref instead of erroring
                if visited.contains(&canonical) {
                    if let Some(inline_pointer) = file_locations.get(&canonical) {
                        // Convert to local ref pointing to where the file was first inlined
                        *value = serde_json::json!({"$ref": format!("#{}", inline_pointer)});
                    }
                    // If no known location yet (shouldn't happen), leave as-is
                    return Ok(());
                }

                // Load external file
                let content = std::fs::read_to_string(&canonical).with_context(|| {
                    format!(
                        "Failed to read $ref target {} (referenced from {})",
                        canonical.display(),
                        base_dir.display()
                    )
                })?;
                let mut external: serde_json::Value = serde_yaml_ng::from_str(&content)
                    .or_else(|_| serde_json::from_str::<serde_json::Value>(&content))
                    .with_context(|| {
                        format!(
                            "Failed to parse {} (referenced from {})",
                            canonical.display(),
                            base_dir.display()
                        )
                    })?;

                // Record where this file is being inlined BEFORE recursing
                // (needed for self-referencing schemas like User → User)
                file_locations.insert(canonical.clone(), current_pointer.to_string());

                // Recursively bundle the loaded file (it may have its own external refs)
                let ext_dir = canonical.parent().ok_or_else(|| {
                    anyhow::anyhow!("Cannot determine parent dir of {}", canonical.display())
                })?;
                visited.push(canonical.clone());
                bundle_refs_impl(
                    &mut external,
                    ext_dir,
                    visited,
                    file_locations,
                    current_pointer,
                )?;
                visited.pop();

                // Navigate to fragment if present
                let resolved = if let Some(ref frag) = fragment {
                    json_pointer_get(&external, frag)
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "Invalid fragment in $ref '{}': pointer '{}' not found in {}",
                                ref_str,
                                frag,
                                canonical.display()
                            )
                        })?
                        .clone()
                } else {
                    external
                };

                // Replace the $ref object with the resolved content
                *value = resolved;
                return Ok(());
            }

            // Not a $ref — recurse into all values, tracking position
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in keys {
                let child_pointer = format!("{}/{}", current_pointer, key);
                if let Some(v) = map.get_mut(&key) {
                    bundle_refs_impl(v, base_dir, visited, file_locations, &child_pointer)?;
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for (i, v) in arr.iter_mut().enumerate() {
                let child_pointer = format!("{}/{}", current_pointer, i);
                bundle_refs_impl(v, base_dir, visited, file_locations, &child_pointer)?;
            }
        }
        // Primitives — nothing to do
        _ => {}
    }
    Ok(())
}

/// Extracts the schema name from a $ref string like "#/components/schemas/Pet".
pub fn schema_name_from_ref(reference: &str) -> Option<&str> {
    let name = reference.strip_prefix("#/components/schemas/")?;
    if !name.is_empty() && !name.contains('/') {
        Some(name)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;

    #[test]
    fn test_load_config_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let result = load_config(tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_load_config_found() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "document: ./openapi.yaml\n",
        )
        .unwrap();

        let (config, config_dir) = load_config(tmp.path()).expect("should find config");
        assert_eq!(config.document.as_deref(), Some("./openapi.yaml"));
        assert_eq!(config_dir, tmp.path());
    }

    #[test]
    fn test_load_config_with_variables() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "document: ./openapi.yaml\nvariables:\n  tenant: acme-corp\n",
        )
        .unwrap();

        let (config, _) = load_config(tmp.path()).expect("should find config");
        let vars = config.variables.as_ref().unwrap();
        assert_eq!(vars.get("tenant").unwrap(), "acme-corp");
    }

    #[test]
    fn test_resolve_prefers_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("my-spec.yaml");
        fs::write(
            &spec_path,
            "openapi: \"3.0.0\"\ninfo:\n  title: Test\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();

        // Also write a config pointing to a different file
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "document: ./other-doc.yaml\n",
        )
        .unwrap();

        let config = load_config(tmp.path());
        let result = resolve_doc_path(Some(spec_path.to_str().unwrap()), &config, tmp.path());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    fn test_resolve_uses_config() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("openapi.yaml");
        fs::write(
            &spec_path,
            "openapi: \"3.0.0\"\ninfo:\n  title: Test\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "document: ./openapi.yaml\n",
        )
        .unwrap();

        let config = load_config(tmp.path());
        let result = resolve_doc_path(None, &config, tmp.path());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    fn test_resolve_autodetect() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("openapi.yaml");
        fs::write(
            &spec_path,
            "openapi: \"3.0.0\"\ninfo:\n  title: Test\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();

        let result = resolve_doc_path(None, &None, tmp.path());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    fn test_resolve_error_when_nothing_found() {
        let tmp = tempfile::tempdir().unwrap();
        let result = resolve_doc_path(None, &None, tmp.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No OpenAPI document found"), "Error: {}", err);
    }

    #[test]
    fn test_load_config_walks_up() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub").join("deep");
        fs::create_dir_all(&sub).unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "document: ./openapi.yaml\n",
        )
        .unwrap();

        let (config, config_dir) = load_config(&sub).expect("should find config by walking up");
        assert_eq!(config.document.as_deref(), Some("./openapi.yaml"));
        assert_eq!(config_dir, tmp.path());
    }

    #[test]
    fn test_parse_petstore() {
        let result = load_document(
            Some("tests/fixtures/petstore.yaml"),
            std::path::Path::new("."),
        );
        let loaded = result.expect("should parse petstore fixture");
        assert_eq!(loaded.api.info.title, "Petstore API");
        assert_eq!(loaded.api.info.version, "1.0.0");
    }

    #[test]
    fn test_parse_bad_yaml() {
        let tmp = tempfile::tempdir().unwrap();
        let bad_path = tmp.path().join("bad.yaml");
        fs::write(&bad_path, "this is not valid openapi yaml {{{").unwrap();

        let result = load_document(Some(bad_path.to_str().unwrap()), tmp.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Failed to parse"), "Error: {}", err);
    }

    #[test]
    fn test_schema_name_from_ref() {
        assert_eq!(
            schema_name_from_ref("#/components/schemas/Pet"),
            Some("Pet")
        );
        assert_eq!(
            schema_name_from_ref("#/components/schemas/PetList"),
            Some("PetList")
        );
    }

    #[test]
    fn test_schema_name_invalid() {
        assert_eq!(schema_name_from_ref("#/components/other/Pet"), None);
        assert_eq!(schema_name_from_ref("#/definitions/Pet"), None);
        assert_eq!(schema_name_from_ref(""), None);
    }

    #[test]
    fn test_resolve_named_spec_by_name() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("public.yaml");
        fs::write(
            &spec_path,
            "openapi: \"3.0.0\"\ninfo:\n  title: Public\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "documents:\n  public: ./public.yaml\ndefault: public\n",
        )
        .unwrap();

        let config = load_config(tmp.path());
        let result = resolve_doc_path(Some("public"), &config, tmp.path());
        assert!(
            result.is_ok(),
            "Should resolve named document: {:?}",
            result
        );
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    fn test_resolve_uses_default_from_specs() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("public.yaml");
        fs::write(
            &spec_path,
            "openapi: \"3.0.0\"\ninfo:\n  title: Public\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "documents:\n  public: ./public.yaml\ndefault: public\n",
        )
        .unwrap();

        let config = load_config(tmp.path());
        // No --spec flag: should use default
        let result = resolve_doc_path(None, &config, tmp.path());
        assert!(result.is_ok(), "Should use default document: {:?}", result);
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    fn test_resolve_errors_on_multi_spec_no_default() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_a = tmp.path().join("a.yaml");
        let spec_b = tmp.path().join("b.yaml");
        fs::write(
            &spec_a,
            "openapi: \"3.0.0\"\ninfo:\n  title: A\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();
        fs::write(
            &spec_b,
            "openapi: \"3.0.0\"\ninfo:\n  title: B\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "documents:\n  a: ./a.yaml\n  b: ./b.yaml\n",
        )
        .unwrap();

        let config = load_config(tmp.path());
        let result = resolve_doc_path(None, &config, tmp.path());
        assert!(
            result.is_err(),
            "Should error when multiple documents and no default"
        );
        assert!(
            result.unwrap_err().to_string().contains("--doc"),
            "Error should mention --doc"
        );
    }

    #[test]
    fn test_backward_compat_single_spec_field() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("api.yaml");
        fs::write(
            &spec_path,
            "openapi: \"3.0.0\"\ninfo:\n  title: API\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "document: ./api.yaml\n",
        )
        .unwrap();

        let config = load_config(tmp.path());
        let result = resolve_doc_path(None, &config, tmp.path());
        assert!(
            result.is_ok(),
            "Single document: field should still work: {:?}",
            result
        );
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    #[serial]
    fn test_resolve_uses_env_var_when_no_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("env-spec.yaml");
        fs::write(
            &spec_path,
            "openapi: \"3.0.0\"\ninfo:\n  title: Env\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();

        unsafe { std::env::set_var("PHYLLOTAXIS_DOCUMENT", spec_path.to_str().unwrap()) };
        let result = resolve_doc_path(None, &None, tmp.path());
        unsafe { std::env::remove_var("PHYLLOTAXIS_DOCUMENT") };

        assert!(result.is_ok(), "Env var should resolve: {:?}", result);
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    #[serial]
    fn test_resolve_flag_wins_over_env_var() {
        let tmp = tempfile::tempdir().unwrap();
        let flag_spec = tmp.path().join("flag-spec.yaml");
        let env_spec = tmp.path().join("env-spec.yaml");
        fs::write(
            &flag_spec,
            "openapi: \"3.0.0\"\ninfo:\n  title: Flag\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();
        fs::write(
            &env_spec,
            "openapi: \"3.0.0\"\ninfo:\n  title: Env\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();

        unsafe { std::env::set_var("PHYLLOTAXIS_DOCUMENT", env_spec.to_str().unwrap()) };
        let result = resolve_doc_path(Some(flag_spec.to_str().unwrap()), &None, tmp.path());
        unsafe { std::env::remove_var("PHYLLOTAXIS_DOCUMENT") };

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), flag_spec, "Flag should win over env var");
    }

    #[test]
    #[serial]
    fn test_resolve_env_var_wins_over_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config_spec = tmp.path().join("config-spec.yaml");
        let env_spec = tmp.path().join("env-spec.yaml");
        fs::write(
            &config_spec,
            "openapi: \"3.0.0\"\ninfo:\n  title: Config\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();
        fs::write(
            &env_spec,
            "openapi: \"3.0.0\"\ninfo:\n  title: Env\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "document: ./config-doc.yaml\n",
        )
        .unwrap();

        let config = load_config(tmp.path());
        unsafe { std::env::set_var("PHYLLOTAXIS_DOCUMENT", env_spec.to_str().unwrap()) };
        let result = resolve_doc_path(None, &config, tmp.path());
        unsafe { std::env::remove_var("PHYLLOTAXIS_DOCUMENT") };

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), env_spec, "Env var should win over config");
    }

    #[test]
    #[serial]
    fn test_resolve_env_var_not_found_is_error() {
        let tmp = tempfile::tempdir().unwrap();

        unsafe { std::env::set_var("PHYLLOTAXIS_DOCUMENT", "/nonexistent/path.yaml") };
        let result = resolve_doc_path(None, &None, tmp.path());
        unsafe { std::env::remove_var("PHYLLOTAXIS_DOCUMENT") };

        assert!(
            result.is_err(),
            "Should error when env var points to missing file"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("PHYLLOTAXIS_DOCUMENT"),
            "Error should mention PHYLLOTAXIS_SPEC"
        );
    }

    #[test]
    #[serial]
    fn test_resolve_env_var_empty_falls_through() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("openapi.yaml");
        fs::write(
            &spec_path,
            "openapi: \"3.0.0\"\ninfo:\n  title: Auto\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();

        unsafe { std::env::set_var("PHYLLOTAXIS_DOCUMENT", "") };
        let result = resolve_doc_path(None, &None, tmp.path());
        unsafe { std::env::remove_var("PHYLLOTAXIS_DOCUMENT") };

        assert!(
            result.is_ok(),
            "Empty env var should fall through to auto-detect"
        );
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    fn test_parse_external_ref_bare_file() {
        let (path, fragment) = parse_external_ref("./schemas/pet.yaml");
        assert_eq!(path, "./schemas/pet.yaml");
        assert_eq!(fragment, None);
    }

    #[test]
    fn test_parse_external_ref_with_fragment() {
        let (path, fragment) = parse_external_ref("./schemas.yaml#/components/schemas/Pet");
        assert_eq!(path, "./schemas.yaml");
        assert_eq!(fragment.as_deref(), Some("/components/schemas/Pet"));
    }

    #[test]
    fn test_parse_external_ref_empty_fragment() {
        let (path, fragment) = parse_external_ref("./file.yaml#");
        assert_eq!(path, "./file.yaml");
        assert_eq!(fragment, None);
    }

    #[test]
    fn test_parse_external_ref_absolute_path_with_fragment() {
        let (path, fragment) = parse_external_ref("/abs/path/schema.yaml#/Foo");
        assert_eq!(path, "/abs/path/schema.yaml");
        assert_eq!(fragment.as_deref(), Some("/Foo"));
    }

    #[test]
    fn test_parse_external_ref_no_dot_prefix() {
        let (path, fragment) = parse_external_ref("schemas/pet.yaml");
        assert_eq!(path, "schemas/pet.yaml");
        assert_eq!(fragment, None);
    }

    #[test]
    fn test_json_pointer_root() {
        let val = serde_json::json!({"a": 1});
        assert_eq!(json_pointer_get(&val, ""), Some(&val));
    }

    #[test]
    fn test_json_pointer_simple() {
        let val = serde_json::json!({"components": {"schemas": {"Pet": {"type": "object"}}}});
        let result = json_pointer_get(&val, "/components/schemas/Pet");
        assert_eq!(result, Some(&serde_json::json!({"type": "object"})));
    }

    #[test]
    fn test_json_pointer_missing_key() {
        let val = serde_json::json!({"a": 1});
        assert_eq!(json_pointer_get(&val, "/b"), None);
    }

    #[test]
    fn test_json_pointer_escape_tilde1() {
        // ~1 decodes to /
        let val = serde_json::json!({"a/b": 42});
        assert_eq!(
            json_pointer_get(&val, "/a~1b"),
            Some(&serde_json::json!(42))
        );
    }

    #[test]
    fn test_json_pointer_escape_tilde0() {
        // ~0 decodes to ~
        let val = serde_json::json!({"a~b": 99});
        assert_eq!(
            json_pointer_get(&val, "/a~0b"),
            Some(&serde_json::json!(99))
        );
    }

    #[test]
    fn test_json_pointer_intermediate_not_object() {
        // Navigating through a non-object returns None
        let val = serde_json::json!({"a": "not_an_object"});
        assert_eq!(json_pointer_get(&val, "/a/b"), None);
    }
}
