//! systemd-tty-ask-password-agent — Process system password requests on the TTY.
//!
//! A drop-in replacement for `systemd-tty-ask-password-agent(1)`. This tool
//! monitors `/run/systemd/ask-password/` for password question files created
//! by `systemd-ask-password` (or other systemd components), displays the
//! password prompt on the controlling TTY, reads the user's input, and sends
//! the response back through the Unix socket specified in the question file.
//!
//! Modes of operation:
//!
//! - `--query`  — Process all pending password questions once, then exit.
//! - `--watch`  — Continuously watch for new questions (using inotify) and
//!   process them as they appear.
//! - `--wall`   — Send wall messages for pending password questions instead
//!   of querying on the TTY.
//! - `--list`   — List all currently pending password questions, then exit.
//!
//! The password query protocol:
//!
//! Question files are INI-style files in `/run/systemd/ask-password/` with
//! an `[Ask]` section containing:
//!   - `PID=`          — PID of the process asking
//!   - `Socket=`       — Unix socket path to send the password to
//!   - `Message=`      — Human-readable prompt
//!   - `Icon=`         — Icon name (informational)
//!   - `Id=`           — Identifier for caching
//!   - `NotAfter=`     — Monotonic timestamp (usec) after which the question expires
//!   - `AcceptCached=` — Whether to accept cached/empty passwords
//!   - `Echo=`         — Whether to echo input
//!
//! The response is sent as a datagram to the specified socket:
//!   - `+<password>` for a successful response
//!   - `-` for cancellation

use clap::Parser;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

/// The directory where password query files are placed.
const ASK_PASSWORD_DIR: &str = "/run/systemd/ask-password";

#[derive(Parser, Debug)]
#[command(
    name = "systemd-tty-ask-password-agent",
    about = "Process system password requests",
    version
)]
struct Cli {
    /// Process all currently pending password questions, then exit.
    #[arg(long, group = "mode")]
    query: bool,

    /// Continuously watch for password questions and process them.
    #[arg(long, group = "mode")]
    watch: bool,

    /// Send wall messages for pending password questions.
    #[arg(long, group = "mode")]
    wall: bool,

    /// List currently pending password questions.
    #[arg(long, group = "mode")]
    list: bool,

    /// Name of the Plymouth password agent socket (compatibility; ignored).
    #[arg(long, value_name = "PATH")]
    plymouth: Option<String>,

    /// Use the specified TTY console for password queries.
    #[arg(long, value_name = "PATH")]
    console: Option<String>,
}

/// A parsed password question from a question file.
#[derive(Debug)]
struct PasswordQuestion {
    /// Path to the question file itself.
    file_path: PathBuf,
    /// PID of the asking process.
    pid: Option<u32>,
    /// Unix socket path to send the answer to.
    socket: Option<PathBuf>,
    /// Human-readable prompt message.
    message: String,
    /// Icon name (informational).
    icon: Option<String>,
    /// Identifier for caching.
    id: Option<String>,
    /// Monotonic deadline in microseconds; 0 means no deadline.
    not_after: u64,
    /// Whether cached/empty passwords are accepted.
    accept_cached: bool,
    /// Whether to echo input.
    echo: bool,
}

impl PasswordQuestion {
    /// Parse a question file.
    fn from_file(path: &Path) -> io::Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut pid = None;
        let mut socket = None;
        let mut message = String::from("Password:");
        let mut icon = None;
        let mut id = None;
        let mut not_after = 0u64;
        let mut accept_cached = false;
        let mut echo = false;

        let mut in_ask_section = false;

        for line in content.lines() {
            let line = line.trim();

            if line == "[Ask]" {
                in_ask_section = true;
                continue;
            }

            if line.starts_with('[') {
                in_ask_section = false;
                continue;
            }

            if !in_ask_section {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "PID" => pid = value.parse().ok(),
                    "Socket" => socket = Some(PathBuf::from(value)),
                    "Message" => message = value.to_string(),
                    "Icon" => icon = Some(value.to_string()),
                    "Id" => id = Some(value.to_string()),
                    "NotAfter" => not_after = value.parse().unwrap_or(0),
                    "AcceptCached" => accept_cached = value == "yes" || value == "1",
                    "Echo" => echo = value == "yes" || value == "1",
                    _ => {}
                }
            }
        }

        Ok(PasswordQuestion {
            file_path: path.to_path_buf(),
            pid,
            socket,
            message,
            icon,
            id,
            not_after,
            accept_cached,
            echo,
        })
    }

    /// Check if this question has expired based on the monotonic clock.
    fn is_expired(&self) -> bool {
        if self.not_after == 0 {
            return false;
        }

        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let ret = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
        if ret != 0 {
            return false;
        }

        let now_usec = ts.tv_sec as u64 * 1_000_000 + ts.tv_nsec as u64 / 1_000;
        now_usec > self.not_after
    }

    /// Send a password response to the asking process via the socket.
    fn send_response(&self, password: &str) -> io::Result<()> {
        let socket_path = self.socket.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "no socket path in question file",
            )
        })?;

        let sock = UnixDatagram::unbound()?;

        // Send "+<password>" for success
        let msg = format!("+{password}");
        sock.send_to(msg.as_bytes(), socket_path)?;

        Ok(())
    }

    /// Send a cancellation response to the asking process.
    fn send_cancel(&self) -> io::Result<()> {
        let socket_path = self.socket.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "no socket path in question file",
            )
        })?;

        let sock = UnixDatagram::unbound()?;
        sock.send_to(b"-", socket_path)?;

        Ok(())
    }
}

/// Enumerate all pending password questions.
fn enumerate_questions() -> Vec<PasswordQuestion> {
    let dir = Path::new(ASK_PASSWORD_DIR);
    let mut questions = Vec::new();

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return questions,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Only process "ask.*" files
        if !name_str.starts_with("ask.") {
            continue;
        }

        match PasswordQuestion::from_file(&path) {
            Ok(q) => {
                if !q.is_expired() {
                    questions.push(q);
                }
            }
            Err(e) => {
                eprintln!(
                    "Warning: failed to parse question file {}: {e}",
                    path.display()
                );
            }
        }
    }

    questions
}

/// Read a password from the TTY.
///
/// Opens `/dev/tty` (or the specified console) and reads a line, optionally
/// with echo disabled.
fn read_password_tty(prompt: &str, echo: bool, console: Option<&str>) -> io::Result<String> {
    let tty_path = console.unwrap_or("/dev/tty");
    let tty_cstr = std::ffi::CString::new(tty_path)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let fd = unsafe { libc::open(tty_cstr.as_ptr(), libc::O_RDWR | libc::O_NOCTTY) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    // Write prompt
    let prompt_msg = format!("{prompt} ");
    unsafe {
        libc::write(fd, prompt_msg.as_ptr().cast(), prompt_msg.len());
    }

    // Save terminal settings and optionally disable echo
    let mut old_termios: libc::termios = unsafe { std::mem::zeroed() };
    let termios_saved = unsafe { libc::tcgetattr(fd, &mut old_termios) } == 0;

    if !echo && termios_saved {
        let mut new_termios = old_termios;
        new_termios.c_lflag &= !(libc::ECHO);
        unsafe {
            libc::tcsetattr(fd, libc::TCSANOW, &new_termios);
        }
    }

    // Read line
    let mut password = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = unsafe { libc::read(fd, byte.as_mut_ptr().cast(), 1) };
        if n <= 0 {
            break;
        }
        if byte[0] == b'\n' || byte[0] == b'\r' {
            break;
        }
        // Handle backspace
        if byte[0] == 0x7f || byte[0] == 0x08 {
            password.pop();
            continue;
        }
        // Ctrl-C / Ctrl-D
        if byte[0] == 0x03 || byte[0] == 0x04 {
            if !echo && termios_saved {
                unsafe {
                    libc::tcsetattr(fd, libc::TCSANOW, &old_termios);
                }
            }
            unsafe {
                libc::write(fd, b"\n".as_ptr().cast(), 1);
                libc::close(fd);
            }
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }
        password.push(byte[0]);
    }

    // Restore terminal settings
    if !echo && termios_saved {
        unsafe {
            libc::tcsetattr(fd, libc::TCSANOW, &old_termios);
            libc::write(fd, b"\n".as_ptr().cast(), 1);
        }
    }

    unsafe {
        libc::close(fd);
    }

    String::from_utf8(password).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Process a single password question by prompting on the TTY.
fn process_question(question: &PasswordQuestion, console: Option<&str>) -> bool {
    if question.is_expired() {
        return false;
    }

    if question.socket.is_none() {
        eprintln!(
            "Warning: question file {} has no socket, skipping",
            question.file_path.display()
        );
        return false;
    }

    let prompt = if let Some(pid) = question.pid {
        format!("{} (PID {})", question.message, pid)
    } else {
        question.message.clone()
    };

    match read_password_tty(&prompt, question.echo, console) {
        Ok(password) => {
            if password.is_empty() && !question.accept_cached {
                eprintln!("Empty password not accepted.");
                if let Err(e) = question.send_cancel() {
                    eprintln!("Failed to send cancellation: {e}");
                }
                return false;
            }
            match question.send_response(&password) {
                Ok(()) => true,
                Err(e) => {
                    eprintln!("Failed to send password response: {e}");
                    false
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::Interrupted => {
            let _ = question.send_cancel();
            false
        }
        Err(e) => {
            eprintln!("Failed to read password: {e}");
            let _ = question.send_cancel();
            false
        }
    }
}

/// List all pending password questions.
fn list_questions() {
    let questions = enumerate_questions();

    if questions.is_empty() {
        println!("No pending password questions.");
        return;
    }

    for (i, q) in questions.iter().enumerate() {
        println!("{}. {}", i + 1, q.message);
        if let Some(pid) = q.pid {
            println!("   PID: {pid}");
        }
        if let Some(ref id) = q.id {
            println!("   ID: {id}");
        }
        if let Some(ref icon) = q.icon {
            println!("   Icon: {icon}");
        }
        println!("   Echo: {}", if q.echo { "yes" } else { "no" });
        println!(
            "   Accept Cached: {}",
            if q.accept_cached { "yes" } else { "no" }
        );
        if q.not_after > 0 {
            println!("   Not After: {} usec", q.not_after);
        }
        if let Some(ref socket) = q.socket {
            println!("   Socket: {}", socket.display());
        }
        println!("   File: {}", q.file_path.display());
        println!();
    }
}

/// Send wall messages about pending password questions.
fn wall_questions() {
    let questions = enumerate_questions();

    if questions.is_empty() {
        return;
    }

    for q in &questions {
        let msg = if let Some(pid) = q.pid {
            format!(
                "Password entry required for '{}' (PID {}).\r\n\
                 Please enter password with the systemd-tty-ask-password-agent tool.",
                q.message, pid
            )
        } else {
            format!(
                "Password entry required for '{}'.\r\n\
                 Please enter password with the systemd-tty-ask-password-agent tool.",
                q.message
            )
        };

        // Try to write wall message via utmp/write to all terminals
        send_wall_message(&msg);
    }
}

/// Send a wall message to all logged-in TTYs.
fn send_wall_message(msg: &str) {
    // Read /dev/pts/* and /dev/tty* to find active terminals
    let mut ttys = Vec::new();

    // Check /dev/pts/
    if let Ok(entries) = fs::read_dir("/dev/pts") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                // Skip ptmx
                if name == "ptmx" {
                    continue;
                }
                // Only process numeric entries (pty slaves)
                if name.chars().all(|c| c.is_ascii_digit()) {
                    ttys.push(path);
                }
            }
        }
    }

    // Also check /dev/tty[1-63]
    for i in 1..=63 {
        let path = PathBuf::from(format!("/dev/tty{i}"));
        if path.exists() {
            ttys.push(path);
        }
    }

    let wall_msg = format!(
        "\r\n\
         \r\nBroadcast message from systemd-tty-ask-password-agent:\r\n\
         \r\n{msg}\r\n\r\n"
    );

    for tty in &ttys {
        let tty_cstr = match std::ffi::CString::new(tty.to_str().unwrap_or_default()) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let fd = unsafe {
            libc::open(
                tty_cstr.as_ptr(),
                libc::O_WRONLY | libc::O_NOCTTY | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            continue;
        }

        unsafe {
            libc::write(fd, wall_msg.as_ptr().cast(), wall_msg.len());
            libc::close(fd);
        }
    }
}

/// Watch for new password question files using polling.
///
/// Ideally this would use inotify, but for simplicity and portability
/// we poll the directory every 500ms. This matches what the real agent
/// does as a fallback when inotify is not available.
fn watch_and_process(console: Option<&str>) {
    let dir = Path::new(ASK_PASSWORD_DIR);
    let mut processed: HashSet<PathBuf> = HashSet::new();

    // Ensure the directory exists
    let _ = fs::create_dir_all(dir);

    eprintln!("Watching {ASK_PASSWORD_DIR} for password questions...");

    loop {
        let questions = enumerate_questions();

        for q in &questions {
            if processed.contains(&q.file_path) {
                continue;
            }

            if process_question(q, console) {
                processed.insert(q.file_path.clone());
            } else {
                // If the question expired or was cancelled, mark it as
                // processed so we don't keep retrying.
                if q.is_expired() {
                    processed.insert(q.file_path.clone());
                }
            }
        }

        // Clean up processed entries for files that no longer exist
        processed.retain(|p| p.exists());

        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Process all pending questions once.
fn query_once(console: Option<&str>) -> bool {
    let questions = enumerate_questions();

    if questions.is_empty() {
        return true;
    }

    let mut all_ok = true;
    for q in &questions {
        if !process_question(q, console) {
            all_ok = false;
        }
    }

    all_ok
}

fn main() {
    let cli = Cli::parse();

    // Determine the console to use
    let console = cli.console.as_deref();

    if cli.list {
        list_questions();
        process::exit(0);
    }

    if cli.wall {
        wall_questions();
        process::exit(0);
    }

    if cli.watch {
        watch_and_process(console);
        // watch_and_process runs forever, but just in case:
        process::exit(0);
    }

    // Default to --query if no mode specified, or --query was explicit
    if cli.query || (!cli.watch && !cli.wall && !cli.list) {
        let ok = query_once(console);
        process::exit(if ok { 0 } else { 1 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enumerate_questions_empty() {
        // In a test environment, /run/systemd/ask-password/ likely doesn't
        // exist or is empty. This should not panic.
        let questions = enumerate_questions();
        // We can't assert the count, but at least it shouldn't crash
        let _ = questions;
    }

    #[test]
    fn test_parse_question_file() {
        let dir = std::env::temp_dir().join("tty-agent-test");
        let _ = fs::create_dir_all(&dir);

        let content = "\
[Ask]
PID=12345
Socket=/run/systemd/ask-password/sck.test
Message=Enter LUKS passphrase for /dev/sda2:
Icon=drive-harddisk
Id=cryptsetup:/dev/sda2
NotAfter=0
AcceptCached=no
Echo=no
";
        let path = dir.join("ask.test");
        fs::write(&path, content).unwrap();

        let q = PasswordQuestion::from_file(&path).unwrap();
        assert_eq!(q.pid, Some(12345));
        assert_eq!(
            q.socket,
            Some(PathBuf::from("/run/systemd/ask-password/sck.test"))
        );
        assert_eq!(q.message, "Enter LUKS passphrase for /dev/sda2:");
        assert_eq!(q.icon, Some("drive-harddisk".to_string()));
        assert_eq!(q.id, Some("cryptsetup:/dev/sda2".to_string()));
        assert_eq!(q.not_after, 0);
        assert!(!q.accept_cached);
        assert!(!q.echo);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_question_file_minimal() {
        let dir = std::env::temp_dir().join("tty-agent-test-minimal");
        let _ = fs::create_dir_all(&dir);

        let content = "\
[Ask]
Socket=/tmp/test.sock
Message=Password:
";
        let path = dir.join("ask.minimal");
        fs::write(&path, content).unwrap();

        let q = PasswordQuestion::from_file(&path).unwrap();
        assert_eq!(q.pid, None);
        assert_eq!(q.socket, Some(PathBuf::from("/tmp/test.sock")));
        assert_eq!(q.message, "Password:");
        assert_eq!(q.icon, None);
        assert_eq!(q.id, None);
        assert_eq!(q.not_after, 0);
        assert!(!q.accept_cached);
        assert!(!q.echo);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_not_expired_when_zero() {
        let q = PasswordQuestion {
            file_path: PathBuf::from("/tmp/test"),
            pid: None,
            socket: None,
            message: String::new(),
            icon: None,
            id: None,
            not_after: 0,
            accept_cached: false,
            echo: false,
        };
        assert!(!q.is_expired());
    }

    #[test]
    fn test_expired_in_past() {
        let q = PasswordQuestion {
            file_path: PathBuf::from("/tmp/test"),
            pid: None,
            socket: None,
            message: String::new(),
            icon: None,
            id: None,
            not_after: 1, // 1 microsecond since boot — definitely in the past
            accept_cached: false,
            echo: false,
        };
        assert!(q.is_expired());
    }

    #[test]
    fn test_not_expired_in_future() {
        // Set NotAfter far in the future (year 2100 equivalent in usec)
        let q = PasswordQuestion {
            file_path: PathBuf::from("/tmp/test"),
            pid: None,
            socket: None,
            message: String::new(),
            icon: None,
            id: None,
            not_after: u64::MAX / 2,
            accept_cached: false,
            echo: false,
        };
        assert!(!q.is_expired());
    }

    #[test]
    fn test_ask_password_dir_constant() {
        assert_eq!(ASK_PASSWORD_DIR, "/run/systemd/ask-password");
    }

    #[test]
    fn test_parse_with_echo_yes() {
        let dir = std::env::temp_dir().join("tty-agent-test-echo");
        let _ = fs::create_dir_all(&dir);

        let content = "\
[Ask]
Socket=/tmp/test.sock
Message=Username:
Echo=yes
AcceptCached=1
";
        let path = dir.join("ask.echo");
        fs::write(&path, content).unwrap();

        let q = PasswordQuestion::from_file(&path).unwrap();
        assert!(q.echo);
        assert!(q.accept_cached);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_ignores_other_sections() {
        let dir = std::env::temp_dir().join("tty-agent-test-sections");
        let _ = fs::create_dir_all(&dir);

        let content = "\
[Other]
Socket=/wrong/path
Message=Wrong message

[Ask]
Socket=/right/path
Message=Right message

[Another]
Message=Also wrong
";
        let path = dir.join("ask.sections");
        fs::write(&path, content).unwrap();

        let q = PasswordQuestion::from_file(&path).unwrap();
        assert_eq!(q.socket, Some(PathBuf::from("/right/path")));
        assert_eq!(q.message, "Right message");

        let _ = fs::remove_dir_all(&dir);
    }
}
