//! systemctl — CLI control tool for the systemd-rs service manager.
//!
//! This is the successor to `rsdctl`. It packs CLI arguments into the
//! JSON-RPC 2.0 format and sends them to the systemd-rs control socket.
//! It reads the response and pretty-prints it.
//!
//! In the future this will grow into a full `systemctl` replacement with
//! all subcommands. For now it provides the same functionality as rsdctl
//! with a systemctl-compatible binary name.

use serde_json::Value;
use std::io::Write;

use libsystemd::control::jsonrpc2::Call;

fn main() {
    let mut args: Vec<_> = std::env::args().collect();
    let _exec_name = args.remove(0);

    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_help();
        return;
    }

    let addr = if let Ok(env_addr) = std::env::var("SYSTEMCTL_ADDR") {
        env_addr
    } else if args.len() >= 2 && (args[0].contains(':') || args[0].starts_with('/')) {
        // First arg looks like an address (host:port or /path/to/socket)
        args.remove(0)
    } else {
        // Default to the systemd-rs control socket
        "/run/systemd/systemd-rs-notify/control.socket".to_owned()
    };

    if args.is_empty() {
        eprintln!("Error: no command specified. Run with --help for usage.");
        std::process::exit(1);
    }

    let params = if args.len() == 2 {
        Some(Value::String(args[1].clone()))
    } else if args.len() > 2 {
        Some(args[1..].iter().cloned().map(Value::String).collect())
    } else {
        None
    };

    let call = Call {
        method: args[0].clone(),
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
            println!("{}", serde_json::to_string_pretty(&resp).unwrap());
        }
        Err(e) => {
            eprintln!("Error communicating with systemd-rs: {e}");
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!(
        "\
systemctl — control tool for the systemd-rs service manager

Usage:
    systemctl [address] <command> [args...]

The address can be a Unix socket path or a TCP host:port.
If omitted, it defaults to /run/systemd/systemd-rs-notify/control.socket.
You can also set the SYSTEMCTL_ADDR environment variable.

Commands:
    list-units                  List all loaded units
    status <unit>               Show status of a unit
    start <unit>                Start a unit
    stop <unit>                 Stop a unit
    restart <unit>              Restart a unit
    start-all                   Start all loaded units
    stop-all                    Stop all units
    load-new                    Load a new unit file
    load-all-new                Load all new unit files
    shutdown                    Shut down the service manager

Examples:
    systemctl list-units
    systemctl status sshd.service
    systemctl restart nginx.service
    systemctl /run/systemd/systemd-rs-notify/control.socket list-units
    systemctl 0.0.0.0:8080 restart test.service"
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
