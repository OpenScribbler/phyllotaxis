use phyllotaxis::{commands, render, spec};

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "phyllotaxis",
    version,
    about = "Progressive disclosure for OpenAPI specs"
)]
struct Cli {
    /// Override spec file location
    #[arg(long, global = true)]
    spec: Option<PathBuf>,

    /// Output in JSON format
    #[arg(long, global = true)]
    json: bool,

    /// Recursively inline nested schemas (max depth 5)
    #[arg(long, global = true)]
    expand: bool,

    /// Cap the number of related schemas shown in schema detail
    #[arg(long, global = true)]
    related_limit: Option<usize>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// List all resource groups, or drill into a specific resource
    #[command(visible_alias = "endpoints")]
    Resources {
        /// Resource name (slug) to drill into
        name: Option<String>,
        /// HTTP method (GET, POST, etc.) for endpoint detail
        method: Option<String>,
        /// Endpoint path for endpoint detail
        path: Option<String>,
    },
    /// List all schemas, or view a specific schema
    Schemas {
        /// Schema name to view
        name: Option<String>,
    },
    /// Show authentication details
    Auth,
    /// Search across all endpoints and schemas
    Search {
        /// Search term
        term: String,
        /// Limit results per category
        #[arg(long)]
        limit: Option<usize>,
    },
    /// List all callbacks, or show detail for a specific callback
    Callbacks {
        /// Callback name to drill into
        name: Option<String>,
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
            if json {
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

    let cwd = std::env::current_dir().expect("cannot determine current directory");
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
        commands::init::run_init(&cwd, spec_path.as_deref());
        return Ok(());
    }

    let spec_flag = cli
        .spec
        .as_ref()
        .map(|p| p.to_str().expect("spec path not valid UTF-8"));
    let loaded = spec::load_spec(spec_flag, &cwd)?;

    match &cli.command {
        None => {
            let data = commands::overview::build(&loaded);
            let output = if cli.json {
                render::json::render_overview(&data, &bin_name, is_tty)
            } else {
                render::text::render_overview(&data, &bin_name, is_tty)
            };
            println!("{}", output);
        }
        Some(Commands::Resources { name, method, path }) => match name {
            None => {
                let groups = commands::resources::extract_resource_groups(&loaded.api);
                let output = if cli.json {
                    render::json::render_resource_list(&groups, &bin_name, is_tty)
                } else {
                    render::text::render_resource_list(&groups, &bin_name, is_tty)
                };
                println!("{}", output);
            }
            Some(name) => {
                // Validate: if method is provided, path must also be provided
                if method.is_some() && path.is_none() {
                    let method_str = method.as_ref().unwrap();
                    let mut msg = "Missing endpoint path.".to_string();
                    if !cli.json {
                        msg.push_str(&format!(
                            "\nUsage: {} resources {} {} <path>",
                            bin_name,
                            name,
                            method_str.to_uppercase()
                        ));
                    }
                    anyhow::bail!("{}", msg);
                }
                if let (Some(method), Some(path)) = (method, path) {
                    // Level 3: endpoint detail
                    match commands::resources::get_endpoint_detail(
                        &loaded.api,
                        method,
                        path,
                        cli.expand,
                        &bin_name,
                    ) {
                        Some(ep) => {
                            let output = if cli.json {
                                render::json::render_endpoint_detail(&ep, is_tty)
                            } else {
                                render::text::render_endpoint_detail(&ep, is_tty)
                            };
                            println!("{}", output);
                        }
                        None => {
                            anyhow::bail!("Endpoint {} {} not found.", method.to_uppercase(), path);
                        }
                    }
                } else {
                    // Level 2: resource detail
                    match commands::resources::get_resource_detail(&loaded.api, name) {
                        Some(group) => {
                            let output = if cli.json {
                                render::json::render_resource_detail(&group, &bin_name, is_tty)
                            } else {
                                render::text::render_resource_detail(&group, &bin_name, is_tty)
                            };
                            println!("{}", output);
                        }
                        None => {
                            let msg = format!("Resource '{}' not found.", name);
                            let groups = commands::resources::extract_resource_groups(&loaded.api);
                            let slugs = commands::resources::suggest_similar(&groups, name);
                            if cli.json {
                                let cmds: Vec<String> = slugs
                                    .iter()
                                    .map(|s| format!("{} resources {}", bin_name, s))
                                    .collect();
                                eprintln!("{}", json_error_with_suggestions(&msg, &cmds));
                                std::process::exit(1);
                            } else {
                                let mut full_msg = msg;
                                if !slugs.is_empty() {
                                    full_msg.push_str("\nDid you mean:");
                                    for s in &slugs {
                                        full_msg
                                            .push_str(&format!("\n  {} resources {}", bin_name, s));
                                    }
                                }
                                anyhow::bail!("{}", full_msg);
                            }
                        }
                    }
                }
            }
        },
        Some(Commands::Schemas { name }) => match name {
            None => {
                let names = commands::schemas::list_schemas(&loaded.api);
                let output = if cli.json {
                    render::json::render_schema_list(&names, &bin_name, is_tty)
                } else {
                    render::text::render_schema_list(&names, &bin_name, is_tty)
                };
                println!("{}", output);
            }
            Some(schema_name) => {
                match commands::schemas::build_schema_model(&loaded.api, schema_name, cli.expand, 5)
                {
                    Some(model) => {
                        let output = if cli.json {
                            render::json::render_schema_detail(&model, &bin_name, is_tty)
                        } else {
                            render::text::render_schema_detail(
                                &model,
                                &bin_name,
                                cli.expand,
                                cli.related_limit,
                                is_tty,
                            )
                        };
                        println!("{}", output);
                    }
                    None => {
                        let msg = format!("Schema '{}' not found.", schema_name);
                        let similar =
                            commands::schemas::suggest_similar_schemas(&loaded.api, schema_name);
                        if cli.json {
                            let cmds: Vec<String> = similar
                                .iter()
                                .map(|s| format!("{} schemas {}", bin_name, s))
                                .collect();
                            eprintln!("{}", json_error_with_suggestions(&msg, &cmds));
                            std::process::exit(1);
                        } else {
                            let mut full_msg = msg;
                            if !similar.is_empty() {
                                full_msg.push_str("\nDid you mean:");
                                for s in &similar {
                                    full_msg.push_str(&format!("\n  {} schemas {}", bin_name, s));
                                }
                            }
                            anyhow::bail!("{}", full_msg);
                        }
                    }
                }
            }
        },
        Some(Commands::Auth) => {
            let model = commands::auth::build_auth_model(&loaded.api);
            let output = if cli.json {
                render::json::render_auth(&model, &bin_name, is_tty)
            } else {
                render::text::render_auth(&model, &bin_name, is_tty)
            };
            println!("{}", output);
        }
        Some(Commands::Search { term, limit }) => {
            let term_trimmed = term.trim();
            if term_trimmed.is_empty() {
                let mut msg = "Please provide a search term.".to_string();
                if !cli.json {
                    msg.push_str(&format!(
                        "\nUse '{} resources' or '{} schemas' to list all items.",
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
        }
        Some(Commands::Callbacks { name }) => {
            let callbacks = commands::callbacks::list_all_callbacks(&loaded.api);
            match name {
                None => {
                    let output = if cli.json {
                        render::json::render_callback_list(&callbacks, &bin_name, is_tty)
                    } else {
                        render::text::render_callback_list(&callbacks, &bin_name, is_tty)
                    };
                    println!("{}", output);
                }
                Some(name) => match commands::callbacks::find_callback(&loaded.api, name) {
                    Some(cb) => {
                        let output = if cli.json {
                            render::json::render_callback_detail(&cb, &bin_name, is_tty)
                        } else {
                            render::text::render_callback_detail(&cb, &bin_name, is_tty)
                        };
                        println!("{}", output);
                    }
                    None => {
                        let msg = format!("Callback '{}' not found.", name);
                        let similar =
                            commands::callbacks::suggest_similar_callbacks(&callbacks, name);
                        if cli.json {
                            let cmds: Vec<String> = similar
                                .iter()
                                .map(|s| format!("{} callbacks {}", bin_name, s))
                                .collect();
                            eprintln!("{}", json_error_with_suggestions(&msg, &cmds));
                            std::process::exit(1);
                        } else {
                            let mut full_msg = msg;
                            if !similar.is_empty() {
                                full_msg.push_str("\nDid you mean:");
                                for s in &similar {
                                    full_msg.push_str(&format!("\n  {} callbacks {}", bin_name, s));
                                }
                            }
                            anyhow::bail!("{}", full_msg);
                        }
                    }
                },
            }
        }
        Some(Commands::Init { .. }) => unreachable!(),
        Some(Commands::Completions { .. }) => unreachable!(),
    }

    Ok(())
}
