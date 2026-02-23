use phyllotaxis::{commands, render, spec};

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "phyllotaxis", about = "Progressive disclosure for OpenAPI specs")]
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

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// List all resource groups, or drill into a specific resource
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

    let is_tty = std::io::stdout().is_terminal()
        && std::env::var("NO_COLOR").is_err()
        && std::env::var("TERM").map(|t| t != "dumb").unwrap_or(true)
        && std::env::var("CLICOLOR").map(|v| v != "0").unwrap_or(true);

    // Completions does not need a spec — generate and exit immediately.
    if let Some(Commands::Completions { shell }) = cli.command {
        use clap::CommandFactory;
        use clap_complete::generate;
        let mut cmd = Cli::command();
        let bin_name = cmd.get_name().to_string();
        generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
        return Ok(());
    }

    // Init does not support --json; it's interactive and always writes to .phyllotaxis.yaml
    if let Some(Commands::Init { spec_path }) = &cli.command {
        commands::init::run_init(&cwd, spec_path.as_deref());
        return Ok(());
    }

    let spec_flag = cli.spec.as_ref().map(|p| p.to_str().expect("spec path not valid UTF-8"));
    let loaded = spec::load_spec(spec_flag, &cwd)?;

    match &cli.command {
        None => {
            let data = commands::overview::build(&loaded);
            let output = if cli.json {
                render::json::render_overview(&data, is_tty)
            } else {
                render::text::render_overview(&data, is_tty)
            };
            println!("{}", output);
        }
        Some(Commands::Resources { name, method, path }) => match name {
            None => {
                let groups = commands::resources::extract_resource_groups(&loaded.api);
                let output = if cli.json {
                    render::json::render_resource_list(&groups, is_tty)
                } else {
                    render::text::render_resource_list(&groups, is_tty)
                };
                println!("{}", output);
            }
            Some(name) => {
                if let (Some(method), Some(path)) = (method, path) {
                    // Level 3: endpoint detail
                    match commands::resources::get_endpoint_detail(&loaded.api, method, path, cli.expand) {
                        Some(ep) => {
                            let output = if cli.json {
                                render::json::render_endpoint_detail(&ep, is_tty)
                            } else {
                                render::text::render_endpoint_detail(&ep, is_tty)
                            };
                            println!("{}", output);
                        }
                        None => {
                            if cli.json {
                                eprintln!("{}", json_error(&format!(
                                    "Endpoint {} {} not found.",
                                    method.to_uppercase(),
                                    path
                                )));
                            } else {
                                eprintln!("Error: Endpoint {} {} not found.", method.to_uppercase(), path);
                            }
                            std::process::exit(1);
                        }
                    }
                } else {
                    // Level 2: resource detail
                    match commands::resources::get_resource_detail(&loaded.api, name) {
                        Some(group) => {
                            let output = if cli.json {
                                render::json::render_resource_detail(&group, is_tty)
                            } else {
                                render::text::render_resource_detail(&group, is_tty)
                            };
                            println!("{}", output);
                        }
                        None => {
                            if cli.json {
                                eprintln!("{}", json_error(&format!("Resource '{}' not found.", name)));
                            } else {
                                let groups = commands::resources::extract_resource_groups(&loaded.api);
                                let suggestions = commands::resources::suggest_similar(&groups, name);
                                eprintln!("Error: Resource '{}' not found.", name);
                                if !suggestions.is_empty() {
                                    eprintln!("Did you mean:");
                                    for s in &suggestions {
                                        eprintln!("  phyllotaxis resources {}", s);
                                    }
                                }
                            }
                            std::process::exit(1);
                        }
                    }
                }
            }
        },
        Some(Commands::Schemas { name }) => match name {
            None => {
                let names = commands::schemas::list_schemas(&loaded.api);
                let output = if cli.json {
                    render::json::render_schema_list(&names, is_tty)
                } else {
                    render::text::render_schema_list(&names, is_tty)
                };
                println!("{}", output);
            }
            Some(schema_name) => {
                match commands::schemas::build_schema_model(
                    &loaded.api,
                    schema_name,
                    cli.expand,
                    5,
                ) {
                    Some(model) => {
                        let output = if cli.json {
                            render::json::render_schema_detail(&model, is_tty)
                        } else {
                            render::text::render_schema_detail(&model, cli.expand, is_tty)
                        };
                        println!("{}", output);
                    }
                    None => {
                        if cli.json {
                            eprintln!("{}", json_error(&format!("Schema '{}' not found.", schema_name)));
                        } else {
                            let suggestions = commands::schemas::suggest_similar_schemas(
                                &loaded.api,
                                schema_name,
                            );
                            eprintln!("Error: Schema '{}' not found.", schema_name);
                            if !suggestions.is_empty() {
                                eprintln!("Did you mean:");
                                for s in &suggestions {
                                    eprintln!("  phyllotaxis schemas {}", s);
                                }
                            }
                        }
                        std::process::exit(1);
                    }
                }
            }
        },
        Some(Commands::Auth) => {
            let model = commands::auth::build_auth_model(&loaded.api);
            let output = if cli.json {
                render::json::render_auth(&model, is_tty)
            } else {
                render::text::render_auth(&model, is_tty)
            };
            println!("{}", output);
        }
        Some(Commands::Search { term }) => {
            let results = commands::search::search(&loaded.api, term);
            let output = if cli.json {
                render::json::render_search(&results, is_tty)
            } else {
                render::text::render_search(&results, is_tty)
            };
            println!("{}", output);
        }
        Some(Commands::Init { .. }) => unreachable!(),
        Some(Commands::Completions { .. }) => unreachable!(),
    }

    Ok(())
}
