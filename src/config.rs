use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub base_image_name: String,
    pub podman_socket: String,
    pub container_prefix: String,
    pub workspace_dir: PathBuf,
    pub max_concurrent_containers: usize,
    pub default_timeout_secs: u64,
    pub default_memory_mb: u64,
    pub default_cpu_limit: f32,
    pub default_output_limit_kb: u64,
    pub allowed_script_extensions: Vec<String>,
    pub seccomp_profile: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base_image_name: "localhost/singleload-runner:latest".to_string(),
            podman_socket: format!(
                "unix:///run/user/{}/podman/podman.sock",
                std::env::var("UID").unwrap_or_else(|_| "1000".to_string())
            ),
            container_prefix: "singleload".to_string(),
            workspace_dir: PathBuf::from("/tmp/singleload"),
            max_concurrent_containers: 10,
            default_timeout_secs: 30,
            default_memory_mb: 512,
            default_cpu_limit: 1.0,
            default_output_limit_kb: 1024,
            allowed_script_extensions: vec![
                ".py".to_string(),
                ".js".to_string(),
                ".php".to_string(),
                ".go".to_string(),
                ".rs".to_string(),
                ".sh".to_string(),
                ".cs".to_string(),
            ],
            seccomp_profile: None,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        // Try to load from environment variables or config file
        // For now, just use defaults
        let mut config = Self::default();

        // Override with environment variables if present
        if let Ok(socket) = std::env::var("SINGLELOAD_PODMAN_SOCKET") {
            config.podman_socket = socket;
        }

        if let Ok(image) = std::env::var("SINGLELOAD_BASE_IMAGE") {
            config.base_image_name = image;
        }

        // Ensure workspace directory exists
        std::fs::create_dir_all(&config.workspace_dir)?;

        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.max_concurrent_containers == 0 {
            anyhow::bail!("max_concurrent_containers must be greater than 0");
        }

        if self.default_timeout_secs == 0 || self.default_timeout_secs > 3600 {
            anyhow::bail!("default_timeout_secs must be between 1 and 3600");
        }

        if self.default_memory_mb < 32 || self.default_memory_mb > 8192 {
            anyhow::bail!("default_memory_mb must be between 32 and 8192");
        }

        if self.default_cpu_limit < 0.1 || self.default_cpu_limit > 4.0 {
            anyhow::bail!("default_cpu_limit must be between 0.1 and 4.0");
        }

        Ok(())
    }
}