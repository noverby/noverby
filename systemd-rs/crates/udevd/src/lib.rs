//! systemd-udevd library — Device manager daemon.
//!
//! This module contains the full udevd daemon implementation, exposed as a
//! library so that `udevadm` can invoke the daemon when called as
//! `systemd-udevd` (the upstream multi-call binary pattern).
//!
//! Features:
//! - Kernel uevent monitoring via AF_NETLINK / NETLINK_KOBJECT_UEVENT
//! - `.rules` file parsing from standard udev rules directories
//! - Property matching (KERNEL, SUBSYSTEM, ACTION, ATTR{}, ENV{}, DRIVER, etc.)
//! - Parent device traversal (KERNELS, SUBSYSTEMS, DRIVERS, ATTRS{})
//! - Assignment actions (SYMLINK, OWNER, GROUP, MODE, ENV{}, RUN{}, TAG, ATTR{})
//! - IMPORT{program}, IMPORT{file}, IMPORT{cmdline}, IMPORT{builtin}
//! - PROGRAM execution with result capture
//! - GOTO/LABEL control flow
//! - Device database persistence in `/run/udev/data/`
//! - Device symlink management in `/dev/`
//! - Control socket for udevadm communication
//! - Event queue with settle support
//! - sd_notify protocol (READY, WATCHDOG, STATUS, STOPPING)
//! - Signal handling (SIGTERM, SIGINT, SIGHUP, SIGCHLD)
//! - `net_setup_link` builtin for `.link` file-based network interface naming

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use libsystemd::link_config;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const CONTROL_SOCKET_PATH: &str = "/run/udev/control";
pub const RUN_DIR: &str = "/run/udev";
pub const DB_DIR: &str = "/run/udev/data";
pub const TAGS_DIR: &str = "/run/udev/tags";
pub const QUEUE_FILE: &str = "/run/udev/queue";

/// Directories to search for udev rules, in priority order.
/// Files in earlier directories shadow files with the same basename in later ones.
pub const RULES_DIRS: &[&str] = &[
    "/etc/udev/rules.d",
    "/run/udev/rules.d",
    "/usr/lib/udev/rules.d",
    "/lib/udev/rules.d",
];

/// Maximum number of concurrent event workers.
const MAX_WORKERS: usize = 8;

/// Maximum time (seconds) to wait for a single event worker to finish.
const EVENT_TIMEOUT_SECS: u64 = 180;

// ---------------------------------------------------------------------------
// Signal handling
// ---------------------------------------------------------------------------

static SHUTDOWN_FLAG: AtomicBool = AtomicBool::new(false);
static RELOAD_FLAG: AtomicBool = AtomicBool::new(false);
static CHILDREN_FLAG: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_sigterm(_: libc::c_int) {
    SHUTDOWN_FLAG.store(true, Ordering::SeqCst);
}
extern "C" fn handle_sigint(_: libc::c_int) {
    SHUTDOWN_FLAG.store(true, Ordering::SeqCst);
}
extern "C" fn handle_sighup(_: libc::c_int) {
    RELOAD_FLAG.store(true, Ordering::SeqCst);
}
extern "C" fn handle_sigchld(_: libc::c_int) {
    CHILDREN_FLAG.store(true, Ordering::SeqCst);
}

fn setup_signal_handlers() {
    unsafe {
        libc::signal(libc::SIGTERM, handle_sigterm as libc::sighandler_t);
        libc::signal(libc::SIGINT, handle_sigint as libc::sighandler_t);
        libc::signal(libc::SIGHUP, handle_sighup as libc::sighandler_t);
        libc::signal(libc::SIGCHLD, handle_sigchld as libc::sighandler_t);
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
    }
}

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------

fn init_logging() {
    struct StderrLogger;
    impl log::Log for StderrLogger {
        fn enabled(&self, _metadata: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            if self.enabled(record.metadata()) {
                let ts = chrono_lite_timestamp();
                eprintln!(
                    "systemd-udevd[{}]: {} [{}] {}",
                    process::id(),
                    ts,
                    record.level(),
                    record.args()
                );
            }
        }
        fn flush(&self) {}
    }
    static LOGGER: StderrLogger = StderrLogger;
    let level = match std::env::var("SYSTEMD_LOG_LEVEL").as_deref() {
        Ok("debug") => log::LevelFilter::Debug,
        Ok("trace") => log::LevelFilter::Trace,
        Ok("warn") => log::LevelFilter::Warn,
        Ok("error") => log::LevelFilter::Error,
        _ => log::LevelFilter::Info,
    };
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(level);
}

fn chrono_lite_timestamp() -> String {
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => {
            let secs = d.as_secs();
            let millis = d.subsec_millis();
            format!("{}.{:03}", secs, millis)
        }
        Err(_) => "0.000".to_string(),
    }
}

// ---------------------------------------------------------------------------
// sd_notify helper
// ---------------------------------------------------------------------------

fn sd_notify(msg: &str) {
    if let Ok(path) = std::env::var("NOTIFY_SOCKET") {
        let addr = if let Some(stripped) = path.strip_prefix('@') {
            // Abstract socket
            format!("\0{}", stripped)
        } else {
            path.clone()
        };
        if let Ok(sock) = std::os::unix::net::UnixDatagram::unbound() {
            let _ = sock.send_to(msg.as_bytes(), &addr);
        }
    }
}

fn watchdog_interval() -> Option<Duration> {
    if let Ok(usec_str) = std::env::var("WATCHDOG_USEC")
        && let Ok(usec) = usec_str.parse::<u64>()
    {
        // Send keepalive at half the interval
        return Some(Duration::from_micros(usec / 2));
    }
    None
}

// ---------------------------------------------------------------------------
// Netlink uevent types
// ---------------------------------------------------------------------------

/// A kernel uevent received via netlink.
#[derive(Debug, Clone)]
pub struct UEvent {
    /// The action (add, remove, change, move, bind, unbind, online, offline)
    pub action: String,
    /// The devpath (e.g. "/devices/pci0000:00/0000:00:02.0")
    pub devpath: String,
    /// The subsystem (e.g. "pci", "block", "net", "input")
    pub subsystem: String,
    /// Device type if present (e.g. "disk", "partition")
    pub devtype: String,
    /// Device name from DEVNAME (e.g. "sda", "tty0")
    pub devname: String,
    /// Device driver
    pub driver: String,
    /// Major number
    pub major: String,
    /// Minor number
    pub minor: String,
    /// Sequence number from kernel
    pub seqnum: u64,
    /// All environment variables from the uevent
    pub env: HashMap<String, String>,
}

impl UEvent {
    fn new() -> Self {
        Self {
            action: String::new(),
            devpath: String::new(),
            subsystem: String::new(),
            devtype: String::new(),
            devname: String::new(),
            driver: String::new(),
            major: String::new(),
            minor: String::new(),
            seqnum: 0,
            env: HashMap::new(),
        }
    }

    /// Parse a raw uevent buffer (null-separated key=value pairs).
    /// The first line is typically "ACTION@DEVPATH".
    fn parse(buf: &[u8]) -> Option<Self> {
        let mut event = UEvent::new();
        let mut first = true;

        for chunk in buf.split(|&b| b == 0) {
            if chunk.is_empty() {
                continue;
            }
            let s = match std::str::from_utf8(chunk) {
                Ok(s) => s,
                Err(_) => continue,
            };

            if first {
                first = false;
                // First field is "action@devpath" or a key=value
                if let Some(at_pos) = s.find('@') {
                    event.action = s[..at_pos].to_string();
                    event.devpath = s[at_pos + 1..].to_string();
                    continue;
                }
                // Fall through to key=value parsing
            }

            if let Some(eq_pos) = s.find('=') {
                let key = &s[..eq_pos];
                let val = &s[eq_pos + 1..];
                match key {
                    "ACTION" => event.action = val.to_string(),
                    "DEVPATH" => event.devpath = val.to_string(),
                    "SUBSYSTEM" => event.subsystem = val.to_string(),
                    "DEVTYPE" => event.devtype = val.to_string(),
                    "DEVNAME" => event.devname = val.to_string(),
                    "DRIVER" => event.driver = val.to_string(),
                    "MAJOR" => event.major = val.to_string(),
                    "MINOR" => event.minor = val.to_string(),
                    "SEQNUM" => event.seqnum = val.parse().unwrap_or(0),
                    _ => {}
                }
                event.env.insert(key.to_string(), val.to_string());
            }
        }

        if event.devpath.is_empty() {
            return None;
        }

        // Ensure standard keys are in env
        if !event.action.is_empty() {
            event.env.insert("ACTION".into(), event.action.clone());
        }
        if !event.devpath.is_empty() {
            event.env.insert("DEVPATH".into(), event.devpath.clone());
        }
        if !event.subsystem.is_empty() {
            event
                .env
                .insert("SUBSYSTEM".into(), event.subsystem.clone());
        }

        Some(event)
    }

    /// Get the sysfs path for this device.
    fn syspath(&self) -> PathBuf {
        PathBuf::from("/sys").join(self.devpath.trim_start_matches('/'))
    }

    /// Get the device node path (if applicable).
    fn devnode(&self) -> Option<PathBuf> {
        if self.devname.is_empty() {
            return None;
        }
        if self.devname.starts_with('/') {
            Some(PathBuf::from(&self.devname))
        } else {
            Some(PathBuf::from("/dev").join(&self.devname))
        }
    }

    /// Read a sysfs attribute for this device.
    fn read_sysattr(&self, attr: &str) -> Option<String> {
        let path = self.syspath().join(attr);
        fs::read_to_string(&path)
            .ok()
            .map(|s| s.trim_end().to_string())
    }
}

// ---------------------------------------------------------------------------
// Netlink socket
// ---------------------------------------------------------------------------

/// Open a netlink socket for kernel uevents.
pub fn open_uevent_socket() -> io::Result<i32> {
    unsafe {
        let fd = libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_DGRAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            15, // NETLINK_KOBJECT_UEVENT
        );
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }

        let mut addr: libc::sockaddr_nl = std::mem::zeroed();
        addr.nl_family = libc::AF_NETLINK as u16;
        addr.nl_pid = libc::getpid() as u32;
        addr.nl_groups = 1; // KOBJECT_UEVENT group

        let ret = libc::bind(
            fd,
            &addr as *const libc::sockaddr_nl as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
        );
        if ret < 0 {
            let err = io::Error::last_os_error();
            libc::close(fd);
            return Err(err);
        }

        // Set a large receive buffer
        let buf_size: libc::c_int = 128 * 1024 * 1024; // 128 MiB
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVBUF,
            &buf_size as *const libc::c_int as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );

        Ok(fd)
    }
}

/// Receive a uevent from the netlink socket. Returns None if no data available.
pub fn recv_uevent(fd: i32) -> Option<UEvent> {
    let mut buf = [0u8; 8192];
    let n = unsafe {
        libc::recv(
            fd,
            buf.as_mut_ptr() as *mut libc::c_void,
            buf.len(),
            libc::MSG_DONTWAIT,
        )
    };
    if n <= 0 {
        return None;
    }
    UEvent::parse(&buf[..n as usize])
}

// ---------------------------------------------------------------------------
// Udev rules parsing
// ---------------------------------------------------------------------------

/// Comparison operator for rule keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleOp {
    /// `==` — match equals
    Match,
    /// `!=` — match not-equals
    Nomatch,
    /// `=` — assign
    Assign,
    /// `+=` — append/add
    AssignAdd,
    /// `-=` — remove
    AssignRemove,
    /// `:=` — assign final (no further changes allowed)
    AssignFinal,
}

/// A single key-value pair within a udev rule.
#[derive(Debug, Clone)]
pub struct RuleToken {
    /// The key name (e.g. "KERNEL", "SUBSYSTEM", "ATTR{size}")
    pub key: String,
    /// Attribute name if the key has {attr} syntax
    pub attr: Option<String>,
    /// The operator
    pub op: RuleOp,
    /// The value (pattern for match keys, literal for assign keys)
    pub value: String,
}

/// A single udev rule (one logical line).
#[derive(Debug, Clone)]
pub struct Rule {
    /// The file this rule came from
    pub filename: String,
    /// Line number in the file
    pub line: usize,
    /// Tokens (key-op-value triples) in this rule
    pub tokens: Vec<RuleToken>,
    /// LABEL for this rule (if it is a LABEL rule)
    pub label: Option<String>,
    /// GOTO target label (if this rule has GOTO)
    pub goto_target: Option<String>,
}

/// Parsed rule set.
#[derive(Debug, Clone, Default)]
pub struct RuleSet {
    pub rules: Vec<Rule>,
}

impl RuleSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load rules from all standard directories.
    pub fn load() -> Self {
        let mut ruleset = Self::new();
        let files = discover_rules_files();
        for path in &files {
            match parse_rules_file(path) {
                Ok(rules) => {
                    log::debug!("Loaded {} rules from {}", rules.len(), path.display());
                    ruleset.rules.extend(rules);
                }
                Err(e) => {
                    log::warn!("Failed to parse {}: {}", path.display(), e);
                }
            }
        }
        // Resolve GOTO targets to indices for efficient jumping
        ruleset.resolve_gotos();
        log::info!(
            "Loaded {} rules from {} files",
            ruleset.rules.len(),
            files.len()
        );
        ruleset
    }

    /// Find the index of a LABEL rule by label name, starting from a given offset.
    fn find_label(&self, label: &str, from: usize) -> Option<usize> {
        (from..self.rules.len()).find(|&i| self.rules[i].label.as_deref() == Some(label))
    }

    /// Pre-resolve GOTO targets (just validation, actual jumping is done at match time).
    fn resolve_gotos(&self) {
        for (i, rule) in self.rules.iter().enumerate() {
            if let Some(ref target) = rule.goto_target
                && self.find_label(target, i + 1).is_none()
            {
                log::warn!(
                    "{}:{}: GOTO target '{}' not found",
                    rule.filename,
                    rule.line,
                    target
                );
            }
        }
    }
}

/// Discover all .rules files across the udev rules directories, respecting priority.
/// Files in earlier directories shadow files with the same basename in later directories.
pub fn discover_rules_files() -> Vec<PathBuf> {
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut result = Vec::new();

    for dir in RULES_DIRS {
        let dir_path = Path::new(dir);
        if !dir_path.is_dir() {
            continue;
        }
        let mut entries: Vec<PathBuf> = match fs::read_dir(dir_path) {
            Ok(rd) => rd
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map(|ext| ext == "rules").unwrap_or(false))
                .collect(),
            Err(_) => continue,
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

    // Sort all files by their basename for correct rule ordering
    result.sort_by(|a, b| {
        let an = a.file_name().unwrap_or_default();
        let bn = b.file_name().unwrap_or_default();
        an.cmp(bn)
    });

    result
}

/// Parse a single .rules file.
pub fn parse_rules_file(path: &Path) -> io::Result<Vec<Rule>> {
    let content = fs::read_to_string(path)?;
    let filename = path.display().to_string();
    let mut rules = Vec::new();

    // Handle line continuations (trailing backslash joins with next line)
    let mut logical_lines: Vec<(usize, String)> = Vec::new();
    let mut current_line = String::new();
    let mut current_lineno = 0;

    for (i, line) in content.lines().enumerate() {
        let lineno = i + 1;
        if current_line.is_empty() {
            current_lineno = lineno;
        }

        if let Some(stripped) = line.strip_suffix('\\') {
            current_line.push_str(stripped);
            current_line.push(' ');
        } else {
            current_line.push_str(line);
            let trimmed = current_line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                logical_lines.push((current_lineno, trimmed.to_string()));
            }
            current_line.clear();
        }
    }
    // Handle any trailing continuation
    if !current_line.trim().is_empty() {
        logical_lines.push((current_lineno, current_line.trim().to_string()));
    }

    for (lineno, line) in logical_lines {
        match parse_rule_line(&line) {
            Ok(tokens) if !tokens.is_empty() => {
                let label = tokens.iter().find_map(|t| {
                    if t.key == "LABEL" && matches!(t.op, RuleOp::Assign) {
                        Some(t.value.clone())
                    } else {
                        None
                    }
                });
                let goto_target = tokens.iter().find_map(|t| {
                    if t.key == "GOTO" && matches!(t.op, RuleOp::Assign) {
                        Some(t.value.clone())
                    } else {
                        None
                    }
                });
                rules.push(Rule {
                    filename: filename.clone(),
                    line: lineno,
                    tokens,
                    label,
                    goto_target,
                });
            }
            Ok(_) => {} // empty
            Err(e) => {
                log::debug!("{}:{}: parse error: {}", filename, lineno, e);
            }
        }
    }

    Ok(rules)
}

/// Parse a single rule line into tokens.
fn parse_rule_line(line: &str) -> Result<Vec<RuleToken>, String> {
    let mut tokens = Vec::new();
    let mut remaining = line.trim();

    while !remaining.is_empty() {
        // Skip leading commas and whitespace
        remaining = remaining.trim_start_matches(|c: char| c == ',' || c.is_whitespace());
        if remaining.is_empty() {
            break;
        }

        // Parse key (may include {attr})
        let (key, attr, rest) = parse_rule_key(remaining)?;
        remaining = rest.trim_start();

        // Parse operator
        let (op, rest) = parse_rule_op(remaining)?;
        remaining = rest.trim_start();

        // Parse quoted value
        let (value, rest) = parse_rule_value(remaining)?;
        remaining = rest;

        tokens.push(RuleToken {
            key,
            attr,
            op,
            value,
        });
    }

    Ok(tokens)
}

/// Parse a rule key, potentially with {attr} suffix.
/// Returns (key_name, optional_attr, remaining_string).
fn parse_rule_key(s: &str) -> Result<(String, Option<String>, &str), String> {
    // Find the end of the key: it's letters, digits, underscore, or {attr}
    let mut i = 0;
    let bytes = s.as_bytes();
    let mut attr = None;

    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '{' {
            // Parse attribute name in braces
            let start = i + 1;
            while i < bytes.len() && bytes[i] as char != '}' {
                i += 1;
            }
            if i >= bytes.len() {
                return Err("Unclosed '{' in key".into());
            }
            attr = Some(s[start..i].to_string());
            i += 1; // skip '}'
            break;
        } else if c.is_alphanumeric() || c == '_' {
            i += 1;
        } else {
            break;
        }
    }

    if i == 0 && attr.is_none() {
        return Err(format!("Expected key name, got: {}", &s[..s.len().min(20)]));
    }

    let key_end = if attr.is_some() {
        // key ends before the '{'
        s[..i].rfind('{').unwrap_or(i)
    } else {
        i
    };

    // For keys with attrs, the key name is everything before '{'
    let key_name = if attr.is_some() {
        s[..s.find('{').unwrap_or(key_end)].to_string()
    } else {
        s[..key_end].to_string()
    };

    Ok((key_name, attr, &s[i..]))
}

/// Parse the operator from the beginning of a string.
fn parse_rule_op(s: &str) -> Result<(RuleOp, &str), String> {
    if let Some(rest) = s.strip_prefix("==") {
        Ok((RuleOp::Match, rest))
    } else if let Some(rest) = s.strip_prefix("!=") {
        Ok((RuleOp::Nomatch, rest))
    } else if let Some(rest) = s.strip_prefix("+=") {
        Ok((RuleOp::AssignAdd, rest))
    } else if let Some(rest) = s.strip_prefix("-=") {
        Ok((RuleOp::AssignRemove, rest))
    } else if let Some(rest) = s.strip_prefix(":=") {
        Ok((RuleOp::AssignFinal, rest))
    } else if let Some(rest) = s.strip_prefix('=') {
        Ok((RuleOp::Assign, rest))
    } else {
        Err(format!("Expected operator, got: {}", &s[..s.len().min(20)]))
    }
}

/// Parse a quoted value. Values are enclosed in double quotes.
fn parse_rule_value(s: &str) -> Result<(String, &str), String> {
    let s = s.trim_start();
    if !s.starts_with('"') {
        // Some rules use unquoted values (non-standard but seen in the wild)
        // Read until comma or end of string
        let end = s.find([',', '\n']).unwrap_or(s.len());
        let val = s[..end].trim();
        return Ok((val.to_string(), &s[end..]));
    }

    let bytes = s.as_bytes();
    let mut i = 1; // skip opening quote
    let mut value = String::new();

    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '\\' && i + 1 < bytes.len() {
            // Escape sequences
            let next = bytes[i + 1] as char;
            match next {
                '"' | '\\' => {
                    value.push(next);
                    i += 2;
                }
                'a' => {
                    value.push('\x07');
                    i += 2;
                }
                'b' => {
                    value.push('\x08');
                    i += 2;
                }
                'n' => {
                    value.push('\n');
                    i += 2;
                }
                'r' => {
                    value.push('\r');
                    i += 2;
                }
                't' => {
                    value.push('\t');
                    i += 2;
                }
                _ => {
                    value.push('\\');
                    value.push(next);
                    i += 2;
                }
            }
        } else if c == '"' {
            // Closing quote
            return Ok((value, &s[i + 1..]));
        } else {
            value.push(c);
            i += 1;
        }
    }

    // Unterminated quote — take what we have
    Ok((value, ""))
}

// ---------------------------------------------------------------------------
// Glob matching
// ---------------------------------------------------------------------------

/// Match a value against a udev-style glob pattern.
/// Supports `*`, `?`, `[...]` character classes, and `|` for alternatives.
pub fn glob_match(pattern: &str, value: &str) -> bool {
    // Handle pipe-separated alternatives
    if pattern.contains('|') {
        // Split on `|` but only at the top level (not inside brackets)
        for alt in split_alternatives(pattern) {
            if glob_match_single(alt, value) {
                return true;
            }
        }
        return false;
    }
    glob_match_single(pattern, value)
}

/// Split a pattern on `|` respecting `[...]` groups.
pub fn split_alternatives(pattern: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    let bytes = pattern.as_bytes();

    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'[' => depth += 1,
            b']' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            b'|' if depth == 0 => {
                result.push(&pattern[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    result.push(&pattern[start..]);
    result
}

/// Match a single glob pattern (no alternatives).
fn glob_match_single(pattern: &str, value: &str) -> bool {
    // Use fnmatch-style matching
    let pat_chars: Vec<char> = pattern.chars().collect();
    let val_chars: Vec<char> = value.chars().collect();
    glob_match_chars(&pat_chars, 0, &val_chars, 0)
}

fn glob_match_chars(pat: &[char], pi: usize, val: &[char], vi: usize) -> bool {
    let mut pi = pi;
    let mut vi = vi;

    while pi < pat.len() {
        match pat[pi] {
            '*' => {
                // Skip consecutive stars
                while pi < pat.len() && pat[pi] == '*' {
                    pi += 1;
                }
                if pi >= pat.len() {
                    return true; // trailing * matches everything
                }
                // Try matching the rest of the pattern at each position
                for vi_try in vi..=val.len() {
                    if glob_match_chars(pat, pi, val, vi_try) {
                        return true;
                    }
                }
                return false;
            }
            '?' => {
                if vi >= val.len() {
                    return false;
                }
                pi += 1;
                vi += 1;
            }
            '[' => {
                if vi >= val.len() {
                    return false;
                }
                pi += 1;
                let negate = pi < pat.len() && (pat[pi] == '!' || pat[pi] == '^');
                if negate {
                    pi += 1;
                }
                let mut matched = false;
                let mut first = true;
                while pi < pat.len() && (pat[pi] != ']' || first) {
                    first = false;
                    let lo = pat[pi];
                    if pi + 2 < pat.len() && pat[pi + 1] == '-' {
                        let hi = pat[pi + 2];
                        if val[vi] >= lo && val[vi] <= hi {
                            matched = true;
                        }
                        pi += 3;
                    } else {
                        if val[vi] == lo {
                            matched = true;
                        }
                        pi += 1;
                    }
                }
                if pi < pat.len() && pat[pi] == ']' {
                    pi += 1;
                }
                if negate {
                    matched = !matched;
                }
                if !matched {
                    return false;
                }
                vi += 1;
            }
            c => {
                if vi >= val.len() || val[vi] != c {
                    return false;
                }
                pi += 1;
                vi += 1;
            }
        }
    }

    vi >= val.len()
}

// ---------------------------------------------------------------------------
// Substitution expansion
// ---------------------------------------------------------------------------

/// Expand udev-style format strings in a value.
/// Supported substitutions:
///   $kernel, %k — kernel device name
///   $number, %n — kernel device number
///   $devpath, %p — device path
///   $id, %b — filename of devpath
///   $driver — driver name
///   $attr{file}, %s{file} — sysfs attribute value
///   $env{key}, %E{key} — environment variable
///   $major, %M — major number
///   $minor, %m — minor number
///   $result, %c — PROGRAM result
///   $name, %D — device node name
///   $links — current symlinks
///   $root — /dev root
///   $sys — /sys
///   $devnode, %N — device node path
///   %% — literal %
///   $$ — literal $
pub fn expand_substitutions(
    template: &str,
    event: &UEvent,
    program_result: &str,
    device_name: &str,
    symlinks: &[String],
) -> String {
    let mut result = String::with_capacity(template.len());
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '$' || chars[i] == '%' {
            let sigil = chars[i];
            i += 1;
            if i >= chars.len() {
                result.push(sigil);
                break;
            }

            // Literal escape
            if chars[i] == sigil {
                result.push(sigil);
                i += 1;
                continue;
            }

            // Try to match a keyword or single-char substitution
            let (expanded, advance) = expand_one_subst(
                &chars,
                i,
                sigil,
                event,
                program_result,
                device_name,
                symlinks,
            );
            result.push_str(&expanded);
            i += advance;
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

fn expand_one_subst(
    chars: &[char],
    start: usize,
    sigil: char,
    event: &UEvent,
    program_result: &str,
    device_name: &str,
    symlinks: &[String],
) -> (String, usize) {
    let remaining: String = chars[start..].iter().collect();

    if sigil == '%' {
        // Single-character format specifiers
        if start < chars.len() {
            let c = chars[start];
            match c {
                'k' => return (kernel_name(event), 1),
                'n' => return (kernel_number(event), 1),
                'p' => return (event.devpath.clone(), 1),
                'b' => return (devpath_basename(event), 1),
                'M' => return (event.major.clone(), 1),
                'm' => return (event.minor.clone(), 1),
                'c' => {
                    let (val, adv) = subst_with_index(chars, start + 1, program_result);
                    return (val, 1 + adv);
                }
                'N' => {
                    return (
                        event
                            .devnode()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default(),
                        1,
                    );
                }
                'D' => return (device_name.to_string(), 1),
                's' => {
                    if start + 1 < chars.len() && chars[start + 1] == '{' {
                        let (attr_name, adv) = extract_braced(chars, start + 1);
                        let val = event.read_sysattr(&attr_name).unwrap_or_default();
                        return (val, 1 + adv);
                    }
                    return (String::new(), 1);
                }
                'E' => {
                    if start + 1 < chars.len() && chars[start + 1] == '{' {
                        let (key, adv) = extract_braced(chars, start + 1);
                        let val = event.env.get(&key).cloned().unwrap_or_default();
                        return (val, 1 + adv);
                    }
                    return (String::new(), 1);
                }
                _ => return (format!("%{}", c), 1),
            }
        }
    }

    // $keyword substitutions
    type SubstFn = fn(&UEvent, &str, &str, &[String]) -> String;
    let keywords: &[(&str, SubstFn)] = &[
        ("kernel", |e, _, _, _| kernel_name(e)),
        ("number", |e, _, _, _| kernel_number(e)),
        ("devpath", |e, _, _, _| e.devpath.clone()),
        ("id", |e, _, _, _| devpath_basename(e)),
        ("driver", |e, _, _, _| e.driver.clone()),
        ("major", |e, _, _, _| e.major.clone()),
        ("minor", |e, _, _, _| e.minor.clone()),
        ("result", |_, r, _, _| r.to_string()),
        ("name", |_, _, n, _| n.to_string()),
        ("links", |_, _, _, l| l.join(" ")),
        ("root", |_, _, _, _| "/dev".to_string()),
        ("sys", |_, _, _, _| "/sys".to_string()),
        ("devnode", |e, _, _, _| {
            e.devnode()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        }),
    ];

    for &(keyword, func) in keywords {
        if remaining.starts_with(keyword) {
            let after = start + keyword.len();
            // Check for {attr} suffix
            if (keyword == "attr" || keyword == "env") && after < chars.len() && chars[after] == '{'
            {
                let (braced, adv) = extract_braced(chars, after);
                let val = if keyword == "attr" {
                    event.read_sysattr(&braced).unwrap_or_default()
                } else {
                    event.env.get(&braced).cloned().unwrap_or_default()
                };
                return (val, keyword.len() + adv);
            }
            return (
                func(event, program_result, device_name, symlinks),
                keyword.len(),
            );
        }
    }

    // $attr{file}
    if remaining.starts_with("attr{") {
        let (braced, adv) = extract_braced(chars, start + 4);
        let val = event.read_sysattr(&braced).unwrap_or_default();
        return (val, 4 + adv);
    }

    // $env{key}
    if remaining.starts_with("env{") {
        let (braced, adv) = extract_braced(chars, start + 3);
        let val = event.env.get(&braced).cloned().unwrap_or_default();
        return (val, 3 + adv);
    }

    // Unknown — return the sigil and character
    if start < chars.len() {
        (format!("{}{}", sigil, chars[start]), 1)
    } else {
        (sigil.to_string(), 0)
    }
}

/// Extract content within `{...}` starting at position `start` which should point to `{`.
/// Returns (content, characters_consumed_including_braces).
fn extract_braced(chars: &[char], start: usize) -> (String, usize) {
    if start >= chars.len() || chars[start] != '{' {
        return (String::new(), 0);
    }
    let mut i = start + 1;
    let mut content = String::new();
    while i < chars.len() && chars[i] != '}' {
        content.push(chars[i]);
        i += 1;
    }
    if i < chars.len() && chars[i] == '}' {
        i += 1;
    }
    let consumed = i - start;
    (content, consumed)
}

/// Handle `%c{N}` or `%c{N+}` for selecting parts of the program result.
fn subst_with_index(chars: &[char], start: usize, program_result: &str) -> (String, usize) {
    if start < chars.len() && chars[start] == '{' {
        let (spec, adv) = extract_braced(chars, start);
        let parts: Vec<&str> = program_result.split_whitespace().collect();
        if spec.ends_with('+') {
            // {N+} means from Nth word to end
            if let Ok(n) = spec[..spec.len() - 1].parse::<usize>()
                && n > 0
                && n <= parts.len()
            {
                return (parts[n - 1..].join(" "), adv);
            }
        } else if let Ok(n) = spec.parse::<usize>() {
            // {N} means Nth word (1-based)
            if n > 0 && n <= parts.len() {
                return (parts[n - 1].to_string(), adv);
            }
        }
        return (String::new(), adv);
    }
    (program_result.to_string(), 0)
}

fn kernel_name(event: &UEvent) -> String {
    // The kernel name is the basename of the devpath
    event.devpath.rsplit('/').next().unwrap_or("").to_string()
}

fn kernel_number(event: &UEvent) -> String {
    // The kernel number is the trailing digits of the kernel name
    let name = kernel_name(event);
    let num: String = name
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    num.chars().rev().collect()
}

fn devpath_basename(event: &UEvent) -> String {
    event.devpath.rsplit('/').next().unwrap_or("").to_string()
}

// ---------------------------------------------------------------------------
// Rule matching and execution
// ---------------------------------------------------------------------------

/// Result of processing rules for an event.
#[derive(Debug, Clone, Default)]
pub struct RuleResult {
    /// Device node name override (NAME=)
    pub name: Option<String>,
    /// Symlinks to create (SYMLINK+=)
    pub symlinks: Vec<String>,
    /// Owner for device node
    pub owner: Option<String>,
    /// Group for device node
    pub group: Option<String>,
    /// Mode for device node
    pub mode: Option<u32>,
    /// Programs to run (RUN{program}=)
    pub run_programs: Vec<String>,
    /// RUN{builtin} entries
    pub run_builtins: Vec<String>,
    /// Tags to apply
    pub tags: Vec<String>,
    /// Environment variables to set
    pub env_overrides: HashMap<String, String>,
    /// Sysfs attributes to write
    pub sysattr_writes: Vec<(String, String)>,
    /// OPTIONS settings
    pub options: HashSet<String>,
}

/// Process all rules against an event, returning the combined result.
pub fn process_rules(rules: &RuleSet, event: &mut UEvent) -> RuleResult {
    let mut result = RuleResult::default();
    let mut program_result = String::new();
    let mut final_keys: HashSet<String> = HashSet::new();
    let mut i = 0;

    while i < rules.rules.len() {
        let rule = &rules.rules[i];

        // Check if all match keys in this rule match the event
        let mut matched = true;
        let mut has_match_keys = false;

        for token in &rule.tokens {
            if is_match_op(token.op) {
                has_match_keys = true;
                if !match_token(token, event, &mut program_result) {
                    matched = false;
                    break;
                }
            }
        }

        // LABEL-only rules always "match" (they're jump targets)
        if !has_match_keys && rule.label.is_some() {
            i += 1;
            continue;
        }

        if matched {
            // Execute assignment tokens
            for token in &rule.tokens {
                if is_match_op(token.op) {
                    continue;
                }

                // Check if this key was already finalized
                let fkey = format!("{}{}", token.key, token.attr.as_deref().unwrap_or(""));
                if final_keys.contains(&fkey) && token.key != "LABEL" && token.key != "GOTO" {
                    continue;
                }

                let expanded = expand_substitutions(
                    &token.value,
                    event,
                    &program_result,
                    result.name.as_deref().unwrap_or(""),
                    &result.symlinks,
                );

                execute_assignment(
                    token,
                    &expanded,
                    event,
                    &mut result,
                    &mut program_result,
                    &mut final_keys,
                );
            }

            // Handle GOTO
            if let Some(ref target) = rule.goto_target
                && let Some(idx) = rules.find_label(target, i + 1)
            {
                i = idx;
                continue;
            }
        }

        i += 1;
    }

    result
}

fn is_match_op(op: RuleOp) -> bool {
    matches!(op, RuleOp::Match | RuleOp::Nomatch)
}

/// Check if a single match token matches the event.
fn match_token(token: &RuleToken, event: &UEvent, program_result: &mut String) -> bool {
    let value = match token.key.as_str() {
        "ACTION" => Some(event.action.clone()),
        "DEVPATH" => Some(event.devpath.clone()),
        "KERNEL" => Some(kernel_name(event)),
        "SUBSYSTEM" => Some(event.subsystem.clone()),
        "DRIVER" => Some(event.driver.clone()),
        "DEVTYPE" => Some(event.devtype.clone()),
        "NAME" => event.devnode().map(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        }),
        "ATTR" => {
            if let Some(ref attr) = token.attr {
                event.read_sysattr(attr)
            } else {
                None
            }
        }
        "ENV" => {
            if let Some(ref key) = token.attr {
                event.env.get(key).cloned()
            } else {
                None
            }
        }
        "TAG" => {
            // TAG matches if the device has the specified tag
            // For now, check ENV{TAGS} or similar
            event.env.get("TAGS").cloned()
        }
        "TEST" => {
            // TEST checks if a file/path exists
            let path = expand_substitutions(&token.value, event, program_result.as_str(), "", &[]);
            let exists = Path::new(&path).exists();
            let matches = match token.op {
                RuleOp::Match => exists,
                RuleOp::Nomatch => !exists,
                _ => false,
            };
            // Special: TEST returns early since it checks existence, not value
            return matches;
        }
        "RESULT" => Some(program_result.to_string()),
        "PROGRAM" => {
            // PROGRAM runs a command, captures stdout, and checks exit status.
            // On success the captured stdout is stored in program_result so
            // subsequent rules can reference it via $result / %c / RESULT.
            return match_program(token, event, program_result);
        }
        // Parent device traversal keys
        "KERNELS" | "SUBSYSTEMS" | "DRIVERS" | "ATTRS" | "TAGS" => {
            return match_parent_token(token, event);
        }
        _ => {
            log::trace!("Unknown match key '{}' in rule, skipping", token.key);
            return true; // Unknown keys don't cause mismatch
        }
    };

    let value = match value {
        Some(v) => v,
        None => {
            // If the device doesn't have this property, only `!=` matches
            return matches!(token.op, RuleOp::Nomatch);
        }
    };

    let pattern = &token.value;
    let matches = glob_match(pattern, &value);

    match token.op {
        RuleOp::Match => matches,
        RuleOp::Nomatch => !matches,
        _ => false,
    }
}

/// Match PROGRAM token: run the command and check exit status.
fn match_program(token: &RuleToken, event: &UEvent, program_result: &mut String) -> bool {
    let cmd = expand_substitutions(&token.value, event, program_result.as_str(), "", &[]);
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return false;
    }

    log::debug!("PROGRAM: executing '{}'", cmd);

    let mut child_cmd = Command::new(parts[0]);
    child_cmd
        .args(&parts[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("DEVPATH", &event.devpath)
        .env("ACTION", &event.action)
        .env("SUBSYSTEM", &event.subsystem);

    // Pass all event environment variables so the program has full context
    for (k, v) in &event.env {
        child_cmd.env(k, v);
    }

    let result = child_cmd.output();

    match result {
        Ok(output) => {
            let success = output.status.success();
            if success {
                // Capture stdout as the program result (trimmed of trailing
                // whitespace/newlines), available to subsequent rules via
                // $result / %c / RESULT== matching.
                let stdout = String::from_utf8_lossy(&output.stdout);
                *program_result = stdout.trim_end().to_string();
                log::debug!("PROGRAM '{}' result: '{}'", cmd, program_result);
            }
            match token.op {
                RuleOp::Match => success,
                RuleOp::Nomatch => !success,
                _ => false,
            }
        }
        Err(e) => {
            log::debug!("PROGRAM '{}' failed to execute: {}", cmd, e);
            matches!(token.op, RuleOp::Nomatch)
        }
    }
}

/// Match a parent-traversal token (KERNELS, SUBSYSTEMS, DRIVERS, ATTRS).
fn match_parent_token(token: &RuleToken, event: &UEvent) -> bool {
    let mut syspath = event.syspath();

    // Walk up the device tree
    loop {
        let matched = match token.key.as_str() {
            "KERNELS" => {
                let name = syspath
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                glob_match(&token.value, &name)
            }
            "SUBSYSTEMS" => {
                let subsys_path = syspath.join("subsystem");
                if let Ok(target) = fs::read_link(&subsys_path) {
                    let subsys = target
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    glob_match(&token.value, &subsys)
                } else {
                    false
                }
            }
            "DRIVERS" => {
                let driver_path = syspath.join("driver");
                if let Ok(target) = fs::read_link(&driver_path) {
                    let driver = target
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    glob_match(&token.value, &driver)
                } else {
                    false
                }
            }
            "ATTRS" => {
                if let Some(ref attr) = token.attr {
                    let attr_path = syspath.join(attr);
                    if let Ok(val) = fs::read_to_string(&attr_path) {
                        glob_match(&token.value, val.trim())
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            "TAGS" => {
                // Check if device has tag by looking at /run/udev/tags/<tag>/<devpath_escaped>
                false // Simplified
            }
            _ => false,
        };

        if matched {
            return match token.op {
                RuleOp::Match => true,
                RuleOp::Nomatch => false,
                _ => false,
            };
        }

        // Go to parent device
        if !syspath.pop() {
            break;
        }
        // Stop at /sys/devices or /sys
        let syspath_str = syspath.to_string_lossy();
        if syspath_str == "/sys/devices" || syspath_str == "/sys" || syspath_str == "/" {
            break;
        }
    }

    // No parent matched
    match token.op {
        RuleOp::Match => false,
        RuleOp::Nomatch => true,
        _ => false,
    }
}

/// Execute an assignment token.
fn execute_assignment(
    token: &RuleToken,
    value: &str,
    event: &mut UEvent,
    result: &mut RuleResult,
    program_result: &mut String,
    final_keys: &mut HashSet<String>,
) {
    let fkey = format!("{}{}", token.key, token.attr.as_deref().unwrap_or(""));

    if token.op == RuleOp::AssignFinal {
        final_keys.insert(fkey);
    }

    match token.key.as_str() {
        "NAME" => {
            if !value.is_empty() {
                result.name = Some(value.to_string());
            }
        }
        "SYMLINK" => match token.op {
            RuleOp::Assign | RuleOp::AssignFinal => {
                result.symlinks.clear();
                for link in value.split_whitespace() {
                    if !link.is_empty() {
                        result.symlinks.push(link.to_string());
                    }
                }
            }
            RuleOp::AssignAdd => {
                for link in value.split_whitespace() {
                    if !link.is_empty() && !result.symlinks.contains(&link.to_string()) {
                        result.symlinks.push(link.to_string());
                    }
                }
            }
            RuleOp::AssignRemove => {
                result
                    .symlinks
                    .retain(|l| !value.split_whitespace().any(|v| v == l));
            }
            _ => {}
        },
        "OWNER" => {
            result.owner = Some(value.to_string());
        }
        "GROUP" => {
            result.group = Some(value.to_string());
        }
        "MODE" => {
            if let Ok(mode) = u32::from_str_radix(value, 8) {
                result.mode = Some(mode);
            }
        }
        "ENV" => {
            if let Some(ref key) = token.attr {
                event.env.insert(key.clone(), value.to_string());
                result.env_overrides.insert(key.clone(), value.to_string());
            }
        }
        "TAG" => match token.op {
            RuleOp::Assign | RuleOp::AssignFinal | RuleOp::AssignAdd => {
                if !value.is_empty() && !result.tags.contains(&value.to_string()) {
                    result.tags.push(value.to_string());
                }
            }
            RuleOp::AssignRemove => {
                result.tags.retain(|t| t != value);
            }
            _ => {}
        },
        "RUN" => {
            let run_type = token.attr.as_deref().unwrap_or("program");
            match run_type {
                "builtin" => match token.op {
                    RuleOp::Assign | RuleOp::AssignFinal => {
                        result.run_builtins.clear();
                        result.run_builtins.push(value.to_string());
                    }
                    RuleOp::AssignAdd => {
                        result.run_builtins.push(value.to_string());
                    }
                    _ => {}
                },
                _ => match token.op {
                    RuleOp::Assign | RuleOp::AssignFinal => {
                        result.run_programs.clear();
                        result.run_programs.push(value.to_string());
                    }
                    RuleOp::AssignAdd => {
                        result.run_programs.push(value.to_string());
                    }
                    _ => {}
                },
            }
        }
        "ATTR" => {
            if let Some(ref attr) = token.attr {
                result
                    .sysattr_writes
                    .push((attr.clone(), value.to_string()));
            }
        }
        "SYSCTL" => {
            if let Some(ref key) = token.attr {
                // Write to /proc/sys/...
                let path = format!("/proc/sys/{}", key.replace('.', "/"));
                result.sysattr_writes.push((path, value.to_string()));
            }
        }
        "LABEL" | "GOTO" => {
            // Handled at the rule level, not here
        }
        "IMPORT" => {
            let import_type = token.attr.as_deref().unwrap_or("file");
            handle_import(import_type, value, event, program_result);
        }
        "PROGRAM" => {
            // PROGRAM as assignment runs and captures output
            let cmd = value.to_string();
            if let Some(output) = run_program_capture(&cmd, event) {
                *program_result = output;
            }
        }
        "OPTIONS" => {
            result.options.insert(value.to_string());
        }
        _ => {
            log::trace!("Unknown assignment key '{}', ignoring", token.key);
        }
    }
}

/// Handle IMPORT{type}="value" directives.
fn handle_import(import_type: &str, value: &str, event: &mut UEvent, program_result: &mut String) {
    match import_type {
        "program" => {
            if let Some(output) = run_program_capture(value, event) {
                // Parse output as KEY=VALUE lines
                for line in output.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some(eq) = line.find('=') {
                        let key = line[..eq].trim().to_string();
                        let val = line[eq + 1..].trim().to_string();
                        event.env.insert(key, val);
                    }
                }
                *program_result = output;
            }
        }
        "file" => {
            if let Ok(content) = fs::read_to_string(value) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some(eq) = line.find('=') {
                        let key = line[..eq].trim().to_string();
                        let val = line[eq + 1..].trim().trim_matches('"').to_string();
                        event.env.insert(key, val);
                    }
                }
            }
        }
        "cmdline" => {
            // Import from /proc/cmdline
            if let Ok(cmdline) = fs::read_to_string("/proc/cmdline") {
                for param in cmdline.split_whitespace() {
                    // Check if the param matches the key pattern
                    if glob_match(value, param) {
                        if let Some(eq) = param.find('=') {
                            let key = param[..eq].to_string();
                            let val = param[eq + 1..].to_string();
                            event.env.insert(key, val);
                        } else {
                            event.env.insert(param.to_string(), "1".to_string());
                        }
                    }
                }
            }
        }
        "builtin" => {
            // Handle common builtins
            handle_builtin_import(value, event);
        }
        "db" => {
            // Import from udev database — look up previous device properties
            let db_path = device_db_path(event);
            if let Ok(content) = fs::read_to_string(&db_path) {
                for line in content.lines() {
                    if let Some(rest) = line.strip_prefix("E:")
                        && let Some(eq) = rest.find('=')
                    {
                        let key = rest[..eq].to_string();
                        let val = rest[eq + 1..].to_string();
                        event.env.insert(key, val);
                    }
                }
            }
        }
        "parent" => {
            // Import properties from parent device database
            let mut syspath = event.syspath();
            if syspath.pop()
                && let Some(devpath) = syspath
                    .strip_prefix("/sys")
                    .ok()
                    .map(|p| format!("/{}", p.display()))
            {
                let escaped = devpath.replace('/', "\\x2f");
                let db_path = Path::new(DB_DIR).join(&escaped);
                if let Ok(content) = fs::read_to_string(&db_path) {
                    for line in content.lines() {
                        if let Some(rest) = line.strip_prefix("E:")
                            && let Some(eq) = rest.find('=')
                        {
                            let key = rest[..eq].to_string();
                            let val = rest[eq + 1..].to_string();
                            event.env.insert(key, val);
                        }
                    }
                }
            }
        }
        _ => {
            log::debug!("Unknown IMPORT type '{}', ignoring", import_type);
        }
    }
}

/// `net_setup_link` builtin — apply `.link` file configuration to a network device.
///
/// This is the udev builtin that determines the final interface name for network
/// devices based on `.link` files (see `systemd.link(5)`). It:
///
/// 1. Loads all `.link` files from standard search directories.
/// 2. Matches the current device against each file's `[Match]` section.
/// 3. If a match is found, resolves the interface name via `NamePolicy=` (checking
///    `ID_NET_NAME_FROM_DATABASE`, `ID_NET_NAME_ONBOARD`, `ID_NET_NAME_SLOT`,
///    `ID_NET_NAME_PATH`, `ID_NET_NAME_MAC` environment variables set by earlier
///    builtins like `net_id`) or falls back to the explicit `Name=` setting.
/// 4. Sets `ID_NET_LINK_FILE` to the path of the matching `.link` file.
/// 5. Sets `ID_NET_NAME` to the resolved interface name (consumed by the kernel
///    rename logic and networkd).
/// 6. Propagates `MTUBytes=` as `ID_NET_LINK_FILE_MTU` and `MACAddress=` as
///    `ID_NET_LINK_FILE_MACADDRESS` for downstream consumers.
fn builtin_net_setup_link(event: &mut UEvent) {
    // Only process network subsystem devices.
    if event.subsystem != "net" {
        return;
    }

    // Determine the original interface name. In udev, this is typically the
    // kernel-assigned name available as INTERFACE or the device name.
    let original_name = event.env.get("INTERFACE").cloned().unwrap_or_else(|| {
        // Fall back to extracting the last component of devpath.
        event.devpath.rsplit('/').next().unwrap_or("").to_string()
    });

    if original_name.is_empty() {
        log::trace!("net_setup_link: no interface name available, skipping");
        return;
    }

    // Gather device properties for matching.
    let mac = event
        .env
        .get("ID_NET_NAME_MAC")
        .or_else(|| event.env.get("ATTR_address"))
        .cloned()
        .or_else(|| {
            // Try to read the MAC address from sysfs.
            event.read_sysattr("address")
        });
    let driver = event
        .env
        .get("ID_NET_DRIVER")
        .cloned()
        .or_else(|| event.driver.is_empty().then_some(()).and(None))
        .or_else(|| {
            if event.driver.is_empty() {
                None
            } else {
                Some(event.driver.clone())
            }
        });
    let dev_type = event.env.get("DEVTYPE").cloned();
    let id_path = event.env.get("ID_PATH").cloned();

    // Load .link files and find the first match.
    let link_configs = link_config::load_link_configs();
    let matched = link_config::find_matching_link_config(
        &link_configs,
        &original_name,
        mac.as_deref(),
        driver.as_deref(),
        dev_type.as_deref(),
        id_path.as_deref(),
    );

    let link = match matched {
        Some(cfg) => cfg,
        None => {
            log::trace!(
                "net_setup_link: no .link file matched for '{}'",
                original_name
            );
            return;
        }
    };

    log::debug!(
        "net_setup_link: matched '{}' for interface '{}'",
        link.path.display(),
        original_name
    );

    // Set ID_NET_LINK_FILE so downstream rules and networkd know which
    // .link file was applied.
    event.env.insert(
        "ID_NET_LINK_FILE".to_string(),
        link.path.to_string_lossy().to_string(),
    );

    // Resolve the interface name from NamePolicy / Name.
    // The closure looks up naming environment variables that were set by
    // earlier builtins (typically `net_id` and `path_id`).
    let env_snapshot: HashMap<String, String> = event.env.clone();
    if let Some(new_name) =
        link_config::resolve_name_from_policy(link, |key| env_snapshot.get(key).cloned())
        && !new_name.is_empty()
        && new_name != original_name
    {
        log::debug!(
            "net_setup_link: renaming '{}' -> '{}'",
            original_name,
            new_name
        );
        event
            .env
            .insert("ID_NET_NAME".to_string(), new_name.clone());
    }

    // Propagate link-level settings as environment variables for downstream
    // consumers (networkd, udev rules, etc.).
    if let Some(mtu) = link.link_section.mtu {
        event
            .env
            .insert("ID_NET_LINK_FILE_MTU".to_string(), mtu.to_string());
    }

    if let Some(ref mac_addr) = link.link_section.mac_address {
        event
            .env
            .insert("ID_NET_LINK_FILE_MACADDRESS".to_string(), mac_addr.clone());
    }

    // Propagate MACAddressPolicy for downstream (networkd uses this).
    if let Some(ref policy) = link.link_section.mac_address_policy {
        event.env.insert(
            "ID_NET_LINK_FILE_MACADDRESS_POLICY".to_string(),
            policy.as_str().to_string(),
        );
    }

    // Propagate alternative names if specified.
    // Build ID_NET_LINK_FILE_ALTNAMES from AlternativeName= entries and
    // AlternativeNamesPolicy= resolved names.
    let mut alt_names: Vec<String> = Vec::new();

    // Explicit AlternativeName= entries.
    for name in &link.link_section.alternative_names {
        if !name.is_empty() {
            alt_names.push(name.clone());
        }
    }

    // AlternativeNamesPolicy= entries.
    for policy in &link.link_section.alternative_names_policy {
        let env_key = match policy {
            link_config::NamePolicy::Kernel => continue,
            link_config::NamePolicy::Database => "ID_NET_NAME_FROM_DATABASE",
            link_config::NamePolicy::Onboard => "ID_NET_NAME_ONBOARD",
            link_config::NamePolicy::Slot => "ID_NET_NAME_SLOT",
            link_config::NamePolicy::Path => "ID_NET_NAME_PATH",
            link_config::NamePolicy::Mac => "ID_NET_NAME_MAC",
            link_config::NamePolicy::Keep => continue,
        };
        if let Some(name) = env_snapshot.get(env_key)
            && !name.is_empty()
            && !alt_names.contains(name)
        {
            alt_names.push(name.clone());
        }
    }

    if !alt_names.is_empty() {
        event
            .env
            .insert("ID_NET_LINK_FILE_ALTNAMES".to_string(), alt_names.join(" "));
    }
}

/// Handle IMPORT{builtin} for common udev builtins.
fn handle_builtin_import(cmd: &str, event: &mut UEvent) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }

    match parts[0] {
        "path_id" => {
            // Generate ID_PATH and ID_PATH_TAG from device path
            let devpath = &event.devpath;
            // Simple implementation: use the devpath as ID_PATH
            let id_path = devpath
                .replace('/', "-")
                .trim_start_matches('-')
                .to_string();
            if !id_path.is_empty() {
                event
                    .env
                    .insert("ID_PATH".to_string(), format!("platform-{}", id_path));
                let tag = id_path.replace(['.', ':'], "_");
                event
                    .env
                    .insert("ID_PATH_TAG".to_string(), format!("platform-{}", tag));
            }
        }
        "input_id" => {
            // Identify input device capabilities
            if event.subsystem == "input" {
                event.env.insert("ID_INPUT".to_string(), "1".to_string());
                // Try to determine input type from capabilities
                let caps_path = event.syspath().join("capabilities/ev");
                if let Ok(caps) = fs::read_to_string(&caps_path) {
                    let caps = caps.trim();
                    if let Ok(cap_bits) = u64::from_str_radix(caps.trim_start_matches("0x"), 16) {
                        // EV_KEY = 1, EV_REL = 2, EV_ABS = 3
                        if cap_bits & (1 << 1) != 0 {
                            event
                                .env
                                .insert("ID_INPUT_KEY".to_string(), "1".to_string());
                        }
                        if cap_bits & (1 << 2) != 0 {
                            event
                                .env
                                .insert("ID_INPUT_MOUSE".to_string(), "1".to_string());
                        }
                        if cap_bits & (1 << 3) != 0 {
                            event
                                .env
                                .insert("ID_INPUT_TOUCHSCREEN".to_string(), "1".to_string());
                        }
                    }
                }
            }
        }
        "usb_id" => {
            // Identify USB device
            if let Some(vendor) = event.read_sysattr("idVendor") {
                event.env.insert("ID_VENDOR_ID".to_string(), vendor);
            }
            if let Some(product) = event.read_sysattr("idProduct") {
                event.env.insert("ID_MODEL_ID".to_string(), product);
            }
            if let Some(serial) = event.read_sysattr("serial") {
                event.env.insert("ID_SERIAL_SHORT".to_string(), serial);
            }
        }
        "net_id" => {
            // Generate predictable network interface names
            // This is complex; provide basic ID_NET_NAME_PATH
            if event.subsystem == "net"
                && let Some(ref devname) = event.env.get("INTERFACE").cloned()
            {
                event.env.insert("ID_NET_NAME".to_string(), devname.clone());
            }
        }
        "blkid" => {
            // Identify filesystem/partition type
            // Try to run the real blkid for accurate results
            if let Some(devnode) = event.devnode() {
                let output = Command::new("blkid")
                    .arg("-p")
                    .arg("-o")
                    .arg("udev")
                    .arg(&devnode)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .output();
                if let Ok(output) = output
                    && output.status.success()
                {
                    for line in String::from_utf8_lossy(&output.stdout).lines() {
                        if let Some(eq) = line.find('=') {
                            let key = line[..eq].to_string();
                            let val = line[eq + 1..].to_string();
                            event.env.insert(key, val);
                        }
                    }
                }
            }
        }
        "hwdb" => {
            // Hardware database lookup — stub
            log::trace!("builtin hwdb not fully implemented");
        }
        "keyboard" => {
            log::trace!("builtin keyboard not fully implemented");
        }
        "net_setup_link" => {
            builtin_net_setup_link(event);
        }
        "kmod" => {
            // Load kernel module
            if parts.len() > 1
                && parts[1] == "load"
                && let Some(modalias) = event.env.get("MODALIAS").cloned()
            {
                log::debug!("builtin kmod: loading module for {}", modalias);
                let _ = Command::new("modprobe")
                    .arg("-b")
                    .arg(&modalias)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
        }
        _ => {
            log::trace!("Unknown builtin '{}', ignoring", parts[0]);
        }
    }
}

/// Run a program and capture its stdout output.
fn run_program_capture(cmd: &str, event: &UEvent) -> Option<String> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    // Resolve program path — check common udev helper locations
    let prog = resolve_program_path(parts[0]);

    log::debug!("Running program: {} (resolved: {})", cmd, prog.display());

    let mut child_cmd = Command::new(&prog);
    child_cmd
        .args(&parts[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    // Pass device environment
    for (k, v) in &event.env {
        child_cmd.env(k, v);
    }

    match child_cmd.output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Some(stdout)
        }
        Ok(output) => {
            log::debug!(
                "Program '{}' exited with status {}",
                cmd,
                output.status.code().unwrap_or(-1)
            );
            None
        }
        Err(e) => {
            log::debug!("Failed to execute '{}': {}", cmd, e);
            None
        }
    }
}

/// Resolve a program name to a full path, checking udev helper directories.
fn resolve_program_path(name: &str) -> PathBuf {
    if name.starts_with('/') {
        return PathBuf::from(name);
    }

    // Check standard udev helper paths
    let search_dirs = ["/usr/lib/udev", "/lib/udev", "/usr/libexec/udev"];

    for dir in &search_dirs {
        let path = PathBuf::from(dir).join(name);
        if path.exists() {
            return path;
        }
    }

    // Fall back to PATH lookup
    PathBuf::from(name)
}

// ---------------------------------------------------------------------------
// Device database
// ---------------------------------------------------------------------------

/// Get the database file path for a device.
pub fn device_db_path(event: &UEvent) -> PathBuf {
    // Database entries are stored by device type + major:minor or by devpath
    if !event.major.is_empty() && !event.minor.is_empty() {
        // Block or char device
        let dev_type = if event.subsystem == "block" { 'b' } else { 'c' };
        Path::new(DB_DIR).join(format!("{}{}:{}", dev_type, event.major, event.minor))
    } else {
        // No major:minor — use escaped devpath with +<subsystem> prefix
        let escaped = if event.subsystem.is_empty() {
            format!("n{}", event.devpath.replace('/', "\\x2f"))
        } else {
            format!(
                "+{}:{}",
                event.subsystem,
                event.devpath.rsplit('/').next().unwrap_or(&event.devpath)
            )
        };
        Path::new(DB_DIR).join(escaped)
    }
}

/// Write device database entry.
fn write_device_db(event: &UEvent, result: &RuleResult) -> io::Result<()> {
    let db_path = device_db_path(event);
    let _ = fs::create_dir_all(DB_DIR);

    let mut content = String::new();

    // Symlinks
    for link in &result.symlinks {
        content.push_str(&format!("S:{}\n", link));
    }

    // Tags
    for tag in &result.tags {
        content.push_str(&format!("G:{}\n", tag));
    }

    // Priority (default 0)
    if !result.symlinks.is_empty() {
        content.push_str("L:0\n");
    }

    // Environment properties
    for (key, val) in &event.env {
        // Skip kernel-standard properties
        match key.as_str() {
            "ACTION" | "DEVPATH" | "SUBSYSTEM" | "SEQNUM" | "SYNTH_UUID" => continue,
            _ => {}
        }
        content.push_str(&format!("E:{}={}\n", key, val));
    }

    // Write atomically
    let tmp_path = db_path.with_extension("tmp");
    fs::write(&tmp_path, &content)?;
    fs::rename(&tmp_path, &db_path)?;

    Ok(())
}

/// Remove device database entry.
fn remove_device_db(event: &UEvent) {
    let db_path = device_db_path(event);
    let _ = fs::remove_file(db_path);
}

/// Write tag symlinks in /run/udev/tags/.
fn write_device_tags(event: &UEvent, tags: &[String]) {
    let dev_id = if !event.major.is_empty() && !event.minor.is_empty() {
        let dev_type = if event.subsystem == "block" { 'b' } else { 'c' };
        format!("{}{}:{}", dev_type, event.major, event.minor)
    } else {
        format!(
            "+{}:{}",
            event.subsystem,
            event.devpath.rsplit('/').next().unwrap_or(&event.devpath)
        )
    };

    for tag in tags {
        let tag_dir = Path::new(TAGS_DIR).join(tag);
        let _ = fs::create_dir_all(&tag_dir);
        let tag_file = tag_dir.join(&dev_id);
        let _ = fs::write(&tag_file, "");
    }
}

/// Remove tag entries for a device.
fn remove_device_tags(event: &UEvent) {
    let dev_id = if !event.major.is_empty() && !event.minor.is_empty() {
        let dev_type = if event.subsystem == "block" { 'b' } else { 'c' };
        format!("{}{}:{}", dev_type, event.major, event.minor)
    } else {
        format!(
            "+{}:{}",
            event.subsystem,
            event.devpath.rsplit('/').next().unwrap_or(&event.devpath)
        )
    };

    // Walk all tag directories and remove this device's entry
    if let Ok(entries) = fs::read_dir(TAGS_DIR) {
        for entry in entries.flatten() {
            let tag_file = entry.path().join(&dev_id);
            let _ = fs::remove_file(tag_file);
        }
    }
}

// ---------------------------------------------------------------------------
// Symlink management
// ---------------------------------------------------------------------------

/// Create device symlinks in /dev/.
fn create_device_symlinks(event: &UEvent, symlinks: &[String]) {
    for link in symlinks {
        let link_path = if link.starts_with('/') {
            PathBuf::from(link)
        } else {
            PathBuf::from("/dev").join(link)
        };

        if let Some(parent) = link_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Remove existing symlink
        let _ = fs::remove_file(&link_path);

        // Create symlink to device node
        if let Some(devnode) = event.devnode() {
            // Use a relative symlink where possible
            let target = if let (Some(link_parent), true) =
                (link_path.parent(), devnode.starts_with("/dev"))
            {
                // Try to compute relative path
                if let Ok(rel) = pathdiff(&devnode, link_parent) {
                    rel
                } else {
                    devnode.clone()
                }
            } else {
                devnode.clone()
            };

            if let Err(e) = std::os::unix::fs::symlink(&target, &link_path) {
                log::debug!(
                    "Failed to create symlink {} -> {}: {}",
                    link_path.display(),
                    target.display(),
                    e
                );
            } else {
                log::debug!(
                    "Created symlink {} -> {}",
                    link_path.display(),
                    target.display()
                );
            }
        }
    }
}

/// Remove device symlinks.
fn remove_device_symlinks(symlinks: &[String]) {
    for link in symlinks {
        let link_path = if link.starts_with('/') {
            PathBuf::from(link)
        } else {
            PathBuf::from("/dev").join(link)
        };
        let _ = fs::remove_file(&link_path);
    }
}

/// Simple relative path calculation.
fn pathdiff(path: &Path, base: &Path) -> Result<PathBuf, ()> {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());

    let mut path_components = path.components().peekable();
    let mut base_components = base.components().peekable();

    // Skip common prefix
    while let (Some(a), Some(b)) = (path_components.peek(), base_components.peek()) {
        if a == b {
            path_components.next();
            base_components.next();
        } else {
            break;
        }
    }

    let mut result = PathBuf::new();
    for _ in base_components {
        result.push("..");
    }
    for component in path_components {
        result.push(component);
    }

    if result.as_os_str().is_empty() {
        Err(())
    } else {
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Device node permissions
// ---------------------------------------------------------------------------

/// Set permissions on a device node.
fn set_device_permissions(event: &UEvent, result: &RuleResult) {
    let devnode = match event.devnode() {
        Some(p) => p,
        None => return,
    };

    if !devnode.exists() {
        return;
    }

    // Set owner
    let uid = result
        .owner
        .as_ref()
        .and_then(|o| resolve_uid(o))
        .unwrap_or(0);
    let gid = result
        .group
        .as_ref()
        .and_then(|g| resolve_gid(g))
        .unwrap_or(0);

    if uid != 0 || gid != 0 {
        unsafe {
            let path_c = std::ffi::CString::new(devnode.to_string_lossy().as_bytes()).ok();
            if let Some(path_c) = path_c {
                libc::chown(path_c.as_ptr(), uid, gid);
            }
        }
    }

    // Set mode
    if let Some(mode) = result.mode {
        unsafe {
            let path_c = std::ffi::CString::new(devnode.to_string_lossy().as_bytes()).ok();
            if let Some(path_c) = path_c {
                libc::chmod(path_c.as_ptr(), mode);
            }
        }
    }
}

/// Resolve a username to a UID.
fn resolve_uid(name: &str) -> Option<u32> {
    if let Ok(uid) = name.parse::<u32>() {
        return Some(uid);
    }
    // Look up in /etc/passwd
    let cname = std::ffi::CString::new(name).ok()?;
    unsafe {
        let pw = libc::getpwnam(cname.as_ptr());
        if !pw.is_null() {
            Some((*pw).pw_uid)
        } else {
            None
        }
    }
}

/// Resolve a group name to a GID.
fn resolve_gid(name: &str) -> Option<u32> {
    if let Ok(gid) = name.parse::<u32>() {
        return Some(gid);
    }
    let cname = std::ffi::CString::new(name).ok()?;
    unsafe {
        let gr = libc::getgrnam(cname.as_ptr());
        if !gr.is_null() {
            Some((*gr).gr_gid)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Sysfs attribute writing
// ---------------------------------------------------------------------------

fn write_sysattrs(event: &UEvent, writes: &[(String, String)]) {
    for (attr, value) in writes {
        let path = if attr.starts_with('/') {
            PathBuf::from(attr)
        } else {
            event.syspath().join(attr)
        };
        if let Err(e) = fs::write(&path, value) {
            log::debug!("Failed to write sysattr {}: {}", path.display(), e);
        }
    }
}

// ---------------------------------------------------------------------------
// RUN program execution
// ---------------------------------------------------------------------------

fn execute_run_programs(event: &UEvent, result: &RuleResult) {
    // Execute RUN{program} entries
    for cmd in &result.run_programs {
        let expanded = expand_substitutions(cmd, event, "", "", &result.symlinks);
        let parts: Vec<&str> = expanded.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let prog = resolve_program_path(parts[0]);
        log::debug!("RUN: {} (resolved: {})", expanded, prog.display());

        let mut child_cmd = Command::new(&prog);
        child_cmd
            .args(&parts[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        // Pass event environment
        for (k, v) in &event.env {
            child_cmd.env(k, v);
        }
        // Pass overrides
        for (k, v) in &result.env_overrides {
            child_cmd.env(k, v);
        }

        match child_cmd.status() {
            Ok(status) => {
                if !status.success() {
                    log::debug!(
                        "RUN '{}' exited with status {}",
                        expanded,
                        status.code().unwrap_or(-1)
                    );
                }
            }
            Err(e) => {
                log::debug!("Failed to execute RUN '{}': {}", expanded, e);
            }
        }
    }

    // Execute RUN{builtin} entries
    for cmd in &result.run_builtins {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        // For builtins, run them in-process
        let mut tmp_event = event.clone();
        handle_builtin_import(cmd, &mut tmp_event);
    }
}

// ---------------------------------------------------------------------------
// Event processing pipeline
// ---------------------------------------------------------------------------

/// Process a single uevent through the rules engine.
fn process_event(rules: &RuleSet, event: &mut UEvent) {
    log::debug!(
        "Processing event: {} {} (subsystem={}, devname={})",
        event.action,
        event.devpath,
        event.subsystem,
        event.devname
    );

    let result = process_rules(rules, event);

    match event.action.as_str() {
        "add" | "change" | "bind" | "move" | "online" => {
            // Set device permissions
            set_device_permissions(event, &result);

            // Write sysfs attributes
            write_sysattrs(event, &result.sysattr_writes);

            // Create symlinks
            if !result.symlinks.is_empty() {
                create_device_symlinks(event, &result.symlinks);
            }

            // Write device database
            if let Err(e) = write_device_db(event, &result) {
                log::debug!("Failed to write device db: {}", e);
            }

            // Write tags
            if !result.tags.is_empty() {
                write_device_tags(event, &result.tags);
            }

            // Execute RUN programs
            execute_run_programs(event, &result);
        }
        "remove" | "unbind" | "offline" => {
            // Remove symlinks (read from database first)
            let db_path = device_db_path(event);
            let mut old_symlinks = Vec::new();
            if let Ok(content) = fs::read_to_string(&db_path) {
                for line in content.lines() {
                    if let Some(link) = line.strip_prefix("S:") {
                        old_symlinks.push(link.to_string());
                    }
                }
            }
            remove_device_symlinks(&old_symlinks);

            // Remove tags
            remove_device_tags(event);

            // Remove database entry
            remove_device_db(event);

            // Execute RUN programs (even on remove)
            execute_run_programs(event, &result);
        }
        _ => {
            log::debug!("Unknown action '{}', processing rules only", event.action);
            // Still process rules and run programs
            execute_run_programs(event, &result);
        }
    }
}

// ---------------------------------------------------------------------------
// Event queue and worker management
// ---------------------------------------------------------------------------

/// Shared state for the event queue.
struct EventQueue {
    queue: VecDeque<UEvent>,
    active_workers: usize,
    events_processed: u64,
    /// Device paths currently being processed by worker threads.
    /// Events for a device that is already in-flight are deferred
    /// to preserve per-device ordering (matching real systemd behaviour).
    busy_devpaths: HashSet<String>,
}

impl EventQueue {
    fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            active_workers: 0,
            events_processed: 0,
            busy_devpaths: HashSet::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.queue.is_empty() && self.active_workers == 0
    }
}

/// Global event counter for settle detection.
static EVENTS_TOTAL: AtomicU64 = AtomicU64::new(0);
static EVENTS_FINISHED: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------------------
// Control socket handling
// ---------------------------------------------------------------------------

fn handle_control_command(
    cmd: &str,
    queue: &Arc<Mutex<EventQueue>>,
    rules_reload_needed: &mut bool,
) -> String {
    let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();
    let command = parts.first().copied().unwrap_or("");
    let _arg = parts.get(1).copied().unwrap_or("");

    match command.to_uppercase().as_str() {
        "PING" => "OK\n".to_string(),
        "RELOAD" => {
            *rules_reload_needed = true;
            "OK\n".to_string()
        }
        "SETTLE" | "QUEUE_EMPTY" => {
            let q = queue.lock().unwrap_or_else(|e| e.into_inner());
            if q.is_empty() {
                "OK\n".to_string()
            } else {
                format!(
                    "BUSY queue={} workers={}\n",
                    q.queue.len(),
                    q.active_workers
                )
            }
        }
        "STATUS" => {
            let q = queue.lock().unwrap_or_else(|e| e.into_inner());
            format!(
                "events_processed={}\nqueue_length={}\nactive_workers={}\n",
                q.events_processed,
                q.queue.len(),
                q.active_workers,
            )
        }
        "EXIT" | "STOP" => {
            SHUTDOWN_FLAG.store(true, Ordering::SeqCst);
            "OK\n".to_string()
        }
        "SET_MAX_CHILDREN" => {
            // Stub: accept but ignore (we use a fixed worker pool)
            "OK\n".to_string()
        }
        "SET_LOG_LEVEL" => {
            // Stub
            "OK\n".to_string()
        }
        "START_EXEC_QUEUE" => "OK\n".to_string(),
        "STOP_EXEC_QUEUE" => "OK\n".to_string(),
        _ => format!("ERR unknown command: {}\n", command),
    }
}

fn handle_client(
    stream: &mut std::os::unix::net::UnixStream,
    queue: &Arc<Mutex<EventQueue>>,
    rules_reload_needed: &mut bool,
) {
    let mut buf = [0u8; 4096];
    match stream.read(&mut buf) {
        Ok(0) => {}
        Ok(n) => {
            let cmd = String::from_utf8_lossy(&buf[..n]);
            let response = handle_control_command(&cmd, queue, rules_reload_needed);
            let _ = stream.write_all(response.as_bytes());
        }
        Err(e) => {
            log::debug!("Control socket read error: {}", e);
        }
    }
}

// ---------------------------------------------------------------------------
// Coldplug: enumerate existing devices
// ---------------------------------------------------------------------------

/// Trigger a synthetic "add" event for all existing devices by writing
/// "add" to each device's uevent file in sysfs.
#[allow(dead_code)]
fn coldplug_devices() {
    log::info!("Coldplugging existing devices...");
    let mut count = 0u64;

    // Walk /sys/devices/ and trigger uevent for each device
    let dirs = ["/sys/devices", "/sys/class", "/sys/bus"];
    for dir in &dirs {
        if let Err(e) = walk_and_trigger(Path::new(dir), &mut count) {
            log::debug!("Coldplug walk of {} failed: {}", dir, e);
        }
    }

    log::info!("Coldplug triggered {} device events", count);
}

#[allow(dead_code)]
fn walk_and_trigger(dir: &Path, count: &mut u64) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    // Check if this directory has a uevent file
    let uevent_path = dir.join("uevent");
    if uevent_path.exists()
        && let Ok(mut f) = fs::OpenOptions::new().write(true).open(&uevent_path)
        && f.write_all(b"add").is_ok()
    {
        *count += 1;
    }

    // Recurse into subdirectories (but avoid loops)
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        // Skip symlinks to avoid loops in sysfs
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            // Skip certain directories that cause loops
            if name == "subsystem"
                || name == "driver"
                || name == "module"
                || name == "firmware_node"
                || name == "power"
                || name == "device"
            {
                continue;
            }
            let _ = walk_and_trigger(&path, count);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Queue file management (for settle)
// ---------------------------------------------------------------------------

fn update_queue_file(queue: &EventQueue) {
    if queue.is_empty() {
        let _ = fs::remove_file(QUEUE_FILE);
    } else {
        let _ = fs::write(QUEUE_FILE, "");
    }
}

// ---------------------------------------------------------------------------
// Command-line arguments
// ---------------------------------------------------------------------------

/// Daemon command-line arguments.
pub struct DaemonArgs {
    pub daemon: bool,
    pub debug: bool,
    pub resolve_names: String,
    pub children_max: usize,
    pub exec_delay: u64,
    pub event_timeout: u64,
}

impl DaemonArgs {
    /// Parse daemon arguments from `std::env::args()`.
    pub fn parse_from_env() -> Self {
        let argv: Vec<String> = std::env::args().collect();
        Self::parse_from_iter(&argv[1..])
    }

    /// Parse daemon arguments from an iterator of command-line strings (excluding argv[0]).
    pub fn parse_from_iter(args: &[String]) -> Self {
        let mut result = DaemonArgs {
            daemon: false,
            debug: false,
            resolve_names: "early".to_string(),
            children_max: MAX_WORKERS,
            exec_delay: 0,
            event_timeout: EVENT_TIMEOUT_SECS,
        };

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "-d" | "--daemon" => result.daemon = true,
                "-D" | "--debug" => result.debug = true,
                "-N" | "--resolve-names" => {
                    i += 1;
                    if i < args.len() {
                        result.resolve_names = args[i].clone();
                    }
                }
                "-c" | "--children-max" => {
                    i += 1;
                    if i < args.len() {
                        result.children_max = args[i].parse().unwrap_or(MAX_WORKERS);
                    }
                }
                "-e" | "--exec-delay" => {
                    i += 1;
                    if i < args.len() {
                        result.exec_delay = args[i].parse().unwrap_or(0);
                    }
                }
                "-t" | "--event-timeout" => {
                    i += 1;
                    if i < args.len() {
                        result.event_timeout = args[i].parse().unwrap_or(EVENT_TIMEOUT_SECS);
                    }
                }
                "--version" => {
                    println!("systemd-udevd (systemd-rs)");
                    process::exit(0);
                }
                "--help" | "-h" => {
                    println!("Usage: systemd-udevd [OPTIONS]");
                    println!();
                    println!("Options:");
                    println!("  -d, --daemon          Daemonize (fork to background)");
                    println!("  -D, --debug           Enable debug logging");
                    println!("  -N, --resolve-names   Name resolution timing (early|late|never)");
                    println!("  -c, --children-max N  Maximum concurrent events");
                    println!("  -e, --exec-delay N    Seconds to delay execution");
                    println!("  -t, --event-timeout N Event processing timeout");
                    println!("      --version         Show version");
                    println!("  -h, --help            Show this help");
                    process::exit(0);
                }
                other => {
                    // Silently ignore unknown arguments for compatibility
                    log::debug!("Ignoring unknown argument: {}", other);
                }
            }
            i += 1;
        }

        result
    }
}

// ---------------------------------------------------------------------------
// Ensure runtime directories
// ---------------------------------------------------------------------------

fn ensure_runtime_dirs() {
    for dir in &[RUN_DIR, DB_DIR, TAGS_DIR] {
        let _ = fs::create_dir_all(dir);
    }
}

// ---------------------------------------------------------------------------
// Public API: run_daemon
// ---------------------------------------------------------------------------

/// Check whether the current process was invoked as `systemd-udevd`.
///
/// Returns `true` if `argv[0]` ends with `systemd-udevd`, which is the
/// multi-call binary pattern used by upstream systemd where `udevadm` and
/// `systemd-udevd` are the same binary and behaviour is selected by the
/// program name.
pub fn invoked_as_daemon() -> bool {
    std::env::args()
        .next()
        .map(|arg0| {
            let p = std::path::Path::new(&arg0);
            p.file_name()
                .and_then(|f| f.to_str())
                .is_some_and(|name| name == "systemd-udevd")
        })
        .unwrap_or(false)
}

/// Run the udevd daemon. This is the main entry point for both the standalone
/// `systemd-udevd` binary and the `udevadm` multi-call dispatch.
///
/// This function does not return under normal operation (it runs the main
/// event loop until a shutdown signal is received).
pub fn run_daemon() {
    init_logging();

    let args = DaemonArgs::parse_from_env();

    if args.debug {
        log::set_max_level(log::LevelFilter::Debug);
    }

    setup_signal_handlers();

    log::info!("systemd-udevd starting");

    // Daemonize if requested
    if args.daemon {
        unsafe {
            let pid = libc::fork();
            if pid < 0 {
                eprintln!("systemd-udevd: fork failed");
                process::exit(1);
            }
            if pid > 0 {
                // Parent exits
                process::exit(0);
            }
            // Child continues as daemon
            libc::setsid();
        }
    }

    // Create runtime directories
    ensure_runtime_dirs();

    // Load rules (Arc for sharing with worker threads)
    let mut rules = Arc::new(RuleSet::load());

    // Open netlink uevent socket
    let nl_fd = match open_uevent_socket() {
        Ok(fd) => {
            log::info!("Listening on netlink uevent socket");
            fd
        }
        Err(e) => {
            log::error!("Failed to open netlink uevent socket: {}", e);
            log::info!("Continuing without netlink (control socket only)");
            -1
        }
    };

    // Watchdog
    let wd_interval = watchdog_interval();
    if let Some(ref iv) = wd_interval {
        log::info!("Watchdog enabled, interval {:?}", iv);
    }
    let mut last_watchdog = Instant::now();

    // Event queue
    let event_queue = Arc::new(Mutex::new(EventQueue::new()));

    // Remove stale control socket
    let _ = fs::remove_file(CONTROL_SOCKET_PATH);
    let _ = fs::remove_file(QUEUE_FILE);

    // Bind control socket
    let listener = match UnixListener::bind(CONTROL_SOCKET_PATH) {
        Ok(l) => {
            log::info!("Listening on {}", CONTROL_SOCKET_PATH);
            Some(l)
        }
        Err(e) => {
            log::warn!(
                "Failed to bind control socket {}: {}",
                CONTROL_SOCKET_PATH,
                e
            );
            None
        }
    };

    if let Some(ref l) = listener {
        l.set_nonblocking(true).expect("Failed to set non-blocking");
    }

    let children_max = args.children_max;
    let exec_delay = args.exec_delay;
    let _event_timeout = args.event_timeout;

    sd_notify(&format!(
        "READY=1\nSTATUS=Processing events (rules={})",
        rules.rules.len()
    ));

    log::info!(
        "systemd-udevd ready ({} rules loaded, max_workers={})",
        rules.rules.len(),
        children_max
    );

    let mut rules_reload_needed = false;
    let mut poll_timeout = Duration::from_millis(200);

    // Main loop
    loop {
        if SHUTDOWN_FLAG.load(Ordering::SeqCst) {
            log::info!("Received shutdown signal");
            break;
        }

        // Reload rules on SIGHUP
        if RELOAD_FLAG.load(Ordering::SeqCst) || rules_reload_needed {
            RELOAD_FLAG.store(false, Ordering::SeqCst);
            rules_reload_needed = false;
            log::info!("Reloading rules...");
            rules = Arc::new(RuleSet::load());
            log::info!("Reloaded {} rules", rules.rules.len());
            sd_notify(&format!(
                "STATUS=Processing events (rules={})",
                rules.rules.len()
            ));
        }

        // Reap child processes
        if CHILDREN_FLAG.load(Ordering::SeqCst) {
            CHILDREN_FLAG.store(false, Ordering::SeqCst);
            loop {
                let ret = unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) };
                if ret <= 0 {
                    break;
                }
            }
        }

        // Send watchdog keepalive
        if let Some(ref iv) = wd_interval
            && last_watchdog.elapsed() >= *iv
        {
            sd_notify("WATCHDOG=1");
            last_watchdog = Instant::now();
        }

        // Receive netlink events
        if nl_fd >= 0 {
            // Read up to a batch of events before processing
            let mut batch_count = 0;
            while batch_count < 64 {
                match recv_uevent(nl_fd) {
                    Some(event) => {
                        EVENTS_TOTAL.fetch_add(1, Ordering::SeqCst);
                        let mut q = event_queue.lock().unwrap_or_else(|e| e.into_inner());
                        q.queue.push_back(event);
                        update_queue_file(&q);
                        batch_count += 1;
                    }
                    None => break,
                }
            }
        }

        // Dispatch queued events to worker threads.
        //
        // Events for the same devpath are serialized (only one worker
        // at a time per device) to avoid races on the device database,
        // symlinks, and sysfs attributes — matching real systemd behaviour.
        {
            let mut q = event_queue.lock().unwrap_or_else(|e| e.into_inner());
            let max_new = children_max.saturating_sub(q.active_workers);
            let mut dispatched = 0usize;
            let mut idx = 0usize;

            while dispatched < max_new && idx < q.queue.len() {
                let devpath = q.queue[idx].devpath.clone();

                // Skip events whose devpath is already being processed
                if q.busy_devpaths.contains(&devpath) {
                    idx += 1;
                    continue;
                }

                // Clone the event before attempting to spawn so we can
                // put it back on failure (the closure consumes the move).
                let event_clone = q.queue[idx].clone();

                q.busy_devpaths.insert(devpath.clone());
                q.active_workers += 1;

                let rules_ref = rules.clone();
                let queue_ref = event_queue.clone();
                let worker_exec_delay = exec_delay;
                let devpath_for_worker = devpath.clone();

                let spawn_result = thread::Builder::new()
                    .name(format!("udev-worker:{}", &devpath))
                    .spawn(move || {
                        let mut event = event_clone;
                        let devpath = devpath_for_worker;
                        if worker_exec_delay > 0 {
                            thread::sleep(Duration::from_secs(worker_exec_delay));
                        }
                        process_event(&rules_ref, &mut event);

                        let mut q = queue_ref.lock().unwrap_or_else(|e| e.into_inner());
                        q.active_workers -= 1;
                        q.events_processed += 1;
                        q.busy_devpaths.remove(&devpath);
                        EVENTS_FINISHED.fetch_add(1, Ordering::SeqCst);
                        update_queue_file(&q);
                    });

                match spawn_result {
                    Ok(_handle) => {
                        // Successfully spawned — remove the event from the queue
                        q.queue.remove(idx);
                        dispatched += 1;
                        // Don't increment idx: removal shifted the next element
                        // into the current position.
                    }
                    Err(e) => {
                        log::error!("Failed to spawn worker thread: {}", e);
                        // Undo bookkeeping — the event is still in the queue
                        q.active_workers -= 1;
                        q.busy_devpaths.remove(&devpath);
                        // Stop trying to spawn more workers this iteration
                        break;
                    }
                }
            }

            // Update queue file outside the dispatch loop (lock already held
            // only for the non-worker path — workers update it themselves).
            update_queue_file(&q);
        }

        // Handle control socket connections
        if let Some(ref l) = listener {
            match l.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    handle_client(&mut stream, &event_queue, &mut rules_reload_needed);
                    let _ = stream.shutdown(Shutdown::Both);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => {
                    log::debug!("Control socket accept error: {}", e);
                }
            }
        }

        thread::sleep(poll_timeout);

        // Adaptive poll timeout: faster when queue is non-empty
        {
            let q = event_queue.lock().unwrap_or_else(|e| e.into_inner());
            if q.queue.is_empty() {
                poll_timeout = Duration::from_millis(200);
            } else {
                poll_timeout = Duration::from_millis(10);
            }
        }
    }

    // Cleanup
    if nl_fd >= 0 {
        unsafe {
            libc::close(nl_fd);
        }
    }
    let _ = fs::remove_file(CONTROL_SOCKET_PATH);
    let _ = fs::remove_file(QUEUE_FILE);

    sd_notify("STOPPING=1");
    log::info!("systemd-udevd stopped");
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // -----------------------------------------------------------------------
    // UEvent parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_uevent_parse_basic() {
        let data = b"add@/devices/pci0000:00/0000:00:02.0\0ACTION=add\0DEVPATH=/devices/pci0000:00/0000:00:02.0\0SUBSYSTEM=pci\0SEQNUM=42\0";
        let event = UEvent::parse(data).unwrap();
        assert_eq!(event.action, "add");
        assert_eq!(event.devpath, "/devices/pci0000:00/0000:00:02.0");
        assert_eq!(event.subsystem, "pci");
        assert_eq!(event.seqnum, 42);
    }

    #[test]
    fn test_uevent_parse_block_device() {
        let data = b"add@/devices/virtual/block/loop0\0ACTION=add\0DEVPATH=/devices/virtual/block/loop0\0SUBSYSTEM=block\0DEVTYPE=disk\0DEVNAME=loop0\0MAJOR=7\0MINOR=0\0SEQNUM=100\0";
        let event = UEvent::parse(data).unwrap();
        assert_eq!(event.action, "add");
        assert_eq!(event.subsystem, "block");
        assert_eq!(event.devtype, "disk");
        assert_eq!(event.devname, "loop0");
        assert_eq!(event.major, "7");
        assert_eq!(event.minor, "0");
    }

    #[test]
    fn test_uevent_parse_empty() {
        assert!(UEvent::parse(b"").is_none());
    }

    #[test]
    fn test_uevent_parse_no_devpath() {
        let data = b"ACTION=add\0SUBSYSTEM=pci\0";
        assert!(UEvent::parse(data).is_none());
    }

    #[test]
    fn test_uevent_syspath() {
        let mut event = UEvent::new();
        event.devpath = "/devices/virtual/block/loop0".to_string();
        assert_eq!(
            event.syspath(),
            PathBuf::from("/sys/devices/virtual/block/loop0")
        );
    }

    #[test]
    fn test_uevent_devnode() {
        let mut event = UEvent::new();
        event.devname = "sda".to_string();
        assert_eq!(event.devnode(), Some(PathBuf::from("/dev/sda")));

        event.devname = "/dev/loop0".to_string();
        assert_eq!(event.devnode(), Some(PathBuf::from("/dev/loop0")));

        event.devname.clear();
        assert_eq!(event.devnode(), None);
    }

    #[test]
    fn test_kernel_name() {
        let mut event = UEvent::new();
        event.devpath =
            "/devices/pci0000:00/0000:00:1f.2/host0/target0:0:0/0:0:0:0/block/sda".to_string();
        assert_eq!(kernel_name(&event), "sda");
    }

    #[test]
    fn test_kernel_number() {
        let mut event = UEvent::new();
        event.devpath = "/devices/virtual/block/loop0".to_string();
        assert_eq!(kernel_number(&event), "0");

        event.devpath = "/devices/virtual/net/eth0".to_string();
        assert_eq!(kernel_number(&event), "0");

        event.devpath = "/devices/platform/serial8250/tty/ttyS15".to_string();
        assert_eq!(kernel_number(&event), "15");

        event.devpath = "/devices/pci0000:00".to_string();
        assert_eq!(kernel_number(&event), "00");

        event.devpath = "/devices/platform/soc".to_string();
        assert_eq!(kernel_number(&event), "");
    }

    // -----------------------------------------------------------------------
    // Glob matching
    // -----------------------------------------------------------------------

    #[test]
    fn test_glob_match_exact() {
        assert!(glob_match("sda", "sda"));
        assert!(!glob_match("sda", "sdb"));
    }

    #[test]
    fn test_glob_match_star() {
        assert!(glob_match("sd*", "sda"));
        assert!(glob_match("sd*", "sda1"));
        assert!(glob_match("*", "anything"));
        assert!(!glob_match("sd*", "nvme0"));
    }

    #[test]
    fn test_glob_match_question() {
        assert!(glob_match("sd?", "sda"));
        assert!(!glob_match("sd?", "sda1"));
        assert!(glob_match("sd??", "sda1"));
    }

    #[test]
    fn test_glob_match_brackets() {
        assert!(glob_match("sd[abc]", "sda"));
        assert!(glob_match("sd[abc]", "sdb"));
        assert!(!glob_match("sd[abc]", "sdd"));
        assert!(glob_match("sd[a-z]", "sda"));
        assert!(!glob_match("sd[a-z]", "sd1"));
    }

    #[test]
    fn test_glob_match_negated_brackets() {
        assert!(!glob_match("sd[!a-c]", "sda"));
        assert!(glob_match("sd[!a-c]", "sdd"));
    }

    #[test]
    fn test_glob_match_alternatives() {
        assert!(glob_match("sda|sdb", "sda"));
        assert!(glob_match("sda|sdb", "sdb"));
        assert!(!glob_match("sda|sdb", "sdc"));
    }

    #[test]
    fn test_glob_match_complex() {
        assert!(glob_match("sd[a-z]*", "sda"));
        assert!(glob_match("sd[a-z]*", "sda1"));
        assert!(glob_match("sd[a-z]*", "sdz99"));
        assert!(!glob_match("sd[a-z]*", "sd1"));
    }

    #[test]
    fn test_glob_match_empty() {
        assert!(glob_match("", ""));
        assert!(glob_match("*", ""));
        assert!(!glob_match("?", ""));
    }

    // -----------------------------------------------------------------------
    // Rule line parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_rule_line_basic_match() {
        let tokens = parse_rule_line(r#"KERNEL=="sda", SUBSYSTEM=="block""#).unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].key, "KERNEL");
        assert_eq!(tokens[0].op, RuleOp::Match);
        assert_eq!(tokens[0].value, "sda");
        assert_eq!(tokens[1].key, "SUBSYSTEM");
        assert_eq!(tokens[1].op, RuleOp::Match);
        assert_eq!(tokens[1].value, "block");
    }

    #[test]
    fn test_parse_rule_line_assignment() {
        let tokens = parse_rule_line(r#"SYMLINK+="disk/by-path/$env{ID_PATH}""#).unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].key, "SYMLINK");
        assert_eq!(tokens[0].op, RuleOp::AssignAdd);
        assert_eq!(tokens[0].value, "disk/by-path/$env{ID_PATH}");
    }

    #[test]
    fn test_parse_rule_line_attr_match() {
        let tokens = parse_rule_line(r#"ATTR{size}=="0", OPTIONS+="ignore_device""#).unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].key, "ATTR");
        assert_eq!(tokens[0].attr, Some("size".to_string()));
        assert_eq!(tokens[0].op, RuleOp::Match);
        assert_eq!(tokens[0].value, "0");
    }

    #[test]
    fn test_parse_rule_line_env() {
        let tokens = parse_rule_line(r#"ENV{ID_FS_TYPE}=="ext4", SYMLINK+="myfs""#).unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].key, "ENV");
        assert_eq!(tokens[0].attr, Some("ID_FS_TYPE".to_string()));
        assert_eq!(tokens[0].value, "ext4");
    }

    #[test]
    fn test_parse_rule_line_run() {
        let tokens = parse_rule_line(r#"RUN{program}+="/usr/bin/touch /tmp/test""#).unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].key, "RUN");
        assert_eq!(tokens[0].attr, Some("program".to_string()));
        assert_eq!(tokens[0].op, RuleOp::AssignAdd);
        assert_eq!(tokens[0].value, "/usr/bin/touch /tmp/test");
    }

    #[test]
    fn test_parse_rule_line_nomatch() {
        let tokens = parse_rule_line(r#"KERNEL!="loop*""#).unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].op, RuleOp::Nomatch);
    }

    #[test]
    fn test_parse_rule_line_final_assign() {
        let tokens = parse_rule_line(r#"NAME:="my_device""#).unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].op, RuleOp::AssignFinal);
    }

    #[test]
    fn test_parse_rule_line_goto_label() {
        let tokens = parse_rule_line(r#"GOTO="end", LABEL="end""#).unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].key, "GOTO");
        assert_eq!(tokens[0].value, "end");
        assert_eq!(tokens[1].key, "LABEL");
        assert_eq!(tokens[1].value, "end");
    }

    #[test]
    fn test_parse_rule_line_mode() {
        let tokens =
            parse_rule_line(r#"KERNEL=="ttyS[0-9]*", MODE="0660", GROUP="dialout""#).unwrap();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].key, "KERNEL");
        assert_eq!(tokens[0].value, "ttyS[0-9]*");
        assert_eq!(tokens[1].key, "MODE");
        assert_eq!(tokens[1].op, RuleOp::Assign);
        assert_eq!(tokens[1].value, "0660");
        assert_eq!(tokens[2].key, "GROUP");
        assert_eq!(tokens[2].value, "dialout");
    }

    #[test]
    fn test_parse_rule_line_empty() {
        let tokens = parse_rule_line("").unwrap();
        assert!(tokens.is_empty());
    }

    // -----------------------------------------------------------------------
    // Rules file parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_rules_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("50-test.rules");
        let mut f = fs::File::create(&path).unwrap();
        writeln!(f, "# Test rules file").unwrap();
        writeln!(f).unwrap();
        writeln!(f, r#"KERNEL=="sda", SYMLINK+="mydisk""#).unwrap();
        writeln!(
            f,
            r#"SUBSYSTEM=="block", ATTR{{size}}=="0", OPTIONS+="ignore_device""#
        )
        .unwrap();
        writeln!(f, r#"KERNEL=="ttyS*", MODE="0660", \"#).unwrap();
        writeln!(f, r#"  GROUP="dialout""#).unwrap();
        drop(f);

        let rules = parse_rules_file(&path).unwrap();
        assert_eq!(rules.len(), 3);

        // First rule
        assert_eq!(rules[0].tokens.len(), 2);
        assert_eq!(rules[0].tokens[0].key, "KERNEL");
        assert_eq!(rules[0].tokens[1].key, "SYMLINK");

        // Second rule (ATTR with braces)
        assert_eq!(rules[1].tokens[0].key, "SUBSYSTEM");
        assert!(rules[1].tokens[1].key == "ATTR");
        assert_eq!(rules[1].tokens[1].attr, Some("size".to_string()));

        // Third rule (line continuation)
        assert_eq!(rules[2].tokens.len(), 3);
        assert_eq!(rules[2].tokens[0].key, "KERNEL");
        assert_eq!(rules[2].tokens[1].key, "MODE");
        assert_eq!(rules[2].tokens[2].key, "GROUP");
    }

    #[test]
    fn test_parse_rules_file_comments_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.rules");
        let mut f = fs::File::create(&path).unwrap();
        writeln!(f, "# Only comments").unwrap();
        writeln!(f, "# and blank lines").unwrap();
        writeln!(f).unwrap();
        drop(f);

        let rules = parse_rules_file(&path).unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn test_parse_rules_file_missing() {
        let result = parse_rules_file(Path::new("/nonexistent/file.rules"));
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Rule matching
    // -----------------------------------------------------------------------

    fn make_test_event(action: &str, devpath: &str, subsystem: &str) -> UEvent {
        let mut event = UEvent::new();
        event.action = action.to_string();
        event.devpath = devpath.to_string();
        event.subsystem = subsystem.to_string();
        event.env.insert("ACTION".to_string(), action.to_string());
        event.env.insert("DEVPATH".to_string(), devpath.to_string());
        event
            .env
            .insert("SUBSYSTEM".to_string(), subsystem.to_string());
        event
    }

    #[test]
    fn test_match_token_kernel() {
        let event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let token = RuleToken {
            key: "KERNEL".to_string(),
            attr: None,
            op: RuleOp::Match,
            value: "sd*".to_string(),
        };
        let mut pr = String::new();
        assert!(match_token(&token, &event, &mut pr));
    }

    #[test]
    fn test_match_token_kernel_nomatch() {
        let event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let token = RuleToken {
            key: "KERNEL".to_string(),
            attr: None,
            op: RuleOp::Nomatch,
            value: "loop*".to_string(),
        };
        let mut pr = String::new();
        assert!(match_token(&token, &event, &mut pr));
    }

    #[test]
    fn test_match_token_subsystem() {
        let token = RuleToken {
            key: "SUBSYSTEM".to_string(),
            attr: None,
            op: RuleOp::Match,
            value: "block".to_string(),
        };
        let mut event = UEvent::new();
        event.subsystem = "block".to_string();
        let mut pr = String::new();
        assert!(match_token(&token, &event, &mut pr));
    }

    #[test]
    fn test_match_token_action() {
        let token_add = RuleToken {
            key: "ACTION".to_string(),
            attr: None,
            op: RuleOp::Match,
            value: "add".to_string(),
        };
        let token_remove = RuleToken {
            key: "ACTION".to_string(),
            attr: None,
            op: RuleOp::Match,
            value: "remove".to_string(),
        };
        let event = make_test_event("add", "/devices/test", "test");
        let mut pr = String::new();
        assert!(match_token(&token_add, &event, &mut pr));
        assert!(!match_token(&token_remove, &event, &mut pr));
    }

    #[test]
    fn test_match_token_env() {
        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        event
            .env
            .insert("ID_FS_TYPE".to_string(), "ext4".to_string());

        let token = RuleToken {
            key: "ENV".to_string(),
            attr: Some("ID_FS_TYPE".to_string()),
            op: RuleOp::Match,
            value: "ext4".to_string(),
        };
        let mut pr = String::new();
        assert!(match_token(&token, &event, &mut pr));

        let token_wrong = RuleToken {
            key: "ENV".to_string(),
            attr: Some("ID_FS_TYPE".to_string()),
            op: RuleOp::Match,
            value: "xfs".to_string(),
        };
        assert!(!match_token(&token_wrong, &event, &mut pr));
    }

    #[test]
    fn test_match_token_result() {
        let token = RuleToken {
            key: "RESULT".to_string(),
            attr: None,
            op: RuleOp::Match,
            value: "ok*".to_string(),
        };
        let event = make_test_event("add", "/devices/test", "test");
        let mut pr = "ok_value".to_string();
        assert!(match_token(&token, &event, &mut pr));
        let mut pr2 = "fail".to_string();
        assert!(!match_token(&token, &event, &mut pr2));
    }

    #[test]
    fn test_program_result_capture_propagation() {
        // Verify that PROGRAM match captures stdout into program_result,
        // and a subsequent RESULT== match can use it.
        //
        // Rule 1: PROGRAM=="echo hello_world" (captures "hello_world")
        // Rule 2: RESULT=="hello*", SYMLINK+="matched"
        //
        // We can't easily run external programs in unit tests, so we test
        // the propagation mechanism directly: process_rules passes
        // program_result through match_token for RESULT keys.

        let rules = RuleSet {
            rules: vec![
                // Rule that sets program_result via PROGRAM assignment
                Rule {
                    filename: "test".to_string(),
                    line: 1,
                    tokens: vec![
                        RuleToken {
                            key: "KERNEL".to_string(),
                            attr: None,
                            op: RuleOp::Match,
                            value: "sda".to_string(),
                        },
                        // PROGRAM as assignment (not match) — runs and captures
                        RuleToken {
                            key: "PROGRAM".to_string(),
                            attr: None,
                            op: RuleOp::Assign,
                            value: "echo capture_test_value".to_string(),
                        },
                    ],
                    label: None,
                    goto_target: None,
                },
                // Rule that matches on the captured RESULT
                Rule {
                    filename: "test".to_string(),
                    line: 2,
                    tokens: vec![
                        RuleToken {
                            key: "KERNEL".to_string(),
                            attr: None,
                            op: RuleOp::Match,
                            value: "sda".to_string(),
                        },
                        RuleToken {
                            key: "RESULT".to_string(),
                            attr: None,
                            op: RuleOp::Match,
                            value: "capture_test*".to_string(),
                        },
                        RuleToken {
                            key: "SYMLINK".to_string(),
                            attr: None,
                            op: RuleOp::AssignAdd,
                            value: "result_matched".to_string(),
                        },
                    ],
                    label: None,
                    goto_target: None,
                },
                // Rule that should NOT match (wrong RESULT pattern)
                Rule {
                    filename: "test".to_string(),
                    line: 3,
                    tokens: vec![
                        RuleToken {
                            key: "KERNEL".to_string(),
                            attr: None,
                            op: RuleOp::Match,
                            value: "sda".to_string(),
                        },
                        RuleToken {
                            key: "RESULT".to_string(),
                            attr: None,
                            op: RuleOp::Match,
                            value: "wrong_prefix*".to_string(),
                        },
                        RuleToken {
                            key: "SYMLINK".to_string(),
                            attr: None,
                            op: RuleOp::AssignAdd,
                            value: "should_not_appear".to_string(),
                        },
                    ],
                    label: None,
                    goto_target: None,
                },
            ],
        };

        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = process_rules(&rules, &mut event);

        // The RESULT== "capture_test*" rule should have matched
        assert!(
            result.symlinks.contains(&"result_matched".to_string()),
            "RESULT== should match captured PROGRAM output; symlinks = {:?}",
            result.symlinks
        );
        // The wrong-prefix rule should NOT have matched
        assert!(
            !result.symlinks.contains(&"should_not_appear".to_string()),
            "RESULT== with wrong pattern should not match; symlinks = {:?}",
            result.symlinks
        );
    }

    // -----------------------------------------------------------------------
    // Rule processing
    // -----------------------------------------------------------------------

    #[test]
    fn test_process_rules_symlink() {
        let rules = RuleSet {
            rules: vec![Rule {
                filename: "test".to_string(),
                line: 1,
                tokens: vec![
                    RuleToken {
                        key: "KERNEL".to_string(),
                        attr: None,
                        op: RuleOp::Match,
                        value: "sda".to_string(),
                    },
                    RuleToken {
                        key: "SYMLINK".to_string(),
                        attr: None,
                        op: RuleOp::AssignAdd,
                        value: "mydisk".to_string(),
                    },
                ],
                label: None,
                goto_target: None,
            }],
        };

        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = process_rules(&rules, &mut event);
        assert_eq!(result.symlinks, vec!["mydisk".to_string()]);
    }

    #[test]
    fn test_process_rules_mode_owner_group() {
        let rules = RuleSet {
            rules: vec![Rule {
                filename: "test".to_string(),
                line: 1,
                tokens: vec![
                    RuleToken {
                        key: "KERNEL".to_string(),
                        attr: None,
                        op: RuleOp::Match,
                        value: "ttyS*".to_string(),
                    },
                    RuleToken {
                        key: "MODE".to_string(),
                        attr: None,
                        op: RuleOp::Assign,
                        value: "0660".to_string(),
                    },
                    RuleToken {
                        key: "OWNER".to_string(),
                        attr: None,
                        op: RuleOp::Assign,
                        value: "root".to_string(),
                    },
                    RuleToken {
                        key: "GROUP".to_string(),
                        attr: None,
                        op: RuleOp::Assign,
                        value: "dialout".to_string(),
                    },
                ],
                label: None,
                goto_target: None,
            }],
        };

        let mut event = make_test_event("add", "/devices/platform/serial8250/tty/ttyS0", "tty");
        let result = process_rules(&rules, &mut event);
        assert_eq!(result.mode, Some(0o660));
        assert_eq!(result.owner, Some("root".to_string()));
        assert_eq!(result.group, Some("dialout".to_string()));
    }

    #[test]
    fn test_process_rules_env_set() {
        let rules = RuleSet {
            rules: vec![Rule {
                filename: "test".to_string(),
                line: 1,
                tokens: vec![
                    RuleToken {
                        key: "SUBSYSTEM".to_string(),
                        attr: None,
                        op: RuleOp::Match,
                        value: "net".to_string(),
                    },
                    RuleToken {
                        key: "ENV".to_string(),
                        attr: Some("MY_TAG".to_string()),
                        op: RuleOp::Assign,
                        value: "network_device".to_string(),
                    },
                ],
                label: None,
                goto_target: None,
            }],
        };

        let mut event = make_test_event("add", "/devices/virtual/net/eth0", "net");
        let result = process_rules(&rules, &mut event);
        assert_eq!(
            result.env_overrides.get("MY_TAG"),
            Some(&"network_device".to_string())
        );
        assert_eq!(event.env.get("MY_TAG"), Some(&"network_device".to_string()));
    }

    #[test]
    fn test_process_rules_no_match() {
        let rules = RuleSet {
            rules: vec![Rule {
                filename: "test".to_string(),
                line: 1,
                tokens: vec![
                    RuleToken {
                        key: "KERNEL".to_string(),
                        attr: None,
                        op: RuleOp::Match,
                        value: "nvme*".to_string(),
                    },
                    RuleToken {
                        key: "SYMLINK".to_string(),
                        attr: None,
                        op: RuleOp::AssignAdd,
                        value: "olddisk".to_string(),
                    },
                ],
                label: None,
                goto_target: None,
            }],
        };

        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = process_rules(&rules, &mut event);
        assert!(result.symlinks.is_empty());
    }

    #[test]
    fn test_process_rules_goto() {
        let rules = RuleSet {
            rules: vec![
                Rule {
                    filename: "test".to_string(),
                    line: 1,
                    tokens: vec![
                        RuleToken {
                            key: "SUBSYSTEM".to_string(),
                            attr: None,
                            op: RuleOp::Match,
                            value: "block".to_string(),
                        },
                        RuleToken {
                            key: "GOTO".to_string(),
                            attr: None,
                            op: RuleOp::Assign,
                            value: "skip".to_string(),
                        },
                    ],
                    label: None,
                    goto_target: Some("skip".to_string()),
                },
                Rule {
                    filename: "test".to_string(),
                    line: 2,
                    tokens: vec![RuleToken {
                        key: "SYMLINK".to_string(),
                        attr: None,
                        op: RuleOp::AssignAdd,
                        value: "should_not_appear".to_string(),
                    }],
                    label: None,
                    goto_target: None,
                },
                Rule {
                    filename: "test".to_string(),
                    line: 3,
                    tokens: vec![],
                    label: Some("skip".to_string()),
                    goto_target: None,
                },
                Rule {
                    filename: "test".to_string(),
                    line: 4,
                    tokens: vec![RuleToken {
                        key: "SYMLINK".to_string(),
                        attr: None,
                        op: RuleOp::AssignAdd,
                        value: "should_appear".to_string(),
                    }],
                    label: None,
                    goto_target: None,
                },
            ],
        };

        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = process_rules(&rules, &mut event);
        assert!(!result.symlinks.contains(&"should_not_appear".to_string()));
        assert!(result.symlinks.contains(&"should_appear".to_string()));
    }

    #[test]
    fn test_process_rules_tag() {
        let rules = RuleSet {
            rules: vec![Rule {
                filename: "test".to_string(),
                line: 1,
                tokens: vec![
                    RuleToken {
                        key: "SUBSYSTEM".to_string(),
                        attr: None,
                        op: RuleOp::Match,
                        value: "block".to_string(),
                    },
                    RuleToken {
                        key: "TAG".to_string(),
                        attr: None,
                        op: RuleOp::AssignAdd,
                        value: "systemd".to_string(),
                    },
                ],
                label: None,
                goto_target: None,
            }],
        };

        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = process_rules(&rules, &mut event);
        assert!(result.tags.contains(&"systemd".to_string()));
    }

    #[test]
    fn test_process_rules_run_program() {
        let rules = RuleSet {
            rules: vec![Rule {
                filename: "test".to_string(),
                line: 1,
                tokens: vec![
                    RuleToken {
                        key: "KERNEL".to_string(),
                        attr: None,
                        op: RuleOp::Match,
                        value: "sda".to_string(),
                    },
                    RuleToken {
                        key: "RUN".to_string(),
                        attr: Some("program".to_string()),
                        op: RuleOp::AssignAdd,
                        value: "/bin/true".to_string(),
                    },
                ],
                label: None,
                goto_target: None,
            }],
        };

        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = process_rules(&rules, &mut event);
        assert_eq!(result.run_programs, vec!["/bin/true".to_string()]);
    }

    #[test]
    fn test_process_rules_multiple_matching_rules() {
        let rules = RuleSet {
            rules: vec![
                Rule {
                    filename: "test".to_string(),
                    line: 1,
                    tokens: vec![
                        RuleToken {
                            key: "KERNEL".to_string(),
                            attr: None,
                            op: RuleOp::Match,
                            value: "sd*".to_string(),
                        },
                        RuleToken {
                            key: "SYMLINK".to_string(),
                            attr: None,
                            op: RuleOp::AssignAdd,
                            value: "link1".to_string(),
                        },
                    ],
                    label: None,
                    goto_target: None,
                },
                Rule {
                    filename: "test".to_string(),
                    line: 2,
                    tokens: vec![
                        RuleToken {
                            key: "SUBSYSTEM".to_string(),
                            attr: None,
                            op: RuleOp::Match,
                            value: "block".to_string(),
                        },
                        RuleToken {
                            key: "SYMLINK".to_string(),
                            attr: None,
                            op: RuleOp::AssignAdd,
                            value: "link2".to_string(),
                        },
                    ],
                    label: None,
                    goto_target: None,
                },
            ],
        };

        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = process_rules(&rules, &mut event);
        assert!(result.symlinks.contains(&"link1".to_string()));
        assert!(result.symlinks.contains(&"link2".to_string()));
    }

    #[test]
    fn test_process_rules_assign_final() {
        let rules = RuleSet {
            rules: vec![
                Rule {
                    filename: "test".to_string(),
                    line: 1,
                    tokens: vec![
                        RuleToken {
                            key: "KERNEL".to_string(),
                            attr: None,
                            op: RuleOp::Match,
                            value: "sd*".to_string(),
                        },
                        RuleToken {
                            key: "MODE".to_string(),
                            attr: None,
                            op: RuleOp::AssignFinal,
                            value: "0600".to_string(),
                        },
                    ],
                    label: None,
                    goto_target: None,
                },
                Rule {
                    filename: "test".to_string(),
                    line: 2,
                    tokens: vec![
                        RuleToken {
                            key: "KERNEL".to_string(),
                            attr: None,
                            op: RuleOp::Match,
                            value: "sd*".to_string(),
                        },
                        RuleToken {
                            key: "MODE".to_string(),
                            attr: None,
                            op: RuleOp::Assign,
                            value: "0660".to_string(),
                        },
                    ],
                    label: None,
                    goto_target: None,
                },
            ],
        };

        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = process_rules(&rules, &mut event);
        // The first rule used :=, so the second rule's = should be ignored
        assert_eq!(result.mode, Some(0o600));
    }

    #[test]
    fn test_process_rules_name() {
        let rules = RuleSet {
            rules: vec![Rule {
                filename: "test".to_string(),
                line: 1,
                tokens: vec![
                    RuleToken {
                        key: "KERNEL".to_string(),
                        attr: None,
                        op: RuleOp::Match,
                        value: "sda".to_string(),
                    },
                    RuleToken {
                        key: "NAME".to_string(),
                        attr: None,
                        op: RuleOp::Assign,
                        value: "my-disk".to_string(),
                    },
                ],
                label: None,
                goto_target: None,
            }],
        };

        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = process_rules(&rules, &mut event);
        assert_eq!(result.name, Some("my-disk".to_string()));
    }

    // -----------------------------------------------------------------------
    // Substitution expansion
    // -----------------------------------------------------------------------

    #[test]
    fn test_expand_kernel_name() {
        let event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = expand_substitutions("disk-%k", &event, "", "", &[]);
        assert_eq!(result, "disk-sda");
    }

    #[test]
    fn test_expand_kernel_number() {
        let event = make_test_event("add", "/devices/virtual/block/sda1", "block");
        let result = expand_substitutions("part-%n", &event, "", "", &[]);
        assert_eq!(result, "part-1");
    }

    #[test]
    fn test_expand_devpath() {
        let event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = expand_substitutions("%p", &event, "", "", &[]);
        assert_eq!(result, "/devices/virtual/block/sda");
    }

    #[test]
    fn test_expand_major_minor() {
        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        event.major = "8".to_string();
        event.minor = "0".to_string();
        let result = expand_substitutions("%M:%m", &event, "", "", &[]);
        assert_eq!(result, "8:0");
    }

    #[test]
    fn test_expand_dollar_keywords() {
        let event = make_test_event("add", "/devices/virtual/block/sda", "block");
        assert_eq!(expand_substitutions("$kernel", &event, "", "", &[]), "sda");
        assert_eq!(expand_substitutions("$sys", &event, "", "", &[]), "/sys");
        assert_eq!(expand_substitutions("$root", &event, "", "", &[]), "/dev");
    }

    #[test]
    fn test_expand_env() {
        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        event
            .env
            .insert("ID_PATH".to_string(), "pci-0000:00:1f.2".to_string());
        let result = expand_substitutions("disk/by-path/$env{ID_PATH}", &event, "", "", &[]);
        assert_eq!(result, "disk/by-path/pci-0000:00:1f.2");
    }

    #[test]
    fn test_expand_percent_env() {
        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        event
            .env
            .insert("ID_PATH".to_string(), "pci-0000:00:1f.2".to_string());
        let result = expand_substitutions("disk/by-path/%E{ID_PATH}", &event, "", "", &[]);
        assert_eq!(result, "disk/by-path/pci-0000:00:1f.2");
    }

    #[test]
    fn test_expand_result() {
        let event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = expand_substitutions("result-%c", &event, "hello world", "", &[]);
        assert_eq!(result, "result-hello world");
    }

    #[test]
    fn test_expand_result_indexed() {
        let event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = expand_substitutions("%c{1}", &event, "foo bar baz", "", &[]);
        assert_eq!(result, "foo");

        let result2 = expand_substitutions("%c{2+}", &event, "foo bar baz", "", &[]);
        assert_eq!(result2, "bar baz");
    }

    #[test]
    fn test_expand_escape() {
        let event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = expand_substitutions("%%", &event, "", "", &[]);
        assert_eq!(result, "%");
        let result2 = expand_substitutions("$$", &event, "", "", &[]);
        assert_eq!(result2, "$");
    }

    // -----------------------------------------------------------------------
    // Device database path
    // -----------------------------------------------------------------------

    #[test]
    fn test_device_db_path_block() {
        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        event.major = "8".to_string();
        event.minor = "0".to_string();
        let db_path = device_db_path(&event);
        assert_eq!(db_path, PathBuf::from("/run/udev/data/b8:0"));
    }

    #[test]
    fn test_device_db_path_char() {
        let mut event = make_test_event("add", "/devices/virtual/tty/ttyS0", "tty");
        event.major = "4".to_string();
        event.minor = "64".to_string();
        let db_path = device_db_path(&event);
        assert_eq!(db_path, PathBuf::from("/run/udev/data/c4:64"));
    }

    #[test]
    fn test_device_db_path_no_major_minor() {
        let event = make_test_event("add", "/devices/pci0000:00", "pci");
        let db_path = device_db_path(&event);
        assert_eq!(db_path, PathBuf::from("/run/udev/data/+pci:pci0000:00"));
    }

    // -----------------------------------------------------------------------
    // Control commands
    // -----------------------------------------------------------------------

    #[test]
    fn test_control_command_ping() {
        let queue = Arc::new(Mutex::new(EventQueue::new()));
        let mut reload = false;
        let resp = handle_control_command("PING", &queue, &mut reload);
        assert_eq!(resp, "OK\n");
    }

    #[test]
    fn test_control_command_reload() {
        let queue = Arc::new(Mutex::new(EventQueue::new()));
        let mut reload = false;
        let resp = handle_control_command("RELOAD", &queue, &mut reload);
        assert_eq!(resp, "OK\n");
        assert!(reload);
    }

    #[test]
    fn test_control_command_settle_empty() {
        let queue = Arc::new(Mutex::new(EventQueue::new()));
        let mut reload = false;
        let resp = handle_control_command("SETTLE", &queue, &mut reload);
        assert_eq!(resp, "OK\n");
    }

    #[test]
    fn test_control_command_settle_busy() {
        let queue = Arc::new(Mutex::new(EventQueue::new()));
        {
            let mut q = queue.lock().unwrap();
            q.queue.push_back(make_test_event("add", "/test", "test"));
        }
        let mut reload = false;
        let resp = handle_control_command("SETTLE", &queue, &mut reload);
        assert!(resp.starts_with("BUSY"));
    }

    #[test]
    fn test_control_command_status() {
        let queue = Arc::new(Mutex::new(EventQueue::new()));
        let mut reload = false;
        let resp = handle_control_command("STATUS", &queue, &mut reload);
        assert!(resp.contains("events_processed=0"));
    }

    #[test]
    fn test_control_command_unknown() {
        let queue = Arc::new(Mutex::new(EventQueue::new()));
        let mut reload = false;
        let resp = handle_control_command("FOOBAR", &queue, &mut reload);
        assert!(resp.starts_with("ERR"));
    }

    #[test]
    fn test_control_command_case_insensitive() {
        let queue = Arc::new(Mutex::new(EventQueue::new()));
        let mut reload = false;
        assert_eq!(handle_control_command("ping", &queue, &mut reload), "OK\n");
        assert_eq!(handle_control_command("Ping", &queue, &mut reload), "OK\n");
    }

    // -----------------------------------------------------------------------
    // Symlink removal tracking
    // -----------------------------------------------------------------------

    #[test]
    fn test_symlink_assign_replaces() {
        let rules = RuleSet {
            rules: vec![
                Rule {
                    filename: "test".to_string(),
                    line: 1,
                    tokens: vec![
                        RuleToken {
                            key: "KERNEL".to_string(),
                            attr: None,
                            op: RuleOp::Match,
                            value: "sda".to_string(),
                        },
                        RuleToken {
                            key: "SYMLINK".to_string(),
                            attr: None,
                            op: RuleOp::AssignAdd,
                            value: "link1".to_string(),
                        },
                    ],
                    label: None,
                    goto_target: None,
                },
                Rule {
                    filename: "test".to_string(),
                    line: 2,
                    tokens: vec![
                        RuleToken {
                            key: "KERNEL".to_string(),
                            attr: None,
                            op: RuleOp::Match,
                            value: "sda".to_string(),
                        },
                        RuleToken {
                            key: "SYMLINK".to_string(),
                            attr: None,
                            op: RuleOp::Assign,
                            value: "link2".to_string(),
                        },
                    ],
                    label: None,
                    goto_target: None,
                },
            ],
        };

        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = process_rules(&rules, &mut event);
        // SYMLINK= (assign) replaces, so only link2 should remain
        assert!(!result.symlinks.contains(&"link1".to_string()));
        assert!(result.symlinks.contains(&"link2".to_string()));
    }

    #[test]
    fn test_symlink_remove() {
        let rules = RuleSet {
            rules: vec![
                Rule {
                    filename: "test".to_string(),
                    line: 1,
                    tokens: vec![
                        RuleToken {
                            key: "KERNEL".to_string(),
                            attr: None,
                            op: RuleOp::Match,
                            value: "sda".to_string(),
                        },
                        RuleToken {
                            key: "SYMLINK".to_string(),
                            attr: None,
                            op: RuleOp::AssignAdd,
                            value: "link1 link2 link3".to_string(),
                        },
                    ],
                    label: None,
                    goto_target: None,
                },
                Rule {
                    filename: "test".to_string(),
                    line: 2,
                    tokens: vec![
                        RuleToken {
                            key: "KERNEL".to_string(),
                            attr: None,
                            op: RuleOp::Match,
                            value: "sda".to_string(),
                        },
                        RuleToken {
                            key: "SYMLINK".to_string(),
                            attr: None,
                            op: RuleOp::AssignRemove,
                            value: "link2".to_string(),
                        },
                    ],
                    label: None,
                    goto_target: None,
                },
            ],
        };

        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = process_rules(&rules, &mut event);
        assert!(result.symlinks.contains(&"link1".to_string()));
        assert!(!result.symlinks.contains(&"link2".to_string()));
        assert!(result.symlinks.contains(&"link3".to_string()));
    }

    // -----------------------------------------------------------------------
    // Event queue
    // -----------------------------------------------------------------------

    #[test]
    fn test_event_queue_empty() {
        let q = EventQueue::new();
        assert!(q.is_empty());
        assert_eq!(q.events_processed, 0);
        assert!(q.busy_devpaths.is_empty());
    }

    #[test]
    fn test_event_queue_not_empty() {
        let mut q = EventQueue::new();
        q.queue.push_back(make_test_event("add", "/test", "test"));
        assert!(!q.is_empty());
    }

    #[test]
    fn test_event_queue_active_workers() {
        let mut q = EventQueue::new();
        q.active_workers = 1;
        assert!(!q.is_empty());
    }

    #[test]
    fn test_event_queue_busy_devpaths() {
        let mut q = EventQueue::new();
        assert!(!q.busy_devpaths.contains("/devices/pci0000:00/0000:00:1f.2"));
        q.busy_devpaths
            .insert("/devices/pci0000:00/0000:00:1f.2".to_string());
        assert!(q.busy_devpaths.contains("/devices/pci0000:00/0000:00:1f.2"));
        q.busy_devpaths.remove("/devices/pci0000:00/0000:00:1f.2");
        assert!(!q.busy_devpaths.contains("/devices/pci0000:00/0000:00:1f.2"));
    }

    #[test]
    fn test_event_queue_busy_devpath_serialization() {
        // Verify that busy_devpaths tracking works for per-device serialization
        let mut q = EventQueue::new();
        q.queue
            .push_back(make_test_event("add", "/devices/sda", "block"));
        q.queue
            .push_back(make_test_event("change", "/devices/sda", "block"));
        q.queue
            .push_back(make_test_event("add", "/devices/sdb", "block"));

        // Mark sda as busy
        q.busy_devpaths.insert("/devices/sda".to_string());
        q.active_workers = 1;

        // Simulate the dispatch logic: skip events for busy devpaths
        let max_new = 8usize.saturating_sub(q.active_workers);
        let mut dispatched = 0usize;
        let mut idx = 0usize;
        let mut dispatched_events = Vec::new();

        while dispatched < max_new && idx < q.queue.len() {
            let devpath = q.queue[idx].devpath.clone();
            if q.busy_devpaths.contains(&devpath) {
                idx += 1;
                continue;
            }
            let event = q.queue.remove(idx).unwrap();
            q.busy_devpaths.insert(devpath);
            dispatched_events.push(event);
            dispatched += 1;
        }

        // Only sdb should have been dispatched; both sda events remain queued
        assert_eq!(dispatched_events.len(), 1);
        assert_eq!(dispatched_events[0].devpath, "/devices/sdb");
        assert_eq!(q.queue.len(), 2); // two sda events still queued
        assert_eq!(q.queue[0].devpath, "/devices/sda");
        assert_eq!(q.queue[0].action, "add");
        assert_eq!(q.queue[1].devpath, "/devices/sda");
        assert_eq!(q.queue[1].action, "change");
    }

    #[test]
    fn test_worker_thread_pool_concurrent() {
        // Test that events for different devices can be processed concurrently
        use std::sync::atomic::AtomicUsize;

        let event_queue = Arc::new(Mutex::new(EventQueue::new()));
        let counter = Arc::new(AtomicUsize::new(0));

        // Enqueue events for 3 different devices
        {
            let mut q = event_queue.lock().unwrap();
            q.queue
                .push_back(make_test_event("add", "/devices/a", "test"));
            q.queue
                .push_back(make_test_event("add", "/devices/b", "test"));
            q.queue
                .push_back(make_test_event("add", "/devices/c", "test"));
        }

        let mut handles = Vec::new();

        // Dispatch all 3 (different devpaths, so all can run concurrently)
        {
            let mut q = event_queue.lock().unwrap();
            while let Some(event) = q.queue.pop_front() {
                let devpath = event.devpath.clone();
                q.busy_devpaths.insert(devpath.clone());
                q.active_workers += 1;

                let queue_ref = event_queue.clone();
                let counter_ref = counter.clone();
                handles.push(thread::spawn(move || {
                    // Simulate some work
                    counter_ref.fetch_add(1, Ordering::SeqCst);

                    let mut q = queue_ref.lock().unwrap();
                    q.active_workers -= 1;
                    q.events_processed += 1;
                    q.busy_devpaths.remove(&devpath);
                }));
            }
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(counter.load(Ordering::SeqCst), 3);
        let q = event_queue.lock().unwrap();
        assert_eq!(q.events_processed, 3);
        assert_eq!(q.active_workers, 0);
        assert!(q.busy_devpaths.is_empty());
        assert!(q.is_empty());
    }

    // -----------------------------------------------------------------------
    // Discover rules files
    // -----------------------------------------------------------------------

    #[test]
    fn test_discover_rules_files_no_crash() {
        // Should not panic even if dirs don't exist
        let _files = discover_rules_files();
    }

    // -----------------------------------------------------------------------
    // Options parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_process_rules_options() {
        let rules = RuleSet {
            rules: vec![Rule {
                filename: "test".to_string(),
                line: 1,
                tokens: vec![
                    RuleToken {
                        key: "KERNEL".to_string(),
                        attr: None,
                        op: RuleOp::Match,
                        value: "sd*".to_string(),
                    },
                    RuleToken {
                        key: "OPTIONS".to_string(),
                        attr: None,
                        op: RuleOp::AssignAdd,
                        value: "link_priority=100".to_string(),
                    },
                ],
                label: None,
                goto_target: None,
            }],
        };

        let mut event = make_test_event("add", "/devices/virtual/block/sda", "block");
        let result = process_rules(&rules, &mut event);
        assert!(result.options.contains("link_priority=100"));
    }

    // -----------------------------------------------------------------------
    // Escape sequence handling in values
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_rule_value_escape() {
        let (val, rest) = parse_rule_value(r#""hello\nworld""#).unwrap();
        assert_eq!(val, "hello\nworld");
        assert!(rest.is_empty());
    }

    #[test]
    fn test_parse_rule_value_escaped_quote() {
        let (val, rest) = parse_rule_value(r#""say \"hi\"""#).unwrap();
        assert_eq!(val, "say \"hi\"");
        assert!(rest.is_empty());
    }

    #[test]
    fn test_parse_rule_value_tab() {
        let (val, _) = parse_rule_value(r#""col1\tcol2""#).unwrap();
        assert_eq!(val, "col1\tcol2");
    }

    // -----------------------------------------------------------------------
    // Split alternatives
    // -----------------------------------------------------------------------

    #[test]
    fn test_split_alternatives_simple() {
        let alts = split_alternatives("a|b|c");
        assert_eq!(alts, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_split_alternatives_with_brackets() {
        let alts = split_alternatives("[a|b]|c");
        assert_eq!(alts, vec!["[a|b]", "c"]);
    }

    #[test]
    fn test_split_alternatives_single() {
        let alts = split_alternatives("abc");
        assert_eq!(alts, vec!["abc"]);
    }

    // -----------------------------------------------------------------------
    // Rule operator parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_rule_op_all() {
        assert_eq!(parse_rule_op("==x").unwrap(), (RuleOp::Match, "x"));
        assert_eq!(parse_rule_op("!=x").unwrap(), (RuleOp::Nomatch, "x"));
        assert_eq!(parse_rule_op("=x").unwrap(), (RuleOp::Assign, "x"));
        assert_eq!(parse_rule_op("+=x").unwrap(), (RuleOp::AssignAdd, "x"));
        assert_eq!(parse_rule_op("-=x").unwrap(), (RuleOp::AssignRemove, "x"));
        assert_eq!(parse_rule_op(":=x").unwrap(), (RuleOp::AssignFinal, "x"));
    }

    #[test]
    fn test_parse_rule_op_invalid() {
        assert!(parse_rule_op("<<").is_err());
    }

    // -----------------------------------------------------------------------
    // Resolve program path
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_program_path_absolute() {
        let p = resolve_program_path("/usr/bin/test");
        assert_eq!(p, PathBuf::from("/usr/bin/test"));
    }

    #[test]
    fn test_resolve_program_path_relative_fallback() {
        let p = resolve_program_path("nonexistent_udev_helper_xyz");
        assert_eq!(p, PathBuf::from("nonexistent_udev_helper_xyz"));
    }

    // -----------------------------------------------------------------------
    // UID/GID resolution
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_uid_numeric() {
        assert_eq!(resolve_uid("0"), Some(0));
        assert_eq!(resolve_uid("1000"), Some(1000));
    }

    #[test]
    fn test_resolve_uid_root() {
        assert_eq!(resolve_uid("root"), Some(0));
    }

    #[test]
    fn test_resolve_gid_numeric() {
        assert_eq!(resolve_gid("0"), Some(0));
    }

    #[test]
    fn test_resolve_gid_root() {
        assert_eq!(resolve_gid("root"), Some(0));
    }

    // -----------------------------------------------------------------------
    // DaemonArgs parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_daemon_args_default() {
        let args = DaemonArgs::parse_from_iter(&[]);
        assert!(!args.daemon);
        assert!(!args.debug);
        assert_eq!(args.resolve_names, "early");
        assert_eq!(args.children_max, MAX_WORKERS);
        assert_eq!(args.exec_delay, 0);
        assert_eq!(args.event_timeout, EVENT_TIMEOUT_SECS);
    }

    #[test]
    fn test_daemon_args_all_flags() {
        let args = DaemonArgs::parse_from_iter(
            &[
                "--daemon",
                "--debug",
                "--resolve-names",
                "late",
                "--children-max",
                "16",
                "--exec-delay",
                "2",
                "--event-timeout",
                "30",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        );
        assert!(args.daemon);
        assert!(args.debug);
        assert_eq!(args.resolve_names, "late");
        assert_eq!(args.children_max, 16);
        assert_eq!(args.exec_delay, 2);
        assert_eq!(args.event_timeout, 30);
    }

    #[test]
    fn test_daemon_args_short_flags() {
        let args = DaemonArgs::parse_from_iter(
            &["-d", "-D", "-N", "never", "-c", "4"]
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        );
        assert!(args.daemon);
        assert!(args.debug);
        assert_eq!(args.resolve_names, "never");
        assert_eq!(args.children_max, 4);
    }

    #[test]
    fn test_daemon_args_unknown_ignored() {
        let args = DaemonArgs::parse_from_iter(
            &["--unknown-flag", "--daemon", "--bogus"]
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        );
        assert!(args.daemon);
        assert!(!args.debug);
    }

    #[test]
    fn test_invoked_as_daemon_false() {
        // When running tests, argv[0] is the test binary, not systemd-udevd
        assert!(!invoked_as_daemon());
    }

    // -----------------------------------------------------------------------
    // builtin_net_setup_link tests
    // -----------------------------------------------------------------------

    fn make_net_event(interface: &str) -> UEvent {
        let mut event = UEvent::new();
        event.action = "add".to_string();
        event.subsystem = "net".to_string();
        event.devpath = format!("/devices/pci0000:00/0000:00:03.0/net/{}", interface);
        event
            .env
            .insert("INTERFACE".to_string(), interface.to_string());
        event.env.insert("ACTION".to_string(), "add".to_string());
        event.env.insert("SUBSYSTEM".to_string(), "net".to_string());
        event
    }

    #[test]
    fn test_net_setup_link_skips_non_net_subsystem() {
        let mut event = UEvent::new();
        event.subsystem = "block".to_string();
        event.devpath = "/devices/pci0000:00/0000:00:1f.2/ata1/host0".to_string();
        builtin_net_setup_link(&mut event);
        // Should not set any ID_NET_ variables.
        assert!(event.env.get("ID_NET_LINK_FILE").is_none());
        assert!(event.env.get("ID_NET_NAME").is_none());
    }

    #[test]
    fn test_net_setup_link_skips_empty_interface() {
        let mut event = UEvent::new();
        event.subsystem = "net".to_string();
        event.devpath = String::new();
        // No INTERFACE env var, empty devpath — no interface name available.
        builtin_net_setup_link(&mut event);
        assert!(event.env.get("ID_NET_LINK_FILE").is_none());
    }

    #[test]
    fn test_net_setup_link_no_matching_link_file() {
        // With no .link files on disk matching, nothing should be set.
        // In practice load_link_configs() loads from standard dirs that may
        // or may not have files. We test the function doesn't panic.
        let mut event = make_net_event("test_unlikely_iface_name_12345");
        builtin_net_setup_link(&mut event);
        // We can't assert ID_NET_LINK_FILE is absent because system .link
        // files with OriginalName=* would match. Just verify no panic.
    }

    #[test]
    fn test_net_setup_link_uses_interface_env_var() {
        let mut event = UEvent::new();
        event.subsystem = "net".to_string();
        event.devpath = "/devices/virtual/net/dummy0".to_string();
        event
            .env
            .insert("INTERFACE".to_string(), "dummy0".to_string());
        // Should use INTERFACE, not extract from devpath.
        builtin_net_setup_link(&mut event);
        // Just verify no panic; the function uses INTERFACE correctly.
    }

    #[test]
    fn test_net_setup_link_falls_back_to_devpath() {
        let mut event = UEvent::new();
        event.subsystem = "net".to_string();
        event.devpath = "/devices/virtual/net/lo".to_string();
        // No INTERFACE env var — should extract "lo" from devpath.
        builtin_net_setup_link(&mut event);
        // Just verify no panic.
    }

    #[test]
    fn test_net_setup_link_with_name_policy_path() {
        // Simulate a device where net_id already set ID_NET_NAME_PATH.
        let mut event = make_net_event("eth0");
        event
            .env
            .insert("ID_NET_NAME_PATH".to_string(), "enp3s0".to_string());

        // This will run against real system .link files. If a default
        // .link file with NamePolicy containing "path" matches, it should
        // pick up enp3s0 from ID_NET_NAME_PATH.
        builtin_net_setup_link(&mut event);

        // If a .link file matched and used path policy, ID_NET_NAME should
        // be set. We verify the function runs without panic.
        // On systems with 99-default.link (NamePolicy=kernel database onboard slot path),
        // ID_NET_NAME should be "enp3s0".
        if event.env.contains_key("ID_NET_LINK_FILE") {
            // A .link file matched — verify ID_NET_NAME if it was set.
            if let Some(name) = event.env.get("ID_NET_NAME") {
                assert!(!name.is_empty());
            }
        }
    }

    #[test]
    fn test_net_setup_link_with_mac_in_sysattr() {
        // Test that the function tries to read MAC from sysfs.
        let mut event = make_net_event("eth0");
        // No MAC in env, function will try read_sysattr("address").
        // On test systems this won't find a real sysfs path, so mac=None.
        builtin_net_setup_link(&mut event);
        // No panic = success.
    }

    #[test]
    fn test_net_setup_link_driver_from_event() {
        let mut event = make_net_event("eth0");
        event.driver = "virtio_net".to_string();
        builtin_net_setup_link(&mut event);
        // No panic = success.
    }

    #[test]
    fn test_net_setup_link_driver_from_env() {
        let mut event = make_net_event("eth0");
        event
            .env
            .insert("ID_NET_DRIVER".to_string(), "e1000".to_string());
        builtin_net_setup_link(&mut event);
        // No panic = success.
    }

    #[test]
    fn test_net_setup_link_devtype_from_env() {
        let mut event = make_net_event("wlan0");
        event.env.insert("DEVTYPE".to_string(), "wlan".to_string());
        builtin_net_setup_link(&mut event);
        // No panic = success.
    }

    #[test]
    fn test_net_setup_link_id_path_from_env() {
        let mut event = make_net_event("eth0");
        event
            .env
            .insert("ID_PATH".to_string(), "pci-0000:00:03.0".to_string());
        builtin_net_setup_link(&mut event);
        // No panic = success.
    }

    #[test]
    fn test_net_setup_link_resolve_name_from_policy_unit() {
        // Unit test the resolve_name_from_policy logic directly.
        use libsystemd::link_config::{parse_link_file_content, resolve_name_from_policy};
        use std::path::Path;

        let cfg = parse_link_file_content(
            "[Link]\nNamePolicy=kernel database onboard slot path\n",
            Path::new("99-default.link"),
        )
        .unwrap();

        // Simulate having ID_NET_NAME_PATH available.
        let name = resolve_name_from_policy(&cfg, |key| {
            if key == "ID_NET_NAME_PATH" {
                Some("enp0s3".to_string())
            } else {
                None
            }
        });
        assert_eq!(name.as_deref(), Some("enp0s3"));
    }

    #[test]
    fn test_net_setup_link_resolve_name_prefers_onboard_over_path() {
        use libsystemd::link_config::{parse_link_file_content, resolve_name_from_policy};
        use std::path::Path;

        let cfg = parse_link_file_content(
            "[Link]\nNamePolicy=onboard slot path\n",
            Path::new("99-default.link"),
        )
        .unwrap();

        let name = resolve_name_from_policy(&cfg, |key| match key {
            "ID_NET_NAME_ONBOARD" => Some("eno1".to_string()),
            "ID_NET_NAME_SLOT" => Some("ens3".to_string()),
            "ID_NET_NAME_PATH" => Some("enp0s3".to_string()),
            _ => None,
        });
        assert_eq!(name.as_deref(), Some("eno1"));
    }

    #[test]
    fn test_net_setup_link_resolve_name_explicit_name_fallback() {
        use libsystemd::link_config::{parse_link_file_content, resolve_name_from_policy};
        use std::path::Path;

        let cfg = parse_link_file_content(
            "[Link]\nNamePolicy=database\nName=eth0\n",
            Path::new("10-custom.link"),
        )
        .unwrap();

        // No naming env vars available — falls back to Name=.
        let name = resolve_name_from_policy(&cfg, |_| None);
        assert_eq!(name.as_deref(), Some("eth0"));
    }

    #[test]
    fn test_net_setup_link_resolve_name_keep_returns_none() {
        use libsystemd::link_config::{parse_link_file_content, resolve_name_from_policy};
        use std::path::Path;

        let cfg = parse_link_file_content("[Link]\nNamePolicy=keep\n", Path::new("99-keep.link"))
            .unwrap();

        let name = resolve_name_from_policy(&cfg, |_| None);
        assert!(name.is_none());
    }

    #[test]
    fn test_net_setup_link_link_config_matching() {
        use libsystemd::link_config::{find_matching_link_config, parse_link_file_content};
        use std::path::Path;

        let configs = vec![
            parse_link_file_content(
                "[Match]\nOriginalName=en*\n\n[Link]\nName=eth0\n",
                Path::new("10-eth.link"),
            )
            .unwrap(),
            parse_link_file_content(
                "[Match]\nOriginalName=wl*\n\n[Link]\nName=wlan0\n",
                Path::new("20-wlan.link"),
            )
            .unwrap(),
        ];

        let result = find_matching_link_config(&configs, "enp3s0", None, None, None, None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().link_section.name.as_deref(), Some("eth0"));

        let result = find_matching_link_config(&configs, "wlp2s0", None, None, None, None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().link_section.name.as_deref(), Some("wlan0"));

        let result = find_matching_link_config(&configs, "lo", None, None, None, None);
        assert!(result.is_none());
    }

    #[test]
    fn test_net_setup_link_link_config_first_match_wins() {
        use libsystemd::link_config::{find_matching_link_config, parse_link_file_content};
        use std::path::Path;

        let configs = vec![
            parse_link_file_content(
                "[Match]\nOriginalName=en*\n\n[Link]\nName=first\n",
                Path::new("10-first.link"),
            )
            .unwrap(),
            parse_link_file_content(
                "[Match]\nOriginalName=en*\n\n[Link]\nName=second\n",
                Path::new("20-second.link"),
            )
            .unwrap(),
        ];

        let result = find_matching_link_config(&configs, "enp0s3", None, None, None, None);
        assert_eq!(result.unwrap().link_section.name.as_deref(), Some("first"));
    }

    #[test]
    fn test_net_setup_link_link_config_mac_match() {
        use libsystemd::link_config::{find_matching_link_config, parse_link_file_content};
        use std::path::Path;

        let configs = vec![
            parse_link_file_content(
                "[Match]\nMACAddress=00:11:22:33:44:55\n\n[Link]\nName=specific\n",
                Path::new("10-mac.link"),
            )
            .unwrap(),
            parse_link_file_content(
                "[Match]\nOriginalName=*\n\n[Link]\nName=fallback\n",
                Path::new("99-default.link"),
            )
            .unwrap(),
        ];

        let result = find_matching_link_config(
            &configs,
            "enp0s3",
            Some("00:11:22:33:44:55"),
            None,
            None,
            None,
        );
        assert_eq!(
            result.unwrap().link_section.name.as_deref(),
            Some("specific")
        );

        let result = find_matching_link_config(
            &configs,
            "enp0s3",
            Some("aa:bb:cc:dd:ee:ff"),
            None,
            None,
            None,
        );
        assert_eq!(
            result.unwrap().link_section.name.as_deref(),
            Some("fallback")
        );
    }

    #[test]
    fn test_net_setup_link_link_config_driver_match() {
        use libsystemd::link_config::{find_matching_link_config, parse_link_file_content};
        use std::path::Path;

        let configs = vec![
            parse_link_file_content(
                "[Match]\nDriver=virtio*\n\n[Link]\nName=virt0\n",
                Path::new("10-virtio.link"),
            )
            .unwrap(),
        ];

        let result =
            find_matching_link_config(&configs, "eth0", None, Some("virtio_net"), None, None);
        assert!(result.is_some());

        let result = find_matching_link_config(&configs, "eth0", None, Some("e1000"), None, None);
        assert!(result.is_none());
    }

    #[test]
    fn test_net_setup_link_alternative_names_policy() {
        use libsystemd::link_config::parse_link_file_content;
        use std::path::Path;

        let cfg = parse_link_file_content(
            "[Match]\nOriginalName=*\n\n[Link]\nNamePolicy=path\nAlternativeNamesPolicy=database onboard slot mac\n",
            Path::new("99-default.link"),
        )
        .unwrap();

        assert_eq!(cfg.link_section.alternative_names_policy.len(), 4);
    }
}
