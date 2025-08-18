#!/bin/bash

# noice-rs Installation Script
# A user-friendly installer for the noice file browser

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
BINARY_NAME="noice"
INSTALL_DIR_USER="$HOME/.local/bin"
INSTALL_DIR_SYSTEM="/usr/local/bin"
CONFIG_SOURCE="src/config.def.rs"
CONFIG_TARGET="config.rs"

# Functions
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_requirements() {
    print_info "Checking requirements..."
    
    # Check if Rust is installed
    if ! command -v cargo >/dev/null 2>&1; then
        print_error "Rust and Cargo are required but not installed."
        print_info "Install from: https://rustup.rs/"
        exit 1
    fi
    
    # Check if we're in the right directory
    if [[ ! -f "Cargo.toml" ]] || [[ ! -f "$CONFIG_SOURCE" ]]; then
        print_error "This script must be run from the noice-rs project directory."
        exit 1
    fi
    
    print_success "Requirements check passed"
}

setup_config() {
    print_info "Setting up configuration..."
    
    if [[ ! -f "$CONFIG_TARGET" ]]; then
        cp "$CONFIG_SOURCE" "$CONFIG_TARGET"
        print_success "Created $CONFIG_TARGET from $CONFIG_SOURCE"
        print_info "You can edit $CONFIG_TARGET to customize keybindings, colors, and behavior"
        print_info "Remember to rebuild after making changes"
    else
        print_info "Configuration file $CONFIG_TARGET already exists"
    fi
}

build_binary() {
    print_info "Building noice-rs in release mode..."
    
    if ! cargo build --release; then
        print_error "Build failed"
        exit 1
    fi
    
    print_success "Build completed successfully"
}

install_binary() {
    local install_dir="$1"
    local use_sudo="$2"
    
    print_info "Installing binary to $install_dir..."
    
    # Create directory if it doesn't exist
    if [[ "$use_sudo" == "true" ]]; then
        sudo mkdir -p "$install_dir"
        sudo cp "target/release/noice-rs" "$install_dir/$BINARY_NAME"
        sudo chmod +x "$install_dir/$BINARY_NAME"
    else
        mkdir -p "$install_dir"
        cp "target/release/noice-rs" "$install_dir/$BINARY_NAME"
        chmod +x "$install_dir/$BINARY_NAME"
    fi
    
    print_success "Binary installed to $install_dir/$BINARY_NAME"
}

install_man_page() {
    local use_sudo="$1"
    
    if [[ ! -f "man/noice-rs.1" ]]; then
        print_warning "Man page not found, skipping man page installation"
        return
    fi
    
    print_info "Installing man page..."
    
    local man_dir_system="/usr/local/share/man/man1"
    local man_dir_user="$HOME/.local/share/man/man1"
    
    if [[ "$use_sudo" == "true" ]]; then
        sudo mkdir -p "$man_dir_system"
        sudo cp "man/noice-rs.1" "$man_dir_system/noice.1"
        sudo chmod 644 "$man_dir_system/noice.1"
        print_success "Man page installed to $man_dir_system/noice.1"
    else
        mkdir -p "$man_dir_user"
        cp "man/noice-rs.1" "$man_dir_user/noice.1"
        chmod 644 "$man_dir_user/noice.1"
        print_success "Man page installed to $man_dir_user/noice.1"
        
        if [[ ":$MANPATH:" != *":$HOME/.local/share/man:"* ]]; then
            print_info "Add this to your shell config to access the man page:"
            echo "export MANPATH=\"\$HOME/.local/share/man:\$MANPATH\""
        fi
    fi
}

check_path() {
    local install_dir="$1"
    
    if [[ ":$PATH:" != *":$install_dir:"* ]]; then
        print_warning "Directory $install_dir is not in your PATH"
        print_info "Add this line to your shell configuration file (.bashrc, .zshrc, etc.):"
        echo "export PATH=\"$install_dir:\$PATH\""
        echo
        print_info "Or run this command to add it to your current session:"
        echo "export PATH=\"$install_dir:\$PATH\""
        echo
    else
        print_success "Directory $install_dir is in your PATH"
    fi
}

show_usage() {
    echo "noice-rs Installation Script"
    echo
    echo "Usage: $0 [OPTIONS]"
    echo
    echo "Options:"
    echo "  --user      Install to user directory ($INSTALL_DIR_USER)"
    echo "  --system    Install to system directory ($INSTALL_DIR_SYSTEM) [requires sudo]"
    echo "  --help      Show this help message"
    echo
    echo "If no option is specified, you will be prompted to choose."
}

main() {
    echo -e "${BLUE}╭─────────────────────────────────────╮${NC}"
    echo -e "${BLUE}│        noice-rs Installation        │${NC}"
    echo -e "${BLUE}╰─────────────────────────────────────╯${NC}"
    echo
    
    # Parse command line arguments
    case "${1:-}" in
        --help|-h)
            show_usage
            exit 0
            ;;
        --user)
            INSTALL_MODE="user"
            ;;
        --system)
            INSTALL_MODE="system"
            ;;
        "")
            # Interactive mode
            echo "Choose installation location:"
            echo "1) User directory ($INSTALL_DIR_USER) [recommended]"
            echo "2) System directory ($INSTALL_DIR_SYSTEM) [requires sudo]"
            echo
            read -p "Enter your choice (1 or 2): " choice
            
            case $choice in
                1)
                    INSTALL_MODE="user"
                    ;;
                2)
                    INSTALL_MODE="system"
                    ;;
                *)
                    print_error "Invalid choice. Please run the script again."
                    exit 1
                    ;;
            esac
            ;;
        *)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
    
    # Run installation steps
    check_requirements
    setup_config
    build_binary
    
    # Install based on chosen mode
    if [[ "$INSTALL_MODE" == "user" ]]; then
        install_binary "$INSTALL_DIR_USER" "false"
        install_man_page "false"
        check_path "$INSTALL_DIR_USER"
    else
        install_binary "$INSTALL_DIR_SYSTEM" "true"
        install_man_page "true"
        check_path "$INSTALL_DIR_SYSTEM"
    fi
    
    echo
    print_success "Installation completed!"
    print_info "Try running: $BINARY_NAME"
    echo
    print_info "To customize noice:"
    print_info "1. Edit config.rs in this directory"
    print_info "2. Run: cargo build --release"
    print_info "3. Run this installer again to update"
    echo
    print_info "For help: $BINARY_NAME --help"
}

# Run main function
main "$@"