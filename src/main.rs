use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod config;
mod container;
mod errors;
mod executor;
mod security;
mod types;

use crate::config::Config;
use crate::container::ContainerManager;
use crate::types::{ExecutionResult, Language};

#[derive(Parser)]
#[command(name = "singleload")]
#[command(author, version, about = "Secure script execution in isolated containers", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable debug output
    #[arg(long, global = true)]
    debug: bool,

    /// Output format (json or text)
    #[arg(long, global = true, default_value = "json")]
    format: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Install the base container image
    Install {
        /// Path to Containerfile (defaults to bundled)
        #[arg(long)]
        containerfile: Option<PathBuf>,

        /// Force rebuild even if image exists
        #[arg(long)]
        force: bool,
    },

    /// Run a script in an isolated container
    Run {
        /// Programming language
        #[arg(long, value_enum)]
        lang: Language,

        /// Path to script file
        #[arg(long)]
        script: PathBuf,

        /// Execution timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,

        /// Memory limit in MB
        #[arg(long, default_value = "512")]
        memory: u64,

        /// CPU limit (0.1-4.0)
        #[arg(long, default_value = "1.0")]
        cpu: f32,

        /// Keep container for debugging
        #[arg(long)]
        debug: bool,

        /// Maximum output size in KB
        #[arg(long, default_value = "1024")]
        max_output: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = if cli.debug {
        EnvFilter::new("singleload=debug,podman_api=debug")
    } else {
        EnvFilter::new("singleload=info")
    };

    let fmt_layer = fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false);

    if cli.format == "json" {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer.json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .init();
    }

    // Load configuration
    let config = Config::load()?;
    let container_manager = ContainerManager::new(config.clone()).await?;

    match cli.command {
        Commands::Install { containerfile, force } => {
            info!("Installing Singleload base image...");
            
            let containerfile_path = containerfile.unwrap_or_else(|| {
                // Use bundled Containerfile
                PathBuf::from("Containerfile")
            });

            container_manager.install_base_image(containerfile_path, force).await?;
            
            if cli.format == "json" {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "success",
                        "message": "Base image installed successfully",
                        "image": config.base_image_name
                    })
                );
            } else {
                println!("✓ Base image installed successfully");
            }
        }

        Commands::Run {
            lang,
            script,
            timeout,
            memory,
            cpu,
            debug,
            max_output,
        } => {
            // Validate inputs
            if !script.exists() {
                anyhow::bail!("Script file not found: {}", script.display());
            }

            if timeout == 0 || timeout > 3600 {
                anyhow::bail!("Timeout must be between 1 and 3600 seconds");
            }

            if memory < 32 || memory > 8192 {
                anyhow::bail!("Memory must be between 32 and 8192 MB");
            }

            if cpu < 0.1 || cpu > 4.0 {
                anyhow::bail!("CPU must be between 0.1 and 4.0");
            }

            if max_output == 0 || max_output > 10240 {
                anyhow::bail!("Max output must be between 1 and 10240 KB");
            }

            // Check if base image exists
            if !container_manager.base_image_exists().await? {
                if cli.format == "json" {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "error",
                            "error": "Base image not found. Please run 'singleload install' first."
                        })
                    );
                } else {
                    eprintln!("Error: Base image not found. Please run 'singleload install' first.");
                }
                std::process::exit(1);
            }

            // Execute script
            let executor = executor::Executor::new(
                container_manager,
                Duration::from_secs(timeout),
                memory * 1024 * 1024, // Convert MB to bytes
                cpu,
                max_output * 1024,    // Convert KB to bytes
            );

            let result = executor.run_script(lang, &script, debug).await;

            // Output result
            match result {
                Ok(execution_result) => {
                    if cli.format == "json" {
                        println!("{}", serde_json::to_string_pretty(&execution_result)?);
                    } else {
                        print_text_result(&execution_result);
                    }
                    
                    if execution_result.exit_code != 0 {
                        std::process::exit(execution_result.exit_code as i32);
                    }
                }
                Err(e) => {
                    if cli.format == "json" {
                        println!(
                            "{}",
                            serde_json::json!({
                                "status": "error",
                                "error": e.to_string()
                            })
                        );
                    } else {
                        eprintln!("Error: {}", e);
                    }
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}

fn print_text_result(result: &ExecutionResult) {
    println!("Status: {}", result.status);
    println!("Exit Code: {}", result.exit_code);
    println!("Duration: {}ms", result.duration_ms);
    
    if !result.stdout.is_empty() {
        println!("\nStdout:\n{}", result.stdout);
    }
    
    if !result.stderr.is_empty() {
        println!("\nStderr:\n{}", result.stderr);
    }
    
    if result.truncated {
        println!("\n⚠ Output was truncated due to size limits");
    }
}