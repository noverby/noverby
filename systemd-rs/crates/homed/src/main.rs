//! systemd-homed — home directory management daemon
//!
//! Manages user home directories with identity records stored as JSON files
//! in `/var/lib/systemd/home/`. Each managed user has a `<username>.identity`
//! file containing their user record and a home area (directory, subvolume,
//! LUKS image, CIFS mount, or fscrypt directory).
//!
//! ## Features
//!
//! - User record management in JSON format (`/var/lib/systemd/home/*.identity`)
//! - Home storage backends: directory (plain), subvolume, luks, cifs, fscrypt
//!   (directory backend fully implemented; others are stubs)
//! - Operations: create, remove, activate, deactivate, update, passwd, resize,
//!   inspect, list, lock, unlock, lock-all, deactivate-all
//! - Control socket at `/run/systemd/homed-control` for `homectl` CLI
//! - sd_notify protocol (READY/WATCHDOG/STATUS/STOPPING)
//! - Signal handling (SIGTERM/SIGINT for shutdown, SIGHUP for reload)
//! - Periodic GC of stale state
//!
//! ## Missing
//!
//! - D-Bus interface (`org.freedesktop.home1`)
//! - LUKS2 encrypted home areas (open/close/resize)
//! - CIFS network mount backend
//! - fscrypt encrypted directory backend
//! - Btrfs subvolume backend (create/snapshot/quota)
//! - PKCS#11 / FIDO2 token authentication
//! - Password quality enforcement (pwquality)
//! - Automatic activation on login / deactivation on logout
//! - Suspend/resume lock integration with logind
//! - Home area size quota enforcement
//! - Recovery key generation

use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::fs as unix_fs;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

const IDENTITY_DIR: &str = "/var/lib/systemd/home";
const RUNTIME_DIR: &str = "/run/systemd/home";
const CONTROL_SOCKET_PATH: &str = "/run/systemd/homed-control";

/// Minimum UID for homed-managed users (from systemd: 60001..60513).
const UID_MIN: u32 = 60001;
/// Maximum UID for homed-managed users.
const UID_MAX: u32 = 60513;

// ---------------------------------------------------------------------------
// Storage type
// ---------------------------------------------------------------------------

/// The backing storage type for a home area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Storage {
    Directory,
    Subvolume,
    Luks,
    Cifs,
    Fscrypt,
}

impl Storage {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "directory" => Some(Self::Directory),
            "subvolume" => Some(Self::Subvolume),
            "luks" => Some(Self::Luks),
            "cifs" => Some(Self::Cifs),
            "fscrypt" => Some(Self::Fscrypt),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Directory => "directory",
            Self::Subvolume => "subvolume",
            Self::Luks => "luks",
            Self::Cifs => "cifs",
            Self::Fscrypt => "fscrypt",
        }
    }
}

impl fmt::Display for Storage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Home state
// ---------------------------------------------------------------------------

/// Runtime state of a managed home area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeState {
    Inactive,
    Activating,
    Active,
    Deactivating,
    Locked,
    /// Home area absent from disk (record exists but image/dir missing).
    Absent,
    /// Home area is in an inconsistent state.
    Dirty,
}

impl HomeState {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "inactive" => Some(Self::Inactive),
            "activating" => Some(Self::Activating),
            "active" => Some(Self::Active),
            "deactivating" => Some(Self::Deactivating),
            "locked" => Some(Self::Locked),
            "absent" => Some(Self::Absent),
            "dirty" => Some(Self::Dirty),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Inactive => "inactive",
            Self::Activating => "activating",
            Self::Active => "active",
            Self::Deactivating => "deactivating",
            Self::Locked => "locked",
            Self::Absent => "absent",
            Self::Dirty => "dirty",
        }
    }
}

impl fmt::Display for HomeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Disposition
// ---------------------------------------------------------------------------

/// User disposition — how the user record came into being.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Regular,
    System,
    Intrinsic,
}

impl Disposition {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "regular" => Some(Self::Regular),
            "system" => Some(Self::System),
            "intrinsic" => Some(Self::Intrinsic),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::System => "system",
            Self::Intrinsic => "intrinsic",
        }
    }
}

impl fmt::Display for Disposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// User record
// ---------------------------------------------------------------------------

/// A managed user identity record (simplified version of systemd's JSON
/// user record as described in `systemd.user-record(5)`).
#[derive(Debug, Clone, PartialEq)]
pub struct UserRecord {
    pub user_name: String,
    pub real_name: String,
    pub uid: u32,
    pub gid: u32,
    pub member_of: Vec<String>,
    pub home_directory: String,
    pub image_path: String,
    pub shell: String,
    pub storage: Storage,
    pub disposition: Disposition,
    pub state: HomeState,
    pub disk_size: Option<u64>,
    pub disk_usage: Option<u64>,
    pub password_hint: Option<String>,
    pub enforce_password_policy: bool,
    pub auto_login: bool,
    /// SHA-512 hashed password(s).  Empty if no password set.
    pub hashed_passwords: Vec<String>,
    /// Microsecond timestamps.
    pub last_change_usec: u64,
    pub last_password_change_usec: u64,
    pub service: String,
    /// Whether the home area is currently locked (suspend protection).
    pub locked: bool,
}

impl UserRecord {
    /// Create a new user record with sane defaults.
    pub fn new(user_name: &str, uid: u32) -> Self {
        let now_usec = now_usec();
        Self {
            user_name: user_name.to_string(),
            real_name: user_name.to_string(),
            uid,
            gid: uid,
            member_of: Vec::new(),
            home_directory: format!("/home/{}", user_name),
            image_path: format!("/home/{}.homedir", user_name),
            shell: "/bin/bash".to_string(),
            storage: Storage::Directory,
            disposition: Disposition::Regular,
            state: HomeState::Inactive,
            disk_size: None,
            disk_usage: None,
            password_hint: None,
            enforce_password_policy: true,
            auto_login: false,
            hashed_passwords: Vec::new(),
            last_change_usec: now_usec,
            last_password_change_usec: now_usec,
            service: "io.systemd.Home".to_string(),
            locked: false,
        }
    }

    // -- JSON serialization (hand-rolled to avoid serde dependency) ----------

    /// Serialize to a JSON string.
    pub fn to_json(&self) -> String {
        let mut s = String::from("{\n");
        json_str_field(&mut s, "userName", &self.user_name, true);
        json_str_field(&mut s, "realName", &self.real_name, true);
        json_u64_field(&mut s, "uid", self.uid as u64, true);
        json_u64_field(&mut s, "gid", self.gid as u64, true);
        json_str_array_field(&mut s, "memberOf", &self.member_of, true);
        json_str_field(&mut s, "homeDirectory", &self.home_directory, true);
        json_str_field(&mut s, "imagePath", &self.image_path, true);
        json_str_field(&mut s, "shell", &self.shell, true);
        json_str_field(&mut s, "storage", self.storage.as_str(), true);
        json_str_field(&mut s, "disposition", self.disposition.as_str(), true);
        json_str_field(&mut s, "state", self.state.as_str(), true);
        json_opt_u64_field(&mut s, "diskSize", self.disk_size, true);
        json_opt_u64_field(&mut s, "diskUsage", self.disk_usage, true);
        json_opt_str_field(&mut s, "passwordHint", self.password_hint.as_deref(), true);
        json_bool_field(
            &mut s,
            "enforcePasswordPolicy",
            self.enforce_password_policy,
            true,
        );
        json_bool_field(&mut s, "autoLogin", self.auto_login, true);
        json_str_array_field(&mut s, "hashedPassword", &self.hashed_passwords, true);
        json_u64_field(&mut s, "lastChangeUSec", self.last_change_usec, true);
        json_u64_field(
            &mut s,
            "lastPasswordChangeUSec",
            self.last_password_change_usec,
            true,
        );
        json_str_field(&mut s, "service", &self.service, true);
        json_bool_field(&mut s, "locked", self.locked, false);
        s.push('}');
        s
    }

    /// Parse from a JSON string.  This is a minimal parser that handles only
    /// the fields we produce in `to_json`.
    pub fn from_json(input: &str) -> Result<Self, String> {
        let fields = parse_json_object(input)?;

        let user_name = get_json_str(&fields, "userName")?;
        let real_name = get_json_str_or(&fields, "realName", &user_name);
        let uid = get_json_u64(&fields, "uid")? as u32;
        let gid = get_json_u64_or(&fields, "gid", uid as u64) as u32;
        let member_of = get_json_str_array(&fields, "memberOf");
        let home_directory =
            get_json_str_or(&fields, "homeDirectory", &format!("/home/{}", user_name));
        let image_path = get_json_str_or(
            &fields,
            "imagePath",
            &format!("/home/{}.homedir", user_name),
        );
        let shell = get_json_str_or(&fields, "shell", "/bin/bash");
        let storage = fields
            .get("storage")
            .and_then(|v| Storage::parse(v.trim_matches('"')))
            .unwrap_or(Storage::Directory);
        let disposition = fields
            .get("disposition")
            .and_then(|v| Disposition::parse(v.trim_matches('"')))
            .unwrap_or(Disposition::Regular);
        let state = fields
            .get("state")
            .and_then(|v| HomeState::parse(v.trim_matches('"')))
            .unwrap_or(HomeState::Inactive);
        let disk_size = get_json_opt_u64(&fields, "diskSize");
        let disk_usage = get_json_opt_u64(&fields, "diskUsage");
        let password_hint = fields.get("passwordHint").and_then(|v| {
            let v = v.trim_matches('"');
            if v == "null" {
                None
            } else {
                Some(v.to_string())
            }
        });
        let enforce_password_policy = get_json_bool_or(&fields, "enforcePasswordPolicy", true);
        let auto_login = get_json_bool_or(&fields, "autoLogin", false);
        let hashed_passwords = get_json_str_array(&fields, "hashedPassword");
        let last_change_usec = get_json_u64_or(&fields, "lastChangeUSec", 0);
        let last_password_change_usec = get_json_u64_or(&fields, "lastPasswordChangeUSec", 0);
        let service = get_json_str_or(&fields, "service", "io.systemd.Home");
        let locked = get_json_bool_or(&fields, "locked", false);

        Ok(Self {
            user_name,
            real_name,
            uid,
            gid,
            member_of,
            home_directory,
            image_path,
            shell,
            storage,
            disposition,
            state,
            disk_size,
            disk_usage,
            password_hint,
            enforce_password_policy,
            auto_login,
            hashed_passwords,
            last_change_usec,
            last_password_change_usec,
            service,
            locked,
        })
    }

    /// Format as a human-readable status block (for `homectl inspect`).
    pub fn format_inspect(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("   User name: {}\n", self.user_name));
        s.push_str(&format!("   Real name: {}\n", self.real_name));
        s.push_str(&format!(" Disposition: {}\n", self.disposition));
        s.push_str(&format!("       State: {}\n", self.state));
        s.push_str(&format!("     Service: {}\n", self.service));
        s.push_str(&format!(" Home Dir.:  {}\n", self.home_directory));
        s.push_str(&format!(" Image Path: {}\n", self.image_path));
        s.push_str(&format!("     Storage: {}\n", self.storage));
        s.push_str(&format!("         UID: {}\n", self.uid));
        s.push_str(&format!("         GID: {}\n", self.gid));
        if !self.member_of.is_empty() {
            s.push_str(&format!("   Member Of: {}\n", self.member_of.join(", ")));
        }
        s.push_str(&format!("       Shell: {}\n", self.shell));
        if let Some(sz) = self.disk_size {
            s.push_str(&format!("   Disk Size: {}\n", format_bytes(sz)));
        }
        if let Some(usage) = self.disk_usage {
            s.push_str(&format!("  Disk Usage: {}\n", format_bytes(usage)));
        }
        if let Some(ref hint) = self.password_hint {
            s.push_str(&format!("   Pass Hint: {}\n", hint));
        }
        s.push_str(&format!(
            "      Locked: {}\n",
            if self.locked { "yes" } else { "no" }
        ));
        s.push_str(&format!(
            "  Auto Login: {}\n",
            if self.auto_login { "yes" } else { "no" }
        ));
        s
    }

    /// Format as key=value properties (for `homectl show`).
    pub fn format_show(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("UserName={}\n", self.user_name));
        s.push_str(&format!("RealName={}\n", self.real_name));
        s.push_str(&format!("Disposition={}\n", self.disposition));
        s.push_str(&format!("State={}\n", self.state));
        s.push_str(&format!("Service={}\n", self.service));
        s.push_str(&format!("HomeDirectory={}\n", self.home_directory));
        s.push_str(&format!("ImagePath={}\n", self.image_path));
        s.push_str(&format!("Storage={}\n", self.storage));
        s.push_str(&format!("UID={}\n", self.uid));
        s.push_str(&format!("GID={}\n", self.gid));
        s.push_str(&format!("Shell={}\n", self.shell));
        if let Some(sz) = self.disk_size {
            s.push_str(&format!("DiskSize={}\n", sz));
        }
        if let Some(usage) = self.disk_usage {
            s.push_str(&format!("DiskUsage={}\n", usage));
        }
        s.push_str(&format!("Locked={}\n", self.locked));
        s.push_str(&format!("AutoLogin={}\n", self.auto_login));
        s.push_str(&format!(
            "EnforcePasswordPolicy={}\n",
            self.enforce_password_policy
        ));
        s
    }
}

// ---------------------------------------------------------------------------
// JSON helpers (minimal, no serde)
// ---------------------------------------------------------------------------

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

fn json_str_field(s: &mut String, key: &str, val: &str, comma: bool) {
    s.push_str(&format!("  \"{}\": \"{}\"", key, json_escape(val)));
    if comma {
        s.push(',');
    }
    s.push('\n');
}

fn json_u64_field(s: &mut String, key: &str, val: u64, comma: bool) {
    s.push_str(&format!("  \"{}\": {}", key, val));
    if comma {
        s.push(',');
    }
    s.push('\n');
}

fn json_bool_field(s: &mut String, key: &str, val: bool, comma: bool) {
    s.push_str(&format!("  \"{}\": {}", key, val));
    if comma {
        s.push(',');
    }
    s.push('\n');
}

fn json_opt_u64_field(s: &mut String, key: &str, val: Option<u64>, comma: bool) {
    match val {
        Some(v) => s.push_str(&format!("  \"{}\": {}", key, v)),
        None => s.push_str(&format!("  \"{}\": null", key)),
    }
    if comma {
        s.push(',');
    }
    s.push('\n');
}

fn json_opt_str_field(s: &mut String, key: &str, val: Option<&str>, comma: bool) {
    match val {
        Some(v) => s.push_str(&format!("  \"{}\": \"{}\"", key, json_escape(v))),
        None => s.push_str(&format!("  \"{}\": null", key)),
    }
    if comma {
        s.push(',');
    }
    s.push('\n');
}

fn json_str_array_field(s: &mut String, key: &str, vals: &[String], comma: bool) {
    if vals.is_empty() {
        s.push_str(&format!("  \"{}\": []", key));
    } else {
        s.push_str(&format!("  \"{}\": [", key));
        for (i, v) in vals.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&format!("\"{}\"", json_escape(v)));
        }
        s.push(']');
    }
    if comma {
        s.push(',');
    }
    s.push('\n');
}

/// Very simple JSON object parser — returns key→raw_value pairs.  Handles
/// strings (with basic escape sequences), numbers, booleans, null, and arrays
/// of strings.  Not a general-purpose parser.
fn parse_json_object(input: &str) -> Result<BTreeMap<String, String>, String> {
    let input = input.trim();
    if !input.starts_with('{') || !input.ends_with('}') {
        return Err("not a JSON object".to_string());
    }
    let inner = &input[1..input.len() - 1];
    let mut map = BTreeMap::new();
    let mut chars = inner.chars().peekable();

    loop {
        skip_ws(&mut chars);
        if chars.peek().is_none() {
            break;
        }
        // Key
        let key = parse_json_string_chars(&mut chars)?;
        skip_ws(&mut chars);
        match chars.next() {
            Some(':') => {}
            _ => return Err("expected ':'".to_string()),
        }
        skip_ws(&mut chars);
        // Value
        let val = parse_json_value_chars(&mut chars)?;
        map.insert(key, val);
        skip_ws(&mut chars);
        if chars.peek() == Some(&',') {
            chars.next();
        }
    }
    Ok(map)
}

fn skip_ws(chars: &mut std::iter::Peekable<std::str::Chars>) {
    while let Some(&c) = chars.peek() {
        if c.is_ascii_whitespace() {
            chars.next();
        } else {
            break;
        }
    }
}

fn parse_json_string_chars(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Result<String, String> {
    match chars.next() {
        Some('"') => {}
        _ => return Err("expected '\"'".to_string()),
    }
    let mut s = String::new();
    loop {
        match chars.next() {
            Some('"') => return Ok(s),
            Some('\\') => match chars.next() {
                Some('"') => s.push('"'),
                Some('\\') => s.push('\\'),
                Some('n') => s.push('\n'),
                Some('r') => s.push('\r'),
                Some('t') => s.push('\t'),
                Some('/') => s.push('/'),
                Some('u') => {
                    let mut hex = String::new();
                    for _ in 0..4 {
                        match chars.next() {
                            Some(c) => hex.push(c),
                            None => return Err("unterminated \\u escape".to_string()),
                        }
                    }
                    if let Ok(cp) = u32::from_str_radix(&hex, 16)
                        && let Some(c) = char::from_u32(cp)
                    {
                        s.push(c);
                    }
                }
                _ => s.push('?'),
            },
            Some(c) => s.push(c),
            None => return Err("unterminated string".to_string()),
        }
    }
}

fn parse_json_value_chars(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Result<String, String> {
    skip_ws(chars);
    match chars.peek() {
        Some('"') => {
            let s = parse_json_string_chars(chars)?;
            Ok(format!("\"{}\"", s))
        }
        Some('[') => {
            // Collect array as raw text
            let mut depth = 0i32;
            let mut arr = String::new();
            for c in chars.by_ref() {
                arr.push(c);
                if c == '[' {
                    depth += 1;
                } else if c == ']' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
            }
            Ok(arr)
        }
        Some('{') => {
            let mut depth = 0i32;
            let mut obj = String::new();
            for c in chars.by_ref() {
                obj.push(c);
                if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
            }
            Ok(obj)
        }
        _ => {
            // number, bool, null
            let mut tok = String::new();
            while let Some(&c) = chars.peek() {
                if c == ',' || c == '}' || c == ']' || c.is_ascii_whitespace() {
                    break;
                }
                tok.push(c);
                chars.next();
            }
            Ok(tok)
        }
    }
}

fn get_json_str(fields: &BTreeMap<String, String>, key: &str) -> Result<String, String> {
    fields
        .get(key)
        .map(|v| v.trim_matches('"').to_string())
        .ok_or_else(|| format!("missing field '{}'", key))
}

fn get_json_str_or(fields: &BTreeMap<String, String>, key: &str, default: &str) -> String {
    fields
        .get(key)
        .map(|v| v.trim_matches('"').to_string())
        .unwrap_or_else(|| default.to_string())
}

fn get_json_u64(fields: &BTreeMap<String, String>, key: &str) -> Result<u64, String> {
    fields
        .get(key)
        .ok_or_else(|| format!("missing field '{}'", key))
        .and_then(|v| {
            v.parse::<u64>()
                .map_err(|e| format!("bad u64 for '{}': {}", key, e))
        })
}

fn get_json_u64_or(fields: &BTreeMap<String, String>, key: &str, default: u64) -> u64 {
    fields
        .get(key)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

fn get_json_opt_u64(fields: &BTreeMap<String, String>, key: &str) -> Option<u64> {
    fields.get(key).and_then(|v| {
        if v == "null" {
            None
        } else {
            v.parse::<u64>().ok()
        }
    })
}

fn get_json_bool_or(fields: &BTreeMap<String, String>, key: &str, default: bool) -> bool {
    fields.get(key).map(|v| v == "true").unwrap_or(default)
}

fn get_json_str_array(fields: &BTreeMap<String, String>, key: &str) -> Vec<String> {
    let raw = match fields.get(key) {
        Some(v) => v.clone(),
        None => return Vec::new(),
    };
    let raw = raw.trim();
    if !raw.starts_with('[') || !raw.ends_with(']') {
        return Vec::new();
    }
    let inner = &raw[1..raw.len() - 1];
    let mut result = Vec::new();
    let mut chars = inner.chars().peekable();
    loop {
        skip_ws(&mut chars);
        if chars.peek().is_none() {
            break;
        }
        if chars.peek() == Some(&'"') {
            if let Ok(s) = parse_json_string_chars(&mut chars) {
                result.push(s);
            }
        } else {
            break;
        }
        skip_ws(&mut chars);
        if chars.peek() == Some(&',') {
            chars.next();
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

fn now_usec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_micros() as u64
}

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    const TIB: u64 = 1024 * GIB;

    if bytes >= TIB {
        format!("{:.1}T", bytes as f64 / TIB as f64)
    } else if bytes >= GIB {
        format!("{:.1}G", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1}M", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1}K", bytes as f64 / KIB as f64)
    } else {
        format!("{}B", bytes)
    }
}

/// Compute disk usage of a directory tree (bytes).
pub fn dir_disk_usage(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if ft.is_dir() {
                total += dir_disk_usage(&entry.path());
            } else if ft.is_file()
                && let Ok(meta) = entry.metadata()
            {
                total += meta.len();
            }
        }
    }
    total
}

// ---------------------------------------------------------------------------
// Password hashing (SHA-512 crypt format, simplified)
// ---------------------------------------------------------------------------

/// Hash a password using a simple SHA-512-based scheme.
/// Real systemd uses `crypt(3)` with `$6$` prefix.  We produce a
/// `$6$homed$<hex-sha512>` string so that `verify_password` can check it.
///
/// This is NOT cryptographically equivalent to `crypt(3)` — it's a minimal
/// stand-in so the full create/passwd/verify workflow can be tested without
/// a libc dependency.
pub fn hash_password(password: &str) -> String {
    // Use a very simple hash: djb2 iterated.  This is NOT secure —
    // a real implementation would call libc crypt(3).  But it's deterministic
    // and lets us roundtrip in tests.
    let mut h: u64 = 5381;
    for b in password.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    // Iterate to fill 64 hex chars
    let mut hex = String::new();
    for i in 0u64..8 {
        let v = h.wrapping_add(i.wrapping_mul(0x9e3779b97f4a7c15));
        hex.push_str(&format!("{:016x}", v));
    }
    format!("$6$homed${}", &hex[..128])
}

/// Verify a password against a stored hash.
pub fn verify_password(password: &str, stored: &str) -> bool {
    let expected = hash_password(password);
    expected == stored
}

// ---------------------------------------------------------------------------
// User name validation
// ---------------------------------------------------------------------------

/// Validate a user name per systemd conventions.
pub fn is_valid_user_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 256 {
        return false;
    }
    // Must start with lowercase letter or underscore
    let first = name.as_bytes()[0];
    if !(first.is_ascii_lowercase() || first == b'_') {
        return false;
    }
    // Remaining chars: lowercase, digit, underscore, hyphen
    for &b in &name.as_bytes()[1..] {
        if !(b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-') {
            return false;
        }
    }
    // Must not be a reserved name
    !matches!(
        name,
        "root"
            | "nobody"
            | "nfsnobody"
            | "daemon"
            | "bin"
            | "sys"
            | "sync"
            | "games"
            | "man"
            | "lp"
            | "mail"
            | "news"
            | "uucp"
            | "proxy"
            | "www-data"
            | "backup"
            | "list"
            | "irc"
            | "gnats"
            | "systemd-network"
            | "systemd-resolve"
            | "messagebus"
            | "sshd"
    )
}

// ---------------------------------------------------------------------------
// Home registry (in-memory state)
// ---------------------------------------------------------------------------

/// Parameters for creating a new home.
pub struct CreateParams<'a> {
    pub user_name: &'a str,
    pub real_name: Option<&'a str>,
    pub shell: Option<&'a str>,
    pub storage: Storage,
    pub password: Option<&'a str>,
    pub home_dir_override: Option<&'a str>,
    pub image_path_override: Option<&'a str>,
}

/// The home registry tracks all known managed home directories.
pub struct HomeRegistry {
    /// Identity directory on disk.
    identity_dir: PathBuf,
    /// Runtime state directory.
    runtime_dir: PathBuf,
    /// In-memory records keyed by user name.
    homes: BTreeMap<String, UserRecord>,
    /// Next UID to allocate.
    next_uid: u32,
}

impl Default for HomeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HomeRegistry {
    pub fn new() -> Self {
        Self::with_paths(Path::new(IDENTITY_DIR), Path::new(RUNTIME_DIR))
    }

    pub fn with_paths(identity_dir: &Path, runtime_dir: &Path) -> Self {
        Self {
            identity_dir: identity_dir.to_path_buf(),
            runtime_dir: runtime_dir.to_path_buf(),
            homes: BTreeMap::new(),
            next_uid: UID_MIN,
        }
    }

    /// Load all identity files from disk.
    pub fn load(&mut self) {
        self.homes.clear();
        if let Ok(entries) = fs::read_dir(&self.identity_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.ends_with(".identity") {
                    continue;
                }
                if name.starts_with('.') {
                    continue;
                }
                if let Ok(data) = fs::read_to_string(entry.path()) {
                    match UserRecord::from_json(&data) {
                        Ok(mut rec) => {
                            // Refresh state from runtime dir
                            let rt_path = self.runtime_dir.join(&rec.user_name);
                            if rt_path.exists()
                                && let Ok(st) = fs::read_to_string(&rt_path)
                            {
                                let st = st.trim();
                                if let Some(state) = HomeState::parse(st) {
                                    rec.state = state;
                                }
                            }
                            // Track highest UID
                            if rec.uid >= self.next_uid {
                                self.next_uid = rec.uid + 1;
                            }
                            self.homes.insert(rec.user_name.clone(), rec);
                        }
                        Err(e) => {
                            log::warn!("Failed to parse {}: {}", name, e);
                        }
                    }
                }
            }
        }
    }

    /// Save a single identity record to disk.
    pub fn save_one(&self, user_name: &str) -> io::Result<()> {
        if let Some(rec) = self.homes.get(user_name) {
            let _ = fs::create_dir_all(&self.identity_dir);
            let path = self.identity_dir.join(format!("{}.identity", user_name));
            let json = rec.to_json();
            fs::write(&path, json)?;
        }
        Ok(())
    }

    /// Save runtime state for a user.
    fn save_runtime_state(&self, user_name: &str) -> io::Result<()> {
        if let Some(rec) = self.homes.get(user_name) {
            let _ = fs::create_dir_all(&self.runtime_dir);
            let path = self.runtime_dir.join(user_name);
            fs::write(&path, rec.state.as_str())?;
        }
        Ok(())
    }

    /// Remove runtime state for a user.
    fn remove_runtime_state(&self, user_name: &str) {
        let path = self.runtime_dir.join(user_name);
        let _ = fs::remove_file(&path);
    }

    /// Allocate the next free UID in the homed range.
    pub fn allocate_uid(&mut self) -> Result<u32, String> {
        if self.next_uid > UID_MAX {
            return Err("UID range exhausted".to_string());
        }
        let uid = self.next_uid;
        self.next_uid += 1;
        Ok(uid)
    }

    /// Get a reference to a record.
    pub fn get(&self, user_name: &str) -> Option<&UserRecord> {
        self.homes.get(user_name)
    }

    /// Get a mutable reference to a record.
    pub fn get_mut(&mut self, user_name: &str) -> Option<&mut UserRecord> {
        self.homes.get_mut(user_name)
    }

    /// Check if a user is registered.
    pub fn contains(&self, user_name: &str) -> bool {
        self.homes.contains_key(user_name)
    }

    /// List all managed users.
    pub fn list(&self) -> Vec<&UserRecord> {
        self.homes.values().collect()
    }

    /// Number of managed homes.
    pub fn len(&self) -> usize {
        self.homes.len()
    }

    /// Whether registry is empty.
    pub fn is_empty(&self) -> bool {
        self.homes.is_empty()
    }

    // -- Operations ---------------------------------------------------------

    /// Create a new managed home.
    #[allow(clippy::too_many_arguments)]
    pub fn create(&mut self, params: CreateParams) -> Result<String, String> {
        if !is_valid_user_name(params.user_name) {
            return Err(format!("Invalid user name: {}", params.user_name));
        }
        if self.homes.contains_key(params.user_name) {
            return Err(format!("User '{}' already exists", params.user_name));
        }
        let uid = self.allocate_uid()?;
        let mut rec = UserRecord::new(params.user_name, uid);
        if let Some(rn) = params.real_name {
            rec.real_name = rn.to_string();
        }
        if let Some(sh) = params.shell {
            rec.shell = sh.to_string();
        }
        rec.storage = params.storage;
        if let Some(hd) = params.home_dir_override {
            rec.home_directory = hd.to_string();
        }
        if let Some(ip) = params.image_path_override {
            rec.image_path = ip.to_string();
        }
        if let Some(pw) = params.password {
            rec.hashed_passwords.push(hash_password(pw));
        }

        // Create the home area on disk
        self.create_home_area(&rec)?;

        let user_name = params.user_name.to_string();
        self.homes.insert(user_name.clone(), rec);
        let _ = self.save_one(&user_name);
        Ok(format!("Created home for user '{}'", user_name))
    }

    /// Create the backing home area on disk.
    fn create_home_area(&self, rec: &UserRecord) -> Result<(), String> {
        match rec.storage {
            Storage::Directory => {
                let image = Path::new(&rec.image_path);
                if image.exists() {
                    return Err(format!("Image path already exists: {}", rec.image_path));
                }
                fs::create_dir_all(image)
                    .map_err(|e| format!("Failed to create {}: {}", rec.image_path, e))?;
                // Set ownership (best-effort, may fail in tests without root)
                let _ = nix::unistd::chown(
                    image,
                    Some(nix::unistd::Uid::from_raw(rec.uid)),
                    Some(nix::unistd::Gid::from_raw(rec.gid)),
                );
                // Set mode 0700
                let _ =
                    fs::set_permissions(image, std::os::unix::fs::PermissionsExt::from_mode(0o700));
                Ok(())
            }
            Storage::Subvolume => {
                // Stub: would create btrfs subvolume
                let image = Path::new(&rec.image_path);
                fs::create_dir_all(image).map_err(|e| {
                    format!("Failed to create subvolume dir {}: {}", rec.image_path, e)
                })?;
                Ok(())
            }
            Storage::Luks | Storage::Cifs | Storage::Fscrypt => Err(format!(
                "{} storage backend is not yet implemented",
                rec.storage
            )),
        }
    }

    /// Remove a managed home.
    pub fn remove(&mut self, user_name: &str) -> Result<String, String> {
        let rec = match self.homes.get(user_name) {
            Some(r) => r.clone(),
            None => return Err(format!("Unknown user: {}", user_name)),
        };
        if rec.state == HomeState::Active || rec.state == HomeState::Locked {
            return Err(format!(
                "Home for '{}' is still active, deactivate first",
                user_name
            ));
        }

        // Remove home area from disk
        self.remove_home_area(&rec)?;

        // Remove identity file
        let id_path = self.identity_dir.join(format!("{}.identity", user_name));
        let _ = fs::remove_file(&id_path);
        self.remove_runtime_state(user_name);
        self.homes.remove(user_name);

        Ok(format!("Removed home for user '{}'", user_name))
    }

    fn remove_home_area(&self, rec: &UserRecord) -> Result<(), String> {
        let image = Path::new(&rec.image_path);
        if image.exists() {
            if image.is_dir() {
                fs::remove_dir_all(image)
                    .map_err(|e| format!("Failed to remove {}: {}", rec.image_path, e))?;
            } else {
                fs::remove_file(image)
                    .map_err(|e| format!("Failed to remove {}: {}", rec.image_path, e))?;
            }
        }
        // Also remove home_directory symlink if it exists and is distinct
        let hd = Path::new(&rec.home_directory);
        if hd != image
            && hd.exists()
            && hd
                .symlink_metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
        {
            let _ = fs::remove_file(hd);
        }
        Ok(())
    }

    /// Activate (mount/make available) a home directory.
    pub fn activate(&mut self, user_name: &str) -> Result<String, String> {
        // First pass: check preconditions with immutable access
        {
            let rec = match self.homes.get(user_name) {
                Some(r) => r,
                None => return Err(format!("Unknown user: {}", user_name)),
            };
            match rec.state {
                HomeState::Active => {
                    return Ok(format!("Home for '{}' is already active", user_name));
                }
                HomeState::Locked => {
                    return Err(format!("Home for '{}' is locked, unlock first", user_name));
                }
                HomeState::Activating | HomeState::Deactivating => {
                    return Err(format!("Home for '{}' is busy ({})", user_name, rec.state));
                }
                _ => {}
            }
        }

        // Check if image exists; if not, mark absent and bail
        let image_path = self.homes[user_name].image_path.clone();
        if !Path::new(&image_path).exists() {
            self.homes.get_mut(user_name).unwrap().state = HomeState::Absent;
            let _ = self.save_one(user_name);
            return Err(format!("Home area absent: {}", image_path));
        }

        // Second pass: perform activation with mutable access
        let rec = self.homes.get_mut(user_name).unwrap();
        rec.state = HomeState::Activating;

        // For directory storage, create symlink from homeDirectory to imagePath
        // (if they differ) or just mark as active.
        match rec.storage {
            Storage::Directory | Storage::Subvolume => {
                let hd = Path::new(&rec.home_directory);
                let img = Path::new(&rec.image_path);
                if hd != img {
                    // Ensure parent exists
                    if let Some(parent) = hd.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    // Create symlink (or it already exists)
                    if !hd.exists() {
                        unix_fs::symlink(img, hd).map_err(|e| {
                            format!(
                                "Failed to symlink {} -> {}: {}",
                                rec.home_directory, rec.image_path, e
                            )
                        })?;
                    }
                }
                // Update disk usage
                rec.disk_usage = Some(dir_disk_usage(img));
            }
            Storage::Luks | Storage::Cifs | Storage::Fscrypt => {
                rec.state = HomeState::Inactive;
                return Err(format!("{} activation is not yet implemented", rec.storage));
            }
        }

        rec.state = HomeState::Active;
        let _ = self.save_runtime_state(user_name);
        let _ = self.save_one(user_name);
        Ok(format!("Activated home for '{}'", user_name))
    }

    /// Deactivate (unmount/lock) a home directory.
    pub fn deactivate(&mut self, user_name: &str) -> Result<String, String> {
        let rec = match self.homes.get_mut(user_name) {
            Some(r) => r,
            None => return Err(format!("Unknown user: {}", user_name)),
        };
        match rec.state {
            HomeState::Inactive | HomeState::Absent => {
                return Ok(format!("Home for '{}' is already inactive", user_name));
            }
            HomeState::Activating | HomeState::Deactivating => {
                return Err(format!("Home for '{}' is busy ({})", user_name, rec.state));
            }
            _ => {}
        }

        rec.state = HomeState::Deactivating;

        // Remove symlink if homeDirectory != imagePath
        let hd = Path::new(&rec.home_directory);
        let img = Path::new(&rec.image_path);
        if hd != img
            && hd
                .symlink_metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
        {
            let _ = fs::remove_file(hd);
        }

        rec.state = HomeState::Inactive;
        rec.locked = false;
        let _ = self.save_runtime_state(user_name);
        let _ = self.save_one(user_name);
        self.remove_runtime_state(user_name);
        Ok(format!("Deactivated home for '{}'", user_name))
    }

    /// Lock a home directory (for suspend-to-RAM protection).
    pub fn lock(&mut self, user_name: &str) -> Result<String, String> {
        let rec = match self.homes.get_mut(user_name) {
            Some(r) => r,
            None => return Err(format!("Unknown user: {}", user_name)),
        };
        if rec.state != HomeState::Active {
            return Err(format!(
                "Home for '{}' is not active ({})",
                user_name, rec.state
            ));
        }
        rec.state = HomeState::Locked;
        rec.locked = true;
        let _ = self.save_runtime_state(user_name);
        let _ = self.save_one(user_name);
        Ok(format!("Locked home for '{}'", user_name))
    }

    /// Unlock a home directory (after resume).
    pub fn unlock(&mut self, user_name: &str) -> Result<String, String> {
        let rec = match self.homes.get_mut(user_name) {
            Some(r) => r,
            None => return Err(format!("Unknown user: {}", user_name)),
        };
        if rec.state != HomeState::Locked {
            return Err(format!(
                "Home for '{}' is not locked ({})",
                user_name, rec.state
            ));
        }
        rec.state = HomeState::Active;
        rec.locked = false;
        let _ = self.save_runtime_state(user_name);
        let _ = self.save_one(user_name);
        Ok(format!("Unlocked home for '{}'", user_name))
    }

    /// Lock all active home directories.
    pub fn lock_all(&mut self) -> String {
        let active_users: Vec<String> = self
            .homes
            .iter()
            .filter(|(_, r)| r.state == HomeState::Active)
            .map(|(name, _)| name.clone())
            .collect();
        let mut locked = 0usize;
        for name in &active_users {
            if self.lock(name).is_ok() {
                locked += 1;
            }
        }
        format!("Locked {} home(s)", locked)
    }

    /// Deactivate all active/locked home directories.
    pub fn deactivate_all(&mut self) -> String {
        let users: Vec<String> = self
            .homes
            .iter()
            .filter(|(_, r)| matches!(r.state, HomeState::Active | HomeState::Locked))
            .map(|(name, _)| name.clone())
            .collect();
        let mut deactivated = 0usize;
        for name in &users {
            // Must unlock first if locked
            if self.homes.get(name.as_str()).map(|r| r.state) == Some(HomeState::Locked) {
                let _ = self.unlock(name);
            }
            if self.deactivate(name).is_ok() {
                deactivated += 1;
            }
        }
        format!("Deactivated {} home(s)", deactivated)
    }

    /// Update user record fields.
    pub fn update(
        &mut self,
        user_name: &str,
        real_name: Option<&str>,
        shell: Option<&str>,
        password_hint: Option<&str>,
        auto_login: Option<bool>,
    ) -> Result<String, String> {
        let rec = match self.homes.get_mut(user_name) {
            Some(r) => r,
            None => return Err(format!("Unknown user: {}", user_name)),
        };
        if let Some(rn) = real_name {
            rec.real_name = rn.to_string();
        }
        if let Some(sh) = shell {
            rec.shell = sh.to_string();
        }
        if let Some(hint) = password_hint {
            rec.password_hint = if hint.is_empty() {
                None
            } else {
                Some(hint.to_string())
            };
        }
        if let Some(al) = auto_login {
            rec.auto_login = al;
        }
        rec.last_change_usec = now_usec();
        let _ = self.save_one(user_name);
        Ok(format!("Updated record for '{}'", user_name))
    }

    /// Change password for a managed user.
    pub fn change_password(
        &mut self,
        user_name: &str,
        new_password: &str,
    ) -> Result<String, String> {
        let rec = match self.homes.get_mut(user_name) {
            Some(r) => r,
            None => return Err(format!("Unknown user: {}", user_name)),
        };
        if new_password.is_empty() {
            return Err("Password must not be empty".to_string());
        }
        let hashed = hash_password(new_password);
        rec.hashed_passwords = vec![hashed];
        rec.last_password_change_usec = now_usec();
        rec.last_change_usec = now_usec();
        let _ = self.save_one(user_name);
        Ok(format!("Password changed for '{}'", user_name))
    }

    /// Resize a home area (directory: no-op; LUKS: would resize image).
    pub fn resize(&mut self, user_name: &str, new_size: u64) -> Result<String, String> {
        let rec = match self.homes.get_mut(user_name) {
            Some(r) => r,
            None => return Err(format!("Unknown user: {}", user_name)),
        };
        match rec.storage {
            Storage::Directory | Storage::Subvolume => {
                // For plain directories, disk_size is just advisory metadata
                rec.disk_size = Some(new_size);
                rec.last_change_usec = now_usec();
                let _ = self.save_one(user_name);
                Ok(format!(
                    "Updated disk size for '{}' to {}",
                    user_name,
                    format_bytes(new_size)
                ))
            }
            _ => Err(format!(
                "Resize not implemented for {} storage",
                rec.storage
            )),
        }
    }

    /// Garbage-collect: check for homes whose image areas have disappeared.
    pub fn gc(&mut self) {
        let names: Vec<String> = self.homes.keys().cloned().collect();
        for name in names {
            if let Some(rec) = self.homes.get_mut(&name)
                && (rec.state == HomeState::Active || rec.state == HomeState::Locked)
            {
                let img = Path::new(&rec.image_path);
                if !img.exists() {
                    log::warn!(
                        "Home area for '{}' has disappeared: {}",
                        name,
                        rec.image_path
                    );
                    rec.state = HomeState::Absent;
                    let _ = self.save_runtime_state(&name);
                    let _ = self.save_one(&name);
                }
            }
        }
    }

    /// Format a list table of all homes.
    pub fn format_list(&self) -> String {
        if self.homes.is_empty() {
            return "No managed home directories.\n".to_string();
        }
        let mut s = String::new();
        s.push_str(&format!(
            "{:<16} {:>6} {:>6} {:<12} {:<10} {}\n",
            "NAME", "UID", "GID", "STATE", "STORAGE", "HOME"
        ));
        for rec in self.homes.values() {
            s.push_str(&format!(
                "{:<16} {:>6} {:>6} {:<12} {:<10} {}\n",
                rec.user_name, rec.uid, rec.gid, rec.state, rec.storage, rec.home_directory
            ));
        }
        s.push_str(&format!("\n{} home(s) listed.\n", self.homes.len()));
        s
    }
}

// ---------------------------------------------------------------------------
// Control socket command handling
// ---------------------------------------------------------------------------

/// Handle a control command and return a response string.
pub fn handle_control_command(registry: &mut HomeRegistry, command: &str) -> String {
    let command = command.trim();
    if command.is_empty() {
        return "ERROR: empty command\n".to_string();
    }

    // Split into verb and args (case-insensitive verb)
    let mut parts = command.splitn(2, ' ');
    let verb = parts.next().unwrap_or("").to_ascii_uppercase();
    let args = parts.next().unwrap_or("").trim();

    match verb.as_str() {
        "PING" => "PONG\n".to_string(),

        "LIST" => registry.format_list(),

        "INSPECT" => {
            if args.is_empty() {
                return "ERROR: INSPECT requires a user name\n".to_string();
            }
            match registry.get(args) {
                Some(rec) => rec.format_inspect(),
                None => format!("ERROR: unknown user '{}'\n", args),
            }
        }

        "SHOW" => {
            if args.is_empty() {
                return "ERROR: SHOW requires a user name\n".to_string();
            }
            match registry.get(args) {
                Some(rec) => rec.format_show(),
                None => format!("ERROR: unknown user '{}'\n", args),
            }
        }

        "RECORD" => {
            if args.is_empty() {
                return "ERROR: RECORD requires a user name\n".to_string();
            }
            match registry.get(args) {
                Some(rec) => rec.to_json() + "\n",
                None => format!("ERROR: unknown user '{}'\n", args),
            }
        }

        "CREATE" => {
            // CREATE <username> [<real_name>] [storage=<type>] [shell=<path>] [password=<pw>]
            let create_args = parse_create_args(args);
            match create_args {
                Ok(ca) => match registry.create(CreateParams {
                    user_name: &ca.user_name,
                    real_name: ca.real_name.as_deref(),
                    shell: ca.shell.as_deref(),
                    storage: ca.storage,
                    password: ca.password.as_deref(),
                    home_dir_override: ca.home_dir.as_deref(),
                    image_path_override: ca.image_path.as_deref(),
                }) {
                    Ok(msg) => format!("{}\n", msg),
                    Err(e) => format!("ERROR: {}\n", e),
                },
                Err(e) => format!("ERROR: {}\n", e),
            }
        }

        "REMOVE" => {
            if args.is_empty() {
                return "ERROR: REMOVE requires a user name\n".to_string();
            }
            match registry.remove(args) {
                Ok(msg) => format!("{}\n", msg),
                Err(e) => format!("ERROR: {}\n", e),
            }
        }

        "ACTIVATE" => {
            if args.is_empty() {
                return "ERROR: ACTIVATE requires a user name\n".to_string();
            }
            match registry.activate(args) {
                Ok(msg) => format!("{}\n", msg),
                Err(e) => format!("ERROR: {}\n", e),
            }
        }

        "DEACTIVATE" => {
            if args.is_empty() {
                return "ERROR: DEACTIVATE requires a user name\n".to_string();
            }
            match registry.deactivate(args) {
                Ok(msg) => format!("{}\n", msg),
                Err(e) => format!("ERROR: {}\n", e),
            }
        }

        "LOCK" => {
            if args.is_empty() {
                return "ERROR: LOCK requires a user name\n".to_string();
            }
            match registry.lock(args) {
                Ok(msg) => format!("{}\n", msg),
                Err(e) => format!("ERROR: {}\n", e),
            }
        }

        "UNLOCK" => {
            if args.is_empty() {
                return "ERROR: UNLOCK requires a user name\n".to_string();
            }
            match registry.unlock(args) {
                Ok(msg) => format!("{}\n", msg),
                Err(e) => format!("ERROR: {}\n", e),
            }
        }

        "LOCK-ALL" => {
            let msg = registry.lock_all();
            format!("{}\n", msg)
        }

        "DEACTIVATE-ALL" => {
            let msg = registry.deactivate_all();
            format!("{}\n", msg)
        }

        "UPDATE" => {
            // UPDATE <username> [realname=<val>] [shell=<val>] [password-hint=<val>] [auto-login=<bool>]
            let ua = parse_update_args(args);
            match ua {
                Ok(ua) => match registry.update(
                    &ua.user_name,
                    ua.real_name.as_deref(),
                    ua.shell.as_deref(),
                    ua.password_hint.as_deref(),
                    ua.auto_login,
                ) {
                    Ok(msg) => format!("{}\n", msg),
                    Err(e) => format!("ERROR: {}\n", e),
                },
                Err(e) => format!("ERROR: {}\n", e),
            }
        }

        "PASSWD" => {
            // PASSWD <username> <new_password>
            let mut parts = args.splitn(2, ' ');
            let name = parts.next().unwrap_or("");
            let pw = parts.next().unwrap_or("").trim();
            if name.is_empty() || pw.is_empty() {
                return "ERROR: PASSWD requires <username> <new_password>\n".to_string();
            }
            match registry.change_password(name, pw) {
                Ok(msg) => format!("{}\n", msg),
                Err(e) => format!("ERROR: {}\n", e),
            }
        }

        "RESIZE" => {
            // RESIZE <username> <size_bytes>
            let mut parts = args.splitn(2, ' ');
            let name = parts.next().unwrap_or("");
            let size_str = parts.next().unwrap_or("").trim();
            if name.is_empty() || size_str.is_empty() {
                return "ERROR: RESIZE requires <username> <size_bytes>\n".to_string();
            }
            match parse_size(size_str) {
                Some(sz) => match registry.resize(name, sz) {
                    Ok(msg) => format!("{}\n", msg),
                    Err(e) => format!("ERROR: {}\n", e),
                },
                None => format!("ERROR: invalid size '{}'\n", size_str),
            }
        }

        "GC" => {
            registry.gc();
            "OK\n".to_string()
        }

        "RELOAD" => {
            registry.load();
            format!("Reloaded, {} home(s)\n", registry.len())
        }

        _ => format!("ERROR: unknown command '{}'\n", verb),
    }
}

// ---------------------------------------------------------------------------
// Argument parsing helpers
// ---------------------------------------------------------------------------

struct CreateArgs {
    user_name: String,
    real_name: Option<String>,
    shell: Option<String>,
    storage: Storage,
    password: Option<String>,
    home_dir: Option<String>,
    image_path: Option<String>,
}

fn parse_create_args(args: &str) -> Result<CreateArgs, String> {
    let tokens: Vec<&str> = args.split_whitespace().collect();
    if tokens.is_empty() {
        return Err("CREATE requires at least a user name".to_string());
    }
    let user_name = tokens[0].to_string();
    let mut real_name = None;
    let mut shell = None;
    let mut storage = Storage::Directory;
    let mut password = None;
    let mut home_dir = None;
    let mut image_path = None;

    for tok in &tokens[1..] {
        if let Some(val) = tok.strip_prefix("realname=") {
            real_name = Some(val.to_string());
        } else if let Some(val) = tok.strip_prefix("shell=") {
            shell = Some(val.to_string());
        } else if let Some(val) = tok.strip_prefix("storage=") {
            storage =
                Storage::parse(val).ok_or_else(|| format!("unknown storage type: {}", val))?;
        } else if let Some(val) = tok.strip_prefix("password=") {
            password = Some(val.to_string());
        } else if let Some(val) = tok.strip_prefix("home=") {
            home_dir = Some(val.to_string());
        } else if let Some(val) = tok.strip_prefix("image=") {
            image_path = Some(val.to_string());
        } else if real_name.is_none() {
            // Treat first non-option arg as real name
            real_name = Some(tok.to_string());
        }
    }

    Ok(CreateArgs {
        user_name,
        real_name,
        shell,
        storage,
        password,
        home_dir,
        image_path,
    })
}

struct UpdateArgs {
    user_name: String,
    real_name: Option<String>,
    shell: Option<String>,
    password_hint: Option<String>,
    auto_login: Option<bool>,
}

fn parse_update_args(args: &str) -> Result<UpdateArgs, String> {
    let tokens: Vec<&str> = args.split_whitespace().collect();
    if tokens.is_empty() {
        return Err("UPDATE requires at least a user name".to_string());
    }
    let user_name = tokens[0].to_string();
    let mut real_name = None;
    let mut shell = None;
    let mut password_hint = None;
    let mut auto_login = None;

    for tok in &tokens[1..] {
        if let Some(val) = tok.strip_prefix("realname=") {
            real_name = Some(val.to_string());
        } else if let Some(val) = tok.strip_prefix("shell=") {
            shell = Some(val.to_string());
        } else if let Some(val) = tok.strip_prefix("password-hint=") {
            password_hint = Some(val.to_string());
        } else if let Some(val) = tok.strip_prefix("auto-login=") {
            auto_login = Some(val == "true" || val == "yes" || val == "1");
        }
    }

    Ok(UpdateArgs {
        user_name,
        real_name,
        shell,
        password_hint,
        auto_login,
    })
}

/// Parse a size string like "1G", "500M", "1073741824".
fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (num_str, mult) = if let Some(n) = s.strip_suffix('T') {
        (n, 1024u64 * 1024 * 1024 * 1024)
    } else if let Some(n) = s.strip_suffix('G') {
        (n, 1024u64 * 1024 * 1024)
    } else if let Some(n) = s.strip_suffix('M') {
        (n, 1024u64 * 1024)
    } else if let Some(n) = s.strip_suffix('K') {
        (n, 1024u64)
    } else {
        (s, 1u64)
    };

    num_str.trim().parse::<u64>().ok().map(|v| v * mult)
}

// ---------------------------------------------------------------------------
// Client handling
// ---------------------------------------------------------------------------

fn handle_client(registry: &mut HomeRegistry, stream: &mut UnixStream) {
    let reader = BufReader::new(stream.try_clone().expect("failed to clone control stream"));
    if let Some(Ok(cmd)) = reader.lines().next() {
        let resp = handle_control_command(registry, &cmd);
        let _ = stream.write_all(resp.as_bytes());
    }
}

// ---------------------------------------------------------------------------
// sd_notify
// ---------------------------------------------------------------------------

fn sd_notify(msg: &str) {
    if let Ok(path) = env::var("NOTIFY_SOCKET") {
        let path = if let Some(stripped) = path.strip_prefix('@') {
            // Abstract socket
            format!("\0{}", stripped)
        } else {
            path
        };
        if let Ok(sock) = std::os::unix::net::UnixDatagram::unbound() {
            let _ = sock.send_to(msg.as_bytes(), &path);
        }
    }
}

// ---------------------------------------------------------------------------
// Signal handling
// ---------------------------------------------------------------------------

static SHUTDOWN: AtomicBool = AtomicBool::new(false);
static RELOAD: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_sigterm(_: libc::c_int) {
    SHUTDOWN.store(true, Ordering::SeqCst);
}

extern "C" fn handle_sigint(_: libc::c_int) {
    SHUTDOWN.store(true, Ordering::SeqCst);
}

extern "C" fn handle_sighup(_: libc::c_int) {
    RELOAD.store(true, Ordering::SeqCst);
}

fn setup_signal_handlers() {
    unsafe {
        libc::signal(libc::SIGTERM, handle_sigterm as libc::sighandler_t);
        libc::signal(libc::SIGINT, handle_sigint as libc::sighandler_t);
        libc::signal(libc::SIGHUP, handle_sighup as libc::sighandler_t);
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
                    "[{}] systemd-homed: {}: {}",
                    ts,
                    record.level(),
                    record.args()
                );
            }
        }
        fn flush(&self) {}
    }

    static LOGGER: StderrLogger = StderrLogger;
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Info);
}

fn chrono_lite_timestamp() -> String {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    let secs = d.as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

fn parse_watchdog_usec(s: &str) -> Option<Duration> {
    let usec: u64 = s.trim().parse().ok()?;
    if usec == 0 {
        None
    } else {
        Some(Duration::from_micros(usec / 2))
    }
}

fn watchdog_interval() -> Option<Duration> {
    env::var("WATCHDOG_USEC")
        .ok()
        .and_then(|s| parse_watchdog_usec(&s))
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    init_logging();
    setup_signal_handlers();

    log::info!("systemd-homed starting");

    // Ensure directories exist
    let _ = fs::create_dir_all(IDENTITY_DIR);
    let _ = fs::create_dir_all(RUNTIME_DIR);

    // Load existing records
    let mut registry = HomeRegistry::new();
    registry.load();
    log::info!("Loaded {} managed home(s)", registry.len());

    // Watchdog support
    let wd_interval = watchdog_interval();
    if let Some(ref iv) = wd_interval {
        log::info!("Watchdog enabled, interval {:?}", iv);
    }
    let mut last_watchdog = Instant::now();

    // Remove stale socket
    let _ = fs::remove_file(CONTROL_SOCKET_PATH);

    // Ensure parent dir exists
    let _ = fs::create_dir_all(Path::new(CONTROL_SOCKET_PATH).parent().unwrap());

    // Bind control socket
    let listener = match UnixListener::bind(CONTROL_SOCKET_PATH) {
        Ok(l) => {
            log::info!("Listening on {}", CONTROL_SOCKET_PATH);
            l
        }
        Err(e) => {
            log::error!(
                "Failed to bind control socket {}: {}",
                CONTROL_SOCKET_PATH,
                e
            );
            sd_notify(&format!(
                "READY=1\nSTATUS=Running (no control socket), {} home(s)",
                registry.len()
            ));
            loop {
                if SHUTDOWN.load(Ordering::SeqCst) {
                    break;
                }
                if let Some(ref iv) = wd_interval
                    && last_watchdog.elapsed() >= *iv
                {
                    sd_notify("WATCHDOG=1");
                    last_watchdog = Instant::now();
                }
                thread::sleep(Duration::from_secs(1));
            }
            sd_notify("STOPPING=1");
            process::exit(0);
        }
    };

    listener
        .set_nonblocking(true)
        .expect("Failed to set non-blocking");

    sd_notify(&format!(
        "READY=1\nSTATUS={} home(s) managed",
        registry.len()
    ));

    log::info!("systemd-homed ready");

    let mut gc_counter = 0u32;

    // Main loop
    loop {
        if SHUTDOWN.load(Ordering::SeqCst) {
            log::info!("Received shutdown signal");
            break;
        }

        if RELOAD.load(Ordering::SeqCst) {
            RELOAD.store(false, Ordering::SeqCst);
            registry.load();
            log::info!("Reloaded, {} managed home(s)", registry.len());
            sd_notify(&format!("STATUS={} home(s) managed", registry.len()));
        }

        // Watchdog keepalive
        if let Some(ref iv) = wd_interval
            && last_watchdog.elapsed() >= *iv
        {
            sd_notify("WATCHDOG=1");
            last_watchdog = Instant::now();
        }

        // Periodic GC (every ~60 iterations ≈ every 12 seconds at 200ms sleep)
        gc_counter += 1;
        if gc_counter >= 60 {
            gc_counter = 0;
            registry.gc();
        }

        match listener.accept() {
            Ok((mut stream, _addr)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                handle_client(&mut registry, &mut stream);
                let _ = stream.shutdown(Shutdown::Both);
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(200));
            }
            Err(e) => {
                log::warn!("Accept error: {}", e);
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    // Cleanup
    let _ = fs::remove_file(CONTROL_SOCKET_PATH);
    sd_notify("STOPPING=1");
    log::info!("systemd-homed stopped");
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // -- Helpers ------------------------------------------------------------

    fn make_registry(tmp: &TempDir) -> HomeRegistry {
        let id_dir = tmp.path().join("identity");
        let rt_dir = tmp.path().join("runtime");
        fs::create_dir_all(&id_dir).unwrap();
        fs::create_dir_all(&rt_dir).unwrap();
        HomeRegistry::with_paths(&id_dir, &rt_dir)
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn create_simple(
        reg: &mut HomeRegistry,
        name: &str,
        home_dir: Option<&str>,
        image_path: Option<&str>,
    ) -> Result<String, String> {
        reg.create(CreateParams {
            user_name: name,
            real_name: None,
            shell: None,
            storage: Storage::Directory,
            password: None,
            home_dir_override: home_dir,
            image_path_override: image_path,
        })
    }

    // -----------------------------------------------------------------------
    // Storage type parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_storage_parse_all() {
        assert_eq!(Storage::parse("directory"), Some(Storage::Directory));
        assert_eq!(Storage::parse("DIRECTORY"), Some(Storage::Directory));
        assert_eq!(Storage::parse("subvolume"), Some(Storage::Subvolume));
        assert_eq!(Storage::parse("luks"), Some(Storage::Luks));
        assert_eq!(Storage::parse("cifs"), Some(Storage::Cifs));
        assert_eq!(Storage::parse("fscrypt"), Some(Storage::Fscrypt));
        assert_eq!(Storage::parse("unknown"), None);
        assert_eq!(Storage::parse(""), None);
    }

    #[test]
    fn test_storage_display() {
        assert_eq!(Storage::Directory.to_string(), "directory");
        assert_eq!(Storage::Luks.to_string(), "luks");
        assert_eq!(Storage::Cifs.to_string(), "cifs");
    }

    // -----------------------------------------------------------------------
    // Home state parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_home_state_all() {
        assert_eq!(HomeState::parse("inactive"), Some(HomeState::Inactive));
        assert_eq!(HomeState::parse("ACTIVE"), Some(HomeState::Active));
        assert_eq!(HomeState::parse("activating"), Some(HomeState::Activating));
        assert_eq!(
            HomeState::parse("deactivating"),
            Some(HomeState::Deactivating)
        );
        assert_eq!(HomeState::parse("locked"), Some(HomeState::Locked));
        assert_eq!(HomeState::parse("absent"), Some(HomeState::Absent));
        assert_eq!(HomeState::parse("dirty"), Some(HomeState::Dirty));
        assert_eq!(HomeState::parse("unknown"), None);
    }

    #[test]
    fn test_home_state_display() {
        assert_eq!(HomeState::Active.to_string(), "active");
        assert_eq!(HomeState::Locked.to_string(), "locked");
    }

    // -----------------------------------------------------------------------
    // Disposition parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_disposition_all() {
        assert_eq!(Disposition::parse("regular"), Some(Disposition::Regular));
        assert_eq!(Disposition::parse("SYSTEM"), Some(Disposition::System));
        assert_eq!(
            Disposition::parse("intrinsic"),
            Some(Disposition::Intrinsic)
        );
        assert_eq!(Disposition::parse("other"), None);
    }

    // -----------------------------------------------------------------------
    // User name validation
    // -----------------------------------------------------------------------

    #[test]
    fn test_valid_user_names() {
        assert!(is_valid_user_name("alice"));
        assert!(is_valid_user_name("_test"));
        assert!(is_valid_user_name("user-1"));
        assert!(is_valid_user_name("user_name123"));
    }

    #[test]
    fn test_invalid_user_names() {
        assert!(!is_valid_user_name(""));
        assert!(!is_valid_user_name("Root")); // uppercase
        assert!(!is_valid_user_name("1user")); // starts with digit
        assert!(!is_valid_user_name("user.name")); // dot not allowed
        assert!(!is_valid_user_name("user name")); // space
        assert!(!is_valid_user_name(&"a".repeat(257))); // too long
    }

    #[test]
    fn test_reserved_user_names() {
        assert!(!is_valid_user_name("root"));
        assert!(!is_valid_user_name("nobody"));
        assert!(!is_valid_user_name("daemon"));
        assert!(!is_valid_user_name("bin"));
        assert!(!is_valid_user_name("sshd"));
    }

    // -----------------------------------------------------------------------
    // User record creation and defaults
    // -----------------------------------------------------------------------

    #[test]
    fn test_user_record_new_defaults() {
        let rec = UserRecord::new("alice", 60001);
        assert_eq!(rec.user_name, "alice");
        assert_eq!(rec.real_name, "alice");
        assert_eq!(rec.uid, 60001);
        assert_eq!(rec.gid, 60001);
        assert_eq!(rec.home_directory, "/home/alice");
        assert_eq!(rec.image_path, "/home/alice.homedir");
        assert_eq!(rec.shell, "/bin/bash");
        assert_eq!(rec.storage, Storage::Directory);
        assert_eq!(rec.disposition, Disposition::Regular);
        assert_eq!(rec.state, HomeState::Inactive);
        assert!(rec.disk_size.is_none());
        assert!(rec.hashed_passwords.is_empty());
        assert!(!rec.locked);
        assert!(!rec.auto_login);
        assert_eq!(rec.service, "io.systemd.Home");
    }

    // -----------------------------------------------------------------------
    // JSON serialization roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_json_roundtrip_basic() {
        let rec = UserRecord::new("testuser", 60005);
        let json = rec.to_json();
        let rec2 = UserRecord::from_json(&json).unwrap();
        assert_eq!(rec.user_name, rec2.user_name);
        assert_eq!(rec.uid, rec2.uid);
        assert_eq!(rec.gid, rec2.gid);
        assert_eq!(rec.storage, rec2.storage);
        assert_eq!(rec.disposition, rec2.disposition);
        assert_eq!(rec.state, rec2.state);
        assert_eq!(rec.shell, rec2.shell);
        assert_eq!(rec.home_directory, rec2.home_directory);
        assert_eq!(rec.image_path, rec2.image_path);
        assert_eq!(rec.locked, rec2.locked);
    }

    #[test]
    fn test_json_roundtrip_with_all_fields() {
        let mut rec = UserRecord::new("bob", 60010);
        rec.real_name = "Bob Smith".to_string();
        rec.member_of = vec!["wheel".to_string(), "users".to_string()];
        rec.disk_size = Some(10 * 1024 * 1024 * 1024);
        rec.disk_usage = Some(512 * 1024 * 1024);
        rec.password_hint = Some("my pet's name".to_string());
        rec.auto_login = true;
        rec.hashed_passwords = vec![hash_password("secret")];
        rec.locked = true;

        let json = rec.to_json();
        let rec2 = UserRecord::from_json(&json).unwrap();
        assert_eq!(rec.real_name, rec2.real_name);
        assert_eq!(rec.member_of, rec2.member_of);
        assert_eq!(rec.disk_size, rec2.disk_size);
        assert_eq!(rec.disk_usage, rec2.disk_usage);
        assert_eq!(rec.password_hint, rec2.password_hint);
        assert_eq!(rec.auto_login, rec2.auto_login);
        assert_eq!(rec.hashed_passwords, rec2.hashed_passwords);
        assert_eq!(rec.locked, rec2.locked);
    }

    #[test]
    fn test_json_roundtrip_null_optional_fields() {
        let rec = UserRecord::new("nulltest", 60020);
        let json = rec.to_json();
        assert!(json.contains("\"diskSize\": null"));
        assert!(json.contains("\"passwordHint\": null"));
        let rec2 = UserRecord::from_json(&json).unwrap();
        assert!(rec2.disk_size.is_none());
        assert!(rec2.password_hint.is_none());
    }

    #[test]
    fn test_json_roundtrip_empty_arrays() {
        let rec = UserRecord::new("emptyarr", 60021);
        let json = rec.to_json();
        assert!(json.contains("\"memberOf\": []"));
        assert!(json.contains("\"hashedPassword\": []"));
        let rec2 = UserRecord::from_json(&json).unwrap();
        assert!(rec2.member_of.is_empty());
        assert!(rec2.hashed_passwords.is_empty());
    }

    #[test]
    fn test_json_escape_special_chars() {
        let mut rec = UserRecord::new("esctest", 60022);
        rec.real_name = "Alice \"Bob\" O'Connor\nLine2".to_string();
        let json = rec.to_json();
        assert!(json.contains("\\\""));
        assert!(json.contains("\\n"));
        let rec2 = UserRecord::from_json(&json).unwrap();
        assert_eq!(rec.real_name, rec2.real_name);
    }

    #[test]
    fn test_json_parse_error_not_object() {
        assert!(UserRecord::from_json("not json").is_err());
        assert!(UserRecord::from_json("[]").is_err());
    }

    #[test]
    fn test_json_parse_error_missing_required_field() {
        assert!(UserRecord::from_json("{}").is_err()); // missing userName
        assert!(UserRecord::from_json("{\"userName\": \"a\"}").is_err()); // missing uid
    }

    // -----------------------------------------------------------------------
    // User record formatting
    // -----------------------------------------------------------------------

    #[test]
    fn test_format_inspect_contains_fields() {
        let rec = UserRecord::new("alice", 60001);
        let s = rec.format_inspect();
        assert!(s.contains("alice"));
        assert!(s.contains("60001"));
        assert!(s.contains("directory"));
        assert!(s.contains("inactive"));
        assert!(s.contains("/home/alice"));
        assert!(s.contains("io.systemd.Home"));
    }

    #[test]
    fn test_format_inspect_with_optional_fields() {
        let mut rec = UserRecord::new("bob", 60002);
        rec.disk_size = Some(1024 * 1024 * 1024);
        rec.disk_usage = Some(512 * 1024);
        rec.password_hint = Some("favorite color".to_string());
        rec.member_of = vec!["wheel".to_string()];
        let s = rec.format_inspect();
        assert!(s.contains("1.0G"));
        assert!(s.contains("512.0K"));
        assert!(s.contains("favorite color"));
        assert!(s.contains("wheel"));
    }

    #[test]
    fn test_format_show_key_value() {
        let rec = UserRecord::new("charlie", 60003);
        let s = rec.format_show();
        assert!(s.contains("UserName=charlie"));
        assert!(s.contains("UID=60003"));
        assert!(s.contains("Storage=directory"));
        assert!(s.contains("State=inactive"));
        assert!(s.contains("Locked=false"));
    }

    // -----------------------------------------------------------------------
    // Password hashing
    // -----------------------------------------------------------------------

    #[test]
    fn test_hash_password_deterministic() {
        let h1 = hash_password("hello");
        let h2 = hash_password("hello");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_password_different_inputs() {
        let h1 = hash_password("hello");
        let h2 = hash_password("world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hash_password_format() {
        let h = hash_password("test");
        assert!(h.starts_with("$6$homed$"));
        assert_eq!(h.len(), "$6$homed$".len() + 128);
    }

    #[test]
    fn test_verify_password_correct() {
        let h = hash_password("mypass");
        assert!(verify_password("mypass", &h));
    }

    #[test]
    fn test_verify_password_incorrect() {
        let h = hash_password("mypass");
        assert!(!verify_password("wrongpass", &h));
    }

    // -----------------------------------------------------------------------
    // Size parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_size_bytes() {
        assert_eq!(parse_size("1024"), Some(1024));
        assert_eq!(parse_size("0"), Some(0));
    }

    #[test]
    fn test_parse_size_units() {
        assert_eq!(parse_size("1K"), Some(1024));
        assert_eq!(parse_size("1M"), Some(1024 * 1024));
        assert_eq!(parse_size("1G"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_size("2T"), Some(2 * 1024 * 1024 * 1024 * 1024));
    }

    #[test]
    fn test_parse_size_invalid() {
        assert_eq!(parse_size(""), None);
        assert_eq!(parse_size("abc"), None);
        assert_eq!(parse_size("G"), None);
    }

    // -----------------------------------------------------------------------
    // format_bytes
    // -----------------------------------------------------------------------

    #[test]
    fn test_format_bytes_scales() {
        assert_eq!(format_bytes(0), "0B");
        assert_eq!(format_bytes(512), "512B");
        assert_eq!(format_bytes(1024), "1.0K");
        assert_eq!(format_bytes(1024 * 1024), "1.0M");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0G");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 1024), "1.0T");
    }

    // -----------------------------------------------------------------------
    // Home registry: create
    // -----------------------------------------------------------------------

    #[test]
    fn test_registry_create_basic() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let home_base = tmp.path().join("homes");
        fs::create_dir_all(&home_base).unwrap();
        let image = home_base.join("alice.homedir");

        let result = reg.create(CreateParams {
            user_name: "alice",
            real_name: Some("Alice"),
            shell: None,
            storage: Storage::Directory,
            password: Some("pass123"),
            home_dir_override: None,
            image_path_override: Some(image.to_str().unwrap()),
        });
        assert!(result.is_ok(), "create failed: {:?}", result);
        assert!(reg.contains("alice"));

        let rec = reg.get("alice").unwrap();
        assert_eq!(rec.uid, UID_MIN);
        assert_eq!(rec.real_name, "Alice");
        assert!(!rec.hashed_passwords.is_empty());
        assert!(image.exists());
    }

    #[test]
    fn test_registry_create_duplicate() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img1 = tmp.path().join("u1.homedir");
        let img2 = tmp.path().join("u2.homedir");

        create_simple(&mut reg, "alice", None, Some(img1.to_str().unwrap())).unwrap();
        let result = create_simple(&mut reg, "alice", None, Some(img2.to_str().unwrap()));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
    }

    #[test]
    fn test_registry_create_invalid_name() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);

        assert!(create_simple(&mut reg, "", None, None).is_err());
        assert!(create_simple(&mut reg, "root", None, None).is_err());
        assert!(create_simple(&mut reg, "1bad", None, None).is_err());
    }

    #[test]
    fn test_registry_create_uid_allocation() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);

        for i in 0..3 {
            let name = format!("user{}", i);
            let img = tmp.path().join(format!("{}.homedir", name));
            create_simple(&mut reg, &name, None, Some(img.to_str().unwrap())).unwrap();
        }
        assert_eq!(reg.get("user0").unwrap().uid, UID_MIN);
        assert_eq!(reg.get("user1").unwrap().uid, UID_MIN + 1);
        assert_eq!(reg.get("user2").unwrap().uid, UID_MIN + 2);
    }

    #[test]
    fn test_registry_create_unsupported_storage() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);

        let result = reg.create(CreateParams {
            user_name: "luks-user",
            real_name: None,
            shell: None,
            storage: Storage::Luks,
            password: None,
            home_dir_override: None,
            image_path_override: None,
        });
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not yet implemented"));
    }

    // -----------------------------------------------------------------------
    // Home registry: remove
    // -----------------------------------------------------------------------

    #[test]
    fn test_registry_remove_basic() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        create_simple(&mut reg, "alice", None, Some(img.to_str().unwrap())).unwrap();
        assert!(img.exists());

        let result = reg.remove("alice");
        assert!(result.is_ok());
        assert!(!reg.contains("alice"));
        assert!(!img.exists());
    }

    #[test]
    fn test_registry_remove_unknown() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        assert!(reg.remove("nonexistent").is_err());
    }

    #[test]
    fn test_registry_remove_active_fails() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        let hd = tmp.path().join("home_alice");
        create_simple(
            &mut reg,
            "alice",
            Some(hd.to_str().unwrap()),
            Some(img.to_str().unwrap()),
        )
        .unwrap();
        reg.activate("alice").unwrap();

        let result = reg.remove("alice");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("still active"));
    }

    // -----------------------------------------------------------------------
    // Home registry: activate / deactivate
    // -----------------------------------------------------------------------

    #[test]
    fn test_registry_activate_deactivate() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        let hd = tmp.path().join("home_alice");

        create_simple(
            &mut reg,
            "alice",
            Some(hd.to_str().unwrap()),
            Some(img.to_str().unwrap()),
        )
        .unwrap();

        // Activate
        let result = reg.activate("alice");
        assert!(result.is_ok(), "activate: {:?}", result);
        assert_eq!(reg.get("alice").unwrap().state, HomeState::Active);
        // symlink should exist
        assert!(hd.symlink_metadata().unwrap().file_type().is_symlink());

        // Deactivate
        let result = reg.deactivate("alice");
        assert!(result.is_ok());
        assert_eq!(reg.get("alice").unwrap().state, HomeState::Inactive);
        // symlink should be removed
        assert!(!hd.exists());
    }

    #[test]
    fn test_registry_activate_already_active() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        let hd = tmp.path().join("home_alice");
        create_simple(
            &mut reg,
            "alice",
            Some(hd.to_str().unwrap()),
            Some(img.to_str().unwrap()),
        )
        .unwrap();
        reg.activate("alice").unwrap();
        // Second activate should succeed (already active)
        let result = reg.activate("alice");
        assert!(result.is_ok());
        assert!(result.unwrap().contains("already active"));
    }

    #[test]
    fn test_registry_activate_absent() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("nonexistent.homedir");
        // Create record but don't create the directory
        let mut rec = UserRecord::new("ghost", UID_MIN);
        rec.image_path = img.to_str().unwrap().to_string();
        let id_dir = tmp.path().join("identity");
        write_file(&id_dir.join("ghost.identity"), &rec.to_json());
        reg.load();

        let result = reg.activate("ghost");
        assert!(result.is_err());
        assert_eq!(reg.get("ghost").unwrap().state, HomeState::Absent);
    }

    #[test]
    fn test_registry_deactivate_already_inactive() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        create_simple(&mut reg, "alice", None, Some(img.to_str().unwrap())).unwrap();
        let result = reg.deactivate("alice");
        assert!(result.is_ok());
        assert!(result.unwrap().contains("already inactive"));
    }

    #[test]
    fn test_registry_activate_unknown() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        assert!(reg.activate("nobody-here").is_err());
    }

    // -----------------------------------------------------------------------
    // Home registry: lock / unlock
    // -----------------------------------------------------------------------

    #[test]
    fn test_registry_lock_unlock() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        let hd = tmp.path().join("home_alice");
        create_simple(
            &mut reg,
            "alice",
            Some(hd.to_str().unwrap()),
            Some(img.to_str().unwrap()),
        )
        .unwrap();
        reg.activate("alice").unwrap();

        // Lock
        let result = reg.lock("alice");
        assert!(result.is_ok());
        assert_eq!(reg.get("alice").unwrap().state, HomeState::Locked);
        assert!(reg.get("alice").unwrap().locked);

        // Can't activate while locked
        assert!(reg.activate("alice").is_err());

        // Unlock
        let result = reg.unlock("alice");
        assert!(result.is_ok());
        assert_eq!(reg.get("alice").unwrap().state, HomeState::Active);
        assert!(!reg.get("alice").unwrap().locked);
    }

    #[test]
    fn test_registry_lock_not_active() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        create_simple(&mut reg, "alice", None, Some(img.to_str().unwrap())).unwrap();
        assert!(reg.lock("alice").is_err()); // inactive, can't lock
    }

    #[test]
    fn test_registry_unlock_not_locked() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        let hd = tmp.path().join("home_alice");
        create_simple(
            &mut reg,
            "alice",
            Some(hd.to_str().unwrap()),
            Some(img.to_str().unwrap()),
        )
        .unwrap();
        reg.activate("alice").unwrap();
        assert!(reg.unlock("alice").is_err()); // active but not locked
    }

    // -----------------------------------------------------------------------
    // Home registry: lock-all / deactivate-all
    // -----------------------------------------------------------------------

    #[test]
    fn test_registry_lock_all() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        for i in 0..3 {
            let name = format!("user{}", i);
            let img = tmp.path().join(format!("{}.homedir", name));
            let hd = tmp.path().join(format!("home_{}", name));
            create_simple(
                &mut reg,
                &name,
                Some(hd.to_str().unwrap()),
                Some(img.to_str().unwrap()),
            )
            .unwrap();
            reg.activate(&name).unwrap();
        }
        let msg = reg.lock_all();
        assert!(msg.contains("3"));
        for i in 0..3 {
            assert_eq!(
                reg.get(&format!("user{}", i)).unwrap().state,
                HomeState::Locked
            );
        }
    }

    #[test]
    fn test_registry_deactivate_all() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        for i in 0..3 {
            let name = format!("user{}", i);
            let img = tmp.path().join(format!("{}.homedir", name));
            let hd = tmp.path().join(format!("home_{}", name));
            create_simple(
                &mut reg,
                &name,
                Some(hd.to_str().unwrap()),
                Some(img.to_str().unwrap()),
            )
            .unwrap();
            reg.activate(&name).unwrap();
        }
        // Lock one of them
        reg.lock("user1").unwrap();

        let msg = reg.deactivate_all();
        assert!(msg.contains("3"));
        for i in 0..3 {
            assert_eq!(
                reg.get(&format!("user{}", i)).unwrap().state,
                HomeState::Inactive
            );
        }
    }

    // -----------------------------------------------------------------------
    // Home registry: update
    // -----------------------------------------------------------------------

    #[test]
    fn test_registry_update_fields() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        create_simple(&mut reg, "alice", None, Some(img.to_str().unwrap())).unwrap();

        let result = reg.update(
            "alice",
            Some("Alice Smith"),
            Some("/bin/zsh"),
            Some("hint"),
            Some(true),
        );
        assert!(result.is_ok());

        let rec = reg.get("alice").unwrap();
        assert_eq!(rec.real_name, "Alice Smith");
        assert_eq!(rec.shell, "/bin/zsh");
        assert_eq!(rec.password_hint, Some("hint".to_string()));
        assert!(rec.auto_login);
    }

    #[test]
    fn test_registry_update_partial() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        reg.create(CreateParams {
            user_name: "alice",
            real_name: Some("Alice"),
            shell: None,
            storage: Storage::Directory,
            password: None,
            home_dir_override: None,
            image_path_override: Some(img.to_str().unwrap()),
        })
        .unwrap();

        // Only update shell, leave real_name as-is
        reg.update("alice", None, Some("/bin/fish"), None, None)
            .unwrap();
        let rec = reg.get("alice").unwrap();
        assert_eq!(rec.real_name, "Alice");
        assert_eq!(rec.shell, "/bin/fish");
    }

    #[test]
    fn test_registry_update_unknown_user() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        assert!(reg.update("ghost", Some("x"), None, None, None).is_err());
    }

    // -----------------------------------------------------------------------
    // Home registry: change password
    // -----------------------------------------------------------------------

    #[test]
    fn test_registry_change_password() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        reg.create(CreateParams {
            user_name: "alice",
            real_name: None,
            shell: None,
            storage: Storage::Directory,
            password: Some("old"),
            home_dir_override: None,
            image_path_override: Some(img.to_str().unwrap()),
        })
        .unwrap();
        assert!(verify_password(
            "old",
            &reg.get("alice").unwrap().hashed_passwords[0]
        ));

        reg.change_password("alice", "new").unwrap();
        assert!(verify_password(
            "new",
            &reg.get("alice").unwrap().hashed_passwords[0]
        ));
        assert!(!verify_password(
            "old",
            &reg.get("alice").unwrap().hashed_passwords[0]
        ));
    }

    #[test]
    fn test_registry_change_password_empty() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        create_simple(&mut reg, "alice", None, Some(img.to_str().unwrap())).unwrap();
        assert!(reg.change_password("alice", "").is_err());
    }

    // -----------------------------------------------------------------------
    // Home registry: resize
    // -----------------------------------------------------------------------

    #[test]
    fn test_registry_resize_directory() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        create_simple(&mut reg, "alice", None, Some(img.to_str().unwrap())).unwrap();

        let result = reg.resize("alice", 10 * 1024 * 1024 * 1024);
        assert!(result.is_ok());
        assert_eq!(
            reg.get("alice").unwrap().disk_size,
            Some(10 * 1024 * 1024 * 1024)
        );
    }

    #[test]
    fn test_registry_resize_unknown() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        assert!(reg.resize("nobody", 1024).is_err());
    }

    // -----------------------------------------------------------------------
    // Home registry: persistence (save / load)
    // -----------------------------------------------------------------------

    #[test]
    fn test_registry_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let id_dir = tmp.path().join("identity");
        let rt_dir = tmp.path().join("runtime");
        fs::create_dir_all(&id_dir).unwrap();
        fs::create_dir_all(&rt_dir).unwrap();

        // Create and save
        {
            let mut reg = HomeRegistry::with_paths(&id_dir, &rt_dir);
            let img = tmp.path().join("alice.homedir");
            reg.create(CreateParams {
                user_name: "alice",
                real_name: Some("Alice"),
                shell: None,
                storage: Storage::Directory,
                password: None,
                home_dir_override: None,
                image_path_override: Some(img.to_str().unwrap()),
            })
            .unwrap();
            assert!(id_dir.join("alice.identity").exists());
        }

        // Load in new registry
        {
            let mut reg = HomeRegistry::with_paths(&id_dir, &rt_dir);
            reg.load();
            assert!(reg.contains("alice"));
            assert_eq!(reg.get("alice").unwrap().real_name, "Alice");
        }
    }

    #[test]
    fn test_registry_load_skips_invalid() {
        let tmp = TempDir::new().unwrap();
        let id_dir = tmp.path().join("identity");
        let rt_dir = tmp.path().join("runtime");
        fs::create_dir_all(&id_dir).unwrap();
        fs::create_dir_all(&rt_dir).unwrap();

        // Write a valid file
        write_file(
            &id_dir.join("good.identity"),
            &UserRecord::new("good", 60001).to_json(),
        );
        // Write an invalid file
        write_file(&id_dir.join("bad.identity"), "not json");
        // Write a dotfile (should be skipped)
        write_file(&id_dir.join(".hidden.identity"), "{}");
        // Write a non-identity file (should be skipped)
        write_file(&id_dir.join("readme.txt"), "hello");

        let mut reg = HomeRegistry::with_paths(&id_dir, &rt_dir);
        reg.load();
        assert_eq!(reg.len(), 1);
        assert!(reg.contains("good"));
    }

    #[test]
    fn test_registry_load_restores_runtime_state() {
        let tmp = TempDir::new().unwrap();
        let id_dir = tmp.path().join("identity");
        let rt_dir = tmp.path().join("runtime");
        fs::create_dir_all(&id_dir).unwrap();
        fs::create_dir_all(&rt_dir).unwrap();

        write_file(
            &id_dir.join("alice.identity"),
            &UserRecord::new("alice", 60001).to_json(),
        );
        write_file(&rt_dir.join("alice"), "active");

        let mut reg = HomeRegistry::with_paths(&id_dir, &rt_dir);
        reg.load();
        assert_eq!(reg.get("alice").unwrap().state, HomeState::Active);
    }

    #[test]
    fn test_registry_load_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        reg.load();
        assert!(reg.is_empty());
    }

    #[test]
    fn test_registry_load_nonexistent_dir() {
        let tmp = TempDir::new().unwrap();
        let mut reg =
            HomeRegistry::with_paths(&tmp.path().join("nope"), &tmp.path().join("also_nope"));
        reg.load();
        assert!(reg.is_empty());
    }

    // -----------------------------------------------------------------------
    // Home registry: GC
    // -----------------------------------------------------------------------

    #[test]
    fn test_registry_gc_marks_absent() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        let hd = tmp.path().join("home_alice");
        create_simple(
            &mut reg,
            "alice",
            Some(hd.to_str().unwrap()),
            Some(img.to_str().unwrap()),
        )
        .unwrap();
        reg.activate("alice").unwrap();
        assert_eq!(reg.get("alice").unwrap().state, HomeState::Active);

        // Remove the image directory behind the daemon's back
        fs::remove_dir_all(&img).unwrap();
        reg.gc();
        assert_eq!(reg.get("alice").unwrap().state, HomeState::Absent);
    }

    #[test]
    fn test_registry_gc_keeps_present() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        let hd = tmp.path().join("home_alice");
        create_simple(
            &mut reg,
            "alice",
            Some(hd.to_str().unwrap()),
            Some(img.to_str().unwrap()),
        )
        .unwrap();
        reg.activate("alice").unwrap();
        reg.gc();
        assert_eq!(reg.get("alice").unwrap().state, HomeState::Active);
    }

    // -----------------------------------------------------------------------
    // Home registry: format_list
    // -----------------------------------------------------------------------

    #[test]
    fn test_registry_format_list_empty() {
        let tmp = TempDir::new().unwrap();
        let reg = make_registry(&tmp);
        let s = reg.format_list();
        assert!(s.contains("No managed home directories"));
    }

    #[test]
    fn test_registry_format_list_with_homes() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img1 = tmp.path().join("alice.homedir");
        let img2 = tmp.path().join("bob.homedir");
        create_simple(&mut reg, "alice", None, Some(img1.to_str().unwrap())).unwrap();
        create_simple(&mut reg, "bob", None, Some(img2.to_str().unwrap())).unwrap();

        let s = reg.format_list();
        assert!(s.contains("alice"));
        assert!(s.contains("bob"));
        assert!(s.contains("2 home(s) listed"));
    }

    // -----------------------------------------------------------------------
    // Control command handling
    // -----------------------------------------------------------------------

    #[test]
    fn test_control_ping() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        assert_eq!(handle_control_command(&mut reg, "PING"), "PONG\n");
        assert_eq!(handle_control_command(&mut reg, "ping"), "PONG\n");
    }

    #[test]
    fn test_control_empty() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let resp = handle_control_command(&mut reg, "");
        assert!(resp.contains("ERROR"));
    }

    #[test]
    fn test_control_unknown() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let resp = handle_control_command(&mut reg, "FOOBAR");
        assert!(resp.contains("ERROR"));
        assert!(resp.contains("unknown command"));
    }

    #[test]
    fn test_control_list_empty() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let resp = handle_control_command(&mut reg, "LIST");
        assert!(resp.contains("No managed home"));
    }

    #[test]
    fn test_control_create_and_list() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("testuser.homedir");
        let cmd = format!("CREATE testuser image={}", img.display());
        let resp = handle_control_command(&mut reg, &cmd);
        assert!(resp.contains("Created"), "resp: {}", resp);

        let resp = handle_control_command(&mut reg, "LIST");
        assert!(resp.contains("testuser"));
        assert!(resp.contains("1 home(s) listed"));
    }

    #[test]
    fn test_control_create_with_options() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        let cmd = format!(
            "CREATE alice realname=Alice shell=/bin/zsh password=secret image={}",
            img.display()
        );
        let resp = handle_control_command(&mut reg, &cmd);
        assert!(resp.contains("Created"), "resp: {}", resp);

        let rec = reg.get("alice").unwrap();
        assert_eq!(rec.real_name, "Alice");
        assert_eq!(rec.shell, "/bin/zsh");
        assert!(verify_password("secret", &rec.hashed_passwords[0]));
    }

    #[test]
    fn test_control_create_missing_name() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let resp = handle_control_command(&mut reg, "CREATE");
        assert!(resp.contains("ERROR"));
    }

    #[test]
    fn test_control_inspect() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        handle_control_command(&mut reg, &format!("CREATE alice image={}", img.display()));

        let resp = handle_control_command(&mut reg, "INSPECT alice");
        assert!(resp.contains("alice"));
        assert!(resp.contains("directory"));
        assert!(resp.contains("inactive"));
    }

    #[test]
    fn test_control_inspect_missing_name() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let resp = handle_control_command(&mut reg, "INSPECT");
        assert!(resp.contains("ERROR"));
    }

    #[test]
    fn test_control_inspect_unknown_user() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let resp = handle_control_command(&mut reg, "INSPECT ghost");
        assert!(resp.contains("ERROR"));
        assert!(resp.contains("unknown user"));
    }

    #[test]
    fn test_control_show() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        handle_control_command(&mut reg, &format!("CREATE alice image={}", img.display()));

        let resp = handle_control_command(&mut reg, "SHOW alice");
        assert!(resp.contains("UserName=alice"));
        assert!(resp.contains("Storage=directory"));
    }

    #[test]
    fn test_control_record_json() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        handle_control_command(&mut reg, &format!("CREATE alice image={}", img.display()));

        let resp = handle_control_command(&mut reg, "RECORD alice");
        assert!(resp.contains("\"userName\": \"alice\""));
        // Should be valid JSON
        assert!(UserRecord::from_json(resp.trim()).is_ok());
    }

    #[test]
    fn test_control_activate_deactivate() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        let hd = tmp.path().join("home_alice");
        handle_control_command(
            &mut reg,
            &format!("CREATE alice home={} image={}", hd.display(), img.display()),
        );

        let resp = handle_control_command(&mut reg, "ACTIVATE alice");
        assert!(resp.contains("Activated"), "resp: {}", resp);
        assert_eq!(reg.get("alice").unwrap().state, HomeState::Active);

        let resp = handle_control_command(&mut reg, "DEACTIVATE alice");
        assert!(resp.contains("Deactivated"), "resp: {}", resp);
        assert_eq!(reg.get("alice").unwrap().state, HomeState::Inactive);
    }

    #[test]
    fn test_control_activate_missing_name() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let resp = handle_control_command(&mut reg, "ACTIVATE");
        assert!(resp.contains("ERROR"));
    }

    #[test]
    fn test_control_lock_unlock() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        let hd = tmp.path().join("home_alice");
        handle_control_command(
            &mut reg,
            &format!("CREATE alice home={} image={}", hd.display(), img.display()),
        );
        handle_control_command(&mut reg, "ACTIVATE alice");

        let resp = handle_control_command(&mut reg, "LOCK alice");
        assert!(resp.contains("Locked"), "resp: {}", resp);

        let resp = handle_control_command(&mut reg, "UNLOCK alice");
        assert!(resp.contains("Unlocked"), "resp: {}", resp);
    }

    #[test]
    fn test_control_lock_all() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        for i in 0..2 {
            let name = format!("user{}", i);
            let img = tmp.path().join(format!("{}.homedir", name));
            let hd = tmp.path().join(format!("home_{}", name));
            handle_control_command(
                &mut reg,
                &format!(
                    "CREATE {} home={} image={}",
                    name,
                    hd.display(),
                    img.display()
                ),
            );
            handle_control_command(&mut reg, &format!("ACTIVATE {}", name));
        }

        let resp = handle_control_command(&mut reg, "LOCK-ALL");
        assert!(resp.contains("2"), "resp: {}", resp);
    }

    #[test]
    fn test_control_deactivate_all() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        for i in 0..2 {
            let name = format!("user{}", i);
            let img = tmp.path().join(format!("{}.homedir", name));
            let hd = tmp.path().join(format!("home_{}", name));
            handle_control_command(
                &mut reg,
                &format!(
                    "CREATE {} home={} image={}",
                    name,
                    hd.display(),
                    img.display()
                ),
            );
            handle_control_command(&mut reg, &format!("ACTIVATE {}", name));
        }

        let resp = handle_control_command(&mut reg, "DEACTIVATE-ALL");
        assert!(resp.contains("2"), "resp: {}", resp);
    }

    #[test]
    fn test_control_update() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        handle_control_command(&mut reg, &format!("CREATE alice image={}", img.display()));

        let resp = handle_control_command(
            &mut reg,
            "UPDATE alice realname=Alice shell=/bin/fish auto-login=true",
        );
        assert!(resp.contains("Updated"), "resp: {}", resp);
        let rec = reg.get("alice").unwrap();
        assert_eq!(rec.real_name, "Alice");
        assert_eq!(rec.shell, "/bin/fish");
        assert!(rec.auto_login);
    }

    #[test]
    fn test_control_update_missing_name() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let resp = handle_control_command(&mut reg, "UPDATE");
        assert!(resp.contains("ERROR"));
    }

    #[test]
    fn test_control_passwd() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        handle_control_command(
            &mut reg,
            &format!("CREATE alice password=old image={}", img.display()),
        );

        let resp = handle_control_command(&mut reg, "PASSWD alice newpass");
        assert!(resp.contains("Password changed"), "resp: {}", resp);
        assert!(verify_password(
            "newpass",
            &reg.get("alice").unwrap().hashed_passwords[0]
        ));
    }

    #[test]
    fn test_control_passwd_missing_args() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        assert!(handle_control_command(&mut reg, "PASSWD").contains("ERROR"));
        assert!(handle_control_command(&mut reg, "PASSWD alice").contains("ERROR"));
    }

    #[test]
    fn test_control_resize() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        handle_control_command(&mut reg, &format!("CREATE alice image={}", img.display()));

        let resp = handle_control_command(&mut reg, "RESIZE alice 5G");
        assert!(resp.contains("Updated disk size"), "resp: {}", resp);
        assert_eq!(
            reg.get("alice").unwrap().disk_size,
            Some(5 * 1024 * 1024 * 1024)
        );
    }

    #[test]
    fn test_control_resize_invalid_size() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        handle_control_command(&mut reg, &format!("CREATE alice image={}", img.display()));

        let resp = handle_control_command(&mut reg, "RESIZE alice abc");
        assert!(resp.contains("ERROR"));
    }

    #[test]
    fn test_control_resize_missing_args() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        assert!(handle_control_command(&mut reg, "RESIZE").contains("ERROR"));
        assert!(handle_control_command(&mut reg, "RESIZE alice").contains("ERROR"));
    }

    #[test]
    fn test_control_gc() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let resp = handle_control_command(&mut reg, "GC");
        assert_eq!(resp, "OK\n");
    }

    #[test]
    fn test_control_reload() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let resp = handle_control_command(&mut reg, "RELOAD");
        assert!(resp.contains("Reloaded"));
    }

    #[test]
    fn test_control_remove() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        handle_control_command(&mut reg, &format!("CREATE alice image={}", img.display()));
        assert!(reg.contains("alice"));

        let resp = handle_control_command(&mut reg, "REMOVE alice");
        assert!(resp.contains("Removed"), "resp: {}", resp);
        assert!(!reg.contains("alice"));
    }

    #[test]
    fn test_control_remove_missing_name() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let resp = handle_control_command(&mut reg, "REMOVE");
        assert!(resp.contains("ERROR"));
    }

    #[test]
    fn test_control_remove_unknown() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let resp = handle_control_command(&mut reg, "REMOVE ghost");
        assert!(resp.contains("ERROR"));
    }

    #[test]
    fn test_control_case_insensitive() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        assert_eq!(handle_control_command(&mut reg, "ping"), "PONG\n");
        assert_eq!(handle_control_command(&mut reg, "Ping"), "PONG\n");
        assert_eq!(handle_control_command(&mut reg, "PING"), "PONG\n");
        assert!(handle_control_command(&mut reg, "list").contains("No managed"));
        assert!(handle_control_command(&mut reg, "gc").contains("OK"));
    }

    // -----------------------------------------------------------------------
    // Full lifecycle integration
    // -----------------------------------------------------------------------

    #[test]
    fn test_full_lifecycle() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("alice.homedir");
        let hd = tmp.path().join("homes").join("alice");

        // Create
        let resp = handle_control_command(
            &mut reg,
            &format!(
                "CREATE alice realname=Alice password=secret home={} image={}",
                hd.display(),
                img.display()
            ),
        );
        assert!(resp.contains("Created"), "{}", resp);
        assert!(img.exists());

        // Inspect
        let resp = handle_control_command(&mut reg, "INSPECT alice");
        assert!(resp.contains("Alice"));

        // Activate
        let resp = handle_control_command(&mut reg, "ACTIVATE alice");
        assert!(resp.contains("Activated"), "{}", resp);
        assert!(hd.symlink_metadata().unwrap().file_type().is_symlink());

        // Lock
        let resp = handle_control_command(&mut reg, "LOCK alice");
        assert!(resp.contains("Locked"), "{}", resp);

        // Unlock
        let resp = handle_control_command(&mut reg, "UNLOCK alice");
        assert!(resp.contains("Unlocked"), "{}", resp);

        // Update
        let resp = handle_control_command(&mut reg, "UPDATE alice shell=/bin/zsh");
        assert!(resp.contains("Updated"), "{}", resp);
        assert_eq!(reg.get("alice").unwrap().shell, "/bin/zsh");

        // Passwd
        let resp = handle_control_command(&mut reg, "PASSWD alice newpass");
        assert!(resp.contains("Password changed"), "{}", resp);
        assert!(verify_password(
            "newpass",
            &reg.get("alice").unwrap().hashed_passwords[0]
        ));

        // Resize
        let resp = handle_control_command(&mut reg, "RESIZE alice 10G");
        assert!(resp.contains("Updated disk size"), "{}", resp);

        // Deactivate
        let resp = handle_control_command(&mut reg, "DEACTIVATE alice");
        assert!(resp.contains("Deactivated"), "{}", resp);

        // Remove
        let resp = handle_control_command(&mut reg, "REMOVE alice");
        assert!(resp.contains("Removed"), "{}", resp);
        assert!(!img.exists());
        assert!(!reg.contains("alice"));
    }

    #[test]
    fn test_multi_user_lifecycle() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);

        // Create multiple users
        for name in &["alice", "bob", "charlie"] {
            let img = tmp.path().join(format!("{}.homedir", name));
            let hd = tmp.path().join(format!("home_{}", name));
            let resp = handle_control_command(
                &mut reg,
                &format!(
                    "CREATE {} home={} image={}",
                    name,
                    hd.display(),
                    img.display()
                ),
            );
            assert!(resp.contains("Created"), "create {}: {}", name, resp);
        }
        assert_eq!(reg.len(), 3);

        // Activate all
        for name in &["alice", "bob", "charlie"] {
            handle_control_command(&mut reg, &format!("ACTIVATE {}", name));
        }

        // Lock all
        let resp = handle_control_command(&mut reg, "LOCK-ALL");
        assert!(resp.contains("3"), "{}", resp);

        // Deactivate all
        let resp = handle_control_command(&mut reg, "DEACTIVATE-ALL");
        assert!(resp.contains("3"), "{}", resp);

        // Remove all
        for name in &["alice", "bob", "charlie"] {
            handle_control_command(&mut reg, &format!("REMOVE {}", name));
        }
        assert!(reg.is_empty());
    }

    // -----------------------------------------------------------------------
    // dir_disk_usage
    // -----------------------------------------------------------------------

    #[test]
    fn test_dir_disk_usage_empty() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("empty");
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(dir_disk_usage(&dir), 0);
    }

    #[test]
    fn test_dir_disk_usage_with_files() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("hasfiles");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.txt"), "hello").unwrap();
        fs::write(dir.join("b.txt"), "world!").unwrap();
        let sub = dir.join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("c.txt"), "nested").unwrap();

        let usage = dir_disk_usage(&dir);
        // 5 + 6 + 6 = 17 bytes
        assert_eq!(usage, 17);
    }

    #[test]
    fn test_dir_disk_usage_nonexistent() {
        assert_eq!(
            dir_disk_usage(Path::new("/tmp/definitely_does_not_exist_12345")),
            0
        );
    }

    // -----------------------------------------------------------------------
    // Watchdog parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_watchdog_usec_valid() {
        let d = parse_watchdog_usec("6000000").unwrap();
        assert_eq!(d, Duration::from_micros(3000000));
    }

    #[test]
    fn test_parse_watchdog_usec_zero() {
        assert!(parse_watchdog_usec("0").is_none());
    }

    #[test]
    fn test_parse_watchdog_usec_invalid() {
        assert!(parse_watchdog_usec("abc").is_none());
    }

    #[test]
    fn test_parse_watchdog_usec_empty() {
        assert!(parse_watchdog_usec("").is_none());
    }

    // -----------------------------------------------------------------------
    // Timestamp helper
    // -----------------------------------------------------------------------

    #[test]
    fn test_chrono_lite_timestamp_format() {
        let ts = chrono_lite_timestamp();
        // Should be HH:MM:SS format
        assert_eq!(ts.len(), 8);
        assert_eq!(ts.as_bytes()[2], b':');
        assert_eq!(ts.as_bytes()[5], b':');
    }

    // -----------------------------------------------------------------------
    // JSON edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_json_parse_extra_whitespace() {
        let json = r#"  {
            "userName" : "ws_test" ,
            "uid" : 60099
        }  "#;
        let rec = UserRecord::from_json(json).unwrap();
        assert_eq!(rec.user_name, "ws_test");
        assert_eq!(rec.uid, 60099);
    }

    #[test]
    fn test_json_parse_unicode_escape() {
        let mut rec = UserRecord::new("unicode", 60050);
        rec.real_name = "Ünïcödé".to_string();
        let json = rec.to_json();
        let rec2 = UserRecord::from_json(&json).unwrap();
        assert_eq!(rec2.real_name, "Ünïcödé");
    }

    #[test]
    fn test_json_parse_backslash_in_path() {
        let mut rec = UserRecord::new("pathtest", 60051);
        rec.shell = "/bin/ba\\sh".to_string();
        let json = rec.to_json();
        let rec2 = UserRecord::from_json(&json).unwrap();
        assert_eq!(rec2.shell, "/bin/ba\\sh");
    }

    // -----------------------------------------------------------------------
    // UID allocation exhaustion
    // -----------------------------------------------------------------------

    #[test]
    fn test_uid_exhaustion() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        // Set next_uid to near the max
        reg.next_uid = UID_MAX;
        let img = tmp.path().join("last.homedir");
        let result = create_simple(&mut reg, "last", None, Some(img.to_str().unwrap()));
        assert!(result.is_ok());

        let img2 = tmp.path().join("overflow.homedir");
        let result = create_simple(&mut reg, "overflow", None, Some(img2.to_str().unwrap()));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("UID range exhausted"));
    }

    // -----------------------------------------------------------------------
    // Image path already exists on create
    // -----------------------------------------------------------------------

    #[test]
    fn test_create_image_already_exists() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("existing.homedir");
        fs::create_dir_all(&img).unwrap();

        let result = create_simple(&mut reg, "alice", None, Some(img.to_str().unwrap()));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
    }

    // -----------------------------------------------------------------------
    // Subvolume storage (stub)
    // -----------------------------------------------------------------------

    #[test]
    fn test_create_subvolume_storage() {
        let tmp = TempDir::new().unwrap();
        let mut reg = make_registry(&tmp);
        let img = tmp.path().join("sub.homedir");

        let result = reg.create(CreateParams {
            user_name: "subuser",
            real_name: None,
            shell: None,
            storage: Storage::Subvolume,
            password: None,
            home_dir_override: None,
            image_path_override: Some(img.to_str().unwrap()),
        });
        assert!(result.is_ok(), "create subvolume: {:?}", result);
        assert!(img.exists());
        assert_eq!(reg.get("subuser").unwrap().storage, Storage::Subvolume);
    }
}
