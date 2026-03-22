use phyllotaxis::{commands, render, spec};

use anyhow::Context;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "phyllotaxis",
    version,
    about = "Progressive disclosure for OpenAPI documents (alias: phyll)"
)]
struct Cli {
    /// OpenAPI document path or URL (overrides config/env/auto-detect)
    document: Option<String>,

    /// List resource groups, or drill into a specific resource
    #[arg(long, num_args(0..=1), default_missing_value = "")]
    resources: Option<String>,

    /// List schemas, or view a specific schema
    #[arg(long, num_args(0..=1), default_missing_value = "")]
    schemas: Option<String>,

    /// Show authentication details
    #[arg(long)]
    auth: bool,

    /// List callbacks, or view a specific callback
    #[arg(long, num_args(0..=1), default_missing_value = "")]
    callbacks: Option<String>,

    /// Show endpoint detail: --endpoint METHOD PATH
    #[arg(long, num_args(2))]
    endpoint: Option<Vec<String>>,

    /// Show which endpoints use this schema (use with --schemas NAME)
    #[arg(long)]
    used_by: bool,

    /// Output in JSON format (default when stdout is piped)
    #[arg(long, global = true, conflicts_with = "text")]
    json: bool,

    /// Output in text format (default when stdout is a terminal)
    #[arg(long, global = true, conflicts_with = "json")]
    text: bool,

    /// Recursively inline nested schemas (max depth 5)
    #[arg(long, global = true)]
    expand: bool,

    /// Cap the number of related schemas shown in schema detail
    #[arg(long, global = true)]
    related_limit: Option<usize>,

    /// Extract a subtree by JSON Pointer (RFC 6901), e.g. '/components/schemas/Pet'
    #[arg(long, value_name = "POINTER")]
    r#for: Option<String>,

    /// Show related schemas inline after endpoint detail
    #[arg(long)]
    context: bool,

    /// Generate a JSON example for the request body or schema
    #[arg(long)]
    example: bool,

    /// Named document from config, path, or URL override
    #[arg(long)]
    doc: Option<String>,

    /// Add a document to the library (does not make it active)
    #[arg(long)]
    add_doc: Option<PathBuf>,

    /// Switch active document by nickname, or add+activate a file path.
    /// Writes to the config that owns the nickname; for new file paths,
    /// defaults to project config (.phyllotaxis/) if one exists, otherwise
    /// user config (~/.config/phyllotaxis/). Use --global to force user config
    #[arg(long)]
    set_doc: Option<String>,

    /// Clear the active document setting
    #[arg(long)]
    unset_doc: bool,

    /// Remove a document from the library by nickname
    #[arg(long)]
    remove_doc: Option<String>,

    /// Show all configured documents
    #[arg(long)]
    list_docs: bool,

    /// Nickname for --add-doc or --set-doc (auto-derived from filename if omitted)
    #[arg(long)]
    name: Option<String>,

    /// Force --add-doc / --set-doc to write to user config (~/.config/phyllotaxis/)
    /// instead of project config (.phyllotaxis/)
    #[arg(long)]
    global: bool,

    /// Force re-download of remote URL specs (bypasses cache)
    #[arg(long)]
    refresh: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Search across all endpoints and schemas
    Search {
        /// Search term
        term: String,
        /// Limit results per category
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Interactive setup — detect document and write config
    Init {
        /// Document file path — skips interactive prompt when provided
        #[arg(long)]
        doc_path: Option<PathBuf>,
    },
    /// Generate shell completion scripts
    #[command(hide = true)]
    Completions {
        /// Shell to generate completions for
        shell: clap_complete::Shell,
    },
}

fn json_error(msg: &str) -> String {
    serde_json::json!({"error": msg}).to_string()
}

fn json_error_with_suggestions(msg: &str, suggestions: &[String]) -> String {
    if suggestions.is_empty() {
        serde_json::json!({"error": msg}).to_string()
    } else {
        serde_json::json!({"error": msg, "suggestions": suggestions}).to_string()
    }
}

/// Pre-formatted error that should be printed to stderr as-is (no wrapping).
/// Used when the error has already been formatted (e.g., JSON with suggestions).
#[derive(Debug)]
struct PreformattedError(String);
impl std::fmt::Display for PreformattedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for PreformattedError {}

/// Extract the binary filename from argv[0], falling back to "phyllotaxis".
fn detect_bin_name() -> String {
    std::env::args()
        .next()
        .as_deref()
        .and_then(|s| std::path::Path::new(s).file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("phyllotaxis")
        .to_string()
}

fn main() -> std::process::ExitCode {
    human_panic::setup_panic!();
    let cli = Cli::parse();
    let json = resolve_output_format(cli.json, cli.text);
    match run(cli, json) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            if e.downcast_ref::<PreformattedError>().is_some() {
                eprintln!("{e:#}");
            } else if json {
                eprintln!("{}", json_error(&format!("{e:#}")));
            } else {
                eprintln!("Error: {e:#}");
            }
            std::process::ExitCode::FAILURE
        }
    }
}

/// Resolve whether to use JSON output:
/// --json forces JSON, --text forces text, otherwise auto-detect from TTY.
fn resolve_output_format(json_flag: bool, text_flag: bool) -> bool {
    use std::io::IsTerminal;
    if json_flag {
        true
    } else if text_flag {
        false
    } else {
        // Auto-detect: piped → JSON, terminal → text
        !std::io::stdout().is_terminal()
    }
}

fn run(cli: Cli, json: bool) -> anyhow::Result<()> {
    use std::io::IsTerminal;

    let cwd = std::env::current_dir().context("Cannot determine current directory")?;
    let bin_name = detect_bin_name();

    let is_tty = std::io::stdout().is_terminal()
        && std::env::var("NO_COLOR").is_err()
        && std::env::var("TERM").map(|t| t != "dumb").unwrap_or(true)
        && std::env::var("CLICOLOR").map(|v| v != "0").unwrap_or(true);

    // Completions does not need a document — generate and exit immediately.
    if let Some(Commands::Completions { shell }) = cli.command {
        use clap::CommandFactory;
        use clap_complete::generate;
        let mut cmd = Cli::command();
        let comp_name = cmd.get_name().to_string();
        generate(shell, &mut cmd, comp_name, &mut std::io::stdout());
        return Ok(());
    }

    // Init does not support --json; it's interactive and always writes to .phyllotaxis/config.yaml
    if let Some(Commands::Init { doc_path }) = &cli.command {
        commands::init::run_init(&cwd, doc_path.as_deref())?;
        return Ok(());
    }

    // ─── Document management operations (mutating, no spec needed) ───────────────

    let scoped = spec::load_config(&cwd);
    let project_root = scoped.project.as_ref().map(|(_, root)| root.as_path());

    if cli.list_docs {
        if json {
            commands::init::run_list_docs_json(&scoped)?;
        } else {
            commands::init::run_list_docs(&scoped)?;
        }
        return Ok(());
    }

    if let Some(ref doc_path) = cli.add_doc {
        let path_str = doc_path.to_string_lossy();
        commands::init::run_add_doc(
            project_root.or(Some(cwd.as_path())),
            cli.global,
            &path_str,
            cli.name.as_deref(),
        )?;
        return Ok(());
    }

    if let Some(ref target) = cli.set_doc {
        // Determine which scope owns this nickname (design: set active in owning scope)
        let in_project = scoped
            .project
            .as_ref()
            .map(|(cfg, _)| cfg.documents.contains_key(target.as_str()))
            .unwrap_or(false);
        let in_user = scoped
            .user
            .as_ref()
            .map(|cfg| cfg.documents.contains_key(target.as_str()))
            .unwrap_or(false);

        let effective_user_scope = if cli.global {
            true
        } else if in_project {
            false
        } else if in_user {
            true
        } else {
            cli.global || project_root.is_none()
        };

        commands::init::run_set_doc(
            project_root.or(Some(cwd.as_path())),
            effective_user_scope,
            target,
            cli.name.as_deref(),
        )?;
        return Ok(());
    }

    if cli.unset_doc {
        commands::init::run_unset_doc(project_root.or(Some(cwd.as_path())), cli.global)?;
        return Ok(());
    }

    if let Some(ref name) = cli.remove_doc {
        commands::init::run_remove_doc(project_root.or(Some(cwd.as_path())), cli.global, name)?;
        return Ok(());
    }

    // Migration guard: old subcommand names removed in v2.0.
    // They parse as the positional `document` argument and would confusingly fail.
    if let Some(ref doc) = cli.document {
        let migration_hint = match doc.as_str() {
            "resources" | "endpoints" => Some(format!(
                "Subcommand '{}' was removed in v2.0. Use: {} --resources [name]",
                doc, bin_name
            )),
            "schemas" => Some(format!(
                "Subcommand 'schemas' was removed in v2.0. Use: {} --schemas [name]",
                bin_name
            )),
            "auth" => Some(format!(
                "Subcommand 'auth' was removed in v2.0. Use: {} --auth",
                bin_name
            )),
            "callbacks" => Some(format!(
                "Subcommand 'callbacks' was removed in v2.0. Use: {} --callbacks [name]",
                bin_name
            )),
            _ => None,
        };
        if let Some(hint) = migration_hint {
            if json {
                return Err(PreformattedError(json_error(&hint)).into());
            }
            anyhow::bail!("{}", hint);
        }
    }

    // Resolve document: positional > --doc > config/env/auto-detect
    let doc_flag = cli.document.as_deref().or(cli.doc.as_deref());
    let loaded = spec::load_document(doc_flag, &cwd, cli.refresh)?;

    // Handle search subcommand
    if let Some(Commands::Search { term, limit }) = &cli.command {
        let term_trimmed = term.trim();
        if term_trimmed.is_empty() {
            let mut msg = "Please provide a search term.".to_string();
            if !json {
                msg.push_str(&format!(
                    "\nUse '{} --resources' or '{} --schemas' to list all items.",
                    bin_name, bin_name
                ));
            }
            anyhow::bail!("{}", msg);
        }
        let results = commands::search::search(&loaded.api, term_trimmed);
        let output = if json {
            render::json::render_search(&results, &bin_name, is_tty)
        } else {
            render::text::render_search(&results, &bin_name, *limit, is_tty)
        };
        println!("{}", output);
        return Ok(());
    }

    // --for: JSON Pointer navigation (mutually exclusive with view flags)
    if let Some(ref raw_pointer) = cli.r#for {
        let has_view_flag = cli.resources.is_some()
            || cli.schemas.is_some()
            || cli.auth
            || cli.callbacks.is_some()
            || cli.endpoint.is_some();
        if has_view_flag {
            anyhow::bail!(
                "--for cannot be combined with view flags \
                 (--resources, --schemas, --auth, --callbacks, --endpoint)"
            );
        }

        let pointer = raw_pointer.strip_prefix('#').unwrap_or(raw_pointer);

        // Empty pointer or "/" are valid per RFC 6901
        if !pointer.is_empty() && !pointer.starts_with('/') {
            anyhow::bail!(
                "Invalid JSON Pointer '{}': must start with '/'",
                raw_pointer
            );
        }

        match spec::json_pointer_get(&loaded.raw_value, pointer) {
            Some(node) => {
                let output = if is_tty && !json {
                    serde_json::to_string_pretty(node)
                } else {
                    serde_json::to_string(node)
                }
                .context("Internal error: failed to serialize JSON Pointer result")?;
                println!("{}", output);
                return Ok(());
            }
            None => {
                let msg = format!("JSON Pointer '{}' not found in document.", raw_pointer);
                if json {
                    return Err(PreformattedError(json_error(&msg)).into());
                }
                anyhow::bail!("{}", msg);
            }
        }
    }

    // Determine if any view flags are set
    let has_any_flag = cli.resources.is_some()
        || cli.schemas.is_some()
        || cli.auth
        || cli.callbacks.is_some()
        || cli.endpoint.is_some();

    if !has_any_flag {
        // No flags = overview (same as before)
        let data = commands::overview::build(&loaded);
        let output = if json {
            render::json::render_overview(&data, &bin_name, is_tty)
        } else {
            render::text::render_overview(&data, &bin_name, is_tty)
        };
        println!("{}", output);
        return Ok(());
    }

    // Common context for all flag handlers
    let ctx = Ctx {
        loaded: &loaded,
        json,
        expand: cli.expand,
        bin_name: &bin_name,
        is_tty,
    };

    // Process each flag, collecting JSON outputs for multi-flag merging
    let mut json_parts: Vec<(&str, String)> = Vec::new();

    if let Some(ref name) = cli.resources {
        if let Some(json) = handle_resources(&ctx, name)? {
            json_parts.push(("resources", json));
        }
    }

    if let Some(ref name) = cli.schemas {
        if let Some(json) = handle_schemas(&ctx, name, cli.used_by, cli.example, cli.related_limit)?
        {
            json_parts.push(("schemas", json));
        }
    }

    if cli.auth {
        if let Some(json) = handle_auth(&ctx)? {
            json_parts.push(("auth", json));
        }
    }

    if let Some(ref name) = cli.callbacks {
        if let Some(json) = handle_callbacks(&ctx, name)? {
            json_parts.push(("callbacks", json));
        }
    }

    if let Some(ref args) = cli.endpoint {
        if let Some(json) = handle_endpoint(&ctx, args, cli.context, cli.example)? {
            json_parts.push(("endpoint", json));
        }
    }

    // In JSON mode: single flag preserves existing shape, multi-flag merges under keys
    if ctx.json && !json_parts.is_empty() {
        if json_parts.len() == 1 {
            println!("{}", json_parts.into_iter().next().unwrap().1);
        } else {
            let mut doc = serde_json::Map::new();
            for (key, json_str) in json_parts {
                let val: serde_json::Value = serde_json::from_str(&json_str)
                    .context("Internal error: invalid JSON from renderer")?;
                doc.insert(key.to_string(), val);
            }
            let output = if is_tty {
                serde_json::to_string_pretty(&doc)
            } else {
                serde_json::to_string(&doc)
            }
            .context("Internal error: failed to serialize merged JSON")?;
            println!("{}", output);
        }
    }

    Ok(())
}

// ─── Flag handlers ───────────────────────────────────────────────────────────

/// Common rendering context passed to all flag handlers.
struct Ctx<'a> {
    loaded: &'a spec::LoadedDocument,
    json: bool,
    expand: bool,
    bin_name: &'a str,
    is_tty: bool,
}

impl Ctx<'_> {
    /// In text mode, print immediately and return None.
    /// In JSON mode, return the string for collection into a merged document.
    fn emit(&self, output: String) -> Option<String> {
        if self.json {
            Some(output)
        } else {
            println!("{}", output);
            None
        }
    }
}

fn handle_resources(ctx: &Ctx, name: &str) -> anyhow::Result<Option<String>> {
    if name.is_empty() {
        let groups = commands::resources::extract_resource_groups(&ctx.loaded.api);
        let output = if ctx.json {
            render::json::render_resource_list(&groups, ctx.bin_name, ctx.is_tty)
        } else {
            render::text::render_resource_list(&groups, ctx.bin_name, ctx.is_tty)
        };
        return Ok(ctx.emit(output));
    }
    match commands::resources::get_resource_detail(&ctx.loaded.api, name) {
        Some(group) => {
            let output = if ctx.json {
                render::json::render_resource_detail(&group, ctx.bin_name, ctx.is_tty)
            } else {
                render::text::render_resource_detail(&group, ctx.bin_name, ctx.is_tty)
            };
            Ok(ctx.emit(output))
        }
        None => {
            let msg = format!("Resource '{}' not found.", name);
            let groups = commands::resources::extract_resource_groups(&ctx.loaded.api);
            let slugs = commands::resources::suggest_similar(&groups, name);
            if ctx.json {
                let cmds: Vec<String> = slugs
                    .iter()
                    .map(|s| format!("{} --resources {}", ctx.bin_name, s))
                    .collect();
                return Err(PreformattedError(json_error_with_suggestions(&msg, &cmds)).into());
            }
            let mut full_msg = msg;
            if !slugs.is_empty() {
                full_msg.push_str("\nDid you mean:");
                for s in &slugs {
                    full_msg.push_str(&format!("\n  {} --resources {}", ctx.bin_name, s));
                }
            }
            anyhow::bail!("{}", full_msg);
        }
    }
}

fn handle_schemas(
    ctx: &Ctx,
    name: &str,
    used_by: bool,
    example: bool,
    related_limit: Option<usize>,
) -> anyhow::Result<Option<String>> {
    if name.is_empty() {
        let names = commands::schemas::list_schemas(&ctx.loaded.api);
        let output = if ctx.json {
            render::json::render_schema_list(&names, ctx.bin_name, ctx.is_tty)
        } else {
            render::text::render_schema_list(&names, ctx.bin_name, ctx.is_tty)
        };
        return Ok(ctx.emit(output));
    }
    if used_by {
        if commands::schemas::find_schema(&ctx.loaded.api, name).is_none() {
            let msg = format!("Schema '{}' not found.", name);
            let similar = commands::schemas::suggest_similar_schemas(&ctx.loaded.api, name);
            if ctx.json {
                let cmds: Vec<String> = similar
                    .iter()
                    .map(|s| format!("{} --schemas {}", ctx.bin_name, s))
                    .collect();
                return Err(PreformattedError(json_error_with_suggestions(&msg, &cmds)).into());
            }
            let mut full_msg = msg;
            if !similar.is_empty() {
                full_msg.push_str("\nDid you mean:");
                for s in &similar {
                    full_msg.push_str(&format!("\n  {} --schemas {}", ctx.bin_name, s));
                }
            }
            anyhow::bail!("{}", full_msg);
        }
        let usages = commands::schemas::find_schema_usage(&ctx.loaded.api, name);
        let output = if ctx.json {
            render::json::render_schema_usage(name, &usages, ctx.is_tty)
        } else {
            render::text::render_schema_usage(name, &usages, ctx.is_tty)
        };
        return Ok(ctx.emit(output));
    }
    if example {
        match commands::examples::generate_example(&ctx.loaded.api, name, false) {
            Some(ex) => {
                let output = if ctx.json {
                    render::json::render_example(name, &ex, ctx.is_tty)
                } else {
                    render::text::render_example(name, &ex, ctx.is_tty)
                };
                return Ok(ctx.emit(output));
            }
            None => {
                let msg = format!("Schema '{}' not found.", name);
                if ctx.json {
                    return Err(PreformattedError(json_error(&msg)).into());
                }
                anyhow::bail!("{}", msg);
            }
        }
    }
    match commands::schemas::build_schema_model(&ctx.loaded.api, name, ctx.expand, 5) {
        Some(model) => {
            let output = if ctx.json {
                render::json::render_schema_detail(&model, ctx.bin_name, ctx.is_tty)
            } else {
                render::text::render_schema_detail(
                    &model,
                    ctx.bin_name,
                    ctx.expand,
                    related_limit,
                    ctx.is_tty,
                )
            };
            Ok(ctx.emit(output))
        }
        None => {
            let msg = format!("Schema '{}' not found.", name);
            let similar = commands::schemas::suggest_similar_schemas(&ctx.loaded.api, name);
            if ctx.json {
                let cmds: Vec<String> = similar
                    .iter()
                    .map(|s| format!("{} --schemas {}", ctx.bin_name, s))
                    .collect();
                return Err(PreformattedError(json_error_with_suggestions(&msg, &cmds)).into());
            }
            let mut full_msg = msg;
            if !similar.is_empty() {
                full_msg.push_str("\nDid you mean:");
                for s in &similar {
                    full_msg.push_str(&format!("\n  {} --schemas {}", ctx.bin_name, s));
                }
            }
            anyhow::bail!("{}", full_msg);
        }
    }
}

fn handle_auth(ctx: &Ctx) -> anyhow::Result<Option<String>> {
    let model = commands::auth::build_auth_model(&ctx.loaded.api);
    let output = if ctx.json {
        render::json::render_auth(&model, ctx.bin_name, ctx.is_tty)
    } else {
        render::text::render_auth(&model, ctx.bin_name, ctx.is_tty)
    };
    Ok(ctx.emit(output))
}

fn handle_callbacks(ctx: &Ctx, name: &str) -> anyhow::Result<Option<String>> {
    let callbacks = commands::callbacks::list_all_callbacks(&ctx.loaded.api);
    if name.is_empty() {
        let output = if ctx.json {
            render::json::render_callback_list(&callbacks, ctx.bin_name, ctx.is_tty)
        } else {
            render::text::render_callback_list(&callbacks, ctx.bin_name, ctx.is_tty)
        };
        return Ok(ctx.emit(output));
    }
    match commands::callbacks::find_callback(&ctx.loaded.api, name) {
        Some(cb) => {
            let output = if ctx.json {
                render::json::render_callback_detail(&cb, ctx.bin_name, ctx.is_tty)
            } else {
                render::text::render_callback_detail(&cb, ctx.bin_name, ctx.is_tty)
            };
            Ok(ctx.emit(output))
        }
        None => {
            let msg = format!("Callback '{}' not found.", name);
            let similar = commands::callbacks::suggest_similar_callbacks(&callbacks, name);
            if ctx.json {
                let cmds: Vec<String> = similar
                    .iter()
                    .map(|s| format!("{} --callbacks {}", ctx.bin_name, s))
                    .collect();
                return Err(PreformattedError(json_error_with_suggestions(&msg, &cmds)).into());
            }
            let mut full_msg = msg;
            if !similar.is_empty() {
                full_msg.push_str("\nDid you mean:");
                for s in &similar {
                    full_msg.push_str(&format!("\n  {} --callbacks {}", ctx.bin_name, s));
                }
            }
            anyhow::bail!("{}", full_msg);
        }
    }
}

fn handle_endpoint(
    ctx: &Ctx,
    args: &[String],
    context: bool,
    example: bool,
) -> anyhow::Result<Option<String>> {
    let method = &args[0];
    let path = &args[1];
    let expand = ctx.expand || context;

    match commands::resources::get_endpoint_detail(
        &ctx.loaded.api,
        method,
        path,
        expand,
        ctx.bin_name,
    ) {
        Some(ep) => {
            if !ctx.json {
                // Text mode: print each section immediately
                let output = render::text::render_endpoint_detail(&ep, ctx.is_tty);
                println!("{}", output);
                if context {
                    let related =
                        commands::resources::collect_related_schemas(&ctx.loaded.api, method, path);
                    println!(
                        "{}",
                        render::text::render_related_schemas(&related, ctx.is_tty)
                    );
                }
                if example {
                    if let Some(ref body) = ep.request_body {
                        if let Some(ref schema_name) = body.schema_ref {
                            if let Some(ex) = commands::examples::generate_example(
                                &ctx.loaded.api,
                                schema_name,
                                false,
                            ) {
                                println!(
                                    "{}",
                                    render::text::render_example(schema_name, &ex, ctx.is_tty)
                                );
                            }
                        }
                    }
                }
                return Ok(None);
            }

            // JSON mode: merge endpoint + context + example into one object
            let ep_json: serde_json::Value =
                serde_json::from_str(&render::json::render_endpoint_detail(&ep, false))
                    .context("Internal error: invalid endpoint JSON")?;
            let mut merged = match ep_json {
                serde_json::Value::Object(map) => map,
                other => {
                    let mut m = serde_json::Map::new();
                    m.insert("detail".to_string(), other);
                    m
                }
            };

            if context {
                let related =
                    commands::resources::collect_related_schemas(&ctx.loaded.api, method, path);
                let val: serde_json::Value =
                    serde_json::from_str(&render::json::render_related_schemas(&related, false))
                        .context("Internal error: invalid related schemas JSON")?;
                if let serde_json::Value::Object(obj) = val {
                    for (k, v) in obj {
                        merged.insert(k, v);
                    }
                }
            }
            if example {
                if let Some(ref body) = ep.request_body {
                    if let Some(ref schema_name) = body.schema_ref {
                        if let Some(ex) = commands::examples::generate_example(
                            &ctx.loaded.api,
                            schema_name,
                            false,
                        ) {
                            let val: serde_json::Value = serde_json::from_str(
                                &render::json::render_example(schema_name, &ex, false),
                            )
                            .context("Internal error: invalid example JSON")?;
                            merged.insert("example".to_string(), val);
                        }
                    }
                }
            }

            let output = if ctx.is_tty {
                serde_json::to_string_pretty(&merged)
            } else {
                serde_json::to_string(&merged)
            }
            .context("Internal error: failed to serialize endpoint JSON")?;
            Ok(Some(output))
        }
        None => {
            anyhow::bail!("Endpoint {} {} not found.", method.to_uppercase(), path);
        }
    }
}
