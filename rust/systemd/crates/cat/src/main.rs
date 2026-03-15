//! systemd-cat — Connect a pipeline or program's output to the journal.
//!
//! A drop-in replacement for `systemd-cat(1)`. This tool connects
//! stdout and stderr of a command (or stdin if no command is given)
//! to the systemd journal using the native journal socket protocol.
//!
//! Supported options:
//!
//! - `-t`, `--identifier=ID`  — Set the syslog identifier (default: "unknown")
//! - `-p`, `--priority=PRIO`  — Set the default log priority for stdout
//! - `--stderr-priority=PRIO` — Set the log priority for stderr
//! - `--level-prefix=BOOL`    — Strip kernel-style `<N>` priority prefixes from lines

use clap::Parser;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixDatagram;
use std::process;

/// The path to the native systemd journal socket.
const JOURNAL_SOCKET: &str = "/run/systemd/journal/socket";

/// Syslog priority levels (matching <syslog.h> and systemd conventions).
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
enum Priority {
    Emergency = 0,
    Alert = 1,
    Critical = 2,
    Error = 3,
    Warning = 4,
    Notice = 5,
    Info = 6,
    Debug = 7,
}

impl Priority {
    fn from_str_or_num(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "emerg" | "emergency" | "0" => Ok(Priority::Emergency),
            "alert" | "1" => Ok(Priority::Alert),
            "crit" | "critical" | "2" => Ok(Priority::Critical),
            "err" | "error" | "3" => Ok(Priority::Error),
            "warning" | "warn" | "4" => Ok(Priority::Warning),
            "notice" | "5" => Ok(Priority::Notice),
            "info" | "6" => Ok(Priority::Info),
            "debug" | "7" => Ok(Priority::Debug),
            _ => Err(format!("Unknown priority: {s}")),
        }
    }

    fn as_u8(self) -> u8 {
        self as u8
    }

    fn from_kernel_prefix(n: u8) -> Option<Self> {
        match n {
            0 => Some(Priority::Emergency),
            1 => Some(Priority::Alert),
            2 => Some(Priority::Critical),
            3 => Some(Priority::Error),
            4 => Some(Priority::Warning),
            5 => Some(Priority::Notice),
            6 => Some(Priority::Info),
            7 => Some(Priority::Debug),
            _ => None,
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "systemd-cat",
    about = "Connect a pipeline or program's output to the journal",
    version
)]
struct Cli {
    /// Set the syslog identifier string. Defaults to the name of the
    /// executed command, or "unknown" if reading from stdin.
    #[arg(short = 't', long = "identifier", value_name = "ID")]
    identifier: Option<String>,

    /// Set the default priority for stdout messages.
    /// Accepts numeric (0-7) or name (emerg, alert, crit, err, warning,
    /// notice, info, debug).
    #[arg(short, long, default_value = "info", value_name = "PRIORITY")]
    priority: String,

    /// Set the priority for stderr messages.
    /// If not specified, stderr uses the same priority as stdout.
    #[arg(long, value_name = "PRIORITY")]
    stderr_priority: Option<String>,

    /// Parse and strip kernel-style priority prefixes (<0> through <7>)
    /// from input lines.
    #[arg(long, default_value = "true", value_name = "BOOL")]
    level_prefix: bool,

    /// The command to execute. If omitted, reads from stdin.
    #[arg(trailing_var_arg = true)]
    command: Vec<String>,
}

/// Build a native journal protocol message.
///
/// The native protocol sends newline-separated `KEY=VALUE` pairs to the
/// journal socket as a single datagram. For binary-safe values, a
/// different encoding is used, but for text lines the simple format
/// suffices.
fn build_journal_message(identifier: &str, priority: Priority, message: &str) -> Vec<u8> {
    let mut msg = Vec::with_capacity(256);

    // PRIORITY=<n>
    writeln!(msg, "PRIORITY={}", priority.as_u8()).unwrap();

    // SYSLOG_IDENTIFIER=<id>
    writeln!(msg, "SYSLOG_IDENTIFIER={identifier}").unwrap();

    // MESSAGE=<text>
    // If the message contains a newline, use the binary-safe encoding:
    //   MESSAGE\n<64-bit LE length><data>
    // Otherwise, use the simple KEY=VALUE form.
    if message.contains('\n') || message.contains('\0') {
        msg.extend_from_slice(b"MESSAGE\n");
        let bytes = message.as_bytes();
        msg.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        msg.extend_from_slice(bytes);
        msg.push(b'\n');
    } else {
        writeln!(msg, "MESSAGE={message}").unwrap();
    }

    // SYSLOG_PID=<pid>
    writeln!(msg, "SYSLOG_PID={}", process::id()).unwrap();

    msg
}

/// Try to parse a kernel-style priority prefix `<N>` from the beginning
/// of a line. Returns `Some((priority, rest_of_line))` if found.
fn parse_level_prefix(line: &str) -> Option<(Priority, &str)> {
    let line = line.strip_prefix('<')?;
    let close = line.find('>')?;
    if close > 1 {
        return None; // Only single-digit priorities
    }
    let digit_str = &line[..close];
    let digit: u8 = digit_str.parse().ok()?;
    let priority = Priority::from_kernel_prefix(digit)?;
    let rest = &line[close + 1..];
    Some((priority, rest))
}

/// Send a single line to the journal socket.
fn send_to_journal(sock: &UnixDatagram, identifier: &str, priority: Priority, message: &str) {
    let msg = build_journal_message(identifier, priority, message);
    // Best-effort: if the socket is unavailable, fall back to stderr.
    if sock.send(&msg).is_err() {
        eprintln!(
            "<{}>{}[{}]: {}",
            priority.as_u8(),
            identifier,
            process::id(),
            message
        );
    }
}

/// Process lines from a reader and send them to the journal.
fn process_lines<R: Read>(
    reader: R,
    sock: &UnixDatagram,
    identifier: &str,
    default_priority: Priority,
    level_prefix: bool,
) {
    let buf_reader = BufReader::new(reader);
    for line in buf_reader.lines() {
        match line {
            Ok(line) => {
                let (priority, text) = if level_prefix {
                    match parse_level_prefix(&line) {
                        Some((p, rest)) => (p, rest),
                        None => (default_priority, line.as_str()),
                    }
                } else {
                    (default_priority, line.as_str())
                };
                send_to_journal(sock, identifier, priority, text);
            }
            Err(e) => {
                eprintln!("systemd-cat: error reading input: {e}");
                break;
            }
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let stdout_priority = Priority::from_str_or_num(&cli.priority).unwrap_or_else(|e| {
        eprintln!("systemd-cat: {e}");
        process::exit(1);
    });

    let stderr_priority = match &cli.stderr_priority {
        Some(s) => Priority::from_str_or_num(s).unwrap_or_else(|e| {
            eprintln!("systemd-cat: {e}");
            process::exit(1);
        }),
        None => stdout_priority,
    };

    // Connect to the journal socket
    let sock = match UnixDatagram::unbound() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("systemd-cat: failed to create socket: {e}");
            process::exit(1);
        }
    };
    if let Err(e) = sock.connect(JOURNAL_SOCKET) {
        eprintln!("systemd-cat: failed to connect to journal socket {JOURNAL_SOCKET}: {e}");
        eprintln!("systemd-cat: falling back to stderr output");
        // Continue anyway — send_to_journal will fall back to stderr
    }

    if cli.command.is_empty() {
        // No command: read from stdin
        let identifier = cli.identifier.as_deref().unwrap_or("unknown");
        let stdin = std::io::stdin();
        process_lines(
            stdin.lock(),
            &sock,
            identifier,
            stdout_priority,
            cli.level_prefix,
        );
    } else {
        // Execute the command and capture its output
        let cmd = &cli.command[0];
        let args = &cli.command[1..];
        let identifier = cli.identifier.as_deref().unwrap_or(cmd.as_str());

        let mut child = match process::Command::new(cmd)
            .args(args)
            .stdout(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                eprintln!("systemd-cat: failed to execute {cmd}: {e}");
                process::exit(1);
            }
        };

        let child_stdout = child.stdout.take();
        let child_stderr = child.stderr.take();

        // Clone socket for stderr thread
        let _sock_fd = {
            use std::os::unix::io::AsRawFd;
            sock.as_raw_fd()
        };

        let identifier_owned = identifier.to_string();
        let level_prefix = cli.level_prefix;

        // Spawn a thread for stderr
        let stderr_handle = if let Some(stderr) = child_stderr {
            let stderr_sock = match UnixDatagram::unbound() {
                Ok(s) => {
                    let _ = s.connect(JOURNAL_SOCKET);
                    s
                }
                Err(_) => {
                    // Fall back: create an unbound socket (send_to_journal
                    // will print to stderr)
                    UnixDatagram::unbound().unwrap()
                }
            };
            let id = identifier_owned.clone();
            Some(std::thread::spawn(move || {
                process_lines(stderr, &stderr_sock, &id, stderr_priority, level_prefix);
            }))
        } else {
            None
        };

        // Process stdout on the main thread
        if let Some(stdout) = child_stdout {
            process_lines(
                stdout,
                &sock,
                &identifier_owned,
                stdout_priority,
                level_prefix,
            );
        }

        // Wait for stderr thread
        if let Some(handle) = stderr_handle {
            let _ = handle.join();
        }

        // Wait for the child and propagate its exit code
        match child.wait() {
            Ok(status) => {
                process::exit(status.code().unwrap_or(1));
            }
            Err(e) => {
                eprintln!("systemd-cat: failed to wait for {cmd}: {e}");
                process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_from_str() {
        assert!(matches!(
            Priority::from_str_or_num("emerg"),
            Ok(Priority::Emergency)
        ));
        assert!(matches!(
            Priority::from_str_or_num("0"),
            Ok(Priority::Emergency)
        ));
        assert!(matches!(
            Priority::from_str_or_num("err"),
            Ok(Priority::Error)
        ));
        assert!(matches!(
            Priority::from_str_or_num("error"),
            Ok(Priority::Error)
        ));
        assert!(matches!(
            Priority::from_str_or_num("3"),
            Ok(Priority::Error)
        ));
        assert!(matches!(
            Priority::from_str_or_num("info"),
            Ok(Priority::Info)
        ));
        assert!(matches!(Priority::from_str_or_num("6"), Ok(Priority::Info)));
        assert!(matches!(
            Priority::from_str_or_num("debug"),
            Ok(Priority::Debug)
        ));
        assert!(matches!(
            Priority::from_str_or_num("7"),
            Ok(Priority::Debug)
        ));
        assert!(Priority::from_str_or_num("unknown").is_err());
    }

    #[test]
    fn test_parse_level_prefix() {
        let (p, rest) = parse_level_prefix("<3>something failed").unwrap();
        assert_eq!(p.as_u8(), 3);
        assert_eq!(rest, "something failed");

        let (p, rest) = parse_level_prefix("<6>info message").unwrap();
        assert_eq!(p.as_u8(), 6);
        assert_eq!(rest, "info message");

        assert!(parse_level_prefix("no prefix").is_none());
        assert!(parse_level_prefix("<>empty").is_none());
        assert!(parse_level_prefix("<99>too big").is_none());
        assert!(parse_level_prefix("<8>out of range").is_none());
    }

    #[test]
    fn test_build_journal_message_simple() {
        let msg = build_journal_message("test", Priority::Info, "hello world");
        let msg_str = String::from_utf8_lossy(&msg);
        assert!(msg_str.contains("PRIORITY=6\n"));
        assert!(msg_str.contains("SYSLOG_IDENTIFIER=test\n"));
        assert!(msg_str.contains("MESSAGE=hello world\n"));
        assert!(msg_str.contains("SYSLOG_PID="));
    }

    #[test]
    fn test_build_journal_message_multiline() {
        let msg = build_journal_message("test", Priority::Error, "line1\nline2");
        // Should use binary-safe encoding for MESSAGE
        assert!(msg.windows(8).any(|w| w == b"MESSAGE\n"));
    }

    #[test]
    fn test_priority_roundtrip() {
        for i in 0..=7u8 {
            let p = Priority::from_kernel_prefix(i).unwrap();
            assert_eq!(p.as_u8(), i);
        }
        assert!(Priority::from_kernel_prefix(8).is_none());
    }
}
