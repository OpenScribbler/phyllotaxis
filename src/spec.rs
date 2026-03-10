use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

#[derive(Debug, serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct Config {
    /// Active document nickname (used when no --doc flag is given)
    pub active: Option<String>,
    /// Named documents map: nickname → relative path
    #[serde(default)]
    pub documents: HashMap<String, String>,
    #[serde(default)]
    pub variables: Option<HashMap<String, String>>,
}

/// Holds configs from both scopes. Each scope is optional — project config requires
/// a `.phyllotaxis/` directory above cwd; user config requires `~/.config/phyllotaxis/`.
#[derive(Debug, Default)]
pub struct ScopedConfig {
    /// Project config + the directory containing `.phyllotaxis/`
    pub project: Option<(Config, PathBuf)>,
    /// User-scope config (no associated directory — paths are absolute in user config)
    pub user: Option<Config>,
}

/// Walk up from `start_dir` looking for `.phyllotaxis/config.yaml`.
/// Also loads user-scope config from `~/.config/phyllotaxis/config.yaml`.
pub fn load_config(start_dir: &Path) -> ScopedConfig {
    let project = find_project_config(start_dir);
    let user = load_user_config();
    ScopedConfig { project, user }
}

fn find_project_config(start_dir: &Path) -> Option<(Config, PathBuf)> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let config_path = dir.join(".phyllotaxis").join("config.yaml");
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

fn load_user_config() -> Option<Config> {
    let config_path = user_config_path()?;
    if !config_path.is_file() {
        return None;
    }
    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: could not read {}: {}", config_path.display(), e);
            return None;
        }
    };
    match serde_yaml_ng::from_str::<Config>(&content) {
        Ok(config) => Some(config),
        Err(e) => {
            eprintln!("Warning: could not parse {}: {}", config_path.display(), e);
            None
        }
    }
}

/// Returns `~/.config/phyllotaxis/config.yaml`, or None if home dir cannot be determined.
pub fn user_config_path() -> Option<PathBuf> {
    dirs_next::home_dir().map(|h| h.join(".config").join("phyllotaxis").join("config.yaml"))
}

/// Returns `.phyllotaxis/config.yaml` relative to the given project root.
pub fn project_config_path(project_root: &Path) -> PathBuf {
    project_root.join(".phyllotaxis").join("config.yaml")
}

/// Resolve the document file path using the priority chain:
///
/// 1. `--doc <name>` — look up in project docs first, then user docs, then treat as file path
/// 2. `PHYLLOTAXIS_DOCUMENT` env var
/// 3. Project active doc
/// 4. User active doc
/// 5. Auto-detect in start_dir
/// 6. Error
pub fn resolve_doc_path(
    doc_flag: Option<&str>,
    scoped: &ScopedConfig,
    start_dir: &Path,
) -> Result<PathBuf> {
    // 1. --doc flag
    if let Some(doc) = doc_flag {
        // Try project docs first
        if let Some((cfg, root)) = &scoped.project {
            if let Some(rel) = cfg.documents.get(doc) {
                return resolve_relative(rel, root).ok_or_else(|| {
                    anyhow::anyhow!(
                        "Named document '{}' → '{}' not found (from {})",
                        doc,
                        rel,
                        root.display()
                    )
                });
            }
        }
        // Try user docs next
        if let Some(cfg) = &scoped.user {
            if let Some(path_str) = cfg.documents.get(doc) {
                let p = PathBuf::from(path_str);
                if p.is_file() {
                    return Ok(p);
                }
                bail!(
                    "Named document '{}' → '{}' not found in user config",
                    doc,
                    path_str
                );
            }
        }
        // Fall back to literal file path
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

    // 2. PHYLLOTAXIS_DOCUMENT env var
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

    // 3. Project active doc
    if let Some((cfg, root)) = &scoped.project {
        if let Some(active) = &cfg.active {
            if let Some(rel) = cfg.documents.get(active.as_str()) {
                if let Some(p) = resolve_relative(rel, root) {
                    return Ok(p);
                }
                bail!(
                    "Active document '{}' → '{}' not found (from {})",
                    active,
                    rel,
                    root.display()
                );
            }
            bail!("Active document '{}' not found in project config.", active);
        }
        // Project config exists but no active — if there are docs, tell the user to pick
        if !cfg.documents.is_empty() {
            let names: Vec<&str> = cfg.documents.keys().map(String::as_str).collect();
            bail!(
                "Project has documents configured but no active set.\n\
                 Use: phyll --set-doc <name>  (available: {})",
                names.join(", ")
            );
        }
    }

    // 4. User active doc
    if let Some(cfg) = &scoped.user {
        if let Some(active) = &cfg.active {
            if let Some(path_str) = cfg.documents.get(active.as_str()) {
                let p = PathBuf::from(path_str);
                if p.is_file() {
                    return Ok(p);
                }
                bail!(
                    "Active user document '{}' → '{}' not found.",
                    active,
                    path_str
                );
            }
            bail!(
                "Active user document '{}' not found in user config.",
                active
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
         2. PHYLLOTAXIS_DOCUMENT env var (not set)\n\
         3. Project config ({})\n\
         4. User config (~/.config/phyllotaxis/config.yaml)\n\
         5. Auto-detect in {} (no openapi files found)\n\n\
         Add a document to get started:\n\
         \x20 phyll --add-doc ./path/to/openapi.yaml\n\
         \x20 phyll --doc ./path/to/openapi.yaml  (one-shot, no config needed)",
        if scoped.project.is_some() {
            "found, no active doc"
        } else {
            "not found"
        },
        start_dir.display(),
    )
}

/// Resolve a relative path string against a base directory.
/// Returns None if the resolved path is not a file.
fn resolve_relative(rel: &str, base: &Path) -> Option<PathBuf> {
    let p = PathBuf::from(rel);
    let resolved = if p.is_absolute() { p } else { base.join(p) };
    if resolved.is_file() {
        Some(resolved)
    } else {
        None
    }
}

#[derive(Debug)]
pub struct LoadedDocument {
    pub api: openapiv3::OpenAPI,
    pub config: Config,
}

/// Load and parse an OpenAPI document. Resolves the document path, reads the file,
/// and parses it as YAML (falling back to JSON).
pub fn load_document(doc_flag: Option<&str>, start_dir: &Path) -> Result<LoadedDocument> {
    let scoped = load_config(start_dir);
    let spec_path = resolve_doc_path(doc_flag, &scoped, start_dir)?;

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

    let config = scoped.project.map(|(c, _)| c).unwrap_or_default();

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
    fn test_config_struct_active_field() {
        let yaml = "active: pets\ndocuments:\n  pets: ./api/petstore.yaml\n";
        let config: Config = serde_yaml_ng::from_str(yaml).expect("should parse");
        assert_eq!(config.active.as_deref(), Some("pets"));
        assert_eq!(
            config.documents.get("pets").map(String::as_str),
            Some("./api/petstore.yaml")
        );
    }

    #[test]
    fn test_config_struct_rejects_old_document_field() {
        // Old single `document:` key must not silently round-trip as active
        let yaml = "document: ./openapi.yaml\n";
        let config: Config = serde_yaml_ng::from_str(yaml).unwrap_or_default();
        // The field doesn't exist in new struct — unknown fields are ignored by serde
        // What matters is that active is None and documents is empty
        assert!(config.active.is_none());
        assert!(config.documents.is_empty());
    }

    #[test]
    fn test_load_config_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let scoped = load_config(tmp.path());
        assert!(scoped.project.is_none(), "no project config in empty dir");
    }

    #[test]
    fn test_load_config_found() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join(".phyllotaxis");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("config.yaml"),
            "active: api\ndocuments:\n  api: ./openapi.yaml\n",
        )
        .unwrap();

        let scoped = load_config(tmp.path());
        let (config, root) = scoped.project.expect("should find project config");
        assert_eq!(config.active.as_deref(), Some("api"));
        assert!(config.documents.contains_key("api"));
        assert_eq!(root, tmp.path());
    }

    #[test]
    fn test_load_config_with_variables() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join(".phyllotaxis");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("config.yaml"),
            "active: api\ndocuments:\n  api: ./openapi.yaml\nvariables:\n  tenant: acme-corp\n",
        )
        .unwrap();

        let scoped = load_config(tmp.path());
        let (config, _) = scoped.project.expect("should find config");
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

        let scoped = ScopedConfig::default();
        let result = resolve_doc_path(Some(spec_path.to_str().unwrap()), &scoped, tmp.path());
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

        let mut cfg = Config::default();
        cfg.active = Some("api".to_string());
        cfg.documents
            .insert("api".to_string(), "./openapi.yaml".to_string());
        let scoped = ScopedConfig {
            project: Some((cfg, tmp.path().to_path_buf())),
            user: None,
        };

        let result = resolve_doc_path(None, &scoped, tmp.path());
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

        let scoped = ScopedConfig::default();
        let result = resolve_doc_path(None, &scoped, tmp.path());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    fn test_resolve_error_when_nothing_found() {
        let tmp = tempfile::tempdir().unwrap();
        let scoped = ScopedConfig::default();
        let result = resolve_doc_path(None, &scoped, tmp.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No OpenAPI document found"), "Error: {}", err);
    }

    #[test]
    fn test_load_config_walks_up() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub").join("deep");
        fs::create_dir_all(&sub).unwrap();
        let config_dir = tmp.path().join(".phyllotaxis");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("config.yaml"),
            "active: api\ndocuments:\n  api: ./openapi.yaml\n",
        )
        .unwrap();

        let scoped = load_config(&sub);
        let (config, root) = scoped.project.expect("should find config by walking up");
        assert_eq!(config.active.as_deref(), Some("api"));
        assert_eq!(root, tmp.path());
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
    fn test_resolve_named_document_by_name() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("public.yaml");
        fs::write(
            &spec_path,
            "openapi: \"3.0.0\"\ninfo:\n  title: Public\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();

        let mut cfg = Config::default();
        cfg.active = Some("public".to_string());
        cfg.documents
            .insert("public".to_string(), "./public.yaml".to_string());
        let scoped = ScopedConfig {
            project: Some((cfg, tmp.path().to_path_buf())),
            user: None,
        };

        let result = resolve_doc_path(Some("public"), &scoped, tmp.path());
        assert!(
            result.is_ok(),
            "Should resolve named document: {:?}",
            result
        );
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    fn test_resolve_uses_active_from_documents() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("public.yaml");
        fs::write(
            &spec_path,
            "openapi: \"3.0.0\"\ninfo:\n  title: Public\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();

        let mut cfg = Config::default();
        cfg.active = Some("public".to_string());
        cfg.documents
            .insert("public".to_string(), "./public.yaml".to_string());
        let scoped = ScopedConfig {
            project: Some((cfg, tmp.path().to_path_buf())),
            user: None,
        };

        let result = resolve_doc_path(None, &scoped, tmp.path());
        assert!(result.is_ok(), "Should use active document: {:?}", result);
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    fn test_resolve_errors_on_multi_document_no_active() {
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

        let mut cfg = Config::default();
        cfg.documents
            .insert("a".to_string(), "./a.yaml".to_string());
        cfg.documents
            .insert("b".to_string(), "./b.yaml".to_string());
        let scoped = ScopedConfig {
            project: Some((cfg, tmp.path().to_path_buf())),
            user: None,
        };

        let result = resolve_doc_path(None, &scoped, tmp.path());
        assert!(
            result.is_err(),
            "Should error when multiple documents and no active"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("--set-doc"),
            "Error should mention --set-doc: {}",
            err
        );
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
        let scoped = ScopedConfig::default();
        let result = resolve_doc_path(None, &scoped, tmp.path());
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
        let scoped = ScopedConfig::default();
        let result = resolve_doc_path(Some(flag_spec.to_str().unwrap()), &scoped, tmp.path());
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
        let mut cfg = Config::default();
        cfg.active = Some("config".to_string());
        cfg.documents
            .insert("config".to_string(), "./config-spec.yaml".to_string());
        let scoped = ScopedConfig {
            project: Some((cfg, tmp.path().to_path_buf())),
            user: None,
        };

        unsafe { std::env::set_var("PHYLLOTAXIS_DOCUMENT", env_spec.to_str().unwrap()) };
        let result = resolve_doc_path(None, &scoped, tmp.path());
        unsafe { std::env::remove_var("PHYLLOTAXIS_DOCUMENT") };

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), env_spec, "Env var should win over config");
    }

    #[test]
    #[serial]
    fn test_resolve_env_var_not_found_is_error() {
        let tmp = tempfile::tempdir().unwrap();

        unsafe { std::env::set_var("PHYLLOTAXIS_DOCUMENT", "/nonexistent/path.yaml") };
        let scoped = ScopedConfig::default();
        let result = resolve_doc_path(None, &scoped, tmp.path());
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
            "Error should mention PHYLLOTAXIS_DOCUMENT"
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
        let scoped = ScopedConfig::default();
        let result = resolve_doc_path(None, &scoped, tmp.path());
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
