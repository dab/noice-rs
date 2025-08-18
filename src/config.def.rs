pub const SHOW_HIDDEN: bool = false;
pub const DIRS_FIRST: bool = true;
pub const USE_COLOR: bool = true;
pub const SHOW_SIZE: bool = false;
pub const TILDE_HOME: bool = false;
pub const VERSION_SORT: bool = true;
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
    UnYank,
    Paste,
    Delete,
    ToggleDirsFirst,
    ToggleHidden,
    ToggleSize,
    ToggleVersionSort,
    Mark,
    UnmarkAll,
    Home,
    End,
    PageUp,
    PageDown,
    HalfPageUp,
    HalfPageDown,
    Rename,
    MakeDir,
    MakeFile,
    Shell,
    Reload,
    Redraw,
    SortByName,
    SortBySize,
    SortByTime,
    MoveFiles,
    Link,
    ChangeDir,
    EditFile,
    MediaPlayer,
    TopMonitor,
    ShowHelp,
    Jump(u8),
    PendingKey(u8),
}

pub const KEYBINDS: &[(u8, Action)] = &[
    (b'q', Action::Quit),
    (b'j', Action::Next),
    (b'k', Action::Previous),
    (b'l', Action::Enter),
    (b'h', Action::Back),
    (b'/', Action::Filter),
    (b'y', Action::Yank),
    (b'u', Action::UnYank),
    (b'p', Action::Paste),
    (b'D', Action::PendingKey(b'D')), // DD for delete
    (b'd', Action::ToggleDirsFirst),
    (b'.', Action::ToggleHidden),
    (b' ', Action::Mark),
    (b'g', Action::PendingKey(b'g')), // gg or g<key> for jumps
    (b'\'', Action::PendingKey(b'\'')), // '<key> for jumps
    (b'G', Action::End),
    (b'n', Action::PendingKey(b'n')), // nf or nd
    (b'r', Action::Rename),
    (b'!', Action::Shell),
    (b'R', Action::Reload),
    (b's', Action::SortByName),
    (b'S', Action::ToggleSize),
    (b't', Action::SortByTime),
    (b'v', Action::ToggleVersionSort),
    (b'm', Action::MoveFiles),
    (b'L', Action::Link),
    (b'c', Action::ChangeDir),
    (b'e', Action::EditFile),
    (b'M', Action::MediaPlayer),
    (b'z', Action::TopMonitor),
    (b'?', Action::ShowHelp),
    (4, Action::HalfPageDown), // Ctrl-D
    (21, Action::HalfPageUp),   // Ctrl-U
    (12, Action::Redraw),       // Ctrl-L
    (8, Action::ToggleHidden),  // Ctrl-H
    (27, Action::Quit),         // ESC
    (10, Action::Enter),        // Enter
    (127, Action::Back),        // Backspace
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
    (b'r', "/"),
    (b'e', "/etc"),
    (b'b', "/bin"),
    (b'u', "/usr"),
    (b'm', "/media"),
    (b'.', "~/.config"),
    (b'\'', ""), // Special case for lastdir - handled separately
];

pub const COLOR_DIR: &str = "\x1b[34m";      // Blue
pub const COLOR_FILE: &str = "\x1b[0m";       // Default
pub const COLOR_EXEC: &str = "\x1b[32m";      // Green
pub const COLOR_LINK: &str = "\x1b[36m";      // Cyan
pub const COLOR_MARKED: &str = "\x1b[33m";    // Yellow
pub const COLOR_RESET: &str = "\x1b[0m";
pub const COLOR_ERROR: &str = "\x1b[31m";     // Red

pub const DEFAULT_PAGER: &str = "less";
pub const DEFAULT_SHELL: &str = "sh";
pub const DEFAULT_EDITOR: &str = "vi";
pub const DEFAULT_TOP: &str = "top";
pub const DEFAULT_MEDIA_PLAYER: &str = "mpv --shuffle";
pub const DEFAULT_MAN_COMMAND: &str = "man noice";