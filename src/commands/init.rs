use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

struct Framework {
    name: &'static str,
    signatures: &'static [&'static str],
    spec_dirs: &'static [&'static str],
}

static FRAMEWORKS: &[Framework] = &[
    Framework {
        name: "Astro",
        signatures: &["astro.config.mjs", "astro.config.ts"],
        spec_dirs: &["src/content"],
    },
    Framework {
        name: "Docusaurus",
        signatures: &["docusaurus.config.js", "docusaurus.config.ts"],
        spec_dirs: &["static"],
    },
    Framework {
        name: "Hugo",
        signatures: &["hugo.toml", "hugo.yaml", "config.toml"],
        spec_dirs: &["static"],
    },
    Framework {
        name: "Jekyll",
        signatures: &["_config.yml", "_config.yaml"],
        spec_dirs: &["assets"],
    },
    Framework {
        name: "MkDocs",
        signatures: &["mkdocs.yml", "mkdocs.yaml"],
        spec_dirs: &["docs"],
    },
];

/// The serializable form of the config file written by init.
/// Fields match the `Config` struct in spec.rs — only what init can write.
#[derive(Debug, Serialize, Deserialize, Default)]
struct PhyllotaxisConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    document: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    documents: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<String>,
}

/// Write a fresh single-document config atomically.
/// Extracted for testability.
pub fn write_init_config(config_path: &Path, doc_path: &str) -> std::io::Result<()> {
    let config = PhyllotaxisConfig {
        document: Some(doc_path.to_string()),
        ..Default::default()
    };
    let content = serde_yaml_ng::to_string(&config).map_err(std::io::Error::other)?;
    atomic_write(config_path, &content)
}

/// Add a named document to an existing config file, using proper YAML round-trip.
/// Extracted for testability.
pub fn write_add_document(config_path: &Path, name: &str, doc_path: &str) -> std::io::Result<()> {
    let existing = std::fs::read_to_string(config_path)?;
    let mut config: PhyllotaxisConfig = serde_yaml_ng::from_str(&existing)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    if config.documents.is_empty() {
        // Migrate from single `document:` to `documents:` map
        if let Some(old_doc) = config.document.take() {
            config.documents.insert("default".to_string(), old_doc);
            if config.default.is_none() {
                config.default = Some("default".to_string());
            }
        }
    }

    config
        .documents
        .insert(name.to_string(), doc_path.to_string());

    let content = serde_yaml_ng::to_string(&config).map_err(std::io::Error::other)?;
    atomic_write(config_path, &content)
}

/// Write content to path atomically: write to .tmp then rename.
/// Both files must be on the same filesystem for the rename to be atomic.
fn atomic_write(path: &Path, content: &str) -> std::io::Result<()> {
    let tmp_path = path.with_extension("yaml.tmp");
    std::fs::write(&tmp_path, content)?;
    std::fs::rename(&tmp_path, path)
}

pub fn detect_framework(dir: &Path) -> Option<&'static str> {
    for fw in FRAMEWORKS {
        for sig in fw.signatures {
            if dir.join(sig).exists() {
                return Some(fw.name);
            }
        }
    }
    None
}

pub fn find_spec_candidates(dir: &Path, framework: Option<&str>) -> Vec<PathBuf> {
    let mut search_dirs = Vec::new();

    // Framework-specific dirs first
    if let Some(fw_name) = framework {
        for fw in FRAMEWORKS {
            if fw.name == fw_name {
                for spec_dir in fw.spec_dirs {
                    let d = dir.join(spec_dir);
                    if d.is_dir() {
                        search_dirs.push(d);
                    }
                }
            }
        }
    }

    // Root dir
    search_dirs.push(dir.to_path_buf());

    // One level of subdirectories
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                search_dirs.push(entry.path());
            }
        }
    }

    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for search_dir in &search_dirs {
        let entries = match std::fs::read_dir(search_dir) {
            Ok(e) => e,
            Err(_) => continue,
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
            if let Ok(canonical) = path.canonicalize() {
                if seen.contains(&canonical) {
                    continue;
                }
                // Check first 200 bytes for "openapi" (bounded read)
                if let Ok(mut file) = std::fs::File::open(&path) {
                    use std::io::Read;
                    let mut buf = [0u8; 200];
                    let n = file.read(&mut buf).unwrap_or(0);
                    let snippet = String::from_utf8_lossy(&buf[..n]);
                    if snippet.contains("openapi") {
                        seen.insert(canonical);
                        results.push(path);
                    }
                }
            }
        }
    }

    results
}

pub fn run_init(start_dir: &Path, doc_path: Option<&Path>) -> anyhow::Result<()> {
    let config_path = start_dir.join(".phyllotaxis.yaml");

    // Non-interactive mode: --doc-path was provided, skip all prompts.
    if let Some(path) = doc_path {
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            start_dir.join(path)
        };

        if !resolved.exists() {
            anyhow::bail!("document not found: {}", resolved.display());
        }

        // Store the path as given (relative stays relative, absolute stays absolute).
        let stored = path.display().to_string();
        write_init_config(&config_path, &stored)
            .with_context(|| "failed to write .phyllotaxis.yaml")?;
        eprintln!("Initialized. Run `phyllotaxis` to see your API overview.");
        return Ok(());
    }

    // Interactive mode — errors here are printed directly since we're in a prompt flow.
    if config_path.exists() {
        run_add_document(start_dir, &config_path);
        return Ok(());
    }

    let framework = detect_framework(start_dir);
    match framework {
        Some(name) => eprintln!("Detected framework: {}", name),
        None => eprintln!("No doc framework detected."),
    }

    let candidates = find_spec_candidates(start_dir, framework);

    if candidates.is_empty() {
        eprintln!("No OpenAPI documents found automatically.");
        eprint!("Enter the path to your OpenAPI document: ");
    } else {
        eprintln!("Found document candidates:");
        for (i, path) in candidates.iter().enumerate() {
            let display = path.strip_prefix(start_dir).unwrap_or(path).display();
            eprintln!("  {}. ./{}", i + 1, display);
        }
        eprint!("Select a document (enter number) or type a path: ");
    }

    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_line(&mut input) {
        eprintln!("Error: failed to read input: {}", e);
        return Ok(());
    }
    let input = input.trim();

    let selected = if let Ok(num) = input.parse::<usize>() {
        if num >= 1 && num <= candidates.len() {
            candidates[num - 1].clone()
        } else {
            PathBuf::from(input)
        }
    } else {
        PathBuf::from(input)
    };

    // Make path relative to start_dir
    let relative = selected
        .strip_prefix(start_dir)
        .unwrap_or(&selected)
        .display()
        .to_string();

    if let Err(e) = write_init_config(&config_path, &relative) {
        eprintln!("Error: failed to write .phyllotaxis.yaml: {}", e);
        return Ok(());
    }

    eprintln!("Initialized. Run `phyllotaxis` to see your API overview.");
    Ok(())
}

/// Called when a config already exists. Prompts to add another named document.
fn run_add_document(start_dir: &Path, config_path: &Path) {
    eprintln!("Config already exists at {}.", config_path.display());
    eprint!("Add another document? Enter a name (or press Enter to cancel): ");

    let mut name_input = String::new();
    if let Err(e) = std::io::stdin().read_line(&mut name_input) {
        eprintln!("Error: failed to read input: {}", e);
        return;
    }
    let name = name_input.trim();

    if name.is_empty() {
        eprintln!("Cancelled. Edit .phyllotaxis.yaml directly to update.");
        return;
    }

    let framework = detect_framework(start_dir);
    let candidates = find_spec_candidates(start_dir, framework);

    if candidates.is_empty() {
        eprint!("Enter the path to the document: ");
    } else {
        eprintln!("Found document candidates:");
        for (i, path) in candidates.iter().enumerate() {
            let display = path.strip_prefix(start_dir).unwrap_or(path).display();
            eprintln!("  {}. ./{}", i + 1, display);
        }
        eprint!("Select a document (enter number) or type a path: ");
    }

    let mut path_input = String::new();
    if let Err(e) = std::io::stdin().read_line(&mut path_input) {
        eprintln!("Error: failed to read input: {}", e);
        return;
    }
    let path_input = path_input.trim();

    let selected = if let Ok(num) = path_input.parse::<usize>() {
        if num >= 1 && num <= candidates.len() {
            candidates[num - 1].clone()
        } else {
            PathBuf::from(path_input)
        }
    } else {
        PathBuf::from(path_input)
    };

    let relative = selected
        .strip_prefix(start_dir)
        .unwrap_or(&selected)
        .display()
        .to_string();

    match write_add_document(config_path, name, &relative) {
        Ok(()) => eprintln!(
            "Added document '{}' → {}. Use `phyllotaxis --doc {} ...` to target it.",
            name, relative, name
        ),
        Err(e) => eprintln!("Error updating config: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ─── Task 3.4: YAML injection tests ───

    #[test]
    fn test_write_init_config_injection_payload_is_escaped() {
        // A path with a newline + YAML key — the classic injection vector
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(".phyllotaxis.yaml");

        let injected_path = "real/path.yaml\ninjected_key: injected_value";
        write_init_config(&config_path, injected_path).unwrap();

        let written = fs::read_to_string(&config_path).unwrap();

        // The injected key must NOT appear as a parsed top-level YAML key.
        // serde_yaml_ng uses block scalars (|-) to safely encode multi-line strings,
        // so the text may appear in the file but must be contained within the scalar value.
        let parsed: PhyllotaxisConfig =
            serde_yaml_ng::from_str(&written).expect("Config file must be valid YAML");

        // No top-level key called "injected_key" — the only key should be "document"
        let top_level: serde_yaml_ng::Value = serde_yaml_ng::from_str(&written).unwrap();
        assert!(
            top_level.get("injected_key").is_none(),
            "YAML injection succeeded — injected_key is a top-level key in:\n{}",
            written
        );

        // The document value must round-trip correctly — the full payload including newline
        // is preserved as the string value, not broken into keys.
        assert_eq!(
            parsed.document.as_deref(),
            Some(injected_path),
            "Document path not preserved after safe serialization"
        );
    }

    #[test]
    fn test_write_init_config_normal_path_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(".phyllotaxis.yaml");

        write_init_config(&config_path, "./openapi.yaml").unwrap();

        let written = fs::read_to_string(&config_path).unwrap();
        let parsed: PhyllotaxisConfig = serde_yaml_ng::from_str(&written).unwrap();
        assert_eq!(parsed.document.as_deref(), Some("./openapi.yaml"));
    }

    #[test]
    fn test_write_add_document_injection_in_path_is_escaped() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(".phyllotaxis.yaml");

        // Start with a valid single-document config
        write_init_config(&config_path, "./openapi.yaml").unwrap();

        // Now add a document whose path contains a YAML injection payload
        let injected_path = "other/doc.yaml\ninjected_key: injected_value";
        write_add_document(&config_path, "extra", injected_path).unwrap();

        let written = fs::read_to_string(&config_path).unwrap();

        // The injected_key must not be a real parsed key in the document
        let top_level: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&written).expect("Config must be valid YAML");
        assert!(
            top_level.get("injected_key").is_none(),
            "YAML injection via doc path — injected_key is a top-level key in:\n{}",
            written
        );
        // And the documents map must not have injected_key as a key
        let docs = top_level
            .get("documents")
            .expect("documents key must exist");
        assert!(
            docs.get("injected_key").is_none(),
            "YAML injection via doc path — injected_key is a documents subkey in:\n{}",
            written
        );
    }

    #[test]
    fn test_write_add_document_injection_in_name_is_escaped() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(".phyllotaxis.yaml");

        write_init_config(&config_path, "./openapi.yaml").unwrap();

        // A name containing YAML-special characters
        let injected_name = "evil\ninjected_key: injected_value";
        write_add_document(&config_path, injected_name, "./other.yaml").unwrap();

        let written = fs::read_to_string(&config_path).unwrap();

        // The injected_key must not appear as a parsed key at any level
        let top_level: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&written).expect("Config must be valid YAML");
        assert!(
            top_level.get("injected_key").is_none(),
            "YAML injection via doc name — injected_key is a top-level key in:\n{}",
            written
        );
        let docs = top_level
            .get("documents")
            .expect("documents key must exist");
        assert!(
            docs.get("injected_key").is_none(),
            "YAML injection via doc name — injected_key is a documents subkey in:\n{}",
            written
        );
    }

    #[test]
    fn test_write_add_document_migrates_single_document_format() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(".phyllotaxis.yaml");

        // Start with single-document format
        write_init_config(&config_path, "./openapi.yaml").unwrap();

        // Add a named document — should migrate and preserve the original
        write_add_document(&config_path, "v2", "./openapi-v2.yaml").unwrap();

        let written = fs::read_to_string(&config_path).unwrap();
        let parsed: PhyllotaxisConfig = serde_yaml_ng::from_str(&written).unwrap();

        assert!(
            parsed.documents.contains_key("default"),
            "Original document should be migrated to 'default'"
        );
        assert_eq!(
            parsed.documents.get("default").map(String::as_str),
            Some("./openapi.yaml")
        );
        assert!(
            parsed.documents.contains_key("v2"),
            "New document 'v2' should be present"
        );
        assert_eq!(
            parsed.documents.get("v2").map(String::as_str),
            Some("./openapi-v2.yaml")
        );
    }

    // ─── Task 3.5: Atomic write tests ───

    #[test]
    fn test_atomic_write_leaves_no_tmp_file_on_success() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(".phyllotaxis.yaml");

        write_init_config(&config_path, "./openapi.yaml").unwrap();

        // The real config file must exist
        assert!(
            config_path.exists(),
            ".phyllotaxis.yaml should exist after successful write"
        );

        // No leftover tmp file
        let tmp_path = config_path.with_extension("yaml.tmp");
        assert!(
            !tmp_path.exists(),
            ".phyllotaxis.yaml.tmp should not exist after successful write"
        );
    }

    #[test]
    fn test_atomic_write_produces_valid_yaml() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(".phyllotaxis.yaml");

        write_init_config(&config_path, "./openapi.yaml").unwrap();

        let written = fs::read_to_string(&config_path).unwrap();
        let result: Result<PhyllotaxisConfig, _> = serde_yaml_ng::from_str(&written);
        assert!(
            result.is_ok(),
            "Config written by init should be valid YAML: {:?}",
            result
        );
    }
}
