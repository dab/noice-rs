# noice-rs

A suckless-aligned file browser written in Rust. Minimal dependencies, compile-time configuration, and simple implementation following the suckless philosophy.

## Features

- **Minimal Dependencies**: Only uses `libc` for terminal control
- **Compile-Time Configuration**: Edit `config.rs` and recompile to customize
- **Fast & Lightweight**: < 1000 lines of code, instant startup
- **Platform Support**: Native file watching on Linux (inotify) and macOS (kqueue)
- **Vi-style Keybindings**: Navigate with h/j/k/l or arrow keys
- **File Operations**: Copy, move, delete, rename files and directories
- **Marking System**: Mark multiple files for bulk operations
- **Filtering**: Filter directory contents with `/`
- **Sorting**: Sort by name, size, or modification time
- **Directory Jumps**: Quick navigation to common directories

## Building

```bash
# Clone the repository
git clone <repository>
cd noice-rs

# First time setup (copies default config)
cp src/config.def.rs config.rs

# Edit config.rs to customize keybindings, colors, file associations, etc.
# (optional - the default config works out of the box)

# Build
cargo build --release

# The binary will be at target/release/noice-rs
```

## Installation

```bash
# Install to /usr/local/bin
sudo cp target/release/noice-rs /usr/local/bin/noice

# Or install to user directory
cp target/release/noice-rs ~/.local/bin/noice
```

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
- `g` / `Home` - Go to first item
- `G` / `End` - Go to last item
- `[` - Page up
- `]` - Page down

### File Operations
- `Space` - Mark/unmark file
- `u` - Unmark all
- `y` - Yank (copy) marked files or current file
- `m` - Mark files for moving
- `p` - Paste yanked files
- `D` - Delete marked files or current file
- `r` - Rename current file
- `n` - Create new directory
- `L` - Create symbolic links

### View Options
- `.` - Toggle hidden files
- `d` - Toggle directories first
- `/` - Filter files
- `s` - Sort by name
- `S` - Sort by size
- `t` - Sort by modification time
- `R` - Reload directory

### Directory Jumps
- `/` - Go to root
- `~` - Go to home
- `c` - Go to ~/.config
- `d` - Go to ~/Downloads
- `D` - Go to ~/Documents
- `t` - Go to /tmp

### Other
- `!` - Open shell in current directory
- `q` / `ESC` - Quit

## Configuration

Edit `config.rs` to customize:

- **Keybindings**: Map keys to actions
- **File Associations**: Define which programs open which file types
- **Directory Jumps**: Set up quick navigation shortcuts
- **Colors**: ANSI color codes for different file types
- **Display Options**: Show/hide hidden files, directories first, etc.

After making changes, rebuild with `cargo build --release`.

## Philosophy

This project follows the suckless philosophy:

- **Simplicity**: Code is simple and easy to understand
- **Minimal Dependencies**: Only `libc` for terminal control
- **Compile-Time Configuration**: No runtime config files
- **User as Developer**: Users are expected to edit source and recompile
- **No Bloat**: Only essential features, no plugins or extensions

## Performance

- **Binary Size**: ~500KB stripped
- **Memory Usage**: < 5MB for typical directories
- **Startup Time**: < 10ms
- **Lines of Code**: < 1000 lines

## Platform Support

- **Linux**: Full support with inotify for file watching
- **macOS**: Full support with kqueue for file watching  
- **BSD**: Should work (untested)
- **Other Unix**: Basic support without file watching

## License

This project follows the suckless philosophy of simplicity and hackability. Use and modify as you see fit.