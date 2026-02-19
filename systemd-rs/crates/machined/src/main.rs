//! systemd-machined — VM and container registration/tracking daemon
//!
//! This is a Rust implementation of systemd's `systemd-machined` daemon. It
//! manages the registration and tracking of virtual machines and containers
//! running on the local host.
//!
//! ## Features
//!
//! - Machine registration (register/terminate) with class (vm/container),
//!   service, scope, leader PID, root directory, and network interfaces
//! - Machine listing and status queries
//! - Runtime state files in `/run/systemd/machines/`
//! - Control socket at `/run/systemd/machined-control` for `machinectl` CLI
//! - sd_notify protocol (READY/WATCHDOG/STATUS/STOPPING)
//! - Signal handling (SIGTERM/SIGINT for shutdown, SIGHUP for reload)
//! - Stale machine cleanup (machines whose leader PID has exited)
//!
//! ## Missing
//!
//! - D-Bus interface (`org.freedesktop.machine1`)
//! - Image management (list/clone/rename/remove/set-limit disk images)
//! - Machine scoping (creating transient scope units)
//! - Copy-to/copy-from operations
//! - PTY forwarding for `login`/`shell`
//! - bind/bind-user mounts
//! - OS image operations (import/export/pull)

use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::{UnixDatagram, UnixListener, UnixStream};
use std::path::Path;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

const MACHINES_DIR: &str = "/run/systemd/machines";
const CONTROL_SOCKET_PATH: &str = "/run/systemd/machined-control";

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// Machine class — either a virtual machine or a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineClass {
    Container,
    Vm,
}

impl MachineClass {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "container" => Some(MachineClass::Container),
            "vm" => Some(MachineClass::Vm),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            MachineClass::Container => "container",
            MachineClass::Vm => "vm",
        }
    }
}

impl fmt::Display for MachineClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// State of a registered machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineState {
    Opening,
    Running,
    Closing,
}

impl MachineState {
    pub fn as_str(&self) -> &'static str {
        match self {
            MachineState::Opening => "opening",
            MachineState::Running => "running",
            MachineState::Closing => "closing",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "opening" => Some(MachineState::Opening),
            "running" => Some(MachineState::Running),
            "closing" => Some(MachineState::Closing),
            _ => None,
        }
    }
}

impl fmt::Display for MachineState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A registered machine.
#[derive(Debug, Clone)]
pub struct Machine {
    /// Unique machine name.
    pub name: String,
    /// Class — VM or container.
    pub class: MachineClass,
    /// Service that registered this machine (e.g. "systemd-nspawn").
    pub service: String,
    /// Scope unit name.
    pub scope: String,
    /// Leader PID in the host PID namespace.
    pub leader: u32,
    /// Root directory (for containers, typically `/`).
    pub root_directory: String,
    /// Network interface indices assigned to this machine.
    pub netif: Vec<u32>,
    /// Timestamp (CLOCK_REALTIME microseconds since epoch) when registered.
    pub timestamp: u64,
    /// Current state.
    pub state: MachineState,
}

impl Machine {
    /// Serialize to an INI-style state file for `/run/systemd/machines/<name>`.
    pub fn to_state_file(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("NAME={}\n", self.name));
        s.push_str(&format!("CLASS={}\n", self.class));
        s.push_str(&format!("SERVICE={}\n", self.service));
        s.push_str(&format!("SCOPE={}\n", self.scope));
        s.push_str(&format!("LEADER={}\n", self.leader));
        s.push_str(&format!("ROOT={}\n", self.root_directory));
        s.push_str(&format!("STATE={}\n", self.state));
        s.push_str(&format!("TIMESTAMP={}\n", self.timestamp));
        if !self.netif.is_empty() {
            let nifs: Vec<String> = self.netif.iter().map(|n| n.to_string()).collect();
            s.push_str(&format!("NETIF={}\n", nifs.join(" ")));
        }
        s
    }

    /// Parse a machine from a state file.
    pub fn from_state_file(content: &str) -> Option<Self> {
        let fields = parse_env_content(content);
        let name = fields.get("NAME")?.clone();
        let class = MachineClass::parse(fields.get("CLASS").map(|s| s.as_str()).unwrap_or(""))?;
        let service = fields.get("SERVICE").cloned().unwrap_or_default();
        let scope = fields.get("SCOPE").cloned().unwrap_or_default();
        let leader = fields
            .get("LEADER")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let root_directory = fields
            .get("ROOT")
            .cloned()
            .unwrap_or_else(|| "/".to_string());
        let state = fields
            .get("STATE")
            .and_then(|s| MachineState::parse(s))
            .unwrap_or(MachineState::Running);
        let timestamp = fields
            .get("TIMESTAMP")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let netif = fields
            .get("NETIF")
            .map(|s| {
                s.split_whitespace()
                    .filter_map(|n| n.parse().ok())
                    .collect()
            })
            .unwrap_or_default();

        Some(Machine {
            name,
            class,
            service,
            scope,
            leader,
            root_directory,
            netif,
            timestamp,
            state,
        })
    }

    /// Check if the leader PID is still alive.
    pub fn is_leader_alive(&self) -> bool {
        if self.leader == 0 {
            return false;
        }
        // kill(pid, 0) checks if process exists without sending a signal
        unsafe { libc::kill(self.leader as i32, 0) == 0 }
    }

    /// Format status output for this machine.
    pub fn format_status(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("       Name: {}\n", self.name));
        s.push_str(&format!("      Class: {}\n", self.class));
        s.push_str(&format!("    Service: {}\n", self.service));
        s.push_str(&format!("      Scope: {}\n", self.scope));
        s.push_str(&format!("     Leader: {}\n", self.leader));
        s.push_str(&format!("       Root: {}\n", self.root_directory));
        s.push_str(&format!("      State: {}\n", self.state));
        s.push_str(&format!(
            "      Since: {}\n",
            format_timestamp(self.timestamp)
        ));
        if !self.netif.is_empty() {
            let nifs: Vec<String> = self.netif.iter().map(|n| n.to_string()).collect();
            s.push_str(&format!("     NetIf: {}\n", nifs.join(" ")));
        }
        s
    }

    /// Format `show` output (key=value pairs).
    pub fn format_show(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("Name={}\n", self.name));
        s.push_str(&format!("Class={}\n", self.class));
        s.push_str(&format!("Service={}\n", self.service));
        s.push_str(&format!("Scope={}\n", self.scope));
        s.push_str(&format!("Leader={}\n", self.leader));
        s.push_str(&format!("RootDirectory={}\n", self.root_directory));
        s.push_str(&format!("State={}\n", self.state));
        s.push_str(&format!("Timestamp={}\n", self.timestamp));
        if !self.netif.is_empty() {
            let nifs: Vec<String> = self.netif.iter().map(|n| n.to_string()).collect();
            s.push_str(&format!("NetworkInterfaces={}\n", nifs.join(" ")));
        }
        s
    }
}

// ---------------------------------------------------------------------------
// Machine registry
// ---------------------------------------------------------------------------

/// In-memory registry of machines.
#[derive(Debug, Default)]
pub struct MachineRegistry {
    machines: BTreeMap<String, Machine>,
}

impl MachineRegistry {
    pub fn new() -> Self {
        Self {
            machines: BTreeMap::new(),
        }
    }

    /// Load existing machines from state files in the default machines directory.
    pub fn load(&mut self) {
        self.load_from(MACHINES_DIR);
    }

    /// Load existing machines from state files in a given directory.
    pub fn load_from(&mut self, dir: &str) {
        self.machines.clear();
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.starts_with('.') {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&path) {
                if let Some(machine) = Machine::from_state_file(&content) {
                    self.machines.insert(machine.name.clone(), machine);
                }
            }
        }
    }

    /// Register a new machine.
    pub fn register(&mut self, machine: Machine) -> Result<(), String> {
        if machine.name.is_empty() {
            return Err("Machine name must not be empty".into());
        }
        if !is_valid_machine_name(&machine.name) {
            return Err(format!("Invalid machine name '{}'", machine.name));
        }
        if self.machines.contains_key(&machine.name) {
            return Err(format!("Machine '{}' already registered", machine.name));
        }
        self.machines.insert(machine.name.clone(), machine);
        Ok(())
    }

    /// Terminate (unregister) a machine by name.
    pub fn terminate(&mut self, name: &str) -> Result<Machine, String> {
        self.terminate_in(name, MACHINES_DIR)
    }

    /// Terminate (unregister) a machine, removing its state file from a given dir.
    pub fn terminate_in(&mut self, name: &str, dir: &str) -> Result<Machine, String> {
        match self.machines.remove(name) {
            Some(machine) => {
                let path = Path::new(dir).join(name);
                let _ = fs::remove_file(&path);
                Ok(machine)
            }
            None => Err(format!("Machine '{}' not found", name)),
        }
    }

    /// Get a machine by name.
    pub fn get(&self, name: &str) -> Option<&Machine> {
        self.machines.get(name)
    }

    /// Get a mutable reference to a machine by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Machine> {
        self.machines.get_mut(name)
    }

    /// List all registered machines, sorted by name.
    pub fn list(&self) -> Vec<&Machine> {
        self.machines.values().collect()
    }

    /// Number of registered machines.
    pub fn count(&self) -> usize {
        self.machines.len()
    }

    /// Find a machine by leader PID.
    pub fn find_by_leader(&self, pid: u32) -> Option<&Machine> {
        self.machines.values().find(|m| m.leader == pid)
    }

    /// Save all machine state files to the default directory.
    pub fn save_all(&self) -> io::Result<()> {
        self.save_all_to(MACHINES_DIR)
    }

    /// Save all machine state files to a given directory.
    pub fn save_all_to(&self, dir: &str) -> io::Result<()> {
        fs::create_dir_all(dir)?;
        for machine in self.machines.values() {
            let path = Path::new(dir).join(&machine.name);
            fs::write(&path, machine.to_state_file())?;
        }
        Ok(())
    }

    /// Save a single machine state file to the default directory.
    pub fn save_one(&self, name: &str) -> io::Result<()> {
        self.save_one_to(name, MACHINES_DIR)
    }

    /// Save a single machine state file to a given directory.
    pub fn save_one_to(&self, name: &str, dir: &str) -> io::Result<()> {
        if let Some(machine) = self.machines.get(name) {
            fs::create_dir_all(dir)?;
            let path = Path::new(dir).join(name);
            fs::write(&path, machine.to_state_file())?;
        }
        Ok(())
    }

    /// Remove dead machines whose leader PID is no longer alive.
    /// Returns list of names of removed machines.
    pub fn gc(&mut self) -> Vec<String> {
        self.gc_in(MACHINES_DIR)
    }

    /// Remove dead machines, removing state files from a given directory.
    pub fn gc_in(&mut self, dir: &str) -> Vec<String> {
        let dead: Vec<String> = self
            .machines
            .iter()
            .filter(|(_, m)| !m.is_leader_alive())
            .map(|(name, _)| name.clone())
            .collect();

        for name in &dead {
            self.machines.remove(name);
            let path = Path::new(dir).join(name);
            let _ = fs::remove_file(&path);
        }

        dead
    }

    /// Format a listing of all machines.
    pub fn format_list(&self) -> String {
        let machines = self.list();
        if machines.is_empty() {
            return "No machines.\n".to_string();
        }
        let mut s = String::new();
        s.push_str(&format!(
            "{:<32} {:>10} {:>12} {:>10}\n",
            "MACHINE", "CLASS", "SERVICE", "STATE"
        ));
        for m in &machines {
            s.push_str(&format!(
                "{:<32} {:>10} {:>12} {:>10}\n",
                m.name, m.class, m.service, m.state
            ));
        }
        s.push_str(&format!("\n{} machines listed.\n", machines.len()));
        s
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate a machine name. Machine names follow hostname rules plus
/// allowing dots for FQDN-like names. `.host` is a special valid name.
pub fn is_valid_machine_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    // `.host` is the special name for the host machine itself
    if name == ".host" {
        return true;
    }
    // Reject other dotfile-style names
    if name.starts_with('.') {
        return false;
    }
    // Must start with alphanumeric or underscore
    let first = match name.chars().next() {
        Some(c) => c,
        None => return false,
    };
    if !first.is_ascii_alphanumeric() && first != '_' {
        return false;
    }
    // Must end with alphanumeric
    let last = name.chars().next_back().unwrap();
    if !last.is_ascii_alphanumeric() {
        return false;
    }
    // Only alphanumeric, hyphen, underscore, dot allowed
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_env_content(content: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let mut value = value.trim().to_string();
            // Strip surrounding quotes
            if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                if value.len() >= 2 {
                    value = value[1..value.len() - 1].to_string();
                }
            }
            map.insert(key.to_string(), value);
        }
    }
    map
}

fn now_usec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

fn format_timestamp(usec: u64) -> String {
    if usec == 0 {
        return "n/a".to_string();
    }
    let secs = usec / 1_000_000;
    let total_days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    let (year, month, day) = days_to_ymd(total_days);

    static MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mon = if month >= 1 && month <= 12 {
        MONTHS[(month - 1) as usize]
    } else {
        "???"
    };

    format!(
        "{} {:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        mon, year, month, day, hours, minutes, seconds
    )
}

fn days_to_ymd(total_days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = total_days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

// ---------------------------------------------------------------------------
// Control socket command handler
// ---------------------------------------------------------------------------

/// Handle a single control command line from a client.
fn handle_control_command(registry: &mut MachineRegistry, line: &str) -> String {
    let parts: Vec<&str> = line.trim().splitn(2, ' ').collect();
    let cmd = parts.first().copied().unwrap_or("");
    let args_str = parts.get(1).copied().unwrap_or("");

    match cmd.to_uppercase().as_str() {
        "LIST" => registry.format_list(),

        "STATUS" => {
            if args_str.is_empty() {
                return format!("Machines: {}\n", registry.count());
            }
            match registry.get(args_str) {
                Some(m) => m.format_status(),
                None => format!("ERROR: Machine '{}' not found\n", args_str),
            }
        }

        "SHOW" => {
            if args_str.is_empty() {
                return "ERROR: Machine name required\n".to_string();
            }
            match registry.get(args_str) {
                Some(m) => m.format_show(),
                None => format!("ERROR: Machine '{}' not found\n", args_str),
            }
        }

        // REGISTER <name> <class> <service> <leader> [root_directory] [scope]
        "REGISTER" => {
            let reg_parts: Vec<&str> = args_str.splitn(6, ' ').collect();
            if reg_parts.len() < 4 {
                return "ERROR: Usage: REGISTER <name> <class> <service> <leader> [root] [scope]\n"
                    .to_string();
            }
            let name = reg_parts[0].to_string();
            let class = match MachineClass::parse(reg_parts[1]) {
                Some(c) => c,
                None => {
                    return format!(
                        "ERROR: Invalid class '{}'. Use 'container' or 'vm'\n",
                        reg_parts[1]
                    )
                }
            };
            let service = reg_parts[2].to_string();
            let leader: u32 = match reg_parts[3].parse() {
                Ok(p) => p,
                Err(_) => return format!("ERROR: Invalid leader PID '{}'\n", reg_parts[3]),
            };
            let root_directory = reg_parts
                .get(4)
                .filter(|s| !s.is_empty())
                .unwrap_or(&"/")
                .to_string();
            let scope = reg_parts.get(5).unwrap_or(&"").to_string();

            let machine = Machine {
                name: name.clone(),
                class,
                service,
                scope,
                leader,
                root_directory,
                netif: Vec::new(),
                timestamp: now_usec(),
                state: MachineState::Running,
            };

            match registry.register(machine) {
                Ok(()) => {
                    let _ = registry.save_one(&name);
                    format!("OK: Registered '{}'\n", name)
                }
                Err(e) => format!("ERROR: {}\n", e),
            }
        }

        "TERMINATE" => {
            if args_str.is_empty() {
                return "ERROR: Machine name required\n".to_string();
            }
            match registry.terminate(args_str) {
                Ok(machine) => {
                    // Optionally kill the leader
                    if machine.leader > 0 {
                        unsafe {
                            libc::kill(machine.leader as i32, libc::SIGTERM);
                        }
                    }
                    format!("OK: Terminated '{}'\n", args_str)
                }
                Err(e) => format!("ERROR: {}\n", e),
            }
        }

        "GC" => {
            let removed = registry.gc();
            if removed.is_empty() {
                "OK: No stale machines\n".to_string()
            } else {
                format!(
                    "OK: Removed {} stale machines: {}\n",
                    removed.len(),
                    removed.join(", ")
                )
            }
        }

        "PING" => "PONG\n".to_string(),

        _ => format!("ERROR: Unknown command '{}'\n", cmd),
    }
}

/// Handle a client connection on the control socket.
fn handle_client(registry: &mut MachineRegistry, stream: &mut UnixStream) {
    let reader = match stream.try_clone() {
        Ok(s) => BufReader::new(s),
        Err(_) => return,
    };

    for line in reader.lines() {
        match line {
            Ok(l) if l.trim().is_empty() => continue,
            Ok(l) => {
                let response = handle_control_command(registry, &l);
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
            Err(_) => break,
        }
    }
}

// ---------------------------------------------------------------------------
// sd_notify helper
// ---------------------------------------------------------------------------

fn sd_notify(msg: &str) {
    let sock_path = match env::var("NOTIFY_SOCKET") {
        Ok(p) => p,
        Err(_) => return,
    };

    let path = if let Some(stripped) = sock_path.strip_prefix('@') {
        format!("\0{}", stripped)
    } else {
        sock_path
    };

    let sock = match UnixDatagram::unbound() {
        Ok(s) => s,
        Err(_) => return,
    };

    let _ = sock.send_to(msg.as_bytes(), &path);
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
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            if self.enabled(record.metadata()) {
                eprintln!(
                    "systemd-machined[{}]: {}: {}",
                    process::id(),
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

// ---------------------------------------------------------------------------
// Watchdog
// ---------------------------------------------------------------------------

fn watchdog_interval() -> Option<Duration> {
    let usec: u64 = env::var("WATCHDOG_USEC").ok()?.parse().ok()?;
    if usec == 0 {
        return None;
    }
    // Send at half the interval
    Some(Duration::from_micros(usec / 2))
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    init_logging();
    setup_signal_handlers();

    log::info!("systemd-machined starting");

    // Load existing machine state
    let mut registry = MachineRegistry::new();
    let _ = fs::create_dir_all(MACHINES_DIR);
    registry.load();
    log::info!("Loaded {} machines from state files", registry.count());

    // GC stale machines on startup
    let removed = registry.gc();
    if !removed.is_empty() {
        log::info!("Removed {} stale machines on startup", removed.len());
    }

    // Watchdog support
    let wd_interval = watchdog_interval();
    if let Some(ref iv) = wd_interval {
        log::info!("Watchdog enabled, interval {:?}", iv);
    }
    let mut last_watchdog = Instant::now();

    // Ensure parent directory exists
    let _ = fs::create_dir_all(Path::new(CONTROL_SOCKET_PATH).parent().unwrap());

    // Remove stale socket
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
                if SHUTDOWN.load(Ordering::SeqCst) {
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
        "READY=1\nSTATUS=Managing {} machines",
        registry.count()
    ));

    log::info!("systemd-machined ready");

    // Periodic GC interval
    let gc_interval = Duration::from_secs(30);
    let mut last_gc = Instant::now();

    // Main loop
    loop {
        if SHUTDOWN.load(Ordering::SeqCst) {
            log::info!("Received shutdown signal");
            break;
        }

        if RELOAD.load(Ordering::SeqCst) {
            RELOAD.store(false, Ordering::SeqCst);
            registry.load();
            log::info!("Reloaded, {} machines", registry.count());
            sd_notify(&format!("STATUS=Managing {} machines", registry.count()));
        }

        // Periodic GC of dead machines
        if last_gc.elapsed() >= gc_interval {
            let removed = registry.gc();
            if !removed.is_empty() {
                log::info!(
                    "GC removed {} stale machines: {}",
                    removed.len(),
                    removed.join(", ")
                );
                sd_notify(&format!("STATUS=Managing {} machines", registry.count()));
            }
            last_gc = Instant::now();
        }

        // Watchdog keepalive
        if let Some(ref iv) = wd_interval {
            if last_watchdog.elapsed() >= *iv {
                sd_notify("WATCHDOG=1");
                last_watchdog = Instant::now();
            }
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
    log::info!("systemd-machined stopped");
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_dir() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    // -- MachineClass -------------------------------------------------------

    #[test]
    fn test_machine_class_parse() {
        assert_eq!(
            MachineClass::parse("container"),
            Some(MachineClass::Container)
        );
        assert_eq!(
            MachineClass::parse("Container"),
            Some(MachineClass::Container)
        );
        assert_eq!(
            MachineClass::parse("CONTAINER"),
            Some(MachineClass::Container)
        );
        assert_eq!(MachineClass::parse("vm"), Some(MachineClass::Vm));
        assert_eq!(MachineClass::parse("VM"), Some(MachineClass::Vm));
        assert_eq!(MachineClass::parse("Vm"), Some(MachineClass::Vm));
        assert_eq!(MachineClass::parse("invalid"), None);
        assert_eq!(MachineClass::parse(""), None);
    }

    #[test]
    fn test_machine_class_display() {
        assert_eq!(format!("{}", MachineClass::Container), "container");
        assert_eq!(format!("{}", MachineClass::Vm), "vm");
    }

    #[test]
    fn test_machine_class_as_str() {
        assert_eq!(MachineClass::Container.as_str(), "container");
        assert_eq!(MachineClass::Vm.as_str(), "vm");
    }

    // -- MachineState -------------------------------------------------------

    #[test]
    fn test_machine_state_parse() {
        assert_eq!(MachineState::parse("opening"), Some(MachineState::Opening));
        assert_eq!(MachineState::parse("running"), Some(MachineState::Running));
        assert_eq!(MachineState::parse("closing"), Some(MachineState::Closing));
        assert_eq!(MachineState::parse("RUNNING"), Some(MachineState::Running));
        assert_eq!(MachineState::parse("invalid"), None);
    }

    #[test]
    fn test_machine_state_display() {
        assert_eq!(format!("{}", MachineState::Opening), "opening");
        assert_eq!(format!("{}", MachineState::Running), "running");
        assert_eq!(format!("{}", MachineState::Closing), "closing");
    }

    // -- Machine state file roundtrip ---------------------------------------

    fn make_test_machine(name: &str) -> Machine {
        Machine {
            name: name.to_string(),
            class: MachineClass::Container,
            service: "systemd-nspawn".to_string(),
            scope: format!("machine-{}.scope", name),
            leader: 12345,
            root_directory: "/".to_string(),
            netif: vec![3, 7],
            timestamp: 1700000000_000000,
            state: MachineState::Running,
        }
    }

    #[test]
    fn test_machine_state_file_roundtrip() {
        let machine = make_test_machine("mycontainer");
        let content = machine.to_state_file();
        let parsed = Machine::from_state_file(&content).unwrap();

        assert_eq!(parsed.name, "mycontainer");
        assert_eq!(parsed.class, MachineClass::Container);
        assert_eq!(parsed.service, "systemd-nspawn");
        assert_eq!(parsed.scope, "machine-mycontainer.scope");
        assert_eq!(parsed.leader, 12345);
        assert_eq!(parsed.root_directory, "/");
        assert_eq!(parsed.state, MachineState::Running);
        assert_eq!(parsed.timestamp, 1700000000_000000);
        assert_eq!(parsed.netif, vec![3, 7]);
    }

    #[test]
    fn test_machine_state_file_no_netif() {
        let mut machine = make_test_machine("nonet");
        machine.netif = vec![];
        let content = machine.to_state_file();
        assert!(!content.contains("NETIF="));
        let parsed = Machine::from_state_file(&content).unwrap();
        assert!(parsed.netif.is_empty());
    }

    #[test]
    fn test_machine_state_file_vm_class() {
        let mut machine = make_test_machine("myvm");
        machine.class = MachineClass::Vm;
        let content = machine.to_state_file();
        assert!(content.contains("CLASS=vm\n"));
        let parsed = Machine::from_state_file(&content).unwrap();
        assert_eq!(parsed.class, MachineClass::Vm);
    }

    #[test]
    fn test_machine_from_state_file_missing_name() {
        let content = "CLASS=container\nLEADER=1\n";
        assert!(Machine::from_state_file(content).is_none());
    }

    #[test]
    fn test_machine_from_state_file_missing_class() {
        let content = "NAME=test\nLEADER=1\n";
        assert!(Machine::from_state_file(content).is_none());
    }

    #[test]
    fn test_machine_from_state_file_invalid_class() {
        let content = "NAME=test\nCLASS=invalid\nLEADER=1\n";
        assert!(Machine::from_state_file(content).is_none());
    }

    #[test]
    fn test_machine_from_state_file_minimal() {
        let content = "NAME=test\nCLASS=container\n";
        let m = Machine::from_state_file(content).unwrap();
        assert_eq!(m.name, "test");
        assert_eq!(m.class, MachineClass::Container);
        assert_eq!(m.leader, 0);
        assert_eq!(m.root_directory, "/");
        assert_eq!(m.state, MachineState::Running);
        assert!(m.netif.is_empty());
    }

    // -- Machine format -----------------------------------------------------

    #[test]
    fn test_machine_format_status() {
        let machine = make_test_machine("mycontainer");
        let status = machine.format_status();
        assert!(status.contains("Name: mycontainer"));
        assert!(status.contains("Class: container"));
        assert!(status.contains("Service: systemd-nspawn"));
        assert!(status.contains("Leader: 12345"));
        assert!(status.contains("State: running"));
        assert!(status.contains("NetIf: 3 7"));
    }

    #[test]
    fn test_machine_format_show() {
        let machine = make_test_machine("mycontainer");
        let show = machine.format_show();
        assert!(show.contains("Name=mycontainer\n"));
        assert!(show.contains("Class=container\n"));
        assert!(show.contains("Leader=12345\n"));
        assert!(show.contains("State=running\n"));
        assert!(show.contains("NetworkInterfaces=3 7\n"));
    }

    #[test]
    fn test_machine_format_show_no_netif() {
        let mut machine = make_test_machine("nonet");
        machine.netif = vec![];
        let show = machine.format_show();
        assert!(!show.contains("NetworkInterfaces="));
    }

    // -- Validation ---------------------------------------------------------

    #[test]
    fn test_is_valid_machine_name_valid() {
        assert!(is_valid_machine_name("mycontainer"));
        assert!(is_valid_machine_name("test-vm"));
        assert!(is_valid_machine_name("my.container"));
        assert!(is_valid_machine_name("a"));
        assert!(is_valid_machine_name("_private"));
        assert!(is_valid_machine_name("test_vm"));
        assert!(is_valid_machine_name(".host"));
    }

    #[test]
    fn test_is_valid_machine_name_invalid() {
        assert!(!is_valid_machine_name(""));
        assert!(!is_valid_machine_name("-starts-with-hyphen"));
        assert!(!is_valid_machine_name("ends-with-hyphen-"));
        assert!(!is_valid_machine_name(".dotfile"));
        assert!(!is_valid_machine_name(".."));
        assert!(!is_valid_machine_name("."));
        assert!(!is_valid_machine_name("has space"));
        assert!(!is_valid_machine_name("has/slash"));
        assert!(!is_valid_machine_name(&"a".repeat(65)));
    }

    #[test]
    fn test_is_valid_machine_name_max_length() {
        assert!(is_valid_machine_name(&"a".repeat(64)));
        assert!(!is_valid_machine_name(&"a".repeat(65)));
    }

    // -- MachineRegistry ----------------------------------------------------

    #[test]
    fn test_registry_register_and_list() {
        let mut reg = MachineRegistry::new();
        assert_eq!(reg.count(), 0);
        assert!(reg.list().is_empty());

        let machine = make_test_machine("test1");
        reg.register(machine).unwrap();
        assert_eq!(reg.count(), 1);
        assert_eq!(reg.list()[0].name, "test1");
    }

    #[test]
    fn test_registry_register_duplicate() {
        let mut reg = MachineRegistry::new();
        reg.register(make_test_machine("test1")).unwrap();
        let result = reg.register(make_test_machine("test1"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already registered"));
    }

    #[test]
    fn test_registry_register_empty_name() {
        let mut reg = MachineRegistry::new();
        let mut machine = make_test_machine("x");
        machine.name = String::new();
        let result = reg.register(machine);
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_register_invalid_name() {
        let mut reg = MachineRegistry::new();
        let mut machine = make_test_machine("x");
        machine.name = "has space".to_string();
        let result = reg.register(machine);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid machine name"));
    }

    #[test]
    fn test_registry_terminate() {
        let mut reg = MachineRegistry::new();
        reg.register(make_test_machine("test1")).unwrap();
        assert_eq!(reg.count(), 1);

        let dir = temp_dir();
        let removed = reg
            .terminate_in("test1", dir.path().to_str().unwrap())
            .unwrap();
        assert_eq!(removed.name, "test1");
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn test_registry_terminate_not_found() {
        let mut reg = MachineRegistry::new();
        let dir = temp_dir();
        let result = reg.terminate_in("nonexistent", dir.path().to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_registry_get() {
        let mut reg = MachineRegistry::new();
        reg.register(make_test_machine("test1")).unwrap();
        assert!(reg.get("test1").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_find_by_leader() {
        let mut reg = MachineRegistry::new();
        let mut m = make_test_machine("test1");
        m.leader = 42;
        reg.register(m).unwrap();

        assert_eq!(reg.find_by_leader(42).unwrap().name, "test1");
        assert!(reg.find_by_leader(99).is_none());
    }

    // -- Registry persistence -----------------------------------------------

    #[test]
    fn test_registry_save_and_load() {
        let dir = temp_dir();
        let dir_path = dir.path().to_str().unwrap();

        let mut reg = MachineRegistry::new();
        reg.register(make_test_machine("alpha")).unwrap();
        let mut beta = make_test_machine("beta");
        beta.class = MachineClass::Vm;
        beta.leader = 99999;
        reg.register(beta).unwrap();
        reg.save_all_to(dir_path).unwrap();

        // Verify files exist
        assert!(dir.path().join("alpha").exists());
        assert!(dir.path().join("beta").exists());

        // Load into a fresh registry
        let mut reg2 = MachineRegistry::new();
        reg2.load_from(dir_path);
        assert_eq!(reg2.count(), 2);
        assert_eq!(reg2.get("alpha").unwrap().class, MachineClass::Container);
        assert_eq!(reg2.get("beta").unwrap().class, MachineClass::Vm);
        assert_eq!(reg2.get("beta").unwrap().leader, 99999);
    }

    #[test]
    fn test_registry_save_one() {
        let dir = temp_dir();
        let dir_path = dir.path().to_str().unwrap();

        let mut reg = MachineRegistry::new();
        reg.register(make_test_machine("single")).unwrap();
        reg.save_one_to("single", dir_path).unwrap();

        assert!(dir.path().join("single").exists());
        let content = fs::read_to_string(dir.path().join("single")).unwrap();
        assert!(content.contains("NAME=single\n"));
    }

    #[test]
    fn test_registry_load_empty_dir() {
        let dir = temp_dir();
        let mut reg = MachineRegistry::new();
        reg.load_from(dir.path().to_str().unwrap());
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn test_registry_load_nonexistent_dir() {
        let mut reg = MachineRegistry::new();
        reg.load_from("/nonexistent/path/to/machines");
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn test_registry_load_skips_dotfiles() {
        let dir = temp_dir();
        let dir_path = dir.path().to_str().unwrap();

        // Write a valid machine file
        let m = make_test_machine("visible");
        fs::write(dir.path().join("visible"), m.to_state_file()).unwrap();

        // Write a dotfile (should be skipped)
        fs::write(dir.path().join(".hidden"), "NAME=hidden\nCLASS=container\n").unwrap();

        let mut reg = MachineRegistry::new();
        reg.load_from(dir_path);
        assert_eq!(reg.count(), 1);
        assert!(reg.get("visible").is_some());
        assert!(reg.get("hidden").is_none());
    }

    #[test]
    fn test_registry_load_skips_invalid_files() {
        let dir = temp_dir();
        let dir_path = dir.path().to_str().unwrap();

        // Write a valid machine file
        fs::write(
            dir.path().join("good"),
            "NAME=good\nCLASS=container\nLEADER=1\n",
        )
        .unwrap();

        // Write an invalid file (missing CLASS)
        fs::write(dir.path().join("bad"), "NAME=bad\nLEADER=1\n").unwrap();

        let mut reg = MachineRegistry::new();
        reg.load_from(dir_path);
        assert_eq!(reg.count(), 1);
        assert!(reg.get("good").is_some());
    }

    // -- Registry GC --------------------------------------------------------

    #[test]
    fn test_registry_gc_keeps_alive() {
        let mut reg = MachineRegistry::new();
        let mut m = make_test_machine("self");
        // Use our own PID so it's definitely alive
        m.leader = process::id();
        reg.register(m).unwrap();

        let dir = temp_dir();
        let removed = reg.gc_in(dir.path().to_str().unwrap());
        assert!(removed.is_empty());
        assert_eq!(reg.count(), 1);
    }

    #[test]
    fn test_registry_gc_removes_dead() {
        let mut reg = MachineRegistry::new();
        let mut m = make_test_machine("dead");
        // PID that almost certainly doesn't exist
        m.leader = 4_000_000;
        reg.register(m).unwrap();

        let dir = temp_dir();
        let removed = reg.gc_in(dir.path().to_str().unwrap());
        assert_eq!(removed, vec!["dead"]);
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn test_registry_gc_leader_zero_is_dead() {
        let mut reg = MachineRegistry::new();
        let mut m = make_test_machine("noleader");
        m.leader = 0;
        reg.register(m).unwrap();

        let dir = temp_dir();
        let removed = reg.gc_in(dir.path().to_str().unwrap());
        assert_eq!(removed, vec!["noleader"]);
    }

    // -- Registry format_list -----------------------------------------------

    #[test]
    fn test_registry_format_list_empty() {
        let reg = MachineRegistry::new();
        let output = reg.format_list();
        assert!(output.contains("No machines."));
    }

    #[test]
    fn test_registry_format_list_with_machines() {
        let mut reg = MachineRegistry::new();
        reg.register(make_test_machine("alpha")).unwrap();
        let mut beta = make_test_machine("beta");
        beta.class = MachineClass::Vm;
        reg.register(beta).unwrap();

        let output = reg.format_list();
        assert!(output.contains("MACHINE"));
        assert!(output.contains("alpha"));
        assert!(output.contains("beta"));
        assert!(output.contains("container"));
        assert!(output.contains("vm"));
        assert!(output.contains("2 machines listed."));
    }

    // -- parse_env_content --------------------------------------------------

    #[test]
    fn test_parse_env_content_basic() {
        let content = "KEY=value\nFOO=bar\n";
        let m = parse_env_content(content);
        assert_eq!(m.get("KEY").unwrap(), "value");
        assert_eq!(m.get("FOO").unwrap(), "bar");
    }

    #[test]
    fn test_parse_env_content_quoted() {
        let content = "KEY=\"hello world\"\n";
        let m = parse_env_content(content);
        assert_eq!(m.get("KEY").unwrap(), "hello world");
    }

    #[test]
    fn test_parse_env_content_single_quoted() {
        let content = "KEY='hello world'\n";
        let m = parse_env_content(content);
        assert_eq!(m.get("KEY").unwrap(), "hello world");
    }

    #[test]
    fn test_parse_env_content_comments_and_blanks() {
        let content = "# comment\n\nKEY=value\n  # another comment\n";
        let m = parse_env_content(content);
        assert_eq!(m.len(), 1);
        assert_eq!(m.get("KEY").unwrap(), "value");
    }

    #[test]
    fn test_parse_env_content_empty() {
        let m = parse_env_content("");
        assert!(m.is_empty());
    }

    // -- format_timestamp / days_to_ymd -------------------------------------

    #[test]
    fn test_format_timestamp_zero() {
        assert_eq!(format_timestamp(0), "n/a");
    }

    #[test]
    fn test_format_timestamp_epoch() {
        // 1970-01-01 00:00:00 UTC
        let ts = format_timestamp(1);
        assert!(ts.contains("1970"));
    }

    #[test]
    fn test_format_timestamp_known_date() {
        // 2023-11-14 22:13:20 UTC = 1700000000 seconds since epoch
        let ts = format_timestamp(1700000000_000000);
        assert!(ts.contains("2023"));
        assert!(ts.contains("Nov"));
        assert!(ts.contains("14"));
    }

    #[test]
    fn test_days_to_ymd_epoch() {
        let (y, m, d) = days_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_days_to_ymd_known() {
        // 2000-01-01 is day 10957
        let (y, m, d) = days_to_ymd(10957);
        assert_eq!((y, m, d), (2000, 1, 1));
    }

    // -- now_usec -----------------------------------------------------------

    #[test]
    fn test_now_usec_nonzero() {
        assert!(now_usec() > 0);
    }

    // -- Control command handler --------------------------------------------

    #[test]
    fn test_handle_command_ping() {
        let mut reg = MachineRegistry::new();
        let resp = handle_control_command(&mut reg, "PING");
        assert_eq!(resp, "PONG\n");
    }

    #[test]
    fn test_handle_command_list_empty() {
        let mut reg = MachineRegistry::new();
        let resp = handle_control_command(&mut reg, "LIST");
        assert!(resp.contains("No machines."));
    }

    #[test]
    fn test_handle_command_list_with_machines() {
        let mut reg = MachineRegistry::new();
        reg.register(make_test_machine("test1")).unwrap();
        let resp = handle_control_command(&mut reg, "LIST");
        assert!(resp.contains("test1"));
        assert!(resp.contains("1 machines listed."));
    }

    #[test]
    fn test_handle_command_status_global() {
        let mut reg = MachineRegistry::new();
        reg.register(make_test_machine("test1")).unwrap();
        let resp = handle_control_command(&mut reg, "STATUS");
        assert!(resp.contains("Machines: 1"));
    }

    #[test]
    fn test_handle_command_status_specific() {
        let mut reg = MachineRegistry::new();
        reg.register(make_test_machine("test1")).unwrap();
        let resp = handle_control_command(&mut reg, "STATUS test1");
        assert!(resp.contains("Name: test1"));
        assert!(resp.contains("Class: container"));
    }

    #[test]
    fn test_handle_command_status_not_found() {
        let mut reg = MachineRegistry::new();
        let resp = handle_control_command(&mut reg, "STATUS nonexistent");
        assert!(resp.contains("ERROR"));
        assert!(resp.contains("not found"));
    }

    #[test]
    fn test_handle_command_show() {
        let mut reg = MachineRegistry::new();
        reg.register(make_test_machine("test1")).unwrap();
        let resp = handle_control_command(&mut reg, "SHOW test1");
        assert!(resp.contains("Name=test1\n"));
        assert!(resp.contains("Class=container\n"));
    }

    #[test]
    fn test_handle_command_show_missing_name() {
        let mut reg = MachineRegistry::new();
        let resp = handle_control_command(&mut reg, "SHOW");
        assert!(resp.contains("ERROR"));
    }

    #[test]
    fn test_handle_command_register() {
        let mut reg = MachineRegistry::new();
        let resp = handle_control_command(
            &mut reg,
            "REGISTER myvm vm qemu 1234 /var/lib/machines/myvm",
        );
        assert!(resp.contains("OK"));
        assert!(resp.contains("Registered 'myvm'"));
        assert_eq!(reg.count(), 1);

        let m = reg.get("myvm").unwrap();
        assert_eq!(m.class, MachineClass::Vm);
        assert_eq!(m.service, "qemu");
        assert_eq!(m.leader, 1234);
        assert_eq!(m.root_directory, "/var/lib/machines/myvm");
    }

    #[test]
    fn test_handle_command_register_container() {
        let mut reg = MachineRegistry::new();
        let resp = handle_control_command(&mut reg, "REGISTER myc container systemd-nspawn 5678");
        assert!(resp.contains("OK"));

        let m = reg.get("myc").unwrap();
        assert_eq!(m.class, MachineClass::Container);
        assert_eq!(m.root_directory, "/");
    }

    #[test]
    fn test_handle_command_register_invalid_class() {
        let mut reg = MachineRegistry::new();
        let resp = handle_control_command(&mut reg, "REGISTER test invalid svc 1");
        assert!(resp.contains("ERROR"));
        assert!(resp.contains("Invalid class"));
    }

    #[test]
    fn test_handle_command_register_invalid_pid() {
        let mut reg = MachineRegistry::new();
        let resp = handle_control_command(&mut reg, "REGISTER test container svc notanumber");
        assert!(resp.contains("ERROR"));
        assert!(resp.contains("Invalid leader PID"));
    }

    #[test]
    fn test_handle_command_register_too_few_args() {
        let mut reg = MachineRegistry::new();
        let resp = handle_control_command(&mut reg, "REGISTER test container");
        assert!(resp.contains("ERROR"));
        assert!(resp.contains("Usage"));
    }

    #[test]
    fn test_handle_command_register_duplicate() {
        let mut reg = MachineRegistry::new();
        handle_control_command(&mut reg, "REGISTER test1 container svc 1");
        let resp = handle_control_command(&mut reg, "REGISTER test1 container svc 2");
        assert!(resp.contains("ERROR"));
        assert!(resp.contains("already registered"));
    }

    #[test]
    fn test_handle_command_terminate() {
        let mut reg = MachineRegistry::new();
        reg.register(make_test_machine("todel")).unwrap();
        assert_eq!(reg.count(), 1);
        let resp = handle_control_command(&mut reg, "TERMINATE todel");
        assert!(resp.contains("OK"));
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn test_handle_command_terminate_not_found() {
        let mut reg = MachineRegistry::new();
        let resp = handle_control_command(&mut reg, "TERMINATE nonexistent");
        assert!(resp.contains("ERROR"));
        assert!(resp.contains("not found"));
    }

    #[test]
    fn test_handle_command_terminate_missing_name() {
        let mut reg = MachineRegistry::new();
        let resp = handle_control_command(&mut reg, "TERMINATE");
        assert!(resp.contains("ERROR"));
    }

    #[test]
    fn test_handle_command_gc_empty() {
        let mut reg = MachineRegistry::new();
        let resp = handle_control_command(&mut reg, "GC");
        assert!(resp.contains("No stale machines"));
    }

    #[test]
    fn test_handle_command_unknown() {
        let mut reg = MachineRegistry::new();
        let resp = handle_control_command(&mut reg, "FOOBAR");
        assert!(resp.contains("ERROR"));
        assert!(resp.contains("Unknown command"));
    }

    #[test]
    fn test_handle_command_empty() {
        let mut reg = MachineRegistry::new();
        let resp = handle_control_command(&mut reg, "");
        assert!(resp.contains("ERROR"));
    }

    #[test]
    fn test_handle_command_case_insensitive() {
        let mut reg = MachineRegistry::new();
        let resp1 = handle_control_command(&mut reg, "ping");
        assert_eq!(resp1, "PONG\n");
        let resp2 = handle_control_command(&mut reg, "Ping");
        assert_eq!(resp2, "PONG\n");
    }

    // -- is_leader_alive (basic) -------------------------------------------

    #[test]
    fn test_machine_is_leader_alive_self() {
        let mut m = make_test_machine("self");
        m.leader = process::id();
        assert!(m.is_leader_alive());
    }

    #[test]
    fn test_machine_is_leader_alive_zero() {
        let mut m = make_test_machine("zero");
        m.leader = 0;
        assert!(!m.is_leader_alive());
    }

    #[test]
    fn test_machine_is_leader_alive_nonexistent() {
        let mut m = make_test_machine("ghost");
        m.leader = 4_000_000;
        assert!(!m.is_leader_alive());
    }

    // -- Terminate with state file removal ----------------------------------

    #[test]
    fn test_terminate_removes_state_file() {
        let dir = temp_dir();
        let dir_path = dir.path().to_str().unwrap();

        let mut reg = MachineRegistry::new();
        reg.register(make_test_machine("todel")).unwrap();
        reg.save_all_to(dir_path).unwrap();
        assert!(dir.path().join("todel").exists());

        reg.terminate_in("todel", dir_path).unwrap();
        assert!(!dir.path().join("todel").exists());
        assert_eq!(reg.count(), 0);
    }

    // -- Multiple machines --------------------------------------------------

    #[test]
    fn test_registry_multiple_machines() {
        let mut reg = MachineRegistry::new();
        for i in 0..10 {
            let mut m = make_test_machine(&format!("machine{}", i));
            m.leader = 10000 + i;
            reg.register(m).unwrap();
        }
        assert_eq!(reg.count(), 10);

        // List is sorted by name (BTreeMap)
        let names: Vec<&str> = reg.list().iter().map(|m| m.name.as_str()).collect();
        let mut sorted_names = names.clone();
        sorted_names.sort();
        assert_eq!(names, sorted_names);
    }

    // -- Register via control command with scope ----------------------------

    #[test]
    fn test_handle_command_register_with_scope() {
        let mut reg = MachineRegistry::new();
        let resp = handle_control_command(
            &mut reg,
            "REGISTER myvm vm qemu 1234 /var/lib/machines/myvm machine-myvm.scope",
        );
        assert!(resp.contains("OK"));
        let m = reg.get("myvm").unwrap();
        assert_eq!(m.scope, "machine-myvm.scope");
    }
}
