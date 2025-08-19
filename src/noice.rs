use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::cmp::Ordering;
use std::process::{Command, Stdio};
use std::env;
use std::collections::HashSet;
use libc::{termios, tcgetattr, tcsetattr, TCSANOW, ECHO, ICANON, VMIN, VTIME};
use std::mem;

#[cfg(target_os = "linux")]
use libc::{inotify_init1, inotify_add_watch, inotify_event, IN_NONBLOCK, IN_MODIFY, IN_CREATE, IN_DELETE, IN_MOVED_FROM, IN_MOVED_TO};

#[cfg(target_os = "macos")]
use libc::{kqueue, kevent, EVFILT_VNODE, EV_ADD, EV_CLEAR, NOTE_WRITE};

include!(concat!(env!("OUT_DIR"), "/config.rs"));

#[derive(Clone)]
pub struct Entry {
    name: String,
    path: PathBuf,
    is_dir: bool,
    is_exec: bool,
    is_link: bool,
    size: u64,
    marked: bool,
    mtime: SystemTime,
}

pub struct State {
    // Large heap-allocated fields first for better alignment
    dir: PathBuf,
    entries: Vec<Entry>,
    yanked: Vec<PathBuf>,
    filter: Option<String>,
    message: Option<String>,
    save_file: Option<String>,
    last_dir: Option<PathBuf>,
    filter_input: Option<String>,
    cached_dir_display: Option<String>,
    
    // usize fields (8 bytes on 64-bit systems)
    cursor: usize,
    view_offset: usize,
    term_height: usize,
    term_width: usize,
    longest_entry_width: usize,
    cached_counts: (usize, usize), // (marked_count, yanked_count)
    
    // Enum fields (typically 1 byte + padding)
    sort_mode: SortMode,
    
    // Small fields last to minimize padding
    show_hidden: bool,
    dirs_first: bool,
    show_size: bool,
    version_sort: bool,
    use_color: bool,
    tilde_home: bool,
    cache_dirty: bool,
    
    // Optional single-byte field
    pending_key: Option<u8>,
}

#[derive(Clone, Copy)]
pub enum SortMode {
    Name,
    Size,
    Time,
    Version,
}


static mut ORIG_TERMIOS: Option<termios> = None;

#[cfg(target_os = "linux")]
struct FileWatcher {
    fd: i32,
}

#[cfg(target_os = "linux")]
impl FileWatcher {
    fn new(path: &Path) -> io::Result<Self> {
        unsafe {
            let fd = inotify_init1(IN_NONBLOCK);
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            
            let path_cstr = std::ffi::CString::new(path.to_str().unwrap())?;
            let watch = inotify_add_watch(fd, path_cstr.as_ptr(), 
                IN_MODIFY | IN_CREATE | IN_DELETE | IN_MOVED_FROM | IN_MOVED_TO);
            
            if watch < 0 {
                libc::close(fd);
                return Err(io::Error::last_os_error());
            }
            
            Ok(FileWatcher { fd })
        }
    }
    
    fn has_event(&self) -> bool {
        unsafe {
            let mut buffer = [0u8; 1024];
            let result = libc::read(self.fd, buffer.as_mut_ptr() as *mut libc::c_void, buffer.len());
            result > 0
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for FileWatcher {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

#[cfg(target_os = "macos")]
struct FileWatcher {
    kq: i32,
    #[allow(dead_code)]
    path: PathBuf,
}

#[cfg(target_os = "macos")]
impl FileWatcher {
    fn new(path: &Path) -> io::Result<Self> {
        unsafe {
            let kq = kqueue();
            if kq < 0 {
                return Err(io::Error::last_os_error());
            }
            
            let fd = libc::open(path.to_str().unwrap().as_ptr() as *const i8, libc::O_RDONLY);
            if fd < 0 {
                libc::close(kq);
                return Err(io::Error::last_os_error());
            }
            
            let mut event: kevent = mem::zeroed();
            event.ident = fd as usize;
            event.filter = EVFILT_VNODE;
            event.flags = EV_ADD | EV_CLEAR;
            event.fflags = NOTE_WRITE;
            
            let result = kevent(kq, &event, 1, std::ptr::null_mut(), 0, std::ptr::null());
            libc::close(fd);
            
            if result < 0 {
                libc::close(kq);
                return Err(io::Error::last_os_error());
            }
            
            Ok(FileWatcher { kq, path: path.to_path_buf() })
        }
    }
    
    fn has_event(&self) -> bool {
        unsafe {
            let mut event: kevent = mem::zeroed();
            let timeout: libc::timespec = mem::zeroed();
            let result = kevent(self.kq, std::ptr::null(), 0, &mut event, 1, &timeout);
            result > 0
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for FileWatcher {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.kq);
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
struct FileWatcher;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
impl FileWatcher {
    fn new(_path: &Path) -> io::Result<Self> {
        Ok(FileWatcher)
    }
    
    fn has_event(&self) -> bool {
        false
    }
}

pub fn run(dir: &str, use_color: bool, tilde_home: bool, save_file: Option<String>) -> io::Result<()> {
    setup_terminal()?;
    
    let result = run_browser(dir, use_color, tilde_home, save_file);
    
    restore_terminal()?;
    
    result
}

fn run_browser(dir: &str, use_color: bool, tilde_home: bool, save_file: Option<String>) -> io::Result<()> {
    let initial_dir = expand_tilde(dir);
    let canonical_dir = initial_dir.canonicalize()
        .unwrap_or_else(|_| initial_dir.clone());
    
    let mut state = State {
        dir: canonical_dir,
        entries: Vec::new(),
        cursor: 0,
        yanked: Vec::new(),
        filter: None,
        show_hidden: SHOW_HIDDEN,
        dirs_first: DIRS_FIRST,
        show_size: SHOW_SIZE,
        version_sort: VERSION_SORT,
        sort_mode: if VERSION_SORT { SortMode::Version } else { SortMode::Name },
        view_offset: 0,
        term_height: 24,
        term_width: 80,
        message: None,
        use_color,
        tilde_home,
        save_file,
        last_dir: None,
        pending_key: None,
        filter_input: None,
        longest_entry_width: 0,
        // Initialize display cache
        cached_dir_display: None,
        cached_counts: (0, 0),
        cache_dirty: true,
    };
    
    update_terminal_size(&mut state)?;
    
    // Load session state if save_file provided
    if let Some(save_file) = state.save_file.clone() {
        load_session(&mut state, &save_file);
    }
    
    load_directory(&mut state)?;
    
    let mut watcher = FileWatcher::new(&state.dir).ok();
    
    loop {
        // Check for terminal resize before rendering
        if update_terminal_size(&mut state)? {
            // Terminal was resized, force a full redraw
            print!("\x1b[2J\x1b[H"); // Clear entire screen and move cursor to home
            io::stdout().flush()?;
        }
        
        render(&mut state)?;
        
        if let Some(ref w) = watcher {
            if w.has_event() {
                load_directory(&mut state)?;
            }
        }
        
        match get_key()? {
            Some(keys) => {
                if let Some(_) = state.filter_input {
                    // Handle inline filtering input
                    if handle_filter_input(&mut state, keys[0])? {
                        load_directory(&mut state)?;
                    }
                } else if let Some(pending) = state.pending_key {
                    // Handle two-key combos
                    state.pending_key = None;
                    let combo_action = handle_two_key_combo(pending, keys.get(0).copied().unwrap_or(0));
                    if let Some(action) = combo_action {
                        if handle_action(action, &mut state, keys[0])? {
                            break;
                        }
                    }
                } else if let Some(action) = keys_to_action(&keys) {
                    if handle_action(action, &mut state, keys[0])? {
                        break;
                    }
                }
                
                // Update watcher when directory changes
                watcher = FileWatcher::new(&state.dir).ok();
            }
            None => continue,
        }
    }
    
    // Save session state before exit
    if let Some(ref save_file) = state.save_file {
        save_session(&state, save_file);
    }
    
    Ok(())
}

fn setup_terminal() -> io::Result<()> {
    unsafe {
        let mut termios = mem::zeroed();
        if tcgetattr(0, &mut termios) != 0 {
            return Err(io::Error::last_os_error());
        }
        ORIG_TERMIOS = Some(termios.clone());
        
        termios.c_lflag &= !(ECHO | ICANON);
        termios.c_cc[VMIN] = 1;
        termios.c_cc[VTIME] = 0;
        
        if tcsetattr(0, TCSANOW, &termios) != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    
    print!("\x1b[?1049h");  // Alt screen
    print!("\x1b[?25l");     // Hide cursor
    io::stdout().flush()
}

fn restore_terminal() -> io::Result<()> {
    unsafe {
        if let Some(termios) = ORIG_TERMIOS {
            tcsetattr(0, TCSANOW, &termios);
        }
    }
    
    print!("\x1b[?25h");     // Show cursor
    print!("\x1b[?1049l");   // Exit alt screen
    print!("\x1b[0m");       // Reset colors
    io::stdout().flush()
}

fn update_terminal_size(state: &mut State) -> io::Result<bool> {
    unsafe {
        let mut ws: libc::winsize = mem::zeroed();
        if libc::ioctl(0, libc::TIOCGWINSZ, &mut ws) == 0 {
            let new_height = ws.ws_row as usize;
            let new_width = ws.ws_col as usize;
            
            // Check if size actually changed
            if state.term_height != new_height || state.term_width != new_width {
                state.term_height = new_height;
                state.term_width = new_width;
                state.cache_dirty = true; // Invalidate cache on resize
                adjust_view_offset(state);
                return Ok(true); // Size changed
            }
        }
    }
    Ok(false) // No change
}

fn load_session(state: &mut State, save_file: &str) {
    if let Ok(content) = fs::read_to_string(save_file) {
        for line in content.lines() {
            if let Some((key, value)) = line.split_once('=') {
                match key {
                    "last_dir" => {
                        let path = PathBuf::from(value);
                        if path.exists() && path.is_dir() {
                            state.last_dir = Some(path);
                        }
                    }
                    "current_dir" => {
                        let path = PathBuf::from(value);
                        if path.exists() && path.is_dir() {
                            state.dir = path;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn save_session(state: &State, save_file: &str) {
    let mut content = String::new();
    content.push_str(&format!("current_dir={}\n", state.dir.display()));
    if let Some(ref last_dir) = state.last_dir {
        content.push_str(&format!("last_dir={}\n", last_dir.display()));
    }
    
    let _ = fs::write(save_file, content);
}

fn format_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "K", "M", "G", "T"];
    let mut size = size as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{:.0}{}", size, UNITS[unit_index])
    } else {
        format!("{:.1}{}", size, UNITS[unit_index])
    }
}

fn version_compare(a: &str, b: &str) -> Ordering {
    // Simple version number comparison - extract numeric parts
    let extract_numbers = |s: &str| -> Vec<u32> {
        let mut nums = Vec::new();
        let mut current_num = String::new();
        
        for ch in s.chars() {
            if ch.is_ascii_digit() {
                current_num.push(ch);
            } else {
                if !current_num.is_empty() {
                    if let Ok(n) = current_num.parse::<u32>() {
                        nums.push(n);
                    }
                    current_num.clear();
                }
            }
        }
        
        if !current_num.is_empty() {
            if let Ok(n) = current_num.parse::<u32>() {
                nums.push(n);
            }
        }
        
        nums
    };
    
    let nums_a = extract_numbers(a);
    let nums_b = extract_numbers(b);
    
    for (na, nb) in nums_a.iter().zip(nums_b.iter()) {
        match na.cmp(nb) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    
    nums_a.len().cmp(&nums_b.len())
}

fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") || path == "~" {
        if let Ok(home) = env::var("HOME") {
            let path = if path == "~" {
                home
            } else {
                format!("{}{}", home, &path[1..])
            };
            return PathBuf::from(path);
        }
    }
    PathBuf::from(path)
}

fn load_directory(state: &mut State) -> io::Result<()> {
    let mut entries = Vec::new();
    
    let read_dir = match fs::read_dir(&state.dir) {
        Ok(rd) => rd,
        Err(e) => {
            state.message = Some(format!("Error reading directory: {}", e));
            return Ok(());
        }
    };
    
    for entry in read_dir {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        
        if !state.show_hidden && name.starts_with('.') {
            continue;
        }
        
        if let Some(ref filter) = state.filter {
            if !name.to_lowercase().contains(&filter.to_lowercase()) {
                continue;
            }
        }
        
        let metadata = entry.metadata()?;
        let path = entry.path();
        
        let is_link = match entry.file_type() {
            Ok(ft) => ft.is_symlink(),
            Err(_) => false,
        };
        
        entries.push(Entry {
            name,
            path: path.clone(),
            is_dir: metadata.is_dir(),
            is_exec: !metadata.is_dir() && (metadata.permissions().mode() & 0o111 != 0),
            is_link,
            size: metadata.len(),
            marked: false,
            mtime: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        });
    }
    
    entries.sort_by(|a, b| {
        if state.dirs_first {
            match (a.is_dir, b.is_dir) {
                (true, false) => return Ordering::Less,
                (false, true) => return Ordering::Greater,
                _ => {}
            }
        }
        
        match state.sort_mode {
            SortMode::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortMode::Size => b.size.cmp(&a.size),
            SortMode::Time => b.mtime.cmp(&a.mtime),
            SortMode::Version => version_compare(&a.name, &b.name),
        }
    });
    
    // Optimize: Use references to avoid cloning paths for marked preservation
    let old_marked: HashSet<&Path> = state.entries.iter()
        .filter(|e| e.marked)
        .map(|e| e.path.as_path())
        .collect();
    
    for entry in &mut entries {
        if old_marked.contains(entry.path.as_path()) {
            entry.marked = true;
        }
    }
    
    state.entries = entries;
    state.cache_dirty = true; // Invalidate cache when entries change
    
    // Calculate longest entry width for column alignment (like original C code)
    state.longest_entry_width = state.entries.iter()
        .map(|entry| {
            let suffix = if entry.is_dir {
                "/"
            } else if entry.is_link {
                "@"  
            } else if entry.is_exec {
                "*"
            } else {
                ""
            };
            entry.name.chars().count() + suffix.chars().count()
        })
        .max()
        .unwrap_or(0);
    
    if state.cursor >= state.entries.len() {
        state.cursor = state.entries.len().saturating_sub(1);
    }
    
    adjust_view_offset(state);
    
    Ok(())
}

fn adjust_view_offset(state: &mut State) {
    let view_height = state.term_height.saturating_sub(4); // Header + footer
    
    if state.cursor < state.view_offset {
        state.view_offset = state.cursor;
    } else if state.cursor >= state.view_offset + view_height {
        state.view_offset = state.cursor.saturating_sub(view_height - 1);
    }
}

fn update_display_cache(state: &mut State) {
    if !state.cache_dirty {
        return;
    }
    
    // Update directory display cache
    state.cached_dir_display = Some(if state.tilde_home {
        if let Ok(home) = env::var("HOME") {
            let dir_str = state.dir.to_string_lossy();
            if dir_str.starts_with(&home) {
                format!("~{}", &dir_str[home.len()..])
            } else {
                dir_str.to_string()
            }
        } else {
            state.dir.display().to_string()
        }
    } else if state.dir.to_string_lossy() == "/" {
        "/".to_string()
    } else {
        state.dir.display().to_string()
    });
    
    // Update counts cache
    let marked_count = state.entries.iter().filter(|e| e.marked).count();
    let yanked_count = state.yanked.len();
    state.cached_counts = (marked_count, yanked_count);
    
    state.cache_dirty = false;
}

fn render(state: &mut State) -> io::Result<()> {
    print!("\x1b[H\x1b[2J"); // Clear screen
    
    // Update cache if dirty
    update_display_cache(state);
    
    // Header - use cached directory display
    let dir_display = state.cached_dir_display.as_ref().unwrap();
    
    if state.use_color {
        println!(" \x1b[1m{}\x1b[0m", dir_display);
    } else {
        println!(" {}", dir_display);
    }
    
    if let Some(ref filter) = state.filter {
        println!(" Filter: {}", filter);
    }
    
    // Header bottom border (no gap, like original C implementation)
    println!("{}", "─".repeat(state.term_width));
    
    // Entries
    let view_height = state.term_height.saturating_sub(5); // Header(2) + border(1) + footer(1) + margin(1)
    let end = (state.view_offset + view_height).min(state.entries.len());
    
    for i in state.view_offset..end {
        if let Some(entry) = state.entries.get(i) {
            // Cursor gets exactly one space, others align to match
            let prefix = if i == state.cursor {
                if entry.marked {
                    ">* "  // Use string literals instead of format!
                } else {
                    "> "  // Use string literals instead of format!
                }
            } else {
                if entry.marked {
                    " * "  // Use string literals instead of format!
                } else {
                    "  "  // Use string literals instead of format!
                }
            };
            
            let (color_start, color_end) = if state.use_color {
                let color = if entry.marked {
                    COLOR_MARKED
                } else if entry.is_link {
                    COLOR_LINK
                } else if entry.is_dir {
                    COLOR_DIR
                } else if entry.is_exec {
                    COLOR_EXEC
                } else {
                    COLOR_FILE
                };
                (color, COLOR_RESET)
            } else {
                ("", "")
            };
            
            let suffix = if entry.is_dir {
                "/"
            } else if entry.is_link {
                "@"
            } else if entry.is_exec {
                "*"
            } else {
                ""
            };
            
            // Calculate name width without allocating string
            let name_width = entry.name.chars().count() + suffix.chars().count();
            
            // Pad name to longest entry width for column alignment (like original)
            let padding_needed = state.longest_entry_width.saturating_sub(name_width);
            
            // Print directly to avoid string allocations
            print!(" {}{}{}{}", prefix, color_start, &entry.name, suffix);
            
            // Print padding spaces directly
            for _ in 0..padding_needed {
                print!(" ");
            }
            
            print!("{}", color_end);
            
            // Print size if needed
            if state.show_size {
                if !entry.is_dir {
                    print!(" {:>8}", format_size(entry.size));
                } else {
                    print!("         "); // 9 spaces for alignment with size column
                }
            }
            
            println!(); // End the line
        }
    }
    
    // Fill remaining lines
    for _ in end - state.view_offset..view_height {
        println!();
    }
    
    // Footer / Filter Input / Message
    if let Some(ref filter_input) = state.filter_input {
        // Show filter input prompt
        print!(" Filter: {}", filter_input);
        if state.use_color {
            print!("{}█{}", COLOR_ERROR, COLOR_RESET); // Show cursor
        } else {
            print!("_"); // Simple cursor
        }
    } else if let Some(ref msg) = state.message {
        if state.use_color {
            print!(" {}{}{}", COLOR_ERROR, msg, COLOR_RESET);
        } else {
            print!(" {}", msg);
        }
    } else {
        // Use cached counts for better performance
        let (marked_count, yanked_count) = state.cached_counts;
        
        print!(" {}/{}", state.cursor + 1, state.entries.len());
        
        if marked_count > 0 {
            print!(" [{}*]", marked_count);
        }
        
        if yanked_count > 0 {
            print!(" [{} yanked]", yanked_count);
        }
        
        if let Some(ref current_filter) = state.filter {
            print!(" (filtered: {})", current_filter);
        }
    }
    
    io::stdout().flush()
}

fn get_key() -> io::Result<Option<Vec<u8>>> {
    let mut buf = [0u8; 4];
    
    // Set non-blocking read with timeout
    unsafe {
        let mut fds: libc::fd_set = mem::zeroed();
        libc::FD_SET(0, &mut fds);
        
        let mut timeout = libc::timeval {
            tv_sec: 0,
            tv_usec: 100000, // 100ms timeout
        };
        
        let result = libc::select(1, &mut fds, std::ptr::null_mut(), std::ptr::null_mut(), &mut timeout);
        if result <= 0 {
            return Ok(None);
        }
        
        // Read directly from file descriptor 0 (stdin) to avoid buffering issues
        let n = libc::read(0, buf.as_mut_ptr() as *mut libc::c_void, 1);
        if n != 1 {
            return Ok(None);
        }
        
        if buf[0] == 27 { // ESC sequence
            // Check if more data is available for escape sequence
            let mut fds: libc::fd_set = mem::zeroed();
            libc::FD_SET(0, &mut fds);
            
            let mut timeout = libc::timeval {
                tv_sec: 0,
                tv_usec: 10000, // 10ms timeout for escape sequences
            };
            
            let result = libc::select(1, &mut fds, std::ptr::null_mut(), std::ptr::null_mut(), &mut timeout);
            if result > 0 {
                // Try to read the next byte
                let n = libc::read(0, buf.as_mut_ptr().add(1) as *mut libc::c_void, 1);
                if n == 1 && buf[1] == b'[' {
                    // This looks like an arrow key sequence, read the third byte
                    let mut fds: libc::fd_set = mem::zeroed();
                    libc::FD_SET(0, &mut fds);
                    
                    let mut timeout = libc::timeval {
                        tv_sec: 0,
                        tv_usec: 10000, // 10ms timeout for the third byte
                    };
                    
                    let result = libc::select(1, &mut fds, std::ptr::null_mut(), std::ptr::null_mut(), &mut timeout);
                    if result > 0 {
                        let n = libc::read(0, buf.as_mut_ptr().add(2) as *mut libc::c_void, 1);
                        if n == 1 {
                            // Successfully read the arrow key sequence
                            return Ok(Some(vec![buf[0], buf[1], buf[2]]));
                        }
                    }
                    // Failed to read third byte, return what we have
                    return Ok(Some(vec![buf[0], buf[1]]));
                }
            }
            // No additional data or not an arrow sequence, treat as standalone ESC
            return Ok(Some(vec![buf[0]]));
        } else {
            return Ok(Some(vec![buf[0]]));
        }
    }
}

fn handle_filter_input(state: &mut State, key: u8) -> io::Result<bool> {
    if let Some(ref mut filter_input) = state.filter_input {
        match key {
            10 | 13 => { // Enter
                // Apply the filter
                if filter_input.is_empty() {
                    state.filter = None;
                } else {
                    state.filter = Some(filter_input.clone());
                }
                state.filter_input = None;
                state.cursor = 0;
                state.view_offset = 0;
                return Ok(true); // Reload directory
            }
            27 => { // ESC
                // Cancel filtering
                state.filter_input = None;
                return Ok(false);
            }
            127 | 8 => { // Backspace
                filter_input.pop();
                return Ok(false);
            }
            32..=126 => { // Printable ASCII
                filter_input.push(key as char);
                return Ok(false);
            }
            _ => return Ok(false),
        }
    }
    Ok(false)
}

fn handle_two_key_combo(first: u8, second: u8) -> Option<Action> {
    match (first, second) {
        (b'g', b'g') => Some(Action::Home),
        (b'g', key) => {
            // g<key> for directory jumps
            Some(Action::Jump(key))
        }
        (b'\'', key) => {
            // '<key> for directory jumps  
            Some(Action::Jump(key))
        }
        (b'D', b'D') => Some(Action::Delete),
        (b'n', b'f') => Some(Action::MakeFile),
        (b'n', b'd') => Some(Action::MakeDir),
        _ => None,
    }
}

fn keys_to_action(keys: &[u8]) -> Option<Action> {
    // Check for arrow keys (ESC sequences)
    if keys.len() == 3 && keys[0] == 27 && keys[1] == b'[' {
        return match keys[2] {
            b'A' => Some(Action::Previous), // Up arrow
            b'B' => Some(Action::Next),     // Down arrow
            b'C' => Some(Action::Enter),    // Right arrow
            b'D' => Some(Action::Back),     // Left arrow
            b'H' => Some(Action::Home),     // Home
            b'F' => Some(Action::End),      // End
            _ => None,
        };
    }
    
    // Single key bindings
    if keys.len() == 1 {
        let key = keys[0];
        
        for &(k, action) in KEYBINDS {
            if k == key {
                return Some(action);
            }
        }
        
        for &(k, _) in DIR_JUMPS {
            if k == key {
                return Some(Action::Jump(k));
            }
        }
    }
    
    None
}

fn handle_delete_action(state: &mut State) -> io::Result<()> {
    // Optimize: Use references first, only clone when needed for deletion
    let marked_paths: Vec<&Path> = state.entries.iter()
        .filter(|e| e.marked)
        .map(|e| e.path.as_path())
        .collect();
    
    let paths_to_delete: Vec<PathBuf> = if marked_paths.is_empty() {
        state.entries.get(state.cursor)
            .map(|e| vec![e.path.clone()])
            .unwrap_or_default()
    } else {
        marked_paths.into_iter().map(|p| p.to_path_buf()).collect()
    };
    let to_delete = paths_to_delete;
    
    if !to_delete.is_empty() {
        restore_terminal()?;
        print!("Delete {} item(s)? [y/N] ", to_delete.len());
        io::stdout().flush()?;
        
        let mut response = String::new();
        io::stdin().read_line(&mut response)?;
        
        setup_terminal()?;
        
        if response.trim().to_lowercase() == "y" {
            let mut errors = Vec::new();
            
            for path in to_delete {
                let result = if path.is_dir() {
                    fs::remove_dir_all(&path)
                } else {
                    fs::remove_file(&path)
                };
                
                if let Err(e) = result {
                    errors.push(format!("{}: {}", path.display(), e));
                }
            }
            
            if errors.is_empty() {
                state.message = Some("Deleted".to_string());
            } else {
                state.message = Some(format!("Errors: {}", errors.join(", ")));
            }
            
            load_directory(state)?;
        }
    }
    Ok(())
}

fn handle_file_operations(action: Action, state: &mut State) -> io::Result<()> {
    match action {
        Action::MoveFiles => {
            if state.yanked.is_empty() {
                state.message = Some("Nothing to move".to_string());
            } else {
                let mut success = 0;
                let mut errors = Vec::new();
                
                for path in &state.yanked {
                    let dest = state.dir.join(path.file_name().unwrap());
                    
                    match move_file(path, &dest) {
                        Ok(_) => success += 1,
                        Err(e) => errors.push(format!("{}: {}", path.display(), e)),
                    }
                }
                
                if errors.is_empty() {
                    state.message = Some(format!("Moved {} item(s)", success));
                    state.yanked.clear();
                } else {
                    state.message = Some(format!("Errors: {}", errors.join(", ")));
                }
                
                load_directory(state)?;
            }
        }
        
        Action::Paste => {
            if state.yanked.is_empty() {
                state.message = Some("Nothing to paste".to_string());
            } else {
                let mut success = 0;
                let mut errors = Vec::new();
                
                for path in &state.yanked {
                    let dest = state.dir.join(path.file_name().unwrap());
                    
                    match copy_recursive(path, &dest) {
                        Ok(_) => success += 1,
                        Err(e) => errors.push(format!("{}: {}", path.display(), e)),
                    }
                }
                
                if errors.is_empty() {
                    state.message = Some(format!("Pasted {} item(s)", success));
                } else {
                    state.message = Some(format!("Errors: {}", errors.join(", ")));
                }
                
                load_directory(state)?;
            }
        }
        
        Action::Link => {
            if state.yanked.is_empty() {
                state.message = Some("Nothing to link".to_string());
            } else {
                let mut success = 0;
                let mut errors = Vec::new();
                
                for path in &state.yanked {
                    let dest = state.dir.join(path.file_name().unwrap());
                    
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::symlink;
                        match symlink(path, &dest) {
                            Ok(_) => success += 1,
                            Err(e) => errors.push(format!("{}: {}", path.display(), e)),
                        }
                    }
                }
                
                if errors.is_empty() {
                    state.message = Some(format!("Created {} link(s)", success));
                } else {
                    state.message = Some(format!("Errors: {}", errors.join(", ")));
                }
                
                load_directory(state)?;
            }
        }
        
        _ => unreachable!(),
    }
    Ok(())
}

fn handle_action(action: Action, state: &mut State, _key: u8) -> io::Result<bool> {
    state.message = None;
    
    match action {
        Action::Quit => return Ok(true),
        
        Action::Next => {
            if state.cursor < state.entries.len().saturating_sub(1) {
                state.cursor += 1;
                adjust_view_offset(state);
            }
        }
        
        Action::Previous => {
            state.cursor = state.cursor.saturating_sub(1);
            adjust_view_offset(state);
        }
        
        Action::Home => {
            state.cursor = 0;
            state.view_offset = 0;
        }
        
        Action::End => {
            state.cursor = state.entries.len().saturating_sub(1);
            adjust_view_offset(state);
        }
        
        Action::PageUp => {
            let page_size = state.term_height.saturating_sub(4);
            state.cursor = state.cursor.saturating_sub(page_size);
            adjust_view_offset(state);
        }
        
        Action::PageDown => {
            let page_size = state.term_height.saturating_sub(4);
            state.cursor = (state.cursor + page_size).min(state.entries.len().saturating_sub(1));
            adjust_view_offset(state);
        }
        
        Action::Enter => {
            if let Some(entry) = state.entries.get(state.cursor) {
                if entry.is_dir {
                    // Canonicalize the path to ensure it's absolute
                    let new_dir = entry.path.canonicalize()
                        .unwrap_or_else(|_| entry.path.clone());
                    state.dir = new_dir;
                    state.cursor = 0;
                    state.view_offset = 0;
                    state.filter = None;
                    state.cache_dirty = true; // Invalidate cache when directory changes
                    load_directory(state)?;
                } else {
                    open_file(&entry.path)?;
                }
            }
        }
        
        Action::Back => {
            if let Some(parent) = state.dir.parent() {
                let old_dir = state.dir.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string());
                
                state.dir = parent.to_path_buf();
                state.cursor = 0;
                state.view_offset = 0;
                state.filter = None;
                load_directory(state)?;
                
                if let Some(old) = old_dir {
                    for (i, entry) in state.entries.iter().enumerate() {
                        if entry.name == old {
                            state.cursor = i;
                            adjust_view_offset(state);
                            break;
                        }
                    }
                }
            }
        }
        
        Action::Mark => {
            if let Some(entry) = state.entries.get_mut(state.cursor) {
                entry.marked = !entry.marked;
                state.cache_dirty = true; // Invalidate cache when marking changes
                if state.cursor < state.entries.len() - 1 {
                    state.cursor += 1;
                    adjust_view_offset(state);
                }
            }
        }
        
        Action::UnmarkAll => {
            for entry in &mut state.entries {
                entry.marked = false;
            }
            state.cache_dirty = true; // Invalidate cache when unmarking all
        }
        
        Action::Yank => {
            // Optimize: Clear and rebuild yanked list to avoid cloning
            state.yanked.clear();
            state.yanked.extend(
                state.entries.iter()
                    .filter(|e| e.marked)
                    .map(|e| e.path.clone())
            );
            
            if state.yanked.is_empty() {
                if let Some(entry) = state.entries.get(state.cursor) {
                    state.yanked.push(entry.path.clone());
                }
            }
            
            if !state.yanked.is_empty() {
                state.message = Some(format!("Yanked {} item(s)", state.yanked.len()));
            }
            state.cache_dirty = true; // Invalidate cache when yank list changes
        }
        
        Action::MoveFiles | Action::Paste | Action::Link => {
            handle_file_operations(action, state)?;
        }
        
        Action::Delete => {
            handle_delete_action(state)?;
        }
        
        Action::Rename => {
            if let Some(entry) = state.entries.get(state.cursor) {
                restore_terminal()?;
                print!("Rename '{}' to: ", entry.name);
                io::stdout().flush()?;
                
                let mut new_name = String::new();
                io::stdin().read_line(&mut new_name)?;
                let new_name = new_name.trim();
                
                setup_terminal()?;
                
                if !new_name.is_empty() && new_name != entry.name {
                    let new_path = state.dir.join(new_name);
                    
                    match fs::rename(&entry.path, &new_path) {
                        Ok(_) => {
                            state.message = Some("Renamed".to_string());
                            load_directory(state)?;
                        }
                        Err(e) => {
                            state.message = Some(format!("Error: {}", e));
                        }
                    }
                }
            }
        }
        
        Action::MakeDir => {
            restore_terminal()?;
            print!("New directory name: ");
            io::stdout().flush()?;
            
            let mut name = String::new();
            io::stdin().read_line(&mut name)?;
            let name = name.trim();
            
            setup_terminal()?;
            
            if !name.is_empty() {
                let path = state.dir.join(name);
                
                match fs::create_dir(&path) {
                    Ok(_) => {
                        state.message = Some("Directory created".to_string());
                        load_directory(state)?;
                    }
                    Err(e) => {
                        state.message = Some(format!("Error: {}", e));
                    }
                }
            }
        }
        
        Action::Shell => {
            restore_terminal()?;
            
            let shell = env::var("SHELL").unwrap_or_else(|_| DEFAULT_SHELL.to_string());
            Command::new(&shell)
                .current_dir(&state.dir)
                .status()?;
            
            setup_terminal()?;
            let _ = update_terminal_size(state)?;
            load_directory(state)?;
        }
        
        Action::Filter => {
            // Start inline filtering mode
            state.filter_input = Some(String::new());
        }
        
        Action::ToggleHidden => {
            state.show_hidden = !state.show_hidden;
            state.cursor = 0;
            state.view_offset = 0;
            load_directory(state)?;
        }
        
        Action::ToggleDirsFirst => {
            state.dirs_first = !state.dirs_first;
            load_directory(state)?;
        }
        
        Action::SortByName => {
            state.sort_mode = SortMode::Name;
            load_directory(state)?;
        }
        
        Action::SortBySize => {
            state.sort_mode = SortMode::Size;
            load_directory(state)?;
        }
        
        Action::SortByTime => {
            state.sort_mode = SortMode::Time;
            load_directory(state)?;
        }
        
        Action::Reload => {
            load_directory(state)?;
            state.message = Some("Reloaded".to_string());
        }
        
        Action::Jump(key) => {
            // Handle special case for lastdir  
            if key == b'\'' {
                if let Some(last) = state.last_dir.take() {
                    let temp = std::mem::replace(&mut state.dir, last);
                    state.last_dir = Some(temp);
                    state.cursor = 0;
                    state.view_offset = 0;
                    state.filter = None;
                    load_directory(state)?;
                } else {
                    state.message = Some("No previous directory".to_string());
                }
                return Ok(false);
            }
            
            for &(k, path) in DIR_JUMPS {
                if k == key && !path.is_empty() {
                    let new_dir = expand_tilde(path);
                    if new_dir.exists() && new_dir.is_dir() {
                        // Canonicalize the path to ensure it's absolute
                        let canonical_dir = new_dir.canonicalize().unwrap_or(new_dir);
                        state.last_dir = Some(std::mem::replace(&mut state.dir, canonical_dir));
                        state.cursor = 0;
                        state.view_offset = 0;
                        state.filter = None;
                        load_directory(state)?;
                    } else {
                        state.message = Some(format!("Directory not found: {}", path));
                    }
                    break;
                }
            }
        }
        
        Action::UnYank => {
            state.yanked.clear();
            state.message = Some("Cleared yanked items".to_string());
        }
        
        
        Action::ToggleSize => {
            state.show_size = !state.show_size;
            state.message = Some(format!("Show size: {}", if state.show_size { "on" } else { "off" }));
        }
        
        Action::ToggleVersionSort => {
            state.version_sort = !state.version_sort;
            if state.version_sort {
                state.sort_mode = SortMode::Version;
            } else {
                state.sort_mode = SortMode::Name;
            }
            load_directory(state)?;
            state.message = Some(format!("Version sort: {}", if state.version_sort { "on" } else { "off" }));
        }
        
        Action::HalfPageUp => {
            let half_page = (state.term_height / 2).max(1);
            state.cursor = state.cursor.saturating_sub(half_page);
            adjust_view_offset(state);
        }
        
        Action::HalfPageDown => {
            let half_page = (state.term_height / 2).max(1);
            state.cursor = (state.cursor + half_page).min(state.entries.len().saturating_sub(1));
            adjust_view_offset(state);
        }
        
        Action::MakeFile => {
            restore_terminal()?;
            print!("New file name: ");
            io::stdout().flush()?;
            
            let mut name = String::new();
            io::stdin().read_line(&mut name)?;
            let name = name.trim();
            
            setup_terminal()?;
            
            if !name.is_empty() {
                let path = state.dir.join(name);
                
                // Create parent directories if needed
                if let Some(parent) = path.parent() {
                    if !parent.exists() {
                        fs::create_dir_all(parent)?;
                    }
                }
                
                match fs::File::create(&path) {
                    Ok(_) => {
                        state.message = Some("File created".to_string());
                        load_directory(state)?;
                    }
                    Err(e) => {
                        state.message = Some(format!("Error: {}", e));
                    }
                }
            }
        }
        
        Action::Redraw => {
            // Force redraw by clearing message
            state.message = None;
        }
        
        Action::ChangeDir => {
            restore_terminal()?;
            print!("Change to directory: ");
            io::stdout().flush()?;
            
            let mut dir_input = String::new();
            io::stdin().read_line(&mut dir_input)?;
            let dir_input = dir_input.trim();
            
            setup_terminal()?;
            
            if !dir_input.is_empty() {
                let new_dir = expand_tilde(dir_input);
                if new_dir.exists() && new_dir.is_dir() {
                    let canonical_dir = new_dir.canonicalize().unwrap_or(new_dir);
                    state.last_dir = Some(std::mem::replace(&mut state.dir, canonical_dir));
                    state.cursor = 0;
                    state.view_offset = 0;
                    state.filter = None;
                    load_directory(state)?;
                } else {
                    state.message = Some(format!("Directory not found: {}", dir_input));
                }
            }
        }
        
        Action::EditFile => {
            if let Some(entry) = state.entries.get(state.cursor) {
                restore_terminal()?;
                
                let editor = env::var("EDITOR").unwrap_or_else(|_| DEFAULT_EDITOR.to_string());
                let result = Command::new(&editor)
                    .arg(&entry.path)
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .status();
                
                setup_terminal()?;
                let _ = update_terminal_size(state)?;
                load_directory(state)?;
                
                if let Err(e) = result {
                    state.message = Some(format!("Error running editor: {}", e));
                }
            }
        }
        
        Action::MediaPlayer => {
            if let Some(entry) = state.entries.get(state.cursor) {
                restore_terminal()?;
                
                let player = env::var("NOICEMP").unwrap_or_else(|_| DEFAULT_MEDIA_PLAYER.to_string());
                let parts: Vec<&str> = player.split_whitespace().collect();
                let (cmd, args) = if let Some((first, rest)) = parts.split_first() {
                    (*first, rest)
                } else {
                    (player.as_str(), &[][..])
                };
                
                let result = Command::new(cmd)
                    .args(args)
                    .arg(&entry.path)
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .status();
                
                setup_terminal()?;
                let _ = update_terminal_size(state)?;
                
                if let Err(e) = result {
                    state.message = Some(format!("Error running media player: {}", e));
                }
            }
        }
        
        Action::TopMonitor => {
            restore_terminal()?;
            
            let top = env::var("NOICETOP").unwrap_or_else(|_| DEFAULT_TOP.to_string());
            let result = Command::new(&top)
                .current_dir(&state.dir)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status();
            
            setup_terminal()?;
            let _ = update_terminal_size(state)?;
            
            if let Err(e) = result {
                state.message = Some(format!("Error running top: {}", e));
            }
        }
        
        Action::ShowHelp => {
            restore_terminal()?;
            
            let man_cmd = env::var("NOICEMAN").unwrap_or_else(|_| DEFAULT_MAN_COMMAND.to_string());
            let parts: Vec<&str> = man_cmd.split_whitespace().collect();
            let (cmd, args) = if let Some((first, rest)) = parts.split_first() {
                (*first, rest)
            } else {
                (man_cmd.as_str(), &[][..])
            };
            
            let result = Command::new(cmd)
                .args(args)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status();
            
            setup_terminal()?;
            let _ = update_terminal_size(state)?;
            
            if let Err(e) = result {
                state.message = Some(format!("Error running help: {}", e));
            }
        }
        
        Action::PendingKey(key) => {
            state.pending_key = Some(key);
            return Ok(false);
        }
    }
    
    Ok(false)
}

fn open_file(path: &Path) -> io::Result<()> {
    let name = path.to_string_lossy();
    
    for &(pattern, cmd) in FILE_RULES {
        if simple_match(pattern, &name) {
            restore_terminal()?;
            
            let result = Command::new(cmd)
                .arg(path)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status();
            
            setup_terminal()?;
            
            return result.map(|_| ());
        }
    }
    
    restore_terminal()?;
    
    let pager = env::var("PAGER").unwrap_or_else(|_| DEFAULT_PAGER.to_string());
    let result = Command::new(&pager)
        .arg(path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();
    
    setup_terminal()?;
    
    result.map(|_| ())
}

fn simple_match(pattern: &str, text: &str) -> bool {
    if pattern.starts_with(r"\.") && pattern.ends_with("$") {
        let suffix = &pattern[2..pattern.len()-1];
        text.ends_with(&format!(".{}", suffix))
    } else {
        text.contains(pattern)
    }
}

fn copy_recursive(from: &Path, to: &Path) -> io::Result<()> {
    if from.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            let to = to.join(entry.file_name());
            copy_recursive(&entry.path(), &to)?;
        }
    } else {
        if to.exists() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, 
                format!("{} already exists", to.display())));
        }
        fs::copy(from, to)?;
    }
    Ok(())
}

fn move_file(from: &Path, to: &Path) -> io::Result<()> {
    if to.exists() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, 
            format!("{} already exists", to.display())));
    }
    
    fs::rename(from, to).or_else(|_| {
        copy_recursive(from, to)?;
        if from.is_dir() {
            fs::remove_dir_all(from)
        } else {
            fs::remove_file(from)
        }
    })
}