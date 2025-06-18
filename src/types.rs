use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Python,
    Javascript,
    Php,
    Go,
    Rust,
    Bash,
    #[value(name = "dotnet")]
    DotNet,
}

impl Language {
    pub fn file_extension(&self) -> &'static str {
        match self {
            Language::Python => ".py",
            Language::Javascript => ".js",
            Language::Php => ".php",
            Language::Go => ".go",
            Language::Rust => ".rs",
            Language::Bash => ".sh",
            Language::DotNet => ".cs",
        }
    }

    pub fn command(&self) -> &'static str {
        match self {
            Language::Python => "python3",
            Language::Javascript => "node",
            Language::Php => "php",
            Language::Go => "go",
            Language::Rust => "rustc",
            Language::Bash => "bash",
            Language::DotNet => "dotnet",
        }
    }

    pub fn runner_args(&self, script_path: &str) -> Vec<String> {
        match self {
            Language::Python => vec![script_path.to_string()],
            Language::Javascript => vec![script_path.to_string()],
            Language::Php => vec![script_path.to_string()],
            Language::Go => vec!["run".to_string(), script_path.to_string()],
            Language::Rust => {
                // For Rust, we compile and run
                vec![
                    script_path.to_string(),
                    "-o".to_string(),
                    "/tmp/rust_binary".to_string(),
                    "&&".to_string(),
                    "/tmp/rust_binary".to_string(),
                ]
            }
            Language::Bash => vec![script_path.to_string()],
            Language::DotNet => vec!["run".to_string(), script_path.to_string()],
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub status: String,
    pub exit_code: u32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

impl ExecutionResult {
    pub fn success(exit_code: u32, stdout: String, stderr: String, duration_ms: u64, truncated: bool) -> Self {
        Self {
            status: if exit_code == 0 { "success".to_string() } else { "failed".to_string() },
            exit_code,
            stdout,
            stderr,
            duration_ms,
            error: None,
            truncated,
        }
    }

    pub fn error(error: String, duration_ms: u64) -> Self {
        Self {
            status: "error".to_string(),
            exit_code: 1,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms,
            error: Some(error),
            truncated: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContainerConfig {
    pub image: String,
    pub name: String,
    pub memory_limit: u64,
    pub cpu_limit: f32,
    pub timeout: std::time::Duration,
    pub network_disabled: bool,
    pub read_only: bool,
    pub user: String,
    pub security_opts: Vec<String>,
    pub cap_drop: Vec<String>,
    pub mounts: Vec<Mount>,
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct Mount {
    pub source: String,
    pub target: String,
    pub read_only: bool,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            image: String::new(),
            name: String::new(),
            memory_limit: 512 * 1024 * 1024, // 512MB
            cpu_limit: 1.0,
            timeout: std::time::Duration::from_secs(30),
            network_disabled: true,
            read_only: true,
            user: "65532:65532", // nonroot user
            security_opts: vec![
                "no-new-privileges".to_string(),
                "seccomp=unconfined".to_string(), // We'll use a custom profile later
            ],
            cap_drop: vec!["ALL".to_string()],
            mounts: vec![],
            env: vec![],
        }
    }
}