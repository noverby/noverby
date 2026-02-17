//! systemd-analyze — Analyze and debug the system manager.
//!
//! A drop-in replacement for `systemd-analyze(1)`. Provides various
//! subcommands for inspecting boot performance, validating unit files,
//! parsing calendar/time expressions, and querying the service manager.

use clap::{Parser, Subcommand};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── CLI ───────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "systemd-analyze",
    about = "Analyze and debug the system manager",
    version,
    disable_help_subcommand = true
)]
struct Cli {
    /// Operate on the system instance (default)
    #[arg(long, global = true)]
    system: bool,

    /// Operate on the user instance
    #[arg(long, global = true)]
    user: bool,

    /// Do not pipe output into a pager
    #[arg(long, global = true)]
    no_pager: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Show time spent in the kernel, initrd, and userspace during boot
    Time,

    /// List running units ordered by time to initialize
    Blame {
        /// Do not pipe output into a pager
        #[arg(long)]
        no_pager: bool,
    },

    /// Print a tree of the time-critical chain of units
    #[command(name = "critical-chain")]
    CriticalChain {
        /// Unit(s) to show the chain for (default: default.target)
        units: Vec<String>,

        /// Fuzz timespan for chain analysis
        #[arg(long)]
        fuzz: Option<String>,

        /// Do not pipe output into a pager
        #[arg(long)]
        no_pager: bool,
    },

    /// Generate a dependency graph in dot(1) format
    Dot {
        /// Pattern(s) to filter unit names (glob)
        patterns: Vec<String>,

        /// Show 'Requires' dependencies
        #[arg(long)]
        require: bool,

        /// Show ordering dependencies (After/Before)
        #[arg(long)]
        order: bool,

        /// From pattern
        #[arg(long)]
        from_pattern: Option<String>,

        /// To pattern
        #[arg(long)]
        to_pattern: Option<String>,
    },

    /// Normalize a calendar time specification and calculate next elapse
    Calendar {
        /// Calendar expression(s) to parse
        expressions: Vec<String>,

        /// Number of iterations to show
        #[arg(short = 'i', long, default_value_t = 1)]
        iterations: u32,

        /// Base time for relative calculations
        #[arg(long)]
        base_time: Option<String>,
    },

    /// Normalize a time span specification
    Timespan {
        /// Time span expression(s) to parse
        expressions: Vec<String>,
    },

    /// Normalize a timestamp specification
    Timestamp {
        /// Timestamp expression(s) to parse
        expressions: Vec<String>,
    },

    /// Verify unit file(s) for correctness
    Verify {
        /// Unit file(s) to verify
        files: Vec<String>,

        /// Also check man page references
        #[arg(long)]
        man: bool,

        /// Also check generator output
        #[arg(long)]
        generators: Option<String>,
    },

    /// Evaluate Condition*= and Assert*= expressions
    Condition {
        /// Condition expression(s) to evaluate
        expressions: Vec<String>,

        /// Unit to evaluate conditions for
        #[arg(long)]
        unit: Option<String>,
    },

    /// List unit file search paths
    #[command(name = "unit-paths")]
    UnitPaths,

    /// Get/set the log level of the manager
    #[command(name = "log-level")]
    LogLevel {
        /// New log level to set (omit to query current level)
        level: Option<String>,
    },

    /// Get/set the log target of the manager
    #[command(name = "log-target")]
    LogTarget {
        /// New log target to set (omit to query current target)
        target: Option<String>,
    },

    /// Get/set service watchdog state
    #[command(name = "service-watchdogs")]
    ServiceWatchdogs {
        /// New state (yes/no) (omit to query current state)
        state: Option<String>,
    },

    /// Show service security assessment
    Security {
        /// Unit(s) to assess (default: all loaded services)
        units: Vec<String>,

        /// Do not pipe output into a pager
        #[arg(long)]
        no_pager: bool,
    },
}

// ── Boot timing data structures ───────────────────────────────────────────

/// Timing info for a single unit, read from /run/systemd-rs/timing/
#[derive(Debug, Clone)]
struct UnitTiming {
    name: String,
    /// Microseconds from manager start to unit activation start
    activating_us: u64,
    /// Microseconds from manager start to unit becoming active
    active_us: u64,
}

impl UnitTiming {
    fn duration_us(&self) -> u64 {
        self.active_us.saturating_sub(self.activating_us)
    }
}

/// Overall boot timing phases
#[derive(Debug, Clone, Default)]
struct BootTiming {
    firmware_us: Option<u64>,
    loader_us: Option<u64>,
    kernel_us: Option<u64>,
    initrd_us: Option<u64>,
    userspace_us: Option<u64>,
    total_us: Option<u64>,
}

// ── Time span constants ───────────────────────────────────────────────────

const USEC_PER_SEC: u64 = 1_000_000;
const USEC_PER_MSEC: u64 = 1_000;
const USEC_PER_MINUTE: u64 = 60 * USEC_PER_SEC;
const USEC_PER_HOUR: u64 = 60 * USEC_PER_MINUTE;
const USEC_PER_DAY: u64 = 24 * USEC_PER_HOUR;
const USEC_PER_WEEK: u64 = 7 * USEC_PER_DAY;
const USEC_PER_MONTH: u64 = 2_629_800 * USEC_PER_SEC; // 30.44 days
const USEC_PER_YEAR: u64 = 31_557_600 * USEC_PER_SEC; // 365.25 days

// ── Time span parsing ─────────────────────────────────────────────────────

/// A parsed time span value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TimeSpan {
    usec: u64,
}

impl TimeSpan {
    fn parse(input: &str) -> Result<Self, String> {
        let input = input.trim();

        if input == "infinity" {
            return Ok(TimeSpan { usec: u64::MAX });
        }

        // Try parsing as a plain number (treated as microseconds)
        if let Ok(v) = input.parse::<u64>() {
            return Ok(TimeSpan { usec: v });
        }

        let mut total: u64 = 0;
        let mut remaining = input;
        let mut found_any = false;

        while !remaining.is_empty() {
            remaining = remaining.trim_start();
            if remaining.is_empty() {
                break;
            }

            // Parse the numeric part
            let num_end = remaining
                .find(|c: char| !c.is_ascii_digit() && c != '.')
                .unwrap_or(remaining.len());

            if num_end == 0 {
                return Err(format!("Failed to parse time span: {input}"));
            }

            let num_str = &remaining[..num_end];
            let value: f64 = num_str
                .parse()
                .map_err(|_| format!("Invalid number in time span: {num_str}"))?;

            remaining = remaining[num_end..].trim_start();

            // Parse the unit suffix
            let (multiplier, unit_len) = parse_time_unit(remaining)?;

            total = total.saturating_add((value * multiplier as f64) as u64);
            remaining = &remaining[unit_len..];
            found_any = true;
        }

        if !found_any {
            return Err(format!("Failed to parse time span: {input}"));
        }

        Ok(TimeSpan { usec: total })
    }
}

impl fmt::Display for TimeSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.usec == u64::MAX {
            return write!(f, "infinity");
        }
        if self.usec == 0 {
            return write!(f, "0");
        }

        let mut remaining = self.usec;
        let mut parts = Vec::new();

        let units: &[(u64, &str)] = &[
            (USEC_PER_YEAR, "y"),
            (USEC_PER_MONTH, "month"),
            (USEC_PER_WEEK, "w"),
            (USEC_PER_DAY, "d"),
            (USEC_PER_HOUR, "h"),
            (USEC_PER_MINUTE, "min"),
            (USEC_PER_SEC, "s"),
            (USEC_PER_MSEC, "ms"),
            (1, "us"),
        ];

        for &(divisor, suffix) in units {
            if remaining >= divisor {
                let count = remaining / divisor;
                remaining %= divisor;
                parts.push(format!("{count}{suffix}"));
            }
        }

        write!(f, "{}", parts.join(" "))
    }
}

/// Format microseconds into a human-readable duration string.
fn format_usec(usec: u64) -> String {
    if usec == 0 {
        return "0".to_string();
    }

    if usec < USEC_PER_MSEC {
        return format!("{}us", usec);
    }
    if usec < USEC_PER_SEC {
        let ms = usec as f64 / USEC_PER_MSEC as f64;
        return format!("{ms:.0}ms");
    }
    if usec < USEC_PER_MINUTE {
        let s = usec as f64 / USEC_PER_SEC as f64;
        return format!("{s:.3}s");
    }

    let span = TimeSpan { usec };
    format!("{span}")
}

/// Parse a time unit suffix and return (multiplier_in_usec, chars_consumed).
fn parse_time_unit(s: &str) -> Result<(u64, usize), String> {
    let units: &[(&str, u64)] = &[
        ("microseconds", 1),
        ("microsecond", 1),
        ("milliseconds", USEC_PER_MSEC),
        ("millisecond", USEC_PER_MSEC),
        ("minutes", USEC_PER_MINUTE),
        ("minute", USEC_PER_MINUTE),
        ("months", USEC_PER_MONTH),
        ("month", USEC_PER_MONTH),
        ("seconds", USEC_PER_SEC),
        ("second", USEC_PER_SEC),
        ("hours", USEC_PER_HOUR),
        ("hour", USEC_PER_HOUR),
        ("weeks", USEC_PER_WEEK),
        ("week", USEC_PER_WEEK),
        ("days", USEC_PER_DAY),
        ("day", USEC_PER_DAY),
        ("years", USEC_PER_YEAR),
        ("year", USEC_PER_YEAR),
        ("msec", USEC_PER_MSEC),
        ("usec", 1),
        ("min", USEC_PER_MINUTE),
        ("ms", USEC_PER_MSEC),
        ("us", 1),
        ("sec", USEC_PER_SEC),
        ("s", USEC_PER_SEC),
        ("m", USEC_PER_MINUTE),
        ("h", USEC_PER_HOUR),
        ("d", USEC_PER_DAY),
        ("w", USEC_PER_WEEK),
        ("y", USEC_PER_YEAR),
    ];

    let lower = s.to_lowercase();
    for &(suffix, mult) in units {
        if lower.starts_with(suffix) {
            let after = &s[suffix.len()..];
            if after.is_empty()
                || after.starts_with(' ')
                || after.starts_with(|c: char| c.is_ascii_digit())
            {
                return Ok((mult, suffix.len()));
            }
        }
    }

    // If no unit, assume microseconds
    if s.is_empty() || s.starts_with(|c: char| c.is_ascii_digit()) {
        return Ok((1, 0));
    }

    Err(format!("Unknown time unit in: {s}"))
}

// ── Calendar expression parsing ───────────────────────────────────────────

#[derive(Debug, Clone)]
struct CalendarSpec {
    original: String,
    normalized: String,
}

impl CalendarSpec {
    fn parse(input: &str) -> Result<Self, String> {
        let input = input.trim();

        // Handle well-known shorthands
        let normalized = match input.to_lowercase().as_str() {
            "minutely" => "*-*-* *:*:00".to_string(),
            "hourly" => "*-*-* *:00:00".to_string(),
            "daily" => "*-*-* 00:00:00".to_string(),
            "monthly" => "*-*-01 00:00:00".to_string(),
            "weekly" => "Mon *-*-* 00:00:00".to_string(),
            "yearly" | "annually" => "*-01-01 00:00:00".to_string(),
            "quarterly" => "*-01,04,07,10-01 00:00:00".to_string(),
            "semiannually" | "semi-annually" => "*-01,07-01 00:00:00".to_string(),
            _ => normalize_calendar_expression(input)?,
        };

        Ok(CalendarSpec {
            original: input.to_string(),
            normalized,
        })
    }
}

fn normalize_calendar_expression(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Empty calendar expression".to_string());
    }

    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty calendar expression".to_string());
    }

    let has_dow = is_day_of_week(parts[0]);

    let (dow_part, rest) = if has_dow && parts.len() > 1 {
        (Some(parts[0]), &parts[1..])
    } else if has_dow {
        (Some(parts[0]), [].as_slice())
    } else {
        (None, &parts[..])
    };

    let mut date_part = None;
    let mut time_part = None;

    for &p in rest {
        if p.contains('-') || (p.contains('*') && !p.contains(':')) {
            date_part = Some(p);
        } else if p.contains(':') {
            time_part = Some(p);
        } else {
            date_part = Some(p);
        }
    }

    let mut result = String::new();

    if let Some(dow) = dow_part {
        result.push_str(dow);
        result.push(' ');
    }

    if let Some(date) = date_part {
        result.push_str(date);
    } else {
        result.push_str("*-*-*");
    }

    result.push(' ');

    if let Some(time) = time_part {
        let colon_count = time.chars().filter(|&c| c == ':').count();
        result.push_str(time);
        if colon_count == 1 {
            result.push_str(":00");
        }
    } else {
        result.push_str("00:00:00");
    }

    Ok(result)
}

fn is_day_of_week(s: &str) -> bool {
    let lower = s.to_lowercase();
    let lower = lower.trim_end_matches(',');
    matches!(
        lower,
        "mon"
            | "tue"
            | "wed"
            | "thu"
            | "fri"
            | "sat"
            | "sun"
            | "monday"
            | "tuesday"
            | "wednesday"
            | "thursday"
            | "friday"
            | "saturday"
            | "sunday"
    )
}

// ── Timestamp parsing ─────────────────────────────────────────────────────

fn parse_timestamp(input: &str) -> Result<SystemTime, String> {
    let input = input.trim();

    match input.to_lowercase().as_str() {
        "now" => return Ok(SystemTime::now()),
        "today" => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?;
            let secs = now.as_secs();
            let day_start = secs - (secs % 86400);
            return Ok(UNIX_EPOCH + Duration::from_secs(day_start));
        }
        "yesterday" => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?;
            let secs = now.as_secs();
            let day_start = secs - (secs % 86400) - 86400;
            return Ok(UNIX_EPOCH + Duration::from_secs(day_start));
        }
        "tomorrow" => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?;
            let secs = now.as_secs();
            let day_start = secs - (secs % 86400) + 86400;
            return Ok(UNIX_EPOCH + Duration::from_secs(day_start));
        }
        "epoch" => return Ok(UNIX_EPOCH),
        _ => {}
    }

    // Relative: "+5min", "-2h"
    if let Some(rest) = input.strip_prefix('+') {
        let span = TimeSpan::parse(rest)?;
        return Ok(SystemTime::now() + Duration::from_micros(span.usec));
    }
    if let Some(rest) = input.strip_prefix('-') {
        let span = TimeSpan::parse(rest)?;
        return SystemTime::now()
            .checked_sub(Duration::from_micros(span.usec))
            .ok_or_else(|| "Timestamp would be before UNIX epoch".to_string());
    }

    // "N ago"
    if let Some(rest) = input.strip_suffix("ago") {
        let span = TimeSpan::parse(rest.trim())?;
        return SystemTime::now()
            .checked_sub(Duration::from_micros(span.usec))
            .ok_or_else(|| "Timestamp would be before UNIX epoch".to_string());
    }

    // "@EPOCH_SECONDS"
    if let Some(rest) = input.strip_prefix('@') {
        let secs: u64 = rest
            .trim()
            .parse()
            .map_err(|_| format!("Invalid epoch timestamp: {rest}"))?;
        return Ok(UNIX_EPOCH + Duration::from_secs(secs));
    }

    // YYYY-MM-DD HH:MM:SS
    if let Some(ts) = try_parse_datetime(input) {
        return Ok(ts);
    }

    Err(format!("Failed to parse timestamp: {input}"))
}

fn try_parse_datetime(input: &str) -> Option<SystemTime> {
    let parts: Vec<&str> = input.splitn(2, |c| c == ' ' || c == 'T').collect();

    let date_str = parts.first()?;
    let time_str = parts.get(1).copied().unwrap_or("00:00:00");

    let date_parts: Vec<&str> = date_str.split('-').collect();
    if date_parts.len() != 3 {
        return None;
    }

    let year: i64 = date_parts[0].parse().ok()?;
    let month: u32 = date_parts[1].parse().ok()?;
    let day: u32 = date_parts[2].parse().ok()?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let time_parts: Vec<&str> = time_str.split(':').collect();
    let hour: u32 = time_parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minute: u32 = time_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let second: u32 = time_parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    let days = days_from_civil(year, month, day);
    let secs = days as u64 * 86400 + hour as u64 * 3600 + minute as u64 * 60 + second as u64;

    Some(UNIX_EPOCH + Duration::from_secs(secs))
}

/// Convert a civil date to days since UNIX epoch (Howard Hinnant algorithm).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let m = if month <= 2 {
        month as i64 + 9
    } else {
        month as i64 - 3
    };
    let era = if y >= 0 { y / 400 } else { (y - 399) / 400 };
    let yoe = (y - era * 400) as u64;
    let doy = (153 * m as u64 + 2) / 5 + day as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i64 - 719468
}

/// Convert days since UNIX epoch to civil date.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 {
        z / 146097
    } else {
        (z - 146096) / 146097
    };
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

/// Day of week from days since epoch (0=Thu, 1=Fri, ..., 6=Wed).
fn weekday_from_days(days: i64) -> u32 {
    ((days % 7 + 7) % 7) as u32
}

/// Format a SystemTime as a human-readable timestamp.
fn format_timestamp(t: SystemTime) -> String {
    let dur = t.duration_since(UNIX_EPOCH).unwrap_or_default();
    let total_secs = dur.as_secs();
    let days = (total_secs / 86400) as i64;
    let tod = total_secs % 86400;
    let hour = tod / 3600;
    let minute = (tod % 3600) / 60;
    let second = tod % 60;
    let (year, month, day) = civil_from_days(days);
    let wd = weekday_from_days(days);
    let dow = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    let dow_name = dow[wd as usize % 7];
    format!(
        "{} {}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        dow_name, year, month, day, hour, minute, second
    )
}

// ── Boot timing data ──────────────────────────────────────────────────────

/// Read boot timing from /run/systemd-rs/boot-timing if available.
fn read_boot_timing() -> BootTiming {
    let mut bt = BootTiming::default();
    let path = Path::new("/run/systemd-rs/boot-timing");
    if let Ok(content) = fs::read_to_string(path) {
        for line in content.lines() {
            let line = line.trim();
            if let Some((key, val)) = line.split_once('=') {
                let val: u64 = match val.trim().parse() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                match key.trim() {
                    "firmware" => bt.firmware_us = Some(val),
                    "loader" => bt.loader_us = Some(val),
                    "kernel" => bt.kernel_us = Some(val),
                    "initrd" => bt.initrd_us = Some(val),
                    "userspace" => bt.userspace_us = Some(val),
                    "total" => bt.total_us = Some(val),
                    _ => {}
                }
            }
        }
    }
    // Try /proc/stat for kernel boot time as fallback
    if bt.kernel_us.is_none() {
        if let Ok(content) = fs::read_to_string("/proc/stat") {
            for line in content.lines() {
                if let Some(rest) = line.strip_prefix("btime ") {
                    if let Ok(btime) = rest.trim().parse::<u64>() {
                        if let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) {
                            let uptime_s = now.as_secs().saturating_sub(btime);
                            bt.kernel_us = Some(uptime_s * USEC_PER_SEC);
                        }
                    }
                }
            }
        }
    }
    bt
}

/// Read per-unit timing data from /run/systemd-rs/timing/.
fn read_unit_timings() -> Vec<UnitTiming> {
    let dir = Path::new("/run/systemd-rs/timing");
    let mut timings = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Ok(content) = fs::read_to_string(entry.path()) {
                let mut activating = None;
                let mut active = None;
                for line in content.lines() {
                    if let Some((k, v)) = line.split_once('=') {
                        let v: u64 = match v.trim().parse() {
                            Ok(x) => x,
                            Err(_) => continue,
                        };
                        match k.trim() {
                            "activating" => activating = Some(v),
                            "active" => active = Some(v),
                            _ => {}
                        }
                    }
                }
                if let (Some(a), Some(b)) = (activating, active) {
                    timings.push(UnitTiming {
                        name,
                        activating_us: a,
                        active_us: b,
                    });
                }
            }
        }
    }
    timings
}

// ── Condition evaluation ──────────────────────────────────────────────────

fn evaluate_condition(expr: &str) -> (bool, &'static str) {
    let (negated, key, value) = if let Some((k, v)) = expr.split_once('=') {
        let (neg, k) = if let Some(stripped) = k.strip_suffix('!') {
            (true, stripped)
        } else {
            (false, k)
        };
        (neg, k.trim(), v.trim())
    } else {
        return (false, "Invalid condition expression (expected KEY=VALUE)");
    };

    let result = match key {
        "ConditionPathExists" | "AssertPathExists" => {
            let (negate_path, path) = if let Some(p) = value.strip_prefix('!') {
                (true, p)
            } else {
                (false, value)
            };
            let exists = Path::new(path).exists();
            if negate_path { !exists } else { exists }
        }
        "ConditionPathIsDirectory" | "AssertPathIsDirectory" => {
            let (np, path) = if let Some(p) = value.strip_prefix('!') {
                (true, p)
            } else {
                (false, value)
            };
            let is_dir = Path::new(path).is_dir();
            if np { !is_dir } else { is_dir }
        }
        "ConditionPathIsSymbolicLink" | "AssertPathIsSymbolicLink" => {
            let (np, path) = if let Some(p) = value.strip_prefix('!') {
                (true, p)
            } else {
                (false, value)
            };
            let is_link = Path::new(path).is_symlink();
            if np { !is_link } else { is_link }
        }
        "ConditionFileIsExecutable" | "AssertFileIsExecutable" => {
            use std::os::unix::fs::PermissionsExt;
            let (np, path) = if let Some(p) = value.strip_prefix('!') {
                (true, p)
            } else {
                (false, value)
            };
            let is_exec = fs::metadata(path)
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false);
            if np { !is_exec } else { is_exec }
        }
        "ConditionFileNotEmpty" | "AssertFileNotEmpty" => {
            let (np, path) = if let Some(p) = value.strip_prefix('!') {
                (true, p)
            } else {
                (false, value)
            };
            let not_empty = fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false);
            if np { !not_empty } else { not_empty }
        }
        "ConditionDirectoryNotEmpty" | "AssertDirectoryNotEmpty" => {
            let (np, path) = if let Some(p) = value.strip_prefix('!') {
                (true, p)
            } else {
                (false, value)
            };
            let not_empty = fs::read_dir(path)
                .map(|mut d| d.next().is_some())
                .unwrap_or(false);
            if np { !not_empty } else { not_empty }
        }
        "ConditionVirtualization" | "AssertVirtualization" => {
            let detected = detect_virtualization();
            if value.is_empty() || value == "yes" {
                detected.is_some()
            } else if value == "no" {
                detected.is_none()
            } else {
                detected.as_deref() == Some(value)
            }
        }
        "ConditionHost" | "AssertHost" => {
            let hostname = hostname();
            hostname.as_deref() == Some(value)
        }
        "ConditionKernelCommandLine" | "AssertKernelCommandLine" => {
            let cmdline = fs::read_to_string("/proc/cmdline").unwrap_or_default();
            cmdline
                .split_whitespace()
                .any(|arg| arg == value || arg.starts_with(&format!("{value}=")))
        }
        _ => {
            return (false, "Unknown condition type");
        }
    };

    let final_result = if negated { !result } else { result };
    if final_result {
        (true, "met")
    } else {
        (false, "not met")
    }
}

fn detect_virtualization() -> Option<String> {
    // Check /proc/cpuinfo for hypervisor flag
    if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
        if cpuinfo.contains("hypervisor") {
            // Try to identify the specific hypervisor
            if let Ok(dmi) = fs::read_to_string("/sys/class/dmi/id/product_name") {
                let name = dmi.trim().to_lowercase();
                if name.contains("kvm") || name.contains("qemu") {
                    return Some("kvm".to_string());
                }
                if name.contains("virtualbox") {
                    return Some("oracle".to_string());
                }
                if name.contains("vmware") {
                    return Some("vmware".to_string());
                }
                if name.contains("cloud hypervisor") {
                    return Some("kvm".to_string());
                }
                return Some("vm".to_string());
            }
            return Some("vm".to_string());
        }
    }
    // Check for container
    if Path::new("/run/.containerenv").exists() {
        return Some("podman".to_string());
    }
    if Path::new("/.dockerenv").exists() {
        return Some("docker".to_string());
    }
    if let Ok(content) = fs::read_to_string("/proc/1/environ") {
        if content.contains("container=") {
            return Some("container-other".to_string());
        }
    }
    None
}

fn hostname() -> Option<String> {
    fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// ── Unit paths ────────────────────────────────────────────────────────────

fn system_unit_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/etc/systemd/system.control"),
        PathBuf::from("/run/systemd/system.control"),
        PathBuf::from("/run/systemd/transient"),
        PathBuf::from("/run/systemd/generator.early"),
        PathBuf::from("/etc/systemd/system"),
        PathBuf::from("/etc/systemd/system.attached"),
        PathBuf::from("/run/systemd/system"),
        PathBuf::from("/run/systemd/system.attached"),
        PathBuf::from("/run/systemd/generator"),
        PathBuf::from("/usr/local/lib/systemd/system"),
        PathBuf::from("/usr/lib/systemd/system"),
        PathBuf::from("/run/systemd/generator.late"),
    ]
}

fn user_unit_paths() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let xdg_config = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
    let uid = fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u32>().ok())
        })
        .unwrap_or(0);
    let xdg_runtime =
        std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| format!("/run/user/{}", uid));
    let xdg_data =
        std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home}/.local/share"));

    vec![
        PathBuf::from(format!("{xdg_config}/systemd/user.control")),
        PathBuf::from(format!("{xdg_runtime}/systemd/user.control")),
        PathBuf::from(format!("{xdg_runtime}/systemd/transient")),
        PathBuf::from(format!("{xdg_runtime}/systemd/generator.early")),
        PathBuf::from(format!("{xdg_config}/systemd/user")),
        PathBuf::from(format!("{xdg_config}/systemd/user.attached")),
        PathBuf::from("/etc/systemd/user"),
        PathBuf::from(format!("{xdg_runtime}/systemd/user")),
        PathBuf::from("/run/systemd/user"),
        PathBuf::from(format!("{xdg_runtime}/systemd/generator")),
        PathBuf::from(format!("{xdg_data}/systemd/user")),
        PathBuf::from("/usr/local/lib/systemd/user"),
        PathBuf::from("/usr/lib/systemd/user"),
        PathBuf::from(format!("{xdg_runtime}/systemd/generator.late")),
    ]
}

// ── Unit file verification ────────────────────────────────────────────────

fn verify_unit_file(path: &str) -> Vec<String> {
    let mut issues = Vec::new();
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            issues.push(format!("{path}: Cannot open: {e}"));
            return issues;
        }
    };

    let mut has_unit_section = false;
    let mut has_service_section = false;
    let mut has_exec_start = false;
    let mut has_type = false;
    let mut current_section = String::new();

    for (lineno, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].to_string();
            match current_section.as_str() {
                "Unit" => has_unit_section = true,
                "Service" => has_service_section = true,
                "Install" | "Socket" | "Timer" | "Mount" | "Automount" | "Swap" | "Path"
                | "Slice" | "Scope" => {}
                other => {
                    issues.push(format!(
                        "{path}:{}: Unknown section [{}]",
                        lineno + 1,
                        other
                    ));
                }
            }
            continue;
        }

        if !line.contains('=') {
            issues.push(format!(
                "{path}:{}: Line is not a key=value pair: {}",
                lineno + 1,
                line
            ));
            continue;
        }

        if let Some((key, _val)) = line.split_once('=') {
            let key = key.trim();
            match current_section.as_str() {
                "Service" => {
                    if key == "ExecStart" {
                        has_exec_start = true;
                    }
                    if key == "Type" {
                        has_type = true;
                    }
                }
                _ => {}
            }
        }
    }

    if path.ends_with(".service") {
        if !has_unit_section && !has_service_section {
            issues.push(format!("{path}: Missing [Unit] and [Service] sections"));
        }
        if has_service_section && !has_exec_start && !has_type {
            issues.push(format!(
                "{path}: Service has no ExecStart= and no Type= setting"
            ));
        }
    }

    if issues.is_empty() {
        issues.push(format!("{path}: OK"));
    }

    issues
}

// ── Main ──────────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    match cli.command {
        None | Some(Command::Time) => cmd_time(),
        Some(Command::Blame { .. }) => cmd_blame(),
        Some(Command::CriticalChain { ref units, .. }) => cmd_critical_chain(units),
        Some(Command::Dot {
            ref patterns,
            require,
            order,
            ref from_pattern,
            ref to_pattern,
        }) => cmd_dot(patterns, require, order, from_pattern, to_pattern),
        Some(Command::Calendar {
            ref expressions,
            iterations,
            ..
        }) => cmd_calendar(expressions, iterations),
        Some(Command::Timespan { ref expressions }) => cmd_timespan(expressions),
        Some(Command::Timestamp { ref expressions }) => cmd_timestamp(expressions),
        Some(Command::Verify { ref files, .. }) => cmd_verify(files),
        Some(Command::Condition {
            ref expressions, ..
        }) => cmd_condition(expressions),
        Some(Command::UnitPaths) => cmd_unit_paths(cli.user),
        Some(Command::LogLevel { ref level }) => cmd_log_level(level),
        Some(Command::LogTarget { ref target }) => cmd_log_target(target),
        Some(Command::ServiceWatchdogs { ref state }) => cmd_service_watchdogs(state),
        Some(Command::Security { ref units, .. }) => cmd_security(units),
    }
}

fn cmd_time() {
    let bt = read_boot_timing();

    println!("Startup finished in:");
    if let Some(k) = bt.kernel_us {
        println!("  kernel: {}", format_usec(k));
    }
    if let Some(i) = bt.initrd_us {
        println!("  initrd: {}", format_usec(i));
    }
    if let Some(u) = bt.userspace_us {
        println!("  userspace: {}", format_usec(u));
    }

    // If we have nothing from our timing file, try /proc/uptime
    if bt.kernel_us.is_none() && bt.initrd_us.is_none() && bt.userspace_us.is_none() {
        if let Ok(content) = fs::read_to_string("/proc/uptime") {
            if let Some(uptime_str) = content.split_whitespace().next() {
                if let Ok(uptime_s) = uptime_str.parse::<f64>() {
                    let uptime_us = (uptime_s * USEC_PER_SEC as f64) as u64;
                    println!(
                        "  (system running for {}; no detailed timing available)",
                        format_usec(uptime_us)
                    );
                }
            }
        }
    }

    if let Some(t) = bt.total_us {
        println!("  total: {}", format_usec(t));
    }
}

fn cmd_blame() {
    let mut timings = read_unit_timings();

    if timings.is_empty() {
        // No timing data; show a message
        println!("No unit timing data available.");
        println!("(Boot timing data is written to /run/systemd-rs/timing/)");
        return;
    }

    timings.sort_by(|a, b| b.duration_us().cmp(&a.duration_us()));

    for t in &timings {
        let dur = t.duration_us();
        println!("{:>10} {}", format_usec(dur), t.name);
    }
}

fn cmd_critical_chain(units: &[String]) {
    let target = if units.is_empty() {
        "default.target"
    } else {
        &units[0]
    };

    let timings = read_unit_timings();
    if timings.is_empty() {
        println!("The time when unit became active or started is not known.");
        println!("(No timing data in /run/systemd-rs/timing/)");
        return;
    }

    println!("The time when unit became active or started is printed after the \"@\" character.");
    println!("The time the unit took to start is printed after the \"+\" character.");
    println!();
    println!("{target}");

    // Find the target and its dependencies (simplified: just show sorted by activation time)
    let mut sorted = timings.clone();
    sorted.sort_by_key(|t| t.active_us);

    for t in sorted.iter().rev().take(10) {
        println!(
            "  {} @{} +{}",
            t.name,
            format_usec(t.activating_us),
            format_usec(t.duration_us())
        );
    }
}

fn cmd_dot(
    _patterns: &[String],
    _require: bool,
    _order: bool,
    _from_pattern: &Option<String>,
    _to_pattern: &Option<String>,
) {
    // Read unit files from standard paths and generate dot output
    println!("digraph systemd {{");
    println!("  rankdir=LR;");
    println!("  node [shape=box];");

    let search_paths = system_unit_paths();
    for dir in &search_paths {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let unit_name = entry.file_name().to_string_lossy().to_string();
                if let Ok(content) = fs::read_to_string(&path) {
                    for line in content.lines() {
                        let line = line.trim();
                        for prefix in &[
                            "Requires=",
                            "Wants=",
                            "After=",
                            "Before=",
                            "BindsTo=",
                            "PartOf=",
                            "Conflicts=",
                        ] {
                            if let Some(deps) = line.strip_prefix(prefix) {
                                let style = match *prefix {
                                    "Requires=" | "BindsTo=" => "bold",
                                    "Wants=" => "dashed",
                                    "After=" | "Before=" => "dotted",
                                    "Conflicts=" => "bold,color=red",
                                    "PartOf=" => "bold,color=blue",
                                    _ => "",
                                };
                                for dep in deps.split_whitespace() {
                                    println!(
                                        "  \"{}\" -> \"{}\" [style={}];",
                                        unit_name, dep, style
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    println!("}}");
}

fn cmd_calendar(expressions: &[String], iterations: u32) {
    if expressions.is_empty() {
        eprintln!("No calendar expression specified.");
        process::exit(1);
    }

    for expr in expressions {
        match CalendarSpec::parse(expr) {
            Ok(spec) => {
                println!("  Original form: {}", spec.original);
                println!("Normalized form: {}", spec.normalized);
                if iterations > 0 {
                    println!("    Next elapse: n/a (full calendar arithmetic not yet implemented)");
                }
                println!();
            }
            Err(e) => {
                eprintln!("Failed to parse calendar expression '{}': {}", expr, e);
                process::exit(1);
            }
        }
    }
}

fn cmd_timespan(expressions: &[String]) {
    if expressions.is_empty() {
        eprintln!("No time span expression specified.");
        process::exit(1);
    }

    for expr in expressions {
        match TimeSpan::parse(expr) {
            Ok(span) => {
                println!("Original: {expr}");
                println!("      {}:", format_usec(span.usec));
                println!("   {} us", span.usec);
                println!("   {span}");
                println!();
            }
            Err(e) => {
                eprintln!("Failed to parse time span '{}': {}", expr, e);
                process::exit(1);
            }
        }
    }
}

fn cmd_timestamp(expressions: &[String]) {
    if expressions.is_empty() {
        eprintln!("No timestamp expression specified.");
        process::exit(1);
    }

    for expr in expressions {
        match parse_timestamp(expr) {
            Ok(ts) => {
                let dur = ts.duration_since(UNIX_EPOCH).unwrap_or_default();
                println!("  Original form: {expr}");
                println!("Normalized form: {}", format_timestamp(ts));
                println!("       (in UTC): {}", format_timestamp(ts));
                println!("   UNIX seconds: @{}", dur.as_secs());
                let from_now = if let Ok(d) = ts.duration_since(SystemTime::now()) {
                    format!("in {}", format_usec(d.as_micros() as u64))
                } else if let Ok(d) = SystemTime::now().duration_since(ts) {
                    format!("{} ago", format_usec(d.as_micros() as u64))
                } else {
                    "now".to_string()
                };
                println!("      From now: {from_now}");
                println!();
            }
            Err(e) => {
                eprintln!("Failed to parse timestamp '{}': {}", expr, e);
                process::exit(1);
            }
        }
    }
}

fn cmd_verify(files: &[String]) {
    if files.is_empty() {
        eprintln!("No unit files specified.");
        process::exit(1);
    }

    let mut any_error = false;
    for file in files {
        let issues = verify_unit_file(file);
        for issue in &issues {
            if !issue.ends_with(": OK") {
                any_error = true;
            }
            println!("{issue}");
        }
    }

    if any_error {
        process::exit(1);
    }
}

fn cmd_condition(expressions: &[String]) {
    if expressions.is_empty() {
        eprintln!("No condition expressions specified.");
        process::exit(1);
    }

    let mut all_met = true;
    for expr in expressions {
        let (met, reason) = evaluate_condition(expr);
        let status = if met { "met" } else { "not met" };
        println!("{expr}: {status} ({reason})");
        if !met {
            all_met = false;
        }
    }

    if !all_met {
        process::exit(1);
    }
}

fn cmd_unit_paths(user_mode: bool) {
    let paths = if user_mode {
        user_unit_paths()
    } else {
        system_unit_paths()
    };

    for p in &paths {
        println!("{}", p.display());
    }
}

fn cmd_log_level(level: &Option<String>) {
    match level {
        Some(l) => {
            // Validate the level
            match l.to_lowercase().as_str() {
                "emerg" | "alert" | "crit" | "err" | "warning" | "notice" | "info" | "debug" => {
                    println!("Would set log level to: {l}");
                    println!("(Not yet implemented: requires communication with PID 1)");
                }
                _ => {
                    eprintln!("Invalid log level: {l}");
                    process::exit(1);
                }
            }
        }
        None => {
            // Query current level — read from /run/systemd-rs/log-level if available
            if let Ok(level) = fs::read_to_string("/run/systemd-rs/log-level") {
                println!("{}", level.trim());
            } else {
                println!("info");
            }
        }
    }
}

fn cmd_log_target(target: &Option<String>) {
    match target {
        Some(t) => match t.as_str() {
            "console" | "journal" | "kmsg" | "journal-or-kmsg" | "null" | "auto" => {
                println!("Would set log target to: {t}");
                println!("(Not yet implemented: requires communication with PID 1)");
            }
            _ => {
                eprintln!("Invalid log target: {t}");
                process::exit(1);
            }
        },
        None => {
            if let Ok(target) = fs::read_to_string("/run/systemd-rs/log-target") {
                println!("{}", target.trim());
            } else {
                println!("journal-or-kmsg");
            }
        }
    }
}

fn cmd_service_watchdogs(state: &Option<String>) {
    match state {
        Some(s) => match s.as_str() {
            "yes" | "no" | "1" | "0" | "true" | "false" | "on" | "off" => {
                println!("Would set service-watchdogs to: {s}");
                println!("(Not yet implemented: requires communication with PID 1)");
            }
            _ => {
                eprintln!("Invalid boolean value: {s}");
                process::exit(1);
            }
        },
        None => {
            println!("yes");
        }
    }
}

fn cmd_security(units: &[String]) {
    if units.is_empty() {
        println!("UNIT                                 EXPOSURE PREDICATE HAPPY");
        println!("No units specified and service enumeration not yet implemented.");
        return;
    }

    for unit in units {
        // Basic security assessment — check for obvious concerns
        let path = find_unit_file(unit);
        match path {
            Some(p) => {
                let content = fs::read_to_string(&p).unwrap_or_default();
                let mut score: f64 = 10.0; // Start at max exposure

                // Check for security hardening directives
                let hardening: &[(&str, f64)] = &[
                    ("ProtectSystem=", -1.0),
                    ("ProtectHome=", -1.0),
                    ("PrivateTmp=", -0.5),
                    ("NoNewPrivileges=", -0.5),
                    ("PrivateDevices=", -0.5),
                    ("ProtectKernelTunables=", -0.5),
                    ("ProtectKernelModules=", -0.5),
                    ("ProtectControlGroups=", -0.5),
                    ("RestrictSUIDSGID=", -0.5),
                    ("MemoryDenyWriteExecute=", -0.5),
                    ("RestrictNamespaces=", -0.5),
                    ("RestrictRealtime=", -0.5),
                    ("LockPersonality=", -0.5),
                    ("DynamicUser=", -1.0),
                    ("CapabilityBoundingSet=", -0.5),
                    ("SystemCallFilter=", -1.0),
                ];

                for (directive, adjustment) in hardening {
                    if content.contains(directive) {
                        score += adjustment;
                    }
                }

                // Penalize for running as root
                if content.contains("User=root")
                    || (!content.contains("User=") && !content.contains("DynamicUser="))
                {
                    score += 0.5;
                }

                score = score.clamp(0.0, 10.0);

                let predicate = if score <= 2.0 {
                    "OK"
                } else if score <= 5.0 {
                    "MEDIUM"
                } else if score <= 7.5 {
                    "EXPOSED"
                } else {
                    "UNSAFE"
                };

                let happy = if score <= 2.0 {
                    "😀"
                } else if score <= 5.0 {
                    "😐"
                } else if score <= 7.5 {
                    "🙁"
                } else {
                    "😨"
                };

                println!("{:<40} {:>4.1} {:<10} {}", unit, score, predicate, happy);
            }
            None => {
                println!("{unit}: unit file not found");
            }
        }
    }
}

fn find_unit_file(name: &str) -> Option<PathBuf> {
    for dir in system_unit_paths() {
        let path = dir.join(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // TimeSpan parsing tests

    #[test]
    fn test_timespan_parse_seconds() {
        let ts = TimeSpan::parse("5s").unwrap();
        assert_eq!(ts.usec, 5 * USEC_PER_SEC);
    }

    #[test]
    fn test_timespan_parse_minutes() {
        let ts = TimeSpan::parse("3min").unwrap();
        assert_eq!(ts.usec, 3 * USEC_PER_MINUTE);
    }

    #[test]
    fn test_timespan_parse_hours() {
        let ts = TimeSpan::parse("2h").unwrap();
        assert_eq!(ts.usec, 2 * USEC_PER_HOUR);
    }

    #[test]
    fn test_timespan_parse_days() {
        let ts = TimeSpan::parse("1d").unwrap();
        assert_eq!(ts.usec, USEC_PER_DAY);
    }

    #[test]
    fn test_timespan_parse_weeks() {
        let ts = TimeSpan::parse("2w").unwrap();
        assert_eq!(ts.usec, 2 * USEC_PER_WEEK);
    }

    #[test]
    fn test_timespan_parse_milliseconds() {
        let ts = TimeSpan::parse("500ms").unwrap();
        assert_eq!(ts.usec, 500 * USEC_PER_MSEC);
    }

    #[test]
    fn test_timespan_parse_microseconds() {
        let ts = TimeSpan::parse("100us").unwrap();
        assert_eq!(ts.usec, 100);
    }

    #[test]
    fn test_timespan_parse_compound() {
        let ts = TimeSpan::parse("1h 30min").unwrap();
        assert_eq!(ts.usec, USEC_PER_HOUR + 30 * USEC_PER_MINUTE);
    }

    #[test]
    fn test_timespan_parse_compound_no_space() {
        let ts = TimeSpan::parse("2h30min").unwrap();
        assert_eq!(ts.usec, 2 * USEC_PER_HOUR + 30 * USEC_PER_MINUTE);
    }

    #[test]
    fn test_timespan_parse_infinity() {
        let ts = TimeSpan::parse("infinity").unwrap();
        assert_eq!(ts.usec, u64::MAX);
    }

    #[test]
    fn test_timespan_parse_bare_number() {
        let ts = TimeSpan::parse("5000000").unwrap();
        assert_eq!(ts.usec, 5_000_000);
    }

    #[test]
    fn test_timespan_parse_full_word() {
        let ts = TimeSpan::parse("3 seconds").unwrap();
        assert_eq!(ts.usec, 3 * USEC_PER_SEC);
    }

    #[test]
    fn test_timespan_parse_error() {
        assert!(TimeSpan::parse("foobar").is_err());
    }

    #[test]
    fn test_timespan_display_zero() {
        let ts = TimeSpan { usec: 0 };
        assert_eq!(ts.to_string(), "0");
    }

    #[test]
    fn test_timespan_display_seconds() {
        let ts = TimeSpan {
            usec: 5 * USEC_PER_SEC,
        };
        assert_eq!(ts.to_string(), "5s");
    }

    #[test]
    fn test_timespan_display_compound() {
        let ts = TimeSpan {
            usec: USEC_PER_HOUR + 30 * USEC_PER_MINUTE + 15 * USEC_PER_SEC,
        };
        assert_eq!(ts.to_string(), "1h 30min 15s");
    }

    #[test]
    fn test_timespan_display_infinity() {
        let ts = TimeSpan { usec: u64::MAX };
        assert_eq!(ts.to_string(), "infinity");
    }

    // Calendar parsing tests

    #[test]
    fn test_calendar_parse_daily() {
        let spec = CalendarSpec::parse("daily").unwrap();
        assert_eq!(spec.normalized, "*-*-* 00:00:00");
    }

    #[test]
    fn test_calendar_parse_weekly() {
        let spec = CalendarSpec::parse("weekly").unwrap();
        assert_eq!(spec.normalized, "Mon *-*-* 00:00:00");
    }

    #[test]
    fn test_calendar_parse_monthly() {
        let spec = CalendarSpec::parse("monthly").unwrap();
        assert_eq!(spec.normalized, "*-*-01 00:00:00");
    }

    #[test]
    fn test_calendar_parse_yearly() {
        let spec = CalendarSpec::parse("yearly").unwrap();
        assert_eq!(spec.normalized, "*-01-01 00:00:00");
    }

    #[test]
    fn test_calendar_parse_hourly() {
        let spec = CalendarSpec::parse("hourly").unwrap();
        assert_eq!(spec.normalized, "*-*-* *:00:00");
    }

    #[test]
    fn test_calendar_parse_minutely() {
        let spec = CalendarSpec::parse("minutely").unwrap();
        assert_eq!(spec.normalized, "*-*-* *:*:00");
    }

    #[test]
    fn test_calendar_parse_quarterly() {
        let spec = CalendarSpec::parse("quarterly").unwrap();
        assert_eq!(spec.normalized, "*-01,04,07,10-01 00:00:00");
    }

    #[test]
    fn test_calendar_parse_custom() {
        let spec = CalendarSpec::parse("*-*-* 03:00").unwrap();
        assert_eq!(spec.normalized, "*-*-* 03:00:00");
    }

    #[test]
    fn test_calendar_parse_empty_error() {
        assert!(CalendarSpec::parse("").is_err());
    }

    // Timestamp parsing tests

    #[test]
    fn test_timestamp_now() {
        let ts = parse_timestamp("now").unwrap();
        let dur = ts.duration_since(UNIX_EPOCH).unwrap();
        assert!(dur.as_secs() > 0);
    }

    #[test]
    fn test_timestamp_epoch() {
        let ts = parse_timestamp("epoch").unwrap();
        assert_eq!(ts, UNIX_EPOCH);
    }

    #[test]
    fn test_timestamp_at_epoch() {
        let ts = parse_timestamp("@0").unwrap();
        assert_eq!(ts, UNIX_EPOCH);
    }

    #[test]
    fn test_timestamp_at_value() {
        let ts = parse_timestamp("@1700000000").unwrap();
        let dur = ts.duration_since(UNIX_EPOCH).unwrap();
        assert_eq!(dur.as_secs(), 1_700_000_000);
    }

    #[test]
    fn test_timestamp_datetime() {
        let ts = parse_timestamp("2023-11-14 12:00:00").unwrap();
        let dur = ts.duration_since(UNIX_EPOCH).unwrap();
        // 2023-11-14 12:00:00 UTC = 1699963200
        assert!(dur.as_secs() > 1_699_900_000);
        assert!(dur.as_secs() < 1_700_100_000);
    }

    #[test]
    fn test_timestamp_relative_plus() {
        let before = SystemTime::now();
        let ts = parse_timestamp("+5s").unwrap();
        let after = SystemTime::now();
        assert!(ts > before);
        let dur = ts.duration_since(after).unwrap_or_default();
        // Should be roughly 5 seconds from now (within 1 second tolerance)
        assert!(dur.as_secs() <= 6);
    }

    #[test]
    fn test_timestamp_invalid() {
        assert!(parse_timestamp("not-a-timestamp").is_err());
    }

    // Date conversion tests

    #[test]
    fn test_days_from_civil_epoch() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }

    #[test]
    fn test_days_from_civil_day2() {
        assert_eq!(days_from_civil(1970, 1, 2), 1);
    }

    #[test]
    fn test_days_from_civil_2000() {
        // 2000-01-01 is day 10957
        assert_eq!(days_from_civil(2000, 1, 1), 10957);
    }

    #[test]
    fn test_civil_from_days_epoch() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn test_civil_from_days_2000() {
        assert_eq!(civil_from_days(10957), (2000, 1, 1));
    }

    #[test]
    fn test_civil_roundtrip() {
        for &(y, m, d) in &[(1970, 1, 1), (2000, 2, 29), (2023, 12, 31), (1999, 6, 15)] {
            let days = days_from_civil(y, m, d);
            assert_eq!(civil_from_days(days), (y, m, d));
        }
    }

    #[test]
    fn test_weekday_epoch() {
        // 1970-01-01 was a Thursday (index 0 in our system)
        assert_eq!(weekday_from_days(0), 0); // Thursday
    }

    // Format tests

    #[test]
    fn test_format_usec_zero() {
        assert_eq!(format_usec(0), "0");
    }

    #[test]
    fn test_format_usec_microseconds() {
        assert_eq!(format_usec(500), "500us");
    }

    #[test]
    fn test_format_usec_milliseconds() {
        assert_eq!(format_usec(5000), "5ms");
    }

    #[test]
    fn test_format_usec_seconds() {
        assert_eq!(format_usec(1_500_000), "1.500s");
    }

    #[test]
    fn test_format_usec_minutes() {
        let ts = TimeSpan {
            usec: 2 * USEC_PER_MINUTE + 30 * USEC_PER_SEC,
        };
        assert_eq!(ts.to_string(), "2min 30s");
    }

    // is_day_of_week tests

    #[test]
    fn test_is_day_of_week() {
        assert!(is_day_of_week("Mon"));
        assert!(is_day_of_week("monday"));
        assert!(is_day_of_week("Fri"));
        assert!(is_day_of_week("SUNDAY"));
        assert!(!is_day_of_week("Foo"));
        assert!(!is_day_of_week("2023"));
    }

    // Unit paths tests

    #[test]
    fn test_system_unit_paths_not_empty() {
        let paths = system_unit_paths();
        assert!(!paths.is_empty());
        assert!(
            paths
                .iter()
                .any(|p| p == &PathBuf::from("/etc/systemd/system"))
        );
    }

    #[test]
    fn test_user_unit_paths_not_empty() {
        let paths = user_unit_paths();
        assert!(!paths.is_empty());
    }

    // Verify tests

    #[test]
    fn test_verify_nonexistent_file() {
        let issues = verify_unit_file("/nonexistent/test.service");
        assert!(!issues.is_empty());
        assert!(issues[0].contains("Cannot open"));
    }

    // Condition tests

    #[test]
    fn test_condition_path_exists_root() {
        let (met, _) = evaluate_condition("ConditionPathExists=/");
        assert!(met);
    }

    #[test]
    fn test_condition_path_exists_missing() {
        let (met, _) = evaluate_condition("ConditionPathExists=/nonexistent/path/foobar");
        assert!(!met);
    }

    #[test]
    fn test_condition_path_is_directory() {
        let (met, _) = evaluate_condition("ConditionPathIsDirectory=/tmp");
        assert!(met);
    }

    #[test]
    fn test_condition_path_is_directory_file() {
        let (met, _) = evaluate_condition("ConditionPathIsDirectory=/etc/hostname");
        // /etc/hostname might not exist in all test environments
        // but if it does, it's not a directory
        if Path::new("/etc/hostname").exists() {
            assert!(!met);
        }
    }

    #[test]
    fn test_condition_invalid_format() {
        let (met, reason) = evaluate_condition("garbage");
        assert!(!met);
        assert!(reason.contains("Invalid"));
    }

    #[test]
    fn test_condition_unknown_type() {
        let (met, reason) = evaluate_condition("ConditionFoo=bar");
        assert!(!met);
        assert!(reason.contains("Unknown"));
    }

    // Boot timing tests

    #[test]
    fn test_read_boot_timing_no_panic() {
        let _bt = read_boot_timing();
    }

    #[test]
    fn test_read_unit_timings_no_panic() {
        let _timings = read_unit_timings();
    }

    // Detect virtualization test

    #[test]
    fn test_detect_virtualization_no_panic() {
        let _v = detect_virtualization();
    }

    // Format timestamp test

    #[test]
    fn test_format_timestamp_epoch() {
        let s = format_timestamp(UNIX_EPOCH);
        assert!(s.contains("1970"));
        assert!(s.contains("Thu"));
    }

    #[test]
    fn test_format_timestamp_2000() {
        let t = UNIX_EPOCH + Duration::from_secs(946684800); // 2000-01-01 00:00:00 UTC
        let s = format_timestamp(t);
        assert!(s.contains("2000"));
    }

    // Hostname test

    #[test]
    fn test_hostname_no_panic() {
        let _h = hostname();
    }

    // Unit timing tests

    #[test]
    fn test_unit_timing_duration() {
        let t = UnitTiming {
            name: "test.service".to_string(),
            activating_us: 1000,
            active_us: 5000,
        };
        assert_eq!(t.duration_us(), 4000);
    }

    #[test]
    fn test_unit_timing_duration_zero() {
        let t = UnitTiming {
            name: "test.service".to_string(),
            activating_us: 5000,
            active_us: 5000,
        };
        assert_eq!(t.duration_us(), 0);
    }

    #[test]
    fn test_unit_timing_duration_saturating() {
        let t = UnitTiming {
            name: "test.service".to_string(),
            activating_us: 10000,
            active_us: 5000,
        };
        assert_eq!(t.duration_us(), 0);
    }

    // Find unit file test

    #[test]
    fn test_find_unit_file_nonexistent() {
        assert!(find_unit_file("nonexistent-unit-name-12345.service").is_none());
    }

    // Parse time unit edge cases

    #[test]
    fn test_parse_time_unit_empty() {
        let (mult, len) = parse_time_unit("").unwrap();
        assert_eq!(mult, 1);
        assert_eq!(len, 0);
    }

    #[test]
    fn test_parse_time_unit_unknown() {
        assert!(parse_time_unit("foobar").is_err());
    }

    // Normalize calendar expression

    #[test]
    fn test_normalize_calendar_date_only() {
        let norm = normalize_calendar_expression("2023-12-25").unwrap();
        assert!(norm.contains("2023-12-25"));
        assert!(norm.contains("00:00:00"));
    }

    #[test]
    fn test_normalize_calendar_time_only() {
        let norm = normalize_calendar_expression("12:30").unwrap();
        assert!(norm.contains("12:30:00"));
    }

    #[test]
    fn test_normalize_calendar_with_dow() {
        let norm = normalize_calendar_expression("Mon *-*-* 09:00").unwrap();
        assert!(norm.starts_with("Mon"));
        assert!(norm.contains("09:00:00"));
    }
}
