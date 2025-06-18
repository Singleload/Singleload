#[cfg(test)]
mod tests {
    use singleload::types::Language;
    use std::process::Command;

    #[test]
    fn test_language_extensions() {
        assert_eq!(Language::Python.file_extension(), ".py");
        assert_eq!(Language::Javascript.file_extension(), ".js");
        assert_eq!(Language::Php.file_extension(), ".php");
        assert_eq!(Language::Go.file_extension(), ".go");
        assert_eq!(Language::Rust.file_extension(), ".rs");
        assert_eq!(Language::Bash.file_extension(), ".sh");
        assert_eq!(Language::DotNet.file_extension(), ".cs");
    }

    #[test]
    fn test_language_commands() {
        assert_eq!(Language::Python.command(), "python3");
        assert_eq!(Language::Javascript.command(), "node");
        assert_eq!(Language::Php.command(), "php");
        assert_eq!(Language::Go.command(), "go");
        assert_eq!(Language::Rust.command(), "rustc");
        assert_eq!(Language::Bash.command(), "bash");
        assert_eq!(Language::DotNet.command(), "dotnet");
    }

    #[test]
    #[ignore] // Requires built binary
    fn test_cli_help() {
        let output = Command::new("./target/debug/singleload")
            .arg("--help")
            .output()
            .expect("Failed to execute singleload");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Secure script execution in isolated containers"));
    }
}