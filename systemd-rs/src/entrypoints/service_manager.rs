use log::{error, trace, warn};
use signal_hook::iterator::Signals;
use std::sync::{Arc, Mutex, RwLock};

use crate::config;
use crate::control;
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

    let (log_conf, conf) = config::load_config();

    logging::setup_logging(&log_conf).unwrap();

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
        let run_info: &runtime_info::RuntimeInfo = &run_info.read().unwrap();
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
