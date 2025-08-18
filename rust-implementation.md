# Suckless-Aligned Rust Implementation Plan for Noice

## Philosophy Statement

This implementation follows the suckless philosophy: software that sucks less through radical simplicity, minimal dependencies, and compile-time configuration. We reject complexity in favor of hackability and user control.

**Core Principles:**
- **Less is exponentially more** - Remove code, don't add it
- **Compile-time > Runtime** - Configuration through source modification
- **Zero dependencies when possible** - Implement from scratch
- **Obvious correctness > Test coverage** - Simple code that clearly works
- **User as developer** - Users modify and recompile

## Architecture (Minimal)

```
noice-rs/
├── Cargo.toml          # Minimal/zero dependencies
├── build.rs            # Copies config.def.rs to config.rs if missing
├── src/
│   ├── main.rs         # Entry point (~200 lines)
│   ├── config.def.rs   # Default compile-time configuration
│   └── noice.rs        # All functionality (~3000 lines)
└── config.rs           # User's custom config (gitignored)
```

## Dependencies (Zero-to-Minimal)

```toml
[package]
name = "noice-rs"
version = "0.1.0"
edition = "2021"

[dependencies]
# Option 1: True zero-dependency (implement everything)
# (no dependencies)

# Option 2: Minimal pragmatic (platform abstractions only)
libc = { version = "0.2", default-features = false }

[build-dependencies]
# None - build.rs uses std only

[profile.release]
opt-level = "z"     # Optimize for size
lto = true          # Link-time optimization
codegen-units = 1   # Single codegen unit
strip = true        # Strip symbols
panic = "abort"     # No unwinding
```

## Implementation Approach

### Phase 1: Core Structure (Day 1-2)

#### `config.def.rs` - Compile-Time Configuration
```rust
// Compile-time configuration - users edit this and recompile
pub const SHOW_HIDDEN: bool = false;
pub const DIRS_FIRST: bool = true;
pub const USE_COLOR: bool = true;
pub const CURSOR: &str = " > ";
pub const NO_CURSOR: &str = "   ";
pub const YANK_SYMBOL: &str = "* ";

// Keybindings - compile-time array
pub const KEYBINDS: &[(u8, Action)] = &[
    (b'q', Action::Quit),
    (b'j', Action::Next),
    (b'k', Action::Previous),
    (b'l', Action::Enter),
    (b'h', Action::Back),
    (b'/', Action::Filter),
    (b'y', Action::Yank),
    (b'p', Action::Paste),
    (b'D', Action::Delete),
    (b'd', Action::ToggleDirsFirst),
    (b'.', Action::ToggleHidden),
    // ... more bindings
];

// File associations - compile-time
pub const FILE_RULES: &[(&str, &str)] = &[
    (r"\.mp4$", "mpv"),
    (r"\.pdf$", "zathura"),
    (r"\.txt$", "vim"),
    // ... more rules
];

// Directory jumps
pub const DIR_JUMPS: &[(u8, &str)] = &[
    (b'r', "/"),
    (b'h', "~"),
    (b'c', "~/.config"),
    // ... more jumps
];

// Colors as ANSI escape codes (compile-time)
pub const COLOR_DIR: &str = "\x1b[34m";      // Blue
pub const COLOR_FILE: &str = "\x1b[0m";       // Default
pub const COLOR_EXEC: &str = "\x1b[32m";      // Green
pub const COLOR_LINK: &str = "\x1b[36m";      // Cyan
pub const COLOR_RESET: &str = "\x1b[0m";
```

#### `main.rs` - Entry Point
```rust
mod noice;

fn main() {
    // Parse single optional directory argument
    let dir = std::env::args().nth(1)
        .unwrap_or_else(|| ".".to_string());
    
    // Run the browser
    if let Err(e) = noice::run(&dir) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
```

#### `noice.rs` - Core Implementation (Simplified Example)
```rust
use std::fs;
use std::io::{self, Write, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

include!("../config.rs");  // Include compile-time config

#[derive(Clone)]
pub struct Entry {
    name: String,
    path: PathBuf,
    is_dir: bool,
    is_exec: bool,
    size: u64,
    marked: bool,
}

pub struct State {
    dir: PathBuf,
    entries: Vec<Entry>,
    cursor: usize,
    yanked: Vec<PathBuf>,
}

#[derive(Copy, Clone)]
pub enum Action {
    Quit, Next, Previous, Enter, Back,
    Filter, Yank, Paste, Delete, ToggleHidden,
    ToggleDirsFirst, Mark, // ... etc
}

pub fn run(dir: &str) -> io::Result<()> {
    // Setup terminal
    setup_terminal()?;
    
    let mut state = State {
        dir: PathBuf::from(dir),
        entries: Vec::new(),
        cursor: 0,
        yanked: Vec::new(),
    };
    
    // Main loop
    loop {
        load_directory(&mut state)?;
        render(&state)?;
        
        match get_key()? {
            Some(key) => {
                if let Some(action) = key_to_action(key) {
                    if handle_action(action, &mut state)? {
                        break; // Quit
                    }
                }
            }
            None => continue,
        }
    }
    
    restore_terminal()?;
    Ok(())
}

fn setup_terminal() -> io::Result<()> {
    // Raw mode using termios directly via libc
    print!("\x1b[?1049h");  // Alt screen
    print!("\x1b[?25l");     // Hide cursor
    io::stdout().flush()
}

fn restore_terminal() -> io::Result<()> {
    print!("\x1b[?25h");     // Show cursor
    print!("\x1b[?1049l");   // Exit alt screen
    io::stdout().flush()
}

fn load_directory(state: &mut State) -> io::Result<()> {
    let mut entries = Vec::new();
    
    for entry in fs::read_dir(&state.dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        
        // Skip hidden files if configured
        if !SHOW_HIDDEN && name.starts_with('.') {
            continue;
        }
        
        let metadata = entry.metadata()?;
        entries.push(Entry {
            name,
            path: entry.path(),
            is_dir: metadata.is_dir(),
            is_exec: metadata.permissions().mode() & 0o111 != 0,
            size: metadata.len(),
            marked: false,
        });
    }
    
    // Simple sort
    entries.sort_by(|a, b| {
        if DIRS_FIRST {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        } else {
            a.name.cmp(&b.name)
        }
    });
    
    state.entries = entries;
    state.cursor = state.cursor.min(state.entries.len().saturating_sub(1));
    Ok(())
}

fn render(state: &State) -> io::Result<()> {
    print!("\x1b[H\x1b[2J"); // Clear screen
    
    // Header
    println!("{}", state.dir.display());
    println!();
    
    // Entries
    for (i, entry) in state.entries.iter().enumerate() {
        let cursor = if i == state.cursor { CURSOR } else { NO_CURSOR };
        let mark = if entry.marked { YANK_SYMBOL } else { "  " };
        
        let color = if entry.is_dir {
            COLOR_DIR
        } else if entry.is_exec {
            COLOR_EXEC
        } else {
            COLOR_FILE
        };
        
        println!("{}{}{}{}{}", cursor, mark, color, entry.name, COLOR_RESET);
    }
    
    io::stdout().flush()
}

fn get_key() -> io::Result<Option<u8>> {
    let mut buf = [0u8; 1];
    if io::stdin().read_exact(&mut buf).is_ok() {
        Ok(Some(buf[0]))
    } else {
        Ok(None)
    }
}

fn key_to_action(key: u8) -> Option<Action> {
    KEYBINDS.iter()
        .find(|(k, _)| *k == key)
        .map(|(_, action)| *action)
}

fn handle_action(action: Action, state: &mut State) -> io::Result<bool> {
    match action {
        Action::Quit => return Ok(true),
        Action::Next => {
            if state.cursor < state.entries.len().saturating_sub(1) {
                state.cursor += 1;
            }
        }
        Action::Previous => {
            state.cursor = state.cursor.saturating_sub(1);
        }
        Action::Enter => {
            if let Some(entry) = state.entries.get(state.cursor) {
                if entry.is_dir {
                    state.dir = entry.path.clone();
                    state.cursor = 0;
                } else {
                    open_file(&entry.path)?;
                }
            }
        }
        Action::Back => {
            if let Some(parent) = state.dir.parent() {
                state.dir = parent.to_path_buf();
                state.cursor = 0;
            }
        }
        Action::Mark => {
            if let Some(entry) = state.entries.get_mut(state.cursor) {
                entry.marked = !entry.marked;
            }
        }
        Action::Yank => {
            state.yanked = state.entries.iter()
                .filter(|e| e.marked)
                .map(|e| e.path.clone())
                .collect();
            if state.yanked.is_empty() {
                if let Some(entry) = state.entries.get(state.cursor) {
                    state.yanked.push(entry.path.clone());
                }
            }
        }
        Action::Paste => {
            for path in &state.yanked {
                let dest = state.dir.join(path.file_name().unwrap());
                fs::copy(path, dest)?;
            }
        }
        Action::Delete => {
            let marked: Vec<_> = state.entries.iter()
                .filter(|e| e.marked)
                .map(|e| e.path.clone())
                .collect();
            
            let to_delete = if marked.is_empty() {
                state.entries.get(state.cursor)
                    .map(|e| vec![e.path.clone()])
                    .unwrap_or_default()
            } else {
                marked
            };
            
            for path in to_delete {
                if path.is_dir() {
                    fs::remove_dir_all(&path)?;
                } else {
                    fs::remove_file(&path)?;
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

fn open_file(path: &Path) -> io::Result<()> {
    let name = path.to_string_lossy();
    
    for (pattern, cmd) in FILE_RULES {
        if simple_match(pattern, &name) {
            std::process::Command::new(cmd)
                .arg(path)
                .spawn()?;
            return Ok(());
        }
    }
    
    // Default: open with less
    std::process::Command::new("less")
        .arg(path)
        .status()?;
    Ok(())
}

fn simple_match(pattern: &str, text: &str) -> bool {
    // Simplified pattern matching without regex
    // Just check suffix for now
    if pattern.starts_with(r"\.") && pattern.ends_with("$") {
        let suffix = &pattern[1..pattern.len()-1];
        text.ends_with(suffix)
    } else {
        text.contains(pattern)
    }
}
```

### Phase 2: Terminal Handling (Day 3)

Implement raw terminal I/O without dependencies:

```rust
#[cfg(unix)]
mod term {
    use libc::{termios, tcgetattr, tcsetattr, TCSANOW, ECHO, ICANON};
    use std::mem;
    
    static mut ORIG_TERMIOS: Option<termios> = None;
    
    pub fn enable_raw_mode() {
        unsafe {
            let mut termios = mem::zeroed();
            tcgetattr(0, &mut termios);
            ORIG_TERMIOS = Some(termios.clone());
            
            termios.c_lflag &= !(ECHO | ICANON);
            tcsetattr(0, TCSANOW, &termios);
        }
    }
    
    pub fn disable_raw_mode() {
        unsafe {
            if let Some(termios) = ORIG_TERMIOS {
                tcsetattr(0, TCSANOW, &termios);
            }
        }
    }
}
```

### Phase 3: File Operations (Day 4)

Implement file operations using std library only:

```rust
fn copy_recursive(from: &Path, to: &Path) -> io::Result<()> {
    if from.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            let to = to.join(entry.file_name());
            copy_recursive(&entry.path(), &to)?;
        }
    } else {
        fs::copy(from, to)?;
    }
    Ok(())
}

fn move_file(from: &Path, to: &Path) -> io::Result<()> {
    fs::rename(from, to).or_else(|_| {
        copy_recursive(from, to)?;
        if from.is_dir() {
            fs::remove_dir_all(from)
        } else {
            fs::remove_file(from)
        }
    })
}
```

### Phase 4: Platform File Watching (Day 5)

Direct implementation without notify crate:

```rust
#[cfg(target_os = "linux")]
mod watch {
    use libc::{inotify_init, inotify_add_watch, IN_MODIFY};
    
    pub struct Watcher {
        fd: i32,
    }
    
    impl Watcher {
        pub fn new(path: &Path) -> io::Result<Self> {
            unsafe {
                let fd = inotify_init();
                if fd < 0 {
                    return Err(io::Error::last_os_error());
                }
                
                let path_cstr = std::ffi::CString::new(path.to_str().unwrap())?;
                inotify_add_watch(fd, path_cstr.as_ptr(), IN_MODIFY);
                
                Ok(Watcher { fd })
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod watch {
    // kqueue implementation
}
```

## Testing Philosophy

Following suckless principles, we prioritize **obvious correctness** over test coverage:

1. **Manual Testing**: Primary testing method - actually use the software
2. **Simple Integration Test**: One test binary that exercises core workflows
3. **No Unit Tests**: Code should be simple enough to be obviously correct
4. **User as Tester**: Users who compile from source are expected to test

### Single Integration Test

```rust
// tests/basic.rs
#[test]
fn test_basic_operations() {
    // Create temp directory
    // Add some files
    // Run noice operations
    // Verify results
    // Maximum 100 lines
}
```

## Build Instructions

```bash
# First time setup
cp src/config.def.rs config.rs
# Edit config.rs to customize

# Build
cargo build --release

# Install
sudo cp target/release/noice-rs /usr/local/bin/noice

# Uninstall
sudo rm /usr/local/bin/noice
```

## Performance Targets

- **Binary size**: < 500KB stripped (achievable with zero deps)
- **Startup time**: < 10ms
- **Memory usage**: < 5MB for 10,000 files
- **No allocations in hot paths**: Reuse buffers

## Suckless Compliance Checklist

✓ **Single source file** (main.rs + noice.rs only)  
✓ **Compile-time configuration** (config.def.rs pattern)  
✓ **Zero/minimal dependencies** (libc only)  
✓ **< 4000 lines of code** (target: 3000)  
✓ **No runtime config files**  
✓ **User modifies source to customize**  
✓ **Simple, obvious algorithms**  
✓ **No abstraction layers**  
✓ **Direct system calls where needed**  
✓ **Manual memory management where beneficial**  

## Anti-Patterns to Avoid

❌ Runtime configuration files  
❌ Plugin systems  
❌ Extensive error handling  
❌ Generic abstractions  
❌ Async/await complexity  
❌ Dependency injection  
❌ Multiple trait implementations  
❌ Complex type hierarchies  
❌ Extensive testing frameworks  
❌ Build-time code generation  

## Development Timeline (Simplified)

### Week 1: Core Implementation
- Day 1-2: Basic file browser with navigation
- Day 3: Terminal handling
- Day 4: File operations
- Day 5: Platform-specific features

### Week 2: Polish
- Day 1-2: Bug fixes from usage
- Day 3: Performance optimization
- Day 4: Documentation (man page)
- Day 5: Release

## Conclusion

This plan embraces the suckless philosophy completely. The result will be a fast, minimal, hackable file browser that users can understand and modify. By rejecting complexity and dependencies, we achieve:

- **Simplicity**: ~3000 lines anyone can understand
- **Performance**: Near-instant startup, minimal memory
- **Hackability**: Users modify source directly
- **Reliability**: Simple code with obvious behavior
- **Portability**: Minimal platform-specific code

Remember: Every line of code is a liability. The best code is no code. The second best is simple, obvious code that does exactly what it needs to do and nothing more.
