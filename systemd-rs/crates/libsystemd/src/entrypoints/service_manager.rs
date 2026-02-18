use log::{error, trace, warn};
use signal_hook::iterator::Signals;
use std::sync::{Arc, Mutex, RwLock};

use crate::config;
use crate::control;
use crate::generators;
use crate::lock_ext::RwLockExt;
use crate::logging;
use crate::notification_handler;
use crate::platform;
use crate::runtime_info;
use crate::signal_handler;
use crate::socket_activation;
use crate::units;

pub fn run_service_manager() {
    pid1_specific_setup();

    let cli_args = CliArgs::try_parse().unwrap_or_else(|e| {
        unrecoverable_error(e.to_string());
        unreachable!();
    });

    let (log_conf, mut conf) = config::load_config();

    logging::setup_logging(&log_conf).unwrap();

    // Augment PATH with binary directories derived from the unit search
    // paths.  Many upstream systemd unit files use bare command names in
    // ExecStart= (e.g. `systemd-tmpfiles`, `udevadm`).  Real systemd
    // resolves these via compiled-in prefix paths; systemd-rs adds the
    // relevant package directories to PATH instead.
    config::augment_path_from_unit_dirs(&conf.unit_dirs);

    // Run external generators before loading units.
    //
    // Generators are small executables (e.g. systemd-gpt-auto-generator,
    // zram-generator) that dynamically create unit files at boot time.
    // They are called with three output directory arguments and can write
    // unit files, symlinks, and .wants/.requires directories.
    //
    // Built-in generators (fstab, getty) are skipped since systemd-rs has
    // native implementations.  The output directories are then inserted
    // into the unit search path at the correct priority positions.
    let generator_output = generators::run_generators(&conf.unit_dirs);
    generators::augment_unit_dirs_with_generators(&mut conf.unit_dirs, &generator_output);

    #[cfg(feature = "cgroups")]
    {
        platform::cgroups::move_to_own_cgroup(&std::path::PathBuf::from("/sys/fs/cgroup")).unwrap();
    }

    // TODO make configurable
    let should_go_to_new_session = false;
    if should_go_to_new_session && !move_to_new_session() {
        return;
    }

    crate::platform::become_subreaper(true);

    let run_info = prepare_runtimeinfo(&conf, cli_args.dry_run);

    let signals = match Signals::new([
        signal_hook::consts::SIGCHLD,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGQUIT,
    ]) {
        Ok(signals) => signals,
        Err(e) => {
            unrecoverable_error(format!("Couldnt setup listening to the signals: {e}"));
            // unrecoverable_error always shutsdown systemd-rs
            unreachable!("");
        }
    };
    // listen to signals
    let handle = start_signal_handler_thread(signals, run_info.clone());

    // listen on user commands like listunits/kill/restart...
    control::open_all_sockets(run_info.clone(), &conf);

    start_notification_handler_thread(run_info.clone());
    start_stdout_handler_thread(run_info.clone());
    start_stderr_handler_thread(run_info.clone());

    socket_activation::start_socketactivation_thread(run_info.clone());

    trace!("Started all helper threads. Start activating units");

    let target_id: units::UnitId = {
        let run_info: &runtime_info::RuntimeInfo = &run_info.read_poisoned();
        use std::convert::TryInto;
        run_info.config.target_unit.as_str().try_into().unwrap()
    };

    // parallel startup of all services
    units::activate_needed_units(target_id, run_info);

    handle.join().unwrap();
}

fn find_shell_path() -> Option<std::path::PathBuf> {
    let possible_paths = vec![
        std::path::PathBuf::from("/bin/sh"),
        std::path::PathBuf::from("/sbin/sh"),
        std::path::PathBuf::from("/usr/bin/sh"),
    ];

    // TODO make configurable
    possible_paths.into_iter().find(|p| p.exists())
}

fn unrecoverable_error(error: String) {
    if nix::unistd::getpid().as_raw() == 1 {
        eprintln!("Unrecoverable error: {error}");
        if let Some(shell_path) = find_shell_path() {
            match std::process::Command::new(shell_path).spawn() {
                Ok(mut child) => match child.wait() {
                    Ok(_) => {
                        let dur = std::time::Duration::from_secs(10);
                        eprintln!("Returned from shell. Will exit after sleeping: {dur:?}");
                        std::thread::sleep(dur);
                        std::process::exit(1);
                    }
                    Err(e) => {
                        let dur = std::time::Duration::from_secs(1_000_000);
                        eprintln!(
                            "Error while waiting on the shell: {e}. Will sleep for {dur:?} and then exit"
                        );
                        std::thread::sleep(dur);
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    let dur = std::time::Duration::from_secs(1_000_000);
                    eprintln!(
                        "Error while starting the shell: {e}. Will sleep for {dur:?} and then exit"
                    );
                    std::thread::sleep(dur);
                    std::process::exit(1);
                }
            }
        } else {
            let dur = std::time::Duration::from_secs(10);
            eprintln!("Cannot find a shell for emergency. Will sleep for {dur:?} and then exit");
            std::thread::sleep(dur);
            std::process::exit(1);
        }
    } else {
        panic!("{}", error);
    }
}

fn move_to_new_session() -> bool {
    match unsafe { nix::unistd::fork() } {
        Ok(nix::unistd::ForkResult::Child) => {
            nix::unistd::setsid().unwrap();
            true
        }
        Ok(nix::unistd::ForkResult::Parent { .. }) => false,
        Err(e) => {
            error!("Fork before setsid failed: {e}");
            false
        }
    }
}

#[cfg(target_os = "linux")]
fn pid1_specific_setup() {
    if nix::unistd::getpid().as_raw() != 1 {
        return;
    }

    // When running as PID 1, the inherited stdin/stdout/stderr may be broken
    // pipes (e.g. the NixOS stage-2 init script redirects stdout through a
    // tee process that can die before exec'ing the service manager).  Reopen
    // the standard file descriptors matching what real systemd does at startup:
    //   - stdin  → /dev/null   (PID 1 never reads from the console)
    //   - stdout → /dev/console (write-only, O_NOCTTY)
    //   - stderr → /dev/console (write-only, O_NOCTTY)
    //
    // Opening /dev/console without O_NOCTTY would make it PID 1's controlling
    // terminal, which prevents getty from later acquiring it via TIOCSCTTY and
    // breaks terminal control for login shells.  Real systemd uses
    // make_null_stdio() + a separate write-only /dev/console fd for logging.

    // stdin → /dev/null
    let null_fd = unsafe {
        libc::open(
            b"/dev/null\0".as_ptr().cast(),
            libc::O_RDWR | libc::O_CLOEXEC,
        )
    };
    if null_fd >= 0 {
        let _ = unsafe { libc::dup2(null_fd, libc::STDIN_FILENO) };
        if null_fd > libc::STDERR_FILENO {
            unsafe { libc::close(null_fd) };
        }
    }

    // stdout/stderr → /dev/console (write-only, O_NOCTTY)
    let console_fd = unsafe {
        libc::open(
            b"/dev/console\0".as_ptr().cast(),
            libc::O_WRONLY | libc::O_NOCTTY | libc::O_CLOEXEC,
        )
    };
    if console_fd >= 0 {
        let _ = unsafe { libc::dup2(console_fd, libc::STDOUT_FILENO) };
        let _ = unsafe { libc::dup2(console_fd, libc::STDERR_FILENO) };
        if console_fd > libc::STDERR_FILENO {
            unsafe { libc::close(console_fd) };
        }
    }

    // Remount root filesystem read-write if needed.
    let flags = nix::mount::MsFlags::MS_REMOUNT;
    let source: Option<&str> = None;
    let fs_type: Option<&str> = None;
    let data: Option<&str> = None;
    let _ = nix::mount::mount(source, "/", fs_type, flags, data);

    // Mount essential tmpfs mount points that NixOS services depend on.
    //
    // Mount units are now implemented, but these early hardcoded mounts
    // serve as fallbacks that run before the unit loading pipeline.
    // Several NixOS services (notably suid-sgid-wrappers.service) have
    // RequiresMountsFor= dependencies on paths that need a tmpfs.  Without
    // these mounts the wrapper setup fails, which breaks PAM/NSS and
    // prevents login.
    //
    // Each entry is (where, options).  We only mount if the path doesn't
    // already have something mounted on it (checked via a simple stat of
    // the path — if the directory doesn't exist we create it first).
    let tmpfs_mounts: &[(&str, nix::mount::MsFlags, &str)] = &[
        (
            "/run/wrappers",
            nix::mount::MsFlags::MS_NODEV,
            "mode=755,size=50%",
        ),
        ("/run/initramfs", nix::mount::MsFlags::empty(), "mode=0700"),
    ];

    for &(where_path, flags, options) in tmpfs_mounts {
        // Create the mount point directory if it doesn't exist
        let _ = std::fs::create_dir_all(where_path);

        // Skip if already mounted (a very rough check: try to mount and
        // ignore EBUSY which means something is already there).
        let mount_result = nix::mount::mount(
            Some("tmpfs"),
            where_path,
            Some("tmpfs"),
            flags,
            Some(options),
        );
        match mount_result {
            Ok(()) => {
                eprintln!("systemd-rs: mounted tmpfs on {where_path}");
            }
            Err(nix::Error::EBUSY) => {
                // Already mounted — fine
            }
            Err(e) => {
                eprintln!("systemd-rs: failed to mount tmpfs on {where_path}: {e}");
            }
        }
    }

    // Mount cgroup2 filesystem at /sys/fs/cgroup.
    //
    // Real systemd mounts this very early during PID 1 initialization.
    // Many services (notably systemd-journald) need cgroups to work —
    // without this mount, journald fails with "Failed to acquire cgroup
    // root path: No medium found" and exits before sending READY=1.
    //
    // We mount cgroup2 (the unified hierarchy) which is what modern
    // systemd expects.  The mount is done with nsdelegate and memory_recursiveprot
    // options matching what real systemd uses.
    let _ = std::fs::create_dir_all("/sys/fs/cgroup");
    match nix::mount::mount(
        Some("cgroup2"),
        "/sys/fs/cgroup",
        Some("cgroup2"),
        nix::mount::MsFlags::MS_NOSUID
            | nix::mount::MsFlags::MS_NODEV
            | nix::mount::MsFlags::MS_NOEXEC,
        Some("nsdelegate,memory_recursiveprot"),
    ) {
        Ok(()) => {
            eprintln!("systemd-rs: mounted cgroup2 on /sys/fs/cgroup");
        }
        Err(nix::Error::EBUSY) => {
            // Already mounted — fine
        }
        Err(e) => {
            eprintln!("systemd-rs: failed to mount cgroup2 on /sys/fs/cgroup: {e}");
        }
    }

    // Ensure /etc/machine-id exists.
    //
    // systemd-journald uses the machine-id to name the journal directory
    // under /var/log/journal/<machine-id>/.  If /etc/machine-id is missing
    // or empty, journald cannot create persistent storage and may fail.
    // Real systemd generates this file very early (via systemd-machine-id-setup
    // or first-boot logic).  We generate a random one if it doesn't exist.
    let machine_id_path = std::path::Path::new("/etc/machine-id");
    if !machine_id_path.exists()
        || std::fs::metadata(machine_id_path).map_or(true, |m| m.len() == 0)
    {
        // Generate a random 128-bit ID formatted as 32 hex chars + newline
        let mut buf = [0u8; 16];
        if let Ok(f) = std::fs::File::open("/dev/urandom") {
            use std::io::Read;
            let mut f = f;
            if f.read_exact(&mut buf).is_ok() {
                let hex: String = buf.iter().map(|b| format!("{b:02x}")).collect();
                if std::fs::write(machine_id_path, format!("{hex}\n")).is_ok() {
                    eprintln!("systemd-rs: generated /etc/machine-id");
                }
            }
        }
    }

    // Set the system hostname from /etc/hostname.
    //
    // Real systemd reads /etc/hostname very early during PID 1
    // initialization and calls sethostname(2).  Without this, the
    // kernel hostname remains empty (shown as "(none)" in prompts
    // and login banners).
    let hostname_path = std::path::Path::new("/etc/hostname");
    if hostname_path.exists() {
        if let Ok(raw) = std::fs::read_to_string(hostname_path) {
            let hostname = raw.trim();
            if !hostname.is_empty() {
                match nix::unistd::sethostname(hostname) {
                    Ok(()) => {
                        eprintln!("systemd-rs: set hostname to '{hostname}'");
                    }
                    Err(e) => {
                        eprintln!("systemd-rs: failed to set hostname: {e}");
                    }
                }
            }
        }
    }

    // Ensure home directories exist for all users in /etc/passwd.
    //
    // On NixOS the activation script runs update-users-groups.pl
    // BEFORE exec'ing into systemd-rs, so /etc/passwd is already
    // populated with all declared users by this point.  However the
    // Perl script may fail to create the home directory (e.g. missing
    // /var/lib/nixos state dir on first boot).  We create any missing
    // home directories here — before services (including getty) start —
    // to avoid the "No directory, logging in with HOME=/" message.
    ensure_home_directories();

    // Ensure /var/log/journal exists so that systemd-journald can use
    // persistent storage and `journalctl --flush` succeeds.  Normally
    // systemd-tmpfiles-setup creates this, but it may run after (or
    // concurrently with) systemd-journal-flush.service.  This early
    // creation serves as a fallback before mount units are activated.
    let _ = std::fs::create_dir_all("/var/log/journal");

    // ── PAM / NSS prerequisite diagnostics ──────────────────────────────
    //
    // Log the state of critical files that PAM and NSS need.  If any of
    // these are missing at this point, authentication will fail later with
    // "Authentication service cannot retrieve authentication info".
    //
    // /etc/shadow  – password hashes; must exist and be readable by root
    // /etc/nsswitch.conf – NSS module configuration
    // /run/wrappers/bin/unix_chkpwd – suid helper used by pam_unix
    //   (created later by suid-sgid-wrappers.service, so it is expected
    //    to be absent here; logged for post-mortem debugging)

    let shadow = std::path::Path::new("/etc/shadow");
    if shadow.exists() {
        eprintln!("systemd-rs: /etc/shadow exists (ok)");
    } else {
        eprintln!("systemd-rs: WARNING: /etc/shadow does not exist — PAM authentication will fail");
    }

    let nsswitch = std::path::Path::new("/etc/nsswitch.conf");
    if nsswitch.exists() {
        eprintln!("systemd-rs: /etc/nsswitch.conf exists (ok)");
    } else {
        eprintln!("systemd-rs: WARNING: /etc/nsswitch.conf does not exist — NSS lookups may fail");
    }

    let chkpwd = std::path::Path::new("/run/wrappers/bin/unix_chkpwd");
    if chkpwd.exists() {
        eprintln!("systemd-rs: /run/wrappers/bin/unix_chkpwd exists (ok)");
    } else {
        eprintln!(
            "systemd-rs: /run/wrappers/bin/unix_chkpwd not yet present \
             (expected — suid-sgid-wrappers.service will create it)"
        );
    }

    // Check that /run/wrappers is mounted without nosuid — the suid bit on
    // unix_chkpwd only works if the filesystem allows it.
    if let Ok(mounts) = std::fs::read_to_string("/proc/mounts") {
        for line in mounts.lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() >= 4 && fields[1] == "/run/wrappers" {
                let opts = fields[3];
                if opts.split(',').any(|o| o == "nosuid") {
                    eprintln!(
                        "systemd-rs: WARNING: /run/wrappers is mounted with nosuid — \
                         suid wrappers will not work! (options: {opts})"
                    );
                } else {
                    eprintln!(
                        "systemd-rs: /run/wrappers mounted without nosuid (ok, options: {opts})"
                    );
                }
                break;
            }
        }
    }
}

/// Read /etc/passwd and create any missing home directories with the
/// correct ownership and mode.  Skips trivial homes like "/" and
/// "/var/empty".
fn ensure_home_directories() {
    let passwd_path = std::path::Path::new("/etc/passwd");
    if !passwd_path.exists() {
        eprintln!("systemd-rs: /etc/passwd does not exist, skipping home directory creation");
        return;
    }
    let passwd_contents = match std::fs::read_to_string(passwd_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("systemd-rs: failed to read /etc/passwd: {e}");
            return;
        }
    };

    let line_count = passwd_contents.lines().count();
    eprintln!("systemd-rs: ensuring home directories ({line_count} passwd entries)");

    for line in passwd_contents.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        // passwd format: name:x:uid:gid:gecos:home:shell
        if fields.len() < 7 {
            continue;
        }
        let user = fields[0];
        let home = fields[5];
        // Skip non-directory or special homes
        if home.is_empty()
            || home == "/"
            || home == "/nonexistent"
            || home == "/dev/null"
            || home == "/var/empty"
            || home == "/run/systemd"
        {
            continue;
        }
        let home_path = std::path::Path::new(home);
        if home_path.exists() {
            continue;
        }
        let uid: u32 = match fields[2].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let gid: u32 = match fields[3].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        match std::fs::create_dir_all(home_path) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("systemd-rs: failed to create home {home} for {user}: {e}");
                continue;
            }
        }
        // Set ownership to the user/group from passwd
        let c_path = match std::ffi::CString::new(home) {
            Ok(p) => p,
            Err(_) => continue,
        };
        unsafe { libc::chown(c_path.as_ptr(), uid, gid) };
        unsafe { libc::chmod(c_path.as_ptr(), 0o700) };
        eprintln!("systemd-rs: created home directory {home} for {user} (uid={uid}, gid={gid})");
    }
}

#[cfg(not(target_os = "linux"))]
fn pid1_specific_setup() {}

fn prepare_runtimeinfo(conf: &config::Config, dry_run: bool) -> runtime_info::ArcMutRuntimeInfo {
    // initial loading of the units and matching of the various before/after settings
    // also opening all fildescriptors in the socket files
    let mut unit_table =
        units::load_all_units(&conf.unit_dirs, &conf.target_unit).expect("loading unit files");
    trace!("Finished loading units");

    // Break dependency cycles instead of aborting, matching systemd behavior.
    // systemd warns about cycles and removes ordering edges to break them.
    let broken_cycles = units::break_dependency_cycles(&mut unit_table);
    if !broken_cycles.is_empty() {
        warn!(
            "Broke {} dependency cycle(s). See warnings above for details.",
            broken_cycles.len()
        );
    }
    trace!("Unit dependencies passed sanity checks");
    let unit_table = unit_table;

    if dry_run {
        warn!("Exit after loading because --dry-run was passed");
        unrecoverable_error("Started as dry-run".into());
    }

    let pid_table = Mutex::new(std::collections::HashMap::new());

    Arc::new(RwLock::new(runtime_info::RuntimeInfo {
        unit_table,
        pid_table,
        fd_store: std::sync::RwLock::new(crate::fd_store::FDStore::default()),
        config: conf.clone(),
        stdout_eventfd: platform::make_event_fd().unwrap(),
        stderr_eventfd: platform::make_event_fd().unwrap(),
        notification_eventfd: platform::make_event_fd().unwrap(),
        socket_activation_eventfd: platform::make_event_fd().unwrap(),
    }))
}

fn start_notification_handler_thread(run_info: runtime_info::ArcMutRuntimeInfo) {
    std::thread::spawn(move || {
        notification_handler::handle_all_streams(run_info);
    });
}
fn start_stdout_handler_thread(run_info: runtime_info::ArcMutRuntimeInfo) {
    std::thread::spawn(move || {
        notification_handler::handle_all_std_out(run_info);
    });
}
fn start_stderr_handler_thread(run_info: runtime_info::ArcMutRuntimeInfo) {
    std::thread::spawn(move || {
        notification_handler::handle_all_std_err(run_info);
    });
}
fn start_signal_handler_thread(
    signals: Signals,
    run_info: runtime_info::ArcMutRuntimeInfo,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        // listen on signals from the child processes
        signal_handler::handle_signals(signals, run_info);
    })
}

use clap::Parser;

#[derive(Parser, Debug)]
struct CliArgs {
    #[arg(short, long)]
    dry_run: bool,
}
