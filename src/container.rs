use crate::config::Config;
use crate::errors::SingleloadError;
use crate::security::{PathSanitizer, SeccompProfile};
use crate::types::{ContainerConfig, Mount};
use anyhow::Result;
use podman_api::models::{ContainerCreateResponse, SpecGenerator};
use podman_api::opts::{ContainerCreateOpts, ContainerListOpts, ImageBuildOpts};
use podman_api::{api::Container as PodmanContainer, Podman};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tracing::{debug, info, warn};

pub struct ContainerManager {
    podman: Podman,
    config: Config,
    seccomp_profile: SeccompProfile,
}

impl ContainerManager {
    pub async fn new(config: Config) -> Result<Self> {
        let podman = Podman::new(&config.podman_socket)?;
        
        // Test connection
        podman.ping().await
            .map_err(|e| SingleloadError::PodmanApi(e))?;

        Ok(Self {
            podman,
            config,
            seccomp_profile: SeccompProfile::default(),
        })
    }

    pub async fn install_base_image(&self, containerfile: PathBuf, force: bool) -> Result<()> {
        // Check if image already exists
        if !force && self.base_image_exists().await? {
            info!("Base image already exists. Use --force to rebuild.");
            return Ok(());
        }

        info!("Building base image from {}", containerfile.display());

        // Create a temporary directory for the build context
        let temp_dir = TempDir::new()?;
        let context_dir = temp_dir.path();

        // Copy Containerfile to the context directory
        let dest_containerfile = context_dir.join("Containerfile");
        std::fs::copy(&containerfile, &dest_containerfile)?;

        // Build the image
        let build_opts = ImageBuildOpts::builder()
            .dockerfile("Containerfile".to_string())
            .t(vec![self.config.base_image_name.clone()])
            .pull(true)
            .rm(true)
            .forcerm(true)
            .build();

        let mut build_stream = self.podman.images().build(&build_opts, context_dir).await?;

        // Process build output
        while let Some(output) = build_stream.next().await {
            match output {
                Ok(info) => {
                    if let Some(stream) = info.stream {
                        debug!("Build: {}", stream.trim());
                    }
                    if let Some(error) = info.error {
                        return Err(SingleloadError::Container(format!("Build error: {}", error)).into());
                    }
                }
                Err(e) => {
                    return Err(SingleloadError::Container(format!("Build failed: {}", e)).into());
                }
            }
        }

        info!("Base image built successfully: {}", self.config.base_image_name);
        Ok(())
    }

    pub async fn base_image_exists(&self) -> Result<bool> {
        let images = self.podman.images();
        match images.get(&self.config.base_image_name).inspect().await {
            Ok(_) => Ok(true),
            Err(podman_api::Error::NotFound(_)) => Ok(false),
            Err(e) => Err(SingleloadError::PodmanApi(e).into()),
        }
    }

    pub async fn create_container(&self, config: ContainerConfig) -> Result<String> {
        // Create seccomp profile file
        let seccomp_file = self.write_seccomp_profile().await?;

        // Build container spec
        let mut spec = SpecGenerator {
            image: Some(config.image),
            name: Some(config.name.clone()),
            user: Some(config.user),
            userns: Some("host".to_string()),
            network: Some("none".to_string()),
            read_only_filesystem: Some(config.read_only),
            remove: Some(!config.debug), // Auto-remove unless debugging
            ..Default::default()
        };

        // Resource limits
        spec.resource_limits = Some(HashMap::from([
            ("memory".to_string(), serde_json::json!(config.memory_limit)),
            ("cpu-shares".to_string(), serde_json::json!((config.cpu_limit * 1024.0) as i64)),
            ("pids".to_string(), serde_json::json!(100)), // Limit process creation
        ]));

        // Security options
        spec.security_opt = Some(vec![
            "no-new-privileges".to_string(),
            format!("seccomp={}", seccomp_file.path().display()),
        ]);

        // Capabilities - drop all
        spec.cap_drop = Some(vec!["ALL".to_string()]);
        spec.cap_add = None;

        // Environment variables
        let mut env = HashMap::new();
        for (k, v) in config.env {
            env.insert(k, v);
        }
        spec.env = Some(env);

        // Mounts
        let mut mounts = vec![];
        for mount in config.mounts {
            mounts.push(HashMap::from([
                ("type".to_string(), serde_json::json!("bind")),
                ("source".to_string(), serde_json::json!(mount.source)),
                ("destination".to_string(), serde_json::json!(mount.target)),
                ("readonly".to_string(), serde_json::json!(mount.read_only)),
                ("propagation".to_string(), serde_json::json!("rprivate")),
            ]));
        }
        
        // Add /tmp as writable tmpfs
        mounts.push(HashMap::from([
            ("type".to_string(), serde_json::json!("tmpfs")),
            ("destination".to_string(), serde_json::json!("/tmp")),
            ("tmpfs-size".to_string(), serde_json::json!("100m")),
        ]));
        
        spec.mounts = Some(mounts);

        // Create container
        let create_opts = ContainerCreateOpts::builder()
            .param("name", config.name.clone())
            .build();

        let response: ContainerCreateResponse = self.podman
            .containers()
            .create(&spec, &create_opts)
            .await
            .map_err(|e| SingleloadError::Container(format!("Failed to create container: {}", e)))?;

        Ok(response.id)
    }

    pub async fn start_container(&self, container_id: &str) -> Result<()> {
        let container = self.podman.containers().get(container_id);
        container.start(None).await
            .map_err(|e| SingleloadError::Container(format!("Failed to start container: {}", e)))?;
        Ok(())
    }

    pub async fn wait_container(&self, container_id: &str, timeout: std::time::Duration) -> Result<i32> {
        let container = self.podman.containers().get(container_id);
        
        // Use tokio timeout
        match tokio::time::timeout(timeout, container.wait(None)).await {
            Ok(Ok(exit_code)) => Ok(exit_code as i32),
            Ok(Err(e)) => Err(SingleloadError::Container(format!("Container wait failed: {}", e)).into()),
            Err(_) => {
                // Timeout - try to stop the container
                let _ = container.stop(None).await;
                Err(SingleloadError::Timeout.into())
            }
        }
    }

    pub async fn get_container_logs(&self, container_id: &str) -> Result<(String, String)> {
        let container = self.podman.containers().get(container_id);
        
        let logs = container.logs(None).await
            .map_err(|e| SingleloadError::Container(format!("Failed to get logs: {}", e)))?;

        // Parse logs - podman returns them in a specific format
        let (stdout, stderr) = self.parse_container_logs(logs);
        
        Ok((stdout, stderr))
    }

    pub async fn remove_container(&self, container_id: &str) -> Result<()> {
        let container = self.podman.containers().get(container_id);
        
        // Force remove
        if let Err(e) = container.remove(None).await {
            warn!("Failed to remove container {}: {}", container_id, e);
        }
        
        Ok(())
    }

    pub async fn cleanup_old_containers(&self) -> Result<()> {
        let list_opts = ContainerListOpts::builder()
            .all(true)
            .build();

        let containers = self.podman.containers().list(&list_opts).await?;
        
        for container in containers {
            if let Some(names) = container.names {
                for name in names {
                    if name.starts_with(&format!("/{}-", self.config.container_prefix)) {
                        // Check if container is old (> 1 hour)
                        if let Some(created) = container.created {
                            let age = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)?
                                .as_secs() - created as u64;
                            
                            if age > 3600 {
                                info!("Cleaning up old container: {}", name);
                                if let Some(id) = &container.id {
                                    let _ = self.remove_container(id).await;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    async fn write_seccomp_profile(&self) -> Result<TempDir> {
        let temp_dir = TempDir::new()?;
        let profile_path = temp_dir.path().join("seccomp.json");
        
        std::fs::write(&profile_path, &self.seccomp_profile.content)?;
        
        Ok(temp_dir)
    }

    fn parse_container_logs(&self, logs: String) -> (String, String) {
        let mut stdout = String::new();
        let mut stderr = String::new();

        for line in logs.lines() {
            // Podman log format: "stream_type message"
            // Stream type 1 = stdout, 2 = stderr
            if line.starts_with("1 ") {
                stdout.push_str(&line[2..]);
                stdout.push('\n');
            } else if line.starts_with("2 ") {
                stderr.push_str(&line[2..]);
                stderr.push('\n');
            } else {
                // Fallback - treat as stdout
                stdout.push_str(line);
                stdout.push('\n');
            }
        }

        (stdout.trim_end().to_string(), stderr.trim_end().to_string())
    }
}