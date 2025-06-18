#!/bin/bash
set -euo pipefail

# Singleload build and installation script
# This script builds Singleload and optionally installs it system-wide

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Functions
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

check_dependencies() {
    local missing_deps=()

    # Check for Rust
    if ! command -v cargo &> /dev/null; then
        missing_deps+=("Rust/Cargo")
    else
        local rust_version=$(rustc --version | cut -d' ' -f2)
        print_info "Found Rust $rust_version"
    fi

    # Check for Podman
    if ! command -v podman &> /dev/null; then
        missing_deps+=("Podman")
    else
        local podman_version=$(podman --version | cut -d' ' -f3)
        print_info "Found Podman $podman_version"
        
        # Check if rootless podman is configured
        if [[ -z "${XDG_RUNTIME_DIR:-}" ]]; then
            print_warning "XDG_RUNTIME_DIR not set. Rootless Podman may not work properly."
        elif [[ ! -S "${XDG_RUNTIME_DIR}/podman/podman.sock" ]]; then
            print_warning "Podman socket not found. Starting podman.socket service..."
            systemctl --user start podman.socket || true
        fi
    fi

    # Check for user namespaces
    if [[ -f /proc/sys/kernel/unprivileged_userns_clone ]]; then
        if [[ $(cat /proc/sys/kernel/unprivileged_userns_clone) != "1" ]]; then
            print_warning "User namespaces may not be enabled. This is required for rootless containers."
        fi
    fi

    if [[ ${#missing_deps[@]} -gt 0 ]]; then
        print_error "Missing dependencies: ${missing_deps[*]}"
        print_error "Please install missing dependencies before building."
        exit 1
    fi
}

build_release() {
    print_info "Building Singleload in release mode..."
    
    # Clean previous builds
    cargo clean
    
    # Build with optimizations
    RUSTFLAGS="-C target-cpu=native" cargo build --release
    
    if [[ $? -eq 0 ]]; then
        print_info "Build completed successfully!"
        print_info "Binary location: ${SCRIPT_DIR}/target/release/singleload"
    else
        print_error "Build failed!"
        exit 1
    fi
}

build_debug() {
    print_info "Building Singleload in debug mode..."
    
    cargo build
    
    if [[ $? -eq 0 ]]; then
        print_info "Debug build completed successfully!"
        print_info "Binary location: ${SCRIPT_DIR}/target/debug/singleload"
    else
        print_error "Build failed!"
        exit 1
    fi
}

install_binary() {
    local install_path="${1:-/usr/local/bin}"
    local binary_path="${SCRIPT_DIR}/target/release/singleload"
    
    if [[ ! -f "$binary_path" ]]; then
        print_error "Release binary not found. Run './build.sh release' first."
        exit 1
    fi
    
    print_info "Installing singleload to $install_path..."
    
    if [[ -w "$install_path" ]]; then
        cp "$binary_path" "$install_path/"
        chmod +x "$install_path/singleload"
    else
        print_info "Requesting sudo access to install to $install_path..."
        sudo cp "$binary_path" "$install_path/"
        sudo chmod +x "$install_path/singleload"
    fi
    
    print_info "Installation completed!"
    print_info "You can now run: singleload --help"
}

install_containerfile() {
    local install_dir="${HOME}/.config/singleload"
    
    print_info "Installing Containerfile to $install_dir..."
    
    mkdir -p "$install_dir"
    cp "${SCRIPT_DIR}/Containerfile" "$install_dir/"
    
    print_info "Containerfile installed to $install_dir/Containerfile"
}

run_tests() {
    print_info "Running tests..."
    
    # Set test environment variables
    export RUST_TEST_THREADS=1
    export RUST_LOG=singleload=debug
    
    cargo test -- --test-threads=1 --nocapture
}

print_usage() {
    cat << EOF
Singleload Build Script

Usage: ./build.sh [COMMAND] [OPTIONS]

Commands:
    release         Build optimized release binary (default)
    debug           Build debug binary with symbols
    test            Run all tests
    install         Install singleload to system
    clean           Clean build artifacts
    help            Show this help message

Options:
    --install-path PATH    Custom installation path (default: /usr/local/bin)
    --skip-deps           Skip dependency checks

Examples:
    ./build.sh                      # Build release version
    ./build.sh release              # Build release version
    ./build.sh debug                # Build debug version
    ./build.sh test                 # Run tests
    ./build.sh install              # Build and install
    ./build.sh install --install-path ~/.local/bin

EOF
}

# Main script
main() {
    local command="${1:-release}"
    local install_path="/usr/local/bin"
    local skip_deps=false
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --install-path)
                install_path="$2"
                shift 2
                ;;
            --skip-deps)
                skip_deps=true
                shift
                ;;
            *)
                shift
                ;;
        esac
    done
    
    # Header
    echo "======================================"
    echo "Singleload Build Script"
    echo "======================================"
    echo
    
    # Check dependencies unless skipped
    if [[ "$skip_deps" != true ]]; then
        check_dependencies
    fi
    
    case "$command" in
        release)
            build_release
            ;;
        debug)
            build_debug
            ;;
        test)
            run_tests
            ;;
        install)
            build_release
            install_binary "$install_path"
            install_containerfile
            print_info ""
            print_info "Next step: Run 'singleload install' to build the container base image"
            ;;
        clean)
            print_info "Cleaning build artifacts..."
            cargo clean
            rm -rf "${HOME}/.config/singleload/cache"
            print_info "Clean completed!"
            ;;
        help|--help|-h)
            print_usage
            ;;
        *)
            print_error "Unknown command: $command"
            print_usage
            exit 1
            ;;
    esac
}

# Run main function with all arguments
main "$@"