use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use futures::future::join_all;
use maysemantic::StateMgr;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;

#[derive(Parser)]
#[command(name = "may")]
#[command(version = "0.1.0")]
#[command(author = "May Semantic Layer Contributors")]
#[command(about = "CLI tool for the May Semantic Layer", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Validates the semantic models in the provided directory
    Validate {
        /// Optional path to the YAML definitions directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Compiles the semantic models into optimized SQL dialects
    Compile {
        /// Optional path to the YAML definitions directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
    /// Executes a semantic query locally (simulation)
    Run {
        /// The semantic query to execute (e.g. "Revenue by Region")
        #[arg(short, long)]
        query: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Validate { path }) => {
            let target_path_str = path.as_deref().unwrap_or(".");
            let target_path = Path::new(target_path_str);

            println!(
                "{} {}",
                "Validating models at path:".bold(),
                target_path_str.cyan()
            );

            if !target_path.exists() {
                anyhow::bail!("Path does not exist: {}", target_path_str);
            }

            let state_mgr = Arc::new(StateMgr::new());

            let mut entries = Vec::new();
            if target_path.is_file() {
                entries.push(target_path.to_path_buf());
            } else {
                let mut dir = fs::read_dir(target_path).await?;
                while let Some(entry) = dir.next_entry().await? {
                    let p = entry.path();
                    if p.extension()
                        .is_some_and(|ext| ext == "yml" || ext == "yaml")
                    {
                        entries.push(p);
                    }
                }
            };

            let mut tasks = Vec::new();
            for entry in entries {
                let mgr = Arc::clone(&state_mgr);
                tasks.push(tokio::spawn(async move {
                    let file_name = entry.file_name().unwrap().to_string_lossy().into_owned();
                    let content = fs::read_to_string(&entry).await;

                    match content {
                        Ok(c) => match mgr.load_from_yaml(&c) {
                            Ok(_) => (file_name, Ok(())),
                            Err(e) => (file_name, Err(e.to_string())),
                        },
                        Err(e) => (file_name, Err(format!("Failed to read file: {}", e))),
                    }
                }));
            }

            let results = join_all(tasks).await;
            let mut files_processed = 0;
            let mut errors = 0;

            for res in results {
                let (file_name, validation_res) = res?;
                match validation_res {
                    Ok(_) => {
                        println!(
                            "{} {} ... {}",
                            "PASS".green().bold(),
                            file_name.bold(),
                            "OK".green().bold()
                        );
                    }
                    Err(e) => {
                        println!(
                            "{} {} ... {}",
                            "FAIL".red().bold(),
                            file_name.bold(),
                            "FAILED".red().bold()
                        );
                        println!("   {} {}", "Error:".red().bold(), e);
                        errors += 1;
                    }
                }
                files_processed += 1;
            }

            println!("\n{}", "--- Validation Summary ---".bold());
            println!("{}: {}", "Total files processed".bold(), files_processed);

            if errors > 0 {
                println!(
                    "{}: {}",
                    "Total errors found".red().bold(),
                    errors.to_string().red().bold()
                );
                std::process::exit(1);
            } else {
                println!(
                    "{}: {}",
                    "Total errors found".green().bold(),
                    "0".green().bold()
                );
            }
        }
        Some(Commands::Compile { path }) => {
            let target_path_str = path.as_deref().unwrap_or(".");
            println!(
                "{} {}",
                "Compiling models at path:".bold(),
                target_path_str.cyan()
            );

            let state_mgr = StateMgr::new();
            match state_mgr.load_dir(target_path_str).await {
                Ok(_) => {
                    let (models, entities, metrics) =
                        state_mgr.get_stats().map_err(|e| anyhow::anyhow!(e))?;
                    println!("\n{}", "Compilation Successful".green().bold());
                    println!("{}: {}", "Models loaded".bold(), models);
                    println!("{}: {}", "Entities identified".bold(), entities);
                    println!("{}: {}", "Metrics ready".bold(), metrics);
                }
                Err(e) => {
                    println!("{} {}", "Compilation FAILED:".red().bold(), e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Run { query }) => {
            println!("{} \"{}\"", "Running query:".bold(), query.cyan().bold());
            println!(
                "{}",
                "(Execution engine integration coming in MAY-2.0.0)".bright_black()
            );
        }
        None => {
            println!("May Semantic Layer CLI. Run `may --help` for usage.");
        }
    }

    Ok(())
}

#[cfg(test)]
mod cli_tests;
