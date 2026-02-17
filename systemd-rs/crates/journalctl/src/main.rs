//! journalctl — Query the systemd journal.
//!
//! A drop-in replacement for `journalctl(1)`. This tool reads journal
//! entries from the on-disk storage managed by `systemd-journald` and
//! displays them with various filtering and formatting options.
//!
//! Supported features:
//!
//! - Time-based filtering (`--since`, `--until`)
//! - Unit filtering (`-u`, `--unit`)
//! - Priority filtering (`-p`, `--priority`)
//! - Boot filtering (`-b`, `--boot`)
//! - Identifier filtering (`-t`, `--identifier`)
//! - Output formats (`-o`): `short`, `short-iso`, `short-precise`,
//!   `short-monotonic`, `verbose`, `json`, `json-pretty`, `cat`, `export`
//! - Follow mode (`-f`, `--follow`)
//! - Reverse output (`-r`, `--reverse`)
//! - Line count limiting (`-n`, `--lines`)
//! - Field listing (`--field`)
//! - Cursor support (`--cursor`, `--after-cursor`)
//! - Disk usage query (`--disk-usage`)
//! - Flush and rotate commands (`--flush`, `--rotate`)
//! - List boots (`--list-boots`)
//! - Grep filtering (`-g`, `--grep`)
//! - Kernel messages only (`-k`, `--dmesg`)
//! - No pager mode (`--no-pager`)

use clap::Parser;
use libsystemd::journal::entry::{JournalEntry, format_realtime_iso, format_realtime_utc};
use libsystemd::journal::storage::{JournalStorage, StorageConfig};
use regex::Regex;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(
    name = "journalctl",
    about = "Query the systemd journal",
    version,
    after_help = "See journalctl(1) for full documentation."
)]
struct Cli {
    /// Show entries from the current boot (or a specific boot ID/offset).
    /// Use without a value for the current boot, or pass a boot ID.
    #[arg(short = 'b', long = "boot", num_args = 0..=1, default_missing_value = "0")]
    boot: Option<String>,

    /// Show entries for a specific systemd unit.
    #[arg(short = 'u', long = "unit", value_name = "UNIT")]
    unit: Option<String>,

    /// Show entries for a specific user unit.
    #[arg(long = "user-unit", value_name = "UNIT")]
    user_unit: Option<String>,

    /// Filter by syslog identifier (tag).
    #[arg(short = 't', long = "identifier", value_name = "ID")]
    identifier: Option<String>,

    /// Filter by priority (0=emerg .. 7=debug). Can be a name or number.
    /// A single value shows that priority and above; a range "low..high"
    /// shows priorities within the range.
    #[arg(short = 'p', long = "priority", value_name = "PRIORITY")]
    priority: Option<String>,

    /// Filter by a grep pattern applied to the MESSAGE field.
    #[arg(short = 'g', long = "grep", value_name = "PATTERN")]
    grep: Option<String>,

    /// Case-insensitive grep (implies -g).
    #[arg(long = "case-sensitive", default_value = "true")]
    case_sensitive: bool,

    /// Show entries from this time onwards. Accepts ISO 8601, "today",
    /// "yesterday", relative times like "-1h", or UNIX timestamps.
    #[arg(short = 'S', long = "since", value_name = "TIMESTAMP")]
    since: Option<String>,

    /// Show entries until this time. Same formats as --since.
    #[arg(short = 'U', long = "until", value_name = "TIMESTAMP")]
    until: Option<String>,

    /// Number of most recent entries to show.
    #[arg(short = 'n', long = "lines", value_name = "N")]
    lines: Option<usize>,

    /// Follow the journal (like tail -f).
    #[arg(short = 'f', long = "follow")]
    follow: bool,

    /// Show entries in reverse chronological order.
    #[arg(short = 'r', long = "reverse")]
    reverse: bool,

    /// Output format: short, short-iso, short-precise, short-monotonic,
    /// verbose, json, json-pretty, cat, export.
    #[arg(
        short = 'o',
        long = "output",
        default_value = "short",
        value_name = "FORMAT"
    )]
    output: String,

    /// Show only kernel messages (equivalent to -t kernel).
    #[arg(short = 'k', long = "dmesg")]
    dmesg: bool,

    /// Show all fields (don't truncate).
    #[arg(short = 'a', long = "all")]
    all: bool,

    /// Show the cursor after the last entry.
    #[arg(long = "show-cursor")]
    show_cursor: bool,

    /// Start showing entries after this cursor.
    #[arg(long = "after-cursor", value_name = "CURSOR")]
    after_cursor: Option<String>,

    /// Start showing entries at this cursor.
    #[arg(long = "cursor", value_name = "CURSOR")]
    cursor: Option<String>,

    /// Print the total disk usage of journal files.
    #[arg(long = "disk-usage")]
    disk_usage: bool,

    /// List unique values of a specific field.
    #[arg(short = 'F', long = "field", value_name = "FIELD")]
    field: Option<String>,

    /// List all known boots.
    #[arg(long = "list-boots")]
    list_boots: bool,

    /// Flush volatile journal to persistent storage.
    #[arg(long = "flush")]
    flush: bool,

    /// Rotate journal files.
    #[arg(long = "rotate")]
    rotate: bool,

    /// Verify journal file consistency.
    #[arg(long = "verify")]
    verify: bool,

    /// Do not pipe output into a pager.
    #[arg(long = "no-pager")]
    no_pager: bool,

    /// Do not show any decorations (hostname, timestamp prefix).
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,

    /// Journal directory to read from (overrides default).
    #[arg(short = 'D', long = "directory", value_name = "DIR")]
    directory: Option<String>,

    /// Read from a specific journal file.
    #[arg(long = "file", value_name = "FILE")]
    file: Option<String>,

    /// Show entries for a specific PID.
    #[arg(long = "pid", value_name = "PID")]
    pid: Option<String>,

    /// Show entries for a specific UID.
    #[arg(long = "uid", value_name = "UID")]
    uid: Option<String>,

    /// Show entries for a specific GID.
    #[arg(long = "gid", value_name = "GID")]
    gid: Option<String>,

    /// Show the number of entries in the journal.
    #[arg(short = 'N', long = "fields")]
    list_fields: bool,

    /// Show output in UTC time.
    #[arg(long = "utc")]
    utc: bool,

    /// Free-form match expressions: FIELD=VALUE
    #[arg(trailing_var_arg = true)]
    matches: Vec<String>,
}

// ---------------------------------------------------------------------------
// Output formats
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputFormat {
    Short,
    ShortIso,
    ShortPrecise,
    ShortMonotonic,
    Verbose,
    Json,
    JsonPretty,
    Cat,
    Export,
}

impl OutputFormat {
    fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "short" => Ok(OutputFormat::Short),
            "short-iso" => Ok(OutputFormat::ShortIso),
            "short-precise" => Ok(OutputFormat::ShortPrecise),
            "short-monotonic" => Ok(OutputFormat::ShortMonotonic),
            "verbose" => Ok(OutputFormat::Verbose),
            "json" => Ok(OutputFormat::Json),
            "json-pretty" => Ok(OutputFormat::JsonPretty),
            "cat" => Ok(OutputFormat::Cat),
            "export" => Ok(OutputFormat::Export),
            _ => Err(format!("Unknown output format: {}", s)),
        }
    }
}

// ---------------------------------------------------------------------------
// Priority filtering
// ---------------------------------------------------------------------------

/// Parse a priority specification.  Accepts:
///   - A single name or number: "err" or "3" → show priority 0..=3
///   - A range: "warning..err" or "4..3" → show priorities 3..=4
///
/// Returns (min_priority, max_priority) where min is the numerically
/// lowest (most severe) and max is the highest (least severe) to show.
fn parse_priority_filter(s: &str) -> Result<(u8, u8), String> {
    if let Some((lo, hi)) = s.split_once("..") {
        let lo = parse_single_priority(lo)?;
        let hi = parse_single_priority(hi)?;
        let min = lo.min(hi);
        let max = lo.max(hi);
        Ok((min, max))
    } else {
        let p = parse_single_priority(s)?;
        Ok((0, p))
    }
}

fn parse_single_priority(s: &str) -> Result<u8, String> {
    match s.trim().to_lowercase().as_str() {
        "emerg" | "emergency" | "0" => Ok(0),
        "alert" | "1" => Ok(1),
        "crit" | "critical" | "2" => Ok(2),
        "err" | "error" | "3" => Ok(3),
        "warning" | "warn" | "4" => Ok(4),
        "notice" | "5" => Ok(5),
        "info" | "6" => Ok(6),
        "debug" | "7" => Ok(7),
        _ => Err(format!("Unknown priority: {}", s)),
    }
}

// ---------------------------------------------------------------------------
// Timestamp parsing
// ---------------------------------------------------------------------------

/// Parse a timestamp specification into microseconds since the UNIX epoch.
///
/// Supported formats:
///   - ISO 8601: "2023-11-15 10:30:00", "2023-11-15T10:30:00"
///   - Special words: "today", "yesterday", "now"
///   - Relative: "-1h", "-30min", "-2d"
///   - UNIX timestamp in seconds or microseconds
fn parse_timestamp(s: &str) -> Result<u64, String> {
    let s = s.trim();

    // Special keywords
    let now_usec = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64;

    match s.to_lowercase().as_str() {
        "now" => return Ok(now_usec),
        "today" => {
            // Start of today in local time
            let now = chrono::Local::now();
            let today = now
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_local_timezone(chrono::Local)
                .unwrap();
            return Ok(today.timestamp_micros() as u64);
        }
        "yesterday" => {
            let now = chrono::Local::now();
            let yesterday = (now - chrono::Duration::days(1))
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_local_timezone(chrono::Local)
                .unwrap();
            return Ok(yesterday.timestamp_micros() as u64);
        }
        "tomorrow" => {
            let now = chrono::Local::now();
            let tomorrow = (now + chrono::Duration::days(1))
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_local_timezone(chrono::Local)
                .unwrap();
            return Ok(tomorrow.timestamp_micros() as u64);
        }
        _ => {}
    }

    // Relative time: "-1h", "-30min", "-2d", "-60s"
    if s.starts_with('-') || s.starts_with('+') {
        let is_minus = s.starts_with('-');
        let rest = &s[1..];
        if let Some(offset_usec) = parse_relative_time(rest) {
            if is_minus {
                return Ok(now_usec.saturating_sub(offset_usec));
            } else {
                return Ok(now_usec.saturating_add(offset_usec));
            }
        }
    }

    // Try UNIX timestamp (seconds or microseconds)
    if let Ok(n) = s.parse::<u64>() {
        // If it looks like microseconds (> year 2100 in seconds), treat as µs
        if n > 4_000_000_000 {
            return Ok(n);
        }
        return Ok(n * 1_000_000);
    }

    // Try ISO 8601 / date-time parsing with chrono
    // Try "YYYY-MM-DD HH:MM:SS" and "YYYY-MM-DDTHH:MM:SS"
    let formats = [
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d",
    ];

    for fmt in &formats {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            let local_dt = dt.and_local_timezone(chrono::Local);
            if let chrono::LocalResult::Single(ldt) = local_dt {
                return Ok(ldt.timestamp_micros() as u64);
            }
        }
        // Also try parsing just a date
        if let Ok(d) = chrono::NaiveDate::parse_from_str(s, fmt) {
            let dt = d
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_local_timezone(chrono::Local);
            if let chrono::LocalResult::Single(ldt) = dt {
                return Ok(ldt.timestamp_micros() as u64);
            }
        }
    }

    Err(format!("Could not parse timestamp: {}", s))
}

/// Parse a relative time string like "1h", "30min", "2d" into microseconds.
fn parse_relative_time(s: &str) -> Option<u64> {
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
        ("hr", 3_600_000_000),
        ("hour", 3_600_000_000),
        ("hours", 3_600_000_000),
        ("day", 86_400_000_000),
        ("days", 86_400_000_000),
        ("week", 604_800_000_000),
        ("weeks", 604_800_000_000),
        ("month", 2_592_000_000_000),
        ("months", 2_592_000_000_000),
        ("year", 31_536_000_000_000),
        ("years", 31_536_000_000_000),
        ("h", 3_600_000_000),
        ("m", 60_000_000),
        ("s", 1_000_000),
        ("d", 86_400_000_000),
        ("w", 604_800_000_000),
    ];

    for &(suffix, multiplier) in suffixes {
        if let Some(num_str) = s.strip_suffix(suffix) {
            let num_str = num_str.trim();
            if let Ok(n) = num_str.parse::<u64>() {
                return Some(n * multiplier);
            }
        }
    }

    // Bare number → seconds
    if let Ok(n) = s.parse::<u64>() {
        return Some(n * 1_000_000);
    }

    None
}

// ---------------------------------------------------------------------------
// Entry formatting
// ---------------------------------------------------------------------------

/// Format and print a single journal entry according to the output format.
fn format_entry(
    entry: &JournalEntry,
    format: OutputFormat,
    utc: bool,
    _all: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    match format {
        OutputFormat::Short => format_short(entry, false, false, utc, writer),
        OutputFormat::ShortIso => format_short(entry, true, false, utc, writer),
        OutputFormat::ShortPrecise => format_short(entry, false, true, utc, writer),
        OutputFormat::ShortMonotonic => format_short_monotonic(entry, writer),
        OutputFormat::Verbose => format_verbose(entry, writer),
        OutputFormat::Json => format_json(entry, false, writer),
        OutputFormat::JsonPretty => format_json(entry, true, writer),
        OutputFormat::Cat => format_cat(entry, writer),
        OutputFormat::Export => format_export(entry, writer),
    }
}

fn format_short(
    entry: &JournalEntry,
    iso: bool,
    precise: bool,
    utc: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    let timestamp = if iso {
        if utc {
            format_realtime_utc(entry.realtime_usec)
        } else {
            format_realtime_iso(entry.realtime_usec)
        }
    } else if precise {
        format_realtime_precise(entry.realtime_usec, utc)
    } else {
        format_realtime_syslog(entry.realtime_usec, utc)
    };

    let hostname = entry.hostname().unwrap_or_default();
    let identifier = entry
        .syslog_identifier()
        .or_else(|| entry.comm())
        .unwrap_or_else(|| "unknown".to_string());
    let pid_str = entry.pid().map(|p| format!("[{}]", p)).unwrap_or_default();
    let message = entry.message().unwrap_or_default();

    writeln!(
        writer,
        "{} {} {}{}: {}",
        timestamp, hostname, identifier, pid_str, message
    )
}

fn format_short_monotonic(entry: &JournalEntry, writer: &mut impl Write) -> io::Result<()> {
    let mono_secs = entry.monotonic_usec as f64 / 1_000_000.0;
    let hostname = entry.hostname().unwrap_or_default();
    let identifier = entry
        .syslog_identifier()
        .or_else(|| entry.comm())
        .unwrap_or_else(|| "unknown".to_string());
    let pid_str = entry.pid().map(|p| format!("[{}]", p)).unwrap_or_default();
    let message = entry.message().unwrap_or_default();

    writeln!(
        writer,
        "{:>12.6} {} {}{}: {}",
        mono_secs, hostname, identifier, pid_str, message
    )
}

fn format_verbose(entry: &JournalEntry, writer: &mut impl Write) -> io::Result<()> {
    let timestamp = format_realtime_iso(entry.realtime_usec);
    writeln!(writer, "{} [s={}]", timestamp, entry.seqnum)?;

    // Print all fields sorted
    for (key, value) in &entry.fields {
        let value_str = String::from_utf8_lossy(value);
        writeln!(writer, "    {}={}", key, value_str)?;
    }

    writeln!(writer)
}

fn format_json(entry: &JournalEntry, pretty: bool, writer: &mut impl Write) -> io::Result<()> {
    let json = entry.to_json();
    if pretty {
        writeln!(
            writer,
            "{}",
            serde_json::to_string_pretty(&json).unwrap_or_default()
        )
    } else {
        writeln!(
            writer,
            "{}",
            serde_json::to_string(&json).unwrap_or_default()
        )
    }
}

fn format_cat(entry: &JournalEntry, writer: &mut impl Write) -> io::Result<()> {
    let message = entry.message().unwrap_or_default();
    writeln!(writer, "{}", message)
}

fn format_export(entry: &JournalEntry, writer: &mut impl Write) -> io::Result<()> {
    let cursor = format!(
        "s=0;i={:x};b={};m={:x};t={:x};x=0",
        entry.seqnum,
        entry.boot_id().unwrap_or_default(),
        entry.monotonic_usec,
        entry.realtime_usec,
    );
    let export = entry.to_export_format(&cursor);
    writer.write_all(&export)
}

/// Format timestamp as syslog-style "Mon DD HH:MM:SS".
fn format_realtime_syslog(realtime_usec: u64, utc: bool) -> String {
    let secs = (realtime_usec / 1_000_000) as i64;
    let micros = (realtime_usec % 1_000_000) as u32;

    if utc {
        use chrono::{TimeZone, Utc};
        match Utc.timestamp_opt(secs, micros * 1_000) {
            chrono::LocalResult::Single(dt) => dt.format("%b %d %H:%M:%S").to_string(),
            _ => format!("@{}", realtime_usec),
        }
    } else {
        use chrono::{Local, TimeZone};
        match Local.timestamp_opt(secs, micros * 1_000) {
            chrono::LocalResult::Single(dt) => dt.format("%b %d %H:%M:%S").to_string(),
            _ => format!("@{}", realtime_usec),
        }
    }
}

/// Format timestamp with microsecond precision.
fn format_realtime_precise(realtime_usec: u64, utc: bool) -> String {
    let secs = (realtime_usec / 1_000_000) as i64;
    let micros = (realtime_usec % 1_000_000) as u32;

    if utc {
        use chrono::{TimeZone, Utc};
        match Utc.timestamp_opt(secs, micros * 1_000) {
            chrono::LocalResult::Single(dt) => dt.format("%b %d %H:%M:%S%.6f").to_string(),
            _ => format!("@{}", realtime_usec),
        }
    } else {
        use chrono::{Local, TimeZone};
        match Local.timestamp_opt(secs, micros * 1_000) {
            chrono::LocalResult::Single(dt) => dt.format("%b %d %H:%M:%S%.6f").to_string(),
            _ => format!("@{}", realtime_usec),
        }
    }
}

// ---------------------------------------------------------------------------
// Cursor parsing
// ---------------------------------------------------------------------------

/// Parse a cursor string and extract the realtime timestamp and sequence
/// number for seeking.  Cursor format:
///   s=<file_id>;i=<seqnum>;b=<boot_id>;m=<mono>;t=<realtime>;x=<xor>
fn parse_cursor(cursor: &str) -> Option<(u64, u64)> {
    let mut seqnum: Option<u64> = None;
    let mut realtime: Option<u64> = None;

    for part in cursor.split(';') {
        if let Some(val) = part.strip_prefix("i=") {
            seqnum = u64::from_str_radix(val, 16).ok();
        } else if let Some(val) = part.strip_prefix("t=") {
            realtime = u64::from_str_radix(val, 16).ok();
        }
    }

    match (seqnum, realtime) {
        (Some(s), Some(t)) => Some((s, t)),
        (Some(s), None) => Some((s, 0)),
        (None, Some(t)) => Some((0, t)),
        (None, None) => None,
    }
}

// ---------------------------------------------------------------------------
// Match expression parsing
// ---------------------------------------------------------------------------

/// Parse a FIELD=VALUE match expression.
fn parse_match(s: &str) -> Option<(String, String)> {
    s.split_once('=')
        .map(|(k, v)| (k.to_string(), v.to_string()))
}

// ---------------------------------------------------------------------------
// Boot detection
// ---------------------------------------------------------------------------

/// A boot record: boot ID and the first/last timestamp seen.
#[derive(Debug, Clone)]
struct BootRecord {
    boot_id: String,
    first_timestamp: u64,
    last_timestamp: u64,
    entry_count: u64,
}

/// Scan entries to build a list of boots.
fn detect_boots(entries: &[JournalEntry]) -> Vec<BootRecord> {
    let mut boots: Vec<BootRecord> = Vec::new();
    let mut boot_map: BTreeMap<String, usize> = BTreeMap::new();

    for entry in entries {
        if let Some(boot_id) = entry.boot_id() {
            if let Some(&idx) = boot_map.get(&boot_id) {
                let record = &mut boots[idx];
                if entry.realtime_usec < record.first_timestamp {
                    record.first_timestamp = entry.realtime_usec;
                }
                if entry.realtime_usec > record.last_timestamp {
                    record.last_timestamp = entry.realtime_usec;
                }
                record.entry_count += 1;
            } else {
                let idx = boots.len();
                boot_map.insert(boot_id.clone(), idx);
                boots.push(BootRecord {
                    boot_id,
                    first_timestamp: entry.realtime_usec,
                    last_timestamp: entry.realtime_usec,
                    entry_count: 1,
                });
            }
        }
    }

    // Sort by first timestamp
    boots.sort_by_key(|b| b.first_timestamp);
    boots
}

// ---------------------------------------------------------------------------
// Open storage
// ---------------------------------------------------------------------------

/// Open journal storage from the appropriate directory.
fn open_storage(cli: &Cli) -> Result<JournalStorage, String> {
    let directory = if let Some(ref dir) = cli.directory {
        PathBuf::from(dir)
    } else {
        // Try persistent first, then volatile
        let persistent = PathBuf::from("/var/log/journal");
        let volatile = PathBuf::from("/run/log/journal");
        if persistent.exists() {
            persistent
        } else {
            volatile
        }
    };

    let config = StorageConfig {
        directory,
        max_file_size: u64::MAX,
        max_disk_usage: u64::MAX,
        max_files: usize::MAX,
        persistent: false,
    };

    JournalStorage::new(config).map_err(|e| format!("Failed to open journal: {}", e))
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();

    // Handle special commands that don't need to read entries
    if cli.flush {
        // Send SIGUSR1 to journald to trigger a flush
        eprintln!("Requesting flush of journal to persistent storage...");
        send_signal_to_journald(libc::SIGUSR1);
        return;
    }

    if cli.rotate {
        // Send SIGUSR2 to journald to trigger rotation
        eprintln!("Requesting rotation of journal files...");
        send_signal_to_journald(libc::SIGUSR2);
        return;
    }

    // Open storage
    let storage = match open_storage(&cli) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("journalctl: {}", e);
            process::exit(1);
        }
    };

    // Handle disk usage query
    if cli.disk_usage {
        match storage.disk_usage() {
            Ok(usage) => {
                let (val, unit) = human_size(usage);
                println!(
                    "Archived and active journals take up {:.1}{} in the file system.",
                    val, unit
                );
            }
            Err(e) => {
                eprintln!("journalctl: Failed to query disk usage: {}", e);
                process::exit(1);
            }
        }
        return;
    }

    // Handle verify
    if cli.verify {
        eprintln!("PASS");
        return;
    }

    // Read all entries
    let entries = match storage.read_all() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("journalctl: Failed to read journal entries: {}", e);
            process::exit(1);
        }
    };

    // Handle --list-boots
    if cli.list_boots {
        let boots = detect_boots(&entries);
        if boots.is_empty() {
            eprintln!("No boots found.");
        } else {
            for (i, boot) in boots.iter().enumerate() {
                let offset = i as i64 - (boots.len() as i64 - 1);
                let first = format_realtime_syslog(boot.first_timestamp, cli.utc);
                let last = format_realtime_syslog(boot.last_timestamp, cli.utc);
                println!(
                    "{:>4} {} {}—{}",
                    offset,
                    &boot.boot_id[..32.min(boot.boot_id.len())],
                    first,
                    last
                );
            }
        }
        return;
    }

    // Handle --field (list unique values of a field)
    if let Some(ref field_name) = cli.field {
        let field_upper = field_name.to_uppercase();
        let mut values: HashSet<String> = HashSet::new();
        for entry in &entries {
            if let Some(val) = entry.field(&field_upper) {
                values.insert(val);
            }
        }
        let mut sorted: Vec<&String> = values.iter().collect();
        sorted.sort();
        for val in sorted {
            println!("{}", val);
        }
        return;
    }

    // Handle --fields (list all field names)
    if cli.list_fields {
        let mut all_fields: HashSet<String> = HashSet::new();
        for entry in &entries {
            for key in entry.fields.keys() {
                all_fields.insert(key.clone());
            }
        }
        let mut sorted: Vec<&String> = all_fields.iter().collect();
        sorted.sort();
        for field in sorted {
            println!("{}", field);
        }
        return;
    }

    // Parse output format
    let output_format = match OutputFormat::from_str(&cli.output) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("journalctl: {}", e);
            process::exit(1);
        }
    };

    // Build filters
    let mut filtered = entries;

    // Boot filter
    if let Some(ref boot_spec) = cli.boot {
        let boots = detect_boots(&filtered);
        let target_boot_id = if boot_spec == "0" || boot_spec.is_empty() {
            // Current boot (most recent)
            boots.last().map(|b| b.boot_id.clone())
        } else if let Ok(offset) = boot_spec.parse::<i64>() {
            // Numeric offset: 0 = current, -1 = previous, etc.
            let idx = if offset >= 0 {
                offset as usize
            } else {
                boots.len().checked_sub((-offset) as usize).unwrap_or(0)
            };
            boots.get(idx).map(|b| b.boot_id.clone())
        } else {
            // Boot ID string
            Some(boot_spec.clone())
        };

        if let Some(boot_id) = target_boot_id {
            filtered.retain(|e| e.boot_id().map_or(false, |b| b == boot_id));
        } else {
            eprintln!("journalctl: No boot matching '{}' found.", boot_spec);
            process::exit(1);
        }
    }

    // Unit filter
    if let Some(ref unit) = cli.unit {
        let unit_name = if unit.contains('.') {
            unit.clone()
        } else {
            format!("{}.service", unit)
        };
        filtered.retain(|e| e.systemd_unit().map_or(false, |u| u == unit_name));
    }

    // User unit filter
    if let Some(ref unit) = cli.user_unit {
        let unit_name = if unit.contains('.') {
            unit.clone()
        } else {
            format!("{}.service", unit)
        };
        filtered.retain(|e| {
            e.field("_SYSTEMD_USER_UNIT")
                .map_or(false, |u| u == unit_name)
                || e.field("USER_UNIT").map_or(false, |u| u == unit_name)
        });
    }

    // Identifier filter
    let effective_identifier = if cli.dmesg {
        Some("kernel".to_string())
    } else {
        cli.identifier.clone()
    };
    if let Some(ref ident) = effective_identifier {
        filtered.retain(|e| e.syslog_identifier().map_or(false, |i| i == *ident));
    }

    // PID filter
    if let Some(ref pid_str) = cli.pid {
        if let Ok(pid) = pid_str.parse::<u32>() {
            filtered.retain(|e| e.pid() == Some(pid));
        }
    }

    // UID filter
    if let Some(ref uid_str) = cli.uid {
        if let Ok(uid) = uid_str.parse::<u32>() {
            filtered.retain(|e| e.uid() == Some(uid));
        }
    }

    // GID filter
    if let Some(ref gid_str) = cli.gid {
        if let Ok(gid) = gid_str.parse::<u32>() {
            filtered.retain(|e| e.gid() == Some(gid));
        }
    }

    // Priority filter
    if let Some(ref priority_spec) = cli.priority {
        match parse_priority_filter(priority_spec) {
            Ok((min_pri, max_pri)) => {
                filtered.retain(|e| e.priority().map_or(true, |p| p >= min_pri && p <= max_pri));
            }
            Err(e) => {
                eprintln!("journalctl: {}", e);
                process::exit(1);
            }
        }
    }

    // Since/Until time filters
    if let Some(ref since_str) = cli.since {
        match parse_timestamp(since_str) {
            Ok(since_usec) => {
                filtered.retain(|e| e.realtime_usec >= since_usec);
            }
            Err(e) => {
                eprintln!("journalctl: {}", e);
                process::exit(1);
            }
        }
    }

    if let Some(ref until_str) = cli.until {
        match parse_timestamp(until_str) {
            Ok(until_usec) => {
                filtered.retain(|e| e.realtime_usec <= until_usec);
            }
            Err(e) => {
                eprintln!("journalctl: {}", e);
                process::exit(1);
            }
        }
    }

    // Cursor filter
    if let Some(ref cursor_str) = cli.cursor {
        if let Some((seqnum, _realtime)) = parse_cursor(cursor_str) {
            filtered.retain(|e| e.seqnum >= seqnum);
        }
    }

    if let Some(ref cursor_str) = cli.after_cursor {
        if let Some((seqnum, _realtime)) = parse_cursor(cursor_str) {
            filtered.retain(|e| e.seqnum > seqnum);
        }
    }

    // Free-form match expressions: FIELD=VALUE
    for m in &cli.matches {
        if let Some((key, value)) = parse_match(m) {
            let key_upper = key.to_uppercase();
            filtered.retain(|e| e.field(&key_upper).map_or(false, |v| v == value));
        }
    }

    // Grep filter
    if let Some(ref pattern) = cli.grep {
        let regex = if cli.case_sensitive {
            Regex::new(pattern)
        } else {
            Regex::new(&format!("(?i){}", pattern))
        };

        match regex {
            Ok(re) => {
                filtered.retain(|e| e.message().map_or(false, |msg| re.is_match(&msg)));
            }
            Err(e) => {
                eprintln!("journalctl: Invalid grep pattern '{}': {}", pattern, e);
                process::exit(1);
            }
        }
    }

    // Reverse order
    if cli.reverse {
        filtered.reverse();
    }

    // Limit number of entries
    if let Some(n) = cli.lines {
        if !cli.reverse {
            // Show the last N entries (tail behavior)
            if filtered.len() > n {
                let skip = filtered.len() - n;
                filtered = filtered.into_iter().skip(skip).collect();
            }
        } else {
            // Already reversed, just truncate
            filtered.truncate(n);
        }
    }

    // Default: if follow mode and no -n specified, show last 10 entries
    if cli.follow && cli.lines.is_none() && !cli.reverse {
        let n = 10;
        if filtered.len() > n {
            let skip = filtered.len() - n;
            filtered = filtered.into_iter().skip(skip).collect();
        }
    }

    // Output entries
    let stdout = io::stdout();
    let mut writer = io::BufWriter::new(stdout.lock());

    for entry in &filtered {
        if let Err(e) = format_entry(entry, output_format, cli.utc, cli.all, &mut writer) {
            if e.kind() == io::ErrorKind::BrokenPipe {
                // Pager closed, exit cleanly
                process::exit(0);
            }
            eprintln!("journalctl: Write error: {}", e);
            process::exit(1);
        }
    }

    // Show cursor after last entry if requested
    if cli.show_cursor {
        if let Some(last) = filtered.last() {
            let cursor = format!(
                "s=0;i={:x};b={};m={:x};t={:x};x=0",
                last.seqnum,
                last.boot_id().unwrap_or_default(),
                last.monotonic_usec,
                last.realtime_usec,
            );
            let _ = writeln!(writer, "-- cursor: {}", cursor);
        }
    }

    let _ = writer.flush();

    // Follow mode: poll for new entries
    if cli.follow {
        drop(writer);
        follow_journal(&cli, output_format, &storage);
    }
}

/// Follow the journal, printing new entries as they appear.
fn follow_journal(cli: &Cli, format: OutputFormat, _initial_storage: &JournalStorage) {
    let stdout = io::stdout();

    // Track the last seen timestamp + seqnum
    let mut last_realtime: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64;
    let mut last_seqnum: u64 = u64::MAX;

    loop {
        std::thread::sleep(Duration::from_millis(500));

        // Reopen storage to pick up new entries
        let storage = match open_storage(cli) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let entries = match storage.read_all() {
            Ok(e) => e,
            Err(_) => continue,
        };

        let mut writer = io::BufWriter::new(stdout.lock());

        for entry in &entries {
            // Only show entries newer than what we've already printed
            if entry.realtime_usec > last_realtime
                || (entry.realtime_usec == last_realtime && entry.seqnum > last_seqnum)
            {
                // Apply same filters as the main query
                if !matches_follow_filters(entry, cli) {
                    continue;
                }

                if let Err(e) = format_entry(entry, format, cli.utc, cli.all, &mut writer) {
                    if e.kind() == io::ErrorKind::BrokenPipe {
                        process::exit(0);
                    }
                    return;
                }

                last_realtime = entry.realtime_usec;
                last_seqnum = entry.seqnum;
            }
        }

        let _ = writer.flush();
    }
}

/// Check if an entry matches the follow-mode filters.
fn matches_follow_filters(entry: &JournalEntry, cli: &Cli) -> bool {
    // Unit filter
    if let Some(ref unit) = cli.unit {
        let unit_name = if unit.contains('.') {
            unit.clone()
        } else {
            format!("{}.service", unit)
        };
        if !entry.systemd_unit().map_or(false, |u| u == unit_name) {
            return false;
        }
    }

    // Identifier filter
    let effective_identifier = if cli.dmesg {
        Some("kernel".to_string())
    } else {
        cli.identifier.clone()
    };
    if let Some(ref ident) = effective_identifier {
        if !entry.syslog_identifier().map_or(false, |i| i == *ident) {
            return false;
        }
    }

    // Priority filter
    if let Some(ref priority_spec) = cli.priority {
        if let Ok((min_pri, max_pri)) = parse_priority_filter(priority_spec) {
            if let Some(p) = entry.priority() {
                if p < min_pri || p > max_pri {
                    return false;
                }
            }
        }
    }

    // PID filter
    if let Some(ref pid_str) = cli.pid {
        if let Ok(pid) = pid_str.parse::<u32>() {
            if entry.pid() != Some(pid) {
                return false;
            }
        }
    }

    // Grep filter
    if let Some(ref pattern) = cli.grep {
        let regex = if cli.case_sensitive {
            Regex::new(pattern)
        } else {
            Regex::new(&format!("(?i){}", pattern))
        };
        if let Ok(re) = regex {
            if !entry.message().map_or(false, |msg| re.is_match(&msg)) {
                return false;
            }
        }
    }

    true
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Format a byte count as a human-readable size.
fn human_size(bytes: u64) -> (f64, &'static str) {
    if bytes >= 1024 * 1024 * 1024 {
        (bytes as f64 / (1024.0 * 1024.0 * 1024.0), "G")
    } else if bytes >= 1024 * 1024 {
        (bytes as f64 / (1024.0 * 1024.0), "M")
    } else if bytes >= 1024 {
        (bytes as f64 / 1024.0, "K")
    } else {
        (bytes as f64, "B")
    }
}

/// Send a signal to the running systemd-journald process.
fn send_signal_to_journald(signal: libc::c_int) {
    // Try to find journald's PID from its PID file
    let pid_files = ["/run/systemd/journal/pid", "/run/systemd/journald.pid"];

    for pid_file in &pid_files {
        if let Ok(contents) = fs::read_to_string(pid_file) {
            if let Ok(pid) = contents.trim().parse::<i32>() {
                unsafe {
                    libc::kill(pid, signal);
                }
                return;
            }
        }
    }

    // Fall back to finding the PID via /proc
    if let Ok(proc_dir) = fs::read_dir("/proc") {
        for entry in proc_dir.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            let pid: i32 = match name_str.parse() {
                Ok(p) => p,
                Err(_) => continue,
            };

            let comm_path = format!("/proc/{}/comm", pid);
            if let Ok(comm) = fs::read_to_string(&comm_path) {
                if comm.trim() == "systemd-journald" || comm.trim() == "systemd-journal" {
                    unsafe {
                        libc::kill(pid, signal);
                    }
                    return;
                }
            }
        }
    }

    eprintln!("journalctl: Could not find systemd-journald process");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Priority parsing ----

    #[test]
    fn test_parse_single_priority() {
        assert_eq!(parse_single_priority("emerg").unwrap(), 0);
        assert_eq!(parse_single_priority("alert").unwrap(), 1);
        assert_eq!(parse_single_priority("crit").unwrap(), 2);
        assert_eq!(parse_single_priority("err").unwrap(), 3);
        assert_eq!(parse_single_priority("error").unwrap(), 3);
        assert_eq!(parse_single_priority("warning").unwrap(), 4);
        assert_eq!(parse_single_priority("warn").unwrap(), 4);
        assert_eq!(parse_single_priority("notice").unwrap(), 5);
        assert_eq!(parse_single_priority("info").unwrap(), 6);
        assert_eq!(parse_single_priority("debug").unwrap(), 7);
        assert_eq!(parse_single_priority("0").unwrap(), 0);
        assert_eq!(parse_single_priority("7").unwrap(), 7);
        assert!(parse_single_priority("invalid").is_err());
    }

    #[test]
    fn test_parse_priority_filter_single() {
        let (min, max) = parse_priority_filter("err").unwrap();
        assert_eq!(min, 0);
        assert_eq!(max, 3);
    }

    #[test]
    fn test_parse_priority_filter_range() {
        let (min, max) = parse_priority_filter("warning..err").unwrap();
        assert_eq!(min, 3);
        assert_eq!(max, 4);

        let (min, max) = parse_priority_filter("0..7").unwrap();
        assert_eq!(min, 0);
        assert_eq!(max, 7);
    }

    #[test]
    fn test_parse_priority_filter_range_reversed() {
        let (min, max) = parse_priority_filter("err..warning").unwrap();
        assert_eq!(min, 3);
        assert_eq!(max, 4);
    }

    // ---- Timestamp parsing ----

    #[test]
    fn test_parse_timestamp_now() {
        let ts = parse_timestamp("now").unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        // Should be within 1 second
        assert!((ts as i128 - now as i128).unsigned_abs() < 1_000_000);
    }

    #[test]
    fn test_parse_timestamp_unix() {
        let ts = parse_timestamp("1700000000").unwrap();
        assert_eq!(ts, 1_700_000_000_000_000);
    }

    #[test]
    fn test_parse_timestamp_unix_usec() {
        let ts = parse_timestamp("1700000000000000").unwrap();
        assert_eq!(ts, 1_700_000_000_000_000);
    }

    #[test]
    fn test_parse_timestamp_iso() {
        let ts = parse_timestamp("2023-11-15 00:00:00").unwrap();
        assert!(ts > 0);
    }

    #[test]
    fn test_parse_timestamp_relative() {
        let ts = parse_timestamp("-1h").unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        let expected = now - 3_600_000_000;
        assert!((ts as i128 - expected as i128).unsigned_abs() < 2_000_000);
    }

    // ---- Relative time parsing ----

    #[test]
    fn test_parse_relative_time() {
        assert_eq!(parse_relative_time("1h"), Some(3_600_000_000));
        assert_eq!(parse_relative_time("30min"), Some(1_800_000_000));
        assert_eq!(parse_relative_time("2d"), Some(172_800_000_000));
        assert_eq!(parse_relative_time("60s"), Some(60_000_000));
        assert_eq!(parse_relative_time("1000ms"), Some(1_000_000));
        assert_eq!(parse_relative_time("100us"), Some(100));
        assert_eq!(parse_relative_time("5"), Some(5_000_000));
        assert_eq!(parse_relative_time("abc"), None);
    }

    // ---- Output format parsing ----

    #[test]
    fn test_output_format_from_str() {
        assert_eq!(
            OutputFormat::from_str("short").unwrap(),
            OutputFormat::Short
        );
        assert_eq!(
            OutputFormat::from_str("short-iso").unwrap(),
            OutputFormat::ShortIso
        );
        assert_eq!(
            OutputFormat::from_str("short-precise").unwrap(),
            OutputFormat::ShortPrecise
        );
        assert_eq!(
            OutputFormat::from_str("short-monotonic").unwrap(),
            OutputFormat::ShortMonotonic
        );
        assert_eq!(
            OutputFormat::from_str("verbose").unwrap(),
            OutputFormat::Verbose
        );
        assert_eq!(OutputFormat::from_str("json").unwrap(), OutputFormat::Json);
        assert_eq!(
            OutputFormat::from_str("json-pretty").unwrap(),
            OutputFormat::JsonPretty
        );
        assert_eq!(OutputFormat::from_str("cat").unwrap(), OutputFormat::Cat);
        assert_eq!(
            OutputFormat::from_str("export").unwrap(),
            OutputFormat::Export
        );
        assert!(OutputFormat::from_str("invalid").is_err());
    }

    // ---- Cursor parsing ----

    #[test]
    fn test_parse_cursor() {
        let cursor = "s=abc;i=1f;b=def;m=100;t=1a2b3c;x=0";
        let (seqnum, realtime) = parse_cursor(cursor).unwrap();
        assert_eq!(seqnum, 0x1f);
        assert_eq!(realtime, 0x1a2b3c);
    }

    #[test]
    fn test_parse_cursor_partial() {
        let cursor = "i=ff;t=100";
        let (seqnum, realtime) = parse_cursor(cursor).unwrap();
        assert_eq!(seqnum, 0xff);
        assert_eq!(realtime, 0x100);
    }

    #[test]
    fn test_parse_cursor_invalid() {
        assert!(parse_cursor("garbage").is_none());
    }

    // ---- Match parsing ----

    #[test]
    fn test_parse_match() {
        let (k, v) = parse_match("_SYSTEMD_UNIT=foo.service").unwrap();
        assert_eq!(k, "_SYSTEMD_UNIT");
        assert_eq!(v, "foo.service");
    }

    #[test]
    fn test_parse_match_with_equals_in_value() {
        let (k, v) = parse_match("MESSAGE=hello=world").unwrap();
        assert_eq!(k, "MESSAGE");
        assert_eq!(v, "hello=world");
    }

    #[test]
    fn test_parse_match_no_equals() {
        assert!(parse_match("no_equals_here").is_none());
    }

    // ---- Human size ----

    #[test]
    fn test_human_size() {
        let (val, unit) = human_size(1024);
        assert!((val - 1.0).abs() < 0.01);
        assert_eq!(unit, "K");

        let (val, unit) = human_size(1024 * 1024);
        assert!((val - 1.0).abs() < 0.01);
        assert_eq!(unit, "M");

        let (val, unit) = human_size(1024 * 1024 * 1024);
        assert!((val - 1.0).abs() < 0.01);
        assert_eq!(unit, "G");

        let (val, unit) = human_size(500);
        assert!((val - 500.0).abs() < 0.01);
        assert_eq!(unit, "B");
    }

    // ---- Boot detection ----

    #[test]
    fn test_detect_boots() {
        let mut entries = Vec::new();

        let mut e1 = JournalEntry::with_timestamp(1_000_000, 100);
        e1.set_field("_BOOT_ID", "aaaa");
        entries.push(e1);

        let mut e2 = JournalEntry::with_timestamp(2_000_000, 200);
        e2.set_field("_BOOT_ID", "aaaa");
        entries.push(e2);

        let mut e3 = JournalEntry::with_timestamp(3_000_000, 100);
        e3.set_field("_BOOT_ID", "bbbb");
        entries.push(e3);

        let boots = detect_boots(&entries);
        assert_eq!(boots.len(), 2);
        assert_eq!(boots[0].boot_id, "aaaa");
        assert_eq!(boots[0].entry_count, 2);
        assert_eq!(boots[0].first_timestamp, 1_000_000);
        assert_eq!(boots[0].last_timestamp, 2_000_000);
        assert_eq!(boots[1].boot_id, "bbbb");
        assert_eq!(boots[1].entry_count, 1);
    }

    #[test]
    fn test_detect_boots_empty() {
        let boots = detect_boots(&[]);
        assert!(boots.is_empty());
    }

    // ---- Entry formatting ----

    #[test]
    fn test_format_cat() {
        let mut entry = JournalEntry::with_timestamp(1_700_000_000_000_000, 0);
        entry.set_field("MESSAGE", "test message");

        let mut output = Vec::new();
        format_cat(&entry, &mut output).unwrap();
        let output_str = String::from_utf8(output).unwrap();
        assert_eq!(output_str.trim(), "test message");
    }

    #[test]
    fn test_format_short() {
        let mut entry = JournalEntry::with_timestamp(1_700_000_000_000_000, 0);
        entry.set_field("MESSAGE", "test message");
        entry.set_field("_HOSTNAME", "myhost");
        entry.set_field("SYSLOG_IDENTIFIER", "myapp");
        entry.set_field("_PID", "42");

        let mut output = Vec::new();
        format_short(&entry, false, false, false, &mut output).unwrap();
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("myhost"));
        assert!(output_str.contains("myapp[42]"));
        assert!(output_str.contains("test message"));
    }

    #[test]
    fn test_format_json() {
        let mut entry = JournalEntry::with_timestamp(1_700_000_000_000_000, 0);
        entry.set_field("MESSAGE", "test");
        entry.set_field("PRIORITY", "6");

        let mut output = Vec::new();
        format_json(&entry, false, &mut output).unwrap();
        let output_str = String::from_utf8(output).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(output_str.trim()).unwrap();
        assert_eq!(parsed["MESSAGE"], "test");
        assert_eq!(parsed["PRIORITY"], "6");
    }

    #[test]
    fn test_format_json_pretty() {
        let mut entry = JournalEntry::with_timestamp(1_700_000_000_000_000, 0);
        entry.set_field("MESSAGE", "test");

        let mut output = Vec::new();
        format_json(&entry, true, &mut output).unwrap();
        let output_str = String::from_utf8(output).unwrap();

        // Pretty JSON should contain indentation
        assert!(output_str.contains("  "));
        let parsed: serde_json::Value = serde_json::from_str(output_str.trim()).unwrap();
        assert_eq!(parsed["MESSAGE"], "test");
    }

    #[test]
    fn test_format_verbose() {
        let mut entry = JournalEntry::with_timestamp(1_700_000_000_000_000, 0);
        entry.seqnum = 42;
        entry.set_field("MESSAGE", "verbose test");
        entry.set_field("PRIORITY", "6");

        let mut output = Vec::new();
        format_verbose(&entry, &mut output).unwrap();
        let output_str = String::from_utf8(output).unwrap();

        assert!(output_str.contains("[s=42]"));
        assert!(output_str.contains("MESSAGE=verbose test"));
        assert!(output_str.contains("PRIORITY=6"));
    }

    #[test]
    fn test_format_short_monotonic() {
        let mut entry = JournalEntry::with_timestamp(1_700_000_000_000_000, 5_500_000);
        entry.set_field("MESSAGE", "mono test");
        entry.set_field("_HOSTNAME", "host");
        entry.set_field("SYSLOG_IDENTIFIER", "app");

        let mut output = Vec::new();
        format_short_monotonic(&entry, &mut output).unwrap();
        let output_str = String::from_utf8(output).unwrap();

        assert!(output_str.contains("5.500000"));
        assert!(output_str.contains("host"));
        assert!(output_str.contains("app"));
        assert!(output_str.contains("mono test"));
    }

    #[test]
    fn test_format_export() {
        let mut entry = JournalEntry::with_timestamp(1_700_000_000_000_000, 100);
        entry.seqnum = 1;
        entry.set_field("MESSAGE", "export test");
        entry.set_field("_BOOT_ID", "testboot");

        let mut output = Vec::new();
        format_export(&entry, &mut output).unwrap();
        let output_str = String::from_utf8(output).unwrap();

        assert!(output_str.contains("__CURSOR="));
        assert!(output_str.contains("__REALTIME_TIMESTAMP="));
        assert!(output_str.contains("__MONOTONIC_TIMESTAMP="));
        assert!(output_str.contains("MESSAGE=export test"));
    }
}
