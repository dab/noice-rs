pub const SHOW_HIDDEN: bool = false;
pub const DIRS_FIRST: bool = true;
pub const USE_COLOR: bool = true;
pub const CURSOR: &str = " > ";
pub const NO_CURSOR: &str = "   ";
pub const YANK_SYMBOL: &str = "* ";

#[derive(Copy, Clone, Debug)]
pub enum Action {
    Quit,
    Next,
    Previous,
    Enter,
    Back,
    Filter,
    Yank,
    Paste,
    Delete,
    ToggleDirsFirst,
    ToggleHidden,
    Mark,
    UnmarkAll,
    Home,
    End,
    PageUp,
    PageDown,
    Rename,
    MakeDir,
    Shell,
    Reload,
    SortByName,
    SortBySize,
    SortByTime,
    MoveFiles,
    Link,
    Jump(u8),
}

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
    (b' ', Action::Mark),
    (b'u', Action::UnmarkAll),
    (b'g', Action::Home),
    (b'G', Action::End),
    (b'r', Action::Rename),
    (b'n', Action::MakeDir),
    (b'!', Action::Shell),
    (b'R', Action::Reload),
    (b's', Action::SortByName),
    (b'S', Action::SortBySize),
    (b't', Action::SortByTime),
    (b'm', Action::MoveFiles),
    (b'L', Action::Link),
    (27, Action::Quit), // ESC
    (10, Action::Enter), // Enter
    (127, Action::Back), // Backspace
];

pub const FILE_RULES: &[(&str, &str)] = &[
    (r"\.mp4$", "mpv"),
    (r"\.mkv$", "mpv"),
    (r"\.avi$", "mpv"),
    (r"\.mov$", "mpv"),
    (r"\.webm$", "mpv"),
    (r"\.pdf$", "open"),
    (r"\.txt$", "vim"),
    (r"\.rs$", "vim"),
    (r"\.c$", "vim"),
    (r"\.h$", "vim"),
    (r"\.cpp$", "vim"),
    (r"\.py$", "vim"),
    (r"\.js$", "vim"),
    (r"\.json$", "vim"),
    (r"\.toml$", "vim"),
    (r"\.yaml$", "vim"),
    (r"\.yml$", "vim"),
    (r"\.md$", "vim"),
    (r"\.sh$", "vim"),
    (r"\.jpg$", "open"),
    (r"\.jpeg$", "open"),
    (r"\.png$", "open"),
    (r"\.gif$", "open"),
    (r"\.svg$", "open"),
    (r"\.html$", "open"),
    (r"\.htm$", "open"),
];

pub const DIR_JUMPS: &[(u8, &str)] = &[
    (b'/', "/"),
    (b'~', "~"),
    (b'c', "~/.config"),
    (b'd', "~/Downloads"),
    (b'D', "~/Documents"),
    (b't', "/tmp"),
    (b'e', "/etc"),
    (b'u', "/usr"),
    (b'v', "/var"),
];

pub const COLOR_DIR: &str = "\x1b[34m";      // Blue
pub const COLOR_FILE: &str = "\x1b[0m";       // Default
pub const COLOR_EXEC: &str = "\x1b[32m";      // Green
pub const COLOR_LINK: &str = "\x1b[36m";      // Cyan
pub const COLOR_MARKED: &str = "\x1b[33m";    // Yellow
pub const COLOR_RESET: &str = "\x1b[0m";
pub const COLOR_ERROR: &str = "\x1b[31m";     // Red

pub const DEFAULT_EDITOR: &str = "vim";
pub const DEFAULT_PAGER: &str = "less";
pub const DEFAULT_SHELL: &str = "sh";