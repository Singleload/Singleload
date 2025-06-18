# Singleload

A secure, production-grade Rust CLI tool for executing untrusted scripts in isolated Podman containers.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Features

- **Secure by Default**: Runs scripts in rootless Podman containers with multiple security layers
- **Multi-Language Support**: Python, JavaScript (Node.js), PHP, Go, Rust, Bash, and .NET
- **Resource Control**: CPU, memory, timeout, and output size limits
- **Distroless Containers**: Minimal attack surface with Google's distroless base images
- **Defense in Depth**: Seccomp profiles, dropped capabilities, no network, read-only filesystem
- **JSON Output**: Structured output for easy integration with other tools

## Security Architecture

Singleload implements multiple security layers:

1. **Rootless Podman**: No privileged daemon, containers run as unprivileged user
2. **Distroless Base Image**: Minimal container with no shell or package manager
3. **Seccomp Filtering**: Blocks dangerous system calls
4. **Capability Dropping**: All Linux capabilities dropped by default
5. **Network Isolation**: No network access for containers
6. **Read-only Filesystem**: Prevents persistent modifications
7. **Resource Limits**: CPU, memory, process, and output limits
8. **Input Validation**: Script path and content validation

## Installation

### Prerequisites

- Linux system (Debian/Ubuntu recommended)
- Rust 1.87.0 or later
- Podman 5.5.0 or later (rootless mode configured)
- User namespaces enabled

### Install Podman (if not already installed)

#### Option 1: Install Podman 5.x Static Binary (Recommended for Debian 12)

Since Debian 12's repositories only contain older versions of Podman (3.x or 4.x), we recommend installing the static binary to get Podman 5.x:

```bash
# Install runtime dependencies from Debian repos
sudo apt install -y \
  crun \
  fuse-overlayfs \
  slirp4netns \
  uidmap \
  libgpgme11 \
  libseccomp2 \
  catatonit

# Download Podman 5.2.5 static release
cd /tmp
wget https://github.com/containers/podman/releases/download/v5.2.5/podman-remote-release-linux_amd64.tar.gz

# Extract and install
tar -xzf podman-remote-release-linux_amd64.tar.gz
sudo cp -r podman-remote-release-linux_amd64/usr/* /usr/
sudo chmod +x /usr/bin/podman

# Download and install conmon (container monitor)
wget https://github.com/containers/conmon/releases/download/v2.1.12/conmon.amd64
sudo install -m 755 conmon.amd64 /usr/bin/conmon

# Create systemd service files for rootless podman
sudo mkdir -p /usr/lib/systemd/user

sudo tee /usr/lib/systemd/user/podman.service << 'EOF'
[Unit]
Description=Podman API Service
Requires=podman.socket
After=podman.socket
Documentation=man:podman-system-service(1)
StartLimitIntervalSec=0

[Service]
Type=exec
KillMode=process
ExecStart=/usr/bin/podman system service

[Install]
WantedBy=default.target
EOF

sudo tee /usr/lib/systemd/user/podman.socket << 'EOF'
[Unit]
Description=Podman API Socket
Documentation=man:podman-system-service(1)

[Socket]
ListenStream=%t/podman/podman.sock
SocketMode=0660

[Install]
WantedBy=sockets.target
EOF

# Configure rootless mode
systemctl --user daemon-reload
```

#### Option 2: Install from Distribution Repositories

For distributions with newer Podman versions:

```bash
# Debian/Ubuntu (may have older versions)
sudo apt update
sudo apt install -y podman

# Fedora/RHEL (usually has latest versions)
sudo dnf install -y podman

# Arch Linux
sudo pacman -S podman
```

### Configure Rootless Podman

Regardless of installation method, configure rootless mode:

```bash
systemctl --user enable --now podman.socket
```

### Build Singleload

```bash
# Clone the repository
git clone https://github.com/Singleload/Singleload.git
cd Singleload

# Build in release mode
cargo build --release

# Install to system
sudo cp target/release/singleload /usr/local/bin/
```

### Install Base Container Image

```bash
# Build and cache the secure base image
singleload install

# Or with custom Containerfile
singleload install --containerfile /path/to/Containerfile
```

## Usage

### Basic Script Execution

```bash
# Run a Python script
singleload run --lang python --script hello.py

# Run with custom limits
singleload run --lang javascript --script app.js \
  --timeout 10 \
  --memory 256 \
  --cpu 0.5 \
  --max-output 2048
```

### Supported Languages

- `python` - Python 3.11
- `javascript` - Node.js 22 LTS
- `php` - PHP 8.2
- `go` - Go 1.23
- `rust` - Rust 1.87
- `bash` - Bash 5.2
- `dotnet` - .NET 8 LTS

### Output Format

```json
{
  "status": "success",
  "exit_code": 0,
  "stdout": "Hello from Python",
  "stderr": "",
  "duration_ms": 110,
  "truncated": false
}
```

### Debug Mode

Keep container for debugging:

```bash
singleload run --lang python --script debug.py --debug
```

## CLI Options

### Global Options

- `--debug` - Enable debug logging
- `--format <json|text>` - Output format (default: json)

### Install Command

```bash
singleload install [OPTIONS]
```

Options:
- `--containerfile <PATH>` - Custom Containerfile path
- `--force` - Force rebuild even if image exists

### Run Command

```bash
singleload run [OPTIONS] --lang <LANGUAGE> --script <PATH>
```

Options:
- `--lang <LANGUAGE>` - Programming language (required)
- `--script <PATH>` - Path to script file (required)
- `--timeout <SECONDS>` - Execution timeout (default: 30, max: 3600)
- `--memory <MB>` - Memory limit in MB (default: 512, max: 8192)
- `--cpu <FLOAT>` - CPU limit (default: 1.0, range: 0.1-4.0)
- `--max-output <KB>` - Maximum output size in KB (default: 1024, max: 10240)
- `--debug` - Keep container for debugging

## Environment Variables

- `SINGLELOAD_PODMAN_SOCKET` - Override Podman socket path
- `SINGLELOAD_BASE_IMAGE` - Override base image name

## Example Scripts

### Python

```python
# hello.py
print("Hello from secure Python container!")
```

### JavaScript

```javascript
// app.js
console.log("Node.js running in isolation");
console.log("Process info:", process.versions);
```

### Go

```go
// main.go
package main

import "fmt"

func main() {
    fmt.Println("Go execution in Podman")
}
```

## Production Deployment

### System Requirements

- Linux kernel 4.18+ with user namespaces
- Podman 5.0+ configured for rootless operation
- systemd user sessions enabled
- Adequate disk space for container images (~500MB)

### Security Considerations

1. **File System Access**: Scripts can only read their own directory
2. **Network Access**: Disabled by default, no external connections
3. **Process Limits**: Maximum 100 processes per container
4. **Resource Limits**: Enforced CPU, memory, and output limits
5. **Timeout**: Automatic termination after timeout
6. **User Isolation**: Runs as non-root user (UID 65532)

### Performance Tuning

```bash
# Increase default limits for production
export SINGLELOAD_DEFAULT_MEMORY_MB=1024
export SINGLELOAD_DEFAULT_TIMEOUT_SECS=60

# Pre-pull base image for faster cold starts
podman pull localhost/singleload-runner:latest
```

### Monitoring and Logging

Singleload outputs structured logs in JSON format when `--format json` is used:

```bash
# Enable JSON logging for parsing
singleload run --lang python --script app.py --format json 2>&1 | jq
```

## Troubleshooting

### Base Image Not Found

```bash
Error: Base image not found. Please run 'singleload install' first.
```

Solution: Run `singleload install` to build the base container image.

### Podman Socket Connection Failed

```bash
Error: Container error: Failed to connect to Podman
```

Solution:
1. Check if Podman socket is running: `systemctl --user status podman.socket`
2. Start if needed: `systemctl --user start podman.socket`
3. Verify socket path: `echo $XDG_RUNTIME_DIR/podman/podman.sock`

### Permission Denied

```bash
Error: Security violation: Cannot mount system directory
```

Solution: Ensure scripts are not in system directories (/etc, /usr, etc.)

### Resource Limits Exceeded

```bash
{
  "status": "error",
  "error": "Execution timeout exceeded"
}
```

Solution: Increase timeout with `--timeout` flag or optimize script performance.

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run with debug output
RUST_LOG=debug cargo test -- --nocapture

# Run specific test
cargo test test_python_execution
```

### Building Debug Version

```bash
# Build with debug symbols
cargo build

# Run with verbose logging
RUST_LOG=singleload=debug ./target/debug/singleload run --lang python --script test.py
```

### Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Security Reporting

For security vulnerabilities, please email security@singleload.dev instead of using public issues.

## Architecture

```
┌─────────────────┐
│   CLI (Rust)    │
├─────────────────┤
│ Security Layer  │
│ - Path validation│
│ - Content check │
├─────────────────┤
│ Container Mgmt  │
│ - Podman API    │
│ - Resource limits│
├─────────────────┤
│ Execution Engine│
│ - Script runner │
│ - Output capture│
└─────────────────┘
        │
        ▼
┌─────────────────┐
│ Rootless Podman │
├─────────────────┤
│ Security Options│
│ - Seccomp       │
│ - No capabilities│
│ - No network    │
├─────────────────┤
│ Distroless Image│
│ - Minimal base  │
│ - No shell      │
│ - Runtime only  │
└─────────────────┘
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [Podman](https://podman.io/) for rootless container technology
- [Google Distroless](https://github.com/GoogleContainerTools/distroless) for secure base images
- Rust community for excellent security-focused libraries

## Links

- **Repository**: https://github.com/Singleload/Singleload
- **Issues**: https://github.com/Singleload/Singleload/issues
- **Documentation**: https://github.com/Singleload/Singleload/wiki