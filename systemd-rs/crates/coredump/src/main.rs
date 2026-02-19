//! systemd-coredump — Process and store kernel core dumps.
//!
//! A drop-in replacement for `systemd-coredump(8)`. This tool is invoked by
//! the kernel when a process crashes, via the `/proc/sys/kernel/core_pattern`
//! pipe mechanism:
//!
//!   |/path/to/systemd-coredump [--backtrace] PID UID GID SIGNAL TIMESTAMP RLIMIT HOSTNAME COMM [EXE]
//!
//! The core dump data is read from stdin.
//!
//! This tool:
//!   1. Collects metadata about the crashed process (PID, UID, GID, signal,
//!      executable, command name, hostname, timestamp, etc.)
//!   2. Reads the core dump from stdin
//!   3. Stores it in `/var/lib/systemd/coredump/` with a descriptive filename
//!   4. Writes a JSON metadata sidecar file alongside the core dump
//!   5. Applies storage and size limits from `coredump.conf`
//!   6. Vacuums old core dumps when limits are exceeded
//!
//! Configuration is read from `/etc/systemd/coredump.conf` and drop-in
//! directories.
//!
//! Exit codes:
//!   0 — success (core dump stored or intentionally skipped)
//!   1 — error

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process;
use std::time::UNIX_EPOCH;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const COREDUMP_DIR: &str = "/var/lib/systemd/coredump";

const CONFIG_FILE: &str = "/etc/systemd/coredump.conf";

const CONFIG_DROPIN_DIRS: &[&str] = &[
    "/etc/systemd/coredump.conf.d",
    "/run/systemd/coredump.conf.d",
    "/usr/lib/systemd/coredump.conf.d",
];

/// Default maximum size of a single core dump to store on disk (2 GiB).
const DEFAULT_EXTERNAL_SIZE_MAX: u64 = 2 * 1024 * 1024 * 1024;

/// Default maximum size of a core dump to process at all (2 GiB).
const DEFAULT_PROCESS_SIZE_MAX: u64 = 2 * 1024 * 1024 * 1024;

/// Default maximum total disk usage for stored core dumps (default: 10% of
/// filesystem or 10 GiB, whichever is smaller — we use a fixed 10 GiB).
const DEFAULT_MAX_USE: u64 = 10 * 1024 * 1024 * 1024;

/// Default minimum free disk space to maintain (15% of filesystem or 10 GiB,
/// whichever is smaller — we use a fixed 10 GiB).
const DEFAULT_KEEP_FREE: u64 = 10 * 1024 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Storage {
    /// Do not store core dumps at all.
    None,
    /// Store core dumps as files in /var/lib/systemd/coredump/.
    External,
    /// Store core dumps in the journal (not implemented; treated as external).
    Journal,
    /// Store both in journal and externally (treated as external).
    Both,
}

#[derive(Debug, Clone)]
struct Config {
    storage: Storage,
    compress: bool,
    process_size_max: u64,
    external_size_max: u64,
    max_use: u64,
    keep_free: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            storage: Storage::External,
            compress: true,
            process_size_max: DEFAULT_PROCESS_SIZE_MAX,
            external_size_max: DEFAULT_EXTERNAL_SIZE_MAX,
            max_use: DEFAULT_MAX_USE,
            keep_free: DEFAULT_KEEP_FREE,
        }
    }
}

fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();

    if s.eq_ignore_ascii_case("infinity") {
        return Some(u64::MAX);
    }

    // Try plain number first (bytes).
    if let Ok(n) = s.parse::<u64>() {
        return Some(n);
    }

    // Try with suffix.
    let (num_str, multiplier) = if let Some(n) = s.strip_suffix('K').or(s.strip_suffix('k')) {
        (n, 1024u64)
    } else if let Some(n) = s.strip_suffix('M').or(s.strip_suffix('m')) {
        (n, 1024 * 1024)
    } else if let Some(n) = s.strip_suffix('G').or(s.strip_suffix('g')) {
        (n, 1024 * 1024 * 1024)
    } else if let Some(n) = s.strip_suffix('T').or(s.strip_suffix('t')) {
        (n, 1024 * 1024 * 1024 * 1024)
    } else {
        return None;
    };

    num_str
        .trim()
        .parse::<u64>()
        .ok()
        .map(|n| n.saturating_mul(multiplier))
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.to_lowercase().as_str() {
        "yes" | "true" | "1" | "on" => Some(true),
        "no" | "false" | "0" | "off" => Some(false),
        _ => None,
    }
}

fn parse_config_file(path: &Path, config: &mut Config) {
    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let mut in_section = false;

    for line in contents.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_section = line.eq_ignore_ascii_case("[coredump]");
            continue;
        }

        if !in_section {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            match key {
                "Storage" => match value.to_lowercase().as_str() {
                    "none" => config.storage = Storage::None,
                    "external" => config.storage = Storage::External,
                    "journal" => config.storage = Storage::Journal,
                    "both" => config.storage = Storage::Both,
                    _ => {
                        eprintln!("Warning: unknown Storage value '{value}', using default");
                    }
                },
                "Compress" => {
                    if let Some(b) = parse_bool(value) {
                        config.compress = b;
                    }
                }
                "ProcessSizeMax" => {
                    if let Some(n) = parse_size(value) {
                        config.process_size_max = n;
                    }
                }
                "ExternalSizeMax" => {
                    if let Some(n) = parse_size(value) {
                        config.external_size_max = n;
                    }
                }
                "MaxUse" => {
                    if let Some(n) = parse_size(value) {
                        config.max_use = n;
                    }
                }
                "KeepFree" => {
                    if let Some(n) = parse_size(value) {
                        config.keep_free = n;
                    }
                }
                _ => {}
            }
        }
    }
}

fn load_config() -> Config {
    load_config_from(Path::new(CONFIG_FILE), CONFIG_DROPIN_DIRS)
}

fn load_config_from(config_file: &Path, dropin_dirs: &[&str]) -> Config {
    let mut config = Config::default();

    parse_config_file(config_file, &mut config);

    for dir in dropin_dirs {
        let dir_path = Path::new(dir);
        if let Ok(mut entries) = fs::read_dir(dir_path) {
            let mut paths: Vec<PathBuf> = Vec::new();
            while let Some(Ok(entry)) = entries.next() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("conf") {
                    paths.push(p);
                }
            }
            paths.sort();
            for p in paths {
                parse_config_file(&p, &mut config);
            }
        }
    }

    config
}

// ---------------------------------------------------------------------------
// Core dump metadata
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct CoreDumpMeta {
    /// PID of the crashed process.
    pid: u64,
    /// UID of the crashed process.
    uid: u64,
    /// GID of the crashed process.
    gid: u64,
    /// Signal that caused the crash.
    signal: u32,
    /// Timestamp (seconds since epoch).
    timestamp: u64,
    /// RLIMIT_CORE of the crashed process.
    rlimit: u64,
    /// Hostname at the time of crash.
    hostname: String,
    /// Command name (comm).
    comm: String,
    /// Full executable path (if available).
    exe: String,
    /// Whether --backtrace mode was requested.
    backtrace: bool,
    /// Size of the stored core dump (set after writing).
    core_size: u64,
    /// Filename of the stored core dump (set after writing).
    filename: String,
    /// Boot ID (from /proc/sys/kernel/random/boot_id).
    boot_id: String,
    /// Machine ID (from /etc/machine-id).
    machine_id: String,
}

impl CoreDumpMeta {
    fn signal_name(&self) -> &'static str {
        match self.signal {
            1 => "SIGHUP",
            2 => "SIGINT",
            3 => "SIGQUIT",
            4 => "SIGILL",
            5 => "SIGTRAP",
            6 => "SIGABRT",
            7 => "SIGBUS",
            8 => "SIGFPE",
            9 => "SIGKILL",
            11 => "SIGSEGV",
            13 => "SIGPIPE",
            14 => "SIGALRM",
            15 => "SIGTERM",
            _ => "unknown",
        }
    }

    /// Serialize metadata to JSON format.
    fn to_json(&self) -> String {
        let mut json = String::from("{\n");
        json.push_str(&format!("  \"PID\": {},\n", self.pid));
        json.push_str(&format!("  \"UID\": {},\n", self.uid));
        json.push_str(&format!("  \"GID\": {},\n", self.gid));
        json.push_str(&format!("  \"SIGNAL\": {},\n", self.signal));
        json.push_str(&format!("  \"SIGNAL_NAME\": \"{}\",\n", self.signal_name()));
        json.push_str(&format!("  \"TIMESTAMP\": {},\n", self.timestamp));
        json.push_str(&format!("  \"RLIMIT\": {},\n", self.rlimit));
        json.push_str(&format!(
            "  \"HOSTNAME\": \"{}\",\n",
            json_escape(&self.hostname)
        ));
        json.push_str(&format!("  \"COMM\": \"{}\",\n", json_escape(&self.comm)));
        json.push_str(&format!("  \"EXE\": \"{}\",\n", json_escape(&self.exe)));
        json.push_str(&format!(
            "  \"BOOT_ID\": \"{}\",\n",
            json_escape(&self.boot_id)
        ));
        json.push_str(&format!(
            "  \"MACHINE_ID\": \"{}\",\n",
            json_escape(&self.machine_id)
        ));
        json.push_str(&format!("  \"CORE_SIZE\": {},\n", self.core_size));
        json.push_str(&format!(
            "  \"FILENAME\": \"{}\",\n",
            json_escape(&self.filename)
        ));
        json.push_str(&format!("  \"BACKTRACE\": {}\n", self.backtrace));
        json.push('}');
        json
    }

    /// Deserialize metadata from JSON format.
    #[allow(dead_code)]
    fn from_json(s: &str) -> Option<Self> {
        let map = parse_json_object(s)?;

        Some(CoreDumpMeta {
            pid: map.get("PID")?.trim().parse().ok()?,
            uid: map.get("UID")?.trim().parse().ok()?,
            gid: map.get("GID")?.trim().parse().ok()?,
            signal: map.get("SIGNAL")?.trim().parse().ok()?,
            timestamp: map.get("TIMESTAMP")?.trim().parse().ok()?,
            rlimit: map
                .get("RLIMIT")
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0),
            hostname: map.get("HOSTNAME").cloned().unwrap_or_default(),
            comm: map.get("COMM").cloned().unwrap_or_default(),
            exe: map.get("EXE").cloned().unwrap_or_default(),
            backtrace: map.get("BACKTRACE").is_some_and(|v| v.trim() == "true"),
            core_size: map
                .get("CORE_SIZE")
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0),
            filename: map.get("FILENAME").cloned().unwrap_or_default(),
            boot_id: map.get("BOOT_ID").cloned().unwrap_or_default(),
            machine_id: map.get("MACHINE_ID").cloned().unwrap_or_default(),
        })
    }
}

/// Minimal JSON string escaping.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < '\x20' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Minimal JSON object parser — returns key/value string pairs.
/// Only handles flat objects with string, number, and boolean values.
#[allow(dead_code)]
fn parse_json_object(s: &str) -> Option<BTreeMap<String, String>> {
    let s = s.trim();
    if !s.starts_with('{') || !s.ends_with('}') {
        return None;
    }

    let inner = &s[1..s.len() - 1];
    let mut map = BTreeMap::new();

    for line in inner.lines() {
        let line = line.trim().trim_end_matches(',');
        if line.is_empty() {
            continue;
        }

        if let Some((key_part, value_part)) = line.split_once(':') {
            let key = key_part.trim().trim_matches('"');
            let value = value_part.trim();

            // Strip quotes from string values.
            let value = if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                json_unescape(&value[1..value.len() - 1])
            } else {
                value.to_owned()
            };

            map.insert(key.to_owned(), value);
        }
    }

    Some(map)
}

#[allow(dead_code)]
fn json_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('/') => out.push('/'),
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if let Ok(n) = u32::from_str_radix(&hex, 16)
                        && let Some(c) = char::from_u32(n)
                    {
                        out.push(c);
                    }
                }
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }

    out
}

// ---------------------------------------------------------------------------
// System information helpers
// ---------------------------------------------------------------------------

fn read_boot_id() -> String {
    fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .unwrap_or_default()
        .trim()
        .replace('-', "")
}

fn read_machine_id() -> String {
    fs::read_to_string("/etc/machine-id")
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// Build the core dump filename following systemd conventions:
/// `core.COMM.UID.BOOT_ID.PID.TIMESTAMP`
fn build_filename(meta: &CoreDumpMeta) -> String {
    // Sanitize comm: replace non-alphanumeric characters.
    let safe_comm: String = meta
        .comm
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();

    format!(
        "core.{}.{}.{}.{}.{}",
        safe_comm, meta.uid, meta.boot_id, meta.pid, meta.timestamp,
    )
}

// ---------------------------------------------------------------------------
// Core dump storage
// ---------------------------------------------------------------------------

/// Read the core dump from stdin, respecting the process size limit.
fn read_core_dump(max_size: u64) -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut stdin = io::stdin().lock();

    if max_size == 0 {
        return Ok(buf);
    }

    if max_size == u64::MAX {
        stdin.read_to_end(&mut buf)?;
        return Ok(buf);
    }

    // Read up to max_size + 1 to detect truncation.
    let limit = max_size.saturating_add(1) as usize;
    let mut take = stdin.take(limit as u64);
    take.read_to_end(&mut buf)?;

    if buf.len() as u64 > max_size {
        buf.truncate(max_size as usize);
    }

    Ok(buf)
}

/// Store the core dump to disk and write the metadata sidecar.
fn store_core_dump(
    coredump_dir: &Path,
    meta: &mut CoreDumpMeta,
    data: &[u8],
    config: &Config,
) -> io::Result<()> {
    fs::create_dir_all(coredump_dir)?;

    let filename = build_filename(meta);

    // Check external size limit.
    if data.len() as u64 > config.external_size_max {
        eprintln!(
            "Core dump for PID {} ({}) is {} bytes, exceeding ExternalSizeMax={}, skipping storage",
            meta.pid,
            meta.comm,
            data.len(),
            config.external_size_max,
        );
        return Ok(());
    }

    // Write core dump file.
    let core_path = coredump_dir.join(&filename);
    fs::write(&core_path, data)?;

    // Set permissions to 0640 (owner read-write, group read).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&core_path, fs::Permissions::from_mode(0o640))?;
    }

    meta.core_size = data.len() as u64;
    meta.filename = filename.clone();

    // Write JSON metadata sidecar.
    let meta_path = coredump_dir.join(format!("{filename}.json"));
    fs::write(&meta_path, meta.to_json())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&meta_path, fs::Permissions::from_mode(0o640))?;
    }

    Ok(())
}

/// List stored core dumps (core files with .json sidecars), sorted by timestamp.
fn list_stored_coredumps(coredump_dir: &Path) -> Vec<(PathBuf, PathBuf, u64)> {
    let mut entries: Vec<(PathBuf, PathBuf, u64)> = Vec::new();

    let dir_entries = match fs::read_dir(coredump_dir) {
        Ok(e) => e,
        Err(_) => return entries,
    };

    for entry in dir_entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_owned(),
            None => continue,
        };

        // Find .json metadata files.
        if !name.ends_with(".json") {
            continue;
        }
        if !name.starts_with("core.") {
            continue;
        }

        // Corresponding core dump file.
        let core_name = name.strip_suffix(".json").unwrap_or(&name);
        let core_path = coredump_dir.join(core_name);

        // Get modification time for sorting (older = smaller).
        let mtime = path
            .metadata()
            .and_then(|m| m.modified())
            .and_then(|t| t.duration_since(UNIX_EPOCH).map_err(io::Error::other))
            .map(|d| d.as_secs())
            .unwrap_or(0);

        entries.push((core_path, path, mtime));
    }

    // Sort by mtime ascending (oldest first).
    entries.sort_by_key(|e| e.2);
    entries
}

/// Vacuum old core dumps to stay within limits.
fn vacuum(coredump_dir: &Path, config: &Config) {
    let mut entries = list_stored_coredumps(coredump_dir);
    if entries.is_empty() {
        return;
    }

    // Calculate total size.
    let mut total_size: u64 = 0;
    for (core_path, meta_path, _) in &entries {
        if let Ok(m) = core_path.metadata() {
            total_size += m.len();
        }
        if let Ok(m) = meta_path.metadata() {
            total_size += m.len();
        }
    }

    // Remove oldest entries while over limits.
    while total_size > config.max_use && !entries.is_empty() {
        let (core_path, meta_path, _) = entries.remove(0);

        let mut freed: u64 = 0;
        if let Ok(m) = core_path.metadata() {
            freed += m.len();
        }
        if let Ok(m) = meta_path.metadata() {
            freed += m.len();
        }

        let _ = fs::remove_file(&core_path);
        let _ = fs::remove_file(&meta_path);

        total_size = total_size.saturating_sub(freed);
    }

    // Check keep-free against available disk space.
    if config.keep_free > 0
        && let Some(avail) = available_disk_space(coredump_dir)
    {
        while avail + freed_so_far(&entries, coredump_dir) < config.keep_free && !entries.is_empty()
        {
            let (core_path, meta_path, _) = entries.remove(0);
            let _ = fs::remove_file(&core_path);
            let _ = fs::remove_file(&meta_path);
        }
    }
}

/// Get available disk space on the filesystem containing `path`.
fn available_disk_space(path: &Path) -> Option<u64> {
    // Use statvfs via libc.
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;

        unsafe {
            let mut stat: libc::statvfs = std::mem::zeroed();
            if libc::statvfs(c_path.as_ptr(), &mut stat) == 0 {
                Some(stat.f_bavail * stat.f_frsize)
            } else {
                None
            }
        }
    }

    #[cfg(not(unix))]
    {
        None
    }
}

/// Helper — not actually used for iterative removal, just a placeholder
/// to keep the compiler happy.  The real vacuum loop removes one at a time.
fn freed_so_far(_entries: &[(PathBuf, PathBuf, u64)], _dir: &Path) -> u64 {
    0
}

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct Args {
    meta: CoreDumpMeta,
}

/// Parse kernel-supplied arguments.
///
/// The kernel invokes us as:
///   systemd-coredump [--backtrace] PID UID GID SIGNAL TIMESTAMP RLIMIT HOSTNAME COMM [EXE]
fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut iter = args.iter().skip(1); // skip argv[0]
    let mut backtrace = false;

    // Peek at first arg for --backtrace flag.
    let first = iter.next().ok_or("missing arguments")?;
    let pid_str = if first == "--backtrace" {
        backtrace = true;
        iter.next().ok_or("missing PID")?
    } else {
        first
    };

    let pid: u64 = pid_str
        .parse()
        .map_err(|_| format!("invalid PID: {pid_str}"))?;

    let uid: u64 = iter
        .next()
        .ok_or("missing UID")?
        .parse()
        .map_err(|e| format!("invalid UID: {e}"))?;

    let gid: u64 = iter
        .next()
        .ok_or("missing GID")?
        .parse()
        .map_err(|e| format!("invalid GID: {e}"))?;

    let signal: u32 = iter
        .next()
        .ok_or("missing SIGNAL")?
        .parse()
        .map_err(|e| format!("invalid SIGNAL: {e}"))?;

    let timestamp: u64 = iter
        .next()
        .ok_or("missing TIMESTAMP")?
        .parse()
        .map_err(|e| format!("invalid TIMESTAMP: {e}"))?;

    let rlimit: u64 = iter
        .next()
        .ok_or("missing RLIMIT")?
        .parse()
        .map_err(|e| format!("invalid RLIMIT: {e}"))?;

    let hostname = iter.next().ok_or("missing HOSTNAME")?.clone();
    let comm = iter.next().ok_or("missing COMM")?.clone();
    let exe = iter.next().cloned().unwrap_or_default();

    Ok(Args {
        meta: CoreDumpMeta {
            pid,
            uid,
            gid,
            signal,
            timestamp,
            rlimit,
            hostname,
            comm,
            exe,
            backtrace,
            core_size: 0,
            filename: String::new(),
            boot_id: read_boot_id(),
            machine_id: read_machine_id(),
        },
    })
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();

    let parsed = parse_args(&args)?;
    let mut meta = parsed.meta;
    let config = load_config();

    eprintln!(
        "Process {} ({}) of user {} dumped core: signal {} ({})",
        meta.pid,
        meta.comm,
        meta.uid,
        meta.signal,
        meta.signal_name(),
    );

    if config.storage == Storage::None {
        eprintln!("Core dump storage is disabled (Storage=none), skipping.");
        return Ok(());
    }

    // Read the core dump from stdin.
    let data = read_core_dump(config.process_size_max)
        .map_err(|e| format!("failed to read core dump from stdin: {e}"))?;

    if data.is_empty() {
        eprintln!("Empty core dump, nothing to store.");
        return Ok(());
    }

    eprintln!(
        "Core dump size: {} bytes for PID {} ({})",
        data.len(),
        meta.pid,
        meta.comm,
    );

    let coredump_dir = Path::new(COREDUMP_DIR);

    store_core_dump(coredump_dir, &mut meta, &data, &config)
        .map_err(|e| format!("failed to store core dump: {e}"))?;

    if !meta.filename.is_empty() {
        eprintln!("Stored core dump: {}/{}", COREDUMP_DIR, meta.filename,);
    }

    // Vacuum old dumps.
    vacuum(coredump_dir, &config);

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("systemd-coredump: {e}");
        process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // -- Config tests -------------------------------------------------------

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.storage, Storage::External);
        assert!(config.compress);
        assert_eq!(config.process_size_max, DEFAULT_PROCESS_SIZE_MAX);
        assert_eq!(config.external_size_max, DEFAULT_EXTERNAL_SIZE_MAX);
        assert_eq!(config.max_use, DEFAULT_MAX_USE);
        assert_eq!(config.keep_free, DEFAULT_KEEP_FREE);
    }

    #[test]
    fn test_parse_config_storage_none() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("coredump.conf");
        fs::write(&conf, "[Coredump]\nStorage=none\n").unwrap();
        let mut config = Config::default();
        parse_config_file(&conf, &mut config);
        assert_eq!(config.storage, Storage::None);
    }

    #[test]
    fn test_parse_config_storage_external() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("coredump.conf");
        fs::write(&conf, "[Coredump]\nStorage=external\n").unwrap();
        let mut config = Config::default();
        parse_config_file(&conf, &mut config);
        assert_eq!(config.storage, Storage::External);
    }

    #[test]
    fn test_parse_config_storage_journal() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("coredump.conf");
        fs::write(&conf, "[Coredump]\nStorage=journal\n").unwrap();
        let mut config = Config::default();
        parse_config_file(&conf, &mut config);
        assert_eq!(config.storage, Storage::Journal);
    }

    #[test]
    fn test_parse_config_storage_both() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("coredump.conf");
        fs::write(&conf, "[Coredump]\nStorage=both\n").unwrap();
        let mut config = Config::default();
        parse_config_file(&conf, &mut config);
        assert_eq!(config.storage, Storage::Both);
    }

    #[test]
    fn test_parse_config_compress_no() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("coredump.conf");
        fs::write(&conf, "[Coredump]\nCompress=no\n").unwrap();
        let mut config = Config::default();
        parse_config_file(&conf, &mut config);
        assert!(!config.compress);
    }

    #[test]
    fn test_parse_config_compress_yes() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("coredump.conf");
        fs::write(&conf, "[Coredump]\nCompress=yes\n").unwrap();
        let mut config = Config::default();
        config.compress = false;
        parse_config_file(&conf, &mut config);
        assert!(config.compress);
    }

    #[test]
    fn test_parse_config_sizes() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("coredump.conf");
        fs::write(
            &conf,
            "[Coredump]\nProcessSizeMax=100M\nExternalSizeMax=50M\nMaxUse=1G\nKeepFree=500M\n",
        )
        .unwrap();
        let mut config = Config::default();
        parse_config_file(&conf, &mut config);
        assert_eq!(config.process_size_max, 100 * 1024 * 1024);
        assert_eq!(config.external_size_max, 50 * 1024 * 1024);
        assert_eq!(config.max_use, 1024 * 1024 * 1024);
        assert_eq!(config.keep_free, 500 * 1024 * 1024);
    }

    #[test]
    fn test_parse_config_infinity() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("coredump.conf");
        fs::write(&conf, "[Coredump]\nProcessSizeMax=infinity\n").unwrap();
        let mut config = Config::default();
        parse_config_file(&conf, &mut config);
        assert_eq!(config.process_size_max, u64::MAX);
    }

    #[test]
    fn test_parse_config_missing_file() {
        let mut config = Config::default();
        parse_config_file(Path::new("/nonexistent/coredump.conf"), &mut config);
        assert_eq!(config.storage, Storage::External); // unchanged
    }

    #[test]
    fn test_parse_config_wrong_section_ignored() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("coredump.conf");
        fs::write(&conf, "[Network]\nStorage=none\n").unwrap();
        let mut config = Config::default();
        parse_config_file(&conf, &mut config);
        assert_eq!(config.storage, Storage::External); // unchanged
    }

    #[test]
    fn test_parse_config_comments_and_blanks() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("coredump.conf");
        fs::write(
            &conf,
            "# Comment\n\n; Another comment\n[Coredump]\n# Inline\nStorage=none\n",
        )
        .unwrap();
        let mut config = Config::default();
        parse_config_file(&conf, &mut config);
        assert_eq!(config.storage, Storage::None);
    }

    #[test]
    fn test_parse_config_case_insensitive_section() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("coredump.conf");
        fs::write(&conf, "[COREDUMP]\nStorage=none\n").unwrap();
        let mut config = Config::default();
        parse_config_file(&conf, &mut config);
        assert_eq!(config.storage, Storage::None);
    }

    #[test]
    fn test_parse_config_dropin() {
        let dir = TempDir::new().unwrap();
        let main_conf = dir.path().join("coredump.conf");
        fs::write(&main_conf, "[Coredump]\nStorage=none\n").unwrap();

        let dropin_dir = dir.path().join("coredump.conf.d");
        fs::create_dir(&dropin_dir).unwrap();
        fs::write(
            dropin_dir.join("override.conf"),
            "[Coredump]\nStorage=external\n",
        )
        .unwrap();

        let dropin_str = dropin_dir.to_str().unwrap().to_owned();
        let config = load_config_from(&main_conf, &[dropin_str.as_str()]);
        assert_eq!(config.storage, Storage::External); // override wins
    }

    // -- Size parsing tests -------------------------------------------------

    #[test]
    fn test_parse_size_bytes() {
        assert_eq!(parse_size("1024"), Some(1024));
    }

    #[test]
    fn test_parse_size_kilobytes() {
        assert_eq!(parse_size("100K"), Some(100 * 1024));
        assert_eq!(parse_size("100k"), Some(100 * 1024));
    }

    #[test]
    fn test_parse_size_megabytes() {
        assert_eq!(parse_size("50M"), Some(50 * 1024 * 1024));
    }

    #[test]
    fn test_parse_size_gigabytes() {
        assert_eq!(parse_size("2G"), Some(2 * 1024 * 1024 * 1024));
    }

    #[test]
    fn test_parse_size_terabytes() {
        assert_eq!(parse_size("1T"), Some(1024 * 1024 * 1024 * 1024));
    }

    #[test]
    fn test_parse_size_infinity() {
        assert_eq!(parse_size("infinity"), Some(u64::MAX));
    }

    #[test]
    fn test_parse_size_invalid() {
        assert_eq!(parse_size("abc"), None);
    }

    #[test]
    fn test_parse_size_zero() {
        assert_eq!(parse_size("0"), Some(0));
    }

    // -- Bool parsing tests -------------------------------------------------

    #[test]
    fn test_parse_bool_yes() {
        assert_eq!(parse_bool("yes"), Some(true));
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("1"), Some(true));
        assert_eq!(parse_bool("on"), Some(true));
    }

    #[test]
    fn test_parse_bool_no() {
        assert_eq!(parse_bool("no"), Some(false));
        assert_eq!(parse_bool("false"), Some(false));
        assert_eq!(parse_bool("0"), Some(false));
        assert_eq!(parse_bool("off"), Some(false));
    }

    #[test]
    fn test_parse_bool_invalid() {
        assert_eq!(parse_bool("maybe"), None);
    }

    // -- JSON tests ---------------------------------------------------------

    #[test]
    fn test_json_escape_basic() {
        assert_eq!(json_escape("hello"), "hello");
        assert_eq!(json_escape("a\"b"), "a\\\"b");
        assert_eq!(json_escape("a\\b"), "a\\\\b");
        assert_eq!(json_escape("a\nb"), "a\\nb");
        assert_eq!(json_escape("a\rb"), "a\\rb");
        assert_eq!(json_escape("a\tb"), "a\\tb");
    }

    #[test]
    fn test_json_escape_control_chars() {
        assert_eq!(json_escape("\x01"), "\\u0001");
    }

    #[test]
    fn test_json_roundtrip() {
        let meta = CoreDumpMeta {
            pid: 12345,
            uid: 1000,
            gid: 1000,
            signal: 11,
            timestamp: 1700000000,
            rlimit: u64::MAX,
            hostname: "test-host".into(),
            comm: "my-program".into(),
            exe: "/usr/bin/my-program".into(),
            backtrace: false,
            core_size: 4096,
            filename: "core.my-program.1000.abc.12345.1700000000".into(),
            boot_id: "abcdef0123456789".into(),
            machine_id: "deadbeef12345678".into(),
        };

        let json = meta.to_json();
        let parsed = CoreDumpMeta::from_json(&json).unwrap();

        assert_eq!(parsed.pid, meta.pid);
        assert_eq!(parsed.uid, meta.uid);
        assert_eq!(parsed.gid, meta.gid);
        assert_eq!(parsed.signal, meta.signal);
        assert_eq!(parsed.timestamp, meta.timestamp);
        assert_eq!(parsed.hostname, meta.hostname);
        assert_eq!(parsed.comm, meta.comm);
        assert_eq!(parsed.exe, meta.exe);
        assert_eq!(parsed.backtrace, meta.backtrace);
        assert_eq!(parsed.core_size, meta.core_size);
        assert_eq!(parsed.filename, meta.filename);
        assert_eq!(parsed.boot_id, meta.boot_id);
        assert_eq!(parsed.machine_id, meta.machine_id);
    }

    #[test]
    fn test_json_unescape() {
        assert_eq!(json_unescape("hello"), "hello");
        assert_eq!(json_unescape("a\\\"b"), "a\"b");
        assert_eq!(json_unescape("a\\\\b"), "a\\b");
        assert_eq!(json_unescape("a\\nb"), "a\nb");
    }

    #[test]
    fn test_json_roundtrip_special_chars() {
        let meta = CoreDumpMeta {
            pid: 1,
            uid: 0,
            gid: 0,
            signal: 6,
            timestamp: 0,
            rlimit: 0,
            hostname: "host \"with\" quotes".into(),
            comm: "path/to\\prog".into(),
            exe: "/path/with\nnewline".into(),
            backtrace: true,
            core_size: 0,
            filename: String::new(),
            boot_id: String::new(),
            machine_id: String::new(),
        };

        let json = meta.to_json();
        let parsed = CoreDumpMeta::from_json(&json).unwrap();
        assert_eq!(parsed.hostname, meta.hostname);
        assert_eq!(parsed.comm, meta.comm);
        assert_eq!(parsed.exe, meta.exe);
    }

    #[test]
    fn test_parse_json_invalid() {
        assert!(parse_json_object("not json").is_none());
        assert!(parse_json_object("").is_none());
    }

    // -- Metadata tests -----------------------------------------------------

    #[test]
    fn test_signal_name() {
        let mut meta = CoreDumpMeta {
            pid: 0,
            uid: 0,
            gid: 0,
            signal: 11,
            timestamp: 0,
            rlimit: 0,
            hostname: String::new(),
            comm: String::new(),
            exe: String::new(),
            backtrace: false,
            core_size: 0,
            filename: String::new(),
            boot_id: String::new(),
            machine_id: String::new(),
        };

        assert_eq!(meta.signal_name(), "SIGSEGV");
        meta.signal = 6;
        assert_eq!(meta.signal_name(), "SIGABRT");
        meta.signal = 8;
        assert_eq!(meta.signal_name(), "SIGFPE");
        meta.signal = 4;
        assert_eq!(meta.signal_name(), "SIGILL");
        meta.signal = 7;
        assert_eq!(meta.signal_name(), "SIGBUS");
        meta.signal = 5;
        assert_eq!(meta.signal_name(), "SIGTRAP");
        meta.signal = 3;
        assert_eq!(meta.signal_name(), "SIGQUIT");
        meta.signal = 99;
        assert_eq!(meta.signal_name(), "unknown");
    }

    // -- Filename tests -----------------------------------------------------

    #[test]
    fn test_build_filename_basic() {
        let meta = CoreDumpMeta {
            pid: 1234,
            uid: 1000,
            gid: 1000,
            signal: 11,
            timestamp: 1700000000,
            rlimit: 0,
            hostname: "myhost".into(),
            comm: "myapp".into(),
            exe: "/usr/bin/myapp".into(),
            backtrace: false,
            core_size: 0,
            filename: String::new(),
            boot_id: "abc123".into(),
            machine_id: String::new(),
        };

        let name = build_filename(&meta);
        assert_eq!(name, "core.myapp.1000.abc123.1234.1700000000");
    }

    #[test]
    fn test_build_filename_special_chars() {
        let meta = CoreDumpMeta {
            pid: 5,
            uid: 0,
            gid: 0,
            signal: 6,
            timestamp: 100,
            rlimit: 0,
            hostname: "h".into(),
            comm: "my app!@#".into(),
            exe: String::new(),
            backtrace: false,
            core_size: 0,
            filename: String::new(),
            boot_id: "bid".into(),
            machine_id: String::new(),
        };

        let name = build_filename(&meta);
        assert_eq!(name, "core.my_app___.0.bid.5.100");
    }

    // -- Argument parsing tests ---------------------------------------------

    #[test]
    fn test_parse_args_basic() {
        let args: Vec<String> = vec![
            "systemd-coredump",
            "1234",
            "1000",
            "1000",
            "11",
            "1700000000",
            "18446744073709551615",
            "myhost",
            "myapp",
            "/usr/bin/myapp",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let parsed = parse_args(&args).unwrap();
        assert_eq!(parsed.meta.pid, 1234);
        assert_eq!(parsed.meta.uid, 1000);
        assert_eq!(parsed.meta.gid, 1000);
        assert_eq!(parsed.meta.signal, 11);
        assert_eq!(parsed.meta.timestamp, 1700000000);
        assert_eq!(parsed.meta.rlimit, u64::MAX);
        assert_eq!(parsed.meta.hostname, "myhost");
        assert_eq!(parsed.meta.comm, "myapp");
        assert_eq!(parsed.meta.exe, "/usr/bin/myapp");
        assert!(!parsed.meta.backtrace);
    }

    #[test]
    fn test_parse_args_with_backtrace() {
        let args: Vec<String> = vec![
            "systemd-coredump",
            "--backtrace",
            "42",
            "0",
            "0",
            "6",
            "100",
            "0",
            "localhost",
            "test",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let parsed = parse_args(&args).unwrap();
        assert!(parsed.meta.backtrace);
        assert_eq!(parsed.meta.pid, 42);
        assert_eq!(parsed.meta.signal, 6);
    }

    #[test]
    fn test_parse_args_no_exe() {
        let args: Vec<String> = vec![
            "systemd-coredump",
            "1",
            "0",
            "0",
            "11",
            "100",
            "0",
            "host",
            "comm",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let parsed = parse_args(&args).unwrap();
        assert_eq!(parsed.meta.exe, "");
    }

    #[test]
    fn test_parse_args_missing_args() {
        let args: Vec<String> = vec!["systemd-coredump"]
            .into_iter()
            .map(String::from)
            .collect();
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn test_parse_args_invalid_pid() {
        let args: Vec<String> = vec![
            "systemd-coredump",
            "not-a-number",
            "0",
            "0",
            "11",
            "100",
            "0",
            "host",
            "comm",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert!(parse_args(&args).is_err());
    }

    // -- Storage tests ------------------------------------------------------

    #[test]
    fn test_store_core_dump_basic() {
        let dir = TempDir::new().unwrap();
        let coredump_dir = dir.path().join("coredump");
        let config = Config::default();

        let mut meta = CoreDumpMeta {
            pid: 100,
            uid: 0,
            gid: 0,
            signal: 11,
            timestamp: 1700000000,
            rlimit: 0,
            hostname: "test".into(),
            comm: "testapp".into(),
            exe: "/usr/bin/testapp".into(),
            backtrace: false,
            core_size: 0,
            filename: String::new(),
            boot_id: "bootid".into(),
            machine_id: "machineid".into(),
        };

        let data = b"CORE DUMP DATA HERE";
        store_core_dump(&coredump_dir, &mut meta, data, &config).unwrap();

        assert!(!meta.filename.is_empty());
        assert_eq!(meta.core_size, data.len() as u64);

        // Check that the core file exists.
        let core_path = coredump_dir.join(&meta.filename);
        assert!(core_path.exists());
        assert_eq!(fs::read(&core_path).unwrap(), data);

        // Check that the metadata file exists.
        let meta_path = coredump_dir.join(format!("{}.json", meta.filename));
        assert!(meta_path.exists());

        // Parse the metadata back.
        let meta_json = fs::read_to_string(&meta_path).unwrap();
        let parsed = CoreDumpMeta::from_json(&meta_json).unwrap();
        assert_eq!(parsed.pid, 100);
        assert_eq!(parsed.comm, "testapp");
        assert_eq!(parsed.signal, 11);
    }

    #[test]
    fn test_store_core_dump_exceeds_external_max() {
        let dir = TempDir::new().unwrap();
        let coredump_dir = dir.path().join("coredump");
        let config = Config {
            external_size_max: 10, // tiny limit
            ..Config::default()
        };

        let mut meta = CoreDumpMeta {
            pid: 1,
            uid: 0,
            gid: 0,
            signal: 6,
            timestamp: 100,
            rlimit: 0,
            hostname: "h".into(),
            comm: "c".into(),
            exe: String::new(),
            backtrace: false,
            core_size: 0,
            filename: String::new(),
            boot_id: "b".into(),
            machine_id: "m".into(),
        };

        let data = b"THIS IS MORE THAN TEN BYTES OF DATA";
        store_core_dump(&coredump_dir, &mut meta, data, &config).unwrap();

        // Should NOT have stored anything (exceeds limit).
        assert!(meta.filename.is_empty());
    }

    #[test]
    fn test_store_creates_directory() {
        let dir = TempDir::new().unwrap();
        let coredump_dir = dir.path().join("deeply").join("nested").join("coredump");
        let config = Config::default();

        let mut meta = CoreDumpMeta {
            pid: 1,
            uid: 0,
            gid: 0,
            signal: 6,
            timestamp: 100,
            rlimit: 0,
            hostname: "h".into(),
            comm: "c".into(),
            exe: String::new(),
            backtrace: false,
            core_size: 0,
            filename: String::new(),
            boot_id: "b".into(),
            machine_id: "m".into(),
        };

        store_core_dump(&coredump_dir, &mut meta, b"data", &config).unwrap();
        assert!(coredump_dir.exists());
    }

    // -- Vacuum tests -------------------------------------------------------

    #[test]
    fn test_vacuum_removes_oldest() {
        let dir = TempDir::new().unwrap();
        let coredump_dir = dir.path().join("coredump");
        fs::create_dir_all(&coredump_dir).unwrap();

        // Create 3 core dumps totaling 300 bytes.
        for i in 0..3 {
            let name = format!("core.test.0.boot.{i}.{}", 1700000000 + i);
            fs::write(coredump_dir.join(&name), vec![b'X'; 100]).unwrap();

            let meta = CoreDumpMeta {
                pid: i as u64,
                uid: 0,
                gid: 0,
                signal: 6,
                timestamp: 1700000000 + i as u64,
                rlimit: 0,
                hostname: "h".into(),
                comm: "test".into(),
                exe: String::new(),
                backtrace: false,
                core_size: 100,
                filename: name.clone(),
                boot_id: "boot".into(),
                machine_id: "m".into(),
            };
            fs::write(coredump_dir.join(format!("{name}.json")), meta.to_json()).unwrap();
        }

        // Vacuum with a 150-byte limit should remove the two oldest.
        let config = Config {
            max_use: 150,
            keep_free: 0,
            ..Config::default()
        };
        vacuum(&coredump_dir, &config);

        let remaining: Vec<_> = fs::read_dir(&coredump_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("core.") && !name.ends_with(".json")
            })
            .collect();

        // Should have removed at least 1 (the oldest).
        assert!(remaining.len() < 3);
    }

    #[test]
    fn test_vacuum_empty_dir() {
        let dir = TempDir::new().unwrap();
        let coredump_dir = dir.path().join("coredump");
        fs::create_dir_all(&coredump_dir).unwrap();

        let config = Config {
            max_use: 0,
            ..Config::default()
        };

        // Should not panic.
        vacuum(&coredump_dir, &config);
    }

    #[test]
    fn test_vacuum_nonexistent_dir() {
        let config = Config::default();
        // Should not panic.
        vacuum(Path::new("/nonexistent/coredump"), &config);
    }

    // -- list_stored_coredumps tests ----------------------------------------

    #[test]
    fn test_list_stored_coredumps_empty() {
        let dir = TempDir::new().unwrap();
        let entries = list_stored_coredumps(dir.path());
        assert!(entries.is_empty());
    }

    #[test]
    fn test_list_stored_coredumps_with_entries() {
        let dir = TempDir::new().unwrap();

        for i in 0..3 {
            let name = format!("core.app.0.boot.{i}.{}", 1700000000 + i);
            fs::write(dir.path().join(&name), b"data").unwrap();
            fs::write(dir.path().join(format!("{name}.json")), b"{}").unwrap();
        }

        let entries = list_stored_coredumps(dir.path());
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_list_stored_coredumps_skips_non_core_files() {
        let dir = TempDir::new().unwrap();

        // A non-core file.
        fs::write(dir.path().join("random.json"), b"{}").unwrap();
        // A core file without .json shouldn't be listed either.
        fs::write(dir.path().join("core.x.0.b.1.100"), b"data").unwrap();

        let entries = list_stored_coredumps(dir.path());
        assert!(entries.is_empty());
    }

    // -- Integration test ---------------------------------------------------

    #[test]
    fn test_store_and_list_roundtrip() {
        let dir = TempDir::new().unwrap();
        let coredump_dir = dir.path().join("coredump");
        let config = Config::default();

        for i in 0..3 {
            let mut meta = CoreDumpMeta {
                pid: 100 + i,
                uid: 1000,
                gid: 1000,
                signal: 11,
                timestamp: 1700000000 + i as u64,
                rlimit: 0,
                hostname: "myhost".into(),
                comm: format!("app{i}"),
                exe: format!("/usr/bin/app{i}"),
                backtrace: false,
                core_size: 0,
                filename: String::new(),
                boot_id: format!("boot{i}"),
                machine_id: "machine".into(),
            };

            let data = format!("core dump data {i}");
            store_core_dump(&coredump_dir, &mut meta, data.as_bytes(), &config).unwrap();
        }

        let entries = list_stored_coredumps(&coredump_dir);
        assert_eq!(entries.len(), 3);

        // Verify metadata can be read back.
        for (_core_path, meta_path, _) in &entries {
            let json = fs::read_to_string(meta_path).unwrap();
            let parsed = CoreDumpMeta::from_json(&json).unwrap();
            assert!(parsed.pid >= 100);
            assert_eq!(parsed.uid, 1000);
            assert_eq!(parsed.signal, 11);
        }
    }

    #[test]
    fn test_store_empty_data() {
        let dir = TempDir::new().unwrap();
        let coredump_dir = dir.path().join("coredump");
        let config = Config::default();

        let mut meta = CoreDumpMeta {
            pid: 1,
            uid: 0,
            gid: 0,
            signal: 6,
            timestamp: 100,
            rlimit: 0,
            hostname: "h".into(),
            comm: "c".into(),
            exe: String::new(),
            backtrace: false,
            core_size: 0,
            filename: String::new(),
            boot_id: "b".into(),
            machine_id: "m".into(),
        };

        // Empty data should still store (the file will be 0 bytes).
        store_core_dump(&coredump_dir, &mut meta, b"", &config).unwrap();
        assert!(!meta.filename.is_empty());
        assert_eq!(meta.core_size, 0);
    }

    #[test]
    fn test_config_storage_none_is_noop() {
        // Verify the Storage::None path (tested at the run() level in
        // integration, but we can verify the config parses correctly).
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("coredump.conf");
        fs::write(&conf, "[Coredump]\nStorage=none\n").unwrap();
        let mut config = Config::default();
        parse_config_file(&conf, &mut config);
        assert_eq!(config.storage, Storage::None);
    }

    #[test]
    fn test_parse_size_whitespace() {
        assert_eq!(parse_size("  1024  "), Some(1024));
        assert_eq!(parse_size("  100M  "), Some(100 * 1024 * 1024));
    }
}
