# Installation Guide for noice-rs

This document provides a comprehensive guide to all available installation methods for noice-rs, a minimal suckless-style file browser written in Rust.

## Quick Start (Recommended)

For most users, the easiest installation method is using the provided installation script:

```bash
git clone https://github.com/dab/noice-rs.git
cd noice-rs
./install.sh
```

## Installation Methods

### 1. Installation Script 🚀

**Best for:** Most users, especially those new to Rust

The installation script handles everything automatically:
- Checks requirements (Rust/Cargo)
- Sets up configuration
- Builds the project
- Installs binary and man page
- Provides clear feedback and instructions

```bash
# Interactive installation
./install.sh

# Direct installation options
./install.sh --user      # Install to ~/.local/bin
./install.sh --system    # Install to /usr/local/bin (requires sudo)
```

### 2. Cargo Install 📦

**Best for:** Rust developers and users with Rust already installed

```bash
cargo install noice-rs
```

This installs the `noice` binary to `~/.cargo/bin`.

### 3. Pre-compiled Binaries 💾

**Best for:** Users who don't want to compile from source

1. Download from [GitHub Releases](https://github.com/dab/noice-rs/releases)
2. Extract the binary for your platform
3. Place in your PATH and make executable

```bash
chmod +x noice
mv noice ~/.local/bin/  # or /usr/local/bin with sudo
```

### 4. Manual Build 🔧

**Best for:** Advanced users who want full control

```bash
git clone https://github.com/dab/noice-rs.git
cd noice-rs
cp src/config.def.rs config.rs  # Setup configuration
cargo build --release           # Build
cp target/release/noice-rs ~/.local/bin/noice  # Install
```

### 5. Package Managers 📱

**Status:** Planned for future releases

- **Homebrew (macOS):** `brew install noice-rs` (coming soon)
- **AUR (Arch Linux):** `yay -S noice-rs` (coming soon)

## Automated Releases

The project uses `cargo-dist` for automated GitHub releases:

- **Multi-platform binaries:** Linux (x86_64, aarch64), macOS (x86_64, aarch64), Windows
- **Shell installers:** Automatic installation scripts
- **Homebrew integration:** Ready for tap distribution
- **GitHub Actions:** Fully automated CI/CD pipeline

## Configuration

noice-rs follows the suckless philosophy of compile-time configuration:

1. Edit `config.rs` to customize keybindings, colors, and behavior
2. Rebuild: `cargo build --release`
3. Reinstall: `./install.sh` or copy binary manually

## Verification

After installation, verify everything works:

```bash
# Test the binary
noice --help

# Test the man page (if installed)
man noice

# Run the file browser
noice
```

## Troubleshooting

### PATH Issues

If the binary isn't found, ensure your installation directory is in PATH:

```bash
# For ~/.local/bin
export PATH="$HOME/.local/bin:$PATH"

# For ~/.cargo/bin
export PATH="$HOME/.cargo/bin:$PATH"
```

Add these lines to your shell configuration file (.bashrc, .zshrc, etc.).

### Man Page Issues

If the man page isn't found:

```bash
export MANPATH="$HOME/.local/share/man:$MANPATH"
```

### Build Issues

Ensure you have:
- Rust 1.70+ (check with `rustc --version`)
- Cargo (comes with Rust)
- A C compiler (for libc dependency)

## Publishing to crates.io

For maintainers, the project is ready for crates.io publishing:

```bash
cargo publish --no-verify --allow-dirty
```

The `--no-verify` flag is needed due to the compile-time configuration system.

## Performance

- **Binary size:** ~380KB (stripped release build)
- **Memory usage:** < 5MB for typical directories
- **Startup time:** < 10ms
- **Dependencies:** Only libc for terminal control

## Philosophy

This installation system balances:
- **Suckless principles:** Minimal, hackable, compile-time configuration
- **Modern convenience:** Multiple installation methods, automated releases
- **User experience:** Clear documentation, helpful scripts, good error messages

Users who want to customize can still edit source and recompile, while casual users get easy installation through standard package management.