//! Journal entry data model.
//!
//! A journal entry is a set of key-value fields (all `String` → `String`)
//! plus some metadata that journald attaches automatically (timestamps,
//! boot ID, machine ID, PID, UID, GID, …).
//!
//! The field names follow the systemd journal conventions documented in
//! `systemd.journal-fields(7)`.

use std::collections::BTreeMap;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single journal entry.
///
/// Fields are stored in a `BTreeMap` so that serialisation is
/// deterministic (sorted by key).  The well-known fields listed below
/// are *also* stored in the map — there are no separate struct members
/// for them.  Helper methods provide convenient typed access.
///
/// ## Well-known user fields
///
/// | Field               | Description                          |
/// |---------------------|--------------------------------------|
/// | `MESSAGE`           | Human-readable log message           |
/// | `MESSAGE_ID`        | 128-bit message identifier           |
/// | `PRIORITY`          | Syslog priority (0–7)                |
/// | `CODE_FILE`         | Source file name                      |
/// | `CODE_LINE`         | Source line number                    |
/// | `CODE_FUNC`         | Source function name                  |
/// | `SYSLOG_FACILITY`   | Syslog facility                      |
/// | `SYSLOG_IDENTIFIER` | Syslog identifier (tag)              |
/// | `SYSLOG_PID`        | Syslog PID                           |
///
/// ## Well-known trusted fields (prefixed with `_`)
///
/// | Field                      | Description                        |
/// |----------------------------|------------------------------------|
/// | `_PID`                     | Process ID of the logging process  |
/// | `_UID`                     | User ID                            |
/// | `_GID`                     | Group ID                           |
/// | `_COMM`                    | Process command name               |
/// | `_EXE`                     | Executable path                    |
/// | `_CMDLINE`                 | Full command line                  |
/// | `_SYSTEMD_UNIT`            | systemd unit name                  |
/// | `_SYSTEMD_SLICE`           | systemd slice                      |
/// | `_SYSTEMD_CGROUP`          | cgroup path                        |
/// | `_BOOT_ID`                 | 128-bit boot ID                    |
/// | `_MACHINE_ID`              | 128-bit machine ID                 |
/// | `_HOSTNAME`                | Host name                          |
/// | `_TRANSPORT`               | Transport used (journal, syslog, …)|
/// | `_SOURCE_REALTIME_TIMESTAMP` | Original timestamp from client   |
///
/// ## Timestamps
///
/// | Field                          | Description                    |
/// |--------------------------------|--------------------------------|
/// | `__REALTIME_TIMESTAMP`         | Realtime (wall clock) in µs    |
/// | `__MONOTONIC_TIMESTAMP`        | Monotonic clock in µs          |
///
/// Double-underscore fields are *address* fields managed by the journal
/// implementation and are never settable by clients.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalEntry {
    /// All fields of this entry, including well-known and custom ones.
    pub fields: BTreeMap<String, Vec<u8>>,

    /// Realtime (wall-clock) timestamp in microseconds since the UNIX epoch.
    /// Set by journald when the entry is received.
    pub realtime_usec: u64,

    /// Monotonic timestamp in microseconds since boot.
    pub monotonic_usec: u64,

    /// Sequence number (journal-internal, monotonically increasing).
    pub seqnum: u64,
}

impl JournalEntry {
    /// Create a new empty journal entry with the current timestamps.
    pub fn new() -> Self {
        let realtime_usec = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        let monotonic_usec = monotonic_usec_now();

        JournalEntry {
            fields: BTreeMap::new(),
            realtime_usec,
            monotonic_usec,
            seqnum: 0,
        }
    }

    /// Create an entry with a specific realtime timestamp (for testing /
    /// replay).
    pub fn with_timestamp(realtime_usec: u64, monotonic_usec: u64) -> Self {
        JournalEntry {
            fields: BTreeMap::new(),
            realtime_usec,
            monotonic_usec,
            seqnum: 0,
        }
    }

    // ------------------------------------------------------------------
    // Field accessors (convenience wrappers)
    // ------------------------------------------------------------------

    /// Insert a UTF-8 field.
    pub fn set_field(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.fields.insert(key.into(), value.into().into_bytes());
    }

    /// Insert a binary field.
    pub fn set_field_bytes(&mut self, key: impl Into<String>, value: Vec<u8>) {
        self.fields.insert(key.into(), value);
    }

    /// Get a field value as a UTF-8 string (lossy).
    pub fn field(&self, key: &str) -> Option<String> {
        self.fields
            .get(key)
            .map(|v| String::from_utf8_lossy(v).into_owned())
    }

    /// Get a field value as raw bytes.
    pub fn field_bytes(&self, key: &str) -> Option<&[u8]> {
        self.fields.get(key).map(|v| v.as_slice())
    }

    /// Return the `MESSAGE` field (lossy UTF-8).
    pub fn message(&self) -> Option<String> {
        self.field("MESSAGE")
    }

    /// Return the `PRIORITY` field parsed as `u8`, or `None`.
    pub fn priority(&self) -> Option<u8> {
        self.field("PRIORITY").and_then(|s| s.parse::<u8>().ok())
    }

    /// Return the syslog identifier (`SYSLOG_IDENTIFIER`).
    pub fn syslog_identifier(&self) -> Option<String> {
        self.field("SYSLOG_IDENTIFIER")
    }

    /// Return the `_PID` trusted field.
    pub fn pid(&self) -> Option<u32> {
        self.field("_PID").and_then(|s| s.parse::<u32>().ok())
    }

    /// Return the `_UID` trusted field.
    pub fn uid(&self) -> Option<u32> {
        self.field("_UID").and_then(|s| s.parse::<u32>().ok())
    }

    /// Return the `_GID` trusted field.
    pub fn gid(&self) -> Option<u32> {
        self.field("_GID").and_then(|s| s.parse::<u32>().ok())
    }

    /// Return the `_SYSTEMD_UNIT` trusted field.
    pub fn systemd_unit(&self) -> Option<String> {
        self.field("_SYSTEMD_UNIT")
    }

    /// Return the `_BOOT_ID` trusted field.
    pub fn boot_id(&self) -> Option<String> {
        self.field("_BOOT_ID")
    }

    /// Return the `_MACHINE_ID` trusted field.
    pub fn machine_id(&self) -> Option<String> {
        self.field("_MACHINE_ID")
    }

    /// Return the `_HOSTNAME` trusted field.
    pub fn hostname(&self) -> Option<String> {
        self.field("_HOSTNAME")
    }

    /// Return the `_TRANSPORT` trusted field.
    pub fn transport(&self) -> Option<String> {
        self.field("_TRANSPORT")
    }

    /// Return the `_COMM` trusted field (process command name).
    pub fn comm(&self) -> Option<String> {
        self.field("_COMM")
    }

    /// Return the `_EXE` trusted field (executable path).
    pub fn exe(&self) -> Option<String> {
        self.field("_EXE")
    }

    // ------------------------------------------------------------------
    // Trusted metadata helpers (set by journald, not the client)
    // ------------------------------------------------------------------

    /// Attach trusted process metadata looked up from `/proc/<pid>`.
    pub fn set_trusted_process_fields(&mut self, pid: u32) {
        self.set_field("_PID", pid.to_string());

        // Best-effort: read metadata from procfs
        let proc_base = format!("/proc/{}", pid);

        // _COMM
        if let Ok(comm) = std::fs::read_to_string(format!("{}/comm", proc_base)) {
            self.set_field("_COMM", comm.trim());
        }

        // _EXE
        if let Ok(exe) = std::fs::read_link(format!("{}/exe", proc_base)) {
            self.set_field("_EXE", exe.to_string_lossy().as_ref());
        }

        // _CMDLINE
        if let Ok(cmdline_bytes) = std::fs::read(format!("{}/cmdline", proc_base)) {
            // cmdline is NUL-separated; convert to space-separated
            let cmdline: String = cmdline_bytes
                .split(|&b| b == 0)
                .filter(|s| !s.is_empty())
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .collect::<Vec<_>>()
                .join(" ");
            if !cmdline.is_empty() {
                self.set_field("_CMDLINE", cmdline);
            }
        }

        // _UID / _GID — read from /proc/<pid>/status
        if let Ok(status) = std::fs::read_to_string(format!("{}/status", proc_base)) {
            for line in status.lines() {
                if let Some(rest) = line.strip_prefix("Uid:") {
                    // Format: "Uid:\treal\teffective\tsaved\tfs"
                    if let Some(uid_str) = rest.split_whitespace().next() {
                        self.set_field("_UID", uid_str);
                    }
                } else if let Some(rest) = line.strip_prefix("Gid:")
                    && let Some(gid_str) = rest.split_whitespace().next()
                {
                    self.set_field("_GID", gid_str);
                }
            }
        }

        // _SYSTEMD_CGROUP — read from /proc/<pid>/cgroup
        if let Ok(cgroup) = std::fs::read_to_string(format!("{}/cgroup", proc_base)) {
            // cgroup v2 format: "0::<path>"
            for line in cgroup.lines() {
                if let Some(rest) = line.strip_prefix("0::") {
                    self.set_field("_SYSTEMD_CGROUP", rest);
                    // Derive _SYSTEMD_UNIT from cgroup path
                    // e.g. /system.slice/foo.service → foo.service
                    if let Some(unit) = derive_unit_from_cgroup(rest) {
                        self.set_field("_SYSTEMD_UNIT", unit);
                    }
                    if let Some(slice) = derive_slice_from_cgroup(rest) {
                        self.set_field("_SYSTEMD_SLICE", slice);
                    }
                    break;
                }
            }
        }
    }

    /// Set the boot ID from /proc/sys/kernel/random/boot_id.
    pub fn set_boot_id(&mut self) {
        if let Ok(boot_id) = std::fs::read_to_string("/proc/sys/kernel/random/boot_id") {
            // Remove dashes to match systemd's 128-bit hex format
            let boot_id = boot_id.trim().replace('-', "");
            self.set_field("_BOOT_ID", boot_id);
        }
    }

    /// Set the machine ID from /etc/machine-id.
    pub fn set_machine_id(&mut self) {
        if let Ok(machine_id) = std::fs::read_to_string("/etc/machine-id") {
            self.set_field("_MACHINE_ID", machine_id.trim());
        }
    }

    /// Set the hostname.
    pub fn set_hostname(&mut self) {
        if let Ok(hostname) = std::fs::read_to_string("/proc/sys/kernel/hostname") {
            self.set_field("_HOSTNAME", hostname.trim());
        }
    }

    /// Populate all automatic trusted metadata fields.
    pub fn set_all_trusted_fields(&mut self, client_pid: u32, transport: &str) {
        self.set_trusted_process_fields(client_pid);
        self.set_field("_TRANSPORT", transport);
        self.set_boot_id();
        self.set_machine_id();
        self.set_hostname();
    }

    // ------------------------------------------------------------------
    // Serialisation — native journal export format
    // ------------------------------------------------------------------

    /// Serialise this entry in the native journal export format.
    ///
    /// The export format is documented in `systemd-journal-export(5)`:
    ///
    /// - Each entry starts with `__CURSOR=…`, `__REALTIME_TIMESTAMP=…`,
    ///   `__MONOTONIC_TIMESTAMP=…` address fields.
    /// - Followed by all user and trusted fields.
    /// - Text fields: `KEY=VALUE\n`
    /// - Binary fields: `KEY\n<8-byte LE length><data>\n`
    /// - Entries are separated by a blank line (`\n`).
    pub fn to_export_format(&self, cursor: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(1024);

        // Address fields
        out.extend_from_slice(format!("__CURSOR={}\n", cursor).as_bytes());
        out.extend_from_slice(format!("__REALTIME_TIMESTAMP={}\n", self.realtime_usec).as_bytes());
        out.extend_from_slice(
            format!("__MONOTONIC_TIMESTAMP={}\n", self.monotonic_usec).as_bytes(),
        );

        // User and trusted fields
        for (key, value) in &self.fields {
            if is_binary_safe(value) {
                out.extend_from_slice(key.as_bytes());
                out.push(b'=');
                out.extend_from_slice(value);
                out.push(b'\n');
            } else {
                // Binary encoding
                out.extend_from_slice(key.as_bytes());
                out.push(b'\n');
                out.extend_from_slice(&(value.len() as u64).to_le_bytes());
                out.extend_from_slice(value);
                out.push(b'\n');
            }
        }

        // Trailing blank line to separate entries
        out.push(b'\n');

        out
    }

    /// Serialise this entry as a JSON object.
    pub fn to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        map.insert(
            "__REALTIME_TIMESTAMP".to_string(),
            serde_json::Value::String(self.realtime_usec.to_string()),
        );
        map.insert(
            "__MONOTONIC_TIMESTAMP".to_string(),
            serde_json::Value::String(self.monotonic_usec.to_string()),
        );

        for (key, value) in &self.fields {
            let json_value = if let Ok(s) = std::str::from_utf8(value) {
                serde_json::Value::String(s.to_string())
            } else {
                // Binary data as JSON array of integers
                serde_json::Value::Array(
                    value
                        .iter()
                        .map(|&b| serde_json::Value::Number(b.into()))
                        .collect(),
                )
            };
            map.insert(key.clone(), json_value);
        }

        serde_json::Value::Object(map)
    }
}

impl Default for JournalEntry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for JournalEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Short format similar to `journalctl --output=short`
        let timestamp = format_realtime_short(self.realtime_usec);
        let hostname = self.hostname().unwrap_or_default();
        let identifier = self
            .syslog_identifier()
            .or_else(|| self.comm())
            .unwrap_or_else(|| "unknown".to_string());
        let pid_str = self.pid().map(|p| format!("[{}]", p)).unwrap_or_default();
        let message = self.message().unwrap_or_default();

        write!(
            f,
            "{} {} {}{}: {}",
            timestamp, hostname, identifier, pid_str, message
        )
    }
}

// ------------------------------------------------------------------
// Helper functions
// ------------------------------------------------------------------

/// Get the monotonic clock time in microseconds.
fn monotonic_usec_now() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts);
    }
    (ts.tv_sec as u64) * 1_000_000 + (ts.tv_nsec as u64) / 1_000
}

/// Check whether a byte slice is safe to include in the `KEY=VALUE\n`
/// text format (i.e., it contains no NUL bytes and no newlines in the
/// middle).
fn is_binary_safe(data: &[u8]) -> bool {
    !data.contains(&0) && !data.contains(&b'\n')
}

/// Derive a systemd unit name from a cgroup path.
///
/// Cgroup paths typically look like:
///   /system.slice/foo.service
///   /user.slice/user-1000.slice/session-1.scope
///
/// We take the last component that looks like a unit name.
fn derive_unit_from_cgroup(cgroup_path: &str) -> Option<String> {
    let components: Vec<&str> = cgroup_path.split('/').filter(|s| !s.is_empty()).collect();

    // Walk backwards to find the first component that has a unit suffix
    for component in components.iter().rev() {
        if component.ends_with(".service")
            || component.ends_with(".scope")
            || component.ends_with(".mount")
            || component.ends_with(".socket")
            || component.ends_with(".timer")
            || component.ends_with(".path")
            || component.ends_with(".swap")
            || component.ends_with(".target")
        {
            return Some(component.to_string());
        }
    }

    None
}

/// Derive a systemd slice from a cgroup path.
fn derive_slice_from_cgroup(cgroup_path: &str) -> Option<String> {
    let components: Vec<&str> = cgroup_path.split('/').filter(|s| !s.is_empty()).collect();

    for component in &components {
        if component.ends_with(".slice") {
            return Some(component.to_string());
        }
    }

    None
}

/// Format a realtime timestamp (µs since epoch) in the short syslog-style
/// format: `Mon DD HH:MM:SS` (local time).
fn format_realtime_short(realtime_usec: u64) -> String {
    use chrono::{Local, TimeZone};

    let secs = (realtime_usec / 1_000_000) as i64;
    let micros = (realtime_usec % 1_000_000) as u32;

    match Local.timestamp_opt(secs, micros * 1_000) {
        chrono::LocalResult::Single(dt) => dt.format("%b %d %H:%M:%S").to_string(),
        _ => format!("@{}", realtime_usec),
    }
}

/// Format a realtime timestamp in ISO 8601 format with microsecond precision.
pub fn format_realtime_iso(realtime_usec: u64) -> String {
    use chrono::{Local, TimeZone};

    let secs = (realtime_usec / 1_000_000) as i64;
    let micros = (realtime_usec % 1_000_000) as u32;

    match Local.timestamp_opt(secs, micros * 1_000) {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%dT%H:%M:%S%.6f%:z").to_string(),
        _ => format!("@{}", realtime_usec),
    }
}

/// Format a realtime timestamp in UTC ISO 8601 format.
pub fn format_realtime_utc(realtime_usec: u64) -> String {
    use chrono::{TimeZone, Utc};

    let secs = (realtime_usec / 1_000_000) as i64;
    let micros = (realtime_usec % 1_000_000) as u32;

    match Utc.timestamp_opt(secs, micros * 1_000) {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string(),
        _ => format!("@{}", realtime_usec),
    }
}

/// Priority level name.
pub fn priority_name(p: u8) -> &'static str {
    match p {
        0 => "emerg",
        1 => "alert",
        2 => "crit",
        3 => "err",
        4 => "warning",
        5 => "notice",
        6 => "info",
        7 => "debug",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_entry_has_timestamps() {
        let entry = JournalEntry::new();
        assert!(entry.realtime_usec > 0);
        // Monotonic might be 0 in some test environments
    }

    #[test]
    fn test_set_and_get_field() {
        let mut entry = JournalEntry::new();
        entry.set_field("MESSAGE", "hello world");
        assert_eq!(entry.message(), Some("hello world".to_string()));
        assert_eq!(entry.field("MESSAGE"), Some("hello world".to_string()));
    }

    #[test]
    fn test_set_and_get_field_bytes() {
        let mut entry = JournalEntry::new();
        entry.set_field_bytes("BINARY_DATA", vec![0x00, 0x01, 0x02, 0xff]);
        let bytes = entry.field_bytes("BINARY_DATA").unwrap();
        assert_eq!(bytes, &[0x00, 0x01, 0x02, 0xff]);
        // Lossy string should still work
        assert!(entry.field("BINARY_DATA").is_some());
    }

    #[test]
    fn test_priority() {
        let mut entry = JournalEntry::new();
        entry.set_field("PRIORITY", "3");
        assert_eq!(entry.priority(), Some(3));

        entry.set_field("PRIORITY", "invalid");
        assert_eq!(entry.priority(), None);
    }

    #[test]
    fn test_pid_uid_gid() {
        let mut entry = JournalEntry::new();
        entry.set_field("_PID", "1234");
        entry.set_field("_UID", "1000");
        entry.set_field("_GID", "1000");
        assert_eq!(entry.pid(), Some(1234));
        assert_eq!(entry.uid(), Some(1000));
        assert_eq!(entry.gid(), Some(1000));
    }

    #[test]
    fn test_derive_unit_from_cgroup() {
        assert_eq!(
            derive_unit_from_cgroup("/system.slice/foo.service"),
            Some("foo.service".to_string())
        );
        assert_eq!(
            derive_unit_from_cgroup("/system.slice/dbus.socket"),
            Some("dbus.socket".to_string())
        );
        assert_eq!(
            derive_unit_from_cgroup("/user.slice/user-1000.slice/session-1.scope"),
            Some("session-1.scope".to_string())
        );
        assert_eq!(derive_unit_from_cgroup("/"), None);
        assert_eq!(derive_unit_from_cgroup(""), None);
    }

    #[test]
    fn test_derive_slice_from_cgroup() {
        assert_eq!(
            derive_slice_from_cgroup("/system.slice/foo.service"),
            Some("system.slice".to_string())
        );
        assert_eq!(
            derive_slice_from_cgroup("/user.slice/user-1000.slice/session-1.scope"),
            Some("user.slice".to_string())
        );
        assert_eq!(derive_slice_from_cgroup("/foo.service"), None);
    }

    #[test]
    fn test_is_binary_safe() {
        assert!(is_binary_safe(b"hello world"));
        assert!(is_binary_safe(b""));
        assert!(is_binary_safe(b"line with spaces and stuff!"));
        assert!(!is_binary_safe(b"has\nnewline"));
        assert!(!is_binary_safe(b"has\0null"));
    }

    #[test]
    fn test_with_timestamp() {
        let entry = JournalEntry::with_timestamp(1_000_000, 500_000);
        assert_eq!(entry.realtime_usec, 1_000_000);
        assert_eq!(entry.monotonic_usec, 500_000);
    }

    #[test]
    fn test_to_json() {
        let mut entry = JournalEntry::with_timestamp(1_700_000_000_000_000, 100_000);
        entry.set_field("MESSAGE", "test message");
        entry.set_field("PRIORITY", "6");
        entry.set_field("SYSLOG_IDENTIFIER", "myapp");

        let json = entry.to_json();
        assert_eq!(json["MESSAGE"], "test message");
        assert_eq!(json["PRIORITY"], "6");
        assert_eq!(json["SYSLOG_IDENTIFIER"], "myapp");
        assert_eq!(json["__REALTIME_TIMESTAMP"], "1700000000000000");
    }

    #[test]
    fn test_export_format_text_fields() {
        let mut entry = JournalEntry::with_timestamp(1_700_000_000_000_000, 100_000);
        entry.set_field("MESSAGE", "hello");
        entry.set_field("PRIORITY", "6");

        let export = entry.to_export_format("s=abc;i=1;b=def;m=100000;t=1700000000000000;x=0");
        let export_str = String::from_utf8_lossy(&export);

        assert!(export_str.contains("__CURSOR="));
        assert!(export_str.contains("__REALTIME_TIMESTAMP=1700000000000000\n"));
        assert!(export_str.contains("__MONOTONIC_TIMESTAMP=100000\n"));
        assert!(export_str.contains("MESSAGE=hello\n"));
        assert!(export_str.contains("PRIORITY=6\n"));
        // Must end with double newline
        assert!(export_str.ends_with("\n\n"));
    }

    #[test]
    fn test_export_format_binary_field() {
        let mut entry = JournalEntry::with_timestamp(1_000_000, 0);
        entry.set_field_bytes("BINARY", vec![0x00, 0x01, 0x0a, 0xff]);

        let export = entry.to_export_format("cursor");
        // Binary field should use the length-prefixed format
        // BINARY\n<8-byte LE length><data>\n
        assert!(export.windows(7).any(|w| w == b"BINARY\n"));
    }

    #[test]
    fn test_display_format() {
        let mut entry = JournalEntry::with_timestamp(1_700_000_000_000_000, 0);
        entry.set_field("MESSAGE", "System started");
        entry.set_field("_HOSTNAME", "myhost");
        entry.set_field("SYSLOG_IDENTIFIER", "systemd");
        entry.set_field("_PID", "1");

        let display = format!("{}", entry);
        assert!(display.contains("myhost"));
        assert!(display.contains("systemd[1]"));
        assert!(display.contains("System started"));
    }

    #[test]
    fn test_fields_are_sorted() {
        let mut entry = JournalEntry::new();
        entry.set_field("ZEBRA", "last");
        entry.set_field("ALPHA", "first");
        entry.set_field("MIDDLE", "mid");

        let keys: Vec<&String> = entry.fields.keys().collect();
        let mut sorted_keys = keys.clone();
        sorted_keys.sort();
        assert_eq!(keys, sorted_keys);
    }

    #[test]
    fn test_priority_name() {
        assert_eq!(priority_name(0), "emerg");
        assert_eq!(priority_name(3), "err");
        assert_eq!(priority_name(6), "info");
        assert_eq!(priority_name(7), "debug");
        assert_eq!(priority_name(8), "unknown");
    }

    #[test]
    fn test_format_realtime_utc() {
        // 2023-11-15T00:00:00.000000Z
        let ts = 1_700_006_400_000_000u64;
        let formatted = format_realtime_utc(ts);
        assert!(formatted.contains("2023-11-15"));
        assert!(formatted.ends_with('Z'));
    }

    #[test]
    fn test_default_impl() {
        let entry = JournalEntry::default();
        assert!(entry.fields.is_empty());
        assert!(entry.realtime_usec > 0);
    }
}
