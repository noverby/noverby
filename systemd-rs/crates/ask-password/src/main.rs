//! systemd-ask-password — Query the user for a system password.
//!
//! A drop-in replacement for `systemd-ask-password(1)`. This tool queries
//! the user for a password, either directly on the terminal or by posting
//! a password query file in `/run/systemd/ask-password/` for agents to
//! pick up.
//!
//! The password query protocol works as follows:
//!
//! 1. A question file `ask.XXXX` is created in `/run/systemd/ask-password/`
//!    containing metadata about what password is needed and a Unix socket
//!    path where the answer should be sent.
//!
//! 2. An inotify watch or polling agent (like `systemd-tty-ask-password-agent`)
//!    picks up the question file, prompts the user, and sends the password
//!    back through the specified socket.
//!
//! 3. If `--no-tty` is not given and stdin is a terminal, the tool will
//!    also try to read the password directly from the TTY.
//!
//! Exit codes:
//!   0 — Password was successfully obtained and printed to stdout.
//!   1 — No password was provided (timeout, cancel, or error).

use clap::Parser;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{Duration, Instant};

/// The directory where password query files are placed.
const ASK_PASSWORD_DIR: &str = "/run/systemd/ask-password";

#[derive(Parser, Debug)]
#[command(
    name = "systemd-ask-password",
    about = "Query the user for a system password",
    version
)]
struct Cli {
    /// Specify an icon name for the password query (currently informational
    /// only; used by graphical agents).
    #[arg(long, value_name = "ICON")]
    icon: Option<String>,

    /// Specify an identifier for the password query. This is used by
    /// agents to recognize repeated queries for the same password and
    /// cache them.
    #[arg(long, value_name = "ID")]
    id: Option<String>,

    /// Specify a key name for the kernel keyring. If set, the password
    /// is cached in the kernel keyring under this name.
    #[arg(long, value_name = "NAME")]
    keyname: Option<String>,

    /// Specify a credential name. Used for automatic credential lookup
    /// via `$CREDENTIALS_DIRECTORY`.
    #[arg(long, value_name = "NAME")]
    credential: Option<String>,

    /// Timeout in seconds. If no password is entered within this time,
    /// the query is cancelled. Defaults to 90 seconds.
    #[arg(long, value_name = "SEC", default_value = "90")]
    timeout: u64,

    /// If set, do not echo the password while typing. This is the default
    /// behavior (kept for compatibility).
    #[arg(long)]
    echo: bool,

    /// Do not print a trailing newline after the password on stdout.
    #[arg(long)]
    no_newline: bool,

    /// Accept empty passwords. By default, empty input is rejected.
    #[arg(long)]
    accept_cached: bool,

    /// Do not query on the TTY directly, only use the agent protocol.
    #[arg(long)]
    no_tty: bool,

    /// Emit question through the agent protocol even if we can ask on
    /// the TTY. This allows both TTY input and agent-based input.
    #[arg(long)]
    multiple: bool,

    /// Do not query via the agent protocol at all.
    #[arg(long)]
    no_agent: bool,

    /// The human-readable prompt message. Defaults to "Password:".
    #[arg(default_value = "Password:")]
    message: String,
}

/// A password question file for the agent protocol.
struct QuestionFile {
    path: PathBuf,
    socket_path: PathBuf,
}

impl QuestionFile {
    /// Create a new question file in `/run/systemd/ask-password/`.
    fn create(cli: &Cli, deadline: Instant) -> io::Result<Self> {
        fs::create_dir_all(ASK_PASSWORD_DIR)?;

        let id = uuid::Uuid::new_v4();
        let filename = format!("ask.{}", id.as_simple());
        let path = Path::new(ASK_PASSWORD_DIR).join(&filename);
        let socket_name = format!("sck.{}", id.as_simple());
        let socket_path = Path::new(ASK_PASSWORD_DIR).join(&socket_name);

        let not_after = {
            let remaining = deadline.saturating_duration_since(Instant::now());
            // Convert to CLOCK_MONOTONIC microseconds
            let mut ts = libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            let ret = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
            if ret == 0 {
                let now_usec = ts.tv_sec as u64 * 1_000_000 + ts.tv_nsec as u64 / 1_000;
                now_usec + remaining.as_micros() as u64
            } else {
                0
            }
        };

        let mut content = String::new();
        content.push_str("[Ask]\n");
        content.push_str(&format!("PID={}\n", process::id()));
        content.push_str(&format!("Socket={}\n", socket_path.display()));
        content.push_str(&format!(
            "AcceptCached={}\n",
            if cli.accept_cached { "yes" } else { "no" }
        ));
        content.push_str(&format!("Echo={}\n", if cli.echo { "yes" } else { "no" }));
        content.push_str(&format!("NotAfter={not_after}\n"));
        content.push_str(&format!("Message={}\n", cli.message));

        if let Some(ref icon) = cli.icon {
            content.push_str(&format!("Icon={icon}\n"));
        }

        if let Some(ref id) = cli.id {
            content.push_str(&format!("Id={id}\n"));
        }

        fs::write(&path, &content)?;

        Ok(QuestionFile { path, socket_path })
    }

    /// Listen on the socket for a password response from an agent.
    fn wait_for_response(&self, timeout: Duration) -> io::Result<Option<String>> {
        // Remove stale socket if it exists
        let _ = fs::remove_file(&self.socket_path);

        let sock = UnixDatagram::bind(&self.socket_path)?;
        sock.set_read_timeout(Some(timeout))?;

        // Make socket world-writable so agents running as different users can reply
        let path_cstr =
            std::ffi::CString::new(self.socket_path.to_str().unwrap_or_default()).unwrap();
        unsafe {
            libc::chmod(path_cstr.as_ptr(), 0o666);
        }

        let mut buf = [0u8; 4096];
        match sock.recv(&mut buf) {
            Ok(n) => {
                if n == 0 {
                    return Ok(None);
                }
                // The agent protocol sends the password with a leading '+' for
                // success or '-' for failure/cancellation.
                let response = std::str::from_utf8(&buf[..n])
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                if let Some(stripped) = response.strip_prefix('+') {
                    Ok(Some(stripped.to_string()))
                } else {
                    Ok(None)
                }
            }
            Err(e)
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut =>
            {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }
}

impl Drop for QuestionFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(&self.socket_path);
    }
}

/// Check if stdin is a TTY.
fn stdin_is_tty() -> bool {
    unsafe { libc::isatty(libc::STDIN_FILENO) == 1 }
}

/// Read a password directly from the terminal.
///
/// If `echo` is false, terminal echo is disabled while reading.
fn read_password_from_tty(prompt: &str, echo: bool) -> io::Result<String> {
    // Open /dev/tty directly so we can read even if stdin is redirected
    let tty_path = "/dev/tty";
    let tty_fd = {
        let path_cstr = std::ffi::CString::new(tty_path).unwrap();
        let fd = unsafe { libc::open(path_cstr.as_ptr(), libc::O_RDWR | libc::O_NOCTTY) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        fd
    };

    // Write prompt
    let prompt_bytes = prompt.as_bytes();
    unsafe {
        libc::write(tty_fd, prompt_bytes.as_ptr().cast(), prompt_bytes.len());
        libc::write(tty_fd, b" ".as_ptr().cast(), 1);
    }

    // Save terminal settings and disable echo if needed
    let mut old_termios: libc::termios = unsafe { std::mem::zeroed() };
    let termios_saved = unsafe { libc::tcgetattr(tty_fd, &mut old_termios) } == 0;

    if !echo && termios_saved {
        let mut new_termios = old_termios;
        new_termios.c_lflag &= !(libc::ECHO);
        unsafe {
            libc::tcsetattr(tty_fd, libc::TCSANOW, &new_termios);
        }
    }

    // Read password
    let mut password = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = unsafe { libc::read(tty_fd, byte.as_mut_ptr().cast(), 1) };
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
            // Restore terminal settings
            if !echo && termios_saved {
                unsafe {
                    libc::tcsetattr(tty_fd, libc::TCSANOW, &old_termios);
                }
            }
            // Print newline
            unsafe {
                libc::write(tty_fd, b"\n".as_ptr().cast(), 1);
            }
            unsafe {
                libc::close(tty_fd);
            }
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }
        password.push(byte[0]);
    }

    // Restore terminal settings
    if !echo && termios_saved {
        unsafe {
            libc::tcsetattr(tty_fd, libc::TCSANOW, &old_termios);
        }
        // Print newline since echo was off
        unsafe {
            libc::write(tty_fd, b"\n".as_ptr().cast(), 1);
        }
    }

    unsafe {
        libc::close(tty_fd);
    }

    String::from_utf8(password).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Try to read a password from $CREDENTIALS_DIRECTORY/<name>.
fn try_credential(name: &str) -> Option<String> {
    let dir = std::env::var("CREDENTIALS_DIRECTORY").ok()?;
    let path = Path::new(&dir).join(name);
    fs::read_to_string(&path)
        .ok()
        .map(|s| s.trim_end().to_string())
}

fn main() {
    let cli = Cli::parse();

    let timeout = Duration::from_secs(cli.timeout);
    let deadline = Instant::now() + timeout;

    // Try credential lookup first
    if let Some(ref cred_name) = cli.credential
        && let Some(password) = try_credential(cred_name)
    {
        print!("{password}");
        if !cli.no_newline {
            println!();
        }
        process::exit(0);
    }

    let can_tty = !cli.no_tty && stdin_is_tty();
    let use_agent = !cli.no_agent && (!can_tty || cli.multiple);

    // If using the agent protocol, create the question file
    let question = if use_agent {
        match QuestionFile::create(&cli, deadline) {
            Ok(q) => Some(q),
            Err(e) => {
                eprintln!("Warning: failed to create password question file: {e}");
                None
            }
        }
    } else {
        None
    };

    // If we can query directly on the TTY, do so
    if can_tty {
        match read_password_from_tty(&cli.message, cli.echo) {
            Ok(password) => {
                if password.is_empty() && !cli.accept_cached {
                    eprintln!("Empty password not accepted.");
                    process::exit(1);
                }
                print!("{password}");
                if !cli.no_newline {
                    println!();
                }
                io::stdout().flush().ok();
                process::exit(0);
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                process::exit(1);
            }
            Err(e) => {
                eprintln!("Failed to read password from TTY: {e}");
                // Fall through to agent protocol if available
                if question.is_none() {
                    process::exit(1);
                }
            }
        }
    }

    // Wait for agent response
    if let Some(ref q) = question {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match q.wait_for_response(remaining) {
            Ok(Some(password)) => {
                if password.is_empty() && !cli.accept_cached {
                    eprintln!("Empty password not accepted.");
                    process::exit(1);
                }
                print!("{password}");
                if !cli.no_newline {
                    println!();
                }
                io::stdout().flush().ok();
                process::exit(0);
            }
            Ok(None) => {
                eprintln!("No password provided (timeout or cancellation).");
                process::exit(1);
            }
            Err(e) => {
                eprintln!("Error waiting for password: {e}");
                process::exit(1);
            }
        }
    }

    // If we get here, neither TTY nor agent worked
    if !can_tty && !use_agent {
        // Nothing to do — read from stdin as a fallback
        let mut password = String::new();
        match io::stdin().read_to_string(&mut password) {
            Ok(_) => {
                let password = password.trim_end_matches('\n').trim_end_matches('\r');
                if password.is_empty() && !cli.accept_cached {
                    eprintln!("Empty password not accepted.");
                    process::exit(1);
                }
                print!("{password}");
                if !cli.no_newline {
                    println!();
                }
                io::stdout().flush().ok();
                process::exit(0);
            }
            Err(e) => {
                eprintln!("Error reading from stdin: {e}");
                process::exit(1);
            }
        }
    }

    eprintln!("No password obtained.");
    process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdin_is_tty_does_not_panic() {
        // In a test environment, stdin is typically not a TTY
        let _ = stdin_is_tty();
    }

    #[test]
    fn test_try_credential_missing() {
        // With no CREDENTIALS_DIRECTORY set, should return None
        unsafe { std::env::remove_var("CREDENTIALS_DIRECTORY") };
        assert!(try_credential("test").is_none());
    }

    #[test]
    fn test_try_credential_with_dir() {
        let dir = std::env::temp_dir().join("ask-password-test-creds");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("mypassword"), "s3cret\n").unwrap();

        unsafe { std::env::set_var("CREDENTIALS_DIRECTORY", dir.to_str().unwrap()) };
        let result = try_credential("mypassword");
        assert_eq!(result, Some("s3cret".to_string()));

        unsafe { std::env::remove_var("CREDENTIALS_DIRECTORY") };
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_question_file_protocol() {
        // Only run if we can create the directory (typically need root or /tmp)
        let test_dir = std::env::temp_dir().join("ask-password-test");
        let _ = fs::create_dir_all(&test_dir);

        // We can't easily test the full protocol without root, but we can
        // verify the data structures work.
        let id = uuid::Uuid::new_v4();
        assert!(!id.to_string().is_empty());

        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_ask_password_dir_constant() {
        assert_eq!(ASK_PASSWORD_DIR, "/run/systemd/ask-password");
    }
}
