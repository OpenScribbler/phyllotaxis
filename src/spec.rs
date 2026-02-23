use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

#[derive(Debug, serde::Deserialize, Default)]
pub struct Config {
    /// Single-spec path (backward compat). Ignored if `specs` is present.
    pub spec: Option<String>,
    /// Named specs map: name → relative path
    #[serde(default)]
    pub specs: HashMap<String, String>,
    /// Default spec name to use when `specs` is present and no --spec flag is given
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

/// Resolve the spec file path using the priority chain:
/// 1. `--spec <name>` — look up in `specs` map in config
/// 2. `--spec <path>` — treat as file path (resolve relative to cwd)
/// 3. Config `default` → resolve from `specs` map
/// 4. Config single `spec` field (backward compat)
/// 5. Auto-detect in start_dir (files containing "openapi:" in first 200 bytes)
/// 6. Error with helpful message
pub fn resolve_spec_path(
    spec_flag: Option<&str>,
    config: &Option<(Config, PathBuf)>,
    start_dir: &Path,
) -> Result<PathBuf> {
    // Helper: resolve a spec path string relative to config_dir
    let resolve_named = |name: &str, config_dir: &Path| -> Option<PathBuf> {
        let path = PathBuf::from(name);
        let resolved = if path.is_absolute() {
            path
        } else {
            config_dir.join(name)
        };
        if resolved.is_file() { Some(resolved) } else { None }
    };

    // 1 & 2. --spec flag
    if let Some(spec) = spec_flag {
        // Try as a named spec in the specs map first
        if let Some((cfg, config_dir)) = config {
            if let Some(named_path) = cfg.specs.get(spec) {
                if let Some(resolved) = resolve_named(named_path, config_dir) {
                    return Ok(resolved);
                }
                bail!(
                    "Named spec '{}' points to '{}' which was not found (resolved from {})",
                    spec,
                    named_path,
                    config_dir.display()
                );
            }
        }

        // Fall back to treating spec as a file path
        let path = PathBuf::from(spec);
        let resolved = if path.is_absolute() {
            path
        } else {
            start_dir.join(path)
        };
        if resolved.is_file() {
            return Ok(resolved);
        }
        bail!("Spec '{}' not found as a named spec or file path.", spec);
    }

    // 2b. PHYLLOTAXIS_SPEC env var
    if let Ok(env_spec) = std::env::var("PHYLLOTAXIS_SPEC") {
        if !env_spec.is_empty() {
            let path = PathBuf::from(&env_spec);
            let resolved = if path.is_absolute() {
                path
            } else {
                start_dir.join(path)
            };
            if resolved.is_file() {
                return Ok(resolved);
            }
            bail!(
                "PHYLLOTAXIS_SPEC='{}' was set but the file was not found.",
                env_spec
            );
        }
    }

    // 3. Config default from specs map
    if let Some((cfg, config_dir)) = config {
        if !cfg.specs.is_empty() {
            let default_name = cfg.default.as_deref().unwrap_or_default();
            if let Some(named_path) = cfg.specs.get(default_name) {
                if let Some(resolved) = resolve_named(named_path, config_dir) {
                    return Ok(resolved);
                }
            }
            // No default set or default not found — error if multiple specs exist
            if cfg.default.is_none() {
                let names: Vec<&str> = cfg.specs.keys().map(String::as_str).collect();
                bail!(
                    "Multiple specs configured but no default set. Use --spec <name>.\n\
                     Available: {}",
                    names.join(", ")
                );
            }
            bail!("Default spec '{}' not found in specs map.", default_name);
        }

        // 4. Backward compat: single `spec` field
        if let Some(spec) = &cfg.spec {
            let path = PathBuf::from(spec);
            let resolved = if path.is_absolute() {
                path
            } else {
                config_dir.join(spec)
            };
            if resolved.is_file() {
                return Ok(resolved);
            }
            bail!(
                "Spec file from config not found: {} (resolved from {})",
                resolved.display(),
                config_dir.display()
            );
        }
    }

    // 5. Auto-detect
    if let Some(found) = auto_detect_spec(start_dir) {
        return Ok(found);
    }

    // 6. Error
    bail!(
        "No OpenAPI spec found. Tried:\n\
         1. --spec flag (not provided)\n\
         2. .phyllotaxis.yaml config ({})\n\
         3. Auto-detect in {} (no openapi files found)\n\n\
         Run 'phyllotaxis init' to set up, or use --spec <path>.",
        if config.is_some() { "found, no spec configured" } else { "not found" },
        start_dir.display(),
    )
}

#[derive(Debug)]
pub struct LoadedSpec {
    pub api: openapiv3::OpenAPI,
    pub config: Config,
}

/// Load and parse an OpenAPI spec. Resolves the spec path, reads the file,
/// and parses it as YAML (falling back to JSON).
pub fn load_spec(spec_flag: Option<&str>, start_dir: &Path) -> Result<LoadedSpec> {
    let config_result = load_config(start_dir);
    let spec_path = resolve_spec_path(spec_flag, &config_result, start_dir)?;

    let content = std::fs::read_to_string(&spec_path)
        .with_context(|| format!("Failed to read {}", spec_path.display()))?;

    // Try YAML first, then JSON
    let api: openapiv3::OpenAPI = serde_yaml_ng::from_str(&content)
        .or_else(|_| serde_json::from_str(&content))
        .with_context(|| format!("Failed to parse {}", spec_path.display()))?;

    let config = config_result
        .map(|(c, _)| c)
        .unwrap_or_default();

    Ok(LoadedSpec { api, config })
}

/// Search for OpenAPI spec files by peeking at file contents.
fn auto_detect_spec(dir: &Path) -> Option<PathBuf> {
    let candidates = ["openapi.yaml", "openapi.yml", "openapi.json",
                      "swagger.yaml", "swagger.yml", "swagger.json"];

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
        // Peek at first 200 bytes
        if let Ok(content) = std::fs::read_to_string(&path) {
            let peek: String = content.chars().take(200).collect();
            if peek.contains("openapi:") || peek.contains("\"openapi\"") {
                return Some(path);
            }
        }
    }

    None
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
            "spec: ./openapi.yaml\n",
        )
        .unwrap();

        let (config, config_dir) = load_config(tmp.path()).expect("should find config");
        assert_eq!(config.spec.as_deref(), Some("./openapi.yaml"));
        assert_eq!(config_dir, tmp.path());
    }

    #[test]
    fn test_load_config_with_variables() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "spec: ./openapi.yaml\nvariables:\n  tenant: acme-corp\n",
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
        fs::write(&spec_path, "openapi: \"3.0.0\"\ninfo:\n  title: Test\n  version: \"1.0\"\npaths: {}\n").unwrap();

        // Also write a config pointing to a different file
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "spec: ./other-spec.yaml\n",
        ).unwrap();

        let config = load_config(tmp.path());
        let result = resolve_spec_path(
            Some(spec_path.to_str().unwrap()),
            &config,
            tmp.path(),
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    fn test_resolve_uses_config() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("openapi.yaml");
        fs::write(&spec_path, "openapi: \"3.0.0\"\ninfo:\n  title: Test\n  version: \"1.0\"\npaths: {}\n").unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "spec: ./openapi.yaml\n",
        ).unwrap();

        let config = load_config(tmp.path());
        let result = resolve_spec_path(None, &config, tmp.path());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    fn test_resolve_autodetect() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("openapi.yaml");
        fs::write(&spec_path, "openapi: \"3.0.0\"\ninfo:\n  title: Test\n  version: \"1.0\"\npaths: {}\n").unwrap();

        let result = resolve_spec_path(None, &None, tmp.path());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    fn test_resolve_error_when_nothing_found() {
        let tmp = tempfile::tempdir().unwrap();
        let result = resolve_spec_path(None, &None, tmp.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No OpenAPI spec found"), "Error: {}", err);
    }

    #[test]
    fn test_load_config_walks_up() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub").join("deep");
        fs::create_dir_all(&sub).unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "spec: ./openapi.yaml\n",
        )
        .unwrap();

        let (config, config_dir) = load_config(&sub).expect("should find config by walking up");
        assert_eq!(config.spec.as_deref(), Some("./openapi.yaml"));
        assert_eq!(config_dir, tmp.path());
    }

    #[test]
    fn test_parse_petstore() {
        let result = load_spec(
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

        let result = load_spec(
            Some(bad_path.to_str().unwrap()),
            tmp.path(),
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Failed to parse"), "Error: {}", err);
    }

    #[test]
    fn test_schema_name_from_ref() {
        assert_eq!(schema_name_from_ref("#/components/schemas/Pet"), Some("Pet"));
        assert_eq!(schema_name_from_ref("#/components/schemas/PetList"), Some("PetList"));
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
        fs::write(&spec_path, "openapi: \"3.0.0\"\ninfo:\n  title: Public\n  version: \"1.0\"\npaths: {}\n").unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "specs:\n  public: ./public.yaml\ndefault: public\n",
        ).unwrap();

        let config = load_config(tmp.path());
        let result = resolve_spec_path(Some("public"), &config, tmp.path());
        assert!(result.is_ok(), "Should resolve named spec: {:?}", result);
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    fn test_resolve_uses_default_from_specs() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("public.yaml");
        fs::write(&spec_path, "openapi: \"3.0.0\"\ninfo:\n  title: Public\n  version: \"1.0\"\npaths: {}\n").unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "specs:\n  public: ./public.yaml\ndefault: public\n",
        ).unwrap();

        let config = load_config(tmp.path());
        // No --spec flag: should use default
        let result = resolve_spec_path(None, &config, tmp.path());
        assert!(result.is_ok(), "Should use default spec: {:?}", result);
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    fn test_resolve_errors_on_multi_spec_no_default() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_a = tmp.path().join("a.yaml");
        let spec_b = tmp.path().join("b.yaml");
        fs::write(&spec_a, "openapi: \"3.0.0\"\ninfo:\n  title: A\n  version: \"1.0\"\npaths: {}\n").unwrap();
        fs::write(&spec_b, "openapi: \"3.0.0\"\ninfo:\n  title: B\n  version: \"1.0\"\npaths: {}\n").unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "specs:\n  a: ./a.yaml\n  b: ./b.yaml\n",
        ).unwrap();

        let config = load_config(tmp.path());
        let result = resolve_spec_path(None, &config, tmp.path());
        assert!(result.is_err(), "Should error when multiple specs and no default");
        assert!(result.unwrap_err().to_string().contains("--spec"), "Error should mention --spec");
    }

    #[test]
    fn test_backward_compat_single_spec_field() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("api.yaml");
        fs::write(&spec_path, "openapi: \"3.0.0\"\ninfo:\n  title: API\n  version: \"1.0\"\npaths: {}\n").unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "spec: ./api.yaml\n",
        ).unwrap();

        let config = load_config(tmp.path());
        let result = resolve_spec_path(None, &config, tmp.path());
        assert!(result.is_ok(), "Single spec: field should still work: {:?}", result);
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    fn test_resolve_uses_env_var_when_no_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("env-spec.yaml");
        fs::write(&spec_path, "openapi: \"3.0.0\"\ninfo:\n  title: Env\n  version: \"1.0\"\npaths: {}\n").unwrap();

        unsafe { std::env::set_var("PHYLLOTAXIS_SPEC", spec_path.to_str().unwrap()) };
        let result = resolve_spec_path(None, &None, tmp.path());
        unsafe { std::env::remove_var("PHYLLOTAXIS_SPEC") };

        assert!(result.is_ok(), "Env var should resolve: {:?}", result);
        assert_eq!(result.unwrap(), spec_path);
    }

    #[test]
    fn test_resolve_flag_wins_over_env_var() {
        let tmp = tempfile::tempdir().unwrap();
        let flag_spec = tmp.path().join("flag-spec.yaml");
        let env_spec = tmp.path().join("env-spec.yaml");
        fs::write(&flag_spec, "openapi: \"3.0.0\"\ninfo:\n  title: Flag\n  version: \"1.0\"\npaths: {}\n").unwrap();
        fs::write(&env_spec, "openapi: \"3.0.0\"\ninfo:\n  title: Env\n  version: \"1.0\"\npaths: {}\n").unwrap();

        unsafe { std::env::set_var("PHYLLOTAXIS_SPEC", env_spec.to_str().unwrap()) };
        let result = resolve_spec_path(Some(flag_spec.to_str().unwrap()), &None, tmp.path());
        unsafe { std::env::remove_var("PHYLLOTAXIS_SPEC") };

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), flag_spec, "Flag should win over env var");
    }

    #[test]
    fn test_resolve_env_var_wins_over_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config_spec = tmp.path().join("config-spec.yaml");
        let env_spec = tmp.path().join("env-spec.yaml");
        fs::write(&config_spec, "openapi: \"3.0.0\"\ninfo:\n  title: Config\n  version: \"1.0\"\npaths: {}\n").unwrap();
        fs::write(&env_spec, "openapi: \"3.0.0\"\ninfo:\n  title: Env\n  version: \"1.0\"\npaths: {}\n").unwrap();
        fs::write(
            tmp.path().join(".phyllotaxis.yaml"),
            "spec: ./config-spec.yaml\n",
        ).unwrap();

        let config = load_config(tmp.path());
        unsafe { std::env::set_var("PHYLLOTAXIS_SPEC", env_spec.to_str().unwrap()) };
        let result = resolve_spec_path(None, &config, tmp.path());
        unsafe { std::env::remove_var("PHYLLOTAXIS_SPEC") };

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), env_spec, "Env var should win over config");
    }

    #[test]
    fn test_resolve_env_var_not_found_is_error() {
        let tmp = tempfile::tempdir().unwrap();

        unsafe { std::env::set_var("PHYLLOTAXIS_SPEC", "/nonexistent/path.yaml") };
        let result = resolve_spec_path(None, &None, tmp.path());
        unsafe { std::env::remove_var("PHYLLOTAXIS_SPEC") };

        assert!(result.is_err(), "Should error when env var points to missing file");
        assert!(
            result.unwrap_err().to_string().contains("PHYLLOTAXIS_SPEC"),
            "Error should mention PHYLLOTAXIS_SPEC"
        );
    }

    #[test]
    fn test_resolve_env_var_empty_falls_through() {
        let tmp = tempfile::tempdir().unwrap();
        let spec_path = tmp.path().join("openapi.yaml");
        fs::write(&spec_path, "openapi: \"3.0.0\"\ninfo:\n  title: Auto\n  version: \"1.0\"\npaths: {}\n").unwrap();

        unsafe { std::env::set_var("PHYLLOTAXIS_SPEC", "") };
        let result = resolve_spec_path(None, &None, tmp.path());
        unsafe { std::env::remove_var("PHYLLOTAXIS_SPEC") };

        assert!(result.is_ok(), "Empty env var should fall through to auto-detect");
        assert_eq!(result.unwrap(), spec_path);
    }
}
