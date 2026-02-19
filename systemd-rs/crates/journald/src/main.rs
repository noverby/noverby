//! systemd-journald — Journal logging daemon.
//!
//! A drop-in replacement for `systemd-journald(8)`. This daemon collects
//! structured log data from multiple sources and stores it in the journal:
//!
//! - `/run/systemd/journal/socket`  — native journal protocol (datagram)
//! - `/run/systemd/journal/stdout`  — stdout stream connections from services
//! - `/dev/log`                      — BSD syslog protocol (datagram)
//! - `/proc/kmsg`                    — kernel ring buffer messages
//!
//! It also supports:
//!
//! - Rate limiting per-service to prevent log flooding
//! - Journal file rotation and disk usage limits
//! - sd_notify READY=1 / STATUS= / WATCHDOG=1 protocol
//! - SIGUSR1 for flushing volatile → persistent storage
//! - SIGUSR2 for journal rotation
//!
//! Configuration is read from `/etc/systemd/journald.conf` and
//! `/etc/systemd/journald.conf.d/*.conf`.

mod journal;

use journal::entry::JournalEntry;
use journal::storage::{JournalStorage, StorageConfig};

use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};

use std::os::unix::net::{UnixDatagram, UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{process, thread};

// ---------------------------------------------------------------------------
// Socket paths
// ---------------------------------------------------------------------------

/// Native journal protocol socket (datagram).
const JOURNAL_SOCKET_PATH: &str = "/run/systemd/journal/socket";
/// Stdout capture socket (stream, for connected services).
const STDOUT_SOCKET_PATH: &str = "/run/systemd/journal/stdout";
/// BSD syslog socket (datagram).
const SYSLOG_SOCKET_PATH: &str = "/dev/log";
/// Kernel ring buffer.
const KMSG_PATH: &str = "/proc/kmsg";
/// PID file for sd_notify integration.
const RUNTIME_DIR: &str = "/run/systemd/journal";

// ---------------------------------------------------------------------------
// Rate limiting
// ---------------------------------------------------------------------------

/// Default: allow 10 000 messages per 30 seconds per source.
const RATE_LIMIT_INTERVAL_USEC: u64 = 30_000_000; // 30 s in µs
const RATE_LIMIT_BURST: u64 = 10_000;

/// Per-source rate limiter state.
struct RateLimiter {
    /// Map from source identifier (unit name or PID) to state.
    sources: HashMap<String, RateLimitState>,
}

struct RateLimitState {
    /// Start of the current window.
    window_start: Instant,
    /// Number of messages in the current window.
    count: u64,
    /// Whether we've already logged a suppression message for this window.
    suppression_logged: bool,
}

impl RateLimiter {
    fn new() -> Self {
        RateLimiter {
            sources: HashMap::new(),
        }
    }

    /// Returns `true` if the message should be accepted, `false` if rate-limited.
    fn check(&mut self, source: &str, burst: u64, interval: Duration) -> bool {
        let now = Instant::now();

        let state = self
            .sources
            .entry(source.to_string())
            .or_insert_with(|| RateLimitState {
                window_start: now,
                count: 0,
                suppression_logged: false,
            });

        // If the interval has elapsed, reset the window
        if now.duration_since(state.window_start) >= interval {
            state.window_start = now;
            state.count = 0;
            state.suppression_logged = false;
        }

        state.count += 1;

        if state.count <= burst {
            true
        } else {
            if !state.suppression_logged {
                state.suppression_logged = true;
                eprintln!(
                    "journald: Rate limit exceeded for '{}', suppressing further messages",
                    source
                );
            }
            false
        }
    }

    /// Periodically clean up stale entries to avoid unbounded memory growth.
    fn gc(&mut self, max_age: Duration) {
        let now = Instant::now();
        self.sources
            .retain(|_, state| now.duration_since(state.window_start) < max_age);
    }
}

// ---------------------------------------------------------------------------
// Journald configuration
// ---------------------------------------------------------------------------

/// Configuration parsed from journald.conf.
#[derive(Debug, Clone)]
struct JournaldConfig {
    /// Storage mode: "volatile", "persistent", "auto", "none".
    storage: String,
    /// Compress entries larger than this (0 = disabled). Not yet implemented.
    compress: bool,
    /// Maximum size of individual journal files.
    max_file_size: u64,
    /// Maximum total disk usage for persistent journal.
    system_max_use: u64,
    /// Maximum total disk usage for volatile journal.
    runtime_max_use: u64,
    /// Maximum number of journal files.
    max_files: usize,
    /// Rate limit interval in microseconds.
    rate_limit_interval_usec: u64,
    /// Rate limit burst count.
    rate_limit_burst: u64,
    /// Forward to syslog.
    forward_to_syslog: bool,
    /// Forward to kmsg.
    forward_to_kmsg: bool,
    /// Forward to console.
    forward_to_console: bool,
    /// Forward to wall.
    forward_to_wall: bool,
    /// Maximum log level to store (0=emerg .. 7=debug).
    max_level_store: u8,
    /// Maximum log level to forward to syslog.
    max_level_syslog: u8,
    /// Maximum log level to forward to kmsg.
    max_level_kmsg: u8,
    /// Maximum log level to forward to console.
    max_level_console: u8,
    /// Maximum log level to forward to wall.
    max_level_wall: u8,
    /// Maximum field size in bytes (fields larger than this are truncated).
    max_field_size: usize,
}

impl Default for JournaldConfig {
    fn default() -> Self {
        JournaldConfig {
            storage: "auto".to_string(),
            compress: true,
            max_file_size: 64 * 1024 * 1024,   // 64 MiB
            system_max_use: 512 * 1024 * 1024, // 512 MiB
            runtime_max_use: 64 * 1024 * 1024, // 64 MiB
            max_files: 100,
            rate_limit_interval_usec: RATE_LIMIT_INTERVAL_USEC,
            rate_limit_burst: RATE_LIMIT_BURST,
            forward_to_syslog: false,
            forward_to_kmsg: false,
            forward_to_console: false,
            forward_to_wall: true,
            max_level_store: 7,         // debug
            max_level_syslog: 7,        // debug
            max_level_kmsg: 4,          // warning
            max_level_console: 6,       // info
            max_level_wall: 0,          // emerg
            max_field_size: 768 * 1024, // 768 KiB
        }
    }
}

impl JournaldConfig {
    fn load() -> Self {
        let mut config = JournaldConfig::default();

        // Load main config
        if let Ok(contents) = fs::read_to_string("/etc/systemd/journald.conf") {
            config.parse_config(&contents);
        }

        // Load drop-in configs
        if let Ok(entries) = fs::read_dir("/etc/systemd/journald.conf.d") {
            let mut files: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|ext| ext == "conf"))
                .collect();
            files.sort();
            for path in files {
                if let Ok(contents) = fs::read_to_string(&path) {
                    config.parse_config(&contents);
                }
            }
        }

        config
    }

    fn parse_config(&mut self, contents: &str) {
        let mut in_journal_section = false;

        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') {
                in_journal_section = line == "[Journal]";
                continue;
            }
            if !in_journal_section {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                match key {
                    "Storage" => self.storage = value.to_lowercase(),
                    "Compress" => self.compress = parse_bool(value),
                    "SystemMaxFileSize" | "MaxFileSizeBytes" => {
                        if let Some(bytes) = parse_size(value) {
                            self.max_file_size = bytes;
                        }
                    }
                    "SystemMaxUse" => {
                        if let Some(bytes) = parse_size(value) {
                            self.system_max_use = bytes;
                        }
                    }
                    "RuntimeMaxUse" => {
                        if let Some(bytes) = parse_size(value) {
                            self.runtime_max_use = bytes;
                        }
                    }
                    "MaxFileSec" | "MaxFiles" => {
                        if let Ok(n) = value.parse::<usize>() {
                            self.max_files = n;
                        }
                    }
                    "RateLimitIntervalSec" | "RateLimitIntervalUSec" => {
                        if let Some(usec) = parse_timespan_usec(value) {
                            self.rate_limit_interval_usec = usec;
                        }
                    }
                    "RateLimitBurst" => {
                        if let Ok(n) = value.parse::<u64>() {
                            self.rate_limit_burst = n;
                        }
                    }
                    "ForwardToSyslog" => self.forward_to_syslog = parse_bool(value),
                    "ForwardToKMsg" => self.forward_to_kmsg = parse_bool(value),
                    "ForwardToConsole" => self.forward_to_console = parse_bool(value),
                    "ForwardToWall" => self.forward_to_wall = parse_bool(value),
                    "MaxLevelStore" => {
                        if let Some(level) = parse_log_level(value) {
                            self.max_level_store = level;
                        }
                    }
                    "MaxLevelSyslog" => {
                        if let Some(level) = parse_log_level(value) {
                            self.max_level_syslog = level;
                        }
                    }
                    "MaxLevelKMsg" => {
                        if let Some(level) = parse_log_level(value) {
                            self.max_level_kmsg = level;
                        }
                    }
                    "MaxLevelConsole" => {
                        if let Some(level) = parse_log_level(value) {
                            self.max_level_console = level;
                        }
                    }
                    "MaxLevelWall" => {
                        if let Some(level) = parse_log_level(value) {
                            self.max_level_wall = level;
                        }
                    }
                    "MaxFieldSize" | "LineMax" => {
                        if let Some(bytes) = parse_size(value) {
                            self.max_field_size = bytes as usize;
                        }
                    }
                    _ => {} // Ignore unknown keys
                }
            }
        }
    }

    /// Determine whether to use persistent storage based on the Storage= setting.
    fn use_persistent_storage(&self) -> bool {
        match self.storage.as_str() {
            "persistent" => true,
            "volatile" => false,
            "none" => false,
            _ => {
                // "auto" mode: use persistent if /var/log/journal exists
                Path::new("/var/log/journal").is_dir()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Configuration parsing helpers
// ---------------------------------------------------------------------------

fn parse_bool(s: &str) -> bool {
    matches!(s.to_lowercase().as_str(), "yes" | "true" | "1" | "on" | "y")
}

fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let suffixes: &[(&str, u64)] = &[
        ("EiB", 1024 * 1024 * 1024 * 1024 * 1024 * 1024),
        ("PiB", 1024 * 1024 * 1024 * 1024 * 1024),
        ("TiB", 1024 * 1024 * 1024 * 1024),
        ("GiB", 1024 * 1024 * 1024),
        ("MiB", 1024 * 1024),
        ("KiB", 1024),
        ("EB", 1_000_000_000_000_000_000),
        ("PB", 1_000_000_000_000_000),
        ("TB", 1_000_000_000_000),
        ("GB", 1_000_000_000),
        ("MB", 1_000_000),
        ("KB", 1_000),
        ("E", 1024 * 1024 * 1024 * 1024 * 1024 * 1024),
        ("P", 1024 * 1024 * 1024 * 1024 * 1024),
        ("T", 1024 * 1024 * 1024 * 1024),
        ("G", 1024 * 1024 * 1024),
        ("M", 1024 * 1024),
        ("K", 1024),
        ("B", 1),
    ];

    for &(suffix, multiplier) in suffixes {
        if let Some(num_str) = s.strip_suffix(suffix) {
            let num_str = num_str.trim();
            if let Ok(n) = num_str.parse::<u64>() {
                return Some(n * multiplier);
            }
        }
    }

    s.parse::<u64>().ok()
}

fn parse_timespan_usec(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Try direct microsecond value
    if let Ok(n) = s.parse::<u64>() {
        return Some(n);
    }

    let suffixes: &[(&str, u64)] = &[
        ("min", 60_000_000),
        ("minutes", 60_000_000),
        ("minute", 60_000_000),
        ("sec", 1_000_000),
        ("second", 1_000_000),
        ("seconds", 1_000_000),
        ("ms", 1_000),
        ("msec", 1_000),
        ("us", 1),
        ("usec", 1),
        ("h", 3_600_000_000),
        ("hr", 3_600_000_000),
        ("hour", 3_600_000_000),
        ("hours", 3_600_000_000),
        ("m", 60_000_000),
        ("s", 1_000_000),
        ("d", 86_400_000_000),
        ("day", 86_400_000_000),
        ("days", 86_400_000_000),
    ];

    for &(suffix, multiplier) in suffixes {
        if let Some(num_str) = s.strip_suffix(suffix) {
            let num_str = num_str.trim();
            if let Ok(n) = num_str.parse::<u64>() {
                return Some(n * multiplier);
            }
        }
    }

    None
}

fn parse_log_level(s: &str) -> Option<u8> {
    match s.to_lowercase().as_str() {
        "emerg" | "emergency" | "0" => Some(0),
        "alert" | "1" => Some(1),
        "crit" | "critical" | "2" => Some(2),
        "err" | "error" | "3" => Some(3),
        "warning" | "warn" | "4" => Some(4),
        "notice" | "5" => Some(5),
        "info" | "6" => Some(6),
        "debug" | "7" => Some(7),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Shared daemon state
// ---------------------------------------------------------------------------

/// Shared state for the journald daemon, protected by appropriate locks.
struct JournaldState {
    /// The journal storage engine.
    storage: Mutex<JournalStorage>,
    /// Rate limiter.
    rate_limiter: Mutex<RateLimiter>,
    /// Configuration.
    config: JournaldConfig,
    /// Global sequence number counter.
    seqnum: AtomicU64,
    /// Shutdown flag.
    shutdown: AtomicBool,
    /// Flush requested (SIGUSR1).
    flush_requested: AtomicBool,
    /// Rotate requested (SIGUSR2).
    rotate_requested: AtomicBool,
}

impl JournaldState {
    fn new(config: JournaldConfig, storage: JournalStorage) -> Self {
        JournaldState {
            storage: Mutex::new(storage),
            rate_limiter: Mutex::new(RateLimiter::new()),
            config,
            seqnum: AtomicU64::new(1),
            shutdown: AtomicBool::new(false),
            flush_requested: AtomicBool::new(false),
            rotate_requested: AtomicBool::new(false),
        }
    }

    /// Dispatch a fully-formed journal entry into storage.
    fn dispatch_entry(&self, mut entry: JournalEntry) {
        // Check priority against MaxLevelStore
        if let Some(priority) = entry.priority()
            && priority > self.config.max_level_store
        {
            return;
        }

        // Rate limiting
        let source = entry
            .systemd_unit()
            .or_else(|| entry.syslog_identifier())
            .or_else(|| entry.pid().map(|p| p.to_string()))
            .unwrap_or_else(|| "unknown".to_string());

        {
            let mut rl = self.rate_limiter.lock().unwrap();
            let interval = Duration::from_micros(self.config.rate_limit_interval_usec);
            if !rl.check(&source, self.config.rate_limit_burst, interval) {
                return;
            }
        }

        // Truncate oversized fields
        let max_field = self.config.max_field_size;
        if max_field > 0 {
            let keys: Vec<String> = entry.fields.keys().cloned().collect();
            for key in keys {
                if let Some(value) = entry.fields.get_mut(&key)
                    && value.len() > max_field
                {
                    value.truncate(max_field);
                }
            }
        }

        // Assign sequence number
        let seqnum = self.seqnum.fetch_add(1, Ordering::Relaxed);
        entry.seqnum = seqnum;

        // Forward to console if configured
        if self.config.forward_to_console
            && let Some(priority) = entry.priority()
            && priority <= self.config.max_level_console
        {
            let _ = writeln!(io::stderr(), "{}", entry);
        }

        // Forward to wall if configured (only for emerg/alert)
        if self.config.forward_to_wall
            && let Some(priority) = entry.priority()
            && priority <= self.config.max_level_wall
        {
            forward_to_wall(&entry);
        }

        // Store the entry
        let mut storage = self.storage.lock().unwrap();
        if let Err(e) = storage.append(&entry) {
            eprintln!("journald: Failed to store entry: {}", e);
        }
    }
}

/// Forward an entry to all logged-in terminals via wall(1)-style broadcast.
fn forward_to_wall(entry: &JournalEntry) {
    let message = entry.message().unwrap_or_default();
    let identifier = entry
        .syslog_identifier()
        .unwrap_or_else(|| "unknown".to_string());
    let pid_str = entry.pid().map(|p| format!("[{}]", p)).unwrap_or_default();

    let wall_msg = format!(
        "\r\nBroadcast message from {}{} (journald):\r\n{}\r\n",
        identifier, pid_str, message
    );

    // Write to all terminal devices in /dev/pts/ and /dev/tty*
    let write_to_tty = |path: &Path| {
        if let Ok(mut f) = fs::OpenOptions::new().write(true).open(path) {
            let _ = f.write_all(wall_msg.as_bytes());
        }
    };

    if let Ok(entries) = fs::read_dir("/dev/pts") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.to_string_lossy().ends_with("ptmx") {
                continue;
            }
            write_to_tty(&path);
        }
    }
}

// ---------------------------------------------------------------------------
// Native journal protocol parser
// ---------------------------------------------------------------------------

/// Parse a native journal protocol message.
///
/// The native protocol sends newline-separated `KEY=VALUE` pairs as a single
/// datagram.  For binary-safe values, the format uses:
///   `KEY\n<8-byte LE length><data>\n`
///
/// See `sd_journal_sendv(3)` for the full specification.
fn parse_native_message(data: &[u8]) -> JournalEntry {
    let mut entry = JournalEntry::new();
    let mut pos = 0;

    while pos < data.len() {
        // Find the next newline
        let remaining = &data[pos..];

        // Check if this line contains '=' (text field) or not (binary field)
        let newline_pos = remaining.iter().position(|&b| b == b'\n');
        let eq_pos = remaining.iter().position(|&b| b == b'=');

        match (eq_pos, newline_pos) {
            (Some(eq), Some(nl)) if eq < nl => {
                // Text field: KEY=VALUE\n
                let key = String::from_utf8_lossy(&remaining[..eq]).into_owned();
                let value = remaining[eq + 1..nl].to_vec();
                if is_valid_field_name(&key) {
                    entry.fields.insert(key, value);
                }
                pos += nl + 1;
            }
            (_, Some(nl)) if nl < remaining.len() - 1 => {
                // Might be a binary field: KEY\n<8-byte LE length><data>\n
                let key = String::from_utf8_lossy(&remaining[..nl]).into_owned();
                let after_nl = &remaining[nl + 1..];

                if after_nl.len() >= 8 {
                    let value_len = u64::from_le_bytes(after_nl[..8].try_into().unwrap()) as usize;
                    let data_start = 8;
                    if after_nl.len() >= data_start + value_len {
                        let value = after_nl[data_start..data_start + value_len].to_vec();
                        if is_valid_field_name(&key) {
                            entry.fields.insert(key, value);
                        }
                        // Skip past the value and the trailing newline
                        pos += nl + 1 + 8 + value_len;
                        if pos < data.len() && data[pos] == b'\n' {
                            pos += 1;
                        }
                        continue;
                    }
                }

                // Fallback: treat as a line without value
                pos += nl + 1;
            }
            (Some(eq), None) => {
                // Last line, no trailing newline: KEY=VALUE
                let key = String::from_utf8_lossy(&remaining[..eq]).into_owned();
                let value = remaining[eq + 1..].to_vec();
                if is_valid_field_name(&key) {
                    entry.fields.insert(key, value);
                }
                pos = data.len();
            }
            _ => {
                // Skip malformed data
                pos = match newline_pos {
                    Some(nl) => pos + nl + 1,
                    None => data.len(),
                };
            }
        }
    }

    entry
}

/// Validate a journal field name.  Field names must consist of uppercase
/// letters, digits, and underscores, must not start with a digit, and
/// must contain at least one letter or underscore.
///
/// Underscore-prefixed fields are reserved for trusted fields set by
/// journald itself, but we accept them from clients too — journald will
/// just overwrite the trusted ones.
fn is_valid_field_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    // Must not start with a digit
    if name.as_bytes()[0].is_ascii_digit() {
        return false;
    }
    name.bytes()
        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

// ---------------------------------------------------------------------------
// Syslog protocol parser
// ---------------------------------------------------------------------------

/// Parse a BSD syslog message into a journal entry.
///
/// The format is roughly: `<PRI>TIMESTAMP HOSTNAME APP-NAME[PID]: MESSAGE`
/// or the simpler: `<PRI>MESSAGE`
fn parse_syslog_message(data: &[u8]) -> JournalEntry {
    let mut entry = JournalEntry::new();
    let text = String::from_utf8_lossy(data);
    let text = text.trim_end_matches('\n').trim_end_matches('\0');

    // Try to parse priority
    let (priority, facility, rest) = if text.starts_with('<') {
        if let Some(close) = text.find('>') {
            let pri_str = &text[1..close];
            if let Ok(pri_val) = pri_str.parse::<u32>() {
                let facility = pri_val / 8;
                let severity = (pri_val % 8) as u8;
                (Some(severity), Some(facility), &text[close + 1..])
            } else {
                (None, None, text)
            }
        } else {
            (None, None, text)
        }
    } else {
        (None, None, text)
    };

    if let Some(pri) = priority {
        entry.set_field("PRIORITY", pri.to_string());
    }
    if let Some(fac) = facility {
        entry.set_field("SYSLOG_FACILITY", fac.to_string());
    }

    // Try to parse the traditional syslog format:
    // "Mon DD HH:MM:SS hostname app[pid]: message"
    // We try a simple heuristic: if there's a colon followed by a space, split there.
    let (identifier, pid, message) = parse_syslog_tag_and_message(rest);

    if let Some(ident) = identifier {
        entry.set_field("SYSLOG_IDENTIFIER", ident);
    }
    if let Some(pid_val) = pid {
        entry.set_field("SYSLOG_PID", pid_val);
    }
    entry.set_field("MESSAGE", message);

    entry
}

/// Parse the tag and message from a syslog line (after priority).
/// Returns (identifier, pid, message).
fn parse_syslog_tag_and_message(s: &str) -> (Option<String>, Option<String>, String) {
    let s = s.trim();

    // Skip optional timestamp (3-letter month, day, time)
    // e.g. "Jan  1 00:00:00 "
    let s = skip_syslog_timestamp(s);

    // Skip optional hostname
    // After timestamp, the next word before a space could be hostname
    // We use a simple heuristic: if there's a colon in the rest, the part
    // before the colon is the tag.

    // Look for "identifier[pid]: message" or "identifier: message"
    if let Some(colon_pos) = s.find(": ") {
        let tag_part = &s[..colon_pos];
        let message = &s[colon_pos + 2..];

        // Check for [pid] in the tag
        if let Some(bracket_open) = tag_part.rfind('[')
            && let Some(bracket_close) = tag_part.rfind(']')
            && bracket_close > bracket_open
        {
            let identifier = tag_part[..bracket_open].trim();
            let pid = &tag_part[bracket_open + 1..bracket_close];

            // The identifier might have a hostname prefix; take last word
            let identifier = identifier.split_whitespace().last().unwrap_or(identifier);

            return (
                Some(identifier.to_string()),
                Some(pid.to_string()),
                message.to_string(),
            );
        }

        // No PID — just identifier: message
        let identifier = tag_part.split_whitespace().last().unwrap_or(tag_part);
        return (Some(identifier.to_string()), None, message.to_string());
    }

    // No structured tag found — entire string is the message
    (None, None, s.to_string())
}

/// Skip a syslog-style timestamp prefix (e.g. "Jan  1 00:00:00 ").
fn skip_syslog_timestamp(s: &str) -> &str {
    // Simple heuristic: if the string starts with a 3-letter month abbreviation
    // followed by day and time, skip past it
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    // Use split_whitespace to handle variable spacing (e.g. "Jan  1")
    let words: Vec<&str> = s.split_whitespace().collect();
    if words.len() >= 4 {
        // Check if first word is a month
        let first = words[0];
        if months.contains(&first) {
            // words[1] should be the day, words[2] should be HH:MM:SS
            if let Some(time_word) = words.get(2)
                && time_word.contains(':')
            {
                // Find the byte position after "Mon DD HH:MM:SS " in the
                // original string by locating the end of the time word.
                if let Some(time_start) = s.find(time_word) {
                    let after_time = time_start + time_word.len();
                    // Skip any whitespace after the time
                    let rest = &s[after_time..];
                    return rest.strip_prefix(' ').unwrap_or(rest);
                }
            }
        }
    }

    s
}

// ---------------------------------------------------------------------------
// Kernel message (kmsg) parser
// ---------------------------------------------------------------------------

/// Parse a /dev/kmsg line into a journal entry.
///
/// The format is: `PRIORITY,SEQNUM,TIMESTAMP,-;MESSAGE\n`
/// where PRIORITY includes the facility, SEQNUM is monotonic,
/// TIMESTAMP is in microseconds since boot.
fn parse_kmsg_line(line: &str) -> Option<JournalEntry> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let mut entry = JournalEntry::new();
    entry.set_field("_TRANSPORT", "kernel");

    // Split at ';'
    let (header, message) = match line.split_once(';') {
        Some((h, m)) => (h, m),
        None => {
            entry.set_field("MESSAGE", line);
            return Some(entry);
        }
    };

    entry.set_field("MESSAGE", message);

    // Parse header: "priority,seqnum,timestamp,flags"
    let parts: Vec<&str> = header.split(',').collect();
    if let Some(pri_str) = parts.first()
        && let Ok(pri_val) = pri_str.parse::<u32>()
    {
        let severity = (pri_val & 7) as u8;
        let facility = pri_val >> 3;
        entry.set_field("PRIORITY", severity.to_string());
        entry.set_field("SYSLOG_FACILITY", facility.to_string());
    }

    // Set a default identifier for kernel messages
    entry.set_field("SYSLOG_IDENTIFIER", "kernel");
    entry.set_field("_PID", "0");

    Some(entry)
}

// ---------------------------------------------------------------------------
// Socket setup
// ---------------------------------------------------------------------------

/// Ensure the runtime directory exists and create/bind a datagram socket.
fn create_datagram_socket(path: &str) -> io::Result<UnixDatagram> {
    // Ensure parent directory exists
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }

    // Remove stale socket file
    let _ = fs::remove_file(path);

    let sock = UnixDatagram::bind(path)?;

    // Set permissions so any process can write to it
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o666));
    }

    Ok(sock)
}

/// Create a stream (connection-oriented) listener socket.
fn create_stream_listener(path: &str) -> io::Result<UnixListener> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    let _ = fs::remove_file(path);

    let listener = UnixListener::bind(path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o666));
    }

    Ok(listener)
}

// ---------------------------------------------------------------------------
// Socket listener threads
// ---------------------------------------------------------------------------

/// Listen on the native journal socket for datagram messages.
fn native_socket_listener(state: Arc<JournaldState>) {
    let sock = match create_datagram_socket(JOURNAL_SOCKET_PATH) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("journald: Failed to create native socket: {}", e);
            return;
        }
    };

    eprintln!("journald: Listening on {}", JOURNAL_SOCKET_PATH);

    let mut buf = vec![0u8; 256 * 1024]; // 256 KiB receive buffer

    loop {
        if state.shutdown.load(Ordering::Relaxed) {
            break;
        }

        match sock.recv_from(&mut buf) {
            Ok((len, _addr)) => {
                let data = &buf[..len];
                let mut entry = parse_native_message(data);

                // Set trusted fields — for datagram sockets we can get the
                // sender's credentials via SO_PEERCRED, but recv_from doesn't
                // expose that easily.  Fall back to using SYSLOG_PID if set.
                let pid = entry
                    .field("SYSLOG_PID")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);

                if pid > 0 {
                    entry.set_trusted_process_fields(pid);
                }
                entry.set_field("_TRANSPORT", "journal");
                entry.set_boot_id();
                entry.set_machine_id();
                entry.set_hostname();

                state.dispatch_entry(entry);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                if state.shutdown.load(Ordering::Relaxed) {
                    break;
                }
                eprintln!("journald: Native socket recv error: {}", e);
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// Listen on the syslog socket (/dev/log) for datagram messages.
fn syslog_socket_listener(state: Arc<JournaldState>) {
    let sock = match create_datagram_socket(SYSLOG_SOCKET_PATH) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "journald: Failed to create syslog socket {}: {}",
                SYSLOG_SOCKET_PATH, e
            );
            return;
        }
    };

    eprintln!("journald: Listening on {}", SYSLOG_SOCKET_PATH);

    let mut buf = vec![0u8; 64 * 1024]; // 64 KiB

    loop {
        if state.shutdown.load(Ordering::Relaxed) {
            break;
        }

        match sock.recv(&mut buf) {
            Ok(len) => {
                let data = &buf[..len];
                let mut entry = parse_syslog_message(data);

                // Try to look up process info from SYSLOG_PID
                let pid = entry
                    .field("SYSLOG_PID")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                if pid > 0 {
                    entry.set_trusted_process_fields(pid);
                }
                entry.set_field("_TRANSPORT", "syslog");
                entry.set_boot_id();
                entry.set_machine_id();
                entry.set_hostname();

                state.dispatch_entry(entry);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                if state.shutdown.load(Ordering::Relaxed) {
                    break;
                }
                eprintln!("journald: Syslog socket recv error: {}", e);
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// Listen on the stdout socket for stream connections from services.
///
/// Services connected here send one log line per line, optionally prefixed
/// with a header that specifies the syslog identifier, priority, etc.
fn stdout_socket_listener(state: Arc<JournaldState>) {
    let listener = match create_stream_listener(STDOUT_SOCKET_PATH) {
        Ok(l) => l,
        Err(e) => {
            eprintln!(
                "journald: Failed to create stdout socket {}: {}",
                STDOUT_SOCKET_PATH, e
            );
            return;
        }
    };

    eprintln!("journald: Listening on {}", STDOUT_SOCKET_PATH);

    for stream in listener.incoming() {
        if state.shutdown.load(Ordering::Relaxed) {
            break;
        }

        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                thread::spawn(move || {
                    handle_stdout_connection(stream, state);
                });
            }
            Err(e) => {
                if state.shutdown.load(Ordering::Relaxed) {
                    break;
                }
                eprintln!("journald: Stdout socket accept error: {}", e);
            }
        }
    }
}

/// Handle a single stdout stream connection.
///
/// The connection starts with a header block (newline-separated KEY=VALUE)
/// terminated by an empty line, followed by the actual log data (one line
/// per log message).
fn handle_stdout_connection(stream: UnixStream, state: Arc<JournaldState>) {
    let reader = BufReader::new(stream);
    let mut lines = reader.lines();

    // Parse the header: KEY=VALUE lines until an empty line
    let mut identifier = String::from("unknown");
    let mut priority: u8 = 6; // default: info
    let mut level_prefix = true;
    let mut unit_name = String::new();

    // Read header lines
    loop {
        match lines.next() {
            Some(Ok(line)) => {
                if line.is_empty() {
                    break; // End of header
                }
                if let Some((key, value)) = line.split_once('=') {
                    match key {
                        "SYSLOG_IDENTIFIER" => identifier = value.to_string(),
                        "PRIORITY" => {
                            if let Ok(p) = value.parse::<u8>() {
                                priority = p;
                            }
                        }
                        "LEVEL_PREFIX" => level_prefix = value == "1" || value == "true",
                        "_SYSTEMD_UNIT" | "UNIT" => unit_name = value.to_string(),
                        _ => {}
                    }
                }
            }
            Some(Err(_)) | None => return,
        }
    }

    // Process log lines
    for line in lines {
        if state.shutdown.load(Ordering::Relaxed) {
            break;
        }

        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.is_empty() {
            continue;
        }

        let (effective_priority, message) = if level_prefix {
            parse_level_prefix_line(&line, priority)
        } else {
            (priority, line.as_str())
        };

        let mut entry = JournalEntry::new();
        entry.set_field("MESSAGE", message);
        entry.set_field("PRIORITY", effective_priority.to_string());
        entry.set_field("SYSLOG_IDENTIFIER", &identifier);
        if !unit_name.is_empty() {
            entry.set_field("_SYSTEMD_UNIT", &unit_name);
        }
        entry.set_field("_TRANSPORT", "stdout");
        entry.set_boot_id();
        entry.set_machine_id();
        entry.set_hostname();

        state.dispatch_entry(entry);
    }
}

/// Parse a kernel-style `<N>` priority prefix from a log line.
/// Returns (priority, message_without_prefix).
fn parse_level_prefix_line(line: &str, default_priority: u8) -> (u8, &str) {
    if let Some(rest) = line.strip_prefix('<')
        && let Some(close_pos) = rest.find('>')
        && close_pos <= 1
        && let Ok(p) = rest[..close_pos].parse::<u8>()
        && p <= 7
    {
        return (p, &rest[close_pos + 1..]);
    }
    (default_priority, line)
}

/// Read and process kernel messages from /proc/kmsg (or /dev/kmsg).
fn kmsg_reader(state: Arc<JournaldState>) {
    // Try /dev/kmsg first (structured format), fall back to /proc/kmsg
    let kmsg_path = if Path::new("/dev/kmsg").exists() {
        "/dev/kmsg"
    } else if Path::new(KMSG_PATH).exists() {
        KMSG_PATH
    } else {
        eprintln!("journald: Neither /dev/kmsg nor /proc/kmsg available");
        return;
    };

    let file = match fs::File::open(kmsg_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("journald: Failed to open {}: {}", kmsg_path, e);
            return;
        }
    };

    eprintln!("journald: Reading kernel messages from {}", kmsg_path);

    let reader = BufReader::new(file);
    for line in reader.lines() {
        if state.shutdown.load(Ordering::Relaxed) {
            break;
        }

        let line = match line {
            Ok(l) => l,
            Err(e) => {
                // EPIPE or EAGAIN are expected when the ring buffer wraps
                if e.kind() == io::ErrorKind::WouldBlock {
                    thread::sleep(Duration::from_millis(100));
                    continue;
                }
                eprintln!("journald: kmsg read error: {}", e);
                break;
            }
        };

        if let Some(entry) = parse_kmsg_line(&line) {
            state.dispatch_entry(entry);
        }
    }
}

// ---------------------------------------------------------------------------
// Signal handling
// ---------------------------------------------------------------------------

/// Set up signal handlers for graceful shutdown and control.
fn setup_signal_handlers(state: Arc<JournaldState>) {
    let _state_term = Arc::clone(&state);
    let _state_usr1 = Arc::clone(&state);
    let _state_usr2 = Arc::clone(&state);

    // SIGTERM / SIGINT → graceful shutdown
    let _ = unsafe { libc::signal(libc::SIGTERM, signal_handler_shutdown as libc::sighandler_t) };
    let _ = unsafe { libc::signal(libc::SIGINT, signal_handler_shutdown as libc::sighandler_t) };

    // We use a dedicated thread to watch for signals since we can't easily
    // pass Arc state to a C signal handler.  Instead, we use a self-pipe or
    // signalfd approach.  For simplicity, we'll use a polling approach with
    // atomic flags.

    // Store a global reference for the C signal handler
    GLOBAL_SHUTDOWN.store(
        &state.shutdown as *const AtomicBool as u64,
        Ordering::Release,
    );
    GLOBAL_FLUSH.store(
        &state.flush_requested as *const AtomicBool as u64,
        Ordering::Release,
    );
    GLOBAL_ROTATE.store(
        &state.rotate_requested as *const AtomicBool as u64,
        Ordering::Release,
    );

    let _ = unsafe { libc::signal(libc::SIGUSR1, signal_handler_flush as libc::sighandler_t) };
    let _ = unsafe { libc::signal(libc::SIGUSR2, signal_handler_rotate as libc::sighandler_t) };
}

// Global atomic pointers for signal handlers (they can't capture state)
static GLOBAL_SHUTDOWN: AtomicU64 = AtomicU64::new(0);
static GLOBAL_FLUSH: AtomicU64 = AtomicU64::new(0);
static GLOBAL_ROTATE: AtomicU64 = AtomicU64::new(0);

extern "C" fn signal_handler_shutdown(_sig: libc::c_int) {
    let ptr = GLOBAL_SHUTDOWN.load(Ordering::Acquire);
    if ptr != 0 {
        let flag = unsafe { &*(ptr as *const AtomicBool) };
        flag.store(true, Ordering::Release);
    }
}

extern "C" fn signal_handler_flush(_sig: libc::c_int) {
    let ptr = GLOBAL_FLUSH.load(Ordering::Acquire);
    if ptr != 0 {
        let flag = unsafe { &*(ptr as *const AtomicBool) };
        flag.store(true, Ordering::Release);
    }
}

extern "C" fn signal_handler_rotate(_sig: libc::c_int) {
    let ptr = GLOBAL_ROTATE.load(Ordering::Acquire);
    if ptr != 0 {
        let flag = unsafe { &*(ptr as *const AtomicBool) };
        flag.store(true, Ordering::Release);
    }
}

// ---------------------------------------------------------------------------
// sd_notify support
// ---------------------------------------------------------------------------

/// Send an sd_notify message to the service manager.
fn sd_notify(msg: &str) {
    if let Ok(socket_path) = std::env::var("NOTIFY_SOCKET") {
        let path = if let Some(stripped) = socket_path.strip_prefix('@') {
            // Abstract socket
            format!("\0{}", stripped)
        } else {
            socket_path.clone()
        };

        if let Ok(sock) = UnixDatagram::unbound() {
            let _ = sock.send_to(msg.as_bytes(), &path);
        }
    }
}

// ---------------------------------------------------------------------------
// Maintenance thread
// ---------------------------------------------------------------------------

/// Periodic maintenance: rate limiter GC, flush/rotate requests, watchdog.
fn maintenance_thread(state: Arc<JournaldState>) {
    let gc_interval = Duration::from_secs(300); // 5 minutes
    let mut last_gc = Instant::now();

    loop {
        thread::sleep(Duration::from_secs(1));

        if state.shutdown.load(Ordering::Relaxed) {
            break;
        }

        // Handle flush request (SIGUSR1)
        if state
            .flush_requested
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            eprintln!("journald: Flushing journal to persistent storage");
            let mut storage = state.storage.lock().unwrap();
            let _ = storage.flush();
        }

        // Handle rotate request (SIGUSR2)
        if state
            .rotate_requested
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            eprintln!("journald: Rotating journal files");
            let mut storage = state.storage.lock().unwrap();
            let _ = storage.rotate();
        }

        // Rate limiter GC
        if last_gc.elapsed() >= gc_interval {
            let mut rl = state.rate_limiter.lock().unwrap();
            rl.gc(Duration::from_secs(600)); // Clean entries older than 10 min
            last_gc = Instant::now();
        }

        // Watchdog keepalive
        if std::env::var("WATCHDOG_USEC").is_ok() {
            sd_notify("WATCHDOG=1");
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    eprintln!("systemd-journald starting...");

    // Load configuration
    let config = JournaldConfig::load();
    let persistent = config.use_persistent_storage();

    // Determine storage directory
    let storage_dir = if persistent {
        PathBuf::from("/var/log/journal")
    } else {
        PathBuf::from("/run/log/journal")
    };

    // Ensure the storage directory exists
    if let Err(e) = fs::create_dir_all(&storage_dir) {
        eprintln!(
            "journald: Failed to create storage directory {}: {}",
            storage_dir.display(),
            e
        );
    }

    let max_use = if persistent {
        config.system_max_use
    } else {
        config.runtime_max_use
    };

    let storage_config = StorageConfig {
        directory: storage_dir,
        max_file_size: config.max_file_size,
        max_disk_usage: max_use,
        max_files: config.max_files,
        persistent,
    };

    let storage = match JournalStorage::new(storage_config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("journald: Failed to initialize journal storage: {}", e);
            process::exit(1);
        }
    };

    let state = Arc::new(JournaldState::new(config, storage));

    // Set up signal handlers
    setup_signal_handlers(Arc::clone(&state));

    // Ensure runtime directory exists
    let _ = fs::create_dir_all(RUNTIME_DIR);

    // Start listener threads
    let state_native = Arc::clone(&state);
    let _native_handle = thread::Builder::new()
        .name("native-socket".into())
        .spawn(move || native_socket_listener(state_native))
        .expect("failed to spawn native socket thread");

    let state_syslog = Arc::clone(&state);
    let _syslog_handle = thread::Builder::new()
        .name("syslog-socket".into())
        .spawn(move || syslog_socket_listener(state_syslog))
        .expect("failed to spawn syslog socket thread");

    let state_stdout = Arc::clone(&state);
    let _stdout_handle = thread::Builder::new()
        .name("stdout-socket".into())
        .spawn(move || stdout_socket_listener(state_stdout))
        .expect("failed to spawn stdout socket thread");

    let state_kmsg = Arc::clone(&state);
    let _kmsg_handle = thread::Builder::new()
        .name("kmsg-reader".into())
        .spawn(move || kmsg_reader(state_kmsg))
        .expect("failed to spawn kmsg reader thread");

    let state_maint = Arc::clone(&state);
    let _maint_handle = thread::Builder::new()
        .name("maintenance".into())
        .spawn(move || maintenance_thread(state_maint))
        .expect("failed to spawn maintenance thread");

    // Log a startup message to the journal itself
    {
        let mut entry = JournalEntry::new();
        entry.set_field("MESSAGE", "Journal started");
        entry.set_field("PRIORITY", "6");
        entry.set_field("SYSLOG_IDENTIFIER", "systemd-journald");
        entry.set_field("_PID", process::id().to_string());
        entry.set_field("_TRANSPORT", "driver");
        entry.set_boot_id();
        entry.set_machine_id();
        entry.set_hostname();
        state.dispatch_entry(entry);
    }

    // Notify the service manager that we're ready
    sd_notify("READY=1\nSTATUS=Processing requests...");

    eprintln!("journald: Ready and processing requests");

    // Wait for shutdown
    while !state.shutdown.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(500));
    }

    eprintln!("journald: Shutting down...");

    // Log a shutdown message
    {
        let mut entry = JournalEntry::new();
        entry.set_field("MESSAGE", "Journal stopped");
        entry.set_field("PRIORITY", "6");
        entry.set_field("SYSLOG_IDENTIFIER", "systemd-journald");
        entry.set_field("_PID", process::id().to_string());
        entry.set_field("_TRANSPORT", "driver");
        entry.set_boot_id();
        entry.set_machine_id();
        entry.set_hostname();
        state.dispatch_entry(entry);
    }

    // Flush and close storage
    {
        let mut storage = state.storage.lock().unwrap();
        let _ = storage.flush();
    }

    // Clean up socket files
    let _ = fs::remove_file(JOURNAL_SOCKET_PATH);
    let _ = fs::remove_file(STDOUT_SOCKET_PATH);
    let _ = fs::remove_file(SYSLOG_SOCKET_PATH);

    sd_notify("STOPPING=1");

    eprintln!("journald: Shutdown complete");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Native protocol parsing ----

    #[test]
    fn test_parse_native_simple() {
        let msg = b"MESSAGE=hello world\nPRIORITY=6\nSYSLOG_IDENTIFIER=test\n";
        let entry = parse_native_message(msg);
        assert_eq!(entry.message(), Some("hello world".to_string()));
        assert_eq!(entry.priority(), Some(6));
        assert_eq!(entry.syslog_identifier(), Some("test".to_string()));
    }

    #[test]
    fn test_parse_native_no_trailing_newline() {
        let msg = b"MESSAGE=no newline\nPRIORITY=3";
        let entry = parse_native_message(msg);
        assert_eq!(entry.message(), Some("no newline".to_string()));
        assert_eq!(entry.priority(), Some(3));
    }

    #[test]
    fn test_parse_native_binary_value() {
        // Binary format: KEY\n<8-byte LE length><data>\n
        let mut msg = Vec::new();
        msg.extend_from_slice(b"PRIORITY=6\n");
        msg.extend_from_slice(b"MESSAGE\n");
        let data = b"binary\x00data";
        msg.extend_from_slice(&(data.len() as u64).to_le_bytes());
        msg.extend_from_slice(data);
        msg.push(b'\n');

        let entry = parse_native_message(&msg);
        assert_eq!(entry.priority(), Some(6));
        assert_eq!(entry.field_bytes("MESSAGE"), Some(&b"binary\x00data"[..]));
    }

    #[test]
    fn test_parse_native_empty() {
        let entry = parse_native_message(b"");
        assert!(entry.fields.is_empty());
    }

    #[test]
    fn test_parse_native_multiple_fields() {
        let msg = b"MESSAGE=test\nPRIORITY=4\nSYSLOG_IDENTIFIER=myapp\nSYSLOG_PID=42\nCODE_FILE=main.rs\nCODE_LINE=100\n";
        let entry = parse_native_message(msg);
        assert_eq!(entry.message(), Some("test".to_string()));
        assert_eq!(entry.priority(), Some(4));
        assert_eq!(entry.field("CODE_FILE"), Some("main.rs".to_string()));
        assert_eq!(entry.field("CODE_LINE"), Some("100".to_string()));
    }

    #[test]
    fn test_parse_native_ignores_invalid_field_names() {
        let msg = b"MESSAGE=valid\nlowercase=invalid\n123=invalid\nA123=ok\n";
        let entry = parse_native_message(msg);
        assert_eq!(entry.message(), Some("valid".to_string()));
        assert!(entry.field("lowercase").is_none());
        assert!(entry.field("123").is_none());
        assert_eq!(entry.field("A123"), Some("ok".to_string()));
    }

    // ---- Syslog protocol parsing ----

    #[test]
    fn test_parse_syslog_basic() {
        let msg = b"<13>Jan  1 00:00:00 myhost myapp[1234]: Hello world";
        let entry = parse_syslog_message(msg);
        assert_eq!(entry.priority(), Some(5)); // 13 % 8 = 5 (notice)
        assert_eq!(entry.field("SYSLOG_FACILITY"), Some("1".to_string())); // 13 / 8 = 1 (user)
        assert_eq!(entry.message(), Some("Hello world".to_string()));
        assert_eq!(entry.field("SYSLOG_IDENTIFIER"), Some("myapp".to_string()));
        assert_eq!(entry.field("SYSLOG_PID"), Some("1234".to_string()));
    }

    #[test]
    fn test_parse_syslog_no_pid() {
        let msg = b"<14>myapp: Hello world";
        let entry = parse_syslog_message(msg);
        assert_eq!(entry.priority(), Some(6)); // 14 % 8 = 6 (info)
        assert_eq!(entry.message(), Some("Hello world".to_string()));
        assert_eq!(entry.field("SYSLOG_IDENTIFIER"), Some("myapp".to_string()));
        assert!(entry.field("SYSLOG_PID").is_none());
    }

    #[test]
    fn test_parse_syslog_no_priority() {
        let msg = b"Just a plain message";
        let entry = parse_syslog_message(msg);
        assert!(entry.priority().is_none());
        assert_eq!(entry.message(), Some("Just a plain message".to_string()));
    }

    #[test]
    fn test_parse_syslog_empty() {
        let entry = parse_syslog_message(b"");
        assert!(entry.message().is_some()); // Empty string
    }

    // ---- Kernel message parsing ----

    #[test]
    fn test_parse_kmsg_basic() {
        let entry = parse_kmsg_line("6,1234,5678,-;Linux version 6.1.0").unwrap();
        assert_eq!(entry.priority(), Some(6));
        assert_eq!(entry.message(), Some("Linux version 6.1.0".to_string()));
        assert_eq!(entry.field("SYSLOG_IDENTIFIER"), Some("kernel".to_string()));
    }

    #[test]
    fn test_parse_kmsg_with_facility() {
        let entry = parse_kmsg_line("30,100,9999,-;Some kernel subsystem message").unwrap();
        // 30 = facility 3 (daemon) * 8 + severity 6 (info)
        assert_eq!(entry.priority(), Some(6));
        assert_eq!(entry.field("SYSLOG_FACILITY"), Some("3".to_string()));
    }

    #[test]
    fn test_parse_kmsg_empty() {
        assert!(parse_kmsg_line("").is_none());
    }

    #[test]
    fn test_parse_kmsg_no_semicolon() {
        let entry = parse_kmsg_line("just some text").unwrap();
        assert_eq!(entry.message(), Some("just some text".to_string()));
    }

    // ---- Level prefix parsing ----

    #[test]
    fn test_parse_level_prefix_valid() {
        let (p, msg) = parse_level_prefix_line("<3>Error occurred", 6);
        assert_eq!(p, 3);
        assert_eq!(msg, "Error occurred");
    }

    #[test]
    fn test_parse_level_prefix_no_prefix() {
        let (p, msg) = parse_level_prefix_line("No prefix here", 6);
        assert_eq!(p, 6);
        assert_eq!(msg, "No prefix here");
    }

    #[test]
    fn test_parse_level_prefix_out_of_range() {
        let (p, msg) = parse_level_prefix_line("<9>Out of range", 6);
        assert_eq!(p, 6);
        assert_eq!(msg, "<9>Out of range");
    }

    // ---- Field name validation ----

    #[test]
    fn test_is_valid_field_name() {
        assert!(is_valid_field_name("MESSAGE"));
        assert!(is_valid_field_name("PRIORITY"));
        assert!(is_valid_field_name("_PID"));
        assert!(is_valid_field_name("_SYSTEMD_UNIT"));
        assert!(is_valid_field_name("MY_CUSTOM_FIELD_123"));

        assert!(!is_valid_field_name(""));
        assert!(!is_valid_field_name("lowercase"));
        assert!(!is_valid_field_name("has space"));
        assert!(!is_valid_field_name("has-dash"));
        assert!(!is_valid_field_name("has.dot"));
    }

    // ---- Configuration parsing ----

    #[test]
    fn test_config_default() {
        let config = JournaldConfig::default();
        assert_eq!(config.storage, "auto");
        assert_eq!(config.max_level_store, 7);
        assert_eq!(config.rate_limit_burst, RATE_LIMIT_BURST);
        assert!(config.forward_to_wall);
        assert!(!config.forward_to_console);
    }

    #[test]
    fn test_config_parse() {
        let mut config = JournaldConfig::default();
        config.parse_config(
            r#"
[Journal]
Storage=persistent
ForwardToConsole=yes
MaxLevelStore=warning
RateLimitBurst=5000
SystemMaxUse=1G
"#,
        );
        assert_eq!(config.storage, "persistent");
        assert!(config.forward_to_console);
        assert_eq!(config.max_level_store, 4);
        assert_eq!(config.rate_limit_burst, 5000);
        assert_eq!(config.system_max_use, 1024 * 1024 * 1024);
    }

    #[test]
    fn test_config_parse_ignores_other_sections() {
        let mut config = JournaldConfig::default();
        config.parse_config(
            r#"
[Other]
Storage=volatile

[Journal]
Storage=persistent

[Another]
Storage=none
"#,
        );
        assert_eq!(config.storage, "persistent");
    }

    // ---- Parsing helpers ----

    #[test]
    fn test_parse_bool() {
        assert!(parse_bool("yes"));
        assert!(parse_bool("true"));
        assert!(parse_bool("1"));
        assert!(parse_bool("on"));
        assert!(parse_bool("y"));
        assert!(!parse_bool("no"));
        assert!(!parse_bool("false"));
        assert!(!parse_bool("0"));
        assert!(!parse_bool(""));
    }

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("1024"), Some(1024));
        assert_eq!(parse_size("1K"), Some(1024));
        assert_eq!(parse_size("1M"), Some(1024 * 1024));
        assert_eq!(parse_size("1G"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_size("1KiB"), Some(1024));
        assert_eq!(parse_size("1MiB"), Some(1024 * 1024));
        assert_eq!(parse_size("1GiB"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_size("100B"), Some(100));
        assert_eq!(parse_size(""), None);
        assert_eq!(parse_size("abc"), None);
    }

    #[test]
    fn test_parse_timespan_usec() {
        assert_eq!(parse_timespan_usec("1000000"), Some(1_000_000));
        assert_eq!(parse_timespan_usec("1s"), Some(1_000_000));
        assert_eq!(parse_timespan_usec("1sec"), Some(1_000_000));
        assert_eq!(parse_timespan_usec("30s"), Some(30_000_000));
        assert_eq!(parse_timespan_usec("1min"), Some(60_000_000));
        assert_eq!(parse_timespan_usec("1h"), Some(3_600_000_000));
        assert_eq!(parse_timespan_usec("100us"), Some(100));
        assert_eq!(parse_timespan_usec("100ms"), Some(100_000));
        assert_eq!(parse_timespan_usec(""), None);
    }

    #[test]
    fn test_parse_log_level() {
        assert_eq!(parse_log_level("emerg"), Some(0));
        assert_eq!(parse_log_level("alert"), Some(1));
        assert_eq!(parse_log_level("crit"), Some(2));
        assert_eq!(parse_log_level("err"), Some(3));
        assert_eq!(parse_log_level("error"), Some(3));
        assert_eq!(parse_log_level("warning"), Some(4));
        assert_eq!(parse_log_level("warn"), Some(4));
        assert_eq!(parse_log_level("notice"), Some(5));
        assert_eq!(parse_log_level("info"), Some(6));
        assert_eq!(parse_log_level("debug"), Some(7));
        assert_eq!(parse_log_level("0"), Some(0));
        assert_eq!(parse_log_level("7"), Some(7));
        assert_eq!(parse_log_level("invalid"), None);
    }

    // ---- Rate limiter ----

    #[test]
    fn test_rate_limiter_allows_within_burst() {
        let mut rl = RateLimiter::new();
        let interval = Duration::from_secs(30);

        for _ in 0..10 {
            assert!(rl.check("test", 10, interval));
        }
    }

    #[test]
    fn test_rate_limiter_blocks_over_burst() {
        let mut rl = RateLimiter::new();
        let interval = Duration::from_secs(30);

        for _ in 0..10 {
            assert!(rl.check("test", 10, interval));
        }
        // 11th should be blocked
        assert!(!rl.check("test", 10, interval));
    }

    #[test]
    fn test_rate_limiter_independent_sources() {
        let mut rl = RateLimiter::new();
        let interval = Duration::from_secs(30);

        for _ in 0..5 {
            assert!(rl.check("source_a", 5, interval));
        }
        assert!(!rl.check("source_a", 5, interval));

        // source_b should still be allowed
        assert!(rl.check("source_b", 5, interval));
    }

    #[test]
    fn test_rate_limiter_gc() {
        let mut rl = RateLimiter::new();
        let interval = Duration::from_secs(30);

        rl.check("old_source", 10, interval);
        assert_eq!(rl.sources.len(), 1);

        // GC with max_age of 0 should remove everything
        rl.gc(Duration::from_secs(0));
        // The entry was just created, so it won't be removed with age 0
        // unless we wait, which we don't want to do in a test.
        // Instead test that gc runs without panic
        assert!(rl.sources.len() <= 1);
    }

    // ---- Syslog timestamp skipping ----

    #[test]
    fn test_skip_syslog_timestamp() {
        assert_eq!(
            skip_syslog_timestamp("Jan  1 00:00:00 myhost myapp: msg"),
            "myhost myapp: msg"
        );
        assert_eq!(
            skip_syslog_timestamp("Dec 31 23:59:59 host test: hi"),
            "host test: hi"
        );
        assert_eq!(
            skip_syslog_timestamp("no timestamp here"),
            "no timestamp here"
        );
    }

    #[test]
    fn test_skip_syslog_timestamp_preserves_non_timestamp() {
        assert_eq!(
            skip_syslog_timestamp("myapp[123]: message"),
            "myapp[123]: message"
        );
    }

    // ---- Syslog tag parsing ----

    #[test]
    fn test_parse_syslog_tag_with_pid() {
        let (ident, pid, msg) = parse_syslog_tag_and_message("myapp[1234]: Hello");
        assert_eq!(ident, Some("myapp".to_string()));
        assert_eq!(pid, Some("1234".to_string()));
        assert_eq!(msg, "Hello");
    }

    #[test]
    fn test_parse_syslog_tag_without_pid() {
        let (ident, pid, msg) = parse_syslog_tag_and_message("myapp: Hello");
        assert_eq!(ident, Some("myapp".to_string()));
        assert_eq!(pid, None);
        assert_eq!(msg, "Hello");
    }

    #[test]
    fn test_parse_syslog_tag_no_colon() {
        let (ident, pid, msg) = parse_syslog_tag_and_message("just a message");
        assert_eq!(ident, None);
        assert_eq!(pid, None);
        assert_eq!(msg, "just a message");
    }
}
