//! systemctl — CLI control tool for the systemd-rs service manager.
//!
//! This is the successor to `rsdctl`. It packs CLI arguments into the
//! JSON-RPC 2.0 format and sends them to the systemd-rs control socket.
//! It reads the response and pretty-prints it.
//!
//! Handles common systemctl flags (stripping them before sending to PID 1):
//!   --no-block, --quiet, --no-wall, --force, --system, --user,
//!   --no-pager, --no-legend, --no-ask-password, --plain, --full,
//!   --show-types, --failed, --all, -a, -f, -q, -l, -t, -p, -n
//!
//! Also handles special commands:
//!   poweroff, reboot, halt  → mapped to "shutdown"
//!   daemon-reload           → mapped to "reload"
//!   try-restart             → forwarded as "try-restart"
//!   reload-or-restart       → forwarded as "reload-or-restart"
//!   condrestart             → alias for "try-restart"
//!   is-active               → checks unit state, exits 0 if active, 3 if not
//!   is-enabled              → checks unit enablement
//!   is-failed               → checks if unit is in failed state
//!   list-dependencies       → show dependency tree for a unit
//!   list-unit-files         → list all unit files on disk with their state
//!   mask                    → symlink unit files to /dev/null
//!   unmask                  → remove /dev/null symlinks for units
//!   disable                 → disable unit(s) (no-op for now)
//!   reset-failed            → clear failed state of unit(s)
//!   kill                    → send signal to a unit's processes
//!   suspend                 → suspend the system (sleep to RAM)
//!   hibernate               → hibernate the system (sleep to disk)
//!   hybrid-sleep            → hybrid sleep (RAM + disk)
//!   suspend-then-hibernate  → suspend first, then hibernate after delay

use serde_json::Value;
use std::io::Write;

use libsystemd::control::jsonrpc2::Call;

/// Flags we recognize and strip from the argument list before sending
/// the command to PID 1. These are common systemctl flags that don't
/// affect the wire protocol.
const KNOWN_FLAGS: &[&str] = &[
    "--no-block",
    "--quiet",
    "--no-wall",
    "--force",
    "--system",
    "--user",
    "--no-pager",
    "--no-legend",
    "--no-ask-password",
    "--plain",
    "--full",
    "--show-types",
    "--failed",
    "--all",
    "--wait",
    "--now",
    "--runtime",
    "--global",
    "--no-reload",
    "--no-warn",
    "--check-inhibitors=auto",
    "--check-inhibitors=yes",
    "--check-inhibitors=no",
];

/// Short flags we recognize and strip.
const KNOWN_SHORT_FLAGS: &[&str] = &["-a", "-f", "-q", "-l"];

/// Short flags that consume the next argument (e.g. `-t service`, `-p MainPID`).
const SHORT_FLAGS_WITH_VALUE: &[&str] = &["-t", "-p", "-n", "-o", "-H", "-M", "-s"];

/// Long flags that consume `=value` or the next argument.
const LONG_FLAGS_WITH_VALUE: &[&str] = &[
    "--type",
    "--property",
    "--lines",
    "--output",
    "--host",
    "--machine",
    "--signal",
    "--kill-mode",
    "--kill-who",
    "--state",
    "--job-mode",
    "--root",
    "--preset-mode",
];

fn main() {
    let mut args: Vec<_> = std::env::args().collect();
    let _exec_name = args.remove(0);

    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_help();
        return;
    }

    if args[0] == "--version" {
        println!("systemctl (systemd-rs) 258");
        return;
    }

    // Determine the control socket address.
    let addr = if let Ok(env_addr) = std::env::var("SYSTEMCTL_ADDR") {
        env_addr
    } else if args.len() >= 2 && (args[0].contains(':') || args[0].starts_with('/')) {
        // First arg looks like an address (host:port or /path/to/socket)
        args.remove(0)
    } else {
        // Default to the systemd-rs control socket
        "/run/systemd/systemd-rs-notify/control.socket".to_owned()
    };

    // Extract known flags and separate them from positional arguments.
    let mut quiet = false;
    let mut positional: Vec<String> = Vec::new();
    let mut property_filter: Vec<String> = Vec::new();
    let mut value_only = false;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        // --value flag (for `show --value -p Prop`)
        if arg == "--value" {
            value_only = true;
            i += 1;
            continue;
        }

        // Check known long flags (exact match).
        if KNOWN_FLAGS.contains(&arg.as_str()) {
            if arg == "--quiet" || arg == "-q" {
                quiet = true;
            }
            i += 1;
            continue;
        }

        // Check known short flags.
        if KNOWN_SHORT_FLAGS.contains(&arg.as_str()) {
            if arg == "-q" {
                quiet = true;
            }
            i += 1;
            continue;
        }

        // Capture -p / --property values for `show` filtering.
        if arg == "-p" || arg == "--property" {
            if i + 1 < args.len() {
                // Value may be comma-separated: -p MainPID,ActiveState
                for part in args[i + 1].split(',') {
                    let trimmed = part.trim();
                    if !trimmed.is_empty() {
                        property_filter.push(trimmed.to_owned());
                    }
                }
            }
            i += 2;
            continue;
        }
        if let Some(rest) = arg.strip_prefix("--property=") {
            for part in rest.split(',') {
                let trimmed = part.trim();
                if !trimmed.is_empty() {
                    property_filter.push(trimmed.to_owned());
                }
            }
            i += 1;
            continue;
        }

        // Check long flags with value (--type=foo or --type foo).
        let mut matched_long = false;
        for flag in LONG_FLAGS_WITH_VALUE {
            if arg == *flag {
                // Consumes next argument.
                i += 2;
                matched_long = true;
                break;
            }
            if arg.starts_with(&format!("{}=", flag)) {
                i += 1;
                matched_long = true;
                break;
            }
        }
        if matched_long {
            continue;
        }

        // Check short flags with value (-t service).
        if SHORT_FLAGS_WITH_VALUE.contains(&arg.as_str()) {
            // Skip this flag and its value.
            i += 2;
            continue;
        }

        // Not a flag — it's a positional argument.
        positional.push(arg.clone());
        i += 1;
    }

    if positional.is_empty() {
        if !quiet {
            eprintln!("Error: no command specified. Run with --help for usage.");
        }
        std::process::exit(1);
    }

    // Extract --signal flag for kill command
    let mut kill_signal: Option<i32> = None;
    {
        let mut i = 0;
        while i < positional.len() {
            if positional[i] == "--signal" || positional[i] == "-s" {
                positional.remove(i);
                if i < positional.len() {
                    if let Ok(sig) = positional[i].parse::<i32>() {
                        kill_signal = Some(sig);
                    } else {
                        // Try signal name mapping
                        kill_signal = match positional[i].as_str() {
                            "SIGTERM" | "TERM" => Some(15),
                            "SIGKILL" | "KILL" => Some(9),
                            "SIGHUP" | "HUP" => Some(1),
                            "SIGINT" | "INT" => Some(2),
                            "SIGUSR1" | "USR1" => Some(10),
                            "SIGUSR2" | "USR2" => Some(12),
                            "SIGCONT" | "CONT" => Some(18),
                            _ => Some(15),
                        };
                    }
                    positional.remove(i);
                }
            } else if let Some(rest) = positional[i].strip_prefix("--signal=") {
                kill_signal = rest.parse::<i32>().ok().or(Some(15));
                positional.remove(i);
            } else {
                i += 1;
            }
        }
    }

    // Extract --reverse flag for list-dependencies
    let mut reverse = false;
    positional.retain(|arg| {
        if arg == "--reverse" {
            reverse = true;
            false
        } else {
            true
        }
    });

    // Map command aliases.
    let command = match positional[0].as_str() {
        "poweroff" | "reboot" | "halt" | "kexec" => {
            positional[0] = "shutdown".to_string();
            &positional[0]
        }
        "daemon-reload" | "daemon-reexec" => {
            positional[0] = "reload".to_string();
            &positional[0]
        }
        "condrestart" => {
            positional[0] = "try-restart".to_string();
            &positional[0]
        }
        "force-reload" => {
            positional[0] = "try-restart".to_string();
            &positional[0]
        }
        // Sleep commands — pass through as-is to PID 1
        "suspend" | "hibernate" | "hybrid-sleep" | "suspend-then-hibernate" => &positional[0],
        // Timer and property commands — pass through
        "list-timers" | "set-property" => &positional[0],
        _ => &positional[0],
    };

    let method = command.clone();
    let params = if method == "list-timers" {
        // list-timers takes no parameters
        None
    } else if method == "set-property" {
        // set-property <unit> <prop=val>...
        if positional.len() < 2 {
            if !quiet {
                eprintln!("Error: set-property requires a unit name.");
            }
            std::process::exit(1);
        }
        let arr: Vec<Value> = positional[1..].iter().cloned().map(Value::String).collect();
        Some(Value::Array(arr))
    } else if method == "list-unit-files" {
        // list-unit-files [--type=TYPE] — optional type filter extracted from -t flag
        // Check if there's a type filter passed as a positional argument
        if positional.len() >= 2 {
            Some(Value::String(positional[1].clone()))
        } else {
            None
        }
    } else if method == "list-dependencies" {
        // list-dependencies <unit> [--reverse]
        if positional.len() < 2 {
            if !quiet {
                eprintln!("Error: list-dependencies requires a unit name.");
            }
            std::process::exit(1);
        }
        let mut arr = vec![Value::String(positional[1].clone())];
        if reverse {
            arr.push(Value::String("--reverse".to_owned()));
        }
        Some(Value::Array(arr))
    } else if method == "kill" {
        // kill <unit> [--signal=SIG]
        if positional.len() < 2 {
            if !quiet {
                eprintln!("Error: kill requires a unit name.");
            }
            std::process::exit(1);
        }
        let mut arr = vec![Value::String(positional[1].clone())];
        if let Some(sig) = kill_signal {
            arr.push(Value::String(sig.to_string()));
        }
        Some(Value::Array(arr))
    } else if method == "suspend"
        || method == "hibernate"
        || method == "hybrid-sleep"
        || method == "suspend-then-hibernate"
    {
        // Sleep commands take no parameters.
        None
    } else if method == "disable" {
        // disable <unit>...
        if positional.len() < 2 {
            if !quiet {
                eprintln!("Error: disable requires at least one unit name.");
            }
            std::process::exit(1);
        }
        if positional.len() == 2 {
            Some(Value::String(positional[1].clone()))
        } else {
            Some(positional[1..].iter().cloned().map(Value::String).collect())
        }
    } else if method == "reset-failed" {
        // reset-failed [unit] — optional unit name
        if positional.len() >= 2 {
            Some(Value::String(positional[1].clone()))
        } else {
            None
        }
    } else if method == "mask" || method == "unmask" {
        // mask/unmask <unit>...
        if positional.len() < 2 {
            if !quiet {
                eprintln!("Error: {} requires at least one unit name.", method);
            }
            std::process::exit(1);
        }
        if positional.len() == 2 {
            Some(Value::String(positional[1].clone()))
        } else {
            Some(positional[1..].iter().cloned().map(Value::String).collect())
        }
    } else if method == "show" {
        // show <unit> [property-filter...] — send unit name + optional filter
        if positional.len() < 2 {
            if !quiet {
                eprintln!("Error: show requires a unit name.");
            }
            std::process::exit(1);
        }
        let mut arr = vec![Value::String(positional[1].clone())];
        for prop in &property_filter {
            arr.push(Value::String(prop.clone()));
        }
        Some(Value::Array(arr))
    } else if positional.len() == 2 {
        Some(Value::String(positional[1].clone()))
    } else if positional.len() > 2 {
        Some(positional[1..].iter().cloned().map(Value::String).collect())
    } else {
        None
    };

    let call = Call {
        method,
        params,
        id: None,
    };
    let str_call = serde_json::to_string(&call.to_json()).unwrap();

    let result = if addr.starts_with('/') {
        send_unix(&addr, &str_call)
    } else {
        send_tcp(&addr, &str_call)
    };

    match result {
        Ok(resp) => {
            handle_response(&positional[0], &resp, quiet, value_only, &property_filter);
        }
        Err(e) => {
            if !quiet {
                eprintln!("Error communicating with systemd-rs: {e}");
            }
            // For is-active, connection failure means the unit is not active.
            if positional[0] == "is-active" {
                if !quiet {
                    println!("inactive");
                }
                std::process::exit(3);
            }
            std::process::exit(1);
        }
    }
}

/// Handle the JSON-RPC response, with special exit code logic for
/// `is-active`, `is-enabled`, and `is-failed`.
fn handle_response(
    command: &str,
    resp: &Value,
    quiet: bool,
    value_only: bool,
    property_filter: &[String],
) {
    // Check for JSON-RPC error responses.
    if let Some(error) = resp.get("error") {
        let message = error
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");

        match command {
            "is-active" => {
                if !quiet {
                    println!("inactive");
                }
                std::process::exit(3);
            }
            "is-enabled" => {
                if !quiet {
                    println!("disabled");
                }
                std::process::exit(1);
            }
            "is-failed" => {
                // Not failed (or unknown) → exit 1
                if !quiet {
                    println!("inactive");
                }
                std::process::exit(1);
            }
            _ => {
                if !quiet {
                    eprintln!("{}", message);
                }
                std::process::exit(1);
            }
        }
    }

    // Success response.
    let result = resp.get("result");

    match command {
        "is-active" => {
            let state = result.and_then(|v| v.as_str()).unwrap_or("active");
            if !quiet {
                println!("{}", state);
            }
            match state {
                "active" | "reloading" => std::process::exit(0),
                _ => std::process::exit(3),
            }
        }
        "is-enabled" => {
            let state = result.and_then(|v| v.as_str()).unwrap_or("enabled");
            if !quiet {
                println!("{}", state);
            }
            match state {
                "enabled" | "enabled-runtime" | "static" | "indirect" | "generated" => {
                    std::process::exit(0)
                }
                _ => std::process::exit(1),
            }
        }
        "is-failed" => {
            let state = result.and_then(|v| v.as_str()).unwrap_or("inactive");
            if !quiet {
                println!("{}", state);
            }
            match state {
                "failed" => std::process::exit(0),
                _ => std::process::exit(1),
            }
        }
        "show" => {
            // The result contains { "show": "Key=Value\n..." }
            if let Some(result) = result
                && let Some(text) = result.get("show").and_then(|v| v.as_str())
                    && !quiet {
                        if value_only && !property_filter.is_empty() {
                            // --value mode: print only the values, one per line
                            for line in text.lines() {
                                if let Some((_key, val)) = line.split_once('=') {
                                    println!("{val}");
                                }
                            }
                        } else {
                            print!("{text}");
                        }
                    }
        }
        "cat" => {
            // The result contains { "cat": "# /path/to/unit\n[Unit]\n..." }
            if let Some(result) = result
                && let Some(text) = result.get("cat").and_then(|v| v.as_str())
                    && !quiet {
                        print!("{text}");
                    }
        }
        "list-unit-files" => {
            if let Some(result) = result
                && let Some(text) = result.get("list-unit-files").and_then(|v| v.as_str())
                    && !quiet {
                        print!("{text}");
                    }
        }
        "disable" => {
            if let Some(result) = result
                && let Some(arr) = result.get("disabled").and_then(|v| v.as_array())
                    && !quiet {
                        for name in arr {
                            if let Some(s) = name.as_str() {
                                println!(
                                    "Removed /etc/systemd/system/multi-user.target.wants/{s}."
                                );
                            }
                        }
                    }
        }
        "reset-failed"
        | "kill"
        | "suspend"
        | "hibernate"
        | "hybrid-sleep"
        | "suspend-then-hibernate"
        | "set-property" => {
            // These return null on success — nothing to print.
        }
        "list-timers" => {
            if let Some(result) = result
                && let Some(arr) = result.as_array()
                    && !quiet {
                        format_timer_table(arr);
                    }
        }
        "list-dependencies" => {
            if let Some(result) = result
                && let Some(text) = result.get("list-dependencies").and_then(|v| v.as_str())
                    && !quiet {
                        print!("{text}");
                    }
        }
        "mask" => {
            if let Some(result) = result
                && let Some(arr) = result.get("masked").and_then(|v| v.as_array())
                    && !quiet {
                        for name in arr {
                            if let Some(s) = name.as_str() {
                                println!("Created symlink /etc/systemd/system/{s} → /dev/null.");
                            }
                        }
                    }
        }
        "unmask" => {
            if let Some(result) = result
                && let Some(arr) = result.get("unmasked").and_then(|v| v.as_array())
                    && !quiet {
                        for name in arr {
                            if let Some(s) = name.as_str() {
                                println!("Removed /etc/systemd/system/{s}.");
                            }
                        }
                    }
        }
        _ => {
            // For all other commands, print the result if non-null and non-empty.
            if !quiet
                && let Some(result) = result {
                    let is_empty =
                        result.is_null() || result.as_array().is_some_and(|a| a.is_empty());
                    if !is_empty {
                        println!("{}", serde_json::to_string_pretty(result).unwrap());
                    }
                }
        }
    }
}

fn format_timer_table(timers: &[Value]) {
    if timers.is_empty() {
        println!("0 timers listed.");
        return;
    }
    // Print header
    println!(
        "{:<40} {:<8} {:<40} TRIGGERS",
        "UNIT", "ACTIVE", "ACTIVATES"
    );
    for timer in timers {
        let unit = timer.get("UNIT").and_then(|v| v.as_str()).unwrap_or("");
        let active = timer.get("ACTIVE").and_then(|v| v.as_str()).unwrap_or("");
        let activates = timer
            .get("ACTIVATES")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let triggers = timer.get("TRIGGERS").and_then(|v| v.as_str()).unwrap_or("");
        println!("{:<40} {:<8} {:<40} {}", unit, active, activates, triggers);
    }
    println!("\n{} timers listed.", timers.len());
}

fn print_help() {
    println!(
        "\
systemctl — control tool for the systemd-rs service manager

Usage:
    systemctl [OPTIONS] <command> [args...]

The control socket defaults to /run/systemd/systemd-rs-notify/control.socket.
You can also set the SYSTEMCTL_ADDR environment variable, or pass a socket
path or TCP address as the first positional argument.

Commands:
    list-units                  List all loaded units
    list-unit-files [TYPE]      List all unit files on disk with their state
    list-dependencies <unit>    Show dependency tree for a unit
    disable <unit>...           Disable one or more units
    reset-failed [unit]         Reset the failed state of a unit (or all)
    kill <unit>                 Send a signal to a unit's processes
    suspend                     Suspend the system
    hibernate                   Hibernate the system
    hybrid-sleep                Hybrid suspend (RAM + disk)
    suspend-then-hibernate      Suspend, then hibernate after a delay
    status <unit>               Show status of a unit
    show <unit>                 Show properties of a unit (key=value format)
    cat <unit>                  Show the unit file source
    start <unit>                Start a unit
    stop <unit>                 Stop a unit
    restart <unit>              Restart a unit
    try-restart <unit>          Restart a unit if it is active
    reload-or-restart <unit>    Reload or restart a unit
    is-active <unit>            Check if a unit is active (exit 0=yes, 3=no)
    is-enabled <unit>           Check if a unit is enabled
    is-failed <unit>            Check if a unit is in failed state
    enable <unit>               Enable (load) a unit
    mask <unit>...              Mask (symlink to /dev/null) one or more units
    unmask <unit>...            Unmask (remove /dev/null symlinks) one or more units
    daemon-reload               Reload the service manager configuration
    poweroff                    Power off the system
    reboot                      Reboot the system
    halt                        Halt the system
    shutdown                    Shut down the service manager

Options:
    --no-block                  Do not wait for the operation to complete
    --quiet, -q                 Suppress output
    --no-wall                   Do not send wall message before shutdown
    --force, -f                 Force the operation
    --no-pager                  Do not pipe output into a pager
    --no-ask-password           Do not ask for password
    --no-legend                 Do not print legend (column headers)
    --system                    Connect to system manager (default)
    --full, -l                  Show full unit names and descriptions
    --all, -a                   Show all units, including inactive
    --reverse                   Show reverse dependencies (for list-dependencies)
    -s, --signal <SIG>          Signal to send (for kill, default: SIGTERM)
    -t, --type <TYPE>           Filter by unit type
    -p, --property <PROP>       Show only specified property (for show)
    --value                     Show only property values (with -p)
    --help, -h                  Show this help
    --version                   Show version

Examples:
    systemctl list-units
    systemctl status sshd.service
    systemctl show sshd.service
    systemctl show -p MainPID,ActiveState sshd.service
    systemctl show --value -p MainPID sshd.service
    systemctl cat sshd.service
    systemctl restart nginx.service
    systemctl --no-block try-restart nscd.service
    systemctl is-active sshd.service
    systemctl list-unit-files
    systemctl list-unit-files service
    systemctl disable tmp.mount
    systemctl reset-failed sshd.service
    systemctl kill --signal=SIGKILL sshd.service
    systemctl suspend
    systemctl hibernate
    systemctl list-dependencies multi-user.target
    systemctl list-dependencies --reverse sshd.service
    systemctl mask tmp.mount
    systemctl unmask tmp.mount"
    );
}

fn send_unix(path: &str, payload: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let mut stream = std::os::unix::net::UnixStream::connect(path)?;
    stream.write_all(payload.as_bytes())?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let resp: Value = serde_json::from_reader(&mut stream)?;
    Ok(resp)
}

fn send_tcp(addr: &str, payload: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let mut stream = std::net::TcpStream::connect(addr)?;
    stream.write_all(payload.as_bytes())?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let resp: Value = serde_json::from_reader(&mut stream)?;
    Ok(resp)
}
