use crate::container::ContainerManager;
use crate::errors::SingleloadError;
use crate::security::{PathSanitizer, SecurityValidator};
use crate::types::{ContainerConfig, ExecutionResult, Language, Mount};
use anyhow::Result;
use std::path::Path;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tracing::{debug, info, warn};

pub struct Executor {
    container_manager: ContainerManager,
    timeout: Duration,
    memory_limit: u64,
    cpu_limit: f32,
    output_limit: u64,
    security_validator: SecurityValidator,
}

impl Executor {
    pub fn new(
        container_manager: ContainerManager,
        timeout: Duration,
        memory_limit: u64,
        cpu_limit: f32,
        output_limit: u64,
    ) -> Self {
        let allowed_extensions = vec![
            ".py".to_string(),
            ".js".to_string(),
            ".php".to_string(),
            ".go".to_string(),
            ".rs".to_string(),
            ".sh".to_string(),
            ".cs".to_string(),
        ];

        Self {
            container_manager,
            timeout,
            memory_limit,
            cpu_limit,
            output_limit,
            security_validator: SecurityValidator::new(allowed_extensions),
        }
    }

    pub async fn run_script(
        &self,
        language: Language,
        script_path: &Path,
        keep_container: bool,
    ) -> Result<ExecutionResult> {
        let start_time = Instant::now();

        // Validate script path
        self.security_validator.validate_script_path(script_path)?;

        // Read and validate script content
        let script_content = std::fs::read(script_path)?;
        self.security_validator.validate_script_content(&script_content)?;

        // Create temporary directory for script
        let temp_dir = TempDir::new()?;
        let container_script_name = format!("script{}", language.file_extension());
        let temp_script_path = temp_dir.path().join(&container_script_name);
        std::fs::write(&temp_script_path, &script_content)?;

        // Prepare container configuration
        let container_name = PathSanitizer::generate_safe_container_name("singleload");
        let mut config = ContainerConfig {
            self.container_manager.config.base_image_name.clone(),
            name: container_name.clone(),
            memory_limit: self.memory_limit,
            cpu_limit: self.cpu_limit,
            timeout: self.timeout,
            ..Default::default()
        };

        // Mount the script directory
        config.mounts.push(Mount {
            source: temp_dir.path().to_string_lossy().to_string(),
            target: "/workspace".to_string(),
            read_only: true,
        });

        // Set environment variables
        config.env.push(("HOME".to_string(), "/tmp".to_string()));
        config.env.push(("USER".to_string(), "nonroot".to_string()));
        config.env.push(("PATH".to_string(), "/usr/local/bin:/usr/bin:/bin".to_string()));

        // For languages that need specific environment variables
        match language {
            Language::Python => {
                config.env.push(("PYTHONUNBUFFERED".to_string(), "1".to_string()));
                config.env.push(("PYTHONDONTWRITEBYTECODE".to_string(), "1".to_string()));
            }
            Language::Go => {
                config.env.push(("GOCACHE".to_string(), "/tmp/gocache".to_string()));
                config.env.push(("GOPATH".to_string(), "/tmp/gopath".to_string()));
            }
            Language::DotNet => {
                config.env.push(("DOTNET_CLI_HOME".to_string(), "/tmp".to_string()));
                config.env.push(("DOTNET_CLI_TELEMETRY_OPTOUT".to_string(), "1".to_string()));
            }
            _ => {}
        }

        // Keep container for debugging if requested
        config.read_only = !keep_container;

        info!("Creating container {} for {} script", container_name, language.command());

        // Create and start container
        let container_id = self.container_manager.create_container(config).await?;
        
        debug!("Container created: {}", container_id);

        // Prepare execution command
        let script_path_in_container = format!("/workspace/{}", container_script_name);
        let exec_command = self.build_exec_command(language, &script_path_in_container);

        // Execute the command in the container
        let exec_result = self.execute_in_container(&container_id, exec_command, keep_container).await;

        // Calculate duration
        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Clean up temporary directory
        drop(temp_dir);

        match exec_result {
            Ok((exit_code, stdout, stderr, truncated)) => {
                Ok(ExecutionResult::success(
                    exit_code as u32,
                    stdout,
                    stderr,
                    duration_ms,
                    truncated,
                ))
            }
            Err(e) => {
                // Ensure container is removed on error
                if !keep_container {
                    let _ = self.container_manager.remove_container(&container_id).await;
                }
                Ok(ExecutionResult::error(e.to_string(), duration_ms))
            }
        }
    }

    async fn execute_in_container(
        &self,
        container_id: &str,
        command: Vec<String>,
        keep_container: bool,
    ) -> Result<(i32, String, String, bool)> {
        // Start the container with the command
        self.container_manager.start_container(container_id).await?;

        // Wait for container to finish
        let exit_code = match self.container_manager.wait_container(container_id, self.timeout).await {
            Ok(code) => code,
            Err(e) => {
                warn!("Container execution failed: {}", e);
                if !keep_container {
                    let _ = self.container_manager.remove_container(container_id).await;
                }
                return Err(e);
            }
        };

        // Get logs
        let (stdout, stderr) = self.container_manager.get_container_logs(container_id).await?;

        // Apply output limits
        let (stdout, stderr, truncated) = self.apply_output_limits(stdout, stderr);

        // Clean up container unless debugging
        if !keep_container {
            self.container_manager.remove_container(container_id).await?;
        } else {
            info!("Container {} kept for debugging", container_id);
        }

        Ok((exit_code, stdout, stderr, truncated))
    }

    fn build_exec_command(&self, language: Language, script_path: &str) -> Vec<String> {
        match language {
            Language::Rust => {
                // For Rust, we need to compile first
                vec![
                    "/bin/bash".to_string(),
                    "-c".to_string(),
                    format!(
                        "cd /tmp && rustc {} -o rust_binary && ./rust_binary",
                        script_path
                    ),
                ]
            }
            Language::Go => {
                vec![
                    language.command().to_string(),
                    "run".to_string(),
                    script_path.to_string(),
                ]
            }
            Language::DotNet => {
                // For .NET, we need to create a project structure
                vec![
                    "/bin/bash".to_string(),
                    "-c".to_string(),
                    format!(
                        "cd /tmp && dotnet new console -o app && cp {} /tmp/app/Program.cs && cd app && dotnet run",
                        script_path
                    ),
                ]
            }
            _ => {
                // For interpreted languages, just run directly
                vec![
                    language.command().to_string(),
                    script_path.to_string(),
                ]
            }
        }
    }

    fn apply_output_limits(&self, stdout: String, stderr: String) -> (String, String, bool) {
        let mut truncated = false;
        let limit = self.output_limit as usize;

        let stdout_limited = if stdout.len() > limit {
            truncated = true;
            format!("{}... [truncated]", &stdout[..limit])
        } else {
            stdout
        };

        let stderr_limited = if stderr.len() > limit {
            truncated = true;
            format!("{}... [truncated]", &stderr[..limit])
        } else {
            stderr
        };

        (stdout_limited, stderr_limited, truncated)
    }
}