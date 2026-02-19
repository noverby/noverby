//! machinectl — query and manage VMs and containers
//!
//! This is a Rust implementation of systemd's `machinectl` command. It
//! communicates with `systemd-machined` via the control socket at
//! `/run/systemd/machined-control` to list, inspect, and manage registered
//! virtual machines and containers.
//!
//! ## Supported commands
//!
//! - `list` — list registered machines
//! - `status <name>` — show machine status
//! - `show <name>` — show machine properties (key=value)
//! - `terminate <name>` — terminate a machine
//! - `poweroff <name>` — alias for terminate
//! - `reboot <name>` — terminate + note (reboot semantics need nspawn)
//! - `kill <name> [--signal=SIG]` — send signal to machine leader
//! - `login <name>` — (stub) login to a container
//! - `shell <name>` — (stub) open a shell in a container
//! - `image list` — (stub) list machine images
//! - `clean` — (stub) clean up hidden/stale machine state
//!
//! ## Missing
//!
//! - D-Bus interface (org.freedesktop.machine1)
//! - Image management (clone/rename/remove/set-limit/import/export/pull)
//! - PTY forwarding for login/shell
//! - copy-to/copy-from operations
//! - bind/bind-user mount operations
//! - enable/disable image management

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process;

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

const CONTROL_SOCKET_PATH: &str = "/run/systemd/machined-control";
const MACHINES_DIR: &str = "/run/systemd/machines";

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Args {
    command: Command,
    machine: Option<String>,
    property: Option<String>,
    signal: String,
    no_pager: bool,
    no_legend: bool,
    no_ask_password: bool,
    host: Option<String>,
    all: bool,
    value: bool,
    full: bool,
    max_addresses: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
enum Command {
    List,
    Status,
    Show,
    Terminate,
    Poweroff,
    Reboot,
    Kill,
    Login,
    Shell,
    ImageList,
    Clean,
    Help,
    ShowImage,
    CatImage,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            command: Command::List,
            machine: None,
            property: None,
            signal: "SIGTERM".to_string(),
            no_pager: false,
            no_legend: false,
            no_ask_password: false,
            host: None,
            all: false,
            value: false,
            full: false,
            max_addresses: None,
        }
    }
}

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let mut args = Args::default();
    let mut positional: Vec<String> = Vec::new();
    let mut i = 0;

    while i < argv.len() {
        let arg = &argv[i];
        match arg.as_str() {
            "--help" | "-h" => {
                args.command = Command::Help;
                return Ok(args);
            }
            "--no-pager" => args.no_pager = true,
            "--no-legend" => args.no_legend = true,
            "--no-ask-password" => args.no_ask_password = true,
            "--all" | "-a" => args.all = true,
            "--value" => args.value = true,
            "--full" | "-l" => args.full = true,
            _ if arg.starts_with("--signal=") => {
                args.signal = arg.strip_prefix("--signal=").unwrap().to_string();
            }
            _ if arg == "--signal" || arg == "-s" => {
                i += 1;
                if i >= argv.len() {
                    return Err("--signal requires a value".to_string());
                }
                args.signal = argv[i].clone();
            }
            _ if arg.starts_with("--property=") || arg.starts_with("-p=") => {
                let val = if let Some(v) = arg.strip_prefix("--property=") {
                    v
                } else {
                    arg.strip_prefix("-p=").unwrap()
                };
                args.property = Some(val.to_string());
            }
            "-p" | "--property" => {
                i += 1;
                if i >= argv.len() {
                    return Err("--property requires a value".to_string());
                }
                args.property = Some(argv[i].clone());
            }
            _ if arg.starts_with("--host=") || arg.starts_with("-H=") => {
                let val = if let Some(v) = arg.strip_prefix("--host=") {
                    v
                } else {
                    arg.strip_prefix("-H=").unwrap()
                };
                args.host = Some(val.to_string());
            }
            "-H" | "--host" => {
                i += 1;
                if i >= argv.len() {
                    return Err("--host requires a value".to_string());
                }
                args.host = Some(argv[i].clone());
            }
            _ if arg.starts_with("--max-addresses=") => {
                let val = arg.strip_prefix("--max-addresses=").unwrap();
                match val.parse::<usize>() {
                    Ok(n) => args.max_addresses = Some(n),
                    Err(_) => return Err(format!("Invalid --max-addresses value: {}", val)),
                }
            }
            "--max-addresses" => {
                i += 1;
                if i >= argv.len() {
                    return Err("--max-addresses requires a value".to_string());
                }
                match argv[i].parse::<usize>() {
                    Ok(n) => args.max_addresses = Some(n),
                    Err(_) => return Err(format!("Invalid --max-addresses value: {}", argv[i])),
                }
            }
            // Skip known but unused flags
            "--quiet" | "-q" | "--mkdir" | "--read-only" => {}
            _ if arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 1 => {
                // Unknown short option — ignore
            }
            _ if arg.starts_with("--") => {
                // Unknown long option — ignore
            }
            _ => {
                positional.push(arg.clone());
            }
        }
        i += 1;
    }

    // Parse command from positional args
    if positional.is_empty() {
        args.command = Command::List;
        return Ok(args);
    }

    let cmd = positional[0].as_str();
    match cmd {
        "list" => {
            args.command = Command::List;
        }
        "status" => {
            args.command = Command::Status;
            args.machine = positional.get(1).cloned();
        }
        "show" => {
            args.command = Command::Show;
            args.machine = positional.get(1).cloned();
        }
        "terminate" => {
            args.command = Command::Terminate;
            args.machine = positional.get(1).cloned();
        }
        "poweroff" => {
            args.command = Command::Poweroff;
            args.machine = positional.get(1).cloned();
        }
        "reboot" => {
            args.command = Command::Reboot;
            args.machine = positional.get(1).cloned();
        }
        "kill" => {
            args.command = Command::Kill;
            args.machine = positional.get(1).cloned();
        }
        "login" => {
            args.command = Command::Login;
            args.machine = positional.get(1).cloned();
        }
        "shell" => {
            args.command = Command::Shell;
            args.machine = positional.get(1).cloned();
        }
        "clean" => {
            args.command = Command::Clean;
        }
        "list-images" => {
            args.command = Command::ImageList;
        }
        "image" => match positional.get(1).map(|s| s.as_str()) {
            Some("show") | Some("status") => {
                args.command = Command::ShowImage;
                args.machine = positional.get(2).cloned();
            }
            Some("cat") => {
                args.command = Command::CatImage;
                args.machine = positional.get(2).cloned();
            }
            _ => {
                args.command = Command::ImageList;
            }
        },
        "help" => {
            args.command = Command::Help;
        }
        _ => {
            return Err(format!("Unknown command '{}'", cmd));
        }
    }

    Ok(args)
}

// ---------------------------------------------------------------------------
// Machine data model (for offline reading)
// ---------------------------------------------------------------------------

/// A machine parsed from a state file in /run/systemd/machines/.
#[derive(Debug, Clone, Default)]
struct MachineInfo {
    name: String,
    class: String,
    service: String,
    scope: String,
    leader: u32,
    root_directory: String,
    state: String,
    timestamp: u64,
    netif: Vec<u32>,
}

impl MachineInfo {
    fn from_state_file(name: &str, content: &str) -> Self {
        let fields = parse_env_content(content);
        MachineInfo {
            name: fields
                .get("NAME")
                .cloned()
                .unwrap_or_else(|| name.to_string()),
            class: fields
                .get("CLASS")
                .cloned()
                .unwrap_or_else(|| "container".to_string()),
            service: fields.get("SERVICE").cloned().unwrap_or_default(),
            scope: fields.get("SCOPE").cloned().unwrap_or_default(),
            leader: fields
                .get("LEADER")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            root_directory: fields
                .get("ROOT")
                .cloned()
                .unwrap_or_else(|| "/".to_string()),
            state: fields
                .get("STATE")
                .cloned()
                .unwrap_or_else(|| "running".to_string()),
            timestamp: fields
                .get("TIMESTAMP")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            netif: fields
                .get("NETIF")
                .map(|s| {
                    s.split_whitespace()
                        .filter_map(|n| n.parse().ok())
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

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
            if value.len() >= 2
                && ((value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\'')))
            {
                value = value[1..value.len() - 1].to_string();
            }
            map.insert(key.to_string(), value);
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Communication with machined
// ---------------------------------------------------------------------------

/// Send a command to the machined control socket and return the response.
fn send_command(cmd: &str) -> Result<String, String> {
    let mut stream = UnixStream::connect(CONTROL_SOCKET_PATH).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound || e.kind() == io::ErrorKind::ConnectionRefused {
            "Failed to connect to systemd-machined. Is the service running?".to_string()
        } else {
            format!("Failed to connect to systemd-machined: {}", e)
        }
    })?;

    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;

    stream
        .write_all(cmd.as_bytes())
        .map_err(|e| format!("Failed to send command: {}", e))?;
    stream
        .write_all(b"\n")
        .map_err(|e| format!("Failed to send newline: {}", e))?;
    stream
        .flush()
        .map_err(|e| format!("Failed to flush: {}", e))?;

    // Signal we're done writing
    let _ = stream.shutdown(Shutdown::Write);

    let mut response = String::new();
    let reader = BufReader::new(&stream);
    for line in reader.lines() {
        match line {
            Ok(l) => {
                if !response.is_empty() {
                    response.push('\n');
                }
                response.push_str(&l);
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(_) => break,
        }
    }

    Ok(response)
}

// ---------------------------------------------------------------------------
// Offline machine reading (fallback when daemon is not running)
// ---------------------------------------------------------------------------

/// Read machines directly from state files when the daemon is unavailable.
fn read_machines_offline() -> Vec<MachineInfo> {
    let mut machines = Vec::new();
    let entries = match fs::read_dir(MACHINES_DIR) {
        Ok(e) => e,
        Err(_) => return machines,
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
            machines.push(MachineInfo::from_state_file(&fname, &content));
        }
    }

    machines.sort_by(|a, b| a.name.cmp(&b.name));
    machines
}

/// Read a single machine from its state file.
fn read_machine_offline(name: &str) -> Option<MachineInfo> {
    let path = Path::new(MACHINES_DIR).join(name);
    let content = fs::read_to_string(&path).ok()?;
    Some(MachineInfo::from_state_file(name, &content))
}

// ---------------------------------------------------------------------------
// Timestamp formatting
// ---------------------------------------------------------------------------

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
    let mon = if (1..=12).contains(&month) {
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
// Command implementations
// ---------------------------------------------------------------------------

fn cmd_list(args: &Args) -> i32 {
    // Try daemon first, fall back to offline
    match send_command("LIST") {
        Ok(resp) => {
            print!("{}", resp);
            0
        }
        Err(_) => {
            let machines = read_machines_offline();
            if machines.is_empty() {
                if !args.no_legend {
                    println!("No machines.");
                }
                return 0;
            }
            if !args.no_legend {
                println!(
                    "{:<32} {:>10} {:>12} {:>10}",
                    "MACHINE", "CLASS", "SERVICE", "STATE"
                );
            }
            for m in &machines {
                println!(
                    "{:<32} {:>10} {:>12} {:>10}",
                    m.name, m.class, m.service, m.state
                );
            }
            if !args.no_legend {
                println!("\n{} machines listed.", machines.len());
            }
            0
        }
    }
}

fn cmd_status(args: &Args) -> i32 {
    let name = match &args.machine {
        Some(n) => n,
        None => {
            // No machine specified — show global status
            match send_command("STATUS") {
                Ok(resp) => {
                    print!("{}", resp);
                    return 0;
                }
                Err(_) => {
                    let machines = read_machines_offline();
                    println!("Machines: {}", machines.len());
                    return 0;
                }
            }
        }
    };

    match send_command(&format!("STATUS {}", name)) {
        Ok(resp) => {
            if resp.starts_with("ERROR") {
                eprintln!("{}", resp.trim());
                return 1;
            }
            print!("{}", resp);
            0
        }
        Err(_) => {
            // Offline fallback
            match read_machine_offline(name) {
                Some(m) => {
                    println!("       Name: {}", m.name);
                    println!("      Class: {}", m.class);
                    println!("    Service: {}", m.service);
                    println!("      Scope: {}", m.scope);
                    println!("     Leader: {}", m.leader);
                    println!("       Root: {}", m.root_directory);
                    println!("      State: {}", m.state);
                    println!("      Since: {}", format_timestamp(m.timestamp));
                    if !m.netif.is_empty() {
                        let nifs: Vec<String> = m.netif.iter().map(|n| n.to_string()).collect();
                        println!("      NetIf: {}", nifs.join(" "));
                    }
                    0
                }
                None => {
                    eprintln!("Machine '{}' not found", name);
                    1
                }
            }
        }
    }
}

fn cmd_show(args: &Args) -> i32 {
    let name = match &args.machine {
        Some(n) => n,
        None => {
            eprintln!("Machine name required");
            return 1;
        }
    };

    match send_command(&format!("SHOW {}", name)) {
        Ok(resp) => {
            if resp.starts_with("ERROR") {
                eprintln!("{}", resp.trim());
                return 1;
            }
            // Apply property filter
            if let Some(ref prop) = args.property {
                for line in resp.lines() {
                    if let Some((key, value)) = line.split_once('=')
                        && key == prop
                    {
                        if args.value {
                            println!("{}", value);
                        } else {
                            println!("{}", line);
                        }
                    }
                }
            } else {
                print!("{}", resp);
            }
            0
        }
        Err(_) => {
            match read_machine_offline(name) {
                Some(m) => {
                    let pairs = [
                        ("Name", m.name.as_str()),
                        ("Class", m.class.as_str()),
                        ("Service", m.service.as_str()),
                        ("Scope", m.scope.as_str()),
                        ("RootDirectory", m.root_directory.as_str()),
                        ("State", m.state.as_str()),
                    ];
                    let leader_s = m.leader.to_string();
                    let ts_s = m.timestamp.to_string();

                    for (key, value) in &pairs {
                        if let Some(ref prop) = args.property
                            && *key != prop.as_str()
                        {
                            continue;
                        }
                        if args.value {
                            println!("{}", value);
                        } else {
                            println!("{}={}", key, value);
                        }
                    }
                    // Leader and Timestamp as separate entries
                    let extras = [("Leader", leader_s.as_str()), ("Timestamp", ts_s.as_str())];
                    for (key, value) in &extras {
                        if let Some(ref prop) = args.property
                            && *key != prop.as_str()
                        {
                            continue;
                        }
                        if args.value {
                            println!("{}", value);
                        } else {
                            println!("{}={}", key, value);
                        }
                    }
                    0
                }
                None => {
                    eprintln!("Machine '{}' not found", name);
                    1
                }
            }
        }
    }
}

fn cmd_terminate(args: &Args) -> i32 {
    let name = match &args.machine {
        Some(n) => n,
        None => {
            eprintln!("Machine name required");
            return 1;
        }
    };

    match send_command(&format!("TERMINATE {}", name)) {
        Ok(resp) => {
            if resp.starts_with("ERROR") {
                eprintln!("{}", resp.trim());
                1
            } else {
                0
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}

fn cmd_kill(args: &Args) -> i32 {
    let name = match &args.machine {
        Some(n) => n,
        None => {
            eprintln!("Machine name required");
            return 1;
        }
    };

    // Read the machine's leader PID and send the signal directly
    match read_machine_offline(name) {
        Some(m) => {
            if m.leader == 0 {
                eprintln!("Machine '{}' has no leader PID", name);
                return 1;
            }
            let signum = parse_signal(&args.signal);
            let ret = unsafe { libc::kill(m.leader as i32, signum) };
            if ret != 0 {
                eprintln!(
                    "Failed to send signal {} to PID {}: {}",
                    args.signal,
                    m.leader,
                    io::Error::last_os_error()
                );
                1
            } else {
                0
            }
        }
        None => {
            eprintln!("Machine '{}' not found", name);
            1
        }
    }
}

fn parse_signal(sig: &str) -> i32 {
    // Try numeric first
    if let Ok(n) = sig.parse::<i32>() {
        return n;
    }
    let s = sig.to_uppercase();
    let s = s.strip_prefix("SIG").unwrap_or(&s);
    match s {
        "HUP" => libc::SIGHUP,
        "INT" => libc::SIGINT,
        "QUIT" => libc::SIGQUIT,
        "ILL" => libc::SIGILL,
        "TRAP" => libc::SIGTRAP,
        "ABRT" | "IOT" => libc::SIGABRT,
        "BUS" => libc::SIGBUS,
        "FPE" => libc::SIGFPE,
        "KILL" => libc::SIGKILL,
        "USR1" => libc::SIGUSR1,
        "SEGV" => libc::SIGSEGV,
        "USR2" => libc::SIGUSR2,
        "PIPE" => libc::SIGPIPE,
        "ALRM" => libc::SIGALRM,
        "TERM" => libc::SIGTERM,
        "CHLD" | "CLD" => libc::SIGCHLD,
        "CONT" => libc::SIGCONT,
        "STOP" => libc::SIGSTOP,
        "TSTP" => libc::SIGTSTP,
        "TTIN" => libc::SIGTTIN,
        "TTOU" => libc::SIGTTOU,
        "URG" => libc::SIGURG,
        "XCPU" => libc::SIGXCPU,
        "XFSZ" => libc::SIGXFSZ,
        "VTALRM" => libc::SIGVTALRM,
        "PROF" => libc::SIGPROF,
        "WINCH" => libc::SIGWINCH,
        "IO" | "POLL" => libc::SIGIO,
        "SYS" => libc::SIGSYS,
        _ => libc::SIGTERM,
    }
}

fn cmd_clean() -> i32 {
    match send_command("GC") {
        Ok(resp) => {
            print!("{}", resp);
            0
        }
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}

fn cmd_image_list() -> i32 {
    // Stub — list machine images from /var/lib/machines
    let image_dir = Path::new("/var/lib/machines");
    if !image_dir.exists() {
        println!("No machine images found.");
        return 0;
    }

    let entries = match fs::read_dir(image_dir) {
        Ok(e) => e,
        Err(_) => {
            println!("No machine images found.");
            return 0;
        }
    };

    let mut images: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        images.push(name);
    }

    if images.is_empty() {
        println!("No machine images found.");
        return 0;
    }

    images.sort();
    println!("{:<32} {:>10} {:>10}", "NAME", "TYPE", "RO");
    for name in &images {
        let path = image_dir.join(name);
        let img_type = if path.is_dir() { "directory" } else { "raw" };
        let ro = if path
            .metadata()
            .map(|m| m.permissions().readonly())
            .unwrap_or(false)
        {
            "ro"
        } else {
            "no"
        };
        println!("{:<32} {:>10} {:>10}", name, img_type, ro);
    }
    println!("\n{} images listed.", images.len());
    0
}

// ---------------------------------------------------------------------------
// Usage
// ---------------------------------------------------------------------------

fn print_usage() {
    println!("machinectl [OPTIONS...] COMMAND ...");
    println!();
    println!("Send control commands to or query the virtual machine and container");
    println!("registration manager.");
    println!();
    println!("Machine Commands:");
    println!("  list                        List running VMs and containers");
    println!("  status NAME...              Show machine status");
    println!("  show [NAME...]              Show properties of one or more machines");
    println!("  terminate NAME...           Terminate one or more machines");
    println!("  kill NAME...                Send signal to processes of a machine");
    println!("  poweroff NAME...            Power off one or more machines");
    println!("  reboot NAME...              Reboot one or more machines");
    println!("  login [NAME]                Get a login prompt in a container");
    println!("  shell [[USER@]NAME]         Invoke a shell in a container");
    println!("  clean                       Clean up stale machine state");
    println!();
    println!("Image Commands:");
    println!("  list-images                 List machine images");
    println!();
    println!("Options:");
    println!("  -h --help                   Show this help");
    println!("  -p --property=NAME          Show only properties by this name");
    println!("  -a --all                    Show all properties, including empty ones");
    println!("     --value                  When showing properties, only print the value");
    println!("  -l --full                   Do not ellipsize output");
    println!("     --no-pager               Do not pipe output into a pager");
    println!("     --no-legend              Do not show the headers and footers");
    println!("     --no-ask-password        Do not ask for system passwords");
    println!("  -s --signal=SIGNAL          Signal to send with kill (default: SIGTERM)");
    println!("  -H --host=[USER@]HOST       Operate on remote host");
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn run(argv: &[String]) -> i32 {
    let args = match parse_args(argv) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("machinectl: {}", e);
            return 1;
        }
    };

    match args.command {
        Command::List => cmd_list(&args),
        Command::Status => cmd_status(&args),
        Command::Show => cmd_show(&args),
        Command::Terminate | Command::Poweroff | Command::Reboot => cmd_terminate(&args),
        Command::Kill => cmd_kill(&args),
        Command::Clean => cmd_clean(),
        Command::ImageList => cmd_image_list(),
        Command::Login => {
            eprintln!("machinectl login: not yet implemented (requires PTY forwarding)");
            1
        }
        Command::Shell => {
            eprintln!("machinectl shell: not yet implemented (requires PTY forwarding)");
            1
        }
        Command::ShowImage | Command::CatImage => {
            eprintln!("machinectl image: not yet implemented");
            1
        }
        Command::Help => {
            print_usage();
            0
        }
    }
}

fn main() {
    let argv: Vec<String> = env::args().skip(1).collect();
    let code = run(&argv);
    process::exit(code);
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        if s.is_empty() {
            return Vec::new();
        }
        s.split_whitespace().map(|x| x.to_string()).collect()
    }

    // -- parse_args ---------------------------------------------------------

    #[test]
    fn test_parse_args_empty() {
        let a = parse_args(&args("")).unwrap();
        assert_eq!(a.command, Command::List);
        assert!(a.machine.is_none());
    }

    #[test]
    fn test_parse_args_list() {
        let a = parse_args(&args("list")).unwrap();
        assert_eq!(a.command, Command::List);
    }

    #[test]
    fn test_parse_args_status() {
        let a = parse_args(&args("status myvm")).unwrap();
        assert_eq!(a.command, Command::Status);
        assert_eq!(a.machine.as_deref(), Some("myvm"));
    }

    #[test]
    fn test_parse_args_status_no_machine() {
        let a = parse_args(&args("status")).unwrap();
        assert_eq!(a.command, Command::Status);
        assert!(a.machine.is_none());
    }

    #[test]
    fn test_parse_args_show() {
        let a = parse_args(&args("show myvm")).unwrap();
        assert_eq!(a.command, Command::Show);
        assert_eq!(a.machine.as_deref(), Some("myvm"));
    }

    #[test]
    fn test_parse_args_show_with_property() {
        let a = parse_args(&args("show myvm -p Name")).unwrap();
        assert_eq!(a.command, Command::Show);
        assert_eq!(a.property.as_deref(), Some("Name"));
    }

    #[test]
    fn test_parse_args_show_with_property_eq() {
        let a = parse_args(&args("show myvm --property=Class")).unwrap();
        assert_eq!(a.property.as_deref(), Some("Class"));
    }

    #[test]
    fn test_parse_args_terminate() {
        let a = parse_args(&args("terminate myvm")).unwrap();
        assert_eq!(a.command, Command::Terminate);
        assert_eq!(a.machine.as_deref(), Some("myvm"));
    }

    #[test]
    fn test_parse_args_poweroff() {
        let a = parse_args(&args("poweroff myvm")).unwrap();
        assert_eq!(a.command, Command::Poweroff);
    }

    #[test]
    fn test_parse_args_reboot() {
        let a = parse_args(&args("reboot myvm")).unwrap();
        assert_eq!(a.command, Command::Reboot);
    }

    #[test]
    fn test_parse_args_kill() {
        let a = parse_args(&args("kill myvm")).unwrap();
        assert_eq!(a.command, Command::Kill);
        assert_eq!(a.signal, "SIGTERM");
    }

    #[test]
    fn test_parse_args_kill_with_signal() {
        let a = parse_args(&args("kill myvm --signal=SIGKILL")).unwrap();
        assert_eq!(a.signal, "SIGKILL");
    }

    #[test]
    fn test_parse_args_kill_with_signal_short() {
        let a = parse_args(&args("kill myvm -s SIGHUP")).unwrap();
        assert_eq!(a.signal, "SIGHUP");
    }

    #[test]
    fn test_parse_args_login() {
        let a = parse_args(&args("login myvm")).unwrap();
        assert_eq!(a.command, Command::Login);
    }

    #[test]
    fn test_parse_args_shell() {
        let a = parse_args(&args("shell myvm")).unwrap();
        assert_eq!(a.command, Command::Shell);
    }

    #[test]
    fn test_parse_args_clean() {
        let a = parse_args(&args("clean")).unwrap();
        assert_eq!(a.command, Command::Clean);
    }

    #[test]
    fn test_parse_args_list_images() {
        let a = parse_args(&args("list-images")).unwrap();
        assert_eq!(a.command, Command::ImageList);
    }

    #[test]
    fn test_parse_args_image_list() {
        let a = parse_args(&args("image list")).unwrap();
        assert_eq!(a.command, Command::ImageList);
    }

    #[test]
    fn test_parse_args_help() {
        let a = parse_args(&args("--help")).unwrap();
        assert_eq!(a.command, Command::Help);
    }

    #[test]
    fn test_parse_args_help_command() {
        let a = parse_args(&args("help")).unwrap();
        assert_eq!(a.command, Command::Help);
    }

    #[test]
    fn test_parse_args_no_pager() {
        let a = parse_args(&args("list --no-pager")).unwrap();
        assert!(a.no_pager);
    }

    #[test]
    fn test_parse_args_no_legend() {
        let a = parse_args(&args("list --no-legend")).unwrap();
        assert!(a.no_legend);
    }

    #[test]
    fn test_parse_args_all() {
        let a = parse_args(&args("show -a myvm")).unwrap();
        assert!(a.all);
    }

    #[test]
    fn test_parse_args_value() {
        let a = parse_args(&args("show --value myvm")).unwrap();
        assert!(a.value);
    }

    #[test]
    fn test_parse_args_full() {
        let a = parse_args(&args("list -l")).unwrap();
        assert!(a.full);
    }

    #[test]
    fn test_parse_args_unknown_command() {
        let result = parse_args(&args("nonexistent"));
        assert!(result.is_err());
    }

    // -- parse_signal -------------------------------------------------------

    #[test]
    fn test_parse_signal_numeric() {
        assert_eq!(parse_signal("9"), 9);
        assert_eq!(parse_signal("15"), 15);
    }

    #[test]
    fn test_parse_signal_named() {
        assert_eq!(parse_signal("SIGTERM"), libc::SIGTERM);
        assert_eq!(parse_signal("SIGKILL"), libc::SIGKILL);
        assert_eq!(parse_signal("SIGHUP"), libc::SIGHUP);
        assert_eq!(parse_signal("SIGINT"), libc::SIGINT);
        assert_eq!(parse_signal("SIGUSR1"), libc::SIGUSR1);
        assert_eq!(parse_signal("SIGUSR2"), libc::SIGUSR2);
    }

    #[test]
    fn test_parse_signal_without_prefix() {
        assert_eq!(parse_signal("TERM"), libc::SIGTERM);
        assert_eq!(parse_signal("KILL"), libc::SIGKILL);
        assert_eq!(parse_signal("HUP"), libc::SIGHUP);
    }

    #[test]
    fn test_parse_signal_case_insensitive() {
        assert_eq!(parse_signal("sigterm"), libc::SIGTERM);
        assert_eq!(parse_signal("Sigkill"), libc::SIGKILL);
        assert_eq!(parse_signal("term"), libc::SIGTERM);
    }

    #[test]
    fn test_parse_signal_unknown_defaults_to_sigterm() {
        assert_eq!(parse_signal("INVALID"), libc::SIGTERM);
    }

    // -- parse_env_content --------------------------------------------------

    #[test]
    fn test_parse_env_content_basic() {
        let m = parse_env_content("KEY=value\nFOO=bar\n");
        assert_eq!(m.get("KEY").unwrap(), "value");
        assert_eq!(m.get("FOO").unwrap(), "bar");
    }

    #[test]
    fn test_parse_env_content_quoted() {
        let m = parse_env_content("KEY=\"hello world\"\n");
        assert_eq!(m.get("KEY").unwrap(), "hello world");
    }

    #[test]
    fn test_parse_env_content_comments_blanks() {
        let m = parse_env_content("# comment\n\nKEY=value\n");
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn test_parse_env_content_empty() {
        let m = parse_env_content("");
        assert!(m.is_empty());
    }

    // -- MachineInfo --------------------------------------------------------

    #[test]
    fn test_machine_info_from_state_file() {
        let content = "NAME=test\nCLASS=container\nSERVICE=systemd-nspawn\nLEADER=1234\nROOT=/\nSTATE=running\nTIMESTAMP=1700000000000000\nNETIF=3 7\n";
        let m = MachineInfo::from_state_file("test", content);
        assert_eq!(m.name, "test");
        assert_eq!(m.class, "container");
        assert_eq!(m.service, "systemd-nspawn");
        assert_eq!(m.leader, 1234);
        assert_eq!(m.root_directory, "/");
        assert_eq!(m.state, "running");
        assert_eq!(m.timestamp, 1700000000000000);
        assert_eq!(m.netif, vec![3, 7]);
    }

    #[test]
    fn test_machine_info_from_state_file_minimal() {
        let content = "CLASS=vm\n";
        let m = MachineInfo::from_state_file("myvm", content);
        assert_eq!(m.name, "myvm"); // Falls back to passed name
        assert_eq!(m.class, "vm");
        assert_eq!(m.leader, 0);
        assert_eq!(m.root_directory, "/");
    }

    // -- format_timestamp / days_to_ymd ------------------------------------

    #[test]
    fn test_format_timestamp_zero() {
        assert_eq!(format_timestamp(0), "n/a");
    }

    #[test]
    fn test_format_timestamp_known() {
        let ts = format_timestamp(1700000000_000000);
        assert!(ts.contains("2023"));
        assert!(ts.contains("Nov"));
    }

    #[test]
    fn test_days_to_ymd_epoch() {
        let (y, m, d) = days_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_days_to_ymd_y2k() {
        let (y, m, d) = days_to_ymd(10957);
        assert_eq!((y, m, d), (2000, 1, 1));
    }

    // -- run (with stubs for commands that need daemon) ---------------------

    #[test]
    fn test_run_help() {
        assert_eq!(run(&args("--help")), 0);
    }

    #[test]
    fn test_run_help_command() {
        assert_eq!(run(&args("help")), 0);
    }

    #[test]
    fn test_run_unknown_command() {
        assert_eq!(run(&args("totally_invalid_command")), 1);
    }

    #[test]
    fn test_run_login_stub() {
        assert_eq!(run(&args("login myvm")), 1);
    }

    #[test]
    fn test_run_shell_stub() {
        assert_eq!(run(&args("shell myvm")), 1);
    }

    // -- read_machines_offline (no state dir) --------------------------------

    #[test]
    fn test_read_machines_offline_empty() {
        // If there's no /run/systemd/machines or it's empty, should return empty
        let machines = read_machines_offline();
        // This is environment-dependent; just check it doesn't panic
        let _ = machines;
    }

    // -- cmd_list offline fallback ------------------------------------------

    #[test]
    fn test_cmd_list_no_legend() {
        let a = Args {
            command: Command::List,
            no_legend: true,
            ..Args::default()
        };
        // Should not panic even if daemon is unavailable
        let _ = cmd_list(&a);
    }
}
