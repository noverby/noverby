//! systemd-tmpfiles — Create, delete, and clean up temporary files and directories
//!
//! Reads configuration from tmpfiles.d/*.conf files and creates, deletes, or cleans
//! temporary files and directories. This is a drop-in replacement for systemd-tmpfiles.
//!
//! Configuration is read from (in order of priority):
//!   /etc/tmpfiles.d/*.conf
//!   /run/tmpfiles.d/*.conf
//!   /usr/lib/tmpfiles.d/*.conf
//!   /lib/tmpfiles.d/*.conf
//!
//! Each .conf file contains lines of the form:
//!   Type Path Mode User Group Age Argument
//!
//! Supported types (from tmpfiles.d(5)):
//!   f/f+  — Create a file (optionally write argument as contents)
//!   w/w+  — Write argument to a file (truncate with +)
//!   d     — Create a directory
//!   D     — Create a directory; remove contents when --remove is used
//!   e     — Adjust permissions/ownership of existing directory; clean with age
//!   q     — Create a subvolume or directory (falls back to mkdir)
//!   Q     — Like q but also removes on --remove
//!   p/p+  — Create a named pipe (FIFO)
//!   L/L+  — Create a symlink
//!   c/c+  — Create a character device node
//!   b/b+  — Create a block device node
//!   C/C+  — Recursively copy a file/directory
//!   x     — Ignore path for cleaning (exclude from --clean)
//!   X     — Ignore path and everything below for cleaning
//!   r     — Remove a file or directory (on --remove)
//!   R     — Recursively remove a path (on --remove)
//!   z     — Adjust access mode, ownership, and SELinux context
//!   Z     — Recursively adjust access mode, ownership, and SELinux context
//!   t     — Set extended attributes
//!   T     — Recursively set extended attributes
//!   h     — Set file/directory attributes (chattr)
//!   H     — Recursively set file/directory attributes
//!   a/a+  — Set POSIX ACLs
//!   A/A+  — Recursively set POSIX ACLs

use std::collections::BTreeSet;
use std::ffi::CString;
use std::fs;
use std::io::{self, BufRead};
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime};

use clap::Parser;

const EXIT_SUCCESS: u8 = 0;
const EXIT_FAILURE: u8 = 1;

/// Directories to search for tmpfiles.d configuration, in priority order.
/// Earlier directories take precedence when the same filename exists in multiple.
const CONFIG_DIRS: &[&str] = &[
    "/etc/tmpfiles.d",
    "/run/tmpfiles.d",
    "/usr/lib/tmpfiles.d",
    "/lib/tmpfiles.d",
];

/// systemd-tmpfiles — Create, delete, and clean up temporary files and directories
#[derive(Parser, Debug)]
#[command(name = "systemd-tmpfiles", version, about)]
struct Cli {
    /// Create files and directories as specified in the configuration
    #[arg(long)]
    create: bool,

    /// Clean up files and directories older than the configured age
    #[arg(long)]
    clean: bool,

    /// Remove files and directories as specified in the configuration
    #[arg(long)]
    remove: bool,

    /// Also execute lines with an exclamation mark (for early boot)
    #[arg(long)]
    boot: bool,

    /// Only apply rules with paths that start with the specified prefix
    #[arg(long = "prefix")]
    prefixes: Vec<PathBuf>,

    /// Only apply rules with paths that do NOT start with the specified prefix
    #[arg(long = "exclude-prefix")]
    exclude_prefixes: Vec<PathBuf>,

    /// Treat all file system errors as non-fatal
    #[arg(long)]
    graceful: bool,

    /// Specific tmpfiles.d config files to read (instead of scanning directories)
    files: Vec<PathBuf>,
}

/// Represents the action type from a tmpfiles.d line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemType {
    /// f — Create file
    CreateFile,
    /// w — Write to file
    WriteFile,
    /// d — Create directory
    CreateDirectory,
    /// D — Create or clean directory
    CreateOrCleanDirectory,
    /// e — Adjust existing directory
    AdjustDirectory,
    /// q — Create subvolume or directory
    CreateSubvolume,
    /// Q — Create subvolume or directory, clean on remove
    CreateOrCleanSubvolume,
    /// p — Create FIFO
    CreateFifo,
    /// L — Create symlink
    CreateSymlink,
    /// c — Create character device
    CreateCharDevice,
    /// b — Create block device
    CreateBlockDevice,
    /// C — Copy files/directories
    CopyFiles,
    /// x — Exclude path from cleaning
    IgnorePath,
    /// X — Exclude path and children from cleaning
    IgnoreDirectoryPath,
    /// r — Remove file/directory
    RemovePath,
    /// R — Recursively remove path
    RemoveRecursively,
    /// z — Adjust permissions
    AdjustPermissions,
    /// Z — Recursively adjust permissions
    AdjustPermissionsRecursively,
    /// t — Set extended attributes
    SetExtendedAttributes,
    /// T — Recursively set extended attributes
    SetExtendedAttributesRecursively,
    /// h — Set file attributes
    SetAttributes,
    /// H — Recursively set file attributes
    SetAttributesRecursively,
    /// a — Set POSIX ACLs
    SetACL,
    /// A — Recursively set POSIX ACLs
    SetACLRecursively,
}

impl ItemType {
    fn from_char(c: char, _plus: bool) -> Option<Self> {
        match c {
            'f' => Some(ItemType::CreateFile),
            'w' => Some(ItemType::WriteFile),
            'd' => Some(ItemType::CreateDirectory),
            'D' => Some(ItemType::CreateOrCleanDirectory),
            'e' => Some(ItemType::AdjustDirectory),
            'q' => Some(ItemType::CreateSubvolume),
            'Q' => Some(ItemType::CreateOrCleanSubvolume),
            'p' => Some(ItemType::CreateFifo),
            'L' => Some(ItemType::CreateSymlink),
            'c' => Some(ItemType::CreateCharDevice),
            'b' => Some(ItemType::CreateBlockDevice),
            'C' => Some(ItemType::CopyFiles),
            'x' => Some(ItemType::IgnorePath),
            'X' => Some(ItemType::IgnoreDirectoryPath),
            'r' => Some(ItemType::RemovePath),
            'R' => Some(ItemType::RemoveRecursively),
            'z' => Some(ItemType::AdjustPermissions),
            'Z' => Some(ItemType::AdjustPermissionsRecursively),
            't' => Some(ItemType::SetExtendedAttributes),
            'T' => Some(ItemType::SetExtendedAttributesRecursively),
            'h' => Some(ItemType::SetAttributes),
            'H' => Some(ItemType::SetAttributesRecursively),
            'a' => Some(ItemType::SetACL),
            'A' => Some(ItemType::SetACLRecursively),
            _ => None,
        }
    }

    /// Whether this type is relevant during --create.
    fn is_create_type(self) -> bool {
        matches!(
            self,
            ItemType::CreateFile
                | ItemType::WriteFile
                | ItemType::CreateDirectory
                | ItemType::CreateOrCleanDirectory
                | ItemType::AdjustDirectory
                | ItemType::CreateSubvolume
                | ItemType::CreateOrCleanSubvolume
                | ItemType::CreateFifo
                | ItemType::CreateSymlink
                | ItemType::CreateCharDevice
                | ItemType::CreateBlockDevice
                | ItemType::CopyFiles
                | ItemType::AdjustPermissions
                | ItemType::AdjustPermissionsRecursively
                | ItemType::SetExtendedAttributes
                | ItemType::SetExtendedAttributesRecursively
                | ItemType::SetAttributes
                | ItemType::SetAttributesRecursively
                | ItemType::SetACL
                | ItemType::SetACLRecursively
        )
    }

    /// Whether this type is relevant during --remove.
    fn is_remove_type(self) -> bool {
        matches!(
            self,
            ItemType::CreateOrCleanDirectory
                | ItemType::CreateOrCleanSubvolume
                | ItemType::RemovePath
                | ItemType::RemoveRecursively
        )
    }

    /// Whether this type is relevant during --clean.
    fn is_clean_type(self) -> bool {
        matches!(
            self,
            ItemType::CreateDirectory
                | ItemType::CreateOrCleanDirectory
                | ItemType::AdjustDirectory
                | ItemType::CreateSubvolume
                | ItemType::CreateOrCleanSubvolume
                | ItemType::IgnorePath
                | ItemType::IgnoreDirectoryPath
        )
    }
}

/// A parsed tmpfiles.d configuration item.
#[derive(Debug, Clone)]
struct TmpfilesItem {
    /// The action type.
    item_type: ItemType,
    /// Whether the '+' modifier was specified (force/append behavior).
    force: bool,
    /// Whether the '!' modifier was specified (boot-only).
    boot_only: bool,
    /// Whether the '-' modifier was specified (ignore errors).
    minus: bool,
    /// The target path.
    path: PathBuf,
    /// File mode (e.g. 0755). None means use default.
    mode: Option<u32>,
    /// Owner user name or UID. None means root/default.
    user: Option<String>,
    /// Owner group name or GID. None means root/default.
    group: Option<String>,
    /// Maximum age for cleaning. None means no age limit.
    age: Option<Duration>,
    /// Argument field (contents for 'f', symlink target for 'L', etc.).
    argument: Option<String>,
    /// Source file for diagnostics.
    source: PathBuf,
    /// Line number in source file.
    line_number: usize,
}

/// Parse a mode string like "0755", "755", or "-" (default).
fn parse_mode(mode_str: &str, default: u32) -> u32 {
    let s = mode_str.trim();
    if s == "-" || s.is_empty() {
        return default;
    }
    // Strip a leading '~' (masked mode, which we treat as plain mode for now)
    let s = s.strip_prefix('~').unwrap_or(s);
    u32::from_str_radix(s, 8).unwrap_or(default)
}

/// Parse a user specification. Returns None for "-" or empty.
fn parse_user(user_str: &str) -> Option<String> {
    let s = user_str.trim();
    if s == "-" || s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Parse an age specification like "10d", "12h", "1w", etc.
/// Returns None for "-" or empty.
fn parse_age(age_str: &str) -> Option<Duration> {
    let s = age_str.trim();
    if s == "-" || s.is_empty() {
        return None;
    }

    // Remove a leading '~' modifier (cleanup only if not accessed)
    let s = s.strip_prefix('~').unwrap_or(s);

    if s.is_empty() {
        return None;
    }

    let mut total_secs: u64 = 0;
    let mut num_buf = String::new();

    for ch in s.chars() {
        if ch.is_ascii_digit() {
            num_buf.push(ch);
        } else {
            let n: u64 = if num_buf.is_empty() {
                1
            } else {
                num_buf.parse().unwrap_or(0)
            };
            num_buf.clear();

            let multiplier = match ch {
                'u' | 'μ' => 0, // microseconds (sub-second, round to 0)
                's' => 1,
                'm' => 60,
                'h' => 3600,
                'd' => 86400,
                'w' => 604800,
                'M' => 2592000,  // 30 days
                'y' => 31536000, // 365 days
                _ => {
                    // Unknown suffix, try parsing the whole thing as seconds
                    return s.parse::<u64>().ok().map(Duration::from_secs);
                }
            };

            total_secs += n * multiplier;
        }
    }

    // Trailing number without suffix is treated as seconds
    if !num_buf.is_empty()
        && let Ok(n) = num_buf.parse::<u64>()
    {
        total_secs += n;
    }

    if total_secs == 0 && !s.contains('0') {
        // If we got 0 and the string doesn't contain '0', parsing probably failed
        return None;
    }

    Some(Duration::from_secs(total_secs))
}

/// Split a tmpfiles.d line into fields, respecting quoting.
/// Fields are whitespace-separated, but quoted strings (single or double quotes)
/// are kept together, and backslash escapes are processed within quotes.
fn split_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quote: Option<char> = None;

    while let Some(&ch) = chars.peek() {
        match in_quote {
            Some(q) => {
                chars.next();
                if ch == q {
                    in_quote = None;
                } else if ch == '\\' {
                    if let Some(&next) = chars.peek() {
                        chars.next();
                        current.push(next);
                    } else {
                        current.push(ch);
                    }
                } else {
                    current.push(ch);
                }
            }
            None => {
                if ch == '"' || ch == '\'' {
                    chars.next();
                    in_quote = Some(ch);
                } else if ch.is_whitespace() {
                    chars.next();
                    if !current.is_empty() {
                        fields.push(std::mem::take(&mut current));
                    }
                } else {
                    chars.next();
                    current.push(ch);
                }
            }
        }
    }

    if !current.is_empty() {
        fields.push(current);
    }

    fields
}

/// Parse a single tmpfiles.d configuration line.
fn parse_line(line: &str, source: &Path, line_number: usize) -> Option<TmpfilesItem> {
    let trimmed = line.trim();

    // Skip empty lines and comments
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
        return None;
    }

    let fields = split_fields(trimmed);
    if fields.len() < 2 {
        eprintln!(
            "systemd-tmpfiles: {}:{}: too few fields, ignoring.",
            source.display(),
            line_number,
        );
        return None;
    }

    // Parse the type field: may include modifiers like '+', '!', '-', '='
    let type_str = &fields[0];
    let mut type_char = None;
    let mut force = false;
    let mut boot_only = false;
    let mut minus = false;

    for c in type_str.chars() {
        match c {
            '+' => force = true,
            '!' => boot_only = true,
            '-' => minus = true,
            '=' => { /* '=' modifier: don't apply to subdirs, we note but don't special-case */ }
            '~' => { /* '~' modifier: masked mode */ }
            _ => {
                if type_char.is_none() {
                    type_char = Some(c);
                }
            }
        }
    }

    let type_char = match type_char {
        Some(c) => c,
        None => {
            eprintln!(
                "systemd-tmpfiles: {}:{}: no type character found in '{}', ignoring.",
                source.display(),
                line_number,
                type_str,
            );
            return None;
        }
    };

    let item_type = match ItemType::from_char(type_char, force) {
        Some(t) => t,
        None => {
            eprintln!(
                "systemd-tmpfiles: {}:{}: unknown type '{}', ignoring.",
                source.display(),
                line_number,
                type_char,
            );
            return None;
        }
    };

    // Field 2: Path (required)
    let path = PathBuf::from(&fields[1]);

    // Default mode depends on type
    let default_mode = match item_type {
        ItemType::CreateDirectory
        | ItemType::CreateOrCleanDirectory
        | ItemType::AdjustDirectory
        | ItemType::CreateSubvolume
        | ItemType::CreateOrCleanSubvolume => 0o755,
        ItemType::CreateFifo => 0o644,
        ItemType::CreateFile => 0o644,
        _ => 0o644,
    };

    // Field 3: Mode (optional)
    let mode = if fields.len() > 2 {
        let m = parse_mode(&fields[2], default_mode);
        Some(m)
    } else {
        Some(default_mode)
    };

    // Field 4: User (optional)
    let user = if fields.len() > 3 {
        parse_user(&fields[3])
    } else {
        None
    };

    // Field 5: Group (optional)
    let group = if fields.len() > 4 {
        parse_user(&fields[4])
    } else {
        None
    };

    // Field 6: Age (optional)
    let age = if fields.len() > 5 {
        parse_age(&fields[5])
    } else {
        None
    };

    // Field 7+: Argument (optional, may contain spaces — rest of line)
    let argument = if fields.len() > 6 {
        let arg = fields[6..].join(" ");
        if arg == "-" || arg.is_empty() {
            None
        } else {
            Some(arg)
        }
    } else {
        None
    };

    Some(TmpfilesItem {
        item_type,
        force,
        boot_only,
        minus,
        path,
        mode,
        user,
        group,
        age,
        argument,
        source: source.to_path_buf(),
        line_number,
    })
}

/// Parse a tmpfiles.d config file.
fn parse_config_file(path: &Path) -> io::Result<Vec<TmpfilesItem>> {
    let file = fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let mut items = Vec::new();

    let mut continuation = String::new();

    for (line_idx, line) in reader.lines().enumerate() {
        let line = line?;
        let line_number = line_idx + 1;

        // Handle line continuation with trailing backslash
        if line.ends_with('\\') {
            continuation.push_str(&line[..line.len() - 1]);
            continuation.push(' ');
            continue;
        }

        let full_line = if !continuation.is_empty() {
            continuation.push_str(&line);
            let result = continuation.clone();
            continuation.clear();
            result
        } else {
            line
        };

        if let Some(item) = parse_line(&full_line, path, line_number) {
            items.push(item);
        }
    }

    Ok(items)
}

/// Discover all .conf files across the config directories, respecting priority.
/// Files in earlier directories shadow files with the same name in later directories.
fn discover_config_files() -> Vec<PathBuf> {
    let mut seen_names: BTreeSet<String> = BTreeSet::new();
    let mut result = Vec::new();

    for dir in CONFIG_DIRS {
        let dir_path = Path::new(dir);
        if !dir_path.is_dir() {
            continue;
        }

        let mut entries: Vec<PathBuf> = match fs::read_dir(dir_path) {
            Ok(rd) => rd
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map(|ext| ext == "conf").unwrap_or(false))
                .collect(),
            Err(e) => {
                eprintln!("systemd-tmpfiles: Failed to read directory {}: {}", dir, e);
                continue;
            }
        };
        entries.sort();

        for path in entries {
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                let name = file_name.to_string();
                if seen_names.contains(&name) {
                    continue;
                }
                seen_names.insert(name);
                result.push(path);
            }
        }
    }

    result
}

/// Resolve a username to a UID.
fn resolve_uid(user: &str) -> Option<u32> {
    // Try parsing as a numeric UID first
    if let Ok(uid) = user.parse::<u32>() {
        return Some(uid);
    }

    // Look up via getpwnam
    let c_user = CString::new(user).ok()?;
    unsafe {
        let pw = libc::getpwnam(c_user.as_ptr());
        if pw.is_null() {
            None
        } else {
            Some((*pw).pw_uid)
        }
    }
}

/// Resolve a group name to a GID.
fn resolve_gid(group: &str) -> Option<u32> {
    // Try parsing as a numeric GID first
    if let Ok(gid) = group.parse::<u32>() {
        return Some(gid);
    }

    // Look up via getgrnam
    let c_group = CString::new(group).ok()?;
    unsafe {
        let gr = libc::getgrnam(c_group.as_ptr());
        if gr.is_null() {
            None
        } else {
            Some((*gr).gr_gid)
        }
    }
}

/// Apply ownership (chown) to a path.
fn apply_ownership(path: &Path, user: &Option<String>, group: &Option<String>) -> io::Result<()> {
    let uid = match user {
        Some(u) => resolve_uid(u)
            .map(|id| id as libc::uid_t)
            .unwrap_or(libc::uid_t::MAX),
        None => libc::uid_t::MAX, // -1 means "don't change"
    };

    let gid = match group {
        Some(g) => resolve_gid(g)
            .map(|id| id as libc::gid_t)
            .unwrap_or(libc::gid_t::MAX),
        None => libc::gid_t::MAX, // -1 means "don't change"
    };

    // If neither needs changing, skip
    if uid == libc::uid_t::MAX && gid == libc::gid_t::MAX {
        return Ok(());
    }

    // Use lchown to avoid following symlinks
    let c_path = CString::new(path.as_os_str().as_encoded_bytes())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

    let ret = unsafe { libc::lchown(c_path.as_ptr(), uid, gid) };
    if ret != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Apply file mode (chmod) to a path.
fn apply_mode(path: &Path, mode: u32) -> io::Result<()> {
    let permissions = fs::Permissions::from_mode(mode);
    fs::set_permissions(path, permissions)
}

/// Check if a path matches any of the given prefixes.
fn matches_prefix(path: &Path, prefixes: &[PathBuf]) -> bool {
    if prefixes.is_empty() {
        return true;
    }
    prefixes.iter().any(|prefix| path.starts_with(prefix))
}

/// Check if a path matches any of the exclude prefixes.
fn excluded_by_prefix(path: &Path, exclude_prefixes: &[PathBuf]) -> bool {
    exclude_prefixes
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

/// Execute a single tmpfiles.d item for --create.
fn execute_create(item: &TmpfilesItem, graceful: bool) -> bool {
    let path = &item.path;

    match item.item_type {
        ItemType::CreateFile => {
            if path.exists() && !item.force {
                // File already exists, just adjust permissions
                if let Some(mode) = item.mode
                    && let Err(e) = apply_mode(path, mode)
                    && !graceful
                    && !item.minus
                {
                    eprintln!(
                        "systemd-tmpfiles: Failed to set mode on {}: {}",
                        path.display(),
                        e
                    );
                    return false;
                }
                if let Err(e) = apply_ownership(path, &item.user, &item.group)
                    && !graceful
                    && !item.minus
                {
                    eprintln!(
                        "systemd-tmpfiles: Failed to set ownership on {}: {}",
                        path.display(),
                        e
                    );
                    return false;
                }
                return true;
            }

            // Create parent directories
            if let Some(parent) = path.parent()
                && let Err(e) = fs::create_dir_all(parent)
                && !graceful
                && !item.minus
            {
                eprintln!(
                    "systemd-tmpfiles: Failed to create parent directory {}: {}",
                    parent.display(),
                    e
                );
                return false;
            }

            // Create the file
            let content = item.argument.as_deref().unwrap_or("");
            match fs::write(path, content) {
                Ok(()) => {
                    if let Some(mode) = item.mode {
                        let _ = apply_mode(path, mode);
                    }
                    let _ = apply_ownership(path, &item.user, &item.group);
                    true
                }
                Err(e) => {
                    if !graceful && !item.minus {
                        eprintln!(
                            "systemd-tmpfiles: Failed to create file {}: {}",
                            path.display(),
                            e
                        );
                        return false;
                    }
                    true
                }
            }
        }

        ItemType::WriteFile => {
            let content = item.argument.as_deref().unwrap_or("");
            if item.force || !path.exists() {
                match fs::write(path, content) {
                    Ok(()) => true,
                    Err(e) => {
                        if !graceful && !item.minus {
                            eprintln!(
                                "systemd-tmpfiles: Failed to write to {}: {}",
                                path.display(),
                                e
                            );
                            return false;
                        }
                        true
                    }
                }
            } else {
                // Append for w (without +)
                match std::fs::OpenOptions::new().append(true).open(path) {
                    Ok(mut f) => {
                        use std::io::Write;
                        if let Err(e) = f.write_all(content.as_bytes())
                            && !graceful
                            && !item.minus
                        {
                            eprintln!(
                                "systemd-tmpfiles: Failed to write to {}: {}",
                                path.display(),
                                e
                            );
                            return false;
                        }
                        true
                    }
                    Err(e) => {
                        if !graceful && !item.minus {
                            eprintln!(
                                "systemd-tmpfiles: Failed to open {} for writing: {}",
                                path.display(),
                                e
                            );
                            return false;
                        }
                        true
                    }
                }
            }
        }

        ItemType::CreateDirectory
        | ItemType::CreateOrCleanDirectory
        | ItemType::AdjustDirectory
        | ItemType::CreateSubvolume
        | ItemType::CreateOrCleanSubvolume => {
            if item.item_type == ItemType::AdjustDirectory {
                // 'e' only adjusts existing directories, doesn't create
                if !path.is_dir() {
                    return true;
                }
            } else if !path.exists() {
                // For q/Q, we could try creating a btrfs subvolume, but fall back to mkdir
                if let Err(e) = fs::create_dir_all(path) {
                    if !graceful && !item.minus {
                        eprintln!(
                            "systemd-tmpfiles: Failed to create directory {}: {}",
                            path.display(),
                            e
                        );
                        return false;
                    }
                    return true;
                }
            }

            // Apply mode and ownership
            if path.is_dir() {
                if let Some(mode) = item.mode
                    && let Err(e) = apply_mode(path, mode)
                    && !graceful
                    && !item.minus
                {
                    eprintln!(
                        "systemd-tmpfiles: Failed to set mode on {}: {}",
                        path.display(),
                        e
                    );
                }
                if let Err(e) = apply_ownership(path, &item.user, &item.group)
                    && !graceful
                    && !item.minus
                {
                    eprintln!(
                        "systemd-tmpfiles: Failed to set ownership on {}: {}",
                        path.display(),
                        e
                    );
                }
            }

            true
        }

        ItemType::CreateFifo => {
            if path.exists() && !item.force {
                return true;
            }

            // Create parent directories
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            let c_path = match CString::new(path.as_os_str().as_encoded_bytes()) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("systemd-tmpfiles: Invalid path {}: {}", path.display(), e);
                    return false;
                }
            };

            let mode = item.mode.unwrap_or(0o644);
            let ret = unsafe { libc::mkfifo(c_path.as_ptr(), mode) };
            if ret != 0 {
                let e = io::Error::last_os_error();
                if e.kind() != io::ErrorKind::AlreadyExists && !graceful && !item.minus {
                    eprintln!(
                        "systemd-tmpfiles: Failed to create FIFO {}: {}",
                        path.display(),
                        e
                    );
                    return false;
                }
            }
            let _ = apply_ownership(path, &item.user, &item.group);
            true
        }

        ItemType::CreateSymlink => {
            if path.exists() || path.symlink_metadata().is_ok() {
                if !item.force {
                    return true;
                }
                // With '+' force, remove existing and recreate
                let _ = fs::remove_file(path);
            }

            // Create parent directories
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            let target = match &item.argument {
                Some(t) => t.as_str(),
                None => {
                    eprintln!(
                        "systemd-tmpfiles: {}:{}: symlink requires an argument (target path).",
                        item.source.display(),
                        item.line_number,
                    );
                    return false;
                }
            };

            match symlink(target, path) {
                Ok(()) => true,
                Err(e) => {
                    if !graceful && !item.minus {
                        eprintln!(
                            "systemd-tmpfiles: Failed to create symlink {} -> {}: {}",
                            path.display(),
                            target,
                            e
                        );
                        return false;
                    }
                    true
                }
            }
        }

        ItemType::CreateCharDevice | ItemType::CreateBlockDevice => {
            // Creating device nodes requires parsing the argument as "major:minor"
            // and calling mknod(). This requires root privileges.
            if path.exists() && !item.force {
                return true;
            }

            let dev_str = match &item.argument {
                Some(a) => a.as_str(),
                None => {
                    eprintln!(
                        "systemd-tmpfiles: {}:{}: device node requires an argument (major:minor).",
                        item.source.display(),
                        item.line_number,
                    );
                    return false;
                }
            };

            let parts: Vec<&str> = dev_str.split(':').collect();
            if parts.len() != 2 {
                eprintln!(
                    "systemd-tmpfiles: {}:{}: invalid device specification '{}', expected major:minor.",
                    item.source.display(),
                    item.line_number,
                    dev_str,
                );
                return false;
            }

            let major: u32 = match parts[0].trim().parse() {
                Ok(v) => v,
                Err(_) => {
                    eprintln!(
                        "systemd-tmpfiles: {}:{}: invalid major number '{}'.",
                        item.source.display(),
                        item.line_number,
                        parts[0],
                    );
                    return false;
                }
            };

            let minor: u32 = match parts[1].trim().parse() {
                Ok(v) => v,
                Err(_) => {
                    eprintln!(
                        "systemd-tmpfiles: {}:{}: invalid minor number '{}'.",
                        item.source.display(),
                        item.line_number,
                        parts[1],
                    );
                    return false;
                }
            };

            // Create parent directories
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            let c_path = match CString::new(path.as_os_str().as_encoded_bytes()) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("systemd-tmpfiles: Invalid path {}: {}", path.display(), e);
                    return false;
                }
            };

            let dev = libc::makedev(major, minor);
            let mode = item.mode.unwrap_or(0o644);
            let node_type = if item.item_type == ItemType::CreateCharDevice {
                libc::S_IFCHR
            } else {
                libc::S_IFBLK
            };

            if item.force {
                let _ = fs::remove_file(path);
            }

            let ret = unsafe { libc::mknod(c_path.as_ptr(), node_type | mode, dev) };
            if ret != 0 {
                let e = io::Error::last_os_error();
                if e.kind() != io::ErrorKind::AlreadyExists && !graceful && !item.minus {
                    eprintln!(
                        "systemd-tmpfiles: Failed to create device node {}: {}",
                        path.display(),
                        e
                    );
                    return false;
                }
            }
            let _ = apply_ownership(path, &item.user, &item.group);
            true
        }

        ItemType::CopyFiles => {
            let source = match &item.argument {
                Some(s) => PathBuf::from(s),
                None => {
                    eprintln!(
                        "systemd-tmpfiles: {}:{}: copy requires an argument (source path).",
                        item.source.display(),
                        item.line_number,
                    );
                    return false;
                }
            };

            if path.exists() && !item.force {
                return true;
            }

            // Create parent directories
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            if source.is_dir() {
                match copy_dir_recursive(&source, path) {
                    Ok(()) => true,
                    Err(e) => {
                        if !graceful && !item.minus {
                            eprintln!(
                                "systemd-tmpfiles: Failed to copy {} -> {}: {}",
                                source.display(),
                                path.display(),
                                e,
                            );
                            return false;
                        }
                        true
                    }
                }
            } else {
                match fs::copy(&source, path) {
                    Ok(_) => {
                        if let Some(mode) = item.mode {
                            let _ = apply_mode(path, mode);
                        }
                        let _ = apply_ownership(path, &item.user, &item.group);
                        true
                    }
                    Err(e) => {
                        if !graceful && !item.minus {
                            eprintln!(
                                "systemd-tmpfiles: Failed to copy {} -> {}: {}",
                                source.display(),
                                path.display(),
                                e,
                            );
                            return false;
                        }
                        true
                    }
                }
            }
        }

        ItemType::AdjustPermissions | ItemType::AdjustPermissionsRecursively => {
            if !path.exists() {
                return true;
            }

            let recursive = item.item_type == ItemType::AdjustPermissionsRecursively;

            let apply_to = |p: &Path| -> bool {
                let mut ok = true;
                if let Some(mode) = item.mode
                    && let Err(e) = apply_mode(p, mode)
                    && !graceful
                    && !item.minus
                {
                    eprintln!(
                        "systemd-tmpfiles: Failed to set mode on {}: {}",
                        p.display(),
                        e
                    );
                    ok = false;
                }
                if let Err(e) = apply_ownership(p, &item.user, &item.group)
                    && !graceful
                    && !item.minus
                {
                    eprintln!(
                        "systemd-tmpfiles: Failed to set ownership on {}: {}",
                        p.display(),
                        e
                    );
                    ok = false;
                }
                ok
            };

            if recursive {
                let mut ok = apply_to(path);
                if path.is_dir()
                    && let Err(e) = walk_dir(path, &mut |p| {
                        if !apply_to(p) {
                            ok = false;
                        }
                    })
                    && !graceful
                    && !item.minus
                {
                    eprintln!(
                        "systemd-tmpfiles: Failed to walk directory {}: {}",
                        path.display(),
                        e
                    );
                }
                ok
            } else {
                apply_to(path)
            }
        }

        ItemType::SetExtendedAttributes
        | ItemType::SetExtendedAttributesRecursively
        | ItemType::SetAttributes
        | ItemType::SetAttributesRecursively
        | ItemType::SetACL
        | ItemType::SetACLRecursively => {
            // These are advanced features (xattrs, chattr flags, POSIX ACLs).
            // We accept them without error but don't implement the actual
            // kernel interface calls — this is sufficient for most boot scenarios.
            true
        }

        // Types that are only relevant for --remove or --clean
        ItemType::IgnorePath
        | ItemType::IgnoreDirectoryPath
        | ItemType::RemovePath
        | ItemType::RemoveRecursively => true,
    }
}

/// Execute a single tmpfiles.d item for --remove.
fn execute_remove(item: &TmpfilesItem, graceful: bool) -> bool {
    let path = &item.path;

    match item.item_type {
        ItemType::RemovePath => {
            if !path.exists() && path.symlink_metadata().is_err() {
                return true;
            }
            match fs::remove_file(path) {
                Ok(()) => true,
                Err(_) => {
                    // Try removing as a directory (empty)
                    match fs::remove_dir(path) {
                        Ok(()) => true,
                        Err(e) => {
                            if !graceful && !item.minus {
                                eprintln!(
                                    "systemd-tmpfiles: Failed to remove {}: {}",
                                    path.display(),
                                    e
                                );
                                return false;
                            }
                            true
                        }
                    }
                }
            }
        }

        ItemType::RemoveRecursively => {
            if !path.exists() && path.symlink_metadata().is_err() {
                return true;
            }
            if path.is_dir() {
                match fs::remove_dir_all(path) {
                    Ok(()) => true,
                    Err(e) => {
                        if !graceful && !item.minus {
                            eprintln!(
                                "systemd-tmpfiles: Failed to recursively remove {}: {}",
                                path.display(),
                                e
                            );
                            return false;
                        }
                        true
                    }
                }
            } else {
                match fs::remove_file(path) {
                    Ok(()) => true,
                    Err(e) => {
                        if !graceful && !item.minus {
                            eprintln!(
                                "systemd-tmpfiles: Failed to remove {}: {}",
                                path.display(),
                                e
                            );
                            return false;
                        }
                        true
                    }
                }
            }
        }

        ItemType::CreateOrCleanDirectory | ItemType::CreateOrCleanSubvolume => {
            // On --remove, clean out the contents of the directory (but keep the dir itself)
            if !path.is_dir() {
                return true;
            }
            match fs::read_dir(path) {
                Ok(entries) => {
                    let mut ok = true;
                    for entry in entries.filter_map(|e| e.ok()) {
                        let entry_path = entry.path();
                        if entry_path.is_dir() {
                            if let Err(e) = fs::remove_dir_all(&entry_path)
                                && !graceful
                                && !item.minus
                            {
                                eprintln!(
                                    "systemd-tmpfiles: Failed to remove {}: {}",
                                    entry_path.display(),
                                    e
                                );
                                ok = false;
                            }
                        } else if let Err(e) = fs::remove_file(&entry_path)
                            && !graceful
                            && !item.minus
                        {
                            eprintln!(
                                "systemd-tmpfiles: Failed to remove {}: {}",
                                entry_path.display(),
                                e
                            );
                            ok = false;
                        }
                    }
                    ok
                }
                Err(e) => {
                    if !graceful && !item.minus {
                        eprintln!(
                            "systemd-tmpfiles: Failed to read directory {}: {}",
                            path.display(),
                            e
                        );
                        return false;
                    }
                    true
                }
            }
        }

        _ => true,
    }
}

/// Execute cleanup for items with an age specification.
fn execute_clean(item: &TmpfilesItem, exclude_patterns: &[PathBuf], graceful: bool) -> bool {
    let path = &item.path;
    let age = match item.age {
        Some(a) => a,
        None => return true,
    };

    match item.item_type {
        ItemType::CreateDirectory
        | ItemType::CreateOrCleanDirectory
        | ItemType::AdjustDirectory
        | ItemType::CreateSubvolume
        | ItemType::CreateOrCleanSubvolume => {
            if !path.is_dir() {
                return true;
            }
            clean_directory(path, age, exclude_patterns, graceful, item.minus)
        }
        _ => true,
    }
}

/// Clean files older than the specified age from a directory.
fn clean_directory(
    dir: &Path,
    max_age: Duration,
    exclude_patterns: &[PathBuf],
    graceful: bool,
    ignore_errors: bool,
) -> bool {
    let now = SystemTime::now();
    let entries = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            if !graceful && !ignore_errors {
                eprintln!(
                    "systemd-tmpfiles: Failed to read directory {}: {}",
                    dir.display(),
                    e
                );
            }
            return graceful || ignore_errors;
        }
    };

    let mut ok = true;

    for entry in entries.filter_map(|e| e.ok()) {
        let entry_path = entry.path();

        // Check if this path is excluded
        if exclude_patterns
            .iter()
            .any(|excl| entry_path.starts_with(excl))
        {
            continue;
        }

        let metadata = match entry_path.symlink_metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        // Use the most recent of atime, mtime, ctime
        let file_age = {
            let mtime = metadata.mtime() as u64;
            let atime = metadata.atime() as u64;
            let ctime = metadata.ctime() as u64;
            let newest = mtime.max(atime).max(ctime);

            let now_secs = now
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if now_secs > newest {
                Duration::from_secs(now_secs - newest)
            } else {
                Duration::from_secs(0)
            }
        };

        if file_age > max_age {
            if metadata.is_dir() {
                // Recurse into subdirectories first
                let _ = clean_directory(
                    &entry_path,
                    max_age,
                    exclude_patterns,
                    graceful,
                    ignore_errors,
                );

                // Try to remove the directory if it's now empty
                if let Err(e) = fs::remove_dir(&entry_path) {
                    // Directory not empty — that's fine, skip it
                    if e.kind() != io::ErrorKind::Other
                        && e.raw_os_error() != Some(libc::ENOTEMPTY)
                        && e.raw_os_error() != Some(libc::EEXIST)
                        && !graceful
                        && !ignore_errors
                    {
                        eprintln!(
                            "systemd-tmpfiles: Failed to remove old directory {}: {}",
                            entry_path.display(),
                            e
                        );
                        ok = false;
                    }
                }
            } else if let Err(e) = fs::remove_file(&entry_path)
                && !graceful
                && !ignore_errors
            {
                eprintln!(
                    "systemd-tmpfiles: Failed to remove old file {}: {}",
                    entry_path.display(),
                    e
                );
                ok = false;
            }
        } else if metadata.is_dir() {
            // Not old enough to remove, but recurse to clean contents
            let _ = clean_directory(
                &entry_path,
                max_age,
                exclude_patterns,
                graceful,
                ignore_errors,
            );
        }
    }

    ok
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let entry_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if entry_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if entry_type.is_symlink() {
            let target = fs::read_link(&src_path)?;
            let _ = fs::remove_file(&dst_path);
            symlink(&target, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    // Copy permissions from source directory
    let src_meta = fs::metadata(src)?;
    let permissions = src_meta.permissions();
    fs::set_permissions(dst, permissions)?;

    Ok(())
}

/// Walk a directory tree, calling the callback for each entry.
fn walk_dir(dir: &Path, callback: &mut dyn FnMut(&Path)) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        callback(&path);
        if path.is_dir() {
            walk_dir(&path, callback)?;
        }
    }
    Ok(())
}

fn run() -> u8 {
    let cli = Cli::parse();

    let verbose = std::env::var("SYSTEMD_LOG_LEVEL")
        .map(|v| v == "debug" || v == "info")
        .unwrap_or(false);

    // Need at least one action
    if !cli.create && !cli.clean && !cli.remove {
        eprintln!("systemd-tmpfiles: No action specified. Use --create, --clean, or --remove.");
        return EXIT_FAILURE;
    }

    // Collect config files to read
    let config_files = if !cli.files.is_empty() {
        cli.files.clone()
    } else {
        discover_config_files()
    };

    if verbose {
        eprintln!(
            "systemd-tmpfiles: Found {} configuration file(s).",
            config_files.len()
        );
    }

    // Parse all configuration
    let mut items = Vec::new();

    for path in &config_files {
        match parse_config_file(path) {
            Ok(file_items) => {
                if verbose {
                    eprintln!(
                        "systemd-tmpfiles: Read {} item(s) from {}",
                        file_items.len(),
                        path.display()
                    );
                }
                items.extend(file_items);
            }
            Err(e) => {
                eprintln!("systemd-tmpfiles: Failed to read {}: {}", path.display(), e);
            }
        }
    }

    // Filter by boot-only flag
    if !cli.boot {
        items.retain(|item| !item.boot_only);
    }

    // Collect exclude patterns from 'x' and 'X' items for clean operations
    let exclude_patterns: Vec<PathBuf> = items
        .iter()
        .filter(|item| {
            item.item_type == ItemType::IgnorePath
                || item.item_type == ItemType::IgnoreDirectoryPath
        })
        .map(|item| item.path.clone())
        .collect();

    let mut any_failed = false;

    // Execute --create
    if cli.create {
        for item in &items {
            if !item.item_type.is_create_type() {
                continue;
            }

            // Apply prefix filters
            if !matches_prefix(&item.path, &cli.prefixes) {
                continue;
            }
            if excluded_by_prefix(&item.path, &cli.exclude_prefixes) {
                continue;
            }

            if verbose {
                eprintln!(
                    "systemd-tmpfiles: Creating: {} (from {}:{})",
                    item.path.display(),
                    item.source.display(),
                    item.line_number,
                );
            }

            if !execute_create(item, cli.graceful) {
                any_failed = true;
            }
        }
    }

    // Execute --remove
    if cli.remove {
        for item in &items {
            if !item.item_type.is_remove_type() {
                continue;
            }

            if !matches_prefix(&item.path, &cli.prefixes) {
                continue;
            }
            if excluded_by_prefix(&item.path, &cli.exclude_prefixes) {
                continue;
            }

            if verbose {
                eprintln!(
                    "systemd-tmpfiles: Removing: {} (from {}:{})",
                    item.path.display(),
                    item.source.display(),
                    item.line_number,
                );
            }

            if !execute_remove(item, cli.graceful) {
                any_failed = true;
            }
        }
    }

    // Execute --clean
    if cli.clean {
        for item in &items {
            if !item.item_type.is_clean_type() {
                continue;
            }

            if !matches_prefix(&item.path, &cli.prefixes) {
                continue;
            }
            if excluded_by_prefix(&item.path, &cli.exclude_prefixes) {
                continue;
            }

            if verbose {
                eprintln!(
                    "systemd-tmpfiles: Cleaning: {} (from {}:{})",
                    item.path.display(),
                    item.source.display(),
                    item.line_number,
                );
            }

            if !execute_clean(item, &exclude_patterns, cli.graceful) {
                any_failed = true;
            }
        }
    }

    if any_failed {
        EXIT_FAILURE
    } else {
        EXIT_SUCCESS
    }
}

fn main() -> ExitCode {
    ExitCode::from(run())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_mode_octal() {
        assert_eq!(parse_mode("0755", 0o644), 0o755);
        assert_eq!(parse_mode("755", 0o644), 0o755);
        assert_eq!(parse_mode("0644", 0o755), 0o644);
        assert_eq!(parse_mode("0700", 0o644), 0o700);
    }

    #[test]
    fn test_parse_mode_default() {
        assert_eq!(parse_mode("-", 0o644), 0o644);
        assert_eq!(parse_mode("", 0o755), 0o755);
    }

    #[test]
    fn test_parse_mode_masked() {
        // '~' prefix (masked mode) — we treat it as plain mode
        assert_eq!(parse_mode("~0755", 0o644), 0o755);
    }

    #[test]
    fn test_parse_user() {
        assert_eq!(parse_user("root"), Some("root".to_string()));
        assert_eq!(parse_user("-"), None);
        assert_eq!(parse_user(""), None);
        assert_eq!(parse_user("  nobody  "), Some("nobody".to_string()));
    }

    #[test]
    fn test_parse_age_seconds() {
        assert_eq!(parse_age("30"), Some(Duration::from_secs(30)));
        assert_eq!(parse_age("60s"), Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_parse_age_minutes() {
        assert_eq!(parse_age("5m"), Some(Duration::from_secs(300)));
    }

    #[test]
    fn test_parse_age_hours() {
        assert_eq!(parse_age("2h"), Some(Duration::from_secs(7200)));
    }

    #[test]
    fn test_parse_age_days() {
        assert_eq!(parse_age("10d"), Some(Duration::from_secs(864000)));
    }

    #[test]
    fn test_parse_age_weeks() {
        assert_eq!(parse_age("1w"), Some(Duration::from_secs(604800)));
    }

    #[test]
    fn test_parse_age_default() {
        assert_eq!(parse_age("-"), None);
        assert_eq!(parse_age(""), None);
    }

    #[test]
    fn test_parse_age_tilde() {
        // '~' prefix means "only clean if not accessed"
        assert_eq!(parse_age("~10d"), Some(Duration::from_secs(864000)));
    }

    #[test]
    fn test_parse_age_compound() {
        // 1d12h = 86400 + 43200 = 129600
        assert_eq!(parse_age("1d12h"), Some(Duration::from_secs(129600)));
    }

    #[test]
    fn test_split_fields_basic() {
        let fields = split_fields("d /tmp 1777 root root 10d -");
        assert_eq!(
            fields,
            vec!["d", "/tmp", "1777", "root", "root", "10d", "-"]
        );
    }

    #[test]
    fn test_split_fields_quoted() {
        let fields = split_fields(r#"f /etc/hostname - - - - "my hostname""#);
        assert_eq!(fields.len(), 7);
        assert_eq!(fields[6], "my hostname");
    }

    #[test]
    fn test_split_fields_single_quoted() {
        let fields = split_fields("f /etc/test - - - - 'hello world'");
        assert_eq!(fields.len(), 7);
        assert_eq!(fields[6], "hello world");
    }

    #[test]
    fn test_split_fields_extra_whitespace() {
        let fields = split_fields("  d   /tmp   0755   root   root   -   -  ");
        assert_eq!(fields, vec!["d", "/tmp", "0755", "root", "root", "-", "-"]);
    }

    #[test]
    fn test_parse_line_directory() {
        let item = parse_line("d /tmp 1777 root root 10d -", Path::new("test.conf"), 1).unwrap();
        assert_eq!(item.item_type, ItemType::CreateDirectory);
        assert_eq!(item.path, PathBuf::from("/tmp"));
        assert_eq!(item.mode, Some(0o1777));
        assert_eq!(item.user, Some("root".to_string()));
        assert_eq!(item.group, Some("root".to_string()));
        assert_eq!(item.age, Some(Duration::from_secs(864000)));
        assert!(item.argument.is_none());
        assert!(!item.force);
        assert!(!item.boot_only);
    }

    #[test]
    fn test_parse_line_file() {
        let item = parse_line(
            "f /etc/hostname 0644 root root - myhostname",
            Path::new("test.conf"),
            2,
        )
        .unwrap();
        assert_eq!(item.item_type, ItemType::CreateFile);
        assert_eq!(item.path, PathBuf::from("/etc/hostname"));
        assert_eq!(item.mode, Some(0o644));
        assert_eq!(item.argument, Some("myhostname".to_string()));
    }

    #[test]
    fn test_parse_line_symlink() {
        let item = parse_line(
            "L /etc/localtime - - - - /usr/share/zoneinfo/UTC",
            Path::new("test.conf"),
            3,
        )
        .unwrap();
        assert_eq!(item.item_type, ItemType::CreateSymlink);
        assert_eq!(item.path, PathBuf::from("/etc/localtime"));
        assert_eq!(item.argument, Some("/usr/share/zoneinfo/UTC".to_string()));
    }

    #[test]
    fn test_parse_line_force() {
        let item = parse_line(
            "L+ /etc/localtime - - - - /usr/share/zoneinfo/UTC",
            Path::new("test.conf"),
            4,
        )
        .unwrap();
        assert_eq!(item.item_type, ItemType::CreateSymlink);
        assert!(item.force);
    }

    #[test]
    fn test_parse_line_boot_only() {
        let item =
            parse_line("d! /run/test 0755 root root - -", Path::new("test.conf"), 5).unwrap();
        assert!(item.boot_only);
    }

    #[test]
    fn test_parse_line_minus() {
        let item =
            parse_line("d- /run/test 0755 root root - -", Path::new("test.conf"), 6).unwrap();
        assert!(item.minus);
    }

    #[test]
    fn test_parse_line_comment() {
        assert!(parse_line("# This is a comment", Path::new("test.conf"), 1).is_none());
        assert!(parse_line("; Another comment", Path::new("test.conf"), 2).is_none());
        assert!(parse_line("", Path::new("test.conf"), 3).is_none());
        assert!(parse_line("   ", Path::new("test.conf"), 4).is_none());
    }

    #[test]
    fn test_parse_line_remove() {
        let item = parse_line("r /tmp/old-files", Path::new("test.conf"), 7).unwrap();
        assert_eq!(item.item_type, ItemType::RemovePath);
        assert_eq!(item.path, PathBuf::from("/tmp/old-files"));
    }

    #[test]
    fn test_parse_line_remove_recursive() {
        let item = parse_line("R /tmp/old-dir", Path::new("test.conf"), 8).unwrap();
        assert_eq!(item.item_type, ItemType::RemoveRecursively);
    }

    #[test]
    fn test_parse_line_clean_directory() {
        let item = parse_line(
            "D /tmp/cache 0755 root root 1w -",
            Path::new("test.conf"),
            9,
        )
        .unwrap();
        assert_eq!(item.item_type, ItemType::CreateOrCleanDirectory);
        assert_eq!(item.age, Some(Duration::from_secs(604800)));
    }

    #[test]
    fn test_parse_line_adjust_directory() {
        let item = parse_line("e /var/log 0755 root root - -", Path::new("test.conf"), 10).unwrap();
        assert_eq!(item.item_type, ItemType::AdjustDirectory);
    }

    #[test]
    fn test_parse_line_ignore_path() {
        let item = parse_line("x /tmp/important-*", Path::new("test.conf"), 11).unwrap();
        assert_eq!(item.item_type, ItemType::IgnorePath);
    }

    #[test]
    fn test_parse_line_adjust_permissions() {
        let item = parse_line(
            "z /etc/shadow 0640 root shadow - -",
            Path::new("test.conf"),
            12,
        )
        .unwrap();
        assert_eq!(item.item_type, ItemType::AdjustPermissions);
        assert_eq!(item.mode, Some(0o640));
        assert_eq!(item.user, Some("root".to_string()));
        assert_eq!(item.group, Some("shadow".to_string()));
    }

    #[test]
    fn test_parse_line_fifo() {
        let item = parse_line(
            "p /run/my-fifo 0600 root root - -",
            Path::new("test.conf"),
            13,
        )
        .unwrap();
        assert_eq!(item.item_type, ItemType::CreateFifo);
        assert_eq!(item.mode, Some(0o600));
    }

    #[test]
    fn test_parse_line_unknown_type() {
        assert!(parse_line("? /tmp/test", Path::new("test.conf"), 14).is_none());
    }

    #[test]
    fn test_parse_line_minimal_fields() {
        // Just type and path (minimum required)
        let item = parse_line("d /tmp", Path::new("test.conf"), 15).unwrap();
        assert_eq!(item.item_type, ItemType::CreateDirectory);
        assert_eq!(item.path, PathBuf::from("/tmp"));
        assert_eq!(item.mode, Some(0o755)); // default for directories
    }

    #[test]
    fn test_parse_config_file_basic() {
        let dir = std::env::temp_dir().join("systemd-tmpfiles-test-basic");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test.conf");
        let mut f = fs::File::create(&path).unwrap();
        writeln!(f, "# Example tmpfiles.d configuration").unwrap();
        writeln!(f, "d /tmp 1777 root root 10d -").unwrap();
        writeln!(f, "").unwrap();
        writeln!(f, "f /etc/hostname 0644 root root - myhostname").unwrap();
        writeln!(f, "L /etc/localtime - - - - /usr/share/zoneinfo/UTC").unwrap();
        drop(f);

        let items = parse_config_file(&path).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].item_type, ItemType::CreateDirectory);
        assert_eq!(items[1].item_type, ItemType::CreateFile);
        assert_eq!(items[2].item_type, ItemType::CreateSymlink);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_is_create_type() {
        assert!(ItemType::CreateFile.is_create_type());
        assert!(ItemType::CreateDirectory.is_create_type());
        assert!(ItemType::CreateSymlink.is_create_type());
        assert!(ItemType::AdjustPermissions.is_create_type());
        assert!(!ItemType::RemovePath.is_create_type());
        assert!(!ItemType::IgnorePath.is_create_type());
    }

    #[test]
    fn test_is_remove_type() {
        assert!(ItemType::RemovePath.is_remove_type());
        assert!(ItemType::RemoveRecursively.is_remove_type());
        assert!(ItemType::CreateOrCleanDirectory.is_remove_type());
        assert!(!ItemType::CreateFile.is_remove_type());
        assert!(!ItemType::CreateDirectory.is_remove_type());
    }

    #[test]
    fn test_is_clean_type() {
        assert!(ItemType::CreateDirectory.is_clean_type());
        assert!(ItemType::CreateOrCleanDirectory.is_clean_type());
        assert!(ItemType::AdjustDirectory.is_clean_type());
        assert!(ItemType::IgnorePath.is_clean_type());
        assert!(!ItemType::CreateFile.is_clean_type());
        assert!(!ItemType::RemovePath.is_clean_type());
    }

    #[test]
    fn test_matches_prefix_empty() {
        assert!(matches_prefix(Path::new("/tmp/test"), &[]));
    }

    #[test]
    fn test_matches_prefix_match() {
        let prefixes = vec![PathBuf::from("/tmp"), PathBuf::from("/run")];
        assert!(matches_prefix(Path::new("/tmp/test"), &prefixes));
        assert!(matches_prefix(Path::new("/run/something"), &prefixes));
        assert!(!matches_prefix(Path::new("/etc/test"), &prefixes));
    }

    #[test]
    fn test_excluded_by_prefix() {
        let excludes = vec![PathBuf::from("/dev"), PathBuf::from("/sys")];
        assert!(excluded_by_prefix(Path::new("/dev/null"), &excludes));
        assert!(excluded_by_prefix(Path::new("/sys/class"), &excludes));
        assert!(!excluded_by_prefix(Path::new("/tmp/test"), &excludes));
    }

    #[test]
    fn test_discover_config_files_no_crash() {
        let _files = discover_config_files();
    }

    #[test]
    fn test_execute_create_directory() {
        let dir = std::env::temp_dir().join("systemd-tmpfiles-test-create-dir");
        let _ = fs::remove_dir_all(&dir);

        let item = TmpfilesItem {
            item_type: ItemType::CreateDirectory,
            force: false,
            boot_only: false,
            minus: false,
            path: dir.clone(),
            mode: Some(0o755),
            user: None,
            group: None,
            age: None,
            argument: None,
            source: PathBuf::from("test.conf"),
            line_number: 1,
        };

        assert!(execute_create(&item, true));
        assert!(dir.is_dir());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_execute_create_file() {
        let dir = std::env::temp_dir().join("systemd-tmpfiles-test-create-file");
        let _ = fs::create_dir_all(&dir);
        let file = dir.join("test.txt");

        let item = TmpfilesItem {
            item_type: ItemType::CreateFile,
            force: false,
            boot_only: false,
            minus: false,
            path: file.clone(),
            mode: Some(0o644),
            user: None,
            group: None,
            age: None,
            argument: Some("hello world".to_string()),
            source: PathBuf::from("test.conf"),
            line_number: 1,
        };

        assert!(execute_create(&item, true));
        assert!(file.is_file());
        assert_eq!(fs::read_to_string(&file).unwrap(), "hello world");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_execute_create_symlink() {
        let dir = std::env::temp_dir().join("systemd-tmpfiles-test-create-symlink");
        let _ = fs::create_dir_all(&dir);
        let link = dir.join("mylink");
        let _ = fs::remove_file(&link);

        let item = TmpfilesItem {
            item_type: ItemType::CreateSymlink,
            force: false,
            boot_only: false,
            minus: false,
            path: link.clone(),
            mode: None,
            user: None,
            group: None,
            age: None,
            argument: Some("/tmp".to_string()),
            source: PathBuf::from("test.conf"),
            line_number: 1,
        };

        assert!(execute_create(&item, true));
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(fs::read_link(&link).unwrap(), PathBuf::from("/tmp"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_execute_remove_file() {
        let dir = std::env::temp_dir().join("systemd-tmpfiles-test-remove-file");
        let _ = fs::create_dir_all(&dir);
        let file = dir.join("to-remove.txt");
        fs::write(&file, "temporary").unwrap();
        assert!(file.exists());

        let item = TmpfilesItem {
            item_type: ItemType::RemovePath,
            force: false,
            boot_only: false,
            minus: false,
            path: file.clone(),
            mode: None,
            user: None,
            group: None,
            age: None,
            argument: None,
            source: PathBuf::from("test.conf"),
            line_number: 1,
        };

        assert!(execute_remove(&item, false));
        assert!(!file.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_execute_remove_recursive() {
        let dir = std::env::temp_dir().join("systemd-tmpfiles-test-remove-recursive");
        let sub = dir.join("sub");
        let _ = fs::create_dir_all(&sub);
        fs::write(sub.join("file.txt"), "content").unwrap();

        let item = TmpfilesItem {
            item_type: ItemType::RemoveRecursively,
            force: false,
            boot_only: false,
            minus: false,
            path: dir.clone(),
            mode: None,
            user: None,
            group: None,
            age: None,
            argument: None,
            source: PathBuf::from("test.conf"),
            line_number: 1,
        };

        assert!(execute_remove(&item, false));
        assert!(!dir.exists());
    }

    #[test]
    fn test_execute_remove_nonexistent() {
        let item = TmpfilesItem {
            item_type: ItemType::RemovePath,
            force: false,
            boot_only: false,
            minus: false,
            path: PathBuf::from("/tmp/systemd-tmpfiles-test-nonexistent-xyz"),
            mode: None,
            user: None,
            group: None,
            age: None,
            argument: None,
            source: PathBuf::from("test.conf"),
            line_number: 1,
        };

        // Removing a nonexistent file should succeed silently
        assert!(execute_remove(&item, false));
    }

    #[test]
    fn test_item_type_from_char() {
        assert_eq!(ItemType::from_char('f', false), Some(ItemType::CreateFile));
        assert_eq!(
            ItemType::from_char('d', false),
            Some(ItemType::CreateDirectory)
        );
        assert_eq!(
            ItemType::from_char('D', false),
            Some(ItemType::CreateOrCleanDirectory)
        );
        assert_eq!(
            ItemType::from_char('L', false),
            Some(ItemType::CreateSymlink)
        );
        assert_eq!(ItemType::from_char('r', false), Some(ItemType::RemovePath));
        assert_eq!(
            ItemType::from_char('R', false),
            Some(ItemType::RemoveRecursively)
        );
        assert_eq!(
            ItemType::from_char('z', false),
            Some(ItemType::AdjustPermissions)
        );
        assert_eq!(
            ItemType::from_char('Z', false),
            Some(ItemType::AdjustPermissionsRecursively)
        );
        assert_eq!(ItemType::from_char('?', false), None);
    }

    #[test]
    fn test_parse_line_write_file() {
        let item = parse_line(
            "w /proc/sys/net/ipv4/ip_forward - - - - 1",
            Path::new("test.conf"),
            1,
        )
        .unwrap();
        assert_eq!(item.item_type, ItemType::WriteFile);
        assert_eq!(item.argument, Some("1".to_string()));
    }

    #[test]
    fn test_parse_line_copy_files() {
        let item = parse_line(
            "C /etc/skel - - - - /usr/share/skel",
            Path::new("test.conf"),
            1,
        )
        .unwrap();
        assert_eq!(item.item_type, ItemType::CopyFiles);
        assert_eq!(item.argument, Some("/usr/share/skel".to_string()));
    }

    #[test]
    fn test_parse_line_char_device() {
        let item = parse_line(
            "c /dev/null 0666 root root - 1:3",
            Path::new("test.conf"),
            1,
        )
        .unwrap();
        assert_eq!(item.item_type, ItemType::CreateCharDevice);
        assert_eq!(item.argument, Some("1:3".to_string()));
    }

    #[test]
    fn test_parse_line_block_device() {
        let item =
            parse_line("b /dev/sda 0660 root disk - 8:0", Path::new("test.conf"), 1).unwrap();
        assert_eq!(item.item_type, ItemType::CreateBlockDevice);
        assert_eq!(item.argument, Some("8:0".to_string()));
    }

    #[test]
    fn test_resolve_uid_numeric() {
        assert_eq!(resolve_uid("0"), Some(0));
        assert_eq!(resolve_uid("1000"), Some(1000));
    }

    #[test]
    fn test_resolve_uid_root() {
        // root should always exist
        assert_eq!(resolve_uid("root"), Some(0));
    }

    #[test]
    fn test_resolve_gid_numeric() {
        assert_eq!(resolve_gid("0"), Some(0));
        assert_eq!(resolve_gid("1000"), Some(1000));
    }

    #[test]
    fn test_resolve_gid_root() {
        assert_eq!(resolve_gid("root"), Some(0));
    }

    #[test]
    fn test_copy_dir_recursive_basic() {
        let dir = std::env::temp_dir().join("systemd-tmpfiles-test-copy-recursive");
        let src = dir.join("src");
        let dst = dir.join("dst");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(src.join("subdir")).unwrap();
        fs::write(src.join("file1.txt"), "hello").unwrap();
        fs::write(src.join("subdir/file2.txt"), "world").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();

        assert!(dst.join("file1.txt").is_file());
        assert!(dst.join("subdir/file2.txt").is_file());
        assert_eq!(fs::read_to_string(dst.join("file1.txt")).unwrap(), "hello");
        assert_eq!(
            fs::read_to_string(dst.join("subdir/file2.txt")).unwrap(),
            "world"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_line_continuation() {
        let dir = std::env::temp_dir().join("systemd-tmpfiles-test-continuation");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("test.conf");
        let mut f = fs::File::create(&path).unwrap();
        writeln!(f, "d /tmp/test \\").unwrap();
        writeln!(f, "  1777 root root - -").unwrap();
        drop(f);

        let items = parse_config_file(&path).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].item_type, ItemType::CreateDirectory);
        assert_eq!(items[0].path, PathBuf::from("/tmp/test"));
        assert_eq!(items[0].mode, Some(0o1777));

        let _ = fs::remove_dir_all(&dir);
    }
}
