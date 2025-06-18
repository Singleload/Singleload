use crate::errors::SingleloadError;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::warn;

pub struct SecurityValidator {
    allowed_extensions: HashSet<String>,
    max_file_size: u64,
}

impl SecurityValidator {
    pub fn new(allowed_extensions: Vec<String>) -> Self {
        Self {
            allowed_extensions: allowed_extensions.into_iter().collect(),
            max_file_size: 10 * 1024 * 1024, // 10MB
        }
    }

    pub fn validate_script_path(&self, path: &Path) -> Result<(), SingleloadError> {
        // Check if file exists
        if !path.exists() {
            return Err(SingleloadError::ScriptNotFound(
                path.display().to_string(),
            ));
        }

        // Check if it's a file (not directory)
        if !path.is_file() {
            return Err(SingleloadError::InvalidInput(
                "Path must be a file, not a directory".to_string(),
            ));
        }

        // Validate extension
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{}", e))
            .unwrap_or_default();

        if !self.allowed_extensions.contains(&extension) {
            return Err(SingleloadError::InvalidInput(format!(
                "File extension '{}' not allowed. Allowed: {:?}",
                extension, self.allowed_extensions
            )));
        }

        // Check file size
        let metadata = std::fs::metadata(path)
            .map_err(|e| SingleloadError::Io(e))?;
        
        if metadata.len() > self.max_file_size {
            return Err(SingleloadError::InvalidInput(format!(
                "File size {} exceeds maximum allowed size of {} bytes",
                metadata.len(),
                self.max_file_size
            )));
        }

        // Validate path traversal attempts
        let canonical = path.canonicalize()
            .map_err(|e| SingleloadError::Io(e))?;
        
        // Ensure the canonical path doesn't contain suspicious patterns
        let path_str = canonical.to_string_lossy();
        if path_str.contains("..") {
            return Err(SingleloadError::SecurityViolation(
                "Path traversal detected".to_string(),
            ));
        }

        Ok(())
    }

    pub fn validate_script_content(&self, content: &[u8]) -> Result<(), SingleloadError> {
        // Check for null bytes
        if content.contains(&0) {
            return Err(SingleloadError::SecurityViolation(
                "Script contains null bytes".to_string(),
            ));
        }

        // Check for suspicious patterns (basic heuristics)
        let content_str = String::from_utf8_lossy(content);
        
        // These are very basic checks - in production, you'd want more sophisticated analysis
        let suspicious_patterns = [
            "/proc/self/",
            "/sys/kernel/",
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "/etc/passwd",
            "/etc/shadow",
            "chmod +s",
            "setuid",
            "CAP_SYS_ADMIN",
        ];

        for pattern in &suspicious_patterns {
            if content_str.contains(pattern) {
                warn!("Suspicious pattern detected in script: {}", pattern);
                // Note: We warn but don't block - the container isolation should handle this
            }
        }

        Ok(())
    }
}

pub struct PathSanitizer;

impl PathSanitizer {
    pub fn sanitize_mount_path(path: &Path) -> Result<PathBuf, SingleloadError> {
        // Get canonical path to resolve symlinks and relative paths
        let canonical = path.canonicalize()
            .map_err(|e| SingleloadError::InvalidInput(
                format!("Invalid path: {}", e)
            ))?;

        // Ensure it's not a system directory
        let forbidden_prefixes = [
            "/proc", "/sys", "/dev", "/etc", "/root", "/boot",
            "/lib", "/lib64", "/usr/lib", "/usr/lib64",
        ];

        let path_str = canonical.to_string_lossy();
        for prefix in &forbidden_prefixes {
            if path_str.starts_with(prefix) {
                return Err(SingleloadError::SecurityViolation(
                    format!("Cannot mount system directory: {}", prefix)
                ));
            }
        }

        Ok(canonical)
    }

    pub fn generate_safe_container_name(prefix: &str) -> String {
        let uuid = uuid::Uuid::new_v4();
        format!("{}-{}", prefix, uuid)
    }
}

#[derive(Debug, Clone)]
pub struct SeccompProfile {
    pub content: String,
}

impl Default for SeccompProfile {
    fn default() -> Self {
        // This is a restrictive seccomp profile that blocks dangerous syscalls
        // In production, you'd load this from a file
        Self {
            content: r#"{
                "defaultAction": "SCMP_ACT_ALLOW",
                "architectures": [
                    "SCMP_ARCH_X86_64",
                    "SCMP_ARCH_X86",
                    "SCMP_ARCH_X32"
                ],
                "syscalls": [
                    {
                        "names": [
                            "acct",
                            "add_key",
                            "bpf",
                            "clock_adjtime",
                            "clock_settime",
                            "create_module",
                            "delete_module",
                            "finit_module",
                            "get_kernel_syms",
                            "get_mempolicy",
                            "init_module",
                            "io_cancel",
                            "io_destroy",
                            "io_getevents",
                            "io_setup",
                            "io_submit",
                            "ioperm",
                            "iopl",
                            "kexec_file_load",
                            "kexec_load",
                            "keyctl",
                            "lookup_dcookie",
                            "mbind",
                            "mount",
                            "move_pages",
                            "nfsservctl",
                            "open_by_handle_at",
                            "perf_event_open",
                            "personality",
                            "pivot_root",
                            "process_vm_readv",
                            "process_vm_writev",
                            "ptrace",
                            "query_module",
                            "quotactl",
                            "reboot",
                            "request_key",
                            "set_mempolicy",
                            "setns",
                            "settimeofday",
                            "stime",
                            "swapoff",
                            "swapon",
                            "sysfs",
                            "umount",
                            "umount2",
                            "unshare",
                            "uselib",
                            "userfaultfd",
                            "ustat",
                            "vm86",
                            "vm86old"
                        ],
                        "action": "SCMP_ACT_ERRNO",
                        "args": [],
                        "comment": "Dangerous system calls that could be used for container escape",
                        "includes": {},
                        "excludes": {},
                        "errnoRet": 1
                    }
                ]
            }"#.to_string(),
        }
    }
}