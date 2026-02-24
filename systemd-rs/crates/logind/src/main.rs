//! systemd-logind — Login and seat management daemon.
//!
//! This daemon manages user login sessions, seats (groups of devices),
//! and handles system power/sleep button events. It provides the
//! `org.freedesktop.login1` D-Bus interface used by desktop environments
//! (GNOME, KDE, etc.) and tools like `loginctl`.
//!
//! Features:
//! - Session tracking (create, release, list, query)
//! - Seat management (seat0 + dynamic seats)
//! - User tracking (sessions per user, state)
//! - Input device monitoring for power/sleep buttons
//! - Inhibitor lock management (shutdown, sleep, idle, etc.)
//! - D-Bus interface (`org.freedesktop.login1`) with Manager, Session, Seat, User objects
//! - D-Bus signal emission (SessionNew/Removed, UserNew/Removed, SeatNew/Removed, etc.)
//! - sd_notify protocol (READY, WATCHDOG, STATUS)
//! - Control socket for loginctl CLI (legacy)
//! - VT (virtual terminal) tracking
//! - Idle hint tracking

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use dbus::blocking::Connection;
use dbus_crossroads::{Crossroads, IfaceBuilder, IfaceToken, MethodErr};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const CONTROL_SOCKET_PATH: &str = "/run/systemd/logind-control";
const RUN_DIR: &str = "/run/systemd";
const SESSIONS_DIR: &str = "/run/systemd/sessions";
const SEATS_DIR: &str = "/run/systemd/seats";
const USERS_DIR: &str = "/run/systemd/users";
const INHIBIT_DIR: &str = "/run/systemd/inhibit";
const INPUT_DIR: &str = "/dev/input";

const DBUS_NAME: &str = "org.freedesktop.login1";
const DBUS_PATH: &str = "/org/freedesktop/login1";
const DBUS_MANAGER_IFACE: &str = "org.freedesktop.login1.Manager";
const DBUS_SESSION_IFACE: &str = "org.freedesktop.login1.Session";
const DBUS_SEAT_IFACE: &str = "org.freedesktop.login1.Seat";
const DBUS_USER_IFACE: &str = "org.freedesktop.login1.User";

const DEFAULT_INHIBIT_DELAY_MAX_USEC: u64 = 5_000_000; // 5s
const DEFAULT_USER_STOP_DELAY_USEC: u64 = 10_000_000; // 10s

// ---------------------------------------------------------------------------
// Signal handling
// ---------------------------------------------------------------------------

static SHUTDOWN_FLAG: AtomicBool = AtomicBool::new(false);
static RELOAD_FLAG: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_sigterm(_: libc::c_int) {
    SHUTDOWN_FLAG.store(true, Ordering::SeqCst);
}
extern "C" fn handle_sigint(_: libc::c_int) {
    SHUTDOWN_FLAG.store(true, Ordering::SeqCst);
}
extern "C" fn handle_sighup(_: libc::c_int) {
    RELOAD_FLAG.store(true, Ordering::SeqCst);
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
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::max_level()
        }
        fn log(&self, record: &log::Record<'_>) {
            if self.enabled(record.metadata()) {
                let ts = chrono_lite_timestamp();
                eprintln!(
                    "[{}] [{}] {}",
                    ts,
                    record.level().as_str().to_lowercase(),
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
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    let s = secs % 60;
    let millis = now.subsec_millis();
    format!("{hours:02}:{mins:02}:{s:02}.{millis:03}")
}

// ---------------------------------------------------------------------------
// sd_notify
// ---------------------------------------------------------------------------

fn sd_notify(state: &str) {
    if let Ok(path) = std::env::var("NOTIFY_SOCKET")
        && let Ok(sock) = std::os::unix::net::UnixDatagram::unbound()
    {
        let _ = sock.send_to(state.as_bytes(), &path);
    }
}

fn watchdog_interval() -> Option<Duration> {
    if let Ok(val) = std::env::var("WATCHDOG_USEC")
        && let Ok(usec) = val.parse::<u64>()
        && usec > 0
    {
        // Kick at half the interval, as recommended
        return Some(Duration::from_micros(usec / 2));
    }
    None
}

// ---------------------------------------------------------------------------
// Session / Seat / User / Inhibitor types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Session {
    /// Unique session ID (e.g. "1", "2", "c1" for automatic sessions)
    pub id: String,
    /// UID of the session owner
    pub uid: u32,
    /// Username
    pub user: String,
    /// Seat this session is attached to (if any)
    pub seat: Option<String>,
    /// Virtual terminal number (0 if none)
    pub vtnr: u32,
    /// Session type: "tty", "x11", "wayland", "mir", "unspecified"
    pub session_type: String,
    /// Session class: "user", "greeter", "lock-screen", "background"
    pub class: String,
    /// Session scope: "pam" or "systemd"
    pub scope: String,
    /// Whether this session is active (foreground on its seat)
    pub active: bool,
    /// Session state: "online", "active", "closing"
    pub state: String,
    /// TTY or display associated with this session
    pub tty: String,
    /// Display (:0 etc.) for graphical sessions
    pub display: String,
    /// Service name (e.g. "sshd", "gdm")
    pub service: String,
    /// Desktop environment (e.g. "gnome", "kde")
    pub desktop: String,
    /// Leader PID (the PAM session leader / login process)
    pub leader: u32,
    /// Remote session
    pub remote: bool,
    /// Remote host (if remote)
    pub remote_host: String,
    /// Remote user (if remote)
    pub remote_user: String,
    /// Creation timestamp (seconds since epoch)
    pub since: u64,
    /// Creation timestamp (monotonic microseconds)
    pub since_monotonic: u64,
    /// Whether session is idle
    pub idle_hint: bool,
    /// Idle since (realtime microseconds)
    pub idle_since_hint: u64,
    /// Idle since (monotonic microseconds)
    pub idle_since_hint_monotonic: u64,
    /// Whether session is locked
    pub locked_hint: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Seat {
    /// Seat name (e.g. "seat0")
    pub id: String,
    /// Active session ID on this seat (if any)
    pub active_session: Option<String>,
    /// All session IDs on this seat
    pub sessions: Vec<String>,
    /// Whether this seat can do graphical output
    pub can_graphical: bool,
    /// Whether this seat supports multiple sessions / VTs
    pub can_multi_session: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct User {
    /// UID
    pub uid: u32,
    /// GID
    pub gid: u32,
    /// Username
    pub name: String,
    /// User state: "active", "online", "lingering", "closing"
    pub state: String,
    /// Session IDs belonging to this user
    pub sessions: Vec<String>,
    /// Slice (e.g. "user-1000.slice")
    pub slice: String,
    /// Service name
    pub service: String,
    /// Runtime path (e.g. /run/user/<uid>)
    pub runtime_path: String,
    /// Login timestamp (seconds since epoch)
    pub since: u64,
    /// Login timestamp (monotonic microseconds)
    pub since_monotonic: u64,
    /// Whether user has linger enabled
    pub linger: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Inhibitor {
    /// What is inhibited: "shutdown", "sleep", "idle", "handle-power-key", etc.
    pub what: String,
    /// Who is requesting the inhibition
    pub who: String,
    /// Why is the inhibition requested
    pub why: String,
    /// Mode: "block" or "delay"
    pub mode: String,
    /// UID of requester
    pub uid: u32,
    /// PID of requester
    pub pid: u32,
    /// Unique inhibitor ID
    pub id: u64,
    /// Timestamp
    pub since: u64,
}

// ---------------------------------------------------------------------------
// LoginManager — core state
// ---------------------------------------------------------------------------

pub struct LoginManager {
    sessions: HashMap<String, Session>,
    seats: HashMap<String, Seat>,
    users: HashMap<u32, User>,
    inhibitors: HashMap<u64, Inhibitor>,
    next_session_id: u64,
    next_inhibitor_id: u64,
    /// Discovered power button input device paths
    power_button_devices: Vec<PathBuf>,
    /// Configuration
    config: LogindConfig,
}

/// Manager configuration (logind.conf)
#[derive(Debug, Clone)]
pub struct LogindConfig {
    pub n_auto_vts: u32,
    pub kill_user_processes: bool,
    pub kill_only_users: Vec<String>,
    pub kill_exclude_users: Vec<String>,
    pub idle_action: String,
    pub idle_action_usec: u64,
    pub inhibit_delay_max_usec: u64,
    pub user_stop_delay_usec: u64,
    pub handle_power_key: String,
    pub handle_suspend_key: String,
    pub handle_hibernate_key: String,
    pub handle_lid_switch: String,
    pub handle_lid_switch_external_power: String,
    pub handle_lid_switch_docked: String,
    pub holdoff_timeout_usec: u64,
    pub remove_ipc: bool,
    pub runtime_directory_size: u64,
    pub runtime_directory_inodes_max: u64,
    pub inhibitors_max: u64,
    pub sessions_max: u64,
}

impl Default for LogindConfig {
    fn default() -> Self {
        Self {
            n_auto_vts: 6,
            kill_user_processes: true,
            kill_only_users: Vec::new(),
            kill_exclude_users: vec!["root".to_string()],
            idle_action: "ignore".to_string(),
            idle_action_usec: 0,
            inhibit_delay_max_usec: DEFAULT_INHIBIT_DELAY_MAX_USEC,
            user_stop_delay_usec: DEFAULT_USER_STOP_DELAY_USEC,
            handle_power_key: "poweroff".to_string(),
            handle_suspend_key: "suspend".to_string(),
            handle_hibernate_key: "hibernate".to_string(),
            handle_lid_switch: "suspend".to_string(),
            handle_lid_switch_external_power: "suspend".to_string(),
            handle_lid_switch_docked: "ignore".to_string(),
            holdoff_timeout_usec: 30_000_000, // 30s
            remove_ipc: true,
            runtime_directory_size: 10 * 1024 * 1024 * 1024, // 10% of physical RAM or 10GiB
            runtime_directory_inodes_max: 0,                 // no limit
            inhibitors_max: 8192,
            sessions_max: 8192,
        }
    }
}

impl LoginManager {
    fn new() -> Self {
        let config = parse_logind_conf();
        let mut mgr = LoginManager {
            sessions: HashMap::new(),
            seats: HashMap::new(),
            users: HashMap::new(),
            inhibitors: HashMap::new(),
            next_session_id: 1,
            next_inhibitor_id: 1,
            power_button_devices: Vec::new(),
            config,
        };

        // Always create seat0 — the default seat
        mgr.seats.insert(
            "seat0".to_string(),
            Seat {
                id: "seat0".to_string(),
                active_session: None,
                sessions: Vec::new(),
                can_graphical: check_seat0_graphical(),
                can_multi_session: true,
            },
        );

        // Enumerate /dev/input for power buttons
        mgr.enumerate_input_devices();

        mgr
    }

    fn enumerate_input_devices(&mut self) {
        self.power_button_devices.clear();
        let input_dir = Path::new(INPUT_DIR);
        if !input_dir.is_dir() {
            return;
        }

        // Look for event devices that are power/sleep buttons by reading
        // their capabilities from sysfs.
        if let Ok(entries) = fs::read_dir("/sys/class/input") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if !name_str.starts_with("event") {
                    continue;
                }
                let sysfs_path = entry.path();
                let device_name_path = sysfs_path.join("device/name");
                if let Ok(dev_name) = fs::read_to_string(&device_name_path) {
                    let dev_name = dev_name.trim().to_lowercase();
                    if dev_name.contains("power button")
                        || dev_name.contains("sleep button")
                        || dev_name.contains("lid switch")
                    {
                        let dev_path = Path::new(INPUT_DIR).join(&*name_str);
                        log::info!(
                            "Watching system buttons on {} ({})",
                            dev_path.display(),
                            dev_name.trim()
                        );
                        self.power_button_devices.push(dev_path);
                    }
                }
            }
        }
    }

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn now_monotonic_usec() -> u64 {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        unsafe {
            libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts);
        }
        (ts.tv_sec as u64) * 1_000_000 + (ts.tv_nsec as u64) / 1000
    }

    /// Create a new session.
    #[allow(clippy::too_many_arguments)]
    fn create_session(
        &mut self,
        uid: u32,
        user: &str,
        seat: Option<&str>,
        vtnr: u32,
        session_type: &str,
        class: &str,
        tty: &str,
        leader: u32,
    ) -> String {
        let id = format!("{}", self.next_session_id);
        self.next_session_id += 1;

        let seat_name = seat.map(|s| s.to_string());
        let now = Self::now_secs();
        let now_mono = Self::now_monotonic_usec();

        let session = Session {
            id: id.clone(),
            uid,
            user: user.to_string(),
            seat: seat_name.clone(),
            vtnr,
            session_type: session_type.to_string(),
            class: class.to_string(),
            scope: "pam".to_string(),
            active: true,
            state: "active".to_string(),
            tty: tty.to_string(),
            display: String::new(),
            service: String::new(),
            desktop: String::new(),
            leader,
            remote: false,
            remote_host: String::new(),
            remote_user: String::new(),
            since: now,
            since_monotonic: now_mono,
            idle_hint: false,
            idle_since_hint: 0,
            idle_since_hint_monotonic: 0,
            locked_hint: false,
        };

        // Register in seat
        if let Some(ref seat_id) = seat_name
            && let Some(seat) = self.seats.get_mut(seat_id)
        {
            seat.sessions.push(id.clone());
            if seat.active_session.is_none() {
                seat.active_session = Some(id.clone());
            }
        }

        // Register in user tracking
        let gid = resolve_user_gid(uid);
        let user_entry = self.users.entry(uid).or_insert_with(|| User {
            uid,
            gid,
            name: user.to_string(),
            state: "active".to_string(),
            sessions: Vec::new(),
            slice: format!("user-{}.slice", uid),
            service: format!("user@{}.service", uid),
            runtime_path: format!("/run/user/{}", uid),
            since: now,
            since_monotonic: now_mono,
            linger: false,
        });
        user_entry.sessions.push(id.clone());
        user_entry.state = "active".to_string();

        // Write session file
        self.write_session_file(&session);

        self.sessions.insert(id.clone(), session);
        id
    }

    /// Release (close) a session.
    fn release_session(&mut self, session_id: &str) -> bool {
        let Some(session) = self.sessions.remove(session_id) else {
            return false;
        };

        // Remove from seat
        if let Some(ref seat_id) = session.seat
            && let Some(seat) = self.seats.get_mut(seat_id)
        {
            seat.sessions.retain(|s| s != session_id);
            if seat.active_session.as_deref() == Some(session_id) {
                seat.active_session = seat.sessions.first().cloned();
            }
        }

        // Remove from user tracking
        if let Some(user) = self.users.get_mut(&session.uid) {
            user.sessions.retain(|s| s != session_id);
            if user.sessions.is_empty() {
                user.state = "closing".to_string();
            }
        }

        // Remove session file
        let session_file = Path::new(SESSIONS_DIR).join(session_id);
        let _ = fs::remove_file(session_file);

        true
    }

    /// Create an inhibitor lock.
    fn create_inhibitor(
        &mut self,
        what: &str,
        who: &str,
        why: &str,
        mode: &str,
        uid: u32,
        pid: u32,
    ) -> u64 {
        let id = self.next_inhibitor_id;
        self.next_inhibitor_id += 1;

        let inhibitor = Inhibitor {
            what: what.to_string(),
            who: who.to_string(),
            why: why.to_string(),
            mode: mode.to_string(),
            uid,
            pid,
            id,
            since: Self::now_secs(),
        };

        // Write inhibitor file
        let inhibit_file = Path::new(INHIBIT_DIR).join(format!("{}", id));
        let content = format!(
            "WHAT={}\nWHO={}\nWHY={}\nMODE={}\nUID={}\nPID={}\n",
            what, who, why, mode, uid, pid
        );
        let _ = fs::write(&inhibit_file, content);

        self.inhibitors.insert(id, inhibitor);
        id
    }

    /// Release an inhibitor lock.
    fn release_inhibitor(&mut self, id: u64) -> bool {
        if self.inhibitors.remove(&id).is_some() {
            let inhibit_file = Path::new(INHIBIT_DIR).join(format!("{}", id));
            let _ = fs::remove_file(inhibit_file);
            true
        } else {
            false
        }
    }

    /// Clean up stale inhibitors (check if PIDs still exist).
    fn cleanup_stale_inhibitors(&mut self) {
        let stale: Vec<u64> = self
            .inhibitors
            .iter()
            .filter(|(_, inhibitor)| {
                if inhibitor.pid > 0 {
                    // Check if the process still exists
                    unsafe { libc::kill(inhibitor.pid as i32, 0) != 0 }
                } else {
                    false
                }
            })
            .map(|(id, _)| *id)
            .collect();

        for id in stale {
            log::info!("Removing stale inhibitor lock {}", id);
            self.release_inhibitor(id);
        }
    }

    /// Activate a session on its seat
    fn activate_session(&mut self, session_id: &str) -> Result<(), String> {
        let session = self
            .sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("Session '{}' not found", session_id))?;

        let seat_id = session
            .seat
            .as_ref()
            .ok_or_else(|| format!("Session '{}' has no seat", session_id))?
            .clone();

        if let Some(seat) = self.seats.get_mut(&seat_id) {
            // Deactivate current active session
            if let Some(ref old_active) = seat.active_session
                && let Some(old_session) = self.sessions.get_mut(old_active)
            {
                old_session.active = false;
                old_session.state = "online".to_string();
            }
            seat.active_session = Some(session_id.to_string());
        }
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.active = true;
            session.state = "active".to_string();
        }
        Ok(())
    }

    /// Lock a session
    fn lock_session(&mut self, session_id: &str) -> Result<(), String> {
        if self.sessions.contains_key(session_id) {
            if let Some(session) = self.sessions.get_mut(session_id) {
                session.locked_hint = true;
            }
            Ok(())
        } else {
            Err(format!("Session '{}' not found", session_id))
        }
    }

    /// Unlock a session
    fn unlock_session(&mut self, session_id: &str) -> Result<(), String> {
        if self.sessions.contains_key(session_id) {
            if let Some(session) = self.sessions.get_mut(session_id) {
                session.locked_hint = false;
            }
            Ok(())
        } else {
            Err(format!("Session '{}' not found", session_id))
        }
    }

    /// Lock all sessions
    fn lock_sessions(&mut self) {
        for session in self.sessions.values_mut() {
            session.locked_hint = true;
        }
    }

    /// Unlock all sessions
    fn unlock_sessions(&mut self) {
        for session in self.sessions.values_mut() {
            session.locked_hint = false;
        }
    }

    /// Set idle hint on a session
    fn set_idle_hint(&mut self, session_id: &str, idle: bool) -> Result<(), String> {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.idle_hint = idle;
            if idle {
                let now_usec = Self::now_secs() * 1_000_000;
                let now_mono = Self::now_monotonic_usec();
                session.idle_since_hint = now_usec;
                session.idle_since_hint_monotonic = now_mono;
            } else {
                session.idle_since_hint = 0;
                session.idle_since_hint_monotonic = 0;
            }
            Ok(())
        } else {
            Err(format!("Session '{}' not found", session_id))
        }
    }

    /// Set locked hint on a session
    fn set_locked_hint(&mut self, session_id: &str, locked: bool) -> Result<(), String> {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.locked_hint = locked;
            Ok(())
        } else {
            Err(format!("Session '{}' not found", session_id))
        }
    }

    /// Set session type
    fn set_session_type(&mut self, session_id: &str, stype: &str) -> Result<(), String> {
        let valid_types = ["tty", "x11", "wayland", "mir", "unspecified"];
        if !valid_types.contains(&stype) {
            return Err(format!("Invalid session type: {}", stype));
        }
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.session_type = stype.to_string();
            Ok(())
        } else {
            Err(format!("Session '{}' not found", session_id))
        }
    }

    /// Kill a session's leader or all processes
    fn kill_session(&self, session_id: &str, who: &str, signal: i32) -> Result<(), String> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| format!("Session '{}' not found", session_id))?;

        if (who == "leader" || who == "all") && session.leader > 0 {
            let ret = unsafe { libc::kill(session.leader as i32, signal) };
            if ret != 0 {
                return Err(format!(
                    "Failed to kill leader PID {}: {}",
                    session.leader,
                    io::Error::last_os_error()
                ));
            }
        }
        // "all" would also kill all processes in the cgroup, but we don't have
        // cgroup integration yet
        Ok(())
    }

    /// Kill a user's sessions
    fn kill_user(&self, uid: u32, signal: i32) -> Result<(), String> {
        let user = self
            .users
            .get(&uid)
            .ok_or_else(|| format!("User {} not found", uid))?;

        for sid in &user.sessions {
            if let Some(session) = self.sessions.get(sid)
                && session.leader > 0
            {
                unsafe {
                    libc::kill(session.leader as i32, signal);
                }
            }
        }
        Ok(())
    }

    /// Terminate user (release all sessions)
    fn terminate_user(&mut self, uid: u32) -> Result<(), String> {
        let user = self
            .users
            .get(&uid)
            .ok_or_else(|| format!("User {} not found", uid))?;

        let session_ids: Vec<String> = user.sessions.clone();
        for sid in &session_ids {
            self.release_session(sid);
        }
        Ok(())
    }

    /// Terminate a seat (release all sessions on it)
    fn terminate_seat(&mut self, seat_id: &str) -> Result<(), String> {
        let seat = self
            .seats
            .get(seat_id)
            .ok_or_else(|| format!("Seat '{}' not found", seat_id))?;

        let session_ids: Vec<String> = seat.sessions.clone();
        for sid in &session_ids {
            self.release_session(sid);
        }
        Ok(())
    }

    /// Check if an action can be performed (checking inhibitors)
    fn can_action(&self, action: &str) -> &'static str {
        let blocked = self.inhibitors.values().any(|inhibitor| {
            inhibitor.mode == "block"
                && (inhibitor.what.contains("shutdown")
                    || inhibitor.what.contains(action)
                    || inhibitor.what.contains("sleep"))
        });
        if blocked { "challenge" } else { "yes" }
    }

    /// Get the global idle hint (any active session not idle => not idle)
    fn global_idle_hint(&self) -> bool {
        // If there are no sessions, consider idle
        if self.sessions.is_empty() {
            return true;
        }
        // If any active session is not idle, global idle is false
        for session in self.sessions.values() {
            if session.active && !session.idle_hint {
                return false;
            }
        }
        true
    }

    /// Get the global idle-since hint (earliest idle session)
    fn global_idle_since_hint(&self) -> (u64, u64) {
        let mut earliest_rt: u64 = 0;
        let mut earliest_mono: u64 = 0;
        for session in self.sessions.values() {
            if session.idle_hint
                && session.idle_since_hint > 0
                && (earliest_rt == 0 || session.idle_since_hint < earliest_rt)
            {
                earliest_rt = session.idle_since_hint;
                earliest_mono = session.idle_since_hint_monotonic;
            }
        }
        (earliest_rt, earliest_mono)
    }

    /// Get which inhibit types are blocked
    fn block_inhibited(&self) -> String {
        let mut types = std::collections::BTreeSet::new();
        for inhibitor in self.inhibitors.values() {
            if inhibitor.mode == "block" {
                for what in inhibitor.what.split(':') {
                    types.insert(what.trim().to_string());
                }
            }
        }
        let v: Vec<&str> = types.iter().map(|s| s.as_str()).collect();
        v.join(":")
    }

    /// Get which inhibit types are delayed
    fn delay_inhibited(&self) -> String {
        let mut types = std::collections::BTreeSet::new();
        for inhibitor in self.inhibitors.values() {
            if inhibitor.mode == "delay" {
                for what in inhibitor.what.split(':') {
                    types.insert(what.trim().to_string());
                }
            }
        }
        let v: Vec<&str> = types.iter().map(|s| s.as_str()).collect();
        v.join(":")
    }

    /// Write session state file to /run/systemd/sessions/<id>
    fn write_session_file(&self, session: &Session) {
        let session_file = Path::new(SESSIONS_DIR).join(&session.id);
        let mut content = String::new();
        content.push_str(&format!("UID={}\n", session.uid));
        content.push_str(&format!("USER={}\n", session.user));
        content.push_str(&format!(
            "ACTIVE={}\n",
            if session.active { "1" } else { "0" }
        ));
        content.push_str(&format!("STATE={}\n", session.state));
        content.push_str(&format!("TYPE={}\n", session.session_type));
        content.push_str(&format!("CLASS={}\n", session.class));
        if let Some(ref seat) = session.seat {
            content.push_str(&format!("SEAT={}\n", seat));
        }
        if session.vtnr > 0 {
            content.push_str(&format!("VTNR={}\n", session.vtnr));
        }
        if !session.tty.is_empty() {
            content.push_str(&format!("TTY={}\n", session.tty));
        }
        if !session.display.is_empty() {
            content.push_str(&format!("DISPLAY={}\n", session.display));
        }
        if !session.service.is_empty() {
            content.push_str(&format!("SERVICE={}\n", session.service));
        }
        if !session.desktop.is_empty() {
            content.push_str(&format!("DESKTOP={}\n", session.desktop));
        }
        if session.leader > 0 {
            content.push_str(&format!("LEADER={}\n", session.leader));
        }
        content.push_str(&format!("SCOPE={}\n", session.scope));
        content.push_str(&format!(
            "REMOTE={}\n",
            if session.remote { "1" } else { "0" }
        ));
        if session.remote && !session.remote_host.is_empty() {
            content.push_str(&format!("REMOTE_HOST={}\n", session.remote_host));
        }
        if session.remote && !session.remote_user.is_empty() {
            content.push_str(&format!("REMOTE_USER={}\n", session.remote_user));
        }
        content.push_str(&format!(
            "REALTIME={}\n",
            session.since.saturating_mul(1_000_000)
        ));
        content.push_str(&format!("MONOTONIC={}\n", session.since_monotonic));
        let _ = fs::write(session_file, content);
    }

    /// Write seat state file to /run/systemd/seats/<id>
    fn write_seat_files(&self) {
        for seat in self.seats.values() {
            let seat_file = Path::new(SEATS_DIR).join(&seat.id);
            let mut content = String::new();
            content.push_str(&format!("ID={}\n", seat.id));
            if let Some(ref active) = seat.active_session {
                content.push_str(&format!("ACTIVE_SESSION={}\n", active));
                if let Some(session) = self.sessions.get(active) {
                    content.push_str(&format!("ACTIVE_SESSION_UID={}\n", session.uid));
                }
            }
            content.push_str(&format!(
                "CAN_GRAPHICAL={}\n",
                if seat.can_graphical { "1" } else { "0" }
            ));
            content.push_str(&format!(
                "CAN_MULTI_SESSION={}\n",
                if seat.can_multi_session { "1" } else { "0" }
            ));
            let sessions_str: Vec<&str> = seat.sessions.iter().map(|s| s.as_str()).collect();
            if !sessions_str.is_empty() {
                content.push_str(&format!("SESSIONS={}\n", sessions_str.join(" ")));
            }
            let _ = fs::write(seat_file, content);
        }
    }

    /// Write user state files to /run/systemd/users/<uid>
    fn write_user_files(&self) {
        for user in self.users.values() {
            let user_file = Path::new(USERS_DIR).join(format!("{}", user.uid));
            let mut content = String::new();
            content.push_str(&format!("NAME={}\n", user.name));
            content.push_str(&format!("STATE={}\n", user.state));
            content.push_str(&format!("SLICE={}\n", user.slice));
            content.push_str(&format!("SERVICE={}\n", user.service));
            content.push_str(&format!("RUNTIME_PATH={}\n", user.runtime_path));
            let sessions_str: Vec<&str> = user.sessions.iter().map(|s| s.as_str()).collect();
            if !sessions_str.is_empty() {
                content.push_str(&format!("SESSIONS={}\n", sessions_str.join(" ")));
                content.push_str(&format!("DISPLAY={}\n", sessions_str[0]));
            }
            content.push_str(&format!(
                "REALTIME={}\n",
                user.since.saturating_mul(1_000_000)
            ));
            content.push_str(&format!("MONOTONIC={}\n", user.since_monotonic));
            let _ = fs::write(user_file, content);
        }
    }

    /// Synchronize all runtime files
    fn sync_runtime_state(&self) {
        // Write session files
        for session in self.sessions.values() {
            self.write_session_file(session);
        }
        // Write seat files
        self.write_seat_files();
        // Write user files
        self.write_user_files();
    }

    /// Auto-detect existing sessions from /proc
    fn detect_existing_sessions(&mut self) {
        // Look for existing utmp entries to detect already-logged-in users.
        // This is a simplified detection — real logind uses PAM hooks.
        let utmp_path = Path::new("/var/run/utmp");
        if !utmp_path.exists() {
            return;
        }

        // Try to parse utmp entries to find active login sessions.
        // Each utmp record is fixed-size; we do a simple scan.
        // For now we'll rely on PAM or systemctl creating sessions.
        log::trace!("Checking utmp for existing sessions");
    }

    fn format_status(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("{} seats logged in.\n", self.seats.len()));
        for seat in self.seats.values() {
            out.push_str(&format!(
                "  Seat {}: {} session(s), active: {}\n",
                seat.id,
                seat.sessions.len(),
                seat.active_session.as_deref().unwrap_or("(none)")
            ));
        }
        out.push_str(&format!("{} sessions.\n", self.sessions.len()));
        out.push_str(&format!("{} users.\n", self.users.len()));
        out.push_str(&format!("{} inhibitors.\n", self.inhibitors.len()));
        out
    }
}

// ---------------------------------------------------------------------------
// Configuration parsing
// ---------------------------------------------------------------------------

fn parse_logind_conf() -> LogindConfig {
    let mut config = LogindConfig::default();

    for path in &[
        "/etc/systemd/logind.conf",
        "/run/systemd/logind.conf.d",
        "/usr/lib/systemd/logind.conf.d",
    ] {
        if Path::new(path).is_file() {
            parse_logind_conf_file(path, &mut config);
        } else if Path::new(path).is_dir()
            && let Ok(entries) = fs::read_dir(path)
        {
            let mut files: Vec<PathBuf> = entries
                .flatten()
                .filter(|e| {
                    e.path()
                        .extension()
                        .map(|ext| ext == "conf")
                        .unwrap_or(false)
                })
                .map(|e| e.path())
                .collect();
            files.sort();
            for file in files {
                parse_logind_conf_file(&file.to_string_lossy(), &mut config);
            }
        }
    }

    config
}

fn parse_logind_conf_file(path: &str, config: &mut LogindConfig) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let mut in_login_section = false;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.eq_ignore_ascii_case("[login]") {
            in_login_section = true;
            continue;
        }
        if line.starts_with('[') {
            in_login_section = false;
            continue;
        }
        if !in_login_section {
            continue;
        }

        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim();
            let val = val.trim();
            match key {
                "NAutoVTs" => {
                    if let Ok(n) = val.parse() {
                        config.n_auto_vts = n;
                    }
                }
                "KillUserProcesses" => {
                    config.kill_user_processes = val == "yes" || val == "true" || val == "1";
                }
                "KillOnlyUsers" => {
                    config.kill_only_users =
                        val.split_whitespace().map(|s| s.to_string()).collect();
                }
                "KillExcludeUsers" => {
                    config.kill_exclude_users =
                        val.split_whitespace().map(|s| s.to_string()).collect();
                }
                "IdleAction" => {
                    config.idle_action = val.to_string();
                }
                "IdleActionSec" => {
                    if let Ok(secs) = parse_timespan_to_usec(val) {
                        config.idle_action_usec = secs;
                    }
                }
                "InhibitDelayMaxSec" => {
                    if let Ok(usec) = parse_timespan_to_usec(val) {
                        config.inhibit_delay_max_usec = usec;
                    }
                }
                "UserStopDelaySec" => {
                    if let Ok(usec) = parse_timespan_to_usec(val) {
                        config.user_stop_delay_usec = usec;
                    }
                }
                "HandlePowerKey" => {
                    config.handle_power_key = val.to_string();
                }
                "HandleSuspendKey" => {
                    config.handle_suspend_key = val.to_string();
                }
                "HandleHibernateKey" => {
                    config.handle_hibernate_key = val.to_string();
                }
                "HandleLidSwitch" => {
                    config.handle_lid_switch = val.to_string();
                }
                "HandleLidSwitchExternalPower" => {
                    config.handle_lid_switch_external_power = val.to_string();
                }
                "HandleLidSwitchDocked" => {
                    config.handle_lid_switch_docked = val.to_string();
                }
                "HoldoffTimeoutSec" => {
                    if let Ok(usec) = parse_timespan_to_usec(val) {
                        config.holdoff_timeout_usec = usec;
                    }
                }
                "RemoveIPC" => {
                    config.remove_ipc = val == "yes" || val == "true" || val == "1";
                }
                "InhibitorsMax" => {
                    if let Ok(n) = val.parse() {
                        config.inhibitors_max = n;
                    }
                }
                "SessionsMax" => {
                    if let Ok(n) = val.parse() {
                        config.sessions_max = n;
                    }
                }
                _ => {}
            }
        }
    }
}

fn parse_timespan_to_usec(val: &str) -> Result<u64, ()> {
    // Simple parser for time spans like "5s", "30", "1min", "500ms"
    let val = val.trim();
    if val.is_empty() {
        return Err(());
    }
    if let Ok(n) = val.parse::<u64>() {
        // Plain number = seconds
        return Ok(n * 1_000_000);
    }
    // Suffixes ordered longest-first within each unit to avoid partial matches
    // (e.g. "5sec" must not match 's' leaving "5se")
    // Order matters: longer suffixes must come before shorter ones that
    // could be a suffix of the longer form.  E.g. "minutes" ends with "s",
    // so the "s" entry would incorrectly match "2minutes" (leaving "2minute"
    // which is not a number).  By trying all multi-char suffixes first and
    // single-char ones last we avoid false positives.
    let suffixes: &[(&str, u64)] = &[
        ("usec", 1),
        ("us", 1),
        ("msec", 1_000),
        ("ms", 1_000),
        ("minutes", 60 * 1_000_000),
        ("minute", 60 * 1_000_000),
        ("min", 60 * 1_000_000),
        ("hours", 3600 * 1_000_000),
        ("hour", 3600 * 1_000_000),
        ("hr", 3600 * 1_000_000),
        ("seconds", 1_000_000),
        ("second", 1_000_000),
        ("sec", 1_000_000),
        ("s", 1_000_000),
        ("h", 3600 * 1_000_000),
    ];
    for &(suffix, multiplier) in suffixes {
        if let Some(num_str) = val.strip_suffix(suffix)
            && let Ok(n) = num_str.trim().parse::<u64>()
        {
            return Ok(n * multiplier);
        }
        // suffix matched but the numeric part didn't parse —
        // keep trying other suffixes (e.g. "2minutes" matches "s"
        // leaving "2minute", which is invalid, but also matches
        // "minutes" leaving "2", which is valid).
    }
    Err(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn check_seat0_graphical() -> bool {
    // Check if any DRM/framebuffer device exists
    Path::new("/dev/dri").is_dir() || Path::new("/dev/fb0").exists()
}

/// Resolve a user's primary GID from /etc/passwd
fn resolve_user_gid(uid: u32) -> u32 {
    if let Ok(content) = fs::read_to_string("/etc/passwd") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 4
                && let Ok(file_uid) = parts[2].parse::<u32>()
                && file_uid == uid
                && let Ok(gid) = parts[3].parse::<u32>()
            {
                return gid;
            }
        }
    }
    uid // fallback: gid = uid
}

/// Convert a session ID to a D-Bus object path component.
/// D-Bus object path components can't start with a digit, so prefix with '_'.
fn session_object_path(session_id: &str) -> dbus::Path<'static> {
    let escaped = session_id.replace('-', "_2d");
    dbus::Path::new(format!("{}/session/_{}", DBUS_PATH, escaped)).unwrap_or_else(|_| {
        dbus::Path::new(format!("{}/session/_unknown", DBUS_PATH)).expect("valid path")
    })
}

/// Convert a seat name to a D-Bus object path.
fn seat_object_path(seat_id: &str) -> dbus::Path<'static> {
    let escaped = seat_id.replace('-', "_2d");
    dbus::Path::new(format!("{}/seat/{}", DBUS_PATH, escaped)).unwrap_or_else(|_| {
        dbus::Path::new(format!("{}/seat/unknown", DBUS_PATH)).expect("valid path")
    })
}

/// Convert a UID to a D-Bus object path.
fn user_object_path(uid: u32) -> dbus::Path<'static> {
    dbus::Path::new(format!("{}/user/_{}", DBUS_PATH, uid))
        .unwrap_or_else(|_| dbus::Path::new(format!("{}/user/_0", DBUS_PATH)).expect("valid path"))
}

/// Extract session ID from a D-Bus object path
fn session_id_from_path(path: &str) -> Option<String> {
    let prefix = format!("{}/session/_", DBUS_PATH);
    path.strip_prefix(&prefix)
        .map(|rest| rest.replace("_2d", "-"))
}

/// Extract seat ID from a D-Bus object path
fn seat_id_from_path(path: &str) -> Option<String> {
    let prefix = format!("{}/seat/", DBUS_PATH);
    path.strip_prefix(&prefix)
        .map(|rest| rest.replace("_2d", "-"))
}

/// Extract UID from a D-Bus object path
fn uid_from_path(path: &str) -> Option<u32> {
    let prefix = format!("{}/user/_", DBUS_PATH);
    if let Some(rest) = path.strip_prefix(&prefix) {
        rest.parse().ok()
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// D-Bus interface setup
// ---------------------------------------------------------------------------

type SharedManager = Arc<Mutex<LoginManager>>;

/// Register the org.freedesktop.login1.Manager interface
fn register_manager_iface(cr: &mut Crossroads, mgr: &SharedManager) -> IfaceToken<SharedManager> {
    let mgr_for_signals = mgr.clone();

    cr.register(DBUS_MANAGER_IFACE, move |b: &mut IfaceBuilder<SharedManager>| {
        // -------------------------------------------------------------------
        // Signals — each statement registers the signal and drops the builder
        // -------------------------------------------------------------------
        b.signal::<(String, dbus::Path<'static>), _>("SessionNew", ("session_id", "object_path"));
        b.signal::<(String, dbus::Path<'static>), _>("SessionRemoved", ("session_id", "object_path"));
        b.signal::<(u32, dbus::Path<'static>), _>("UserNew", ("uid", "object_path"));
        b.signal::<(u32, dbus::Path<'static>), _>("UserRemoved", ("uid", "object_path"));
        b.signal::<(String, dbus::Path<'static>), _>("SeatNew", ("seat_id", "object_path"));
        b.signal::<(String, dbus::Path<'static>), _>("SeatRemoved", ("seat_id", "object_path"));
        b.signal::<(bool,), _>("PrepareForShutdown", ("active",));
        b.signal::<(bool,), _>("PrepareForSleep", ("active",));

        // -------------------------------------------------------------------
        // Properties
        // -------------------------------------------------------------------
        {
            let m = mgr_for_signals.clone();
            b.property("NAutoVTs")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.n_auto_vts)
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("KillOnlyUsers")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.kill_only_users.clone())
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("KillExcludeUsers")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.kill_exclude_users.clone())
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("KillUserProcesses")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.kill_user_processes)
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("IdleHint")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.global_idle_hint())
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("IdleSinceHint")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let (rt, _) = mgr.global_idle_since_hint();
                    Ok(rt)
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("IdleSinceHintMonotonic")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let (_, mono) = mgr.global_idle_since_hint();
                    Ok(mono)
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("BlockInhibited")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.block_inhibited())
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("DelayInhibited")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.delay_inhibited())
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("InhibitDelayMaxUSec")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.inhibit_delay_max_usec)
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("UserStopDelayUSec")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.user_stop_delay_usec)
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("HandlePowerKey")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.handle_power_key.clone())
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("HandleSuspendKey")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.handle_suspend_key.clone())
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("HandleHibernateKey")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.handle_hibernate_key.clone())
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("HandleLidSwitch")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.handle_lid_switch.clone())
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("HandleLidSwitchExternalPower")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.handle_lid_switch_external_power.clone())
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("HandleLidSwitchDocked")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.handle_lid_switch_docked.clone())
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("HoldoffTimeoutUSec")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.holdoff_timeout_usec)
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("IdleAction")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.idle_action.clone())
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("IdleActionUSec")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.idle_action_usec)
                });
        }
        b.property("PreparingForShutdown")
            .get(|_, _: &mut SharedManager| Ok(false));
        b.property("PreparingForSleep")
            .get(|_, _: &mut SharedManager| Ok(false));
        b.property("Docked")
            .get(|_, _: &mut SharedManager| Ok(false));
        b.property("LidClosed")
            .get(|_, _: &mut SharedManager| Ok(false));
        b.property("OnExternalPower")
            .get(|_, _: &mut SharedManager| {
                // Check /sys/class/power_supply/*/online
                let on_ac = check_ac_power();
                Ok(on_ac)
            });
        {
            let m = mgr_for_signals.clone();
            b.property("RemoveIPC")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.remove_ipc)
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("RuntimeDirectorySize")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.runtime_directory_size)
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("RuntimeDirectoryInodesMax")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.runtime_directory_inodes_max)
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("InhibitorsMax")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.inhibitors_max)
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("SessionsMax")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.config.sessions_max)
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("NCurrentSessions")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.sessions.len() as u64)
                });
        }
        {
            let m = mgr_for_signals.clone();
            b.property("NCurrentInhibitors")
                .get(move |_, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    Ok(mgr.inhibitors.len() as u64)
                });
        }

        // -------------------------------------------------------------------
        // Methods
        // -------------------------------------------------------------------

        // GetSession(s) -> o
        {
            let m = mgr_for_signals.clone();
            b.method("GetSession", ("session_id",), ("object_path",), move |_, _: &mut SharedManager, (session_id,): (String,)| {
                let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                if mgr.sessions.contains_key(&session_id) {
                    Ok((session_object_path(&session_id),))
                } else {
                    Err(MethodErr::failed(&format!("No session '{}' known", session_id)))
                }
            });
        }

        // GetSessionByPID(u) -> o
        {
            let m = mgr_for_signals.clone();
            b.method("GetSessionByPID", ("pid",), ("object_path",), move |_, _: &mut SharedManager, (pid,): (u32,)| {
                let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                for session in mgr.sessions.values() {
                    if session.leader == pid {
                        return Ok((session_object_path(&session.id),));
                    }
                }
                Err(MethodErr::failed(&format!("No session for PID {} known", pid)))
            });
        }

        // GetUser(u) -> o
        {
            let m = mgr_for_signals.clone();
            b.method("GetUser", ("uid",), ("object_path",), move |_, _: &mut SharedManager, (uid,): (u32,)| {
                let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                if mgr.users.contains_key(&uid) {
                    Ok((user_object_path(uid),))
                } else {
                    Err(MethodErr::failed(&format!("No user '{}' known", uid)))
                }
            });
        }

        // GetUserByPID(u) -> o
        {
            let m = mgr_for_signals.clone();
            b.method("GetUserByPID", ("pid",), ("object_path",), move |_, _: &mut SharedManager, (pid,): (u32,)| {
                let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                for session in mgr.sessions.values() {
                    if session.leader == pid {
                        return Ok((user_object_path(session.uid),));
                    }
                }
                Err(MethodErr::failed(&format!("No user for PID {} known", pid)))
            });
        }

        // GetSeat(s) -> o
        {
            let m = mgr_for_signals.clone();
            b.method("GetSeat", ("seat_id",), ("object_path",), move |_, _: &mut SharedManager, (seat_id,): (String,)| {
                let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                if mgr.seats.contains_key(&seat_id) {
                    Ok((seat_object_path(&seat_id),))
                } else {
                    Err(MethodErr::failed(&format!("No seat '{}' known", seat_id)))
                }
            });
        }

        // ListSessions() -> a(susso)
        {
            let m = mgr_for_signals.clone();
            b.method("ListSessions", (), ("sessions",), move |_, _: &mut SharedManager, ()| {
                let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                let mut result: Vec<(String, u32, String, String, dbus::Path<'static>)> = Vec::new();
                for session in mgr.sessions.values() {
                    result.push((
                        session.id.clone(),
                        session.uid,
                        session.user.clone(),
                        session.seat.clone().unwrap_or_default(),
                        session_object_path(&session.id),
                    ));
                }
                Ok((result,))
            });
        }

        // ListUsers() -> a(uso)
        {
            let m = mgr_for_signals.clone();
            b.method("ListUsers", (), ("users",), move |_, _: &mut SharedManager, ()| {
                let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                let mut result: Vec<(u32, String, dbus::Path<'static>)> = Vec::new();
                for user in mgr.users.values() {
                    result.push((
                        user.uid,
                        user.name.clone(),
                        user_object_path(user.uid),
                    ));
                }
                Ok((result,))
            });
        }

        // ListSeats() -> a(so)
        {
            let m = mgr_for_signals.clone();
            b.method("ListSeats", (), ("seats",), move |_, _: &mut SharedManager, ()| {
                let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                let mut result: Vec<(String, dbus::Path<'static>)> = Vec::new();
                for seat in mgr.seats.values() {
                    result.push((
                        seat.id.clone(),
                        seat_object_path(&seat.id),
                    ));
                }
                Ok((result,))
            });
        }

        // ListInhibitors() -> a(ssssuu)
        {
            let m = mgr_for_signals.clone();
            b.method("ListInhibitors", (), ("inhibitors",), move |_, _: &mut SharedManager, ()| {
                let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                let mut result: Vec<(String, String, String, String, u32, u32)> = Vec::new();
                for inhibitor in mgr.inhibitors.values() {
                    result.push((
                        inhibitor.what.clone(),
                        inhibitor.who.clone(),
                        inhibitor.why.clone(),
                        inhibitor.mode.clone(),
                        inhibitor.uid,
                        inhibitor.pid,
                    ));
                }
                Ok((result,))
            });
        }

        // CreateSession(uusssssussbssa(sv)) -> soshusub
        // Simplified: we accept basic parameters and return session info
        {
            let m = mgr_for_signals.clone();
            b.method(
                "CreateSession",
                ("uid", "pid", "service", "type", "class", "seat_id", "vtnr", "tty", "display", "remote", "remote_user", "remote_host"),
                ("session_id", "object_path", "runtime_path", "fifo_fd", "uid_out", "seat_id_out", "vtnr_out", "existing"),
                move |_, _: &mut SharedManager, (uid, _pid, _service, stype, class, seat_id, vtnr, tty, _display, _remote, _remote_user, _remote_host): (u32, u32, String, String, String, String, u32, String, String, bool, String, String)| {
                    let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());

                    // Resolve username
                    let user = resolve_uid_to_name(uid);
                    let seat = if seat_id.is_empty() { None } else { Some(seat_id.as_str()) };

                    let id = mgr.create_session(uid, &user, seat, vtnr, &stype, &class, &tty, _pid);
                    mgr.sync_runtime_state();

                    log::info!("New session {} of user {} on {}", id, user, seat.unwrap_or("(no seat)"));

                    let obj_path = session_object_path(&id);
                    let runtime_path = format!("/run/user/{}", uid);

                    // We don't support passing a real FD here, so we return false for fifo_fd usage
                    // The "existing" flag is false since we always create new
                    Ok((id, obj_path, runtime_path, false, uid, seat_id, vtnr, false))
                },
            );
        }

        // ReleaseSession(s)
        {
            let m = mgr_for_signals.clone();
            b.method("ReleaseSession", ("session_id",), (), move |_, _: &mut SharedManager, (session_id,): (String,)| {
                let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                if mgr.release_session(&session_id) {
                    mgr.sync_runtime_state();
                    log::info!("Released session {}", session_id);
                    Ok(())
                } else {
                    Err(MethodErr::failed(&format!("No session '{}' known", session_id)))
                }
            });
        }

        // ActivateSession(s)
        {
            let m = mgr_for_signals.clone();
            b.method("ActivateSession", ("session_id",), (), move |_, _: &mut SharedManager, (session_id,): (String,)| {
                let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                match mgr.activate_session(&session_id) {
                    Ok(()) => {
                        mgr.sync_runtime_state();
                        log::info!("Activated session {}", session_id);
                        Ok(())
                    }
                    Err(e) => Err(MethodErr::failed(&e)),
                }
            });
        }

        // ActivateSessionOnSeat(ss)
        {
            let m = mgr_for_signals.clone();
            b.method("ActivateSessionOnSeat", ("session_id", "seat_id"), (), move |_, _: &mut SharedManager, (session_id, seat_id): (String, String)| {
                let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                // Verify the session is on the specified seat
                if let Some(session) = mgr.sessions.get(&session_id)
                    && session.seat.as_deref() != Some(&seat_id) {
                        return Err(MethodErr::failed(&format!(
                            "Session '{}' not on seat '{}'", session_id, seat_id
                        )));
                    }
                match mgr.activate_session(&session_id) {
                    Ok(()) => {
                        mgr.sync_runtime_state();
                        Ok(())
                    }
                    Err(e) => Err(MethodErr::failed(&e)),
                }
            });
        }

        // LockSession(s)
        {
            let m = mgr_for_signals.clone();
            b.method("LockSession", ("session_id",), (), move |_, _: &mut SharedManager, (session_id,): (String,)| {
                let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                mgr.lock_session(&session_id).map_err(|e| MethodErr::failed(&e))
            });
        }

        // UnlockSession(s)
        {
            let m = mgr_for_signals.clone();
            b.method("UnlockSession", ("session_id",), (), move |_, _: &mut SharedManager, (session_id,): (String,)| {
                let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                mgr.unlock_session(&session_id).map_err(|e| MethodErr::failed(&e))
            });
        }

        // LockSessions()
        {
            let m = mgr_for_signals.clone();
            b.method("LockSessions", (), (), move |_, _: &mut SharedManager, ()| {
                let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                mgr.lock_sessions();
                Ok(())
            });
        }

        // UnlockSessions()
        {
            let m = mgr_for_signals.clone();
            b.method("UnlockSessions", (), (), move |_, _: &mut SharedManager, ()| {
                let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                mgr.unlock_sessions();
                Ok(())
            });
        }

        // KillSession(ssi) — session_id, who ("leader"|"all"), signal_number
        {
            let m = mgr_for_signals.clone();
            b.method("KillSession", ("session_id", "who", "signal_number"), (), move |_, _: &mut SharedManager, (session_id, who, signal): (String, String, i32)| {
                let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                mgr.kill_session(&session_id, &who, signal).map_err(|e| MethodErr::failed(&e))
            });
        }

        // KillUser(ui) — uid, signal_number
        {
            let m = mgr_for_signals.clone();
            b.method("KillUser", ("uid", "signal_number"), (), move |_, _: &mut SharedManager, (uid, signal): (u32, i32)| {
                let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                mgr.kill_user(uid, signal).map_err(|e| MethodErr::failed(&e))
            });
        }

        // TerminateSession(s)
        {
            let m = mgr_for_signals.clone();
            b.method("TerminateSession", ("session_id",), (), move |_, _: &mut SharedManager, (session_id,): (String,)| {
                let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                if mgr.release_session(&session_id) {
                    mgr.sync_runtime_state();
                    log::info!("Terminated session {}", session_id);
                    Ok(())
                } else {
                    Err(MethodErr::failed(&format!("No session '{}' known", session_id)))
                }
            });
        }

        // TerminateUser(u)
        {
            let m = mgr_for_signals.clone();
            b.method("TerminateUser", ("uid",), (), move |_, _: &mut SharedManager, (uid,): (u32,)| {
                let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                match mgr.terminate_user(uid) {
                    Ok(()) => {
                        mgr.sync_runtime_state();
                        log::info!("Terminated user {}", uid);
                        Ok(())
                    }
                    Err(e) => Err(MethodErr::failed(&e)),
                }
            });
        }

        // TerminateSeat(s)
        {
            let m = mgr_for_signals.clone();
            b.method("TerminateSeat", ("seat_id",), (), move |_, _: &mut SharedManager, (seat_id,): (String,)| {
                let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                match mgr.terminate_seat(&seat_id) {
                    Ok(()) => {
                        mgr.sync_runtime_state();
                        Ok(())
                    }
                    Err(e) => Err(MethodErr::failed(&e)),
                }
            });
        }

        // SetUserLinger(ubb) — uid, enable, interactive
        {
            let m = mgr_for_signals.clone();
            b.method("SetUserLinger", ("uid", "enable", "interactive"), (), move |_, _: &mut SharedManager, (uid, enable, _interactive): (u32, bool, bool)| {
                let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(user) = mgr.users.get_mut(&uid) {
                    user.linger = enable;
                    // Create/remove linger file
                    let linger_path = format!("/var/lib/systemd/linger/{}", user.name);
                    if enable {
                        let _ = fs::create_dir_all("/var/lib/systemd/linger");
                        let _ = fs::write(&linger_path, "");
                    } else {
                        let _ = fs::remove_file(&linger_path);
                    }
                    Ok(())
                } else {
                    Err(MethodErr::failed(&format!("User {} not known", uid)))
                }
            });
        }

        // AttachDevice(ssb) — seat_id, sysfs_path, interactive
        b.method("AttachDevice", ("seat_id", "sysfs_path", "interactive"), (), |_, _: &mut SharedManager, (_seat_id, _sysfs_path, _interactive): (String, String, bool)| {
            // Not yet implemented
            Ok(())
        });

        // FlushDevices(b) — interactive
        {
            let m = mgr_for_signals.clone();
            b.method("FlushDevices", ("interactive",), (), move |_, _: &mut SharedManager, (_interactive,): (bool,)| {
                let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                mgr.enumerate_input_devices();
                Ok(())
            });
        }

        // PowerOff(b), Reboot(b), Halt(b), Suspend(b), Hibernate(b), HybridSleep(b), SuspendThenHibernate(b)
        for action in &["PowerOff", "Reboot", "Halt", "Suspend", "Hibernate", "HybridSleep", "SuspendThenHibernate"] {
            let action_name = action.to_string();
            b.method(*action, ("interactive",), (), move |_, _: &mut SharedManager, (_interactive,): (bool,)| {
                log::info!("D-Bus {} requested", action_name);
                // In a full implementation, this would trigger the action
                // through the PID 1 service manager
                Ok(())
            });
        }

        // CanPowerOff() -> s, CanReboot() -> s, etc.
        for (method_name, action_name) in &[
            ("CanPowerOff", "poweroff"),
            ("CanReboot", "reboot"),
            ("CanHalt", "halt"),
            ("CanSuspend", "suspend"),
            ("CanHibernate", "hibernate"),
            ("CanHybridSleep", "hybrid-sleep"),
            ("CanSuspendThenHibernate", "suspend-then-hibernate"),
        ] {
            let m = mgr_for_signals.clone();
            let action = action_name.to_string();
            b.method(*method_name, (), ("result",), move |_, _: &mut SharedManager, ()| {
                let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                Ok((mgr.can_action(&action).to_string(),))
            });
        }

        // Inhibit(ssss) -> h (what, who, why, mode -> fd)
        // Since we can't easily pass FDs through dbus-crossroads callbacks,
        // we return the inhibitor ID as a workaround. Real systemd returns a
        // pipe FD that, when closed, releases the inhibitor.
        {
            let m = mgr_for_signals.clone();
            b.method("Inhibit", ("what", "who", "why", "mode"), ("fd",), move |_, _: &mut SharedManager, (what, who, why, mode): (String, String, String, String)| {
                let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());

                // Validate mode
                if mode != "block" && mode != "delay" {
                    return Err(MethodErr::failed("Invalid mode, must be 'block' or 'delay'"));
                }

                let id = mgr.create_inhibitor(&what, &who, &why, &mode, 0, 0);
                log::info!("New D-Bus inhibitor {} ({}): {} — {}", id, what, who, why);

                // Create a pipe - when the caller closes their end, the inhibitor is released
                let mut fds = [0i32; 2];
                let ret = unsafe { libc::pipe(fds.as_mut_ptr()) };
                if ret == 0 {
                    // Return the write end to the caller; we keep the read end
                    // In practice we'd monitor the read end to detect closure,
                    // but for now we accept and return the FD
                    let fd = unsafe { dbus::arg::OwnedFd::new(fds[1]) };
                    // Close our end of the write side - caller gets ownership
                    unsafe { libc::close(fds[0]); }
                    Ok((fd,))
                } else {
                    Err(MethodErr::failed("Failed to create pipe for inhibitor"))
                }
            });
        }

        // ScheduleShutdown(st) — type, usec
        b.method("ScheduleShutdown", ("shutdown_type", "usec"), (), |_, _: &mut SharedManager, (_stype, _usec): (String, u64)| {
            log::info!("ScheduleShutdown requested (not yet implemented)");
            Ok(())
        });

        // CancelScheduledShutdown() -> b
        b.method("CancelScheduledShutdown", (), ("cancelled",), |_, _: &mut SharedManager, ()| {
            Ok((false,))
        });

        // SetWallMessage(sb) — message, enable
        b.method("SetWallMessage", ("wall_message", "enable"), (), |_, _: &mut SharedManager, (_msg, _enable): (String, bool)| {
            Ok(())
        });
    })
}

/// Register the org.freedesktop.login1.Session interface
fn register_session_iface(cr: &mut Crossroads, mgr: &SharedManager) -> IfaceToken<SharedManager> {
    let mgr_for_iface = mgr.clone();

    cr.register(
        DBUS_SESSION_IFACE,
        move |b: &mut IfaceBuilder<SharedManager>| {
            // Signals — register and drop builders immediately
            b.signal::<(), _>("Lock", ());
            b.signal::<(), _>("Unlock", ());
            b.signal::<(u32, u32, String), _>("PauseDevice", ("major", "minor", "type_"));
            b.signal::<(u32, u32, u32), _>("ResumeDevice", ("major", "minor", "fd"));

            // Properties
            {
                let m = mgr_for_iface.clone();
                b.property("Id").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(sid) = session_id_from_path(&path_str)
                        && let Some(session) = mgr.sessions.get(&sid)
                    {
                        return Ok(session.id.clone());
                    }
                    Ok(String::new())
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("User").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(sid) = session_id_from_path(&path_str)
                        && let Some(session) = mgr.sessions.get(&sid)
                    {
                        return Ok((session.uid, user_object_path(session.uid)));
                    }
                    Ok((0u32, user_object_path(0)))
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Name").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(sid) = session_id_from_path(&path_str)
                        && let Some(session) = mgr.sessions.get(&sid)
                    {
                        return Ok(session.user.clone());
                    }
                    Ok(String::new())
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Timestamp")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(sid) = session_id_from_path(&path_str)
                            && let Some(session) = mgr.sessions.get(&sid)
                        {
                            return Ok(session.since * 1_000_000);
                        }
                        Ok(0u64)
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("TimestampMonotonic")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(sid) = session_id_from_path(&path_str)
                            && let Some(session) = mgr.sessions.get(&sid)
                        {
                            return Ok(session.since_monotonic);
                        }
                        Ok(0u64)
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("VTNr").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(sid) = session_id_from_path(&path_str)
                        && let Some(session) = mgr.sessions.get(&sid)
                    {
                        return Ok(session.vtnr);
                    }
                    Ok(0u32)
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Seat").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(sid) = session_id_from_path(&path_str)
                        && let Some(session) = mgr.sessions.get(&sid)
                    {
                        let seat_id = session.seat.clone().unwrap_or_default();
                        let seat_path = if seat_id.is_empty() {
                            dbus::Path::new("/").expect("valid path")
                        } else {
                            seat_object_path(&seat_id)
                        };
                        return Ok((seat_id, seat_path));
                    }
                    Ok((String::new(), dbus::Path::new("/").expect("valid path")))
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("TTY").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(sid) = session_id_from_path(&path_str)
                        && let Some(session) = mgr.sessions.get(&sid)
                    {
                        return Ok(session.tty.clone());
                    }
                    Ok(String::new())
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Display")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(sid) = session_id_from_path(&path_str)
                            && let Some(session) = mgr.sessions.get(&sid)
                        {
                            return Ok(session.display.clone());
                        }
                        Ok(String::new())
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Remote").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(sid) = session_id_from_path(&path_str)
                        && let Some(session) = mgr.sessions.get(&sid)
                    {
                        return Ok(session.remote);
                    }
                    Ok(false)
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("RemoteHost")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(sid) = session_id_from_path(&path_str)
                            && let Some(session) = mgr.sessions.get(&sid)
                        {
                            return Ok(session.remote_host.clone());
                        }
                        Ok(String::new())
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("RemoteUser")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(sid) = session_id_from_path(&path_str)
                            && let Some(session) = mgr.sessions.get(&sid)
                        {
                            return Ok(session.remote_user.clone());
                        }
                        Ok(String::new())
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Service")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(sid) = session_id_from_path(&path_str)
                            && let Some(session) = mgr.sessions.get(&sid)
                        {
                            return Ok(session.service.clone());
                        }
                        Ok(String::new())
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Desktop")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(sid) = session_id_from_path(&path_str)
                            && let Some(session) = mgr.sessions.get(&sid)
                        {
                            return Ok(session.desktop.clone());
                        }
                        Ok(String::new())
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Scope").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(sid) = session_id_from_path(&path_str)
                        && let Some(session) = mgr.sessions.get(&sid)
                    {
                        return Ok(session.scope.clone());
                    }
                    Ok(String::new())
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Leader").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(sid) = session_id_from_path(&path_str)
                        && let Some(session) = mgr.sessions.get(&sid)
                    {
                        return Ok(session.leader);
                    }
                    Ok(0u32)
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Audit").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(sid) = session_id_from_path(&path_str)
                        && let Some(session) = mgr.sessions.get(&sid)
                    {
                        return Ok(session.leader); // audit session = leader PID for now
                    }
                    Ok(0u32)
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Type").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(sid) = session_id_from_path(&path_str)
                        && let Some(session) = mgr.sessions.get(&sid)
                    {
                        return Ok(session.session_type.clone());
                    }
                    Ok(String::new())
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Class").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(sid) = session_id_from_path(&path_str)
                        && let Some(session) = mgr.sessions.get(&sid)
                    {
                        return Ok(session.class.clone());
                    }
                    Ok(String::new())
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Active").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(sid) = session_id_from_path(&path_str)
                        && let Some(session) = mgr.sessions.get(&sid)
                    {
                        return Ok(session.active);
                    }
                    Ok(false)
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("State").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(sid) = session_id_from_path(&path_str)
                        && let Some(session) = mgr.sessions.get(&sid)
                    {
                        return Ok(session.state.clone());
                    }
                    Ok(String::new())
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("IdleHint")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(sid) = session_id_from_path(&path_str)
                            && let Some(session) = mgr.sessions.get(&sid)
                        {
                            return Ok(session.idle_hint);
                        }
                        Ok(false)
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("IdleSinceHint")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(sid) = session_id_from_path(&path_str)
                            && let Some(session) = mgr.sessions.get(&sid)
                        {
                            return Ok(session.idle_since_hint);
                        }
                        Ok(0u64)
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("IdleSinceHintMonotonic")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(sid) = session_id_from_path(&path_str)
                            && let Some(session) = mgr.sessions.get(&sid)
                        {
                            return Ok(session.idle_since_hint_monotonic);
                        }
                        Ok(0u64)
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("LockedHint")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(sid) = session_id_from_path(&path_str)
                            && let Some(session) = mgr.sessions.get(&sid)
                        {
                            return Ok(session.locked_hint);
                        }
                        Ok(false)
                    });
            }

            // Methods

            // Terminate()
            {
                let m = mgr_for_iface.clone();
                b.method(
                    "Terminate",
                    (),
                    (),
                    move |ctx, _: &mut SharedManager, ()| {
                        let path_str = ctx.path().to_string();
                        let sid = session_id_from_path(&path_str)
                            .ok_or_else(|| MethodErr::failed("Invalid session path"))?;
                        let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        if mgr.release_session(&sid) {
                            mgr.sync_runtime_state();
                            Ok(())
                        } else {
                            Err(MethodErr::failed("Session not found"))
                        }
                    },
                );
            }

            // Activate()
            {
                let m = mgr_for_iface.clone();
                b.method("Activate", (), (), move |ctx, _: &mut SharedManager, ()| {
                    let path_str = ctx.path().to_string();
                    let sid = session_id_from_path(&path_str)
                        .ok_or_else(|| MethodErr::failed("Invalid session path"))?;
                    let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    mgr.activate_session(&sid)
                        .map_err(|e| MethodErr::failed(&e))?;
                    mgr.sync_runtime_state();
                    Ok(())
                });
            }

            // Lock()
            {
                let m = mgr_for_iface.clone();
                b.method("Lock", (), (), move |ctx, _: &mut SharedManager, ()| {
                    let path_str = ctx.path().to_string();
                    let sid = session_id_from_path(&path_str)
                        .ok_or_else(|| MethodErr::failed("Invalid session path"))?;
                    let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    mgr.lock_session(&sid).map_err(|e| MethodErr::failed(&e))
                });
            }

            // Unlock()
            {
                let m = mgr_for_iface.clone();
                b.method("Unlock", (), (), move |ctx, _: &mut SharedManager, ()| {
                    let path_str = ctx.path().to_string();
                    let sid = session_id_from_path(&path_str)
                        .ok_or_else(|| MethodErr::failed("Invalid session path"))?;
                    let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    mgr.unlock_session(&sid).map_err(|e| MethodErr::failed(&e))
                });
            }

            // SetIdleHint(b)
            {
                let m = mgr_for_iface.clone();
                b.method(
                    "SetIdleHint",
                    ("idle",),
                    (),
                    move |ctx, _: &mut SharedManager, (idle,): (bool,)| {
                        let path_str = ctx.path().to_string();
                        let sid = session_id_from_path(&path_str)
                            .ok_or_else(|| MethodErr::failed("Invalid session path"))?;
                        let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        mgr.set_idle_hint(&sid, idle)
                            .map_err(|e| MethodErr::failed(&e))
                    },
                );
            }

            // SetLockedHint(b)
            {
                let m = mgr_for_iface.clone();
                b.method(
                    "SetLockedHint",
                    ("locked",),
                    (),
                    move |ctx, _: &mut SharedManager, (locked,): (bool,)| {
                        let path_str = ctx.path().to_string();
                        let sid = session_id_from_path(&path_str)
                            .ok_or_else(|| MethodErr::failed("Invalid session path"))?;
                        let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        mgr.set_locked_hint(&sid, locked)
                            .map_err(|e| MethodErr::failed(&e))
                    },
                );
            }

            // SetType(s)
            {
                let m = mgr_for_iface.clone();
                b.method(
                    "SetType",
                    ("session_type",),
                    (),
                    move |ctx, _: &mut SharedManager, (stype,): (String,)| {
                        let path_str = ctx.path().to_string();
                        let sid = session_id_from_path(&path_str)
                            .ok_or_else(|| MethodErr::failed("Invalid session path"))?;
                        let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        mgr.set_session_type(&sid, &stype)
                            .map_err(|e| MethodErr::failed(&e))
                    },
                );
            }

            // Kill(si)
            {
                let m = mgr_for_iface.clone();
                b.method(
                    "Kill",
                    ("who", "signal_number"),
                    (),
                    move |ctx, _: &mut SharedManager, (who, signal): (String, i32)| {
                        let path_str = ctx.path().to_string();
                        let sid = session_id_from_path(&path_str)
                            .ok_or_else(|| MethodErr::failed("Invalid session path"))?;
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        mgr.kill_session(&sid, &who, signal)
                            .map_err(|e| MethodErr::failed(&e))
                    },
                );
            }

            // TakeControl(b) — stub
            b.method(
                "TakeControl",
                ("force",),
                (),
                |_, _: &mut SharedManager, (_force,): (bool,)| Ok(()),
            );

            // ReleaseControl() — stub
            b.method("ReleaseControl", (), (), |_, _: &mut SharedManager, ()| {
                Ok(())
            });

            // SetBrightness(ssu) — stub
            b.method(
                "SetBrightness",
                ("subsystem", "name", "brightness"),
                (),
                |_, _: &mut SharedManager, (_sub, _name, _val): (String, String, u32)| Ok(()),
            );

            // TakeDevice(uu) -> hb — stub
            b.method(
                "TakeDevice",
                ("major", "minor"),
                ("fd", "inactive"),
                |_,
                 _: &mut SharedManager,
                 (_major, _minor): (u32, u32)|
                 -> Result<(i32, bool), MethodErr> {
                    Err(MethodErr::failed("TakeDevice not yet implemented"))
                },
            );

            // ReleaseDevice(uu) — stub
            b.method(
                "ReleaseDevice",
                ("major", "minor"),
                (),
                |_, _: &mut SharedManager, (_major, _minor): (u32, u32)| Ok(()),
            );

            // PauseDeviceComplete(uu) — stub
            b.method(
                "PauseDeviceComplete",
                ("major", "minor"),
                (),
                |_, _: &mut SharedManager, (_major, _minor): (u32, u32)| Ok(()),
            );
        },
    )
}

/// Register the org.freedesktop.login1.Seat interface
fn register_seat_iface(cr: &mut Crossroads, mgr: &SharedManager) -> IfaceToken<SharedManager> {
    let mgr_for_iface = mgr.clone();

    cr.register(
        DBUS_SEAT_IFACE,
        move |b: &mut IfaceBuilder<SharedManager>| {
            // Properties
            {
                b.property("Id").get(move |ctx, _: &mut SharedManager| {
                    let path_str = ctx.path().to_string();
                    if let Some(seat_id) = seat_id_from_path(&path_str) {
                        Ok(seat_id)
                    } else {
                        Ok(String::new())
                    }
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("ActiveSession")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(seat_id) = seat_id_from_path(&path_str)
                            && let Some(seat) = mgr.seats.get(&seat_id)
                            && let Some(ref active) = seat.active_session
                        {
                            return Ok((active.clone(), session_object_path(active)));
                        }
                        Ok((String::new(), dbus::Path::new("/").expect("valid path")))
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("CanGraphical")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(seat_id) = seat_id_from_path(&path_str)
                            && let Some(seat) = mgr.seats.get(&seat_id)
                        {
                            return Ok(seat.can_graphical);
                        }
                        Ok(false)
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("CanMultiSession")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(seat_id) = seat_id_from_path(&path_str)
                            && let Some(seat) = mgr.seats.get(&seat_id)
                        {
                            return Ok(seat.can_multi_session);
                        }
                        Ok(false)
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Sessions")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(seat_id) = seat_id_from_path(&path_str)
                            && let Some(seat) = mgr.seats.get(&seat_id)
                        {
                            let sessions: Vec<(String, dbus::Path<'static>)> = seat
                                .sessions
                                .iter()
                                .map(|sid| (sid.clone(), session_object_path(sid)))
                                .collect();
                            return Ok(sessions);
                        }
                        Ok(Vec::new())
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("IdleHint")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(seat_id) = seat_id_from_path(&path_str)
                            && let Some(seat) = mgr.seats.get(&seat_id)
                        {
                            // Seat is idle if all sessions on it are idle
                            let all_idle = seat.sessions.iter().all(|sid| {
                                mgr.sessions.get(sid).map(|s| s.idle_hint).unwrap_or(true)
                            });
                            return Ok(all_idle);
                        }
                        Ok(true)
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("IdleSinceHint")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(seat_id) = seat_id_from_path(&path_str)
                            && let Some(seat) = mgr.seats.get(&seat_id)
                        {
                            let mut earliest: u64 = 0;
                            for sid in &seat.sessions {
                                if let Some(s) = mgr.sessions.get(sid)
                                    && s.idle_hint
                                    && s.idle_since_hint > 0
                                    && (earliest == 0 || s.idle_since_hint < earliest)
                                {
                                    earliest = s.idle_since_hint;
                                }
                            }
                            return Ok(earliest);
                        }
                        Ok(0u64)
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("IdleSinceHintMonotonic")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(seat_id) = seat_id_from_path(&path_str)
                            && let Some(seat) = mgr.seats.get(&seat_id)
                        {
                            let mut earliest: u64 = 0;
                            for sid in &seat.sessions {
                                if let Some(s) = mgr.sessions.get(sid)
                                    && s.idle_hint
                                    && s.idle_since_hint_monotonic > 0
                                    && (earliest == 0 || s.idle_since_hint_monotonic < earliest)
                                {
                                    earliest = s.idle_since_hint_monotonic;
                                }
                            }
                            return Ok(earliest);
                        }
                        Ok(0u64)
                    });
            }

            // Methods

            // Terminate()
            {
                let m = mgr_for_iface.clone();
                b.method(
                    "Terminate",
                    (),
                    (),
                    move |ctx, _: &mut SharedManager, ()| {
                        let path_str = ctx.path().to_string();
                        let seat_id = seat_id_from_path(&path_str)
                            .ok_or_else(|| MethodErr::failed("Invalid seat path"))?;
                        let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        mgr.terminate_seat(&seat_id)
                            .map_err(|e| MethodErr::failed(&e))?;
                        mgr.sync_runtime_state();
                        Ok(())
                    },
                );
            }

            // ActivateSession(s)
            {
                let m = mgr_for_iface.clone();
                b.method(
                    "ActivateSession",
                    ("session_id",),
                    (),
                    move |ctx, _: &mut SharedManager, (session_id,): (String,)| {
                        let path_str = ctx.path().to_string();
                        let seat_id = seat_id_from_path(&path_str)
                            .ok_or_else(|| MethodErr::failed("Invalid seat path"))?;
                        let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        // Verify session is on this seat
                        if let Some(session) = mgr.sessions.get(&session_id)
                            && session.seat.as_deref() != Some(&seat_id)
                        {
                            return Err(MethodErr::failed("Session not on this seat"));
                        }
                        mgr.activate_session(&session_id)
                            .map_err(|e| MethodErr::failed(&e))?;
                        mgr.sync_runtime_state();
                        Ok(())
                    },
                );
            }

            // SwitchTo(u) — VT number
            b.method(
                "SwitchTo",
                ("vtnr",),
                (),
                |_, _: &mut SharedManager, (vtnr,): (u32,)| {
                    // Switch to VT via ioctl
                    log::info!("SwitchTo VT {} requested", vtnr);
                    switch_vt(vtnr);
                    Ok(())
                },
            );

            // SwitchToNext()
            b.method("SwitchToNext", (), (), |_, _: &mut SharedManager, ()| {
                log::info!("SwitchToNext requested");
                Ok(())
            });

            // SwitchToPrevious()
            b.method(
                "SwitchToPrevious",
                (),
                (),
                |_, _: &mut SharedManager, ()| {
                    log::info!("SwitchToPrevious requested");
                    Ok(())
                },
            );
        },
    )
}

/// Register the org.freedesktop.login1.User interface
fn register_user_iface(cr: &mut Crossroads, mgr: &SharedManager) -> IfaceToken<SharedManager> {
    let mgr_for_iface = mgr.clone();

    cr.register(
        DBUS_USER_IFACE,
        move |b: &mut IfaceBuilder<SharedManager>| {
            // Properties
            {
                b.property("UID").get(move |ctx, _: &mut SharedManager| {
                    let path_str = ctx.path().to_string();
                    Ok(uid_from_path(&path_str).unwrap_or(0))
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("GID").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(uid) = uid_from_path(&path_str)
                        && let Some(user) = mgr.users.get(&uid)
                    {
                        return Ok(user.gid);
                    }
                    Ok(0u32)
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Name").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(uid) = uid_from_path(&path_str)
                        && let Some(user) = mgr.users.get(&uid)
                    {
                        return Ok(user.name.clone());
                    }
                    Ok(String::new())
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Timestamp")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(uid) = uid_from_path(&path_str)
                            && let Some(user) = mgr.users.get(&uid)
                        {
                            return Ok(user.since * 1_000_000);
                        }
                        Ok(0u64)
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("TimestampMonotonic")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(uid) = uid_from_path(&path_str)
                            && let Some(user) = mgr.users.get(&uid)
                        {
                            return Ok(user.since_monotonic);
                        }
                        Ok(0u64)
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("RuntimePath")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(uid) = uid_from_path(&path_str)
                            && let Some(user) = mgr.users.get(&uid)
                        {
                            return Ok(user.runtime_path.clone());
                        }
                        Ok(String::new())
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Service")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(uid) = uid_from_path(&path_str)
                            && let Some(user) = mgr.users.get(&uid)
                        {
                            return Ok(user.service.clone());
                        }
                        Ok(String::new())
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Slice").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(uid) = uid_from_path(&path_str)
                        && let Some(user) = mgr.users.get(&uid)
                    {
                        return Ok(user.slice.clone());
                    }
                    Ok(String::new())
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Display")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(uid) = uid_from_path(&path_str)
                            && let Some(user) = mgr.users.get(&uid)
                        {
                            // Display session = first session
                            let display_session =
                                user.sessions.first().cloned().unwrap_or_default();
                            let display_path = if display_session.is_empty() {
                                dbus::Path::new("/").expect("valid path")
                            } else {
                                session_object_path(&display_session)
                            };
                            return Ok((display_session, display_path));
                        }
                        Ok((String::new(), dbus::Path::new("/").expect("valid path")))
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("State").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(uid) = uid_from_path(&path_str)
                        && let Some(user) = mgr.users.get(&uid)
                    {
                        return Ok(user.state.clone());
                    }
                    Ok(String::new())
                });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Sessions")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(uid) = uid_from_path(&path_str)
                            && let Some(user) = mgr.users.get(&uid)
                        {
                            let sessions: Vec<(String, dbus::Path<'static>)> = user
                                .sessions
                                .iter()
                                .map(|sid| (sid.clone(), session_object_path(sid)))
                                .collect();
                            return Ok(sessions);
                        }
                        Ok(Vec::new())
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("IdleHint")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(uid) = uid_from_path(&path_str)
                            && let Some(user) = mgr.users.get(&uid)
                        {
                            let all_idle = user.sessions.iter().all(|sid| {
                                mgr.sessions.get(sid).map(|s| s.idle_hint).unwrap_or(true)
                            });
                            return Ok(all_idle);
                        }
                        Ok(true)
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("IdleSinceHint")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(uid) = uid_from_path(&path_str)
                            && let Some(user) = mgr.users.get(&uid)
                        {
                            let mut earliest: u64 = 0;
                            for sid in &user.sessions {
                                if let Some(s) = mgr.sessions.get(sid)
                                    && s.idle_hint
                                    && s.idle_since_hint > 0
                                    && (earliest == 0 || s.idle_since_hint < earliest)
                                {
                                    earliest = s.idle_since_hint;
                                }
                            }
                            return Ok(earliest);
                        }
                        Ok(0u64)
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("IdleSinceHintMonotonic")
                    .get(move |ctx, _: &mut SharedManager| {
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        let path_str = ctx.path().to_string();
                        if let Some(uid) = uid_from_path(&path_str)
                            && let Some(user) = mgr.users.get(&uid)
                        {
                            let mut earliest: u64 = 0;
                            for sid in &user.sessions {
                                if let Some(s) = mgr.sessions.get(sid)
                                    && s.idle_hint
                                    && s.idle_since_hint_monotonic > 0
                                    && (earliest == 0 || s.idle_since_hint_monotonic < earliest)
                                {
                                    earliest = s.idle_since_hint_monotonic;
                                }
                            }
                            return Ok(earliest);
                        }
                        Ok(0u64)
                    });
            }
            {
                let m = mgr_for_iface.clone();
                b.property("Linger").get(move |ctx, _: &mut SharedManager| {
                    let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                    let path_str = ctx.path().to_string();
                    if let Some(uid) = uid_from_path(&path_str)
                        && let Some(user) = mgr.users.get(&uid)
                    {
                        return Ok(user.linger);
                    }
                    Ok(false)
                });
            }

            // Methods

            // Terminate()
            {
                let m = mgr_for_iface.clone();
                b.method(
                    "Terminate",
                    (),
                    (),
                    move |ctx, _: &mut SharedManager, ()| {
                        let path_str = ctx.path().to_string();
                        let uid = uid_from_path(&path_str)
                            .ok_or_else(|| MethodErr::failed("Invalid user path"))?;
                        let mut mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        mgr.terminate_user(uid).map_err(|e| MethodErr::failed(&e))?;
                        mgr.sync_runtime_state();
                        Ok(())
                    },
                );
            }

            // Kill(i)
            {
                let m = mgr_for_iface.clone();
                b.method(
                    "Kill",
                    ("signal_number",),
                    (),
                    move |ctx, _: &mut SharedManager, (signal,): (i32,)| {
                        let path_str = ctx.path().to_string();
                        let uid = uid_from_path(&path_str)
                            .ok_or_else(|| MethodErr::failed("Invalid user path"))?;
                        let mgr = m.lock().unwrap_or_else(|e| e.into_inner());
                        mgr.kill_user(uid, signal)
                            .map_err(|e| MethodErr::failed(&e))
                    },
                );
            }
        },
    )
}

/// Switch VT using ioctl
fn switch_vt(vtnr: u32) {
    if vtnr == 0 {
        return;
    }
    // Try /dev/tty0 for VT switching
    let fd = unsafe { libc::open(c"/dev/tty0".as_ptr(), libc::O_RDWR | libc::O_NOCTTY) };
    if fd >= 0 {
        unsafe {
            // VT_ACTIVATE = 0x5606
            libc::ioctl(fd, 0x5606, vtnr as libc::c_ulong);
            // VT_WAITACTIVE = 0x5607
            libc::ioctl(fd, 0x5607, vtnr as libc::c_ulong);
            libc::close(fd);
        }
    }
}

/// Check AC power state
fn check_ac_power() -> bool {
    let ps_dir = Path::new("/sys/class/power_supply");
    if let Ok(entries) = fs::read_dir(ps_dir) {
        for entry in entries.flatten() {
            let type_path = entry.path().join("type");
            if let Ok(ps_type) = fs::read_to_string(&type_path)
                && ps_type.trim() == "Mains"
            {
                let online_path = entry.path().join("online");
                if let Ok(online) = fs::read_to_string(&online_path)
                    && online.trim() == "1"
                {
                    return true;
                }
            }
        }
    }
    true // Default to AC power if we can't determine
}

/// Resolve a UID to a username
fn resolve_uid_to_name(uid: u32) -> String {
    if let Ok(content) = fs::read_to_string("/etc/passwd") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 3
                && let Ok(file_uid) = parts[2].parse::<u32>()
                && file_uid == uid
            {
                return parts[0].to_string();
            }
        }
    }
    format!("{}", uid)
}

// ---------------------------------------------------------------------------
// D-Bus signal emission helpers
// ---------------------------------------------------------------------------

fn emit_session_new(conn: &Connection, session_id: &str) {
    let path = session_object_path(session_id);
    let msg = dbus::message::Message::signal(
        &dbus::Path::new(DBUS_PATH).expect("valid path"),
        &dbus::strings::Interface::new(DBUS_MANAGER_IFACE).expect("valid iface"),
        &dbus::strings::Member::new("SessionNew").expect("valid member"),
    )
    .append2(session_id.to_string(), path);
    let _ = conn.channel().send(msg);
}

fn emit_session_removed(conn: &Connection, session_id: &str) {
    let path = session_object_path(session_id);
    let msg = dbus::message::Message::signal(
        &dbus::Path::new(DBUS_PATH).expect("valid path"),
        &dbus::strings::Interface::new(DBUS_MANAGER_IFACE).expect("valid iface"),
        &dbus::strings::Member::new("SessionRemoved").expect("valid member"),
    )
    .append2(session_id.to_string(), path);
    let _ = conn.channel().send(msg);
}

fn emit_user_new(conn: &Connection, uid: u32) {
    let path = user_object_path(uid);
    let msg = dbus::message::Message::signal(
        &dbus::Path::new(DBUS_PATH).expect("valid path"),
        &dbus::strings::Interface::new(DBUS_MANAGER_IFACE).expect("valid iface"),
        &dbus::strings::Member::new("UserNew").expect("valid member"),
    )
    .append2(uid, path);
    let _ = conn.channel().send(msg);
}

fn emit_user_removed(conn: &Connection, uid: u32) {
    let path = user_object_path(uid);
    let msg = dbus::message::Message::signal(
        &dbus::Path::new(DBUS_PATH).expect("valid path"),
        &dbus::strings::Interface::new(DBUS_MANAGER_IFACE).expect("valid iface"),
        &dbus::strings::Member::new("UserRemoved").expect("valid member"),
    )
    .append2(uid, path);
    let _ = conn.channel().send(msg);
}

fn emit_seat_new(conn: &Connection, seat_id: &str) {
    let path = seat_object_path(seat_id);
    let msg = dbus::message::Message::signal(
        &dbus::Path::new(DBUS_PATH).expect("valid path"),
        &dbus::strings::Interface::new(DBUS_MANAGER_IFACE).expect("valid iface"),
        &dbus::strings::Member::new("SeatNew").expect("valid member"),
    )
    .append2(seat_id.to_string(), path);
    let _ = conn.channel().send(msg);
}

#[allow(dead_code)]
fn emit_seat_removed(conn: &Connection, seat_id: &str) {
    let path = seat_object_path(seat_id);
    let msg = dbus::message::Message::signal(
        &dbus::Path::new(DBUS_PATH).expect("valid path"),
        &dbus::strings::Interface::new(DBUS_MANAGER_IFACE).expect("valid iface"),
        &dbus::strings::Member::new("SeatRemoved").expect("valid member"),
    )
    .append2(seat_id.to_string(), path);
    let _ = conn.channel().send(msg);
}

fn emit_prepare_for_shutdown(conn: &Connection, active: bool) {
    let msg = dbus::message::Message::signal(
        &dbus::Path::new(DBUS_PATH).expect("valid path"),
        &dbus::strings::Interface::new(DBUS_MANAGER_IFACE).expect("valid iface"),
        &dbus::strings::Member::new("PrepareForShutdown").expect("valid member"),
    )
    .append1(active);
    let _ = conn.channel().send(msg);
}

#[allow(dead_code)]
fn emit_prepare_for_sleep(conn: &Connection, active: bool) {
    let msg = dbus::message::Message::signal(
        &dbus::Path::new(DBUS_PATH).expect("valid path"),
        &dbus::strings::Interface::new(DBUS_MANAGER_IFACE).expect("valid iface"),
        &dbus::strings::Member::new("PrepareForSleep").expect("valid member"),
    )
    .append1(active);
    let _ = conn.channel().send(msg);
}

#[allow(dead_code)]
fn emit_session_lock(conn: &Connection, session_id: &str) {
    let path = session_object_path(session_id);
    let msg = dbus::message::Message::signal(
        &path,
        &dbus::strings::Interface::new(DBUS_SESSION_IFACE).expect("valid iface"),
        &dbus::strings::Member::new("Lock").expect("valid member"),
    );
    let _ = conn.channel().send(msg);
}

#[allow(dead_code)]
fn emit_session_unlock(conn: &Connection, session_id: &str) {
    let path = session_object_path(session_id);
    let msg = dbus::message::Message::signal(
        &path,
        &dbus::strings::Interface::new(DBUS_SESSION_IFACE).expect("valid iface"),
        &dbus::strings::Member::new("Unlock").expect("valid member"),
    );
    let _ = conn.channel().send(msg);
}

// ---------------------------------------------------------------------------
// Control socket handler (legacy, for loginctl)
// ---------------------------------------------------------------------------

fn handle_control_command(mgr: &mut LoginManager, cmd: &str) -> String {
    let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();
    let command = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();
    let args = parts.get(1).copied().unwrap_or("");

    match command.as_str() {
        "status" => mgr.format_status(),

        "list-sessions" => {
            let sessions: Vec<&Session> = mgr.sessions.values().collect();
            match serde_json::to_string_pretty(&sessions) {
                Ok(json) => json,
                Err(e) => format!("ERROR: {}", e),
            }
        }

        "list-seats" => {
            let seats: Vec<&Seat> = mgr.seats.values().collect();
            match serde_json::to_string_pretty(&seats) {
                Ok(json) => json,
                Err(e) => format!("ERROR: {}", e),
            }
        }

        "list-users" => {
            let users: Vec<&User> = mgr.users.values().collect();
            match serde_json::to_string_pretty(&users) {
                Ok(json) => json,
                Err(e) => format!("ERROR: {}", e),
            }
        }

        "list-inhibitors" => {
            let inhibitors: Vec<&Inhibitor> = mgr.inhibitors.values().collect();
            match serde_json::to_string_pretty(&inhibitors) {
                Ok(json) => json,
                Err(e) => format!("ERROR: {}", e),
            }
        }

        "show-session" => {
            if let Some(session) = mgr.sessions.get(args) {
                match serde_json::to_string_pretty(session) {
                    Ok(json) => json,
                    Err(e) => format!("ERROR: {}", e),
                }
            } else {
                format!("ERROR: Session '{}' not found", args)
            }
        }

        "show-seat" => {
            if let Some(seat) = mgr.seats.get(args) {
                match serde_json::to_string_pretty(seat) {
                    Ok(json) => json,
                    Err(e) => format!("ERROR: {}", e),
                }
            } else {
                format!("ERROR: Seat '{}' not found", args)
            }
        }

        "show-user" => {
            if let Ok(uid) = args.parse::<u32>() {
                if let Some(user) = mgr.users.get(&uid) {
                    match serde_json::to_string_pretty(user) {
                        Ok(json) => json,
                        Err(e) => format!("ERROR: {}", e),
                    }
                } else {
                    format!("ERROR: User {} not found", uid)
                }
            } else {
                // Try to find by name
                if let Some(user) = mgr.users.values().find(|u| u.name == args) {
                    match serde_json::to_string_pretty(user) {
                        Ok(json) => json,
                        Err(e) => format!("ERROR: {}", e),
                    }
                } else {
                    format!("ERROR: User '{}' not found", args)
                }
            }
        }

        "create-session" => {
            // JSON args: {"uid": N, "user": "...", "seat": "seat0", "vtnr": N, "type": "tty", "class": "user", "tty": "/dev/ttyN", "leader": PID}
            match serde_json::from_str::<serde_json::Value>(args) {
                Ok(v) => {
                    let uid = v["uid"].as_u64().unwrap_or(0) as u32;
                    let user = v["user"].as_str().unwrap_or("unknown");
                    let seat = v["seat"].as_str();
                    let vtnr = v["vtnr"].as_u64().unwrap_or(0) as u32;
                    let stype = v["type"].as_str().unwrap_or("tty");
                    let class = v["class"].as_str().unwrap_or("user");
                    let tty = v["tty"].as_str().unwrap_or("");
                    let leader = v["leader"].as_u64().unwrap_or(0) as u32;

                    let id = mgr.create_session(uid, user, seat, vtnr, stype, class, tty, leader);
                    mgr.sync_runtime_state();
                    log::info!(
                        "New session {} of user {} on {}",
                        id,
                        user,
                        seat.unwrap_or("(no seat)")
                    );
                    format!("OK {}", id)
                }
                Err(e) => format!("ERROR: Invalid JSON: {}", e),
            }
        }

        "release-session" => {
            if mgr.release_session(args) {
                mgr.sync_runtime_state();
                log::info!("Released session {}", args);
                "OK".to_string()
            } else {
                format!("ERROR: Session '{}' not found", args)
            }
        }

        "activate-session" => match mgr.activate_session(args) {
            Ok(()) => {
                mgr.sync_runtime_state();
                log::info!("Activated session {}", args);
                "OK".to_string()
            }
            Err(e) => format!("ERROR: {}", e),
        },

        "lock-session" => match mgr.lock_session(args) {
            Ok(()) => {
                log::info!("Locked session {}", args);
                "OK".to_string()
            }
            Err(e) => format!("ERROR: {}", e),
        },

        "unlock-session" => match mgr.unlock_session(args) {
            Ok(()) => {
                log::info!("Unlocked session {}", args);
                "OK".to_string()
            }
            Err(e) => format!("ERROR: {}", e),
        },

        "lock-sessions" => {
            mgr.lock_sessions();
            log::info!("Locked all sessions");
            "OK".to_string()
        }

        "unlock-sessions" => {
            mgr.unlock_sessions();
            log::info!("Unlocked all sessions");
            "OK".to_string()
        }

        "terminate-session" => {
            if mgr.release_session(args) {
                mgr.sync_runtime_state();
                log::info!("Terminated session {}", args);
                "OK".to_string()
            } else {
                format!("ERROR: Session '{}' not found", args)
            }
        }

        "terminate-user" => {
            if let Ok(uid) = args.parse::<u32>() {
                match mgr.terminate_user(uid) {
                    Ok(()) => {
                        mgr.sync_runtime_state();
                        log::info!("Terminated user {}", uid);
                        "OK".to_string()
                    }
                    Err(e) => format!("ERROR: {}", e),
                }
            } else {
                format!("ERROR: Invalid UID '{}'", args)
            }
        }

        "inhibit" => {
            // JSON: {"what": "...", "who": "...", "why": "...", "mode": "block", "uid": N, "pid": N}
            match serde_json::from_str::<serde_json::Value>(args) {
                Ok(v) => {
                    let what = v["what"].as_str().unwrap_or("shutdown");
                    let who = v["who"].as_str().unwrap_or("unknown");
                    let why = v["why"].as_str().unwrap_or("");
                    let mode = v["mode"].as_str().unwrap_or("block");
                    let uid = v["uid"].as_u64().unwrap_or(0) as u32;
                    let pid = v["pid"].as_u64().unwrap_or(0) as u32;
                    let id = mgr.create_inhibitor(what, who, why, mode, uid, pid);
                    log::info!("New inhibitor {} ({}): {} — {}", id, what, who, why);
                    format!("OK {}", id)
                }
                Err(e) => format!("ERROR: Invalid JSON: {}", e),
            }
        }

        "release-inhibitor" => {
            if let Ok(id) = args.parse::<u64>() {
                if mgr.release_inhibitor(id) {
                    log::info!("Released inhibitor {}", id);
                    "OK".to_string()
                } else {
                    format!("ERROR: Inhibitor {} not found", id)
                }
            } else {
                format!("ERROR: Invalid inhibitor ID '{}'", args)
            }
        }

        "can-poweroff"
        | "can-reboot"
        | "can-suspend"
        | "can-hibernate"
        | "can-suspend-then-hibernate"
        | "can-hybrid-sleep" => {
            let action = command.strip_prefix("can-").unwrap_or("shutdown");
            mgr.can_action(action).to_string()
        }

        "poweroff" | "reboot" | "suspend" | "hibernate" => {
            log::info!("Requested system action: {}", command);
            format!("OK (action {} requested)", command)
        }

        "flush-devices" => {
            mgr.enumerate_input_devices();
            log::info!(
                "Flushed devices, found {} power button device(s)",
                mgr.power_button_devices.len()
            );
            "OK".to_string()
        }

        _ => format!("ERROR: Unknown command: {}", command),
    }
}

fn handle_client(mgr: &mut LoginManager, stream: &mut std::os::unix::net::UnixStream) {
    let mut buf = [0u8; 8192];
    match stream.read(&mut buf) {
        Ok(n) if n > 0 => {
            let cmd = String::from_utf8_lossy(&buf[..n]);
            let response = handle_control_command(mgr, &cmd);
            let _ = stream.write_all(response.as_bytes());
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Create runtime directories
// ---------------------------------------------------------------------------

fn ensure_runtime_dirs() {
    for dir in &[RUN_DIR, SESSIONS_DIR, SEATS_DIR, USERS_DIR, INHIBIT_DIR] {
        let _ = fs::create_dir_all(dir);
    }
}

// ---------------------------------------------------------------------------
// D-Bus server setup and management
// ---------------------------------------------------------------------------

struct DbusServer {
    conn: Connection,
    cr: Crossroads,
    mgr: SharedManager,
    session_iface: IfaceToken<SharedManager>,
    #[allow(dead_code)]
    seat_iface: IfaceToken<SharedManager>,
    user_iface: IfaceToken<SharedManager>,
}

impl DbusServer {
    fn new(mgr: SharedManager) -> Result<Self, Box<dyn std::error::Error>> {
        let conn = Connection::new_system()?;
        conn.request_name(DBUS_NAME, false, true, false)?;

        let mut cr = Crossroads::new();

        // Allow introspection
        cr.set_async_support(None);

        let manager_iface = register_manager_iface(&mut cr, &mgr);
        let session_iface = register_session_iface(&mut cr, &mgr);
        let seat_iface = register_seat_iface(&mut cr, &mgr);
        let user_iface = register_user_iface(&mut cr, &mgr);

        // Register the manager object
        cr.insert(DBUS_PATH, &[manager_iface], mgr.clone());

        // Register existing seat objects
        {
            let mgr_guard = mgr.lock().unwrap_or_else(|e| e.into_inner());
            for seat_id in mgr_guard.seats.keys() {
                let path = seat_object_path(seat_id);
                cr.insert(path, &[seat_iface], mgr.clone());
            }
        }

        Ok(DbusServer {
            conn,
            cr,
            mgr,
            session_iface,
            seat_iface,
            user_iface,
        })
    }

    /// Register a session object on the bus
    fn register_session(&mut self, session_id: &str) {
        let path = session_object_path(session_id);
        self.cr
            .insert(path, &[self.session_iface], self.mgr.clone());
    }

    /// Unregister a session object from the bus
    fn unregister_session(&mut self, session_id: &str) {
        let path = session_object_path(session_id);
        let _ = self.cr.remove::<SharedManager>(&path);
    }

    /// Register a seat object on the bus
    #[allow(dead_code)]
    fn register_seat(&mut self, seat_id: &str) {
        let path = seat_object_path(seat_id);
        self.cr.insert(path, &[self.seat_iface], self.mgr.clone());
    }

    /// Register a user object on the bus
    fn register_user(&mut self, uid: u32) {
        let path = user_object_path(uid);
        self.cr.insert(path, &[self.user_iface], self.mgr.clone());
    }

    /// Unregister a user object from the bus
    fn unregister_user(&mut self, uid: u32) {
        let path = user_object_path(uid);
        let _ = self.cr.remove::<SharedManager>(&path);
    }

    /// Process pending D-Bus messages (non-blocking)
    fn process(&mut self, timeout: Duration) -> bool {
        // Use channel-level receive and dispatch through crossroads
        let _ = self.conn.channel().read_write(Some(timeout));

        while let Some(msg) = self.conn.channel().pop_message() {
            let _ = self.cr.handle_message(msg, &self.conn);
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    init_logging();
    setup_signal_handlers();

    log::info!("systemd-logind starting");

    // Create runtime directories
    ensure_runtime_dirs();

    // Initialize login manager with shared state
    let mgr = Arc::new(Mutex::new(LoginManager::new()));

    // Log seat0 status
    {
        let mgr_guard = mgr.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(seat0) = mgr_guard.seats.get("seat0") {
            log::info!("New seat seat0.");
            log::info!(
                "Seat seat0: can_graphical={}, can_multi_session={}",
                seat0.can_graphical,
                seat0.can_multi_session
            );
        }
    }

    // Detect existing sessions
    {
        let mut mgr_guard = mgr.lock().unwrap_or_else(|e| e.into_inner());
        mgr_guard.detect_existing_sessions();
    }

    // Watchdog
    let wd_interval = watchdog_interval();
    if let Some(ref iv) = wd_interval {
        log::info!("Watchdog enabled, interval {:?}", iv);
    }
    let mut last_watchdog = Instant::now();

    // Write initial state files
    {
        let mgr_guard = mgr.lock().unwrap_or_else(|e| e.into_inner());
        mgr_guard.sync_runtime_state();
    }

    // Initialize D-Bus server
    let mut dbus_server = match DbusServer::new(mgr.clone()) {
        Ok(server) => {
            log::info!("D-Bus interface registered on {}", DBUS_NAME);
            Some(server)
        }
        Err(e) => {
            log::warn!(
                "Failed to initialize D-Bus interface: {}. Running without D-Bus.",
                e
            );
            None
        }
    };

    // Emit initial seat on D-Bus
    if let Some(ref server) = dbus_server {
        emit_seat_new(&server.conn, "seat0");
    }

    // Remove stale control socket
    let _ = fs::remove_file(CONTROL_SOCKET_PATH);

    // Bind control socket
    let listener = match UnixListener::bind(CONTROL_SOCKET_PATH) {
        Ok(l) => {
            log::info!("Listening on {}", CONTROL_SOCKET_PATH);
            Some(l)
        }
        Err(e) => {
            log::error!(
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

    sd_notify(&format!(
        "READY=1\nSTATUS=Managing sessions, D-Bus: {}",
        if dbus_server.is_some() {
            "active"
        } else {
            "unavailable"
        }
    ));

    log::info!("systemd-logind ready");

    // Periodic cleanup counter
    let mut cleanup_counter = 0u64;

    // Track sessions/users for D-Bus signal emission
    let mut known_sessions: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut known_users: std::collections::HashSet<u32> = std::collections::HashSet::new();

    // Main loop
    loop {
        if SHUTDOWN_FLAG.load(Ordering::SeqCst) {
            log::info!("Received shutdown signal");
            if let Some(ref server) = dbus_server {
                emit_prepare_for_shutdown(&server.conn, true);
            }
            break;
        }

        if RELOAD_FLAG.load(Ordering::SeqCst) {
            RELOAD_FLAG.store(false, Ordering::SeqCst);
            let mut mgr_guard = mgr.lock().unwrap_or_else(|e| e.into_inner());
            mgr_guard.enumerate_input_devices();
            mgr_guard.sync_runtime_state();
            log::info!("Reloaded configuration");
            sd_notify(&format!(
                "STATUS=Managing {} seat(s), {} session(s)",
                mgr_guard.seats.len(),
                mgr_guard.sessions.len()
            ));
        }

        // Send watchdog keepalive
        if let Some(ref iv) = wd_interval
            && last_watchdog.elapsed() >= *iv
        {
            sd_notify("WATCHDOG=1");
            last_watchdog = Instant::now();
        }

        // Process D-Bus messages
        if let Some(ref mut server) = dbus_server {
            server.process(Duration::from_millis(0));
        }

        // Handle incoming control connections
        if let Some(ref listener) = listener {
            match listener.accept() {
                Ok((mut stream, _addr)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let mut mgr_guard = mgr.lock().unwrap_or_else(|e| e.into_inner());
                    handle_client(&mut mgr_guard, &mut stream);
                    let _ = stream.shutdown(Shutdown::Both);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No connection waiting
                }
                Err(e) => {
                    log::warn!("Accept error: {}", e);
                }
            }
        }

        // Check for new/removed sessions and emit D-Bus signals
        {
            let mgr_guard = mgr.lock().unwrap_or_else(|e| e.into_inner());
            let current_sessions: std::collections::HashSet<String> =
                mgr_guard.sessions.keys().cloned().collect();
            let current_users: std::collections::HashSet<u32> =
                mgr_guard.users.keys().cloned().collect();

            // New sessions
            for sid in current_sessions.difference(&known_sessions) {
                if let Some(ref mut server) = dbus_server {
                    server.register_session(sid);
                    emit_session_new(&server.conn, sid);
                }
                if let Some(session) = mgr_guard.sessions.get(sid) {
                    // Register user if new
                    if !known_users.contains(&session.uid)
                        && let Some(ref mut server) = dbus_server
                    {
                        server.register_user(session.uid);
                        emit_user_new(&server.conn, session.uid);
                    }
                }
            }

            // Removed sessions
            for sid in known_sessions.difference(&current_sessions) {
                if let Some(ref mut server) = dbus_server {
                    emit_session_removed(&server.conn, sid);
                    server.unregister_session(sid);
                }
            }

            // Removed users
            for uid in known_users.difference(&current_users) {
                if let Some(ref mut server) = dbus_server {
                    emit_user_removed(&server.conn, *uid);
                    server.unregister_user(*uid);
                }
            }

            // New users (that didn't come through session creation above)
            for uid in current_users.difference(&known_users) {
                if let Some(ref mut server) = dbus_server {
                    server.register_user(*uid);
                    emit_user_new(&server.conn, *uid);
                }
            }

            known_sessions = current_sessions;
            known_users = current_users;
        }

        // Periodic cleanup (every ~60 iterations = ~12 seconds)
        cleanup_counter += 1;
        if cleanup_counter.is_multiple_of(60) {
            let mut mgr_guard = mgr.lock().unwrap_or_else(|e| e.into_inner());
            mgr_guard.cleanup_stale_inhibitors();
        }

        thread::sleep(Duration::from_millis(200));
    }

    // Cleanup
    let _ = fs::remove_file(CONTROL_SOCKET_PATH);
    // Remove runtime state
    let _ = fs::remove_dir_all(SESSIONS_DIR);
    let _ = fs::remove_dir_all(SEATS_DIR);
    let _ = fs::remove_dir_all(USERS_DIR);
    let _ = fs::remove_dir_all(INHIBIT_DIR);

    sd_notify("STOPPING=1");
    log::info!("systemd-logind stopped");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- LoginManager tests --

    #[test]
    fn test_login_manager_new() {
        let mgr = LoginManager::new();
        assert!(mgr.seats.contains_key("seat0"));
        assert!(mgr.sessions.is_empty());
        assert!(mgr.users.is_empty());
        assert!(mgr.inhibitors.is_empty());
    }

    #[test]
    fn test_create_session() {
        let mut mgr = LoginManager::new();
        let id = mgr.create_session(
            1000,
            "testuser",
            Some("seat0"),
            1,
            "tty",
            "user",
            "/dev/tty1",
            12345,
        );
        assert_eq!(id, "1");
        assert!(mgr.sessions.contains_key("1"));
        assert_eq!(mgr.sessions["1"].uid, 1000);
        assert_eq!(mgr.sessions["1"].user, "testuser");
        assert_eq!(mgr.sessions["1"].vtnr, 1);
        assert!(mgr.sessions["1"].active);
        assert_eq!(mgr.seats["seat0"].sessions, vec!["1"]);
        assert_eq!(mgr.seats["seat0"].active_session, Some("1".to_string()));
        assert!(mgr.users.contains_key(&1000));
    }

    #[test]
    fn test_create_multiple_sessions() {
        let mut mgr = LoginManager::new();
        let id1 = mgr.create_session(
            1000,
            "user1",
            Some("seat0"),
            1,
            "tty",
            "user",
            "/dev/tty1",
            100,
        );
        let id2 = mgr.create_session(
            1001,
            "user2",
            Some("seat0"),
            2,
            "tty",
            "user",
            "/dev/tty2",
            200,
        );
        assert_eq!(id1, "1");
        assert_eq!(id2, "2");
        assert_eq!(mgr.seats["seat0"].sessions.len(), 2);
        // First session stays active
        assert_eq!(mgr.seats["seat0"].active_session, Some("1".to_string()));
    }

    #[test]
    fn test_release_session() {
        let mut mgr = LoginManager::new();
        mgr.create_session(
            1000,
            "testuser",
            Some("seat0"),
            1,
            "tty",
            "user",
            "/dev/tty1",
            100,
        );
        assert!(mgr.release_session("1"));
        assert!(!mgr.sessions.contains_key("1"));
        assert!(mgr.seats["seat0"].sessions.is_empty());
        assert!(mgr.seats["seat0"].active_session.is_none());
        assert_eq!(mgr.users[&1000].state, "closing");
    }

    #[test]
    fn test_release_nonexistent_session() {
        let mut mgr = LoginManager::new();
        assert!(!mgr.release_session("999"));
    }

    #[test]
    fn test_create_inhibitor() {
        let mut mgr = LoginManager::new();
        let id = mgr.create_inhibitor("shutdown", "test", "testing", "block", 0, 0);
        assert_eq!(id, 1);
        assert!(mgr.inhibitors.contains_key(&1));
        assert_eq!(mgr.inhibitors[&1].what, "shutdown");
    }

    #[test]
    fn test_release_inhibitor() {
        let mut mgr = LoginManager::new();
        let id = mgr.create_inhibitor("shutdown", "test", "testing", "block", 0, 0);
        assert!(mgr.release_inhibitor(id));
        assert!(!mgr.inhibitors.contains_key(&id));
    }

    #[test]
    fn test_release_nonexistent_inhibitor() {
        let mut mgr = LoginManager::new();
        assert!(!mgr.release_inhibitor(999));
    }

    #[test]
    fn test_session_without_seat() {
        let mut mgr = LoginManager::new();
        let id = mgr.create_session(1000, "testuser", None, 0, "tty", "user", "/dev/pts/0", 100);
        assert_eq!(id, "1");
        assert!(mgr.sessions["1"].seat.is_none());
        // seat0 should be unchanged
        assert!(mgr.seats["seat0"].sessions.is_empty());
    }

    #[test]
    fn test_multiple_sessions_same_user() {
        let mut mgr = LoginManager::new();
        mgr.create_session(
            1000,
            "testuser",
            Some("seat0"),
            1,
            "tty",
            "user",
            "/dev/tty1",
            100,
        );
        mgr.create_session(1000, "testuser", None, 0, "tty", "user", "/dev/pts/0", 200);
        assert_eq!(mgr.users[&1000].sessions.len(), 2);
        // Release first session — user should still be active
        mgr.release_session("1");
        assert_eq!(mgr.users[&1000].sessions.len(), 1);
        assert_eq!(mgr.users[&1000].state, "active");
        // Release second — user should be closing
        mgr.release_session("2");
        assert_eq!(mgr.users[&1000].sessions.len(), 0);
        assert_eq!(mgr.users[&1000].state, "closing");
    }

    // -- Control command tests --

    #[test]
    fn test_handle_command_status() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(&mut mgr, "status");
        assert!(result.contains("seat"));
        assert!(result.contains("session"));
    }

    #[test]
    fn test_handle_command_list_sessions_empty() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(&mut mgr, "list-sessions");
        assert_eq!(result.trim(), "[]");
    }

    #[test]
    fn test_handle_command_list_seats() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(&mut mgr, "list-seats");
        assert!(result.contains("seat0"));
    }

    #[test]
    fn test_handle_command_create_session_json() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(
            &mut mgr,
            r#"create-session {"uid": 1000, "user": "test", "seat": "seat0", "vtnr": 1, "type": "tty", "class": "user", "tty": "/dev/tty1", "leader": 1234}"#,
        );
        assert!(result.starts_with("OK "));
        assert_eq!(mgr.sessions.len(), 1);
    }

    #[test]
    fn test_handle_command_release_session() {
        let mut mgr = LoginManager::new();
        mgr.create_session(
            1000,
            "test",
            Some("seat0"),
            1,
            "tty",
            "user",
            "/dev/tty1",
            100,
        );
        let result = handle_control_command(&mut mgr, "release-session 1");
        assert_eq!(result, "OK");
        assert!(mgr.sessions.is_empty());
    }

    #[test]
    fn test_handle_command_unknown() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(&mut mgr, "foobar");
        assert!(result.starts_with("ERROR"));
    }

    #[test]
    fn test_handle_command_can_poweroff_no_inhibitors() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(&mut mgr, "can-poweroff");
        assert_eq!(result, "yes");
    }

    #[test]
    fn test_handle_command_can_poweroff_with_inhibitor() {
        let mut mgr = LoginManager::new();
        mgr.create_inhibitor("shutdown", "test", "testing", "block", 0, 0);
        let result = handle_control_command(&mut mgr, "can-poweroff");
        assert_eq!(result, "challenge");
    }

    #[test]
    fn test_handle_command_inhibit() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(
            &mut mgr,
            r#"inhibit {"what": "shutdown", "who": "test", "why": "testing", "mode": "block", "uid": 0, "pid": 1}"#,
        );
        assert!(result.starts_with("OK "));
        assert_eq!(mgr.inhibitors.len(), 1);
    }

    #[test]
    fn test_handle_command_release_inhibitor() {
        let mut mgr = LoginManager::new();
        mgr.create_inhibitor("shutdown", "test", "testing", "block", 0, 0);
        let result = handle_control_command(&mut mgr, "release-inhibitor 1");
        assert_eq!(result, "OK");
        assert!(mgr.inhibitors.is_empty());
    }

    #[test]
    fn test_activate_session() {
        let mut mgr = LoginManager::new();
        mgr.create_session(
            1000,
            "user1",
            Some("seat0"),
            1,
            "tty",
            "user",
            "/dev/tty1",
            100,
        );
        mgr.create_session(
            1001,
            "user2",
            Some("seat0"),
            2,
            "tty",
            "user",
            "/dev/tty2",
            200,
        );
        let result = handle_control_command(&mut mgr, "activate-session 2");
        assert_eq!(result, "OK");
        assert!(!mgr.sessions["1"].active);
        assert_eq!(mgr.sessions["1"].state, "online");
        assert!(mgr.sessions["2"].active);
        assert_eq!(mgr.sessions["2"].state, "active");
        assert_eq!(mgr.seats["seat0"].active_session, Some("2".to_string()));
    }

    #[test]
    fn test_terminate_user() {
        let mut mgr = LoginManager::new();
        mgr.create_session(
            1000,
            "testuser",
            Some("seat0"),
            1,
            "tty",
            "user",
            "/dev/tty1",
            100,
        );
        mgr.create_session(1000, "testuser", None, 0, "tty", "user", "/dev/pts/0", 200);
        let result = handle_control_command(&mut mgr, "terminate-user 1000");
        assert_eq!(result, "OK");
        assert!(mgr.sessions.is_empty());
    }

    #[test]
    fn test_seat0_always_exists() {
        let mgr = LoginManager::new();
        assert!(mgr.seats.contains_key("seat0"));
        assert!(mgr.seats["seat0"].can_multi_session);
    }

    #[test]
    fn test_format_status() {
        let mut mgr = LoginManager::new();
        mgr.create_session(
            1000,
            "test",
            Some("seat0"),
            1,
            "tty",
            "user",
            "/dev/tty1",
            100,
        );
        let status = mgr.format_status();
        assert!(status.contains("1 sessions."));
        assert!(status.contains("1 users."));
        assert!(status.contains("0 inhibitors."));
    }

    #[test]
    fn test_check_seat0_graphical() {
        // Just make sure it doesn't panic; value depends on the system
        let _ = check_seat0_graphical();
    }

    #[test]
    fn test_session_incrementing_ids() {
        let mut mgr = LoginManager::new();
        let id1 = mgr.create_session(1000, "a", None, 0, "tty", "user", "", 1);
        let id2 = mgr.create_session(1001, "b", None, 0, "tty", "user", "", 2);
        let id3 = mgr.create_session(1002, "c", None, 0, "tty", "user", "", 3);
        assert_eq!(id1, "1");
        assert_eq!(id2, "2");
        assert_eq!(id3, "3");
    }

    #[test]
    fn test_inhibitor_incrementing_ids() {
        let mut mgr = LoginManager::new();
        let id1 = mgr.create_inhibitor("shutdown", "a", "", "block", 0, 0);
        let id2 = mgr.create_inhibitor("sleep", "b", "", "delay", 0, 0);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn test_watchdog_interval_none() {
        // When WATCHDOG_USEC is not set, should return None
        unsafe { std::env::remove_var("WATCHDOG_USEC") };
        assert!(watchdog_interval().is_none());
    }

    #[test]
    fn test_lock_unlock_session() {
        let mut mgr = LoginManager::new();
        mgr.create_session(
            1000,
            "test",
            Some("seat0"),
            1,
            "tty",
            "user",
            "/dev/tty1",
            100,
        );
        let result = handle_control_command(&mut mgr, "lock-session 1");
        assert_eq!(result, "OK");
        assert!(mgr.sessions["1"].locked_hint);
        let result = handle_control_command(&mut mgr, "unlock-session 1");
        assert_eq!(result, "OK");
        assert!(!mgr.sessions["1"].locked_hint);
    }

    #[test]
    fn test_lock_nonexistent_session() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(&mut mgr, "lock-session 999");
        assert!(result.starts_with("ERROR"));
    }

    #[test]
    fn test_show_session() {
        let mut mgr = LoginManager::new();
        mgr.create_session(
            1000,
            "testuser",
            Some("seat0"),
            1,
            "tty",
            "user",
            "/dev/tty1",
            100,
        );
        let result = handle_control_command(&mut mgr, "show-session 1");
        assert!(result.contains("testuser"));
        assert!(result.contains("seat0"));
    }

    #[test]
    fn test_show_nonexistent_session() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(&mut mgr, "show-session 999");
        assert!(result.starts_with("ERROR"));
    }

    #[test]
    fn test_show_seat() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(&mut mgr, "show-seat seat0");
        assert!(result.contains("seat0"));
    }

    #[test]
    fn test_show_user_by_uid() {
        let mut mgr = LoginManager::new();
        mgr.create_session(1000, "testuser", None, 0, "tty", "user", "", 100);
        let result = handle_control_command(&mut mgr, "show-user 1000");
        assert!(result.contains("testuser"));
    }

    #[test]
    fn test_show_user_by_name() {
        let mut mgr = LoginManager::new();
        mgr.create_session(1000, "testuser", None, 0, "tty", "user", "", 100);
        let result = handle_control_command(&mut mgr, "show-user testuser");
        assert!(result.contains("testuser"));
    }

    #[test]
    fn test_flush_devices() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(&mut mgr, "flush-devices");
        assert_eq!(result, "OK");
    }

    #[test]
    fn test_can_suspend() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(&mut mgr, "can-suspend");
        assert_eq!(result, "yes");
    }

    #[test]
    fn test_list_inhibitors_empty() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(&mut mgr, "list-inhibitors");
        assert_eq!(result.trim(), "[]");
    }

    #[test]
    fn test_list_users_empty() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(&mut mgr, "list-users");
        assert_eq!(result.trim(), "[]");
    }

    // -- D-Bus object path tests --

    #[test]
    fn test_session_object_path() {
        let path = session_object_path("1");
        assert_eq!(path.to_string(), "/org/freedesktop/login1/session/_1");
    }

    #[test]
    fn test_session_object_path_with_dash() {
        let path = session_object_path("c-1");
        assert_eq!(path.to_string(), "/org/freedesktop/login1/session/_c_2d1");
    }

    #[test]
    fn test_seat_object_path() {
        let path = seat_object_path("seat0");
        assert_eq!(path.to_string(), "/org/freedesktop/login1/seat/seat0");
    }

    #[test]
    fn test_user_object_path() {
        let path = user_object_path(1000);
        assert_eq!(path.to_string(), "/org/freedesktop/login1/user/_1000");
    }

    #[test]
    fn test_session_id_from_path() {
        assert_eq!(
            session_id_from_path("/org/freedesktop/login1/session/_1"),
            Some("1".to_string())
        );
        assert_eq!(
            session_id_from_path("/org/freedesktop/login1/session/_c_2d1"),
            Some("c-1".to_string())
        );
        assert_eq!(session_id_from_path("/wrong/path"), None);
    }

    #[test]
    fn test_seat_id_from_path() {
        assert_eq!(
            seat_id_from_path("/org/freedesktop/login1/seat/seat0"),
            Some("seat0".to_string())
        );
        assert_eq!(seat_id_from_path("/wrong/path"), None);
    }

    #[test]
    fn test_uid_from_path() {
        assert_eq!(
            uid_from_path("/org/freedesktop/login1/user/_1000"),
            Some(1000)
        );
        assert_eq!(uid_from_path("/wrong/path"), None);
    }

    // -- Activate / lock / unlock via LoginManager methods --

    #[test]
    fn test_activate_session_method() {
        let mut mgr = LoginManager::new();
        mgr.create_session(
            1000,
            "user1",
            Some("seat0"),
            1,
            "tty",
            "user",
            "/dev/tty1",
            100,
        );
        mgr.create_session(
            1001,
            "user2",
            Some("seat0"),
            2,
            "tty",
            "user",
            "/dev/tty2",
            200,
        );
        assert!(mgr.activate_session("2").is_ok());
        assert!(!mgr.sessions["1"].active);
        assert!(mgr.sessions["2"].active);
    }

    #[test]
    fn test_activate_session_not_found() {
        let mut mgr = LoginManager::new();
        assert!(mgr.activate_session("999").is_err());
    }

    #[test]
    fn test_activate_session_no_seat() {
        let mut mgr = LoginManager::new();
        mgr.create_session(1000, "user1", None, 0, "tty", "user", "", 100);
        assert!(mgr.activate_session("1").is_err());
    }

    #[test]
    fn test_lock_unlock_session_method() {
        let mut mgr = LoginManager::new();
        mgr.create_session(
            1000,
            "test",
            Some("seat0"),
            1,
            "tty",
            "user",
            "/dev/tty1",
            100,
        );
        assert!(mgr.lock_session("1").is_ok());
        assert!(mgr.sessions["1"].locked_hint);
        assert!(mgr.unlock_session("1").is_ok());
        assert!(!mgr.sessions["1"].locked_hint);
    }

    #[test]
    fn test_lock_session_not_found() {
        let mut mgr = LoginManager::new();
        assert!(mgr.lock_session("999").is_err());
    }

    #[test]
    fn test_lock_all_sessions() {
        let mut mgr = LoginManager::new();
        mgr.create_session(1000, "a", None, 0, "tty", "user", "", 100);
        mgr.create_session(1001, "b", None, 0, "tty", "user", "", 200);
        mgr.lock_sessions();
        assert!(mgr.sessions["1"].locked_hint);
        assert!(mgr.sessions["2"].locked_hint);
        mgr.unlock_sessions();
        assert!(!mgr.sessions["1"].locked_hint);
        assert!(!mgr.sessions["2"].locked_hint);
    }

    #[test]
    fn test_set_idle_hint() {
        let mut mgr = LoginManager::new();
        mgr.create_session(1000, "test", None, 0, "tty", "user", "", 100);
        assert!(mgr.set_idle_hint("1", true).is_ok());
        assert!(mgr.sessions["1"].idle_hint);
        assert!(mgr.sessions["1"].idle_since_hint > 0);
        assert!(mgr.set_idle_hint("1", false).is_ok());
        assert!(!mgr.sessions["1"].idle_hint);
        assert_eq!(mgr.sessions["1"].idle_since_hint, 0);
    }

    #[test]
    fn test_set_locked_hint() {
        let mut mgr = LoginManager::new();
        mgr.create_session(1000, "test", None, 0, "tty", "user", "", 100);
        assert!(mgr.set_locked_hint("1", true).is_ok());
        assert!(mgr.sessions["1"].locked_hint);
        assert!(mgr.set_locked_hint("1", false).is_ok());
        assert!(!mgr.sessions["1"].locked_hint);
    }

    #[test]
    fn test_set_session_type() {
        let mut mgr = LoginManager::new();
        mgr.create_session(1000, "test", None, 0, "tty", "user", "", 100);
        assert!(mgr.set_session_type("1", "wayland").is_ok());
        assert_eq!(mgr.sessions["1"].session_type, "wayland");
        assert!(mgr.set_session_type("1", "x11").is_ok());
        assert_eq!(mgr.sessions["1"].session_type, "x11");
        assert!(mgr.set_session_type("1", "invalid").is_err());
    }

    #[test]
    fn test_terminate_user_method() {
        let mut mgr = LoginManager::new();
        mgr.create_session(1000, "test", None, 0, "tty", "user", "", 100);
        mgr.create_session(1000, "test", None, 0, "tty", "user", "", 200);
        assert!(mgr.terminate_user(1000).is_ok());
        assert!(mgr.sessions.is_empty());
    }

    #[test]
    fn test_terminate_user_not_found() {
        let mut mgr = LoginManager::new();
        assert!(mgr.terminate_user(9999).is_err());
    }

    #[test]
    fn test_terminate_seat_method() {
        let mut mgr = LoginManager::new();
        mgr.create_session(1000, "a", Some("seat0"), 1, "tty", "user", "", 100);
        mgr.create_session(1001, "b", Some("seat0"), 2, "tty", "user", "", 200);
        assert!(mgr.terminate_seat("seat0").is_ok());
        assert!(mgr.sessions.is_empty());
    }

    #[test]
    fn test_terminate_seat_not_found() {
        let mut mgr = LoginManager::new();
        assert!(mgr.terminate_seat("nonexistent").is_err());
    }

    // -- Idle hint tests --

    #[test]
    fn test_global_idle_hint_empty() {
        let mgr = LoginManager::new();
        assert!(mgr.global_idle_hint());
    }

    #[test]
    fn test_global_idle_hint_active_session() {
        let mut mgr = LoginManager::new();
        mgr.create_session(1000, "test", Some("seat0"), 1, "tty", "user", "", 100);
        assert!(!mgr.global_idle_hint()); // active session, not idle
    }

    #[test]
    fn test_global_idle_hint_all_idle() {
        let mut mgr = LoginManager::new();
        mgr.create_session(1000, "test", Some("seat0"), 1, "tty", "user", "", 100);
        mgr.set_idle_hint("1", true).unwrap();
        assert!(mgr.global_idle_hint());
    }

    #[test]
    fn test_global_idle_since_hint() {
        let mut mgr = LoginManager::new();
        mgr.create_session(1000, "test", None, 0, "tty", "user", "", 100);
        mgr.set_idle_hint("1", true).unwrap();
        let (rt, mono) = mgr.global_idle_since_hint();
        assert!(rt > 0);
        assert!(mono > 0);
    }

    // -- Inhibitor tests --

    #[test]
    fn test_can_action_no_inhibitors() {
        let mgr = LoginManager::new();
        assert_eq!(mgr.can_action("poweroff"), "yes");
        assert_eq!(mgr.can_action("reboot"), "yes");
        assert_eq!(mgr.can_action("suspend"), "yes");
    }

    #[test]
    fn test_can_action_with_blocking_inhibitor() {
        let mut mgr = LoginManager::new();
        mgr.create_inhibitor("shutdown", "test", "testing", "block", 0, 0);
        assert_eq!(mgr.can_action("poweroff"), "challenge");
    }

    #[test]
    fn test_can_action_with_delay_inhibitor() {
        let mut mgr = LoginManager::new();
        mgr.create_inhibitor("shutdown", "test", "testing", "delay", 0, 0);
        assert_eq!(mgr.can_action("poweroff"), "yes"); // delay doesn't block
    }

    #[test]
    fn test_block_inhibited() {
        let mut mgr = LoginManager::new();
        assert_eq!(mgr.block_inhibited(), "");
        mgr.create_inhibitor("shutdown", "test", "testing", "block", 0, 0);
        assert_eq!(mgr.block_inhibited(), "shutdown");
        mgr.create_inhibitor("sleep", "test2", "testing", "block", 0, 0);
        let result = mgr.block_inhibited();
        assert!(result.contains("shutdown"));
        assert!(result.contains("sleep"));
    }

    #[test]
    fn test_delay_inhibited() {
        let mut mgr = LoginManager::new();
        assert_eq!(mgr.delay_inhibited(), "");
        mgr.create_inhibitor("shutdown", "test", "testing", "delay", 0, 0);
        assert_eq!(mgr.delay_inhibited(), "shutdown");
    }

    // -- Configuration tests --

    #[test]
    fn test_default_config() {
        let config = LogindConfig::default();
        assert_eq!(config.n_auto_vts, 6);
        assert!(config.kill_user_processes);
        assert_eq!(config.handle_power_key, "poweroff");
        assert_eq!(config.handle_suspend_key, "suspend");
        assert_eq!(config.handle_lid_switch, "suspend");
        assert_eq!(
            config.inhibit_delay_max_usec,
            DEFAULT_INHIBIT_DELAY_MAX_USEC
        );
        assert_eq!(config.sessions_max, 8192);
    }

    #[test]
    fn test_parse_timespan_seconds() {
        assert_eq!(parse_timespan_to_usec("5"), Ok(5_000_000));
        assert_eq!(parse_timespan_to_usec("5s"), Ok(5_000_000));
        assert_eq!(parse_timespan_to_usec("5sec"), Ok(5_000_000));
        assert_eq!(parse_timespan_to_usec("5second"), Ok(5_000_000));
        assert_eq!(parse_timespan_to_usec("5seconds"), Ok(5_000_000));
    }

    #[test]
    fn test_parse_timespan_minutes() {
        assert_eq!(parse_timespan_to_usec("2min"), Ok(120_000_000));
        assert_eq!(parse_timespan_to_usec("2minute"), Ok(120_000_000));
        assert_eq!(parse_timespan_to_usec("2minutes"), Ok(120_000_000));
    }

    #[test]
    fn test_parse_timespan_hours() {
        assert_eq!(parse_timespan_to_usec("1h"), Ok(3_600_000_000));
        assert_eq!(parse_timespan_to_usec("1hr"), Ok(3_600_000_000));
        assert_eq!(parse_timespan_to_usec("1hour"), Ok(3_600_000_000));
    }

    #[test]
    fn test_parse_timespan_milliseconds() {
        assert_eq!(parse_timespan_to_usec("500ms"), Ok(500_000));
        assert_eq!(parse_timespan_to_usec("500msec"), Ok(500_000));
    }

    #[test]
    fn test_parse_timespan_microseconds() {
        assert_eq!(parse_timespan_to_usec("1000us"), Ok(1000));
        assert_eq!(parse_timespan_to_usec("1000usec"), Ok(1000));
    }

    #[test]
    fn test_parse_timespan_invalid() {
        assert!(parse_timespan_to_usec("").is_err());
        assert!(parse_timespan_to_usec("abc").is_err());
    }

    // -- Session fields tests --

    #[test]
    fn test_session_new_fields() {
        let mut mgr = LoginManager::new();
        mgr.create_session(
            1000,
            "test",
            Some("seat0"),
            1,
            "tty",
            "user",
            "/dev/tty1",
            5678,
        );
        let session = &mgr.sessions["1"];
        assert_eq!(session.display, "");
        assert_eq!(session.service, "");
        assert_eq!(session.desktop, "");
        assert!(!session.remote);
        assert_eq!(session.remote_host, "");
        assert_eq!(session.remote_user, "");
        assert!(session.since > 0);
        assert!(session.since_monotonic > 0);
        assert!(!session.idle_hint);
        assert_eq!(session.idle_since_hint, 0);
        assert!(!session.locked_hint);
    }

    #[test]
    fn test_user_new_fields() {
        let mut mgr = LoginManager::new();
        mgr.create_session(1000, "test", None, 0, "tty", "user", "", 100);
        let user = &mgr.users[&1000];
        assert_eq!(user.slice, "user-1000.slice");
        assert_eq!(user.service, "user@1000.service");
        assert_eq!(user.runtime_path, "/run/user/1000");
        assert!(!user.linger);
        assert!(user.since > 0);
    }

    // -- resolve_uid_to_name --

    #[test]
    fn test_resolve_uid_to_name_fallback() {
        // UID 99999 almost certainly doesn't exist
        let name = resolve_uid_to_name(99999);
        assert_eq!(name, "99999");
    }

    #[test]
    fn test_resolve_uid_to_name_root() {
        let name = resolve_uid_to_name(0);
        // On most systems, UID 0 is "root"
        // But in test environments it might not exist
        assert!(!name.is_empty());
    }

    // -- check_ac_power --

    #[test]
    fn test_check_ac_power() {
        // Just make sure it doesn't panic
        let _ = check_ac_power();
    }

    // -- Control command edge cases --

    #[test]
    fn test_handle_command_lock_sessions() {
        let mut mgr = LoginManager::new();
        mgr.create_session(1000, "a", None, 0, "tty", "user", "", 100);
        mgr.create_session(1001, "b", None, 0, "tty", "user", "", 200);
        let result = handle_control_command(&mut mgr, "lock-sessions");
        assert_eq!(result, "OK");
        assert!(mgr.sessions["1"].locked_hint);
        assert!(mgr.sessions["2"].locked_hint);
    }

    #[test]
    fn test_handle_command_unlock_sessions() {
        let mut mgr = LoginManager::new();
        mgr.create_session(1000, "a", None, 0, "tty", "user", "", 100);
        mgr.lock_sessions();
        let result = handle_control_command(&mut mgr, "unlock-sessions");
        assert_eq!(result, "OK");
        assert!(!mgr.sessions["1"].locked_hint);
    }

    #[test]
    fn test_handle_command_terminate_session() {
        let mut mgr = LoginManager::new();
        mgr.create_session(
            1000,
            "test",
            Some("seat0"),
            1,
            "tty",
            "user",
            "/dev/tty1",
            100,
        );
        let result = handle_control_command(&mut mgr, "terminate-session 1");
        assert_eq!(result, "OK");
        assert!(mgr.sessions.is_empty());
    }

    #[test]
    fn test_handle_command_list_users_with_users() {
        let mut mgr = LoginManager::new();
        mgr.create_session(1000, "testuser", None, 0, "tty", "user", "", 100);
        let result = handle_control_command(&mut mgr, "list-users");
        assert!(result.contains("testuser"));
        assert!(result.contains("1000"));
    }

    #[test]
    fn test_handle_command_list_inhibitors_with_inhibitors() {
        let mut mgr = LoginManager::new();
        mgr.create_inhibitor("shutdown", "test-app", "saving work", "block", 1000, 5678);
        let result = handle_control_command(&mut mgr, "list-inhibitors");
        assert!(result.contains("shutdown"));
        assert!(result.contains("test-app"));
        assert!(result.contains("saving work"));
    }

    #[test]
    fn test_handle_command_can_hibernate() {
        let mut mgr = LoginManager::new();
        assert_eq!(handle_control_command(&mut mgr, "can-hibernate"), "yes");
    }

    #[test]
    fn test_handle_command_can_hybrid_sleep() {
        let mut mgr = LoginManager::new();
        assert_eq!(handle_control_command(&mut mgr, "can-hybrid-sleep"), "yes");
    }

    #[test]
    fn test_handle_command_can_suspend_then_hibernate() {
        let mut mgr = LoginManager::new();
        assert_eq!(
            handle_control_command(&mut mgr, "can-suspend-then-hibernate"),
            "yes"
        );
    }

    #[test]
    fn test_handle_command_poweroff() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(&mut mgr, "poweroff");
        assert!(result.starts_with("OK"));
    }

    #[test]
    fn test_handle_command_reboot() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(&mut mgr, "reboot");
        assert!(result.starts_with("OK"));
    }

    #[test]
    fn test_handle_command_suspend() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(&mut mgr, "suspend");
        assert!(result.starts_with("OK"));
    }

    #[test]
    fn test_handle_command_hibernate() {
        let mut mgr = LoginManager::new();
        let result = handle_control_command(&mut mgr, "hibernate");
        assert!(result.starts_with("OK"));
    }

    // -- Config parsing from file --

    #[test]
    fn test_parse_logind_conf_file() {
        let dir = tempfile::tempdir().unwrap();
        let conf_path = dir.path().join("logind.conf");
        fs::write(
            &conf_path,
            "[Login]\nNAutoVTs=3\nKillUserProcesses=no\nHandlePowerKey=ignore\nInhibitDelayMaxSec=10s\nSessionsMax=4096\n",
        )
        .unwrap();

        let mut config = LogindConfig::default();
        parse_logind_conf_file(&conf_path.to_string_lossy(), &mut config);

        assert_eq!(config.n_auto_vts, 3);
        assert!(!config.kill_user_processes);
        assert_eq!(config.handle_power_key, "ignore");
        assert_eq!(config.inhibit_delay_max_usec, 10_000_000);
        assert_eq!(config.sessions_max, 4096);
    }

    #[test]
    fn test_parse_logind_conf_file_wrong_section() {
        let dir = tempfile::tempdir().unwrap();
        let conf_path = dir.path().join("logind.conf");
        fs::write(&conf_path, "[Other]\nNAutoVTs=99\n").unwrap();

        let mut config = LogindConfig::default();
        parse_logind_conf_file(&conf_path.to_string_lossy(), &mut config);

        // Should not be changed since it's in the wrong section
        assert_eq!(config.n_auto_vts, 6);
    }

    #[test]
    fn test_parse_logind_conf_file_comments() {
        let dir = tempfile::tempdir().unwrap();
        let conf_path = dir.path().join("logind.conf");
        fs::write(
            &conf_path,
            "# comment\n; another comment\n[Login]\n#NAutoVTs=99\nNAutoVTs=2\n",
        )
        .unwrap();

        let mut config = LogindConfig::default();
        parse_logind_conf_file(&conf_path.to_string_lossy(), &mut config);

        assert_eq!(config.n_auto_vts, 2);
    }

    #[test]
    fn test_parse_logind_conf_nonexistent() {
        let mut config = LogindConfig::default();
        parse_logind_conf_file("/nonexistent/path/logind.conf", &mut config);
        // Should not crash, config should remain default
        assert_eq!(config.n_auto_vts, 6);
    }

    #[test]
    fn test_parse_logind_conf_all_keys() {
        let dir = tempfile::tempdir().unwrap();
        let conf_path = dir.path().join("logind.conf");
        fs::write(
            &conf_path,
            "[Login]\n\
             NAutoVTs=4\n\
             KillUserProcesses=yes\n\
             KillOnlyUsers=alice bob\n\
             KillExcludeUsers=root nobody\n\
             IdleAction=suspend\n\
             IdleActionSec=30min\n\
             InhibitDelayMaxSec=3\n\
             UserStopDelaySec=20\n\
             HandlePowerKey=suspend\n\
             HandleSuspendKey=hibernate\n\
             HandleHibernateKey=ignore\n\
             HandleLidSwitch=poweroff\n\
             HandleLidSwitchExternalPower=ignore\n\
             HandleLidSwitchDocked=suspend\n\
             HoldoffTimeoutSec=60\n\
             RemoveIPC=no\n\
             InhibitorsMax=100\n\
             SessionsMax=200\n",
        )
        .unwrap();

        let mut config = LogindConfig::default();
        parse_logind_conf_file(&conf_path.to_string_lossy(), &mut config);

        assert_eq!(config.n_auto_vts, 4);
        assert!(config.kill_user_processes);
        assert_eq!(config.kill_only_users, vec!["alice", "bob"]);
        assert_eq!(config.kill_exclude_users, vec!["root", "nobody"]);
        assert_eq!(config.idle_action, "suspend");
        assert_eq!(config.idle_action_usec, 30 * 60 * 1_000_000);
        assert_eq!(config.inhibit_delay_max_usec, 3_000_000);
        assert_eq!(config.user_stop_delay_usec, 20_000_000);
        assert_eq!(config.handle_power_key, "suspend");
        assert_eq!(config.handle_suspend_key, "hibernate");
        assert_eq!(config.handle_hibernate_key, "ignore");
        assert_eq!(config.handle_lid_switch, "poweroff");
        assert_eq!(config.handle_lid_switch_external_power, "ignore");
        assert_eq!(config.handle_lid_switch_docked, "suspend");
        assert_eq!(config.holdoff_timeout_usec, 60_000_000);
        assert!(!config.remove_ipc);
        assert_eq!(config.inhibitors_max, 100);
        assert_eq!(config.sessions_max, 200);
    }

    // -- resolve_user_gid --

    #[test]
    fn test_resolve_user_gid_fallback() {
        // UID 99999 almost certainly doesn't exist
        let gid = resolve_user_gid(99999);
        assert_eq!(gid, 99999); // fallback: gid = uid
    }
}
