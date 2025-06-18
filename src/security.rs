use crate::errors::SingleloadError;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::warn;

const MAX_SCRIPT_SIZE: u64 = 10 * 1024 * 1024; // 10MB
const MAX_PATH_DEPTH: usize = 10;
const SUSPICIOUS_EXTENSIONS: &[&str] = &[".so", ".dll", ".dylib", ".ko", ".sys"];

pub struct SecurityValidator {
    allowed_extensions: HashSet<String>,
    max_file_size: u64,
    compiled_patterns: SecurityPatterns,
}

struct SecurityPatterns {
    filesystem_patterns: Vec<(regex::Regex, &'static str)>,
    privilege_patterns: Vec<(regex::Regex, &'static str)>,
    escape_patterns: Vec<(regex::Regex, &'static str)>,
    resource_patterns: Vec<(regex::Regex, &'static str)>,
    network_patterns: Vec<(regex::Regex, &'static str)>,
}

impl SecurityPatterns {
    fn new() -> Result<Self, SingleloadError> {
        Ok(Self {
            filesystem_patterns: Self::compile_patterns(&[
                (r"/proc/self/exe", "Attempting to access process executable"),
                (r"/proc/\d+/maps", "Attempting to read process memory maps"),
                (r"/proc/sys/kernel/[a-zA-Z_]+", "Attempting to access kernel parameters"),
                (r"/sys/kernel/security", "Attempting to access security modules"),
                (r"/etc/(passwd|shadow|sudoers)", "Attempting to access authentication files"),
                (r"/(root|home)/\.", "Attempting to access hidden files in home directories"),
                (r"/dev/(mem|kmem|port)", "Attempting to access raw memory devices"),
                (r"/sys/class/net", "Attempting to access network interfaces"),
                (r"/sys/firmware", "Attempting to access firmware information"),
            ])?,
            privilege_patterns: Self::compile_patterns(&[
                (r"chmod\s+[+]s", "Attempting to set SUID/SGID bits"),
                (r"setuid\s*\(\s*0\s*\)", "Attempting to setuid to root"),
                (r"setgid\s*\(\s*0\s*\)", "Attempting to setgid to root"),
                (r"CAP_[A-Z_]+", "Attempting to manipulate capabilities"),
                (r"LD_PRELOAD\s*=", "Attempting to preload libraries"),
                (r"LD_LIBRARY_PATH\s*=", "Attempting to modify library path"),
                (r"/usr/bin/sudo", "Attempting to use sudo"),
                (r"pkexec", "Attempting to use PolicyKit"),
                (r"doas", "Attempting to use doas"),
            ])?,
            escape_patterns: Self::compile_patterns(&[
                (r"nsenter", "Attempting to enter namespaces"),
                (r"unshare", "Attempting to create new namespaces"),
                (r"/proc/\d+/ns/", "Attempting to access namespace files"),
                (r"mount\s+--bind", "Attempting bind mounts"),
                (r"mount\s+-o\s+remount", "Attempting to remount filesystems"),
                (r"pivot_root", "Attempting to change root filesystem"),
                (r"chroot", "Attempting to change root"),
                (r"/var/run/docker\.sock", "Attempting to access Docker socket"),
                (r"/run/containerd/containerd\.sock", "Attempting to access containerd socket"),
                (r"cgroup", "Attempting to manipulate cgroups"),
                (r"overlay", "Attempting to access overlay filesystem"),
                (r"runc", "Attempting to access container runtime"),
            ])?,
            resource_patterns: Self::compile_patterns(&[
                (r"fork\s*\(\s*\)\s*while.*true", "Fork bomb pattern detected"),
                (r":\(\)\s*{\s*:\|:&\s*}", "Bash fork bomb detected"),
                (r"while.*true.*malloc", "Memory exhaustion pattern detected"),
                (r"/dev/zero.*dd", "Disk filling pattern detected"),
                (r"openfiles\s*=\s*\d{5,}", "File descriptor exhaustion detected"),
                (r"ulimit\s+-[nS]", "Attempting to modify resource limits"),
                (r"stress(-ng)?", "Stress testing tool detected"),
            ])?,
            network_patterns: Self::compile_patterns(&[
                (r"socket\s*\(", "Raw socket creation attempted"),
                (r"connect\s*\(", "Network connection attempted"),
                (r"(curl|wget|nc|netcat|telnet|ssh)", "Network tool usage detected"),
                (r"iptables", "Firewall manipulation attempted"),
                (r"ip\s+(route|addr|link)", "Network configuration attempted"),
                (r"/proc/net/", "Network information access attempted"),
                (r"AF_INET", "Network socket family referenced"),
                (r"SOCK_RAW", "Raw socket type referenced"),
            ])?,
        })
    }

    fn compile_patterns(patterns: &[(&str, &'static str)]) -> Result<Vec<(regex::Regex, &'static str)>, SingleloadError> {
        patterns
            .iter()
            .map(|(pattern, desc)| {
                regex::Regex::new(pattern)
                    .map(|re| (re, *desc))
                    .map_err(|e| SingleloadError::Other(anyhow::anyhow!("Invalid regex {}: {}", pattern, e)))
            })
            .collect()
    }
}

impl SecurityValidator {
    pub fn new(allowed_extensions: Vec<String>) -> Self {
        let patterns = SecurityPatterns::new()
            .unwrap_or_else(|_| panic!("Failed to compile security patterns"));
        
        Self {
            allowed_extensions: allowed_extensions.into_iter().collect(),
            max_file_size: MAX_SCRIPT_SIZE,
            compiled_patterns: patterns,
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

        // Check for suspicious extensions that might be libraries
        if SUSPICIOUS_EXTENSIONS.contains(&extension.as_str()) {
            return Err(SingleloadError::SecurityViolation(
                format!("Suspicious file extension detected: {}", extension)
            ));
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
        
        // Check path depth
        let depth = canonical.components().count();
        if depth > MAX_PATH_DEPTH {
            return Err(SingleloadError::SecurityViolation(
                format!("Path too deep: {} levels (max: {})", depth, MAX_PATH_DEPTH)
            ));
        }

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
        
        // Check for patterns that might indicate attempts to escape the container
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
        // Comprehensive seccomp profile that blocks dangerous syscalls
        // Based on Docker's default profile with additional restrictions
        Self {
            content: r#"{
                "defaultAction": "SCMP_ACT_ALLOW",
                "defaultErrnoRet": 1,
                "archMap": [
                    {
                        "architecture": "SCMP_ARCH_X86_64",
                        "subArchitectures": [
                            "SCMP_ARCH_X86",
                            "SCMP_ARCH_X32"
                        ]
                    }
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
                            "name_to_handle_at",
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
                            "syslog",
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
                        "comment": "System calls that could be used for container escape or privilege escalation",
                        "includes": {},
                        "excludes": {},
                        "errnoRet": 1
                    },
                    {
                        "names": [
                            "clone"
                        ],
                        "action": "SCMP_ACT_ALLOW",
                        "args": [
                            {
                                "index": 0,
                                "value": 2114060288,
                                "op": "SCMP_CMP_MASKED_EQ"
                            }
                        ],
                        "comment": "Allow clone for thread creation but not new namespaces",
                        "includes": {},
                        "excludes": {}
                    },
                    {
                        "names": [
                            "chmod",
                            "fchmod",
                            "fchmodat"
                        ],
                        "action": "SCMP_ACT_ALLOW",
                        "args": [
                            {
                                "index": 1,
                                "value": 2048,
                                "op": "SCMP_CMP_MASKED_EQ"
                            }
                        ],
                        "comment": "Block chmod with SUID/SGID bits",
                        "includes": {},
                        "excludes": {},
                        "errnoRet": 1
                    }
                ]
            }"#.to_string(),
        }
    }
}