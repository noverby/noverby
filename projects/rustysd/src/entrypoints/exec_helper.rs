use std::path::{Path, PathBuf};

use crate::units::{
    PlatformSpecificServiceFields, RLimitValue, ResourceLimit, StandardInput, UtmpMode,
};

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
    eprintln!(
        "[EXEC_HELPER {}] Starting: {}{}",
        config.name,
        config.cmd.display(),
        if config.args.is_empty() {
            String::new()
        } else {
            format!(" {}", config.args.join(" "))
        }
    );

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
    // Matches systemd's ImportCredential= behaviour: glob patterns are matched
    // against files in the system credential stores and copied into
    // /run/credentials/<unit-name>/.
    if !config.import_credentials.is_empty() {
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
        std::env::set_var("STATE_DIRECTORY", full_paths.join(":"));
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
        std::env::set_var("RUNTIME_DIRECTORY", full_paths.join(":"));
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

    if nix::unistd::getuid().is_root() {
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
        std::env::set_var(k, v);
    }

    std::env::set_var("LISTEN_PID", format!("{}", nix::unistd::getpid()));

    eprintln!(
        "[EXEC_HELPER {}] exec: {} {}",
        config.name,
        config.cmd.display(),
        config.args.join(" ")
    );

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

/// Set up the per-service credential directory and import matching credentials.
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

    let mut imported = 0usize;

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

                // Don't overwrite — first match wins (higher-priority store).
                if dst.exists() {
                    continue;
                }

                match std::fs::copy(&src, &dst) {
                    Ok(_) => {
                        // Make the credential file readable only by the service user (0o400).
                        let _ = unsafe {
                            libc::chmod(
                                std::ffi::CString::new(dst.to_string_lossy().as_bytes())
                                    .unwrap()
                                    .as_ptr(),
                                0o400,
                            )
                        };
                        let _ = nix::unistd::chown(&dst, Some(uid), Some(gid));
                        imported += 1;
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

    if imported > 0 || !config.import_credentials.is_empty() {
        // Always set the env var so the service knows where to look,
        // even if no credentials were found (matches systemd behaviour).
        std::env::set_var("CREDENTIALS_DIRECTORY", &cred_dir);
        eprintln!(
            "[EXEC_HELPER {}] Imported {} credential(s) into {:?}",
            config.name, imported, cred_dir
        );
    }
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

extern "C" {
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

    eprintln!(
        "[EXEC_HELPER {}] Wrote utmp record (id={:?}, line={}, mode={:?})",
        config.name,
        config.utmp_identifier,
        tty_line(config),
        config.utmp_mode,
    );
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
