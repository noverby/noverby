use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use sha2::{Digest, Sha256};

use crate::units::{
    PlatformSpecificServiceFields, RLimitValue, ResourceLimit, StandardInput, UtmpMode,
};

/// Convert a Linux capability name (e.g. "CAP_SYS_TIME") to its numeric
/// value as defined in `<linux/capability.h>`.  Returns `None` for
/// unrecognised names.
fn cap_name_to_number(name: &str) -> Option<u64> {
    match name.to_uppercase().as_str() {
        "CAP_CHOWN" => Some(0),
        "CAP_DAC_OVERRIDE" => Some(1),
        "CAP_DAC_READ_SEARCH" => Some(2),
        "CAP_FOWNER" => Some(3),
        "CAP_FSETID" => Some(4),
        "CAP_KILL" => Some(5),
        "CAP_SETGID" => Some(6),
        "CAP_SETUID" => Some(7),
        "CAP_SETPCAP" => Some(8),
        "CAP_LINUX_IMMUTABLE" => Some(9),
        "CAP_NET_BIND_SERVICE" => Some(10),
        "CAP_NET_BROADCAST" => Some(11),
        "CAP_NET_ADMIN" => Some(12),
        "CAP_NET_RAW" => Some(13),
        "CAP_IPC_LOCK" => Some(14),
        "CAP_IPC_OWNER" => Some(15),
        "CAP_SYS_MODULE" => Some(16),
        "CAP_SYS_RAWIO" => Some(17),
        "CAP_SYS_CHROOT" => Some(18),
        "CAP_SYS_PTRACE" => Some(19),
        "CAP_SYS_PACCT" => Some(20),
        "CAP_SYS_ADMIN" => Some(21),
        "CAP_SYS_BOOT" => Some(22),
        "CAP_SYS_NICE" => Some(23),
        "CAP_SYS_RESOURCE" => Some(24),
        "CAP_SYS_TIME" => Some(25),
        "CAP_SYS_TTY_CONFIG" => Some(26),
        "CAP_MKNOD" => Some(27),
        "CAP_LEASE" => Some(28),
        "CAP_AUDIT_WRITE" => Some(29),
        "CAP_AUDIT_CONTROL" => Some(30),
        "CAP_SETFCAP" => Some(31),
        "CAP_MAC_OVERRIDE" => Some(32),
        "CAP_MAC_ADMIN" => Some(33),
        "CAP_SYSLOG" => Some(34),
        "CAP_WAKE_ALARM" => Some(35),
        "CAP_BLOCK_SUSPEND" => Some(36),
        "CAP_AUDIT_READ" => Some(37),
        "CAP_PERFMON" => Some(38),
        "CAP_BPF" => Some(39),
        "CAP_CHECKPOINT_RESTORE" => Some(40),
        _ => None,
    }
}

/// Resolve the list of ambient capability names to their numeric values,
/// filtering out `~`-prefixed (deny-list) entries and unknown names.
fn resolve_ambient_caps(names: &[String]) -> Vec<u64> {
    let mut caps = Vec::new();
    for name in names {
        let name = name.trim();
        if name.is_empty() || name.starts_with('~') {
            continue;
        }
        if let Some(cap) = cap_name_to_number(name) {
            caps.push(cap);
        } else {
            eprintln!("[EXEC_HELPER] Unknown ambient capability: {name}");
        }
    }
    caps
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct ExecHelperConfig {
    pub name: String,

    pub cmd: PathBuf,
    pub args: Vec<String>,
    /// When true, args[0] is used as argv[0] instead of the filename of cmd.
    /// This corresponds to the '@' prefix in systemd command lines.
    #[serde(default)]
    pub use_first_arg_as_argv0: bool,

    pub env: Vec<(String, String)>,

    pub group: libc::gid_t,
    pub supplementary_groups: Vec<libc::gid_t>,
    pub user: libc::uid_t,

    pub working_directory: Option<PathBuf>,
    pub state_directory: Vec<String>,
    pub logs_directory: Vec<String>,
    pub logs_directory_mode: Option<u32>,
    pub runtime_directory: Vec<String>,

    /// OOMScoreAdjust= — sets the OOM score adjustment for executed processes.
    /// Written to /proc/self/oom_score_adj before exec. Range: -1000 to 1000.
    pub oom_score_adjust: Option<i32>,

    pub platform_specific: PlatformSpecificServiceFields,

    pub limit_nofile: Option<ResourceLimit>,

    /// How stdin should be set up for the service process.
    #[serde(default)]
    pub stdin_option: StandardInput,
    /// Path to the TTY device to use when StandardInput=tty/tty-force/tty-fail.
    /// Defaults to /dev/console if not set.
    pub tty_path: Option<PathBuf>,

    /// TTYReset= — reset the TTY to sane defaults before use.
    /// Matches systemd: resets termios, keyboard mode, switches to text mode.
    #[serde(default)]
    pub tty_reset: bool,
    /// TTYVHangup= — send TIOCVHANGUP to the TTY before use.
    /// Disconnects prior sessions so the new service gets a clean terminal.
    #[serde(default)]
    pub tty_vhangup: bool,
    /// TTYVTDisallocate= — deallocate or clear the VT before use.
    #[serde(default)]
    pub tty_vt_disallocate: bool,

    /// IgnoreSIGPIPE= — if true (the default), SIGPIPE is set to SIG_IGN before
    /// exec'ing the service binary. When false, the default SIGPIPE disposition
    /// (terminate) is left in place. Matches systemd.exec(5).
    #[serde(default = "default_true")]
    pub ignore_sigpipe: bool,

    /// UtmpIdentifier= — the identifier string for utmp/wtmp records (up to 4
    /// characters). When set together with a TTY, a utmp/wtmp login record is
    /// written before exec and a dead record on service exit.
    #[serde(default)]
    pub utmp_identifier: Option<String>,

    /// UtmpMode= — the type of utmp/wtmp record to create (init/login/user).
    #[serde(default)]
    pub utmp_mode: UtmpMode,

    /// ImportCredential= — glob patterns for credentials to import from the
    /// system credential store into the service's credential directory.
    /// The `CREDENTIALS_DIRECTORY` env var is set to the created directory.
    #[serde(default)]
    pub import_credentials: Vec<String>,

    /// LoadCredential=ID:PATH — load a credential from a file at PATH and
    /// place it in the service's credential directory under the name ID.
    #[serde(default)]
    pub load_credentials: Vec<(String, String)>,

    /// LoadCredentialEncrypted=ID:PATH — like LoadCredential= but the file
    /// content is expected to be encrypted. Parsed but decryption is not yet
    /// implemented (credential is loaded as-is).
    #[serde(default)]
    pub load_credentials_encrypted: Vec<(String, String)>,

    /// SetCredential=ID:DATA — set a credential from inline data. The data
    /// is written verbatim to the credential directory under the given ID.
    #[serde(default)]
    pub set_credentials: Vec<(String, String)>,

    /// SetCredentialEncrypted=ID:DATA — like SetCredential= but the inline
    /// data is expected to be encrypted (base64-encoded). Parsed but
    /// decryption is not yet implemented (credential is written as-is).
    #[serde(default)]
    pub set_credentials_encrypted: Vec<(String, String)>,

    /// Whether StandardOutput is set to inherit (or journal/kmsg/tty/unset).
    /// When true AND stdin is a TTY, stdout will be dup'd from the TTY fd.
    #[serde(default = "default_true")]
    pub stdout_is_inherit: bool,
    /// Whether StandardError is set to inherit (or journal/kmsg/tty/unset).
    /// When true AND stdin is a TTY, stderr will be dup'd from the TTY fd.
    #[serde(default = "default_true")]
    pub stderr_is_inherit: bool,
    /// Whether StandardOutput is explicitly set to tty.
    /// When true AND stdin is NOT a TTY, the TTY is opened independently for stdout.
    #[serde(default)]
    pub stdout_is_tty: bool,
    /// Whether StandardError is explicitly set to tty.
    /// When true AND stdin is NOT a TTY, the TTY is opened independently for stderr.
    #[serde(default)]
    pub stderr_is_tty: bool,

    /// AmbientCapabilities= — Linux capability names (e.g. CAP_SYS_TIME) to
    /// raise as ambient capabilities after dropping privileges.  Ambient
    /// capabilities survive execve() even without file capabilities, so the
    /// unprivileged service process retains them.
    #[serde(default)]
    pub ambient_capabilities: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn prepare_exec_args(
    cmd_str: &Path,
    args_str: &[String],
    use_first_arg_as_argv0: bool,
) -> (std::ffi::CString, Vec<std::ffi::CString>) {
    let cmd = std::ffi::CString::new(cmd_str.to_string_lossy().as_bytes()).unwrap();

    let mut args = Vec::new();

    if use_first_arg_as_argv0 {
        // With '@' prefix: args[0] becomes argv[0], remaining args follow
        for word in args_str {
            args.push(std::ffi::CString::new(word.as_str()).unwrap());
        }
    } else {
        // Normal case: filename of cmd becomes argv[0], then all args follow
        let exec_name = std::path::PathBuf::from(cmd_str);
        let exec_name = exec_name.file_name().unwrap();
        let exec_name: Vec<u8> = exec_name.to_str().unwrap().bytes().collect();
        let exec_name = std::ffi::CString::new(exec_name).unwrap();

        args.push(exec_name);

        for word in args_str {
            args.push(std::ffi::CString::new(word.as_str()).unwrap());
        }
    }

    (cmd, args)
}

/// Open a terminal device, retrying on EIO.
/// This matches systemd's open_terminal() which retries because a TTY in the
/// process of being closed may temporarily return EIO.
fn open_terminal(path: &std::ffi::CStr, flags: libc::c_int) -> libc::c_int {
    for attempt in 0..20u32 {
        let fd = unsafe { libc::open(path.as_ptr(), flags) };
        if fd >= 0 {
            return fd;
        }
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::EIO) {
            return -1;
        }
        // EIO — TTY is being closed, retry after 50ms (max ~1s total)
        if attempt >= 19 {
            return -1;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    -1
}

/// Perform a "destructive" TTY reset before the service uses it.
/// This matches systemd's exec_context_tty_reset(): it resets terminal settings,
/// hangs up prior sessions, and optionally disallocates the VT.
/// This is called BEFORE opening the TTY for stdin so the service gets a clean terminal.
fn tty_reset_destructive(config: &ExecHelperConfig) {
    let tty_path = match config.tty_path.as_deref() {
        Some(p) => p,
        None => std::path::Path::new("/dev/console"),
    };

    let tty_path_cstr = match std::ffi::CString::new(tty_path.to_string_lossy().as_bytes()) {
        Ok(c) => c,
        Err(_) => return,
    };

    // Open the TTY non-blocking and without becoming controlling terminal
    let fd = open_terminal(
        &tty_path_cstr,
        libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC | libc::O_NONBLOCK,
    );
    if fd < 0 {
        eprintln!(
            "[EXEC_HELPER {}] Failed to open TTY {:?} for reset: {}",
            config.name,
            tty_path,
            std::io::Error::last_os_error()
        );
        return;
    }

    if config.tty_reset {
        // Reset terminal to sane defaults via termios
        // This matches systemd's terminal_reset_ioctl()
        unsafe {
            // Disable exclusive mode
            let _ = libc::ioctl(fd, libc::TIOCNXCL);

            // Switch to text mode (KD_TEXT = 0x00)
            let _ = libc::ioctl(
                fd, 0x4B3A_u64, /* KDSETMODE */
                0_i32,      /* KD_TEXT */
            );

            // Reset termios to sane defaults
            let mut termios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(fd, &mut termios) == 0 {
                termios.c_iflag &= !(libc::IGNBRK
                    | libc::BRKINT
                    | libc::ISTRIP
                    | libc::INLCR
                    | libc::IGNCR
                    | libc::IUCLC);
                termios.c_iflag |= libc::ICRNL | libc::IMAXBEL | libc::IUTF8;
                termios.c_oflag |= libc::ONLCR | libc::OPOST;
                termios.c_cflag |= libc::CREAD;
                termios.c_lflag = libc::ISIG
                    | libc::ICANON
                    | libc::IEXTEN
                    | libc::ECHO
                    | libc::ECHOE
                    | libc::ECHOK
                    | libc::ECHOCTL
                    | libc::ECHOKE;

                termios.c_cc[libc::VINTR] = 3; // ^C
                termios.c_cc[libc::VQUIT] = 28; // ^\
                termios.c_cc[libc::VERASE] = 127;
                termios.c_cc[libc::VKILL] = 21; // ^U
                termios.c_cc[libc::VEOF] = 4; // ^D
                termios.c_cc[libc::VSTART] = 17; // ^Q
                termios.c_cc[libc::VSTOP] = 19; // ^S
                termios.c_cc[libc::VSUSP] = 26; // ^Z
                termios.c_cc[libc::VLNEXT] = 22; // ^V
                termios.c_cc[libc::VWERASE] = 23; // ^W
                termios.c_cc[libc::VREPRINT] = 18; // ^R
                termios.c_cc[libc::VEOL] = 0;
                termios.c_cc[libc::VEOL2] = 0;
                termios.c_cc[libc::VTIME] = 0;
                termios.c_cc[libc::VMIN] = 1;

                let _ = libc::tcsetattr(fd, libc::TCSANOW, &termios);
            }

            // Flush all pending I/O
            let _ = libc::tcflush(fd, libc::TCIOFLUSH);
        }
    }

    if config.tty_vhangup {
        // Send TIOCVHANGUP — this disconnects any previous sessions from the TTY.
        // This is critical: without it, switching to the VT may show a stale/dead session.
        unsafe {
            let ret = libc::ioctl(fd, libc::TIOCVHANGUP);
            if ret < 0 {
                eprintln!(
                    "[EXEC_HELPER {}] TIOCVHANGUP failed on {:?}: {}",
                    config.name,
                    tty_path,
                    std::io::Error::last_os_error()
                );
            }
        }
    }

    // Close the fd used for reset — we'll re-open it for actual use.
    // After vhangup the fd is dead anyway.
    unsafe {
        libc::close(fd);
    }

    if config.tty_vt_disallocate {
        // Try to disallocate or at least clear the VT.
        // Extract VT number from path like /dev/tty9
        let tty_str = tty_path.to_string_lossy();
        let tty_name = tty_str.strip_prefix("/dev/").unwrap_or(&tty_str);
        if let Some(vt_num_str) = tty_name.strip_prefix("tty") {
            if let Ok(vt_num) = vt_num_str.parse::<libc::c_int>() {
                if vt_num > 0 {
                    // Try VT_DISALLOCATE via /dev/tty0
                    let tty0 = std::ffi::CString::new("/dev/tty0").unwrap();
                    let tty0_fd = open_terminal(
                        &tty0,
                        libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC | libc::O_NONBLOCK,
                    );
                    if tty0_fd >= 0 {
                        let ret = unsafe {
                            libc::ioctl(tty0_fd, 0x5608 /* VT_DISALLOCATE */, vt_num)
                        };
                        unsafe {
                            libc::close(tty0_fd);
                        }
                        if ret >= 0 {
                            return; // Successfully disallocated
                        }
                        // EBUSY means the VT is active — fall through to clear it
                    }
                }
            }
        }

        // If we can't disallocate, at least clear the screen
        let clear_fd = open_terminal(
            &tty_path_cstr,
            libc::O_WRONLY | libc::O_NOCTTY | libc::O_CLOEXEC | libc::O_NONBLOCK,
        );
        if clear_fd >= 0 {
            let clear_seq = b"\x1b[r\x1b[H\x1b[3J\x1bc";
            unsafe {
                let _ = libc::write(clear_fd, clear_seq.as_ptr().cast(), clear_seq.len());
                libc::close(clear_fd);
            }
        }
    }
}

/// Set up stdin for the service based on the StandardInput= setting.
/// Called after reading the exec_helper config (which consumed the original stdin).
/// When StandardOutput=tty or StandardError=tty is set but StandardInput is NOT a TTY,
/// we need to independently open the TTY for output. This matches systemd's behavior
/// where `StandardOutput=tty` always connects stdout to the TTY regardless of stdin.
fn setup_tty_output(config: &ExecHelperConfig) {
    if !config.stdout_is_tty && !config.stderr_is_tty {
        return;
    }
    // Only needed when stdin is NOT a TTY (when stdin IS a TTY, setup_stdin
    // already dup2'd the TTY fd onto stdout/stderr via stdout_is_inherit).
    match config.stdin_option {
        StandardInput::Tty | StandardInput::TtyForce | StandardInput::TtyFail => return,
        StandardInput::Null => {}
    }

    let tty_path = config
        .tty_path
        .as_deref()
        .unwrap_or(Path::new("/dev/console"));
    let tty_path_cstr = match std::ffi::CString::new(tty_path.to_string_lossy().as_bytes()) {
        Ok(c) => c,
        Err(_) => {
            eprintln!(
                "[EXEC_HELPER {}] Invalid TTYPath for output: {:?}",
                config.name, tty_path
            );
            return;
        }
    };

    let tty_fd = open_terminal(&tty_path_cstr, libc::O_WRONLY | libc::O_NOCTTY);
    if tty_fd < 0 {
        eprintln!(
            "[EXEC_HELPER {}] Failed to open TTY {:?} for output: {}",
            config.name,
            tty_path,
            std::io::Error::last_os_error()
        );
        return;
    }

    if config.stdout_is_tty {
        unsafe {
            libc::dup2(tty_fd, libc::STDOUT_FILENO);
        }
    }
    if config.stderr_is_tty {
        unsafe {
            libc::dup2(tty_fd, libc::STDERR_FILENO);
        }
    }
    if tty_fd != libc::STDOUT_FILENO && tty_fd != libc::STDERR_FILENO {
        unsafe {
            libc::close(tty_fd);
        }
    }
}

fn setup_stdin(config: &ExecHelperConfig) {
    match config.stdin_option {
        StandardInput::Null => {
            // Open /dev/null as stdin
            let null_fd = unsafe {
                libc::open(
                    b"/dev/null\0".as_ptr().cast(),
                    libc::O_RDONLY | libc::O_CLOEXEC,
                )
            };
            if null_fd < 0 {
                eprintln!(
                    "[EXEC_HELPER {}] Failed to open /dev/null for stdin: {}",
                    config.name,
                    std::io::Error::last_os_error()
                );
                std::process::exit(1);
            }
            if null_fd != libc::STDIN_FILENO {
                unsafe {
                    libc::dup2(null_fd, libc::STDIN_FILENO);
                    libc::close(null_fd);
                }
            }
        }
        StandardInput::Tty | StandardInput::TtyForce | StandardInput::TtyFail => {
            let tty_path = config
                .tty_path
                .as_deref()
                .unwrap_or(Path::new("/dev/console"));
            let tty_path_cstr = match std::ffi::CString::new(tty_path.to_string_lossy().as_bytes())
            {
                Ok(c) => c,
                Err(_) => {
                    eprintln!(
                        "[EXEC_HELPER {}] Invalid TTYPath: {:?}",
                        config.name, tty_path
                    );
                    std::process::exit(1);
                }
            };

            // Become session leader so we can acquire a controlling terminal.
            // This is required for ALL tty modes, not just tty-force.
            // Without being a session leader, TIOCSCTTY will fail and the
            // shell won't have a controlling terminal (no job control, no
            // signals, etc).
            //
            // Note: setsid() may fail with EPERM if we are already a session
            // leader (e.g. fork_child already called setsid() for us). That's
            // fine — we just need to BE a session leader, not create a new one.
            unsafe {
                let ret = libc::setsid();
                if ret == -1 {
                    let err = std::io::Error::last_os_error();
                    // EPERM means we're already a session leader — that's OK.
                    if err.raw_os_error() != Some(libc::EPERM) {
                        eprintln!("[EXEC_HELPER {}] setsid() failed: {}", config.name, err);
                    }
                }
            }

            // Use open_terminal() which retries on EIO, matching systemd behavior
            let tty_fd = open_terminal(&tty_path_cstr, libc::O_RDWR | libc::O_NOCTTY);
            if tty_fd < 0 {
                let err = std::io::Error::last_os_error();
                eprintln!(
                    "[EXEC_HELPER {}] Failed to open TTY {:?} for stdin: {}",
                    config.name, tty_path, err
                );
                if config.stdin_option == StandardInput::TtyFail {
                    std::process::exit(1);
                }
                // For tty/tty-force, fall back to /dev/null
                eprintln!(
                    "[EXEC_HELPER {}] Falling back to /dev/null for stdin",
                    config.name
                );
                let null_fd = unsafe { libc::open(b"/dev/null\0".as_ptr().cast(), libc::O_RDONLY) };
                if null_fd >= 0 && null_fd != libc::STDIN_FILENO {
                    unsafe {
                        libc::dup2(null_fd, libc::STDIN_FILENO);
                        libc::close(null_fd);
                    }
                }
                return;
            }

            // Make this TTY our controlling terminal.
            // For tty-force, pass 1 to steal the TTY even if another session owns it.
            // For tty/tty-fail, pass 0 which will fail if another session owns it.
            // This matches systemd's behavior where all tty modes acquire a
            // controlling terminal — they only differ in how conflicts are handled.
            //
            // Temporarily ignore SIGHUP during TIOCSCTTY, matching systemd's
            // acquire_terminal() — if we already own the tty, TIOCSCTTY can
            // generate a spurious SIGHUP.
            let force_arg: libc::c_int = if config.stdin_option == StandardInput::TtyForce {
                1
            } else {
                0
            };
            unsafe {
                // Ignore SIGHUP during terminal acquisition
                let mut old_sa: libc::sigaction = std::mem::zeroed();
                let mut ignore_sa: libc::sigaction = std::mem::zeroed();
                ignore_sa.sa_sigaction = libc::SIG_IGN;
                libc::sigaction(libc::SIGHUP, &ignore_sa, &mut old_sa);

                let ret = libc::ioctl(tty_fd, libc::TIOCSCTTY, force_arg);

                // Restore old SIGHUP handler
                libc::sigaction(libc::SIGHUP, &old_sa, std::ptr::null_mut());

                if ret < 0 {
                    let err = std::io::Error::last_os_error();
                    eprintln!(
                        "[EXEC_HELPER {}] Failed to acquire controlling terminal {:?}: {}",
                        config.name, tty_path, err
                    );
                    if config.stdin_option == StandardInput::TtyFail {
                        libc::close(tty_fd);
                        std::process::exit(1);
                    }
                    // For tty/tty-force, continue anyway — the fd is still usable
                    // for I/O even without being the controlling terminal.
                }
            }

            // Dup the TTY fd onto stdin
            if tty_fd != libc::STDIN_FILENO {
                unsafe {
                    libc::dup2(tty_fd, libc::STDIN_FILENO);
                    libc::close(tty_fd);
                }
            }

            // Set stdout/stderr to the TTY when configured as inherit.
            // This is the typical configuration for debug-shell and similar
            // interactive services (StandardOutput=inherit, StandardError=inherit).
            if config.stdout_is_inherit {
                unsafe {
                    libc::dup2(libc::STDIN_FILENO, libc::STDOUT_FILENO);
                }
            }
            if config.stderr_is_inherit {
                unsafe {
                    libc::dup2(libc::STDIN_FILENO, libc::STDERR_FILENO);
                }
            }
        }
    }
}

pub fn run_exec_helper() {
    let config: ExecHelperConfig = serde_json::from_reader(std::io::stdin()).unwrap();

    nix::unistd::close(libc::STDIN_FILENO).expect("I want to be able to close this fd!");

    // Perform "destructive" TTY reset before opening the TTY for stdin.
    // This matches systemd's exec_context_tty_reset() which is called before
    // setup_input(). It resets terminal settings, hangs up prior sessions, and
    // optionally disallocates the VT — ensuring the service gets a clean terminal.
    match config.stdin_option {
        StandardInput::Tty | StandardInput::TtyForce | StandardInput::TtyFail => {
            if config.tty_reset || config.tty_vhangup || config.tty_vt_disallocate {
                tty_reset_destructive(&config);
            }
        }
        _ => {}
    }

    // Set up stdin for the actual service process
    setup_stdin(&config);

    // If StandardOutput=tty or StandardError=tty but stdin is NOT a TTY,
    // open the TTY independently for output.
    setup_tty_output(&config);

    // Apply LimitNOFILE resource limit before anything else
    if let Some(ref limit) = config.limit_nofile {
        let soft = match limit.soft {
            RLimitValue::Value(v) => v as libc::rlim_t,
            RLimitValue::Infinity => libc::RLIM_INFINITY,
        };
        let hard = match limit.hard {
            RLimitValue::Value(v) => v as libc::rlim_t,
            RLimitValue::Infinity => libc::RLIM_INFINITY,
        };
        let rlim = libc::rlimit {
            rlim_cur: soft,
            rlim_max: hard,
        };
        let ret = unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &rlim) };
        if ret != 0 {
            eprintln!(
                "[EXEC_HELPER {}] Failed to set RLIMIT_NOFILE (soft={}, hard={}): {}",
                config.name,
                soft,
                hard,
                std::io::Error::last_os_error()
            );
            std::process::exit(1);
        }
    }

    if let Err(e) =
        crate::services::fork_os_specific::post_fork_os_specific(&config.platform_specific)
    {
        eprintln!("[FORK_CHILD {}] postfork error: {}", config.name, e);
        std::process::exit(1);
    }

    // Import credentials from the system credential store into a per-service
    // credential directory. This must happen BEFORE dropping privileges,
    // because /run/credentials/ is typically only writable by root.
    // Matches systemd's credential directives:
    //   SetCredential=       — write inline data to credential dir
    //   LoadCredential=      — copy file to credential dir
    //   ImportCredential=    — glob-match from system credential stores
    // The order matches systemd: SetCredential first (lowest priority,
    // can be overridden), then LoadCredential/LoadCredentialEncrypted,
    // then ImportCredential (highest priority, won't overwrite).
    let has_credentials = !config.import_credentials.is_empty()
        || !config.load_credentials.is_empty()
        || !config.load_credentials_encrypted.is_empty()
        || !config.set_credentials.is_empty()
        || !config.set_credentials_encrypted.is_empty();
    if has_credentials {
        setup_credentials(&config);
    }

    // Create state directories under /var/lib/ and set STATE_DIRECTORY env var.
    // This must happen BEFORE dropping privileges, because /var/lib/ is
    // typically only writable by root. systemd does the same: it creates
    // and chowns state directories while still running as root, then drops
    // privileges before exec'ing the service binary.
    if !config.state_directory.is_empty() {
        let base = Path::new("/var/lib");
        let mut full_paths = Vec::new();
        for dir_name in &config.state_directory {
            let full_path = base.join(dir_name);
            if let Err(e) = std::fs::create_dir_all(&full_path) {
                eprintln!(
                    "[EXEC_HELPER {}] Failed to create state directory {:?}: {}",
                    config.name, full_path, e
                );
                std::process::exit(1);
            }
            // Set ownership to the service user/group
            let uid = nix::unistd::Uid::from_raw(config.user);
            let gid = nix::unistd::Gid::from_raw(config.group);
            if let Err(e) = nix::unistd::chown(&full_path, Some(uid), Some(gid)) {
                eprintln!(
                    "[EXEC_HELPER {}] Failed to chown state directory {:?}: {}",
                    config.name, full_path, e
                );
                std::process::exit(1);
            }
            full_paths.push(full_path.to_string_lossy().into_owned());
        }
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("STATE_DIRECTORY", full_paths.join(":")) };
    }

    // Create logs directories under /var/log/ and set LOGS_DIRECTORY env var.
    // Same privilege requirements as state directories: must happen before
    // dropping privileges because /var/log/ is typically only writable by root.
    if !config.logs_directory.is_empty() {
        let base = Path::new("/var/log");
        let mode = config.logs_directory_mode.unwrap_or(0o755);
        let mut full_paths = Vec::new();
        for dir_name in &config.logs_directory {
            let full_path = base.join(dir_name);
            if let Err(e) = std::fs::create_dir_all(&full_path) {
                eprintln!(
                    "[EXEC_HELPER {}] Failed to create logs directory {:?}: {}",
                    config.name, full_path, e
                );
                std::process::exit(1);
            }
            // Apply LogsDirectoryMode=
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(mode);
            if let Err(e) = std::fs::set_permissions(&full_path, perms) {
                eprintln!(
                    "[EXEC_HELPER {}] Failed to set mode {:o} on logs directory {:?}: {}",
                    config.name, mode, full_path, e
                );
                std::process::exit(1);
            }
            // Set ownership to the service user/group
            let uid = nix::unistd::Uid::from_raw(config.user);
            let gid = nix::unistd::Gid::from_raw(config.group);
            if let Err(e) = nix::unistd::chown(&full_path, Some(uid), Some(gid)) {
                eprintln!(
                    "[EXEC_HELPER {}] Failed to chown logs directory {:?}: {}",
                    config.name, full_path, e
                );
                std::process::exit(1);
            }
            full_paths.push(full_path.to_string_lossy().into_owned());
        }
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("LOGS_DIRECTORY", full_paths.join(":")) };
    }

    // Create runtime directories under /run/ and set RUNTIME_DIRECTORY env var.
    // Same privilege requirements as state directories: must happen before
    // dropping privileges because /run/ is typically only writable by root.
    if !config.runtime_directory.is_empty() {
        let base = Path::new("/run");
        let mut full_paths = Vec::new();
        for dir_name in &config.runtime_directory {
            let full_path = base.join(dir_name);
            if let Err(e) = std::fs::create_dir_all(&full_path) {
                eprintln!(
                    "[EXEC_HELPER {}] Failed to create runtime directory {:?}: {}",
                    config.name, full_path, e
                );
                std::process::exit(1);
            }
            // Set ownership to the service user/group
            let uid = nix::unistd::Uid::from_raw(config.user);
            let gid = nix::unistd::Gid::from_raw(config.group);
            if let Err(e) = nix::unistd::chown(&full_path, Some(uid), Some(gid)) {
                eprintln!(
                    "[EXEC_HELPER {}] Failed to chown runtime directory {:?}: {}",
                    config.name, full_path, e
                );
                std::process::exit(1);
            }
            full_paths.push(full_path.to_string_lossy().into_owned());
        }
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("RUNTIME_DIRECTORY", full_paths.join(":")) };
    }

    // Apply OOMScoreAdjust= setting. Write the value to /proc/self/oom_score_adj
    // before dropping privileges, because negative values (making the process
    // less likely to be OOM-killed) require root or CAP_SYS_RESOURCE.
    if let Some(adj) = config.oom_score_adjust {
        let path = Path::new("/proc/self/oom_score_adj");
        if let Err(e) = std::fs::write(path, format!("{adj}")) {
            eprintln!(
                "[EXEC_HELPER {}] Failed to set OOMScoreAdjust to {} ({:?}): {}",
                config.name, adj, path, e
            );
            // Non-fatal: log and continue, matching systemd's lenient behavior
            // when the kernel rejects the value or the file is unavailable.
        }
    }

    // Resolve ambient capabilities BEFORE dropping privileges so we can
    // set PR_SET_KEEPCAPS and retain them across the UID change.
    let ambient_caps = resolve_ambient_caps(&config.ambient_capabilities);

    if nix::unistd::getuid().is_root() {
        // If ambient capabilities are requested, tell the kernel to keep
        // permitted capabilities across the setuid() call.  Without this
        // the capability sets are cleared when changing UID from root to
        // an unprivileged user.
        if !ambient_caps.is_empty() {
            let ret = unsafe { libc::prctl(libc::PR_SET_KEEPCAPS, 1, 0, 0, 0) };
            if ret != 0 {
                eprintln!(
                    "[EXEC_HELPER {}] PR_SET_KEEPCAPS failed: {}",
                    config.name,
                    std::io::Error::last_os_error()
                );
            }
        }

        let supp_gids: Vec<nix::unistd::Gid> = config
            .supplementary_groups
            .iter()
            .map(|gid| nix::unistd::Gid::from_raw(*gid))
            .collect();
        match crate::platform::drop_privileges(
            nix::unistd::Gid::from_raw(config.group),
            &supp_gids,
            nix::unistd::Uid::from_raw(config.user),
        ) {
            Ok(()) => { /* Happy */ }
            Err(e) => {
                eprintln!(
                    "[EXEC_HELPER {}] could not drop privileges because: {}",
                    config.name, e
                );
                std::process::exit(1);
            }
        }

        // After dropping privileges, raise the requested ambient
        // capabilities.  We must first re-add each cap to the permitted
        // and effective sets (PR_SET_KEEPCAPS only preserves the permitted
        // set; the effective set is cleared on setuid).  Then we can raise
        // the cap as ambient.
        //
        // We use raw structs matching <linux/capability.h> because the
        // libc crate doesn't expose __user_cap_header_struct /
        // __user_cap_data_struct.
        #[repr(C)]
        struct CapHeader {
            version: u32,
            pid: i32,
        }
        #[repr(C)]
        struct CapData {
            effective: u32,
            permitted: u32,
            inheritable: u32,
        }
        const CAP_V3: u32 = 0x20080522; // _LINUX_CAPABILITY_VERSION_3

        if !ambient_caps.is_empty() {
            // Read current capability sets (version 3 uses two CapData
            // elements covering caps 0-31 and 32-63).
            let mut hdr = CapHeader {
                version: CAP_V3,
                pid: 0,
            };
            let mut data: [CapData; 2] = unsafe { std::mem::zeroed() };

            if unsafe { libc::syscall(libc::SYS_capget, &mut hdr as *mut _, data.as_mut_ptr()) }
                != 0
            {
                eprintln!(
                    "[EXEC_HELPER {}] capget failed: {}",
                    config.name,
                    std::io::Error::last_os_error()
                );
            } else {
                // Set all requested caps in permitted, effective, AND
                // inheritable sets (ambient requires inheritable too).
                for &cap in &ambient_caps {
                    let idx = (cap / 32) as usize;
                    let bit = 1u32 << (cap % 32);
                    if idx < 2 {
                        data[idx].permitted |= bit;
                        data[idx].effective |= bit;
                        data[idx].inheritable |= bit;
                    }
                }

                hdr.version = CAP_V3;
                hdr.pid = 0;
                if unsafe { libc::syscall(libc::SYS_capset, &hdr as *const _, data.as_ptr()) } != 0
                {
                    eprintln!(
                        "[EXEC_HELPER {}] capset failed: {}",
                        config.name,
                        std::io::Error::last_os_error()
                    );
                }
            }

            // Now raise each cap as ambient.
            for &cap in &ambient_caps {
                let ret = unsafe {
                    libc::prctl(
                        libc::PR_CAP_AMBIENT,
                        libc::PR_CAP_AMBIENT_RAISE,
                        cap as libc::c_ulong,
                        0,
                        0,
                    )
                };
                if ret != 0 {
                    eprintln!(
                        "[EXEC_HELPER {}] PR_CAP_AMBIENT_RAISE failed for cap {}: {}",
                        config.name,
                        cap,
                        std::io::Error::last_os_error()
                    );
                }
            }
        }
    }

    let (cmd, args) = prepare_exec_args(&config.cmd, &config.args, config.use_first_arg_as_argv0);

    // change working directory if configured
    if let Some(ref dir) = config.working_directory {
        let dir = if dir == Path::new("~") {
            // Resolve ~ to the home directory of the current user
            match std::env::var("HOME") {
                Ok(home) => PathBuf::from(home),
                Err(_) => {
                    eprintln!(
                        "[EXEC_HELPER {}] WorkingDirectory=~ but $HOME is not set",
                        config.name
                    );
                    std::process::exit(1);
                }
            }
        } else {
            dir.clone()
        };
        if let Err(e) = std::env::set_current_dir(&dir) {
            eprintln!(
                "[EXEC_HELPER {}] Failed to set working directory to {:?}: {}",
                config.name, dir, e
            );
            std::process::exit(1);
        }
    }

    // setup environment vars
    for (k, v) in &config.env {
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var(k, v) };
    }

    // Only set LISTEN_PID when LISTEN_FDS is present in the environment.
    // Real systemd only sets LISTEN_PID for socket-activated services.
    // Setting it unconditionally confuses services like systemd-logind that
    // call sd_listen_fds_with_names() and get unexpected results when
    // LISTEN_PID matches but there are no actual FDs to receive.
    // TODO: Audit that the environment access only happens in single-threaded code.
    if std::env::var("LISTEN_FDS").is_ok() {
        unsafe { std::env::set_var("LISTEN_PID", format!("{}", nix::unistd::getpid())) };
    }

    // Apply IgnoreSIGPIPE= setting. When true (the default), set SIGPIPE to
    // SIG_IGN so that writes to broken pipes produce EPIPE errors instead of
    // killing the process. When false, restore the default disposition.
    unsafe {
        if config.ignore_sigpipe {
            libc::signal(libc::SIGPIPE, libc::SIG_IGN);
        } else {
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
        }
    }

    // Write utmp/wtmp login record if UtmpIdentifier= is set.
    if config.utmp_identifier.is_some() {
        write_utmp_record(&config);
    }

    nix::unistd::execv(&cmd, &args).unwrap();
}

// ---------------------------------------------------------------------------
// ImportCredential= helpers
// ---------------------------------------------------------------------------

/// System credential store directories searched in order (matching systemd).
const CREDENTIAL_STORES: &[&str] = &[
    "/run/credentials/@system",
    "/run/credstore",
    "/etc/credstore",
];

/// Path to the host encryption key used for credential encryption/decryption.
const HOST_KEY_PATH: &str = "/var/lib/systemd/credential.secret";

/// Magic bytes identifying an encrypted credential blob: "sHc\0".
const CRED_MAGIC: [u8; 4] = [0x73, 0x48, 0x63, 0x00];

/// Fixed header size: magic(4) + seal_type(4) + timestamp(8) + not_after(8) + name_len(4) = 28.
const CRED_HEADER_FIXED_SIZE: usize = 28;

/// AES-256-GCM nonce size.
const CRED_AES_IV_SIZE: usize = 12;

/// Seal type: null key (SHA-256 of credential name only).
const CRED_SEAL_NULL: u32 = 0;
/// Seal type: host key (SHA-256 of host_key || credential_name).
const CRED_SEAL_HOST: u32 = 1;

/// Attempt to decrypt an encrypted credential blob.
///
/// The blob may be either raw binary (the wire format produced by
/// `systemd-creds encrypt`) or Base64-encoded. This function tries
/// Base64 decoding first; if that fails it treats the input as raw.
///
/// Returns `Ok(plaintext_bytes)` on success, or `Err(message)` on failure.
/// On any error the caller should fall back to writing the data as-is
/// (matching the previous behaviour) so that services which do their own
/// decryption still work.
fn try_decrypt_credential(data: &[u8], cred_name: &str) -> Result<Vec<u8>, String> {
    // Try Base64 decode first (systemd-creds output is always Base64).
    let blob = {
        let as_str = String::from_utf8_lossy(data);
        let cleaned: String = as_str.chars().filter(|c| !c.is_whitespace()).collect();
        BASE64.decode(&cleaned).unwrap_or_else(|_| data.to_vec())
    };

    if blob.len() < CRED_HEADER_FIXED_SIZE {
        return Err("blob too short for credential header".into());
    }

    // Validate magic.
    if blob[0..4] != CRED_MAGIC {
        return Err("invalid credential magic".into());
    }

    let seal_type = u32::from_le_bytes(blob[4..8].try_into().unwrap());
    let _timestamp = u64::from_le_bytes(blob[8..16].try_into().unwrap());
    let not_after = u64::from_le_bytes(blob[16..24].try_into().unwrap());
    let name_len = u32::from_le_bytes(blob[24..28].try_into().unwrap()) as usize;

    let name_end = CRED_HEADER_FIXED_SIZE + name_len;
    if blob.len() < name_end + CRED_AES_IV_SIZE {
        return Err("blob too short for name + IV".into());
    }

    // Check expiry.
    if not_after != 0 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        if now > not_after {
            return Err(format!(
                "credential expired (not_after={not_after}, now={now})"
            ));
        }
    }

    // Extract IV and ciphertext.
    let iv = &blob[name_end..name_end + CRED_AES_IV_SIZE];
    let ciphertext = &blob[name_end + CRED_AES_IV_SIZE..];

    if ciphertext.len() < 16 {
        // AES-GCM tag is 16 bytes minimum
        return Err("blob too short for ciphertext + GCM tag".into());
    }

    // Derive AES-256 key based on seal type.
    let aes_key: [u8; 32] = match seal_type {
        CRED_SEAL_NULL => {
            let mut h = Sha256::new();
            h.update(cred_name.as_bytes());
            h.finalize().into()
        }
        CRED_SEAL_HOST => {
            let host_key = std::fs::read(HOST_KEY_PATH)
                .map_err(|e| format!("cannot read host key {HOST_KEY_PATH}: {e}"))?;
            let mut h = Sha256::new();
            h.update(&host_key);
            h.update(cred_name.as_bytes());
            h.finalize().into()
        }
        other => {
            return Err(format!(
                "unsupported seal type {other} (TPM2 not implemented)"
            ));
        }
    };

    let cipher =
        Aes256Gcm::new_from_slice(&aes_key).map_err(|e| format!("AES init failed: {e}"))?;
    let nonce = Nonce::from_slice(iv);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "decryption failed (wrong key or corrupted data)".into())
}

/// Set up the per-service credential directory, processing all credential
/// directives in the correct priority order (matching systemd):
///
/// 1. `SetCredential=` / `SetCredentialEncrypted=` — lowest priority, written
///    first so they can be overridden by later directives.
/// 2. `LoadCredential=` / `LoadCredentialEncrypted=` — medium priority, copies
///    from file paths. Overwrites credentials set by `SetCredential=`.
/// 3. `ImportCredential=` — highest priority, glob-matches from system
///    credential stores. Does NOT overwrite existing credentials (first match
///    wins across stores, but won't override LoadCredential/SetCredential).
///
/// Encrypted variants (`SetCredentialEncrypted=`, `LoadCredentialEncrypted=`)
/// are decrypted at runtime using AES-256-GCM with a key derived from the
/// host secret (`/var/lib/systemd/credential.secret`) or a null key. If
/// decryption fails (e.g. no host key, wrong key, corrupted data), the
/// encrypted content is written as-is so services that handle their own
/// decryption still work.
fn setup_credentials(config: &ExecHelperConfig) {
    let cred_dir = PathBuf::from(format!("/run/credentials/{}", config.name));

    // Create the credential directory.
    if let Err(e) = std::fs::create_dir_all(&cred_dir) {
        eprintln!(
            "[EXEC_HELPER {}] Failed to create credentials directory {:?}: {}",
            config.name, cred_dir, e
        );
        // Non-fatal — the service may still work without credentials.
        return;
    }

    // Restrict permissions to owner-only (0o700), matching systemd.
    let ret = unsafe {
        libc::chmod(
            std::ffi::CString::new(cred_dir.to_string_lossy().as_bytes())
                .unwrap()
                .as_ptr(),
            0o700,
        )
    };
    if ret != 0 {
        eprintln!(
            "[EXEC_HELPER {}] Failed to chmod credentials directory {:?}: {}",
            config.name,
            cred_dir,
            std::io::Error::last_os_error()
        );
    }

    // Chown to the service user/group so the service can read them.
    let uid = nix::unistd::Uid::from_raw(config.user);
    let gid = nix::unistd::Gid::from_raw(config.group);
    if let Err(e) = nix::unistd::chown(&cred_dir, Some(uid), Some(gid)) {
        eprintln!(
            "[EXEC_HELPER {}] Failed to chown credentials directory {:?}: {}",
            config.name, cred_dir, e
        );
    }

    let mut _wrote = 0usize;

    // --- Phase 1: SetCredential= (lowest priority) ---
    for (id, data) in &config.set_credentials {
        let dst = cred_dir.join(id);
        match std::fs::write(&dst, data.as_bytes()) {
            Ok(()) => {
                set_credential_perms(&dst, uid, gid);
                _wrote += 1;
            }
            Err(e) => {
                eprintln!(
                    "[EXEC_HELPER {}] Failed to write SetCredential {:?}: {}",
                    config.name, id, e
                );
            }
        }
    }

    // --- Phase 1b: SetCredentialEncrypted= (same priority as SetCredential) ---
    for (id, data) in &config.set_credentials_encrypted {
        let dst = cred_dir.join(id);
        // Don't overwrite a credential set by SetCredential= with the same ID
        // (first writer wins within the same priority level).
        if dst.exists() {
            continue;
        }
        // Try to decrypt; fall back to writing as-is if decryption fails.
        let write_data = match try_decrypt_credential(data.as_bytes(), id) {
            Ok(plaintext) => plaintext,
            Err(e) => {
                eprintln!(
                    "[EXEC_HELPER {}] SetCredentialEncrypted {:?}: decryption failed ({}), writing as-is",
                    config.name, id, e
                );
                data.as_bytes().to_vec()
            }
        };
        match std::fs::write(&dst, &write_data) {
            Ok(()) => {
                set_credential_perms(&dst, uid, gid);
                _wrote += 1;
            }
            Err(e) => {
                eprintln!(
                    "[EXEC_HELPER {}] Failed to write SetCredentialEncrypted {:?}: {}",
                    config.name, id, e
                );
            }
        }
    }

    // --- Phase 2: LoadCredential= (overwrites SetCredential) ---
    for (id, path_str) in &config.load_credentials {
        let src = Path::new(path_str);

        // If the path is not absolute, search credential stores (matching
        // systemd's behaviour for relative LoadCredential= paths).
        let resolved = if src.is_absolute() {
            src.to_path_buf()
        } else {
            let mut found = None;
            for store_dir in CREDENTIAL_STORES {
                let candidate = Path::new(store_dir).join(path_str);
                if candidate.exists() {
                    found = Some(candidate);
                    break;
                }
            }
            match found {
                Some(p) => p,
                None => {
                    eprintln!(
                        "[EXEC_HELPER {}] LoadCredential {:?}: path {:?} not found (searched credential stores)",
                        config.name, id, path_str
                    );
                    continue;
                }
            }
        };

        if !resolved.exists() {
            eprintln!(
                "[EXEC_HELPER {}] LoadCredential {:?}: source {:?} does not exist",
                config.name, id, resolved
            );
            continue;
        }

        let dst = cred_dir.join(id);

        // If the source is a directory, load all files within it as
        // sub-credentials (matching systemd behaviour).
        if resolved.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&resolved) {
                for entry in entries.flatten() {
                    if entry.path().is_file() {
                        let sub_dst = cred_dir.join(entry.file_name());
                        match std::fs::copy(entry.path(), &sub_dst) {
                            Ok(_) => {
                                set_credential_perms(&sub_dst, uid, gid);
                                _wrote += 1;
                            }
                            Err(e) => {
                                eprintln!(
                                    "[EXEC_HELPER {}] Failed to load credential {:?} from dir {:?}: {}",
                                    config.name,
                                    entry.file_name(),
                                    resolved,
                                    e
                                );
                            }
                        }
                    }
                }
            }
        } else {
            match std::fs::copy(&resolved, &dst) {
                Ok(_) => {
                    set_credential_perms(&dst, uid, gid);
                    _wrote += 1;
                }
                Err(e) => {
                    eprintln!(
                        "[EXEC_HELPER {}] Failed to load credential {:?} from {:?}: {}",
                        config.name, id, resolved, e
                    );
                }
            }
        }
    }

    // --- Phase 2b: LoadCredentialEncrypted= (same priority as LoadCredential) ---
    for (id, path_str) in &config.load_credentials_encrypted {
        let src = Path::new(path_str);

        let resolved = if src.is_absolute() {
            src.to_path_buf()
        } else {
            let mut found = None;
            for store_dir in CREDENTIAL_STORES {
                let candidate = Path::new(store_dir).join(path_str);
                if candidate.exists() {
                    found = Some(candidate);
                    break;
                }
            }
            match found {
                Some(p) => p,
                None => {
                    eprintln!(
                        "[EXEC_HELPER {}] LoadCredentialEncrypted {:?}: path {:?} not found",
                        config.name, id, path_str
                    );
                    continue;
                }
            }
        };

        if !resolved.exists() || !resolved.is_file() {
            eprintln!(
                "[EXEC_HELPER {}] LoadCredentialEncrypted {:?}: source {:?} not found or not a file",
                config.name, id, resolved
            );
            continue;
        }

        let dst = cred_dir.join(id);

        // Read the encrypted file and try to decrypt it.
        match std::fs::read(&resolved) {
            Ok(encrypted_data) => {
                let write_data = match try_decrypt_credential(&encrypted_data, id) {
                    Ok(plaintext) => plaintext,
                    Err(e) => {
                        eprintln!(
                            "[EXEC_HELPER {}] LoadCredentialEncrypted {:?}: decryption failed ({}), writing as-is",
                            config.name, id, e
                        );
                        encrypted_data
                    }
                };
                match std::fs::write(&dst, &write_data) {
                    Ok(()) => {
                        set_credential_perms(&dst, uid, gid);
                        _wrote += 1;
                    }
                    Err(e) => {
                        eprintln!(
                            "[EXEC_HELPER {}] Failed to write decrypted credential {:?}: {}",
                            config.name, id, e
                        );
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "[EXEC_HELPER {}] Failed to read encrypted credential {:?} from {:?}: {}",
                    config.name, id, resolved, e
                );
            }
        }
    }

    // --- Phase 3: ImportCredential= (highest priority, won't overwrite) ---
    for pattern in &config.import_credentials {
        for store_dir in CREDENTIAL_STORES {
            let store = Path::new(store_dir);
            if !store.is_dir() {
                continue;
            }

            let entries = match std::fs::read_dir(store) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let name_str = file_name.to_string_lossy();

                if !glob_match(pattern, &name_str) {
                    continue;
                }

                let src = entry.path();
                if !src.is_file() {
                    continue;
                }

                let dst = cred_dir.join(&file_name);

                // Don't overwrite — first match wins (higher-priority store),
                // and ImportCredential never overwrites LoadCredential/SetCredential.
                if dst.exists() {
                    continue;
                }

                match std::fs::copy(&src, &dst) {
                    Ok(_) => {
                        set_credential_perms(&dst, uid, gid);
                        _wrote += 1;
                    }
                    Err(e) => {
                        eprintln!(
                            "[EXEC_HELPER {}] Failed to import credential {:?} -> {:?}: {}",
                            config.name, src, dst, e
                        );
                    }
                }
            }
        }
    }

    // Always set the env var so the service knows where to look,
    // even if no credentials were found (matches systemd behaviour).
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("CREDENTIALS_DIRECTORY", &cred_dir) };
}

/// Set a credential file to owner-read-only (0o400) and chown to service user/group.
fn set_credential_perms(path: &Path, uid: nix::unistd::Uid, gid: nix::unistd::Gid) {
    let _ = unsafe {
        libc::chmod(
            std::ffi::CString::new(path.to_string_lossy().as_bytes())
                .unwrap()
                .as_ptr(),
            0o400,
        )
    };
    let _ = nix::unistd::chown(path, Some(uid), Some(gid));
}

/// Simple glob matcher supporting `*` (any chars) and `?` (single char).
/// This is intentionally minimal — systemd only uses simple filename globs
/// for ImportCredential=.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    glob_match_inner(pattern.as_bytes(), text.as_bytes())
}

fn glob_match_inner(pattern: &[u8], text: &[u8]) -> bool {
    let mut pi = 0;
    let mut ti = 0;
    let mut star_pi = usize::MAX;
    let mut star_ti = 0;

    while ti < text.len() {
        if pi < pattern.len() && (pattern[pi] == b'?' || pattern[pi] == text[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pattern.len() && pattern[pi] == b'*' {
            star_pi = pi;
            star_ti = ti;
            pi += 1;
        } else if star_pi != usize::MAX {
            pi = star_pi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    while pi < pattern.len() && pattern[pi] == b'*' {
        pi += 1;
    }

    pi == pattern.len()
}

// ---------------------------------------------------------------------------
// utmp / wtmp helpers
// ---------------------------------------------------------------------------

unsafe extern "C" {
    fn updwtmpx(file: *const libc::c_char, ut: *const libc::utmpx);
}

/// Path to the wtmp file (standard glibc location).
const WTMP_PATH: &[u8] = b"/var/log/wtmp\0";

/// Derive the TTY line name from a TTY path (e.g. "/dev/tty1" → "tty1").
/// Falls back to the full path if no `/dev/` prefix is found.
fn tty_line(config: &ExecHelperConfig) -> String {
    let path = config
        .tty_path
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("/dev/console"));
    let s = path.to_string_lossy();
    s.strip_prefix("/dev/").unwrap_or(&s).to_string()
}

/// Copy a Rust string into a fixed-size `c_char` array, truncating and
/// NUL-terminating as needed.
fn fill_c_char_buf(buf: &mut [libc::c_char], src: &str) {
    let bytes = src.as_bytes();
    let len = bytes.len().min(buf.len() - 1);
    for (i, &b) in bytes[..len].iter().enumerate() {
        buf[i] = b as libc::c_char;
    }
    // Remaining bytes are already zero from `mem::zeroed()`.
}

/// Build a `libc::utmpx` record from the current exec-helper config.
fn build_utmpx(config: &ExecHelperConfig, ut_type: libc::c_short) -> libc::utmpx {
    let mut ut: libc::utmpx = unsafe { std::mem::zeroed() };
    ut.ut_type = ut_type;
    ut.ut_pid = nix::unistd::getpid().as_raw();

    let line = tty_line(config);
    fill_c_char_buf(&mut ut.ut_line, &line);

    if let Some(ref id) = config.utmp_identifier {
        fill_c_char_buf(&mut ut.ut_id, id);
    } else {
        // Derive from TTY line — use last 4 characters (matches systemd).
        let id_str = if line.len() > 4 {
            &line[line.len() - 4..]
        } else {
            &line
        };
        fill_c_char_buf(&mut ut.ut_id, id_str);
    }

    // For LOGIN_PROCESS the user field is conventionally "LOGIN".
    // For INIT_PROCESS it is often empty or the service name.
    // For USER_PROCESS it should be the login name.
    match ut_type {
        libc::LOGIN_PROCESS => fill_c_char_buf(&mut ut.ut_user, "LOGIN"),
        libc::USER_PROCESS => {
            // Resolve uid → username if possible.
            if let Some(pw) = nix::unistd::User::from_uid(nix::unistd::Uid::from_raw(config.user))
                .ok()
                .flatten()
            {
                fill_c_char_buf(&mut ut.ut_user, &pw.name);
            }
        }
        _ => { /* INIT_PROCESS / DEAD_PROCESS — user field stays empty */ }
    }

    // Timestamp
    let now = unsafe {
        let mut tv: libc::timeval = std::mem::zeroed();
        libc::gettimeofday(&mut tv, std::ptr::null_mut());
        tv
    };
    ut.ut_tv.tv_sec = now.tv_sec as _;
    ut.ut_tv.tv_usec = now.tv_usec as _;

    ut
}

/// Write the initial utmp + wtmp record before exec'ing the service binary.
fn write_utmp_record(config: &ExecHelperConfig) {
    let ut_type: libc::c_short = match config.utmp_mode {
        UtmpMode::Init => libc::INIT_PROCESS as libc::c_short,
        UtmpMode::Login => libc::LOGIN_PROCESS as libc::c_short,
        UtmpMode::User => libc::USER_PROCESS as libc::c_short,
    };

    let ut = build_utmpx(config, ut_type);

    unsafe {
        libc::setutxent();
        let result = libc::pututxline(&ut);
        libc::endutxent();

        if result.is_null() {
            eprintln!(
                "[EXEC_HELPER {}] Failed to write utmp record: {}",
                config.name,
                std::io::Error::last_os_error()
            );
        }

        // Append to wtmp as well.
        updwtmpx(WTMP_PATH.as_ptr() as *const libc::c_char, &ut);
    }
}

/// Write a DEAD_PROCESS utmp + wtmp record.  Called from the service manager
/// (parent process) when a service that had `UtmpIdentifier=` exits.
///
/// `identifier` is the `UtmpIdentifier=` value, `tty_path` the configured
/// TTY, and `pid` the PID of the exited service process.
pub fn write_utmp_dead_record(
    identifier: &str,
    tty_path: Option<&std::path::Path>,
    pid: nix::unistd::Pid,
) {
    let mut ut: libc::utmpx = unsafe { std::mem::zeroed() };
    ut.ut_type = libc::DEAD_PROCESS as libc::c_short;
    ut.ut_pid = pid.as_raw();

    let tty = tty_path.unwrap_or_else(|| std::path::Path::new("/dev/console"));
    let line = tty
        .to_string_lossy()
        .strip_prefix("/dev/")
        .unwrap_or(&tty.to_string_lossy())
        .to_string();
    fill_c_char_buf(&mut ut.ut_line, &line);
    fill_c_char_buf(&mut ut.ut_id, identifier);

    let now = unsafe {
        let mut tv: libc::timeval = std::mem::zeroed();
        libc::gettimeofday(&mut tv, std::ptr::null_mut());
        tv
    };
    ut.ut_tv.tv_sec = now.tv_sec as _;
    ut.ut_tv.tv_usec = now.tv_usec as _;

    unsafe {
        libc::setutxent();
        let result = libc::pututxline(&ut);
        libc::endutxent();

        if result.is_null() {
            // Non-fatal — the utmp file may not exist or be writable.
            log::warn!(
                "Failed to write DEAD_PROCESS utmp record for id={}: {}",
                identifier,
                std::io::Error::last_os_error()
            );
        }

        updwtmpx(WTMP_PATH.as_ptr() as *const libc::c_char, &ut);
    }
}

// ---------------------------------------------------------------------------
// Tests for credential decryption
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::AeadCore;
    use aes_gcm::aead::OsRng;

    /// Build an encrypted credential blob in our wire format using null-key sealing.
    fn make_encrypted_blob(
        plaintext: &[u8],
        cred_name: &str,
        seal_type: u32,
        timestamp: u64,
        not_after: u64,
    ) -> Vec<u8> {
        // Derive key
        let aes_key: [u8; 32] = if seal_type == CRED_SEAL_NULL {
            let mut h = Sha256::new();
            h.update(cred_name.as_bytes());
            h.finalize().into()
        } else {
            panic!("test helper only supports null seal");
        };

        let cipher = Aes256Gcm::new_from_slice(&aes_key).unwrap();
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher.encrypt(&nonce, plaintext).unwrap();

        let name_bytes = cred_name.as_bytes();
        let name_len = name_bytes.len() as u32;

        let mut blob = Vec::new();
        blob.extend_from_slice(&CRED_MAGIC);
        blob.extend_from_slice(&seal_type.to_le_bytes());
        blob.extend_from_slice(&timestamp.to_le_bytes());
        blob.extend_from_slice(&not_after.to_le_bytes());
        blob.extend_from_slice(&name_len.to_le_bytes());
        blob.extend_from_slice(name_bytes);
        blob.extend_from_slice(nonce.as_slice());
        blob.extend_from_slice(&ciphertext);
        blob
    }

    fn now_usec() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64
    }

    #[test]
    fn test_try_decrypt_null_key_roundtrip() {
        let plaintext = b"super-secret-password";
        let cred_name = "db-pass";
        let blob = make_encrypted_blob(plaintext, cred_name, CRED_SEAL_NULL, now_usec(), 0);

        let result = try_decrypt_credential(&blob, cred_name);
        assert!(result.is_ok(), "decryption failed: {:?}", result.err());
        assert_eq!(result.unwrap(), plaintext);
    }

    #[test]
    fn test_try_decrypt_base64_encoded_roundtrip() {
        let plaintext = b"hello-credential";
        let cred_name = "test-cred";
        let blob = make_encrypted_blob(plaintext, cred_name, CRED_SEAL_NULL, now_usec(), 0);

        // Base64-encode the blob (as systemd-creds would output).
        let b64 = BASE64.encode(&blob);

        let result = try_decrypt_credential(b64.as_bytes(), cred_name);
        assert!(
            result.is_ok(),
            "base64 decryption failed: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), plaintext);
    }

    #[test]
    fn test_try_decrypt_bad_magic() {
        let mut blob = vec![0u8; 100];
        blob[0] = 0xFF; // corrupt magic
        let result = try_decrypt_credential(&blob, "test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("magic"));
    }

    #[test]
    fn test_try_decrypt_truncated_header() {
        let blob = vec![0x73, 0x48, 0x63, 0x00]; // just the magic, no more
        let result = try_decrypt_credential(&blob, "test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too short"));
    }

    #[test]
    fn test_try_decrypt_expired_credential() {
        let plaintext = b"expired-data";
        let cred_name = "expiring";
        // not_after = 1 µs after epoch → already expired
        let blob = make_encrypted_blob(plaintext, cred_name, CRED_SEAL_NULL, 0, 1);

        let result = try_decrypt_credential(&blob, cred_name);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expired"));
    }

    #[test]
    fn test_try_decrypt_not_expired_credential() {
        let plaintext = b"still-valid";
        let cred_name = "future";
        let not_after = now_usec() + 3_600_000_000; // 1 hour from now
        let blob = make_encrypted_blob(plaintext, cred_name, CRED_SEAL_NULL, now_usec(), not_after);

        let result = try_decrypt_credential(&blob, cred_name);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), plaintext);
    }

    #[test]
    fn test_try_decrypt_empty_plaintext() {
        let plaintext = b"";
        let cred_name = "empty";
        let blob = make_encrypted_blob(plaintext, cred_name, CRED_SEAL_NULL, now_usec(), 0);

        let result = try_decrypt_credential(&blob, cred_name);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"");
    }

    #[test]
    fn test_try_decrypt_large_payload() {
        let plaintext: Vec<u8> = (0..8192).map(|i| (i % 256) as u8).collect();
        let cred_name = "big";
        let blob = make_encrypted_blob(&plaintext, cred_name, CRED_SEAL_NULL, now_usec(), 0);

        let result = try_decrypt_credential(&blob, cred_name);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), plaintext);
    }

    #[test]
    fn test_try_decrypt_corrupted_ciphertext() {
        let plaintext = b"important";
        let cred_name = "test";
        let mut blob = make_encrypted_blob(plaintext, cred_name, CRED_SEAL_NULL, now_usec(), 0);

        // Corrupt the last byte (part of the GCM authentication tag).
        let last = blob.len() - 1;
        blob[last] ^= 0xFF;

        let result = try_decrypt_credential(&blob, cred_name);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("decryption failed"));
    }

    #[test]
    fn test_try_decrypt_wrong_credential_name() {
        // Decrypting with the wrong name should fail because the
        // AES key is derived from the credential name.
        let plaintext = b"secret";
        let cred_name = "correct-name";
        let blob = make_encrypted_blob(plaintext, cred_name, CRED_SEAL_NULL, now_usec(), 0);

        let result = try_decrypt_credential(&blob, "wrong-name");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("decryption failed"));
    }

    #[test]
    fn test_try_decrypt_unsupported_seal_type() {
        let plaintext = b"data";
        let cred_name = "test";
        // Use seal type 99 (unsupported).
        let mut blob = make_encrypted_blob(plaintext, cred_name, CRED_SEAL_NULL, now_usec(), 0);
        // Overwrite seal_type field at offset 4..8.
        blob[4..8].copy_from_slice(&99u32.to_le_bytes());

        let result = try_decrypt_credential(&blob, cred_name);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unsupported seal type"));
    }

    #[test]
    fn test_try_decrypt_not_a_credential_blob() {
        // Plain text that isn't a credential blob at all should fail gracefully.
        let result = try_decrypt_credential(b"just plain text data", "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_try_decrypt_base64_with_whitespace() {
        let plaintext = b"whitespace-test";
        let cred_name = "ws";
        let blob = make_encrypted_blob(plaintext, cred_name, CRED_SEAL_NULL, now_usec(), 0);

        // Base64-encode and insert whitespace/newlines (as might appear in unit files).
        let b64 = BASE64.encode(&blob);
        let with_ws = format!("  {}  \n  ", b64);

        let result = try_decrypt_credential(with_ws.as_bytes(), cred_name);
        assert!(
            result.is_ok(),
            "whitespace base64 decryption failed: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), plaintext);
    }

    #[test]
    fn test_glob_match_exact() {
        assert!(glob_match("hello", "hello"));
        assert!(!glob_match("hello", "world"));
    }

    #[test]
    fn test_glob_match_star() {
        assert!(glob_match("*.txt", "file.txt"));
        assert!(!glob_match("*.txt", "file.rs"));
        assert!(glob_match("pre*suf", "pre-middle-suf"));
        assert!(glob_match("*", "anything"));
    }

    #[test]
    fn test_glob_match_question() {
        assert!(glob_match("h?llo", "hello"));
        assert!(glob_match("h?llo", "hallo"));
        assert!(!glob_match("h?llo", "hllo"));
    }

    #[test]
    fn test_glob_match_combined() {
        assert!(glob_match("*.service", "sshd.service"));
        assert!(glob_match("my-cred-?", "my-cred-a"));
        assert!(!glob_match("my-cred-?", "my-cred-ab"));
    }
}
