use phyllotaxis::{commands, render, spec};

use anyhow::Context;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "phyllotaxis",
    version,
    about = "Progressive disclosure for OpenAPI specs (alias: phyll)"
)]
struct Cli {
    /// OpenAPI document path (overrides config/env/auto-detect)
    document: Option<PathBuf>,

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

    /// Output in JSON format
    #[arg(long, global = true)]
    json: bool,

    /// Recursively inline nested schemas (max depth 5)
    #[arg(long, global = true)]
    expand: bool,

    /// Cap the number of related schemas shown in schema detail
    #[arg(long, global = true)]
    related_limit: Option<usize>,

    /// Show related schemas inline after endpoint detail
    #[arg(long)]
    context: bool,

    /// Generate a JSON example for the request body or schema
    #[arg(long)]
    example: bool,

    /// Named spec from config, or path override
    #[arg(long)]
    spec: Option<PathBuf>,

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
    /// Interactive setup — detect spec and write config
    Init {
        /// Spec file path — skips interactive prompt when provided
        #[arg(long)]
        spec_path: Option<PathBuf>,
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
    let json = cli.json;
    match run(cli) {
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

fn run(cli: Cli) -> anyhow::Result<()> {
    use std::io::IsTerminal;

    let cwd = std::env::current_dir().context("Cannot determine current directory")?;
    let bin_name = detect_bin_name();

    let is_tty = std::io::stdout().is_terminal()
        && std::env::var("NO_COLOR").is_err()
        && std::env::var("TERM").map(|t| t != "dumb").unwrap_or(true)
        && std::env::var("CLICOLOR").map(|v| v != "0").unwrap_or(true);

    // Completions does not need a spec — generate and exit immediately.
    if let Some(Commands::Completions { shell }) = cli.command {
        use clap::CommandFactory;
        use clap_complete::generate;
        let mut cmd = Cli::command();
        let comp_name = cmd.get_name().to_string();
        generate(shell, &mut cmd, comp_name, &mut std::io::stdout());
        return Ok(());
    }

    // Init does not support --json; it's interactive and always writes to .phyllotaxis.yaml
    if let Some(Commands::Init { spec_path }) = &cli.command {
        commands::init::run_init(&cwd, spec_path.as_deref())?;
        return Ok(());
    }

    // Migration guard: old subcommand names removed in v2.0.
    // They parse as the positional `document` argument and would confusingly fail.
    if let Some(ref doc) = cli.document {
        let name = doc.to_string_lossy();
        let migration_hint = match name.as_ref() {
            "resources" | "endpoints" => Some(format!(
                "Subcommand '{}' was removed in v2.0. Use: {} --resources [name]",
                name, bin_name
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
            if cli.json {
                return Err(PreformattedError(json_error(&hint)).into());
            }
            anyhow::bail!("{}", hint);
        }
    }

    // Resolve document: positional > --spec > config/env/auto-detect
    let doc_path = cli
        .document
        .as_ref()
        .or(cli.spec.as_ref())
        .map(|p| p.to_string_lossy().to_string());
    let loaded = spec::load_spec(doc_path.as_deref(), &cwd)?;

    // Handle search subcommand
    if let Some(Commands::Search { term, limit }) = &cli.command {
        let term_trimmed = term.trim();
        if term_trimmed.is_empty() {
            let mut msg = "Please provide a search term.".to_string();
            if !cli.json {
                msg.push_str(&format!(
                    "\nUse '{} --resources' or '{} --schemas' to list all items.",
                    bin_name, bin_name
                ));
            }
            anyhow::bail!("{}", msg);
        }
        let results = commands::search::search(&loaded.api, term_trimmed);
        let output = if cli.json {
            render::json::render_search(&results, &bin_name, is_tty)
        } else {
            render::text::render_search(&results, &bin_name, *limit, is_tty)
        };
        println!("{}", output);
        return Ok(());
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
        let output = if cli.json {
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
        json: cli.json,
        expand: cli.expand,
        bin_name: &bin_name,
        is_tty,
    };

    // Process each flag independently
    if let Some(ref name) = cli.resources {
        handle_resources(&ctx, name)?;
    }

    if let Some(ref name) = cli.schemas {
        handle_schemas(&ctx, name, cli.used_by, cli.example, cli.related_limit)?;
    }

    if cli.auth {
        handle_auth(&ctx)?;
    }

    if let Some(ref name) = cli.callbacks {
        handle_callbacks(&ctx, name)?;
    }

    if let Some(ref args) = cli.endpoint {
        handle_endpoint(&ctx, args, cli.context, cli.example)?;
    }

    Ok(())
}

// ─── Flag handlers ───────────────────────────────────────────────────────────

/// Common rendering context passed to all flag handlers.
struct Ctx<'a> {
    loaded: &'a spec::LoadedSpec,
    json: bool,
    expand: bool,
    bin_name: &'a str,
    is_tty: bool,
}

fn handle_resources(ctx: &Ctx, name: &str) -> anyhow::Result<()> {
    if name.is_empty() {
        let groups = commands::resources::extract_resource_groups(&ctx.loaded.api);
        let output = if ctx.json {
            render::json::render_resource_list(&groups, ctx.bin_name, ctx.is_tty)
        } else {
            render::text::render_resource_list(&groups, ctx.bin_name, ctx.is_tty)
        };
        println!("{}", output);
    } else {
        match commands::resources::get_resource_detail(&ctx.loaded.api, name) {
            Some(group) => {
                let output = if ctx.json {
                    render::json::render_resource_detail(&group, ctx.bin_name, ctx.is_tty)
                } else {
                    render::text::render_resource_detail(&group, ctx.bin_name, ctx.is_tty)
                };
                println!("{}", output);
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
    Ok(())
}

fn handle_schemas(
    ctx: &Ctx,
    name: &str,
    used_by: bool,
    example: bool,
    related_limit: Option<usize>,
) -> anyhow::Result<()> {
    if name.is_empty() {
        let names = commands::schemas::list_schemas(&ctx.loaded.api);
        let output = if ctx.json {
            render::json::render_schema_list(&names, ctx.bin_name, ctx.is_tty)
        } else {
            render::text::render_schema_list(&names, ctx.bin_name, ctx.is_tty)
        };
        println!("{}", output);
    } else if used_by {
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
        println!("{}", output);
    } else if example {
        match commands::examples::generate_example(&ctx.loaded.api, name, false) {
            Some(ex) => {
                let output = if ctx.json {
                    render::json::render_example(name, &ex, ctx.is_tty)
                } else {
                    render::text::render_example(name, &ex, ctx.is_tty)
                };
                println!("{}", output);
            }
            None => {
                let msg = format!("Schema '{}' not found.", name);
                if ctx.json {
                    return Err(PreformattedError(json_error(&msg)).into());
                }
                anyhow::bail!("{}", msg);
            }
        }
    } else {
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
                println!("{}", output);
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
    Ok(())
}

fn handle_auth(ctx: &Ctx) -> anyhow::Result<()> {
    let model = commands::auth::build_auth_model(&ctx.loaded.api);
    let output = if ctx.json {
        render::json::render_auth(&model, ctx.bin_name, ctx.is_tty)
    } else {
        render::text::render_auth(&model, ctx.bin_name, ctx.is_tty)
    };
    println!("{}", output);
    Ok(())
}

fn handle_callbacks(ctx: &Ctx, name: &str) -> anyhow::Result<()> {
    let callbacks = commands::callbacks::list_all_callbacks(&ctx.loaded.api);
    if name.is_empty() {
        let output = if ctx.json {
            render::json::render_callback_list(&callbacks, ctx.bin_name, ctx.is_tty)
        } else {
            render::text::render_callback_list(&callbacks, ctx.bin_name, ctx.is_tty)
        };
        println!("{}", output);
    } else {
        match commands::callbacks::find_callback(&ctx.loaded.api, name) {
            Some(cb) => {
                let output = if ctx.json {
                    render::json::render_callback_detail(&cb, ctx.bin_name, ctx.is_tty)
                } else {
                    render::text::render_callback_detail(&cb, ctx.bin_name, ctx.is_tty)
                };
                println!("{}", output);
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
    Ok(())
}

fn handle_endpoint(ctx: &Ctx, args: &[String], context: bool, example: bool) -> anyhow::Result<()> {
    let method = &args[0];
    let path = &args[1];

    match commands::resources::get_endpoint_detail(
        &ctx.loaded.api,
        method,
        path,
        ctx.expand,
        ctx.bin_name,
    ) {
        Some(ep) => {
            let output = if ctx.json {
                render::json::render_endpoint_detail(&ep, ctx.is_tty)
            } else {
                render::text::render_endpoint_detail(&ep, ctx.is_tty)
            };
            println!("{}", output);
            if context {
                let related =
                    commands::resources::collect_related_schemas(&ctx.loaded.api, method, path);
                let context_output = if ctx.json {
                    render::json::render_related_schemas(&related, ctx.is_tty)
                } else {
                    render::text::render_related_schemas(&related, ctx.is_tty)
                };
                println!("{}", context_output);
            }
            if example {
                if let Some(ref body) = ep.request_body {
                    if let Some(ref schema_name) = body.schema_ref {
                        if let Some(ex) = commands::examples::generate_example(
                            &ctx.loaded.api,
                            schema_name,
                            false,
                        ) {
                            let output = if ctx.json {
                                render::json::render_example(schema_name, &ex, ctx.is_tty)
                            } else {
                                render::text::render_example(schema_name, &ex, ctx.is_tty)
                            };
                            println!("{}", output);
                        }
                    }
                }
            }
        }
        None => {
            anyhow::bail!("Endpoint {} {} not found.", method.to_uppercase(), path);
        }
    }
    Ok(())
}
