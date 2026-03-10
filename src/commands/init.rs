use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
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

/// The serializable form of the config file written by init and mutating ops.
/// Fields match the `Config` struct in spec.rs.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct PhyllotaxisConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub documents: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<HashMap<String, String>>,
}

pub fn write_config(path: &Path, config: &PhyllotaxisConfig) -> std::io::Result<()> {
    let content = serde_yaml_ng::to_string(config).map_err(std::io::Error::other)?;
    atomic_write(path, &content)
}

/// Write a fresh single-document config to `.phyllotaxis/config.yaml` atomically.
pub fn write_init_config(
    project_root: &Path,
    doc_path: &str,
    nickname: &str,
) -> std::io::Result<()> {
    let config_dir = project_root.join(".phyllotaxis");
    std::fs::create_dir_all(&config_dir)?;
    let config_path = config_dir.join("config.yaml");

    let mut config = PhyllotaxisConfig::default();
    config
        .documents
        .insert(nickname.to_string(), doc_path.to_string());
    config.active = Some(nickname.to_string());

    write_config(&config_path, &config)
}

/// Write content to path atomically: write to .tmp then rename.
/// Both files must be on the same filesystem for the rename to be atomic.
fn atomic_write(path: &Path, content: &str) -> std::io::Result<()> {
    let tmp_path = path.with_extension("yaml.tmp");
    std::fs::write(&tmp_path, content)?;
    std::fs::rename(&tmp_path, path)
}

/// Determine the config file path and how to store the doc path for a given scope.
/// Returns (config_file_path, stored_path_string).
fn resolve_write_target(
    project_root: Option<&Path>,
    user_scope: bool,
    doc_path_str: &str,
) -> anyhow::Result<(PathBuf, String)> {
    match project_root {
        Some(root) if !user_scope => {
            let config_path = crate::spec::project_config_path(root);
            // Project config stores path as given by user
            Ok((config_path, doc_path_str.to_string()))
        }
        _ => {
            let config_path = crate::spec::user_config_path().ok_or_else(|| {
                anyhow::anyhow!("Cannot determine home directory for user config")
            })?;
            // User config stores absolute paths
            let abs = std::fs::canonicalize(doc_path_str)
                .with_context(|| format!("Document not found: {}", doc_path_str))?;
            Ok((config_path, abs.to_string_lossy().to_string()))
        }
    }
}

/// Derive a nickname from `name` arg or from the filename stem of `path_str`.
fn resolve_nickname(name: Option<&str>, path_str: &str) -> anyhow::Result<String> {
    if let Some(n) = name {
        if n.is_empty() {
            bail!("Document name cannot be empty.");
        }
        return Ok(n.to_string());
    }
    let stem = std::path::Path::new(path_str)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("Cannot derive nickname from path: {}", path_str))?;
    Ok(stem.to_string())
}

/// Load config from path if it exists, otherwise return a default.
fn load_or_default_config(config_path: &Path) -> anyhow::Result<PhyllotaxisConfig> {
    if !config_path.is_file() {
        return Ok(PhyllotaxisConfig::default());
    }
    let content = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    serde_yaml_ng::from_str(&content)
        .with_context(|| format!("Failed to parse {}", config_path.display()))
}

fn ensure_parent_dir(path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }
    Ok(())
}

/// Add a document to the library without making it active.
/// `project_root`: directory containing (or to contain) `.phyllotaxis/`
/// `user_scope`: if true, always write to user config
/// `doc_path_str`: path as typed by user (may be relative or absolute)
/// `name`: optional nickname; derived from filename stem if None
pub fn run_add_doc(
    project_root: Option<&Path>,
    user_scope: bool,
    doc_path_str: &str,
    name: Option<&str>,
) -> anyhow::Result<()> {
    let (config_path, stored_path) = resolve_write_target(project_root, user_scope, doc_path_str)?;

    // Validate file exists (user-scope already validates via canonicalize in resolve_write_target)
    if !user_scope {
        let check_path = if let Some(root) = project_root {
            root.join(doc_path_str)
        } else {
            PathBuf::from(doc_path_str)
        };
        if !check_path.is_file() && !PathBuf::from(doc_path_str).is_file() {
            bail!("Document not found: {}", doc_path_str);
        }
    }

    let nickname = resolve_nickname(name, doc_path_str)?;

    let mut config = load_or_default_config(&config_path)?;

    if config.documents.contains_key(&nickname) {
        bail!(
            "Document '{}' already exists. Use --remove-doc {} first, or choose a different --name.",
            nickname, nickname
        );
    }

    config.documents.insert(nickname.clone(), stored_path);
    ensure_parent_dir(&config_path)?;
    write_config(&config_path, &config)
        .with_context(|| format!("Failed to write config to {}", config_path.display()))?;

    eprintln!(
        "Added '{}'. Use `phyll --set-doc {}` to make it active.",
        nickname, nickname
    );
    Ok(())
}

/// Set the active document. `target` is either a known nickname or a file path.
pub fn run_set_doc(
    project_root: Option<&Path>,
    user_scope: bool,
    target: &str,
    name: Option<&str>,
) -> anyhow::Result<()> {
    // Determine config path — for nickname lookups, resolve_write_target may fail
    // on canonicalize (target isn't a file), so we handle that gracefully.
    let config_path = match resolve_write_target(project_root, user_scope, target) {
        Ok((cp, _)) => cp,
        Err(_) => {
            // Target isn't a file path; use scope-based config path
            match project_root {
                Some(root) if !user_scope => crate::spec::project_config_path(root),
                _ => crate::spec::user_config_path().ok_or_else(|| {
                    anyhow::anyhow!("Cannot determine home directory for user config")
                })?,
            }
        }
    };

    let mut config = load_or_default_config(&config_path)?;

    // If target matches an existing nickname, just activate it
    if config.documents.contains_key(target) {
        config.active = Some(target.to_string());
        ensure_parent_dir(&config_path)?;
        write_config(&config_path, &config)
            .with_context(|| format!("Failed to write config to {}", config_path.display()))?;
        eprintln!("Active document set to '{}'.", target);
        return Ok(());
    }

    // Otherwise treat target as a file path — does it exist?
    let target_path = std::path::Path::new(target);
    let resolved = if target_path.is_absolute() {
        target_path.to_path_buf()
    } else if let Some(root) = project_root {
        root.join(target)
    } else {
        std::env::current_dir()
            .with_context(|| "Cannot determine current directory")?
            .join(target)
    };

    if !resolved.is_file() {
        bail!(
            "Document '{}' not found as a nickname or file path.\n\
             To add a new document: phyll --add-doc {}\n\
             To see existing documents: phyll --list-docs",
            target,
            target
        );
    }

    // Add then activate
    let nickname = resolve_nickname(name, target)?;
    if !config.documents.contains_key(&nickname) {
        let stored = if user_scope || project_root.is_none() {
            std::fs::canonicalize(&resolved)
                .with_context(|| format!("Cannot resolve {}", resolved.display()))?
                .to_string_lossy()
                .to_string()
        } else {
            target.to_string()
        };
        config.documents.insert(nickname.clone(), stored);
    }
    config.active = Some(nickname.clone());
    ensure_parent_dir(&config_path)?;
    write_config(&config_path, &config)
        .with_context(|| format!("Failed to write config to {}", config_path.display()))?;
    eprintln!("Added '{}' and set as active.", nickname);
    Ok(())
}

/// Return the config path for the target scope without needing a doc path string.
fn target_config_path(project_root: Option<&Path>, user_scope: bool) -> anyhow::Result<PathBuf> {
    match project_root {
        Some(root) if !user_scope => Ok(crate::spec::project_config_path(root)),
        _ => crate::spec::user_config_path()
            .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory for user config")),
    }
}

pub fn run_unset_doc(project_root: Option<&Path>, user_scope: bool) -> anyhow::Result<()> {
    let config_path = target_config_path(project_root, user_scope)?;
    let mut config = load_or_default_config(&config_path)?;
    if config.active.is_none() {
        eprintln!("No active document set.");
        return Ok(());
    }
    config.active = None;
    write_config(&config_path, &config)
        .with_context(|| format!("Failed to write config to {}", config_path.display()))?;
    eprintln!("Active document cleared.");
    Ok(())
}

pub fn run_remove_doc(
    project_root: Option<&Path>,
    user_scope: bool,
    name: &str,
) -> anyhow::Result<()> {
    let config_path = target_config_path(project_root, user_scope)?;
    let mut config = load_or_default_config(&config_path)?;

    if !config.documents.contains_key(name) {
        bail!("Document '{}' not found in config.", name);
    }

    config.documents.remove(name);
    if config.active.as_deref() == Some(name) {
        config.active = None;
    }

    write_config(&config_path, &config)
        .with_context(|| format!("Failed to write config to {}", config_path.display()))?;
    eprintln!("Removed document '{}'.", name);
    Ok(())
}

/// Format one scope's section of the list-docs output.
pub fn format_list_docs_section(label: &str, config: &PhyllotaxisConfig, _is_tty: bool) -> String {
    let mut out = format!("  {}:\n", label);
    if config.documents.is_empty() {
        out.push_str("    (none)\n");
        return out;
    }
    let mut entries: Vec<(&String, &String)> = config.documents.iter().collect();
    entries.sort_by_key(|(k, _)| k.as_str());

    for (name, path) in entries {
        let active_marker = if config.active.as_deref() == Some(name.as_str()) {
            "  (active)"
        } else {
            ""
        };
        out.push_str(&format!("    {:<16} {}{}\n", name, path, active_marker));
    }
    out
}

/// Convert spec::Config to PhyllotaxisConfig for display.
fn to_display_config(cfg: &crate::spec::Config) -> PhyllotaxisConfig {
    PhyllotaxisConfig {
        active: cfg.active.clone(),
        documents: cfg.documents.clone(),
        variables: cfg.variables.clone(),
    }
}

pub fn run_list_docs(scoped: &crate::spec::ScopedConfig) -> anyhow::Result<()> {
    let has_project = scoped.project.is_some();
    let has_user = scoped.user.is_some();

    if !has_project && !has_user {
        println!("No documents configured.");
        println!();
        println!("Add a document:");
        println!("  phyll --add-doc ./path/to/openapi.yaml");
        return Ok(());
    }

    println!("Documents:");
    if let Some((cfg, root)) = &scoped.project {
        let label = format!("Project ({}/.phyllotaxis/)", root.display());
        print!(
            "{}",
            format_list_docs_section(&label, &to_display_config(cfg), false)
        );
    }
    if let Some(cfg) = &scoped.user {
        print!(
            "{}",
            format_list_docs_section(
                "Personal (~/.config/phyllotaxis/)",
                &to_display_config(cfg),
                false
            )
        );
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct DocEntry {
    pub name: String,
    pub path: String,
    pub active: bool,
}

#[derive(serde::Serialize)]
pub struct ScopeJson {
    pub config_path: String,
    pub active: Option<String>,
    pub documents: Vec<DocEntry>,
}

#[derive(serde::Serialize)]
pub struct ListDocsJson {
    pub project: Option<ScopeJson>,
    pub user: Option<ScopeJson>,
}

pub fn build_list_docs_json(scoped: &crate::spec::ScopedConfig) -> ListDocsJson {
    let project = scoped.project.as_ref().map(|(cfg, root)| {
        let config_path = root
            .join(".phyllotaxis")
            .join("config.yaml")
            .display()
            .to_string();
        let mut documents: Vec<DocEntry> = cfg
            .documents
            .iter()
            .map(|(name, path)| DocEntry {
                name: name.clone(),
                path: path.clone(),
                active: cfg.active.as_deref() == Some(name.as_str()),
            })
            .collect();
        documents.sort_by(|a, b| a.name.cmp(&b.name));
        ScopeJson {
            config_path,
            active: cfg.active.clone(),
            documents,
        }
    });

    let user = scoped.user.as_ref().map(|cfg| {
        let config_path = crate::spec::user_config_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "~/.config/phyllotaxis/config.yaml".to_string());
        let mut documents: Vec<DocEntry> = cfg
            .documents
            .iter()
            .map(|(name, path)| DocEntry {
                name: name.clone(),
                path: path.clone(),
                active: cfg.active.as_deref() == Some(name.as_str()),
            })
            .collect();
        documents.sort_by(|a, b| a.name.cmp(&b.name));
        ScopeJson {
            config_path,
            active: cfg.active.clone(),
            documents,
        }
    });

    ListDocsJson { project, user }
}

pub fn run_list_docs_json(scoped: &crate::spec::ScopedConfig) -> anyhow::Result<()> {
    let data = build_list_docs_json(scoped);
    let out = serde_json::to_string_pretty(&data)
        .with_context(|| "Failed to serialize list-docs JSON")?;
    println!("{}", out);
    Ok(())
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

pub fn find_document_candidates(dir: &Path, framework: Option<&str>) -> Vec<PathBuf> {
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
    let config_dir = start_dir.join(".phyllotaxis");
    let config_path = config_dir.join("config.yaml");

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

        let stored = path.display().to_string();
        let nickname = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("default")
            .to_string();

        write_init_config(start_dir, &stored, &nickname)
            .with_context(|| "failed to write .phyllotaxis/config.yaml")?;
        eprintln!("Initialized. Run `phyllotaxis` to see your API overview.");
        return Ok(());
    }

    // Interactive mode
    if config_path.exists() {
        run_add_document(start_dir, &config_path);
        return Ok(());
    }

    let framework = detect_framework(start_dir);
    match framework {
        Some(name) => eprintln!("Detected framework: {}", name),
        None => eprintln!("No doc framework detected."),
    }

    let candidates = find_document_candidates(start_dir, framework);

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

    let nickname = selected
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("default")
        .to_string();

    if let Err(e) = write_init_config(start_dir, &relative, &nickname) {
        eprintln!("Error: failed to write .phyllotaxis/config.yaml: {}", e);
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
    let candidates = find_document_candidates(start_dir, framework);

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

    // Load existing config, add the new document
    let existing = match std::fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading config: {}", e);
            return;
        }
    };
    let mut config: PhyllotaxisConfig = match serde_yaml_ng::from_str(&existing) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error parsing config: {}", e);
            return;
        }
    };
    config.documents.insert(name.to_string(), relative.clone());
    match write_config(config_path, &config) {
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

    #[test]
    fn test_write_init_config_creates_directory_and_file() {
        let tmp = tempfile::tempdir().unwrap();
        write_init_config(tmp.path(), "./openapi.yaml", "api").unwrap();

        let config_path = tmp.path().join(".phyllotaxis").join("config.yaml");
        assert!(config_path.exists(), "config.yaml must be created");

        let written = fs::read_to_string(&config_path).unwrap();
        let parsed: PhyllotaxisConfig = serde_yaml_ng::from_str(&written).unwrap();
        assert_eq!(parsed.active.as_deref(), Some("api"));
        assert_eq!(
            parsed.documents.get("api").map(String::as_str),
            Some("./openapi.yaml")
        );
    }

    #[test]
    fn test_write_init_config_injection_payload_is_escaped() {
        let tmp = tempfile::tempdir().unwrap();
        let injected_path = "real/path.yaml\ninjected_key: injected_value";
        write_init_config(tmp.path(), injected_path, "api").unwrap();

        let config_path = tmp.path().join(".phyllotaxis").join("config.yaml");
        let written = fs::read_to_string(&config_path).unwrap();

        let top_level: serde_yaml_ng::Value = serde_yaml_ng::from_str(&written).unwrap();
        assert!(
            top_level.get("injected_key").is_none(),
            "YAML injection succeeded — injected_key is a top-level key in:\n{}",
            written
        );
    }

    #[test]
    fn test_write_init_config_normal_path_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        write_init_config(tmp.path(), "./openapi.yaml", "api").unwrap();

        let config_path = tmp.path().join(".phyllotaxis").join("config.yaml");
        let written = fs::read_to_string(&config_path).unwrap();
        let parsed: PhyllotaxisConfig = serde_yaml_ng::from_str(&written).unwrap();
        assert_eq!(parsed.active.as_deref(), Some("api"));
        assert_eq!(
            parsed.documents.get("api").map(String::as_str),
            Some("./openapi.yaml")
        );
    }

    #[test]
    fn test_write_config_injection_in_path_is_escaped() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join(".phyllotaxis");
        fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join("config.yaml");

        let mut cfg = PhyllotaxisConfig::default();
        let injected_path = "other/doc.yaml\ninjected_key: injected_value";
        cfg.documents
            .insert("extra".to_string(), injected_path.to_string());
        write_config(&config_path, &cfg).unwrap();

        let written = fs::read_to_string(&config_path).unwrap();
        let top_level: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&written).expect("Config must be valid YAML");
        assert!(
            top_level.get("injected_key").is_none(),
            "YAML injection via doc path — injected_key is a top-level key in:\n{}",
            written
        );
    }

    #[test]
    fn test_write_config_injection_in_name_is_escaped() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join(".phyllotaxis");
        fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join("config.yaml");

        let mut cfg = PhyllotaxisConfig::default();
        let injected_name = "evil\ninjected_key: injected_value";
        cfg.documents
            .insert(injected_name.to_string(), "./other.yaml".to_string());
        write_config(&config_path, &cfg).unwrap();

        let written = fs::read_to_string(&config_path).unwrap();
        let top_level: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&written).expect("Config must be valid YAML");
        assert!(
            top_level.get("injected_key").is_none(),
            "YAML injection via doc name — injected_key is a top-level key in:\n{}",
            written
        );
    }

    #[test]
    fn test_atomic_write_leaves_no_tmp_file_on_success() {
        let tmp = tempfile::tempdir().unwrap();
        write_init_config(tmp.path(), "./openapi.yaml", "api").unwrap();

        let config_path = tmp.path().join(".phyllotaxis").join("config.yaml");
        assert!(config_path.exists(), "config.yaml should exist");

        let tmp_path = config_path.with_extension("yaml.tmp");
        assert!(
            !tmp_path.exists(),
            "tmp file should not exist after success"
        );
    }

    #[test]
    fn test_atomic_write_produces_valid_yaml() {
        let tmp = tempfile::tempdir().unwrap();
        write_init_config(tmp.path(), "./openapi.yaml", "api").unwrap();

        let config_path = tmp.path().join(".phyllotaxis").join("config.yaml");
        let written = fs::read_to_string(&config_path).unwrap();
        let result: Result<PhyllotaxisConfig, _> = serde_yaml_ng::from_str(&written);
        assert!(
            result.is_ok(),
            "Config written by init should be valid YAML: {:?}",
            result
        );
    }

    #[test]
    fn test_add_doc_creates_project_config() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = tmp.path().join("petstore.yaml");
        fs::write(
            &spec,
            "openapi: \"3.0.0\"\ninfo:\n  title: Pets\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();

        run_add_doc(Some(tmp.path()), false, "./petstore.yaml", Some("pets")).unwrap();

        let config_path = tmp.path().join(".phyllotaxis").join("config.yaml");
        assert!(config_path.exists(), "config.yaml must be created");
        let written = fs::read_to_string(&config_path).unwrap();
        let parsed: PhyllotaxisConfig = serde_yaml_ng::from_str(&written).unwrap();
        assert!(
            parsed.documents.contains_key("pets"),
            "pets must be in documents map"
        );
        assert_eq!(
            parsed.documents.get("pets").map(String::as_str),
            Some("./petstore.yaml")
        );
    }

    #[test]
    fn test_add_doc_auto_nickname_from_filename() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = tmp.path().join("stripe-openapi.yaml");
        fs::write(
            &spec,
            "openapi: \"3.0.0\"\ninfo:\n  title: Stripe\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();

        run_add_doc(Some(tmp.path()), false, "./stripe-openapi.yaml", None).unwrap();

        let config_path = tmp.path().join(".phyllotaxis").join("config.yaml");
        let written = fs::read_to_string(&config_path).unwrap();
        let parsed: PhyllotaxisConfig = serde_yaml_ng::from_str(&written).unwrap();
        assert!(
            parsed.documents.contains_key("stripe-openapi"),
            "Nickname should be stem of filename. Got keys: {:?}",
            parsed.documents.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_add_doc_duplicate_name_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = tmp.path().join("petstore.yaml");
        fs::write(
            &spec,
            "openapi: \"3.0.0\"\ninfo:\n  title: Pets\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();

        run_add_doc(Some(tmp.path()), false, "./petstore.yaml", Some("pets")).unwrap();
        let result = run_add_doc(Some(tmp.path()), false, "./petstore.yaml", Some("pets"));
        assert!(result.is_err(), "Duplicate name should error");
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_add_doc_nonexistent_file_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let result = run_add_doc(Some(tmp.path()), false, "./nonexistent.yaml", Some("ghost"));
        assert!(result.is_err(), "Adding nonexistent file should error");
        assert!(
            result.unwrap_err().to_string().contains("not found"),
            "Error should mention file not found"
        );
    }

    #[test]
    fn test_add_doc_does_not_set_active() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = tmp.path().join("petstore.yaml");
        fs::write(
            &spec,
            "openapi: \"3.0.0\"\ninfo:\n  title: Pets\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();

        run_add_doc(Some(tmp.path()), false, "./petstore.yaml", Some("pets")).unwrap();

        let config_path = tmp.path().join(".phyllotaxis").join("config.yaml");
        let written = fs::read_to_string(&config_path).unwrap();
        let parsed: PhyllotaxisConfig = serde_yaml_ng::from_str(&written).unwrap();
        assert!(parsed.active.is_none(), "--add-doc must not set active");
    }

    #[test]
    fn test_set_doc_by_existing_nickname() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = tmp.path().join("petstore.yaml");
        fs::write(
            &spec,
            "openapi: \"3.0.0\"\ninfo:\n  title: Pets\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();

        run_add_doc(Some(tmp.path()), false, "./petstore.yaml", Some("pets")).unwrap();
        run_set_doc(Some(tmp.path()), false, "pets", None).unwrap();

        let config_path = tmp.path().join(".phyllotaxis").join("config.yaml");
        let written = fs::read_to_string(&config_path).unwrap();
        let parsed: PhyllotaxisConfig = serde_yaml_ng::from_str(&written).unwrap();
        assert_eq!(
            parsed.active.as_deref(),
            Some("pets"),
            "active should be set to 'pets'"
        );
    }

    #[test]
    fn test_set_doc_by_file_path_adds_and_activates() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = tmp.path().join("api.yaml");
        fs::write(
            &spec,
            "openapi: \"3.0.0\"\ninfo:\n  title: API\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();

        run_set_doc(Some(tmp.path()), false, "./api.yaml", Some("myapi")).unwrap();

        let config_path = tmp.path().join(".phyllotaxis").join("config.yaml");
        let written = fs::read_to_string(&config_path).unwrap();
        let parsed: PhyllotaxisConfig = serde_yaml_ng::from_str(&written).unwrap();
        assert_eq!(
            parsed.active.as_deref(),
            Some("myapi"),
            "active should be 'myapi'"
        );
        assert!(
            parsed.documents.contains_key("myapi"),
            "myapi should be in documents"
        );
    }

    #[test]
    fn test_set_doc_unknown_nickname_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let result = run_set_doc(Some(tmp.path()), false, "nonexistent", None);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("not found"), "Error: {}", msg);
        assert!(
            msg.contains("--add-doc"),
            "Error should mention --add-doc: {}",
            msg
        );
    }

    #[test]
    fn test_list_docs_output_contains_active_marker() {
        let mut cfg = PhyllotaxisConfig::default();
        cfg.active = Some("pets".to_string());
        cfg.documents
            .insert("pets".to_string(), "./api/petstore.yaml".to_string());
        cfg.documents
            .insert("internal".to_string(), "./api/internal.yaml".to_string());

        let output = format_list_docs_section("Project (.phyllotaxis/)", &cfg, false);
        assert!(
            output.contains("(active)"),
            "Active doc must be marked. Got:\n{}",
            output
        );
        assert!(
            output.contains("pets"),
            "pets must appear. Got:\n{}",
            output
        );
        assert!(
            output.contains("internal"),
            "internal must appear. Got:\n{}",
            output
        );
    }

    #[test]
    fn test_list_docs_section_empty() {
        let cfg = PhyllotaxisConfig::default();
        let output = format_list_docs_section("Project", &cfg, false);
        assert!(
            output.contains("(none)"),
            "Empty section should show (none). Got:\n{}",
            output
        );
    }

    #[test]
    fn test_build_list_docs_json_structure() {
        use crate::spec::{Config, ScopedConfig};

        let mut cfg = Config::default();
        cfg.active = Some("pets".to_string());
        cfg.documents
            .insert("pets".to_string(), "./petstore.yaml".to_string());

        let scoped = ScopedConfig {
            project: Some((cfg, PathBuf::from("/tmp/test"))),
            user: None,
        };

        let json = build_list_docs_json(&scoped);
        assert!(json.project.is_some());
        assert!(json.user.is_none());
        let project = json.project.unwrap();
        assert_eq!(project.active.as_deref(), Some("pets"));
        assert_eq!(project.documents.len(), 1);
        assert!(project.documents[0].active);
    }

    #[test]
    fn test_unset_doc_clears_active() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = tmp.path().join("petstore.yaml");
        fs::write(
            &spec,
            "openapi: \"3.0.0\"\ninfo:\n  title: Pets\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();

        run_add_doc(Some(tmp.path()), false, "./petstore.yaml", Some("pets")).unwrap();
        run_set_doc(Some(tmp.path()), false, "pets", None).unwrap();
        run_unset_doc(Some(tmp.path()), false).unwrap();

        let config_path = tmp.path().join(".phyllotaxis").join("config.yaml");
        let written = fs::read_to_string(&config_path).unwrap();
        let parsed: PhyllotaxisConfig = serde_yaml_ng::from_str(&written).unwrap();
        assert!(
            parsed.active.is_none(),
            "active should be cleared after unset"
        );
        assert!(
            parsed.documents.contains_key("pets"),
            "pets should still be in documents"
        );
    }

    #[test]
    fn test_remove_doc_removes_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = tmp.path().join("petstore.yaml");
        fs::write(
            &spec,
            "openapi: \"3.0.0\"\ninfo:\n  title: Pets\n  version: \"1.0\"\npaths: {}\n",
        )
        .unwrap();

        run_add_doc(Some(tmp.path()), false, "./petstore.yaml", Some("pets")).unwrap();
        run_set_doc(Some(tmp.path()), false, "pets", None).unwrap();
        run_remove_doc(Some(tmp.path()), false, "pets").unwrap();

        let config_path = tmp.path().join(".phyllotaxis").join("config.yaml");
        let written = fs::read_to_string(&config_path).unwrap();
        let parsed: PhyllotaxisConfig = serde_yaml_ng::from_str(&written).unwrap();
        assert!(
            !parsed.documents.contains_key("pets"),
            "pets should be removed"
        );
        assert!(
            parsed.active.is_none(),
            "active should be cleared when removed doc was active"
        );
    }

    #[test]
    fn test_remove_doc_unknown_name_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let result = run_remove_doc(Some(tmp.path()), false, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_resolve_nickname_explicit() {
        assert_eq!(
            resolve_nickname(Some("myapi"), "./path.yaml").unwrap(),
            "myapi"
        );
    }

    #[test]
    fn test_resolve_nickname_from_path() {
        assert_eq!(
            resolve_nickname(None, "./petstore.yaml").unwrap(),
            "petstore"
        );
        assert_eq!(
            resolve_nickname(None, "stripe-openapi.json").unwrap(),
            "stripe-openapi"
        );
    }

    #[test]
    fn test_resolve_nickname_empty_errors() {
        let result = resolve_nickname(Some(""), "./path.yaml");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_load_or_default_config_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let config = load_or_default_config(&tmp.path().join("nonexistent.yaml")).unwrap();
        assert!(config.documents.is_empty());
        assert!(config.active.is_none());
    }

    #[test]
    fn test_load_or_default_config_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("config.yaml");
        fs::write(
            &config_path,
            "active: api\ndocuments:\n  api: ./openapi.yaml\n",
        )
        .unwrap();

        let config = load_or_default_config(&config_path).unwrap();
        assert_eq!(config.active.as_deref(), Some("api"));
        assert!(config.documents.contains_key("api"));
    }
}
