# Phyllotaxis Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Rust CLI that gives LLM agents progressive disclosure into an OpenAPI 3.0.x spec — drill from overview to resource groups to endpoints to full detail.

**Architecture:** Stateless CLI. Every invocation parses the spec, resolves the command, prints plain text output with drill-deeper hints. No daemon, no cache. The spec is the database. Clap handles arg parsing with a custom command structure (not nested subcommands — we use positional args for the progressive drill-down).

**Tech Stack:** Rust, `clap` (derive), `openapiv3` crate, `serde_yaml`, `serde_json`

**Design Spec:** `docs/phyllotaxis-design-spec.md`

---

### Task 1: Project Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`

**Step 1: Initialize cargo project**

```bash
cargo init --name phyllotaxis
```

**Step 2: Add dependencies to Cargo.toml**

Replace the generated `[dependencies]` section:

```toml
[package]
name = "phyllotaxis"
version = "0.1.0"
edition = "2021"
description = "Progressive disclosure for OpenAPI specs. Built for LLM agents."
license = "MIT"

[dependencies]
clap = { version = "4", features = ["derive"] }
openapiv3 = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
anyhow = "1"

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

**Step 3: Create minimal main.rs**

```rust
use anyhow::Result;

fn main() -> Result<()> {
    println!("phyllotaxis v{}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
```

**Step 4: Create empty lib.rs**

```rust
pub mod spec;
```

**Step 5: Create spec module placeholder**

Create `src/spec.rs`:

```rust
// OpenAPI spec loading and navigation
```

**Step 6: Verify it compiles and runs**

```bash
cargo run
```

Expected: `phyllotaxis v0.1.0`

**Step 7: Commit**

```bash
git add -A
git commit -m "feat: project scaffold with dependencies"
```

---

### Task 2: Spec Loading

**Files:**
- Create: `src/spec.rs`
- Create: `tests/fixtures/minimal.yaml`
- Create: `tests/spec_loading.rs`

**Step 1: Create a minimal test fixture**

Create `tests/fixtures/minimal.yaml`:

```yaml
openapi: "3.0.4"
info:
  title: "Test API"
  version: "1.0.0"
paths:
  /pets:
    get:
      tags:
        - Pets
      summary: List all pets
      operationId: listPets
      responses:
        "200":
          description: A list of pets
    post:
      tags:
        - Pets
      summary: Create a pet
      operationId: createPet
      responses:
        "201":
          description: Pet created
  /pets/{petId}:
    get:
      tags:
        - Pets
      summary: Get a pet by ID
      operationId: getPet
      parameters:
        - name: petId
          in: path
          required: true
          schema:
            type: string
      responses:
        "200":
          description: A pet
tags:
  - name: Pets
    description: Everything about your pets
components:
  schemas:
    Pet:
      type: object
      required:
        - id
        - name
      properties:
        id:
          type: integer
          format: int64
          readOnly: true
        name:
          type: string
        tag:
          type: string
  securitySchemes:
    bearerAuth:
      type: http
      scheme: bearer
security:
  - bearerAuth: []
```

**Step 2: Write failing test for spec loading**

Create `tests/spec_loading.rs`:

```rust
use std::path::Path;

#[test]
fn test_load_yaml_spec() {
    let spec = phyllotaxis::spec::load(Path::new("tests/fixtures/minimal.yaml"))
        .expect("should load spec");
    assert_eq!(spec.info.title, "Test API");
    assert_eq!(spec.info.version.as_deref(), Some("1.0.0"));
}

#[test]
fn test_load_nonexistent_spec() {
    let result = phyllotaxis::spec::load(Path::new("tests/fixtures/nonexistent.yaml"));
    assert!(result.is_err());
}
```

**Step 3: Run test to verify it fails**

```bash
cargo test test_load_yaml_spec
```

Expected: FAIL — `load` function doesn't exist.

**Step 4: Implement spec loading**

Replace `src/spec.rs`:

```rust
use anyhow::{Context, Result};
use openapiv3::OpenAPI;
use std::path::Path;

/// Load and parse an OpenAPI spec from a YAML or JSON file.
pub fn load(path: &Path) -> Result<OpenAPI> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read spec file: {}", path.display()))?;

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let spec: OpenAPI = match ext {
        "json" => serde_json::from_str(&content)
            .with_context(|| "Failed to parse spec as JSON")?,
        _ => serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse spec as YAML")?,
    };

    Ok(spec)
}
```

Update `src/lib.rs`:

```rust
pub mod spec;
```

**Step 5: Run tests to verify they pass**

```bash
cargo test
```

Expected: 2 tests PASS.

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: spec loading from YAML/JSON files"
```

---

### Task 3: Spec Discovery (config file + auto-detect)

**Files:**
- Create: `src/discovery.rs`
- Create: `tests/discovery.rs`
- Modify: `src/lib.rs`

**Step 1: Write failing tests**

Create `tests/discovery.rs`:

```rust
use std::fs;
use tempfile::TempDir;

#[test]
fn test_discover_from_config_file() {
    let dir = TempDir::new().unwrap();
    let spec_path = dir.path().join("api.yaml");
    fs::write(&spec_path, "openapi: '3.0.0'\ninfo:\n  title: Test\n  version: '1.0'\npaths: {}").unwrap();

    let config_path = dir.path().join(".phyllotaxis.yaml");
    fs::write(&config_path, format!("spec: {}", spec_path.display())).unwrap();

    let result = phyllotaxis::discovery::discover(dir.path()).unwrap();
    assert_eq!(result, spec_path);
}

#[test]
fn test_discover_auto_detect_openapi_yaml() {
    let dir = TempDir::new().unwrap();
    let spec_path = dir.path().join("openapi.yaml");
    fs::write(&spec_path, "openapi: '3.0.0'\ninfo:\n  title: Test\n  version: '1.0'\npaths: {}").unwrap();

    let result = phyllotaxis::discovery::discover(dir.path()).unwrap();
    assert_eq!(result, spec_path);
}

#[test]
fn test_discover_auto_detect_openapi_json() {
    let dir = TempDir::new().unwrap();
    let spec_path = dir.path().join("openapi.json");
    fs::write(&spec_path, r#"{"openapi":"3.0.0","info":{"title":"Test","version":"1.0"},"paths":{}}"#).unwrap();

    let result = phyllotaxis::discovery::discover(dir.path()).unwrap();
    assert_eq!(result, spec_path);
}

#[test]
fn test_discover_nothing_found() {
    let dir = TempDir::new().unwrap();
    let result = phyllotaxis::discovery::discover(dir.path());
    assert!(result.is_err());
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test test_discover
```

Expected: FAIL — `discovery` module doesn't exist.

**Step 3: Implement discovery**

Create `src/discovery.rs`:

```rust
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// Config file structure
#[derive(serde::Deserialize)]
struct Config {
    spec: String,
}

/// Well-known spec filenames to auto-detect
const AUTO_DETECT_NAMES: &[&str] = &[
    "openapi.yaml",
    "openapi.yml",
    "openapi.json",
    "swagger.yaml",
    "swagger.yml",
    "swagger.json",
];

/// Discover the spec file path. Priority:
/// 1. .phyllotaxis.yaml config file in given dir (or parents)
/// 2. Auto-detect well-known filenames in given dir
/// 3. Error
pub fn discover(start_dir: &Path) -> Result<PathBuf> {
    // Try config file (walk up directory tree)
    if let Some(path) = find_config(start_dir)? {
        return Ok(path);
    }

    // Try auto-detect in current directory
    for name in AUTO_DETECT_NAMES {
        let candidate = start_dir.join(name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    bail!(
        "No OpenAPI spec found. Either:\n  \
         - Run `phyllotaxis init` to create a .phyllotaxis.yaml config\n  \
         - Place an openapi.yaml in the current directory\n  \
         - Use --spec <path> to specify the spec file"
    )
}

/// Walk up from start_dir looking for .phyllotaxis.yaml
fn find_config(start_dir: &Path) -> Result<Option<PathBuf>> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let config_path = dir.join(".phyllotaxis.yaml");
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read {}", config_path.display()))?;
            let config: Config = serde_yaml::from_str(&content)
                .with_context(|| format!("Failed to parse {}", config_path.display()))?;

            let spec_path = if Path::new(&config.spec).is_absolute() {
                PathBuf::from(&config.spec)
            } else {
                dir.join(&config.spec)
            };

            if !spec_path.exists() {
                bail!(
                    "Config file {} points to {}, but file not found",
                    config_path.display(),
                    spec_path.display()
                );
            }

            return Ok(Some(spec_path));
        }

        if !dir.pop() {
            break;
        }
    }
    Ok(None)
}
```

Update `src/lib.rs`:

```rust
pub mod discovery;
pub mod spec;
```

**Step 4: Run tests**

```bash
cargo test
```

Expected: All pass.

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: spec discovery via config file and auto-detect"
```

---

### Task 4: Slugification

**Files:**
- Create: `src/slug.rs`
- Create: `tests/slug.rs`
- Modify: `src/lib.rs`

**Step 1: Write failing tests**

Create `tests/slug.rs`:

```rust
#[test]
fn test_slugify_basic() {
    assert_eq!(phyllotaxis::slug::slugify("Access Condition"), "access-condition");
}

#[test]
fn test_slugify_with_version() {
    assert_eq!(phyllotaxis::slug::slugify("Access Policy v2"), "access-policy-v2");
}

#[test]
fn test_slugify_strips_deprecated() {
    assert_eq!(phyllotaxis::slug::slugify("Access Policy (Deprecated)"), "access-policy");
}

#[test]
fn test_slugify_pascal_case() {
    assert_eq!(phyllotaxis::slug::slugify("DiscoveryIntegration"), "discovery-integration");
}

#[test]
fn test_slugify_mixed() {
    assert_eq!(phyllotaxis::slug::slugify("MFA SignOn Policy"), "mfa-sign-on-policy");
}

#[test]
fn test_slugify_already_slug() {
    assert_eq!(phyllotaxis::slug::slugify("access-policies"), "access-policies");
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test test_slugify
```

**Step 3: Implement slugification**

Create `src/slug.rs`:

```rust
/// Convert an OpenAPI tag name to a CLI-friendly slug.
///
/// Rules:
/// - Strip "(Deprecated)" suffix
/// - Split PascalCase words
/// - Lowercase, spaces/underscores to hyphens
/// - Collapse multiple hyphens
pub fn slugify(name: &str) -> String {
    // Strip (Deprecated) and similar parenthetical suffixes
    let name = name
        .replace("(Deprecated)", "")
        .replace("(deprecated)", "")
        .trim()
        .to_string();

    // Split PascalCase: insert hyphen before each uppercase letter
    // that follows a lowercase letter or precedes a lowercase letter in a run
    let mut result = String::with_capacity(name.len() + 8);
    let chars: Vec<char> = name.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];
        if c.is_uppercase() && i > 0 {
            let prev = chars[i - 1];
            // Insert hyphen if previous char was lowercase/digit
            if prev.is_lowercase() || prev.is_ascii_digit() {
                result.push('-');
            }
            // Or if this uppercase is followed by lowercase (end of acronym)
            // e.g. "MFA" + "S" -> no split, but "MFAs" -> "mf-as" is wrong
            // Better: "MFASignOn" -> "mfa-sign-on"
            else if prev.is_uppercase() {
                if let Some(&next) = chars.get(i + 1) {
                    if next.is_lowercase() {
                        result.push('-');
                    }
                }
            }
        }

        if c == ' ' || c == '_' {
            result.push('-');
        } else {
            result.push(c.to_ascii_lowercase());
        }
    }

    // Collapse multiple hyphens and trim
    let mut collapsed = String::with_capacity(result.len());
    let mut prev_hyphen = false;
    for c in result.chars() {
        if c == '-' {
            if !prev_hyphen && !collapsed.is_empty() {
                collapsed.push('-');
            }
            prev_hyphen = true;
        } else {
            collapsed.push(c);
            prev_hyphen = false;
        }
    }

    collapsed.trim_end_matches('-').to_string()
}
```

Update `src/lib.rs`:

```rust
pub mod discovery;
pub mod slug;
pub mod spec;
```

**Step 4: Run tests**

```bash
cargo test test_slugify
```

Expected: All pass. If any fail, adjust the PascalCase splitting logic and re-run.

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: tag name slugification"
```

---

### Task 5: CLI Argument Parsing

**Files:**
- Create: `src/cli.rs`
- Modify: `src/main.rs`
- Modify: `src/lib.rs`

**Step 1: Implement CLI parsing**

Create `src/cli.rs`:

```rust
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "phyllotaxis",
    about = "Progressive disclosure for OpenAPI specs. Built for LLM agents.",
    version
)]
pub struct Cli {
    /// Override spec file location
    #[arg(long)]
    pub spec: Option<PathBuf>,

    /// Output in JSON format
    #[arg(long)]
    pub json: bool,

    /// Recursively inline all nested schema definitions
    #[arg(long)]
    pub expand: bool,

    /// The command and its arguments
    ///
    /// Examples:
    ///   phyllotaxis                              API overview
    ///   phyllotaxis resources                    List resource groups
    ///   phyllotaxis resources access-policies    Endpoints for a resource
    ///   phyllotaxis schemas                      List all schemas
    ///   phyllotaxis schemas Pet                  Schema detail
    ///   phyllotaxis auth                         Auth details
    ///   phyllotaxis search <term>                Search everything
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

/// Parsed command from positional args
#[derive(Debug, PartialEq)]
pub enum Command {
    /// No args: show API overview
    Overview,
    /// `resources`: list all resource groups
    ResourceList,
    /// `resources <slug>`: show endpoints for a resource
    ResourceDetail { slug: String },
    /// `resources <slug> <METHOD> <path>`: show endpoint detail
    EndpointDetail {
        slug: String,
        method: String,
        path: String,
    },
    /// `schemas`: list all schemas
    SchemaList,
    /// `schemas <name>`: show schema detail
    SchemaDetail { name: String },
    /// `auth`: show authentication details
    Auth,
    /// `search <term>`: search across everything
    Search { term: String },
}

impl Command {
    pub fn parse(args: &[String]) -> Self {
        if args.is_empty() {
            return Command::Overview;
        }

        match args[0].as_str() {
            "resources" => match args.len() {
                1 => Command::ResourceList,
                2 => Command::ResourceDetail {
                    slug: args[1].clone(),
                },
                4 => Command::EndpointDetail {
                    slug: args[1].clone(),
                    method: args[2].to_uppercase(),
                    path: args[3].clone(),
                },
                _ => Command::ResourceDetail {
                    slug: args[1].clone(),
                },
            },
            "schemas" => match args.len() {
                1 => Command::SchemaList,
                _ => Command::SchemaDetail {
                    name: args[1].clone(),
                },
            },
            "auth" => Command::Auth,
            "search" => Command::Search {
                term: args[1..].join(" "),
            },
            _ => Command::Overview,
        }
    }
}
```

**Step 2: Write tests for command parsing**

Add to bottom of `src/cli.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        if s.is_empty() {
            return vec![];
        }
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn test_parse_overview() {
        assert_eq!(Command::parse(&args("")), Command::Overview);
    }

    #[test]
    fn test_parse_resource_list() {
        assert_eq!(Command::parse(&args("resources")), Command::ResourceList);
    }

    #[test]
    fn test_parse_resource_detail() {
        assert_eq!(
            Command::parse(&args("resources access-policies")),
            Command::ResourceDetail {
                slug: "access-policies".into()
            }
        );
    }

    #[test]
    fn test_parse_endpoint_detail() {
        assert_eq!(
            Command::parse(&args("resources access-policies POST /access-policies")),
            Command::EndpointDetail {
                slug: "access-policies".into(),
                method: "POST".into(),
                path: "/access-policies".into(),
            }
        );
    }

    #[test]
    fn test_parse_schema_list() {
        assert_eq!(Command::parse(&args("schemas")), Command::SchemaList);
    }

    #[test]
    fn test_parse_schema_detail() {
        assert_eq!(
            Command::parse(&args("schemas Pet")),
            Command::SchemaDetail {
                name: "Pet".into()
            }
        );
    }

    #[test]
    fn test_parse_auth() {
        assert_eq!(Command::parse(&args("auth")), Command::Auth);
    }

    #[test]
    fn test_parse_search() {
        assert_eq!(
            Command::parse(&args("search workload")),
            Command::Search {
                term: "workload".into()
            }
        );
    }
}
```

**Step 3: Update main.rs to use CLI**

```rust
mod cli;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    let command = cli::Command::parse(&cli.args);

    // Resolve spec path
    let spec_path = match &cli.spec {
        Some(path) => path.clone(),
        None => phyllotaxis::discovery::discover(&std::env::current_dir()?)?,
    };

    let spec = phyllotaxis::spec::load(&spec_path)?;

    // Dispatch command (placeholder — each will be implemented in subsequent tasks)
    match command {
        cli::Command::Overview => println!("API: {}", spec.info.title),
        _ => println!("Command: {:?}", command),
    }

    Ok(())
}
```

**Step 4: Run tests**

```bash
cargo test
```

Expected: All pass.

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: CLI arg parsing with command dispatch"
```

---

### Task 6: Output Formatters — Level 0 (Overview)

**Files:**
- Create: `src/output/mod.rs`
- Create: `src/output/overview.rs`
- Create: `tests/output_overview.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`

**Step 1: Write failing test**

Create `tests/output_overview.rs`:

```rust
use openapiv3::OpenAPI;
use std::path::Path;

#[test]
fn test_overview_output() {
    let spec = phyllotaxis::spec::load(Path::new("tests/fixtures/minimal.yaml")).unwrap();
    let output = phyllotaxis::output::overview::render(&spec);

    assert!(output.contains("API: Test API"));
    assert!(output.contains("phyllotaxis resources"));
    assert!(output.contains("phyllotaxis schemas"));
    assert!(output.contains("phyllotaxis auth"));
    assert!(output.contains("phyllotaxis search"));
}
```

**Step 2: Run test to verify it fails**

```bash
cargo test test_overview_output
```

**Step 3: Implement overview renderer**

Create `src/output/mod.rs`:

```rust
pub mod overview;
```

Create `src/output/overview.rs`:

```rust
use openapiv3::OpenAPI;

pub fn render(spec: &OpenAPI) -> String {
    let mut out = String::new();

    // API title
    out.push_str(&format!("API: {}\n", spec.info.title));

    // Version
    if let Some(ref version) = spec.info.version {
        out.push_str(&format!("Version: {}\n", version));
    }

    // Base URL
    if let Some(server) = spec.servers.first() {
        out.push_str(&format!("Base URL: {}\n", server.url));
    }

    // Auth summary
    let auth_summary = summarize_auth(spec);
    if !auth_summary.is_empty() {
        out.push_str(&format!("Auth: {}\n", auth_summary));
    }

    out.push('\n');

    // Count resources (tags)
    let tag_count = count_tags(spec);
    let schema_count = count_schemas(spec);

    out.push_str("Commands:\n");
    out.push_str(&format!(
        "  phyllotaxis resources    List all resource groups ({} available)\n",
        tag_count
    ));
    out.push_str(&format!(
        "  phyllotaxis schemas      List all data models ({} available)\n",
        schema_count
    ));
    out.push_str("  phyllotaxis auth         Authentication details\n");
    out.push_str("  phyllotaxis search       Search across all endpoints and schemas\n");

    out
}

fn summarize_auth(spec: &OpenAPI) -> String {
    let components = match &spec.components {
        Some(c) => c,
        None => return String::new(),
    };

    let schemes: Vec<String> = components
        .security_schemes
        .iter()
        .map(|(name, scheme_ref)| {
            match scheme_ref {
                openapiv3::ReferenceOr::Item(scheme) => match scheme {
                    openapiv3::SecurityScheme::HTTP { scheme, .. } => {
                        format!("{} (HTTP {})", name, scheme)
                    }
                    openapiv3::SecurityScheme::APIKey { location, name: key_name, .. } => {
                        format!("API key in {:?}: {}", location, key_name)
                    }
                    openapiv3::SecurityScheme::OAuth2 { .. } => {
                        format!("{} (OAuth2)", name)
                    }
                    openapiv3::SecurityScheme::OpenIDConnect { .. } => {
                        format!("{} (OpenID Connect)", name)
                    }
                },
                openapiv3::ReferenceOr::Reference { .. } => name.clone(),
            }
        })
        .collect();

    schemes.join(", ")
}

fn count_tags(spec: &OpenAPI) -> usize {
    if !spec.tags.is_empty() {
        return spec.tags.len();
    }
    // Fall back: collect unique tags from operations
    let mut tags = std::collections::HashSet::new();
    for (_path, item) in spec.paths.iter() {
        if let openapiv3::ReferenceOr::Item(path_item) = item {
            for (_method, op) in path_item.iter() {
                for tag in &op.tags {
                    tags.insert(tag.clone());
                }
            }
        }
    }
    tags.len()
}

fn count_schemas(spec: &OpenAPI) -> usize {
    spec.components
        .as_ref()
        .map(|c| c.schemas.len())
        .unwrap_or(0)
}
```

Update `src/lib.rs`:

```rust
pub mod discovery;
pub mod output;
pub mod slug;
pub mod spec;
```

**Step 4: Run tests**

```bash
cargo test test_overview_output
```

Expected: PASS. If the openapiv3 `info.version` field is `String` not `Option<String>`, adjust accordingly — check compiler errors and fix.

**Step 5: Wire into main.rs dispatch**

Update the `Command::Overview` arm in `main.rs`:

```rust
cli::Command::Overview => print!("{}", phyllotaxis::output::overview::render(&spec)),
```

**Step 6: Manual smoke test**

```bash
cargo run -- --spec tests/fixtures/minimal.yaml
```

Expected output similar to:
```
API: Test API
Version: 1.0.0
Auth: bearerAuth (HTTP bearer)

Commands:
  phyllotaxis resources    List all resource groups (1 available)
  phyllotaxis schemas      List all data models (1 available)
  phyllotaxis auth         Authentication details
  phyllotaxis search       Search across all endpoints and schemas
```

**Step 7: Commit**

```bash
git add -A
git commit -m "feat: Level 0 overview output"
```

---

### Task 7: Output — Level 1 (Resource List)

**Files:**
- Create: `src/output/resources.rs`
- Create: `tests/output_resources.rs`
- Modify: `src/output/mod.rs`
- Modify: `src/main.rs`

**Step 1: Write failing test**

Create `tests/output_resources.rs`:

```rust
use std::path::Path;

#[test]
fn test_resource_list_output() {
    let spec = phyllotaxis::spec::load(Path::new("tests/fixtures/minimal.yaml")).unwrap();
    let output = phyllotaxis::output::resources::render_list(&spec);

    assert!(output.contains("pets"));
    assert!(output.contains("Everything about your pets"));
    assert!(output.contains("Drill deeper:"));
    assert!(output.contains("phyllotaxis resources <name>"));
}
```

**Step 2: Implement resource list renderer**

Create `src/output/resources.rs`:

```rust
use crate::slug::slugify;
use openapiv3::OpenAPI;

/// Render the list of all resource groups (tags)
pub fn render_list(spec: &OpenAPI) -> String {
    let mut out = String::new();
    out.push_str("Resources:\n");

    let tags = collect_tags(spec);

    // Find max slug width for alignment
    let max_slug = tags.iter().map(|t| t.slug.len()).max().unwrap_or(0);

    for tag in &tags {
        let padding = " ".repeat(max_slug - tag.slug.len() + 2);
        let desc = tag.description.as_deref().unwrap_or("");

        let mut line = format!("  {}{}{}", tag.slug, padding, desc);

        if tag.deprecated {
            if let Some(ref replacement) = tag.replacement {
                line = format!(
                    "  {}{}[DEPRECATED -> use {}] {}",
                    tag.slug, padding, replacement, desc
                );
            } else {
                line = format!("  {}{}[DEPRECATED] {}", tag.slug, padding, desc);
            }
        }

        out.push_str(&line);
        out.push('\n');
    }

    out.push_str("\nDrill deeper:\n");
    out.push_str("  phyllotaxis resources <name>\n");

    out
}

/// A processed tag with slug and metadata
pub struct TagInfo {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub deprecated: bool,
    pub replacement: Option<String>,
}

/// Collect and process all tags from the spec
pub fn collect_tags(spec: &OpenAPI) -> Vec<TagInfo> {
    let mut tags: Vec<TagInfo> = if !spec.tags.is_empty() {
        spec.tags
            .iter()
            .map(|tag| {
                let deprecated = tag.name.contains("(Deprecated)")
                    || tag.name.contains("(deprecated)");
                let replacement = if deprecated {
                    guess_replacement(&tag.name, &spec.tags)
                } else {
                    None
                };

                TagInfo {
                    name: tag.name.clone(),
                    slug: slugify(&tag.name),
                    description: tag.description.clone(),
                    deprecated,
                    replacement,
                }
            })
            .collect()
    } else {
        // Fallback: collect unique tags from operations
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for (_path, item) in spec.paths.iter() {
            if let openapiv3::ReferenceOr::Item(path_item) = item {
                for (_method, op) in path_item.iter() {
                    for tag_name in &op.tags {
                        if seen.insert(tag_name.clone()) {
                            result.push(TagInfo {
                                name: tag_name.clone(),
                                slug: slugify(tag_name),
                                description: None,
                                deprecated: false,
                                replacement: None,
                            });
                        }
                    }
                }
            }
        }
        result
    };

    // Sort: non-deprecated first, then alphabetical
    tags.sort_by(|a, b| {
        a.deprecated
            .cmp(&b.deprecated)
            .then(a.slug.cmp(&b.slug))
    });

    tags
}

/// Try to guess a non-deprecated replacement for a deprecated tag.
/// e.g. "Access Policy (Deprecated)" -> look for "Access Policy v2"
fn guess_replacement(deprecated_name: &str, all_tags: &[openapiv3::Tag]) -> Option<String> {
    let base = deprecated_name
        .replace("(Deprecated)", "")
        .replace("(deprecated)", "")
        .trim()
        .to_string();

    for tag in all_tags {
        if tag.name != deprecated_name
            && tag.name.starts_with(&base)
            && !tag.name.contains("(Deprecated)")
            && !tag.name.contains("(deprecated)")
        {
            return Some(slugify(&tag.name));
        }
    }
    None
}
```

Update `src/output/mod.rs`:

```rust
pub mod overview;
pub mod resources;
```

**Step 3: Run tests**

```bash
cargo test test_resource_list
```

Expected: PASS.

**Step 4: Wire into main.rs**

Add dispatch for `ResourceList`:

```rust
cli::Command::ResourceList => {
    print!("{}", phyllotaxis::output::resources::render_list(&spec));
}
```

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: Level 1 resource list output with deprecation handling"
```

---

### Task 8: Output — Level 2 (Resource Detail / Endpoint List)

**Files:**
- Create: `tests/output_resource_detail.rs`
- Modify: `src/output/resources.rs`
- Modify: `src/main.rs`

**Step 1: Write failing test**

Create `tests/output_resource_detail.rs`:

```rust
use std::path::Path;

#[test]
fn test_resource_detail_output() {
    let spec = phyllotaxis::spec::load(Path::new("tests/fixtures/minimal.yaml")).unwrap();
    let output = phyllotaxis::output::resources::render_detail(&spec, "pets").unwrap();

    assert!(output.contains("Resource: Pets"));
    assert!(output.contains("GET"));
    assert!(output.contains("/pets"));
    assert!(output.contains("POST"));
    assert!(output.contains("Drill deeper:"));
    assert!(output.contains("phyllotaxis resources pets GET /pets"));
}

#[test]
fn test_resource_detail_not_found() {
    let spec = phyllotaxis::spec::load(Path::new("tests/fixtures/minimal.yaml")).unwrap();
    let result = phyllotaxis::output::resources::render_detail(&spec, "nonexistent");
    assert!(result.is_err());
}
```

**Step 2: Implement resource detail renderer**

Add to `src/output/resources.rs`:

```rust
use anyhow::{bail, Result};

/// Render detail for a specific resource group (endpoints list)
pub fn render_detail(spec: &OpenAPI, slug: &str) -> Result<String> {
    let tags = collect_tags(spec);
    let tag = tags
        .iter()
        .find(|t| t.slug == slug)
        .ok_or_else(|| anyhow::anyhow!("Resource '{}' not found. Run `phyllotaxis resources` to see available resources.", slug))?;

    let mut out = String::new();

    // Deprecation warning
    if tag.deprecated {
        if let Some(ref replacement) = tag.replacement {
            out.push_str(&format!(
                "WARNING: DEPRECATED. Use \"{}\" instead.\n  phyllotaxis resources {}\n\n",
                replacement, replacement
            ));
        } else {
            out.push_str("WARNING: DEPRECATED.\n\n");
        }
    }

    out.push_str(&format!("Resource: {}\n", tag.name));
    if let Some(ref desc) = tag.description {
        out.push_str(&format!("Description: {}\n", desc));
    }
    out.push('\n');

    // Collect endpoints for this tag
    let endpoints = collect_endpoints_for_tag(spec, &tag.name);

    if endpoints.is_empty() {
        out.push_str("No endpoints found for this resource.\n");
        return Ok(out);
    }

    out.push_str("Endpoints:\n");

    // Find max method width for alignment
    let max_method = endpoints.iter().map(|e| e.method.len()).max().unwrap_or(0);
    let max_path = endpoints.iter().map(|e| e.path.len()).max().unwrap_or(0);

    for ep in &endpoints {
        let method_pad = " ".repeat(max_method - ep.method.len() + 1);
        let path_pad = " ".repeat(max_path - ep.path.len() + 2);
        let summary = ep.summary.as_deref().unwrap_or("");
        out.push_str(&format!(
            "  {}{}{}{}{}",
            ep.method, method_pad, ep.path, path_pad, summary
        ));
        out.push('\n');
    }

    out.push_str("\nDrill deeper:\n");
    for ep in &endpoints {
        out.push_str(&format!(
            "  phyllotaxis resources {} {} {}\n",
            slug, ep.method, ep.path
        ));
    }

    Ok(out)
}

struct EndpointInfo {
    method: String,
    path: String,
    summary: Option<String>,
}

fn collect_endpoints_for_tag(spec: &OpenAPI, tag_name: &str) -> Vec<EndpointInfo> {
    let mut endpoints = Vec::new();
    let methods = ["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"];

    for (path, item) in spec.paths.iter() {
        if let openapiv3::ReferenceOr::Item(path_item) = item {
            let ops: Vec<(&str, Option<&openapiv3::Operation>)> = vec![
                ("GET", path_item.get.as_ref()),
                ("POST", path_item.post.as_ref()),
                ("PUT", path_item.put.as_ref()),
                ("PATCH", path_item.patch.as_ref()),
                ("DELETE", path_item.delete.as_ref()),
                ("OPTIONS", path_item.options.as_ref()),
                ("HEAD", path_item.head.as_ref()),
            ];

            for (method, op) in ops {
                if let Some(operation) = op {
                    if operation.tags.iter().any(|t| t == tag_name) {
                        endpoints.push(EndpointInfo {
                            method: method.to_string(),
                            path: path.clone(),
                            summary: operation.summary.clone(),
                        });
                    }
                }
            }
        }
    }

    // Sort by path, then method order
    endpoints.sort_by(|a, b| {
        a.path.cmp(&b.path).then_with(|| {
            let ai = methods.iter().position(|m| *m == a.method).unwrap_or(99);
            let bi = methods.iter().position(|m| *m == b.method).unwrap_or(99);
            ai.cmp(&bi)
        })
    });

    endpoints
}
```

**Step 3: Run tests**

```bash
cargo test test_resource_detail
```

Expected: PASS.

**Step 4: Wire into main.rs**

```rust
cli::Command::ResourceDetail { slug } => {
    print!("{}", phyllotaxis::output::resources::render_detail(&spec, &slug)?);
}
```

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: Level 2 resource detail with endpoint listing"
```

---

### Task 9: Output — Level 3 (Endpoint Detail)

**Files:**
- Create: `src/output/endpoint.rs`
- Create: `tests/output_endpoint.rs`
- Modify: `src/output/mod.rs`
- Modify: `src/main.rs`

This is the most complex renderer. It needs to show: method + path, description, auth, parameters, request body fields, response codes, and drill-deeper hints to related schemas.

**Step 1: Write failing test**

Create `tests/output_endpoint.rs`:

```rust
use std::path::Path;

#[test]
fn test_endpoint_detail_get() {
    let spec = phyllotaxis::spec::load(Path::new("tests/fixtures/minimal.yaml")).unwrap();
    let output = phyllotaxis::output::endpoint::render(&spec, "pets", "GET", "/pets/{petId}").unwrap();

    assert!(output.contains("GET /pets/{petId}"));
    assert!(output.contains("petId"));
    assert!(output.contains("path"));
}

#[test]
fn test_endpoint_not_found() {
    let spec = phyllotaxis::spec::load(Path::new("tests/fixtures/minimal.yaml")).unwrap();
    let result = phyllotaxis::output::endpoint::render(&spec, "pets", "GET", "/nonexistent");
    assert!(result.is_err());
}
```

**Step 2: Implement endpoint detail renderer**

Create `src/output/endpoint.rs`:

```rust
use anyhow::{bail, Result};
use openapiv3::*;

pub fn render(spec: &OpenAPI, _resource_slug: &str, method: &str, path: &str) -> Result<String> {
    // Find the path item
    let path_item = match spec.paths.get(path) {
        Some(ReferenceOr::Item(item)) => item,
        _ => bail!("Path '{}' not found. Check the path and try again.", path),
    };

    // Find the operation for the given method
    let operation = match method.to_uppercase().as_str() {
        "GET" => path_item.get.as_ref(),
        "POST" => path_item.post.as_ref(),
        "PUT" => path_item.put.as_ref(),
        "PATCH" => path_item.patch.as_ref(),
        "DELETE" => path_item.delete.as_ref(),
        "OPTIONS" => path_item.options.as_ref(),
        "HEAD" => path_item.head.as_ref(),
        _ => bail!("Unknown HTTP method: {}", method),
    }
    .ok_or_else(|| anyhow::anyhow!("{} {} not found", method, path))?;

    let mut out = String::new();

    // Header
    out.push_str(&format!("{} {}\n", method.to_uppercase(), path));
    if let Some(ref summary) = operation.summary {
        out.push_str(&format!("{}\n", summary));
    }
    if let Some(ref desc) = operation.description {
        if operation.summary.as_deref() != Some(desc.as_str()) {
            out.push_str(&format!("{}\n", desc));
        }
    }

    // Auth
    let auth = describe_auth(spec, operation);
    if !auth.is_empty() {
        out.push_str(&format!("\nAuthentication: {}\n", auth));
    }

    // Parameters
    let params: Vec<&Parameter> = operation
        .parameters
        .iter()
        .chain(path_item.parameters.iter())
        .filter_map(|p| match p {
            ReferenceOr::Item(param) => Some(param),
            _ => None,
        })
        .collect();

    if !params.is_empty() {
        out.push_str("\nParameters:\n");
        for param in &params {
            let param_data = match param {
                Parameter::Query { parameter_data, .. } => Some((parameter_data, "query")),
                Parameter::Header { parameter_data, .. } => Some((parameter_data, "header")),
                Parameter::Path { parameter_data, .. } => Some((parameter_data, "path")),
                Parameter::Cookie { parameter_data, .. } => Some((parameter_data, "cookie")),
            };

            if let Some((data, location)) = param_data {
                let required = if data.required { "(required)" } else { "(optional)" };
                let desc = data.description.as_deref().unwrap_or("");
                out.push_str(&format!(
                    "  {}  {}  {}  {}\n",
                    data.name, location, required, desc
                ));
            }
        }
    }

    // Request body
    if let Some(ref body_ref) = operation.request_body {
        if let ReferenceOr::Item(body) = body_ref {
            out.push_str("\nRequest Body");
            if let Some((content_type, _media)) = body.content.first() {
                out.push_str(&format!(" ({}):\n", content_type));
            } else {
                out.push_str(":\n");
            }

            // Try to render schema fields from the first content type
            if let Some((_ct, media_type)) = body.content.first() {
                if let Some(ref schema_ref) = media_type.schema {
                    render_schema_fields(&mut out, spec, schema_ref, "  ");
                }
            }
        }
    }

    // Responses
    out.push_str("\nResponses:\n");
    for (status, response_ref) in &operation.responses.responses {
        let status_str = match status {
            StatusCode::Code(code) => code.to_string(),
            StatusCode::Range(range) => format!("{}xx", range),
        };
        match response_ref {
            ReferenceOr::Item(response) => {
                out.push_str(&format!("  {}  {}\n", status_str, response.description));
            }
            ReferenceOr::Reference { reference } => {
                out.push_str(&format!("  {}  (see {})\n", status_str, reference));
            }
        }
    }
    if let Some(ref default) = operation.responses.default {
        match default {
            ReferenceOr::Item(response) => {
                out.push_str(&format!("  default  {}\n", response.description));
            }
            _ => {}
        }
    }

    // Drill deeper hints (referenced schemas)
    let schema_refs = collect_referenced_schemas(spec, operation);
    if !schema_refs.is_empty() {
        out.push_str("\nDrill deeper:\n");
        for schema_name in &schema_refs {
            out.push_str(&format!("  phyllotaxis schemas {}\n", schema_name));
        }
    }

    Ok(out)
}

fn describe_auth(spec: &OpenAPI, operation: &Operation) -> String {
    // Use operation-level security if defined, else global
    let security = if !operation.security.is_empty() {
        &operation.security
    } else {
        &spec.security
    };

    if security.is_empty() {
        return "None".to_string();
    }

    let schemes: Vec<String> = security
        .iter()
        .flat_map(|req| req.keys())
        .cloned()
        .collect();

    if schemes.is_empty() {
        "None".to_string()
    } else {
        format!("{} (required)", schemes.join(", "))
    }
}

/// Render schema fields one level deep
fn render_schema_fields(out: &mut String, spec: &OpenAPI, schema_ref: &ReferenceOr<Schema>, indent: &str) {
    let schema = match resolve_schema(spec, schema_ref) {
        Some(s) => s,
        None => return,
    };

    match &schema.schema_kind {
        SchemaKind::Type(Type::Object(obj)) => {
            let required_fields: std::collections::HashSet<&str> =
                obj.required.iter().map(|s| s.as_str()).collect();

            for (name, prop_ref) in &obj.properties {
                let type_str = schema_type_name(spec, prop_ref);
                let required = if required_fields.contains(name.as_str()) {
                    "(required)"
                } else {
                    "(optional)"
                };

                // Check for read-only
                let read_only = match prop_ref {
                    ReferenceOr::Item(box_schema) => {
                        box_schema.schema_data.read_only
                    }
                    _ => false,
                };

                let qualifier = if read_only {
                    "(read-only)"
                } else {
                    required
                };

                let desc = match prop_ref {
                    ReferenceOr::Item(s) => s.schema_data.description.as_deref().unwrap_or(""),
                    _ => "",
                };

                out.push_str(&format!(
                    "{}{}  {}  {}  {}\n",
                    indent, name, type_str, qualifier, desc
                ));
            }
        }
        _ => {
            out.push_str(&format!("{}(schema type not object)\n", indent));
        }
    }
}

/// Get a human-readable type name for a schema reference
fn schema_type_name(spec: &OpenAPI, schema_ref: &ReferenceOr<Box<Schema>>) -> String {
    match schema_ref {
        ReferenceOr::Reference { reference } => {
            // Extract name from "#/components/schemas/FooBar"
            reference
                .rsplit('/')
                .next()
                .unwrap_or("object")
                .to_string()
        }
        ReferenceOr::Item(schema) => match &schema.schema_kind {
            SchemaKind::Type(Type::String(_)) => "string".to_string(),
            SchemaKind::Type(Type::Integer(_)) => "integer".to_string(),
            SchemaKind::Type(Type::Number(_)) => "number".to_string(),
            SchemaKind::Type(Type::Boolean {}) => "boolean".to_string(),
            SchemaKind::Type(Type::Array(arr)) => {
                let inner = arr.items.as_ref().map_or("any".to_string(), |items| {
                    match items.as_ref() {
                        ReferenceOr::Reference { reference } => {
                            let name = reference.rsplit('/').next().unwrap_or("object");
                            format!("{}[]", name)
                        }
                        _ => "array".to_string(),
                    }
                });
                inner
            }
            SchemaKind::Type(Type::Object(_)) => "object".to_string(),
            _ => "any".to_string(),
        },
    }
}

/// Resolve a schema reference to the actual Schema
fn resolve_schema<'a>(spec: &'a OpenAPI, schema_ref: &'a ReferenceOr<Schema>) -> Option<&'a Schema> {
    match schema_ref {
        ReferenceOr::Item(schema) => Some(schema),
        ReferenceOr::Reference { reference } => {
            let name = reference.rsplit('/').next()?;
            let components = spec.components.as_ref()?;
            match components.schemas.get(name)? {
                ReferenceOr::Item(schema) => Some(schema),
                _ => None,
            }
        }
    }
}

/// Collect names of schemas referenced by an operation (for drill-deeper hints)
fn collect_referenced_schemas(_spec: &OpenAPI, operation: &Operation) -> Vec<String> {
    let mut refs = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // From request body
    if let Some(ReferenceOr::Item(body)) = &operation.request_body {
        for (_ct, media) in &body.content {
            if let Some(schema_ref) = &media.schema {
                collect_refs_from_schema(schema_ref, &mut refs, &mut seen);
            }
        }
    }

    // From responses
    for (_status, resp_ref) in &operation.responses.responses {
        if let ReferenceOr::Item(response) = resp_ref {
            for (_ct, media) in &response.content {
                if let Some(schema_ref) = &media.schema {
                    collect_refs_from_schema(schema_ref, &mut refs, &mut seen);
                }
            }
        }
    }

    refs
}

fn collect_refs_from_schema(
    schema_ref: &ReferenceOr<Schema>,
    refs: &mut Vec<String>,
    seen: &mut std::collections::HashSet<String>,
) {
    if let ReferenceOr::Reference { reference } = schema_ref {
        if let Some(name) = reference.rsplit('/').next() {
            if seen.insert(name.to_string()) {
                refs.push(name.to_string());
            }
        }
    }
}
```

Update `src/output/mod.rs`:

```rust
pub mod endpoint;
pub mod overview;
pub mod resources;
```

**Step 3: Run tests**

```bash
cargo test test_endpoint
```

Expected: PASS. There will likely be compiler issues with exact `openapiv3` types — the implementer should fix type mismatches based on compiler errors. Key things to watch:
- `Schema` vs `Box<Schema>` in references
- `security` might be `Option<Vec<...>>` or `Vec<...>` depending on version
- `StatusCode` enum variants

**Step 4: Wire into main.rs**

```rust
cli::Command::EndpointDetail { slug, method, path } => {
    print!("{}", phyllotaxis::output::endpoint::render(&spec, &slug, &method, &path)?);
}
```

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: Level 3 endpoint detail with params, body, responses"
```

---

### Task 10: Output — Schema List and Schema Detail

**Files:**
- Create: `src/output/schemas.rs`
- Create: `tests/output_schemas.rs`
- Modify: `src/output/mod.rs`
- Modify: `src/main.rs`

**Step 1: Write failing tests**

Create `tests/output_schemas.rs`:

```rust
use std::path::Path;

#[test]
fn test_schema_list() {
    let spec = phyllotaxis::spec::load(Path::new("tests/fixtures/minimal.yaml")).unwrap();
    let output = phyllotaxis::output::schemas::render_list(&spec);

    assert!(output.contains("Pet"));
    assert!(output.contains("phyllotaxis schemas <name>"));
}

#[test]
fn test_schema_detail() {
    let spec = phyllotaxis::spec::load(Path::new("tests/fixtures/minimal.yaml")).unwrap();
    let output = phyllotaxis::output::schemas::render_detail(&spec, "Pet", false).unwrap();

    assert!(output.contains("Schema: Pet"));
    assert!(output.contains("id"));
    assert!(output.contains("name"));
    assert!(output.contains("integer"));
    assert!(output.contains("string"));
}

#[test]
fn test_schema_not_found() {
    let spec = phyllotaxis::spec::load(Path::new("tests/fixtures/minimal.yaml")).unwrap();
    let result = phyllotaxis::output::schemas::render_detail(&spec, "Nonexistent", false);
    assert!(result.is_err());
}
```

**Step 2: Implement schema renderers**

Create `src/output/schemas.rs`:

```rust
use anyhow::Result;
use openapiv3::*;

/// Render list of all schemas
pub fn render_list(spec: &OpenAPI) -> String {
    let mut out = String::new();
    out.push_str("Schemas:\n");

    let schemas = match &spec.components {
        Some(components) => &components.schemas,
        None => {
            out.push_str("  (no schemas defined)\n");
            return out;
        }
    };

    // Find max name width for alignment
    let max_name = schemas.keys().map(|k| k.len()).max().unwrap_or(0);

    for (name, schema_ref) in schemas {
        let desc = match schema_ref {
            ReferenceOr::Item(schema) => schema
                .schema_data
                .description
                .as_deref()
                .unwrap_or(""),
            _ => "",
        };
        let padding = " ".repeat(max_name - name.len() + 2);
        out.push_str(&format!("  {}{}{}\n", name, padding, desc));
    }

    out.push_str("\nDrill deeper:\n");
    out.push_str("  phyllotaxis schemas <name>\n");

    out
}

/// Render detail for a specific schema
pub fn render_detail(spec: &OpenAPI, name: &str, expand: bool) -> Result<String> {
    let components = spec
        .components
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No components defined in spec"))?;

    let schema_ref = components
        .schemas
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("Schema '{}' not found. Run `phyllotaxis schemas` to see available schemas.", name))?;

    let schema = match schema_ref {
        ReferenceOr::Item(s) => s,
        ReferenceOr::Reference { reference } => {
            return Ok(format!("Schema: {} (reference to {})\n", name, reference));
        }
    };

    let mut out = String::new();

    if expand {
        out.push_str(&format!("Schema: {} (expanded)\n", name));
    } else {
        out.push_str(&format!("Schema: {}\n", name));
    }

    if let Some(ref desc) = schema.schema_data.description {
        out.push_str(&format!("{}\n", desc));
    }

    out.push('\n');

    // Render fields
    let mut related = Vec::new();
    render_fields(&mut out, spec, schema, "  ", expand, &mut related, 0);

    // Related schemas (drill-deeper hints)
    if !related.is_empty() && !expand {
        out.push_str("\nRelated schemas:\n");
        let mut seen = std::collections::HashSet::new();
        for schema_name in &related {
            if seen.insert(schema_name) {
                out.push_str(&format!("  phyllotaxis schemas {}\n", schema_name));
            }
        }
    }

    Ok(out)
}

fn render_fields(
    out: &mut String,
    spec: &OpenAPI,
    schema: &Schema,
    indent: &str,
    expand: bool,
    related: &mut Vec<String>,
    depth: usize,
) {
    match &schema.schema_kind {
        SchemaKind::Type(Type::Object(obj)) => {
            let required: std::collections::HashSet<&str> =
                obj.required.iter().map(|s| s.as_str()).collect();

            out.push_str(&format!("{}Fields:\n", indent));

            for (field_name, prop_ref) in &obj.properties {
                let (type_str, ref_name) = field_type_and_ref(spec, prop_ref);
                let is_required = required.contains(field_name.as_str());

                let read_only = match prop_ref {
                    ReferenceOr::Item(s) => s.schema_data.read_only,
                    _ => false,
                };

                let qualifier = if read_only {
                    "(read-only)"
                } else if is_required {
                    "(required)"
                } else {
                    "(optional)"
                };

                let desc = match prop_ref {
                    ReferenceOr::Item(s) => s.schema_data.description.as_deref().unwrap_or(""),
                    _ => "",
                };

                out.push_str(&format!(
                    "{}  {}  {}  {}  {}\n",
                    indent, field_name, type_str, qualifier, desc
                ));

                // Track related schemas
                if let Some(ref rn) = ref_name {
                    related.push(rn.clone());

                    // If expanding, inline the referenced schema
                    if expand && depth < 3 {
                        if let Some(nested) = resolve_schema_by_name(spec, rn) {
                            render_fields(
                                out,
                                spec,
                                nested,
                                &format!("{}    ", indent),
                                expand,
                                related,
                                depth + 1,
                            );
                        }
                    }
                }
            }
        }
        SchemaKind::Type(Type::Array(arr)) => {
            out.push_str(&format!("{}Type: array\n", indent));
            if let Some(ref items) = arr.items {
                let (type_str, ref_name) = field_type_and_ref_schema(spec, items.as_ref());
                out.push_str(&format!("{}Items: {}\n", indent, type_str));
                if let Some(rn) = ref_name {
                    related.push(rn);
                }
            }
        }
        SchemaKind::AllOf { all_of } => {
            out.push_str(&format!("{}Composed of (allOf):\n", indent));
            for sub in all_of {
                if let ReferenceOr::Reference { reference } = sub {
                    let name = reference.rsplit('/').next().unwrap_or("?");
                    out.push_str(&format!("{}  - {}\n", indent, name));
                    related.push(name.to_string());
                }
            }
        }
        SchemaKind::OneOf { one_of } => {
            out.push_str(&format!("{}One of:\n", indent));
            for sub in one_of {
                if let ReferenceOr::Reference { reference } = sub {
                    let name = reference.rsplit('/').next().unwrap_or("?");
                    out.push_str(&format!("{}  - {}\n", indent, name));
                    related.push(name.to_string());
                }
            }
        }
        _ => {
            out.push_str(&format!("{}(non-object schema)\n", indent));
        }
    }
}

fn field_type_and_ref(
    spec: &OpenAPI,
    prop_ref: &ReferenceOr<Box<Schema>>,
) -> (String, Option<String>) {
    match prop_ref {
        ReferenceOr::Reference { reference } => {
            let name = reference.rsplit('/').next().unwrap_or("object").to_string();
            (name.clone(), Some(name))
        }
        ReferenceOr::Item(schema) => match &schema.schema_kind {
            SchemaKind::Type(Type::String(s)) => {
                let fmt = s.format.as_str();
                if fmt.is_empty() {
                    ("string".to_string(), None)
                } else {
                    (format!("string({})", fmt), None)
                }
            }
            SchemaKind::Type(Type::Integer(i)) => {
                let fmt = i.format.as_str();
                if fmt.is_empty() {
                    ("integer".to_string(), None)
                } else {
                    (format!("integer({})", fmt), None)
                }
            }
            SchemaKind::Type(Type::Number(_)) => ("number".to_string(), None),
            SchemaKind::Type(Type::Boolean {}) => ("boolean".to_string(), None),
            SchemaKind::Type(Type::Array(arr)) => {
                if let Some(ref items) = arr.items {
                    let (inner, ref_name) = field_type_and_ref_schema(spec, items.as_ref());
                    (format!("{}[]", inner), ref_name)
                } else {
                    ("array".to_string(), None)
                }
            }
            SchemaKind::Type(Type::Object(_)) => ("object".to_string(), None),
            _ => ("any".to_string(), None),
        },
    }
}

fn field_type_and_ref_schema(
    _spec: &OpenAPI,
    schema_ref: &ReferenceOr<Schema>,
) -> (String, Option<String>) {
    match schema_ref {
        ReferenceOr::Reference { reference } => {
            let name = reference.rsplit('/').next().unwrap_or("object").to_string();
            (name.clone(), Some(name))
        }
        _ => ("object".to_string(), None),
    }
}

fn resolve_schema_by_name<'a>(spec: &'a OpenAPI, name: &str) -> Option<&'a Schema> {
    let components = spec.components.as_ref()?;
    match components.schemas.get(name)? {
        ReferenceOr::Item(schema) => Some(schema),
        _ => None,
    }
}
```

Update `src/output/mod.rs`:

```rust
pub mod endpoint;
pub mod overview;
pub mod resources;
pub mod schemas;
```

**Step 3: Run tests**

```bash
cargo test test_schema
```

Expected: PASS.

**Step 4: Wire into main.rs**

```rust
cli::Command::SchemaList => {
    print!("{}", phyllotaxis::output::schemas::render_list(&spec));
}
cli::Command::SchemaDetail { name } => {
    print!("{}", phyllotaxis::output::schemas::render_detail(&spec, &name, cli.expand)?);
}
```

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: schema list and detail views with expand support"
```

---

### Task 11: Output — Auth and Search

**Files:**
- Create: `src/output/auth.rs`
- Create: `src/output/search.rs`
- Create: `tests/output_auth.rs`
- Create: `tests/output_search.rs`
- Modify: `src/output/mod.rs`
- Modify: `src/main.rs`

**Step 1: Write failing tests**

Create `tests/output_auth.rs`:

```rust
use std::path::Path;

#[test]
fn test_auth_output() {
    let spec = phyllotaxis::spec::load(Path::new("tests/fixtures/minimal.yaml")).unwrap();
    let output = phyllotaxis::output::auth::render(&spec);

    assert!(output.contains("bearerAuth"));
    assert!(output.contains("bearer"));
}
```

Create `tests/output_search.rs`:

```rust
use std::path::Path;

#[test]
fn test_search_finds_resources() {
    let spec = phyllotaxis::spec::load(Path::new("tests/fixtures/minimal.yaml")).unwrap();
    let output = phyllotaxis::output::search::render(&spec, "pet");

    assert!(output.contains("Pets"));
    assert!(output.contains("/pets"));
}

#[test]
fn test_search_no_results() {
    let spec = phyllotaxis::spec::load(Path::new("tests/fixtures/minimal.yaml")).unwrap();
    let output = phyllotaxis::output::search::render(&spec, "zzzznonexistent");

    assert!(output.contains("No results"));
}
```

**Step 2: Implement auth renderer**

Create `src/output/auth.rs`:

```rust
use openapiv3::*;

pub fn render(spec: &OpenAPI) -> String {
    let mut out = String::new();
    out.push_str("Authentication:\n\n");

    let components = match &spec.components {
        Some(c) => c,
        None => {
            out.push_str("  No authentication schemes defined.\n");
            return out;
        }
    };

    if components.security_schemes.is_empty() {
        out.push_str("  No authentication schemes defined.\n");
        return out;
    }

    for (name, scheme_ref) in &components.security_schemes {
        match scheme_ref {
            ReferenceOr::Item(scheme) => {
                out.push_str(&format!("  {}:\n", name));
                match scheme {
                    SecurityScheme::HTTP { scheme, bearer_format, .. } => {
                        out.push_str(&format!("    Type: HTTP\n"));
                        out.push_str(&format!("    Scheme: {}\n", scheme));
                        if let Some(fmt) = bearer_format {
                            out.push_str(&format!("    Bearer format: {}\n", fmt));
                        }
                    }
                    SecurityScheme::APIKey { location, name: key_name, .. } => {
                        out.push_str(&format!("    Type: API Key\n"));
                        out.push_str(&format!("    In: {:?}\n", location));
                        out.push_str(&format!("    Name: {}\n", key_name));
                    }
                    SecurityScheme::OAuth2 { flows, .. } => {
                        out.push_str(&format!("    Type: OAuth2\n"));
                        if let Some(ref cc) = flows.client_credentials {
                            out.push_str(&format!("    Client credentials: {}\n", cc.token_url));
                        }
                        if let Some(ref ac) = flows.authorization_code {
                            out.push_str(&format!("    Auth URL: {}\n", ac.authorization_url));
                            out.push_str(&format!("    Token URL: {}\n", ac.token_url));
                        }
                    }
                    SecurityScheme::OpenIDConnect { open_id_connect_url, .. } => {
                        out.push_str(&format!("    Type: OpenID Connect\n"));
                        out.push_str(&format!("    URL: {}\n", open_id_connect_url));
                    }
                }
                out.push('\n');
            }
            ReferenceOr::Reference { reference } => {
                out.push_str(&format!("  {}: (see {})\n\n", name, reference));
            }
        }
    }

    // Global security requirements
    if !spec.security.is_empty() {
        out.push_str("Global security requirements:\n");
        for req in &spec.security {
            for (scheme, scopes) in req {
                if scopes.is_empty() {
                    out.push_str(&format!("  - {}\n", scheme));
                } else {
                    out.push_str(&format!("  - {} (scopes: {})\n", scheme, scopes.join(", ")));
                }
            }
        }
    }

    out
}
```

**Step 3: Implement search renderer**

Create `src/output/search.rs`:

```rust
use crate::slug::slugify;
use openapiv3::*;

pub fn render(spec: &OpenAPI, term: &str) -> String {
    let term_lower = term.to_lowercase();
    let mut out = String::new();
    out.push_str(&format!("Results for \"{}\":\n", term));

    let mut has_results = false;

    // Search tags/resources
    let mut matching_tags = Vec::new();
    for tag in &spec.tags {
        if tag.name.to_lowercase().contains(&term_lower)
            || tag
                .description
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains(&term_lower)
        {
            matching_tags.push(tag);
        }
    }
    if !matching_tags.is_empty() {
        has_results = true;
        out.push_str("\nResources:\n");
        for tag in &matching_tags {
            let slug = slugify(&tag.name);
            let desc = tag.description.as_deref().unwrap_or("");
            out.push_str(&format!("  {}  {}\n", slug, desc));
        }
    }

    // Search endpoints
    let mut matching_endpoints = Vec::new();
    for (path, item) in spec.paths.iter() {
        if let ReferenceOr::Item(path_item) = item {
            let ops: Vec<(&str, Option<&Operation>)> = vec![
                ("GET", path_item.get.as_ref()),
                ("POST", path_item.post.as_ref()),
                ("PUT", path_item.put.as_ref()),
                ("PATCH", path_item.patch.as_ref()),
                ("DELETE", path_item.delete.as_ref()),
            ];
            for (method, op) in ops {
                if let Some(operation) = op {
                    let searchable = format!(
                        "{} {} {} {} {}",
                        path,
                        method,
                        operation.summary.as_deref().unwrap_or(""),
                        operation.description.as_deref().unwrap_or(""),
                        operation.operation_id.as_deref().unwrap_or(""),
                    );
                    if searchable.to_lowercase().contains(&term_lower) {
                        matching_endpoints.push((
                            method.to_string(),
                            path.clone(),
                            operation.summary.clone(),
                            operation.tags.first().map(|t| slugify(t)),
                        ));
                    }
                }
            }
        }
    }
    if !matching_endpoints.is_empty() {
        has_results = true;
        out.push_str("\nEndpoints:\n");
        for (method, path, summary, _tag) in &matching_endpoints {
            let summary_str = summary.as_deref().unwrap_or("");
            out.push_str(&format!("  {}  {}  {}\n", method, path, summary_str));
        }
    }

    // Search schemas
    let mut matching_schemas = Vec::new();
    if let Some(ref components) = spec.components {
        for (name, _schema_ref) in &components.schemas {
            if name.to_lowercase().contains(&term_lower) {
                matching_schemas.push(name.clone());
            }
        }
    }
    if !matching_schemas.is_empty() {
        has_results = true;
        out.push_str("\nSchemas:\n");
        for name in &matching_schemas {
            out.push_str(&format!("  {}\n", name));
        }
    }

    if !has_results {
        out.push_str("\nNo results found.\n");
        return out;
    }

    // Drill deeper hints
    out.push_str("\nDrill deeper:\n");
    for tag in &matching_tags {
        out.push_str(&format!("  phyllotaxis resources {}\n", slugify(&tag.name)));
    }
    for name in &matching_schemas {
        out.push_str(&format!("  phyllotaxis schemas {}\n", name));
    }

    out
}
```

Update `src/output/mod.rs`:

```rust
pub mod auth;
pub mod endpoint;
pub mod overview;
pub mod resources;
pub mod schemas;
pub mod search;
```

**Step 4: Run tests**

```bash
cargo test
```

Expected: All pass.

**Step 5: Wire remaining commands in main.rs**

```rust
cli::Command::Auth => {
    print!("{}", phyllotaxis::output::auth::render(&spec));
}
cli::Command::Search { term } => {
    print!("{}", phyllotaxis::output::search::render(&spec, &term));
}
```

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: auth detail and search commands"
```

---

### Task 12: JSON Output Flag

**Files:**
- Modify: `src/main.rs`
- Create: `tests/json_output.rs`

**Step 1: Write failing test**

Create `tests/json_output.rs`:

```rust
use assert_cmd::Command;

#[test]
fn test_json_flag_outputs_valid_json() {
    let mut cmd = Command::cargo_bin("phyllotaxis").unwrap();
    let output = cmd
        .args(&["--spec", "tests/fixtures/minimal.yaml", "--json", "schemas"])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("--json flag should produce valid JSON");
    assert!(parsed.is_object() || parsed.is_array());
}
```

**Step 2: Implement JSON output mode**

The simplest approach: when `--json` is set, serialize the relevant portion of the parsed `OpenAPI` struct directly.

Add a JSON output module. Create `src/output/json.rs`:

```rust
use anyhow::Result;
use openapiv3::*;
use serde_json::{json, Value};
use crate::slug::slugify;
use crate::cli::Command;

pub fn render(spec: &OpenAPI, command: &Command, expand: bool) -> Result<String> {
    let value = match command {
        Command::Overview => json!({
            "title": spec.info.title,
            "version": spec.info.version,
            "base_url": spec.servers.first().map(|s| &s.url),
            "tag_count": spec.tags.len(),
            "schema_count": spec.components.as_ref().map(|c| c.schemas.len()).unwrap_or(0),
        }),
        Command::ResourceList => {
            let tags: Vec<Value> = spec.tags.iter().map(|t| {
                json!({
                    "name": t.name,
                    "slug": slugify(&t.name),
                    "description": t.description,
                })
            }).collect();
            json!({ "resources": tags })
        }
        Command::SchemaList => {
            let schemas: Vec<Value> = spec.components.as_ref()
                .map(|c| c.schemas.keys().map(|k| json!(k)).collect())
                .unwrap_or_default();
            json!({ "schemas": schemas })
        }
        // For other commands, fall back to a simple message
        _ => json!({ "message": "JSON output not yet implemented for this command" }),
    };

    Ok(serde_json::to_string_pretty(&value)?)
}
```

Update `src/output/mod.rs`:

```rust
pub mod auth;
pub mod endpoint;
pub mod json;
pub mod overview;
pub mod resources;
pub mod schemas;
pub mod search;
```

Update `main.rs` to check the `--json` flag before dispatching:

```rust
if cli.json {
    let command = cli::Command::parse(&cli.args);
    println!("{}", phyllotaxis::output::json::render(&spec, &command, cli.expand)?);
    return Ok(());
}
```

**Step 3: Run tests**

```bash
cargo test test_json
```

Expected: PASS.

**Step 4: Commit**

```bash
git add -A
git commit -m "feat: --json output flag"
```

---

### Task 13: Integration Test with Full CLI

**Files:**
- Create: `tests/cli_integration.rs`

**Step 1: Write integration tests**

```rust
use assert_cmd::Command;
use predicates::prelude::*;

fn phyllotaxis() -> Command {
    let mut cmd = Command::cargo_bin("phyllotaxis").unwrap();
    cmd.args(&["--spec", "tests/fixtures/minimal.yaml"]);
    cmd
}

#[test]
fn test_overview() {
    phyllotaxis()
        .assert()
        .success()
        .stdout(predicate::str::contains("API: Test API"))
        .stdout(predicate::str::contains("phyllotaxis resources"));
}

#[test]
fn test_resource_list() {
    phyllotaxis()
        .arg("resources")
        .assert()
        .success()
        .stdout(predicate::str::contains("pets"));
}

#[test]
fn test_resource_detail() {
    phyllotaxis()
        .args(&["resources", "pets"])
        .assert()
        .success()
        .stdout(predicate::str::contains("GET"))
        .stdout(predicate::str::contains("/pets"));
}

#[test]
fn test_schema_list() {
    phyllotaxis()
        .arg("schemas")
        .assert()
        .success()
        .stdout(predicate::str::contains("Pet"));
}

#[test]
fn test_schema_detail() {
    phyllotaxis()
        .args(&["schemas", "Pet"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Schema: Pet"))
        .stdout(predicate::str::contains("name"));
}

#[test]
fn test_auth() {
    phyllotaxis()
        .arg("auth")
        .assert()
        .success()
        .stdout(predicate::str::contains("bearerAuth"));
}

#[test]
fn test_search() {
    phyllotaxis()
        .args(&["search", "pet"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Results for"))
        .stdout(predicate::str::contains("Pet"));
}

#[test]
fn test_no_spec_found() {
    Command::cargo_bin("phyllotaxis")
        .unwrap()
        .current_dir(std::env::temp_dir())
        .assert()
        .failure()
        .stderr(predicate::str::contains("No OpenAPI spec found"));
}
```

**Step 2: Run all tests**

```bash
cargo test
```

Expected: All pass.

**Step 3: Commit**

```bash
git add -A
git commit -m "feat: CLI integration tests"
```

---

### Task 14: Polish and README

**Files:**
- Create: `README.md`

**Step 1: Create README**

```markdown
# Phyllotaxis

Progressive disclosure for OpenAPI specs. Built for LLM agents.

## Install

```bash
cargo install phyllotaxis
```

## Quick Start

```bash
# Auto-detect openapi.yaml in current directory
phyllotaxis

# Or point at a specific spec
phyllotaxis --spec path/to/openapi.yaml

# Save config so it works everywhere in the repo
phyllotaxis init
```

## Usage

```bash
phyllotaxis                                       # API overview
phyllotaxis resources                             # List all resource groups
phyllotaxis resources access-policies             # Endpoints for a resource
phyllotaxis resources access-policies GET /access-policies  # Full endpoint detail
phyllotaxis schemas                               # List all data models
phyllotaxis schemas AccessPolicy                  # Schema detail
phyllotaxis schemas AccessPolicy --expand         # Schema with nested types inlined
phyllotaxis auth                                  # Authentication details
phyllotaxis search workload                       # Search across everything
```

Every output includes drill-deeper hints showing the exact commands for the next level.

## Flags

- `--spec <path>` — Override spec file location
- `--json` — Output in JSON format
- `--expand` — Recursively inline nested schema definitions

## Why "Phyllotaxis"?

From Ancient Greek *phullon* (leaf) + *taxis* (arrangement). In botany, phyllotaxis is the mathematical pattern governing how leaves arrange on a stem — the algorithm behind Romanesco's fractal spirals.

This tool reveals the organized arrangement of an API's structure, navigable by pattern.
```

**Step 2: Commit**

```bash
git add -A
git commit -m "docs: README"
```

---

## Summary

| Task | What it builds | Key files |
|------|---------------|-----------|
| 1 | Project scaffold | `Cargo.toml`, `src/main.rs` |
| 2 | Spec loading (YAML/JSON) | `src/spec.rs` |
| 3 | Spec discovery (config + auto-detect) | `src/discovery.rs` |
| 4 | Tag name slugification | `src/slug.rs` |
| 5 | CLI argument parsing | `src/cli.rs` |
| 6 | Level 0: API overview | `src/output/overview.rs` |
| 7 | Level 1: Resource list | `src/output/resources.rs` |
| 8 | Level 2: Resource detail (endpoints) | `src/output/resources.rs` |
| 9 | Level 3: Endpoint detail | `src/output/endpoint.rs` |
| 10 | Schema list + detail + expand | `src/output/schemas.rs` |
| 11 | Auth + search commands | `src/output/auth.rs`, `src/output/search.rs` |
| 12 | JSON output flag | `src/output/json.rs` |
| 13 | CLI integration tests | `tests/cli_integration.rs` |
| 14 | README | `README.md` |