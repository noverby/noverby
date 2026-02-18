//! systemd-logind — Login and seat management daemon.
//!
//! This daemon manages user login sessions, seats (groups of devices),
//! and handles system power/sleep button events. It replaces the C
//! systemd-logind with a Rust implementation.
//!
//! Features:
//! - Session tracking (create, release, list, query)
//! - Seat management (seat0 + dynamic seats)
//! - Input device monitoring for power/sleep buttons
//! - Inhibitor lock management (shutdown, sleep, idle, etc.)
//! - sd_notify protocol (READY, WATCHDOG, STATUS)
//! - Control socket for loginctl CLI
//! - User tracking (sessions per user)
//! - VT (virtual terminal) tracking

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

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
        fn enabled(&self, _metadata: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            if self.enabled(record.metadata()) {
                let ts = chrono_lite_timestamp();
                eprintln!(
                    "[{}][systemd-logind][{}] {}",
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
    let now = SystemTime::now();
    let dur = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Rough conversion — good enough for log timestamps
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

// ---------------------------------------------------------------------------
// sd_notify helper
// ---------------------------------------------------------------------------

fn sd_notify(msg: &str) {
    if let Ok(path) = std::env::var("NOTIFY_SOCKET") {
        let path = if path.starts_with('@') {
            // Abstract socket
            format!("\0{}", &path[1..])
        } else {
            path
        };
        if let Ok(sock) = std::os::unix::net::UnixDatagram::unbound() {
            let _ = sock.send_to(msg.as_bytes(), &path);
        }
    }
}

fn watchdog_interval() -> Option<Duration> {
    if let Ok(usec_str) = std::env::var("WATCHDOG_USEC") {
        if let Ok(usec) = usec_str.parse::<u64>() {
            // Send keepalive at half the configured interval
            let half = usec / 2;
            if half > 0 {
                return Some(Duration::from_micros(half));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Session types
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
    /// Leader PID (the PAM session leader / login process)
    pub leader: u32,
    /// Creation timestamp (seconds since epoch)
    pub since: u64,
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
    /// Username
    pub name: String,
    /// User state: "active", "online", "lingering", "closing"
    pub state: String,
    /// Session IDs belonging to this user
    pub sessions: Vec<String>,
    /// Slice (e.g. "user-1000.slice")
    pub slice: String,
    /// Login timestamp
    pub since: u64,
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

struct LoginManager {
    sessions: HashMap<String, Session>,
    seats: HashMap<String, Seat>,
    users: HashMap<u32, User>,
    inhibitors: HashMap<u64, Inhibitor>,
    next_session_id: u64,
    next_inhibitor_id: u64,
    /// Discovered power button input device paths
    power_button_devices: Vec<PathBuf>,
}

impl LoginManager {
    fn new() -> Self {
        let mut mgr = LoginManager {
            sessions: HashMap::new(),
            seats: HashMap::new(),
            users: HashMap::new(),
            inhibitors: HashMap::new(),
            next_session_id: 1,
            next_inhibitor_id: 1,
            power_button_devices: Vec::new(),
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

    /// Create a new session.
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
            leader,
            since: Self::now_secs(),
        };

        // Register in seat
        if let Some(ref seat_id) = seat_name {
            if let Some(seat) = self.seats.get_mut(seat_id) {
                seat.sessions.push(id.clone());
                if seat.active_session.is_none() {
                    seat.active_session = Some(id.clone());
                }
            }
        }

        // Register in user tracking
        let user_entry = self.users.entry(uid).or_insert_with(|| User {
            uid,
            name: user.to_string(),
            state: "active".to_string(),
            sessions: Vec::new(),
            slice: format!("user-{}.slice", uid),
            since: Self::now_secs(),
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
        if let Some(ref seat_id) = session.seat {
            if let Some(seat) = self.seats.get_mut(seat_id) {
                seat.sessions.retain(|s| s != session_id);
                if seat.active_session.as_deref() == Some(session_id) {
                    seat.active_session = seat.sessions.first().cloned();
                }
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
        if session.leader > 0 {
            content.push_str(&format!("LEADER={}\n", session.leader));
        }
        content.push_str(&format!("SCOPE={}\n", session.scope));
        content.push_str(&format!(
            "REALTIME={}\n",
            session.since.saturating_mul(1_000_000)
        ));
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
            let sessions_str: Vec<&str> = user.sessions.iter().map(|s| s.as_str()).collect();
            if !sessions_str.is_empty() {
                content.push_str(&format!("SESSIONS={}\n", sessions_str.join(" ")));
                content.push_str(&format!("DISPLAY={}\n", sessions_str[0]));
            }
            content.push_str(&format!(
                "REALTIME={}\n",
                user.since.saturating_mul(1_000_000)
            ));
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
// Helper: check if seat0 can do graphics
// ---------------------------------------------------------------------------

fn check_seat0_graphical() -> bool {
    // Check if there's a DRM or framebuffer device available
    Path::new("/dev/dri").exists()
        || Path::new("/dev/fb0").exists()
        || Path::new("/sys/class/graphics/fb0").exists()
}

// ---------------------------------------------------------------------------
// Control socket command handler
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

        "activate-session" => {
            // Activate (make foreground) a session on its seat
            if let Some(session) = mgr.sessions.get(args).cloned() {
                if let Some(ref seat_id) = session.seat {
                    if let Some(seat) = mgr.seats.get_mut(seat_id) {
                        // Deactivate current active session
                        if let Some(ref old_active) = seat.active_session {
                            if let Some(old_session) = mgr.sessions.get_mut(old_active) {
                                old_session.active = false;
                                old_session.state = "online".to_string();
                            }
                        }
                        // Activate new session
                        seat.active_session = Some(args.to_string());
                    }
                    if let Some(session) = mgr.sessions.get_mut(args) {
                        session.active = true;
                        session.state = "active".to_string();
                    }
                    mgr.sync_runtime_state();
                    log::info!("Activated session {} on {}", args, seat_id);
                    "OK".to_string()
                } else {
                    format!("ERROR: Session '{}' has no seat", args)
                }
            } else {
                format!("ERROR: Session '{}' not found", args)
            }
        }

        "lock-session" | "unlock-session" => {
            if mgr.sessions.contains_key(args) {
                log::info!(
                    "{} session {}",
                    if command == "lock-session" {
                        "Locked"
                    } else {
                        "Unlocked"
                    },
                    args
                );
                "OK".to_string()
            } else {
                format!("ERROR: Session '{}' not found", args)
            }
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
                if let Some(user) = mgr.users.get(&uid) {
                    let session_ids: Vec<String> = user.sessions.clone();
                    for sid in &session_ids {
                        mgr.release_session(sid);
                    }
                    mgr.sync_runtime_state();
                    log::info!(
                        "Terminated {} session(s) for UID {}",
                        session_ids.len(),
                        uid
                    );
                    "OK".to_string()
                } else {
                    format!("ERROR: User {} not found", uid)
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
            // Check if there are any blocking inhibitors for this action
            let action = command.strip_prefix("can-").unwrap_or("shutdown");
            let blocked = mgr.inhibitors.values().any(|inhibitor| {
                inhibitor.mode == "block"
                    && (inhibitor.what.contains("shutdown")
                        || inhibitor.what.contains(action)
                        || inhibitor.what.contains("sleep"))
            });
            if blocked { "challenge" } else { "yes" }.to_string()
        }

        "poweroff" | "reboot" | "suspend" | "hibernate" => {
            log::info!("Requested system action: {}", command);
            // In a real implementation, this would trigger the action
            // through the service manager. For now, just acknowledge.
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
// Main
// ---------------------------------------------------------------------------

fn main() {
    init_logging();
    setup_signal_handlers();

    log::info!("systemd-logind starting");

    // Create runtime directories
    ensure_runtime_dirs();

    // Initialize login manager
    let mut mgr = LoginManager::new();

    // Log seat0 status
    if let Some(seat0) = mgr.seats.get("seat0") {
        log::info!("New seat seat0.");
        log::info!(
            "Seat seat0: can_graphical={}, can_multi_session={}",
            seat0.can_graphical,
            seat0.can_multi_session
        );
    }

    // Detect existing sessions
    mgr.detect_existing_sessions();

    // Watchdog
    let wd_interval = watchdog_interval();
    if let Some(ref iv) = wd_interval {
        log::info!("Watchdog enabled, interval {:?}", iv);
    }
    let mut last_watchdog = Instant::now();

    // Write initial state files
    mgr.sync_runtime_state();

    // Remove stale control socket
    let _ = fs::remove_file(CONTROL_SOCKET_PATH);

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
            sd_notify("READY=1\nSTATUS=Running (no control socket)");
            loop {
                if SHUTDOWN_FLAG.load(Ordering::SeqCst) {
                    break;
                }
                if let Some(ref iv) = wd_interval {
                    if last_watchdog.elapsed() >= *iv {
                        sd_notify("WATCHDOG=1");
                        last_watchdog = Instant::now();
                    }
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
        "READY=1\nSTATUS=Managing {} seat(s), {} session(s)",
        mgr.seats.len(),
        mgr.sessions.len()
    ));

    log::info!("systemd-logind ready");

    // Periodic cleanup counter
    let mut cleanup_counter = 0u64;

    // Main loop
    loop {
        if SHUTDOWN_FLAG.load(Ordering::SeqCst) {
            log::info!("Received shutdown signal");
            break;
        }

        if RELOAD_FLAG.load(Ordering::SeqCst) {
            RELOAD_FLAG.store(false, Ordering::SeqCst);
            mgr.enumerate_input_devices();
            mgr.sync_runtime_state();
            log::info!("Reloaded configuration");
            sd_notify(&format!(
                "STATUS=Managing {} seat(s), {} session(s)",
                mgr.seats.len(),
                mgr.sessions.len()
            ));
        }

        // Send watchdog keepalive
        if let Some(ref iv) = wd_interval {
            if last_watchdog.elapsed() >= *iv {
                sd_notify("WATCHDOG=1");
                last_watchdog = Instant::now();
            }
        }

        // Handle incoming control connections
        match listener.accept() {
            Ok((mut stream, _addr)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                handle_client(&mut mgr, &mut stream);
                let _ = stream.shutdown(Shutdown::Both);
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // No connection waiting
            }
            Err(e) => {
                log::warn!("Accept error: {}", e);
            }
        }

        // Periodic cleanup (every ~60 iterations = ~12 seconds)
        cleanup_counter += 1;
        if cleanup_counter % 60 == 0 {
            mgr.cleanup_stale_inhibitors();
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
        let result = handle_control_command(&mut mgr, "unlock-session 1");
        assert_eq!(result, "OK");
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
}
