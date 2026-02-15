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

            let flags = libc::O_RDWR | libc::O_NOCTTY;

            let tty_fd = unsafe { libc::open(tty_path_cstr.as_ptr(), flags) };
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
            let force_arg: libc::c_int = if config.stdin_option == StandardInput::TtyForce {
                1
            } else {
                0
            };
            unsafe {
                let ret = libc::ioctl(tty_fd, libc::TIOCSCTTY, force_arg);
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
    println!("Exec helper trying to read config from stdin");
    let config: ExecHelperConfig = serde_json::from_reader(std::io::stdin()).unwrap();
    println!("Apply config: {config:?}");

    nix::unistd::close(libc::STDIN_FILENO).expect("I want to be able to close this fd!");

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

    // create state directories under /var/lib/ and set STATE_DIRECTORY env var
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

    eprintln!("EXECV: {:?} {:?}", &cmd, &args);

    nix::unistd::execv(&cmd, &args).unwrap();
}
