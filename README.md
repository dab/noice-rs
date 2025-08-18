# noice-rs

A suckless-aligned file browser written in Rust. Minimal dependencies, compile-time configuration, and simple implementation following the suckless philosophy.

## Features

- **Minimal Dependencies**: Only uses `libc` for terminal control
- **Compile-Time Configuration**: Edit `config.rs` and recompile to customize
- **Fast & Lightweight**: ~1300 lines of code, instant startup
- **Platform Support**: Native file watching on Linux (inotify) and macOS (kqueue)
- **Vi-style Keybindings**: Navigate with h/j/k/l or arrow keys
- **Two-Key Combos**: Support for `gg`, `DD`, `nf`, `nd`, `g<key>`, `'<key>`
- **File Operations**: Copy, move, delete, rename files and directories
- **File Creation**: Create files and directories with nested path support
- **Marking System**: Mark multiple files for bulk operations
- **Filtering**: Filter directory contents with `/` (search)
- **Multiple Sorting**: Sort by name, size, time, or version number
- **Directory Jumps**: Quick navigation with customizable shortcuts
- **External Programs**: Edit files, play media, run commands via environment variables
- **File Size Display**: Toggle human-readable file size display
- **Session History**: Remember last directory for quick switching
- **Full Feature Parity**: All original noice functionality implemented

## Building

```bash
# Clone the repository
git clone <repository>
cd noice-rs

# Build (config is generated automatically from src/config.def.rs)
cargo build --release

# The binary will be at target/release/noice
```

## Installation

### Method 1: Using the Installation Script (Recommended)

The easiest way to install noice-rs is using the provided installation script:

```bash
# Clone the repository
git clone https://github.com/dab/noice-rs.git
cd noice-rs

# Run the interactive installer
./install.sh

# Or specify installation location directly:
./install.sh --user    # Install to ~/.local/bin
./install.sh --system  # Install to /usr/local/bin (requires sudo)
```

### Method 2: Using cargo install (For Rust Developers)

If you have Rust installed, you can install directly from crates.io:

```bash
cargo install noice-rs
```

This will install the `noice` binary to your `~/.cargo/bin` directory.

### Method 3: Pre-compiled Binaries

Download pre-compiled binaries from the [GitHub Releases](https://github.com/dab/noice-rs/releases) page:

1. Download the appropriate binary for your platform
2. Extract and place in your PATH
3. Make executable: `chmod +x noice`

### Method 4: Manual Build and Install

```bash
# Clone the repository
git clone https://github.com/dab/noice-rs.git
cd noice-rs

# Build in release mode (config generated automatically)
cargo build --release

# Install to system directory
sudo cp target/release/noice /usr/local/bin/noice

# Or install to user directory
mkdir -p ~/.local/bin
cp target/release/noice ~/.local/bin/noice
```

### Package Managers

**Homebrew (macOS):**
```bash
# Coming soon
brew install noice-rs
```

**Arch Linux (AUR):**
```bash
# Coming soon
yay -S noice-rs
```

> **Note:** Make sure your installation directory is in your PATH. If using `~/.local/bin`, add `export PATH="$HOME/.local/bin:$PATH"` to your shell configuration file (.bashrc, .zshrc, etc.).

## Usage

```bash
# Open current directory
noice

# Open specific directory
noice /path/to/directory

# Open home directory
noice ~
```

## Default Keybindings

### Navigation
- `j` / `↓` - Move down
- `k` / `↑` - Move up  
- `l` / `→` / `Enter` - Enter directory / open file
- `h` / `←` / `Backspace` - Go to parent directory
- `gg` / `Home` - Go to first item
- `G` / `End` - Go to last item
- `Ctrl-U` - Half page up
- `Ctrl-D` - Half page down

### File Operations
- `Space` - Mark/unmark file
- `u` - Un-yank (clear yanked list)
- `y` - Yank (copy) marked files or current file
- `m` - Mark files for moving
- `p` - Paste yanked files
- `DD` - Delete marked files or current file (double-D for safety)
- `r` - Rename current file
- `nf` - Create new file
- `nd` - Create new directory
- `L` - Create symbolic links

### View Options
- `.` / `Ctrl-H` - Toggle hidden files
- `d` - Toggle directories first
- `/` - Filter files (search)
- `s` - Sort by name
- `S` - Toggle file size display
- `t` - Sort by modification time
- `v` - Toggle version number sorting
- `Ctrl-L` / `R` - Force redraw/reload

### Directory Operations
- `c` - Change directory (interactive)
- `g<key>` / `'<key>` - Jump to directory by key:
  - `gr` / `'r` - Go to root (/)
  - `ge` / `'e` - Go to /etc
  - `gb` / `'b` - Go to /bin
  - `gu` / `'u` - Go to /usr
  - `gm` / `'m` - Go to /media
  - `g.` / `'.` - Go to ~/.config
- `''` - Jump to last directory (toggle)

### External Programs
- `e` - Edit current file with $EDITOR
- `M` - Open with media player ($NOICEMP)
- `!` - Open shell in current directory ($SHELL)
- `z` - Run system monitor ($NOICETOP)
- `?` - Show manual page ($NOICEMAN)

### Other
- `q` / `ESC` - Quit

## Configuration

Configuration is compile-time based. To customize:

1. **Find your build directory**: After building, config is generated in `$OUT_DIR/config.rs`
2. **Customize config**: Edit the generated config file to customize:
   - **Keybindings**: Map keys to actions, including two-key combos
   - **File Associations**: Define which programs open which file types  
   - **Directory Jumps**: Set up quick navigation shortcuts for `g<key>` and `'<key>`
   - **Colors**: ANSI color codes for different file types
   - **Display Options**: Show/hide hidden files, directories first, file sizes, etc.
   - **Sorting**: Default sort modes (name, size, time, version)
   - **External Programs**: Default commands for editor, media player, etc.
3. **Rebuild**: Run `cargo build --release` to apply changes

Alternatively, modify `src/config.def.rs` directly for permanent changes.

### Environment Variables

The following environment variables can override defaults:
- `EDITOR` - Text editor for `e` command (default: vi)
- `SHELL` - Shell for `!` command (default: sh)
- `NOICEMP` - Media player for `M` command (default: "mpv --shuffle")
- `NOICETOP` - System monitor for `z` command (default: top)
- `NOICEMAN` - Manual command for `?` (default: "man noice")

## Philosophy

This project follows the suckless philosophy:

- **Simplicity**: Code is simple and easy to understand
- **Minimal Dependencies**: Only `libc` for terminal control
- **Compile-Time Configuration**: No runtime config files
- **User as Developer**: Users are expected to edit source and recompile
- **No Bloat**: Only essential features, no plugins or extensions

## Performance

- **Binary Size**: ~380KB (unstripped release build)
- **Memory Usage**: < 5MB for typical directories
- **Startup Time**: < 10ms
- **Lines of Code**: ~1700 lines total (~1400 in core implementation)

## Platform Support

- **Linux**: Full support with inotify for file watching
- **macOS**: Full support with kqueue for file watching  
- **BSD**: Should work (untested)
- **Other Unix**: Basic support without file watching

## License

This project follows the suckless philosophy of simplicity and hackability. Use and modify as you see fit.