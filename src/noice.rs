use std::fs;
use std::io::{self, Write, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::cmp::Ordering;
use std::process::{Command, Stdio};
use std::env;
use libc::{termios, tcgetattr, tcsetattr, TCSANOW, ECHO, ICANON, VMIN, VTIME};
use std::mem;

#[cfg(target_os = "linux")]
use libc::{inotify_init1, inotify_add_watch, inotify_event, IN_NONBLOCK, IN_MODIFY, IN_CREATE, IN_DELETE, IN_MOVED_FROM, IN_MOVED_TO};

#[cfg(target_os = "macos")]
use libc::{kqueue, kevent, EVFILT_VNODE, EV_ADD, EV_CLEAR, NOTE_WRITE};

include!("../config.rs");

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
    dir: PathBuf,
    entries: Vec<Entry>,
    cursor: usize,
    yanked: Vec<PathBuf>,
    filter: Option<String>,
    show_hidden: bool,
    dirs_first: bool,
    sort_mode: SortMode,
    view_offset: usize,
    term_height: usize,
    term_width: usize,
    message: Option<String>,
    yank_mode: YankMode,
}

#[derive(Clone, Copy)]
pub enum SortMode {
    Name,
    Size,
    Time,
}

#[derive(Clone, Copy)]
pub enum YankMode {
    Copy,
    Move,
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

pub fn run(dir: &str) -> io::Result<()> {
    setup_terminal()?;
    
    let result = run_browser(dir);
    
    restore_terminal()?;
    
    result
}

fn run_browser(dir: &str) -> io::Result<()> {
    let mut state = State {
        dir: expand_tilde(dir),
        entries: Vec::new(),
        cursor: 0,
        yanked: Vec::new(),
        filter: None,
        show_hidden: SHOW_HIDDEN,
        dirs_first: DIRS_FIRST,
        sort_mode: SortMode::Name,
        view_offset: 0,
        term_height: 24,
        term_width: 80,
        message: None,
        yank_mode: YankMode::Copy,
    };
    
    update_terminal_size(&mut state)?;
    load_directory(&mut state)?;
    
    let mut watcher = FileWatcher::new(&state.dir).ok();
    
    loop {
        render(&state)?;
        
        if let Some(ref w) = watcher {
            if w.has_event() {
                load_directory(&mut state)?;
            }
        }
        
        match get_key()? {
            Some(keys) => {
                if let Some(action) = keys_to_action(&keys) {
                    if handle_action(action, &mut state, keys[0])? {
                        break;
                    }
                    
                    // Update watcher when directory changes
                    watcher = FileWatcher::new(&state.dir).ok();
                }
            }
            None => continue,
        }
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

fn update_terminal_size(state: &mut State) -> io::Result<()> {
    unsafe {
        let mut ws: libc::winsize = mem::zeroed();
        if libc::ioctl(0, libc::TIOCGWINSZ, &mut ws) == 0 {
            state.term_height = ws.ws_row as usize;
            state.term_width = ws.ws_col as usize;
        }
    }
    Ok(())
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
        }
    });
    
    let old_marked: Vec<PathBuf> = state.entries.iter()
        .filter(|e| e.marked)
        .map(|e| e.path.clone())
        .collect();
    
    for entry in &mut entries {
        if old_marked.contains(&entry.path) {
            entry.marked = true;
        }
    }
    
    state.entries = entries;
    
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

fn render(state: &State) -> io::Result<()> {
    print!("\x1b[H\x1b[2J"); // Clear screen
    
    // Header
    let dir_display = if state.dir.to_string_lossy() == "/" {
        "/".to_string()
    } else {
        state.dir.display().to_string()
    };
    
    if USE_COLOR {
        println!("\x1b[1m{}\x1b[0m", dir_display);
    } else {
        println!("{}", dir_display);
    }
    
    if let Some(ref filter) = state.filter {
        println!("Filter: {}", filter);
    } else {
        println!();
    }
    
    // Entries
    let view_height = state.term_height.saturating_sub(4);
    let end = (state.view_offset + view_height).min(state.entries.len());
    
    for i in state.view_offset..end {
        if let Some(entry) = state.entries.get(i) {
            let cursor = if i == state.cursor { CURSOR } else { NO_CURSOR };
            let mark = if entry.marked { YANK_SYMBOL } else { "  " };
            
            let (color_start, color_end) = if USE_COLOR {
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
            
            let name = format!("{}{}", entry.name, suffix);
            let max_width = state.term_width.saturating_sub(10);
            let truncated = if name.chars().count() > max_width {
                let mut truncated = String::new();
                let mut char_count = 0;
                for ch in name.chars() {
                    if char_count >= max_width - 3 {
                        break;
                    }
                    truncated.push(ch);
                    char_count += 1;
                }
                format!("{}...", truncated)
            } else {
                name
            };
            
            println!("{}{}{}{}{}", cursor, mark, color_start, truncated, color_end);
        }
    }
    
    // Fill remaining lines
    for _ in end - state.view_offset..view_height {
        println!();
    }
    
    // Footer / Message
    if let Some(ref msg) = state.message {
        if USE_COLOR {
            print!("{}{}{}", COLOR_ERROR, msg, COLOR_RESET);
        } else {
            print!("{}", msg);
        }
    } else {
        let marked_count = state.entries.iter().filter(|e| e.marked).count();
        let yanked_count = state.yanked.len();
        
        print!("{}/{}", state.cursor + 1, state.entries.len());
        
        if marked_count > 0 {
            print!(" [{}*]", marked_count);
        }
        
        if yanked_count > 0 {
            let mode = match state.yank_mode {
                YankMode::Copy => "copy",
                YankMode::Move => "move",
            };
            print!(" [{} {}]", yanked_count, mode);
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
    }
    
    match io::stdin().read(&mut buf[..1]) {
        Ok(1) => {
            if buf[0] == 27 { // ESC sequence
                // Try to read more bytes for arrow keys
                if let Ok(2) = io::stdin().read(&mut buf[1..3]) {
                    if buf[1] == b'[' {
                        return Ok(Some(vec![buf[0], buf[1], buf[2]]));
                    }
                }
                Ok(Some(vec![buf[0]]))
            } else {
                Ok(Some(vec![buf[0]]))
            }
        }
        _ => Ok(None),
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
                    state.dir = entry.path.clone();
                    state.cursor = 0;
                    state.view_offset = 0;
                    state.filter = None;
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
            
            state.yank_mode = YankMode::Copy;
            
            if !state.yanked.is_empty() {
                state.message = Some(format!("Yanked {} item(s)", state.yanked.len()));
            }
        }
        
        Action::MoveFiles => {
            state.yanked = state.entries.iter()
                .filter(|e| e.marked)
                .map(|e| e.path.clone())
                .collect();
            
            if state.yanked.is_empty() {
                if let Some(entry) = state.entries.get(state.cursor) {
                    state.yanked.push(entry.path.clone());
                }
            }
            
            state.yank_mode = YankMode::Move;
            
            if !state.yanked.is_empty() {
                state.message = Some(format!("Ready to move {} item(s)", state.yanked.len()));
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
                    
                    let result = match state.yank_mode {
                        YankMode::Copy => copy_recursive(path, &dest),
                        YankMode::Move => move_file(path, &dest),
                    };
                    
                    match result {
                        Ok(_) => success += 1,
                        Err(e) => errors.push(format!("{}: {}", path.display(), e)),
                    }
                }
                
                if errors.is_empty() {
                    state.message = Some(format!("Pasted {} item(s)", success));
                    if matches!(state.yank_mode, YankMode::Move) {
                        state.yanked.clear();
                    }
                } else {
                    state.message = Some(format!("Errors: {}", errors.join(", ")));
                }
                
                load_directory(state)?;
            }
        }
        
        Action::Delete => {
            let to_delete: Vec<_> = state.entries.iter()
                .filter(|e| e.marked)
                .map(|e| e.path.clone())
                .collect();
            
            let to_delete = if to_delete.is_empty() {
                state.entries.get(state.cursor)
                    .map(|e| vec![e.path.clone()])
                    .unwrap_or_default()
            } else {
                to_delete
            };
            
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
            update_terminal_size(state)?;
            load_directory(state)?;
        }
        
        Action::Filter => {
            restore_terminal()?;
            print!("Filter: ");
            io::stdout().flush()?;
            
            let mut filter = String::new();
            io::stdin().read_line(&mut filter)?;
            let filter = filter.trim();
            
            setup_terminal()?;
            
            state.filter = if filter.is_empty() {
                None
            } else {
                Some(filter.to_string())
            };
            
            state.cursor = 0;
            state.view_offset = 0;
            load_directory(state)?;
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
        
        Action::Jump(key) => {
            for &(k, path) in DIR_JUMPS {
                if k == key {
                    let new_dir = expand_tilde(path);
                    if new_dir.exists() && new_dir.is_dir() {
                        state.dir = new_dir;
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