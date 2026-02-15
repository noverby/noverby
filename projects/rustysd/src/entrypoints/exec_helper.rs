use std::path::{Path, PathBuf};

use crate::units::{PlatformSpecificServiceFields, RLimitValue, ResourceLimit, StandardInput};

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

    /// Whether StandardOutput is set to inherit (or journal/kmsg/unset).
    /// When true AND stdin is a TTY, stdout will be dup'd from the TTY fd.
    #[serde(default = "default_true")]
    pub stdout_is_inherit: bool,
    /// Whether StandardError is set to inherit (or journal/kmsg/unset).
    /// When true AND stdin is a TTY, stderr will be dup'd from the TTY fd.
    #[serde(default = "default_true")]
    pub stderr_is_inherit: bool,
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

    nix::unistd::execv(&cmd, &args).unwrap();
}
