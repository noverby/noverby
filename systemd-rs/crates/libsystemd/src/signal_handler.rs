//! Handle signals sent to this process from either the outside or the child processes
//!
//! ## Deadlock prevention
//!
//! The signal handler receives a cloned `ArcMutPidTable` so it can update PID
//! table entries (e.g. `Service` → `ServiceExited`) **without** acquiring the
//! `RuntimeInfo` read lock.  This breaks a 3-way deadlock that otherwise occurs
//! when:
//!
//!   1. Activation threads hold read locks on `RuntimeInfo` while polling
//!      `wait_for_service` (checking the PID table for `ServiceExited`).
//!   2. A `systemctl` command (e.g. from a udev `RUN+=` rule) tries to acquire
//!      a write lock on `RuntimeInfo` — it blocks because readers hold locks,
//!      and on glibc's writer-preferring `pthread_rwlock` all *new* readers are
//!      also blocked.
//!   3. The old exit-handler thread needed a read lock to update the PID table
//!      — but was blocked by the pending writer from (2).
//!
//! Now the signal handler updates the PID table directly (step 3 no longer
//! needs the lock), so `wait_for_service` sees `ServiceExited` promptly,
//! releases its read lock, and the writer from (2) can proceed.

use crate::lock_ext::MutexExt;
use crate::runtime_info::{ArcMutPidTable, ArcMutRuntimeInfo, PidEntry};
use crate::services;
use log::error;
use log::info;
use log::trace;
use signal_hook::iterator::Signals;

pub fn handle_signals(
    mut signals: Signals,
    run_info: ArcMutRuntimeInfo,
    pid_table: ArcMutPidTable,
) {
    loop {
        // Pick up new signals
        for signal in signals.forever() {
            match signal as libc::c_int {
                signal_hook::consts::SIGCHLD => {
                    std::iter::from_fn(get_next_exited_child)
                        .take_while(Result::is_ok)
                        .for_each(|val| {
                            match val {
                                Ok((pid, code)) => {
                                    // Phase 1: Update the PID table immediately,
                                    // WITHOUT acquiring the RuntimeInfo read lock.
                                    // This lets `wait_for_service` (which polls the
                                    // PID table under a RuntimeInfo read lock) see
                                    // the `ServiceExited` entry and proceed.
                                    let unit_id = {
                                        let mut pt = pid_table.lock_poisoned();
                                        match pt.get(&pid) {
                                            Some(PidEntry::Helper(_id, srvc_name)) => {
                                                trace!(
                                                    "Helper process for service: {srvc_name} exited with: {code:?}"
                                                );
                                                pt.insert(pid, PidEntry::HelperExited(code));
                                                None // no further handling needed
                                            }
                                            Some(PidEntry::Service(_id, _srvctype)) => {
                                                // Remove the Service entry and replace
                                                // it with ServiceExited so that
                                                // wait_for_service can observe it.
                                                let entry = pt.remove(&pid);
                                                let id = match entry {
                                                    Some(PidEntry::Service(id, _)) => id,
                                                    _ => unreachable!(),
                                                };
                                                trace!("Save service as exited. PID: {pid}");
                                                pt.insert(pid, PidEntry::ServiceExited(code));
                                                Some(id)
                                            }
                                            Some(
                                                PidEntry::HelperExited(_)
                                                | PidEntry::ServiceExited(_),
                                            ) => {
                                                error!(
                                                    "Pid {pid} exited but was already saved as exited"
                                                );
                                                None
                                            }
                                            None => {
                                                trace!(
                                                    "All processes spawned by systemd-rs have a pid entry. \
                                                     This did not: {pid}. Probably a rerooted orphan."
                                                );
                                                None
                                            }
                                        }
                                    };

                                    // Phase 2: If the exited process was a service,
                                    // spawn a thread to handle restart/cleanup logic.
                                    // That thread *will* need the RuntimeInfo read
                                    // lock, but by now the critical PID-table update
                                    // is already visible.
                                    if let Some(id) = unit_id {
                                        let run_info_clone = run_info.clone();
                                        services::service_exit_handler_new_thread(
                                            pid, id, code, run_info_clone,
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!("{e}");
                                }
                            }
                        });
                }
                signal_hook::consts::SIGTERM
                | signal_hook::consts::SIGINT
                | signal_hook::consts::SIGQUIT => {
                    info!("Received termination signal. systemd-rs checking out");
                    crate::shutdown::shutdown_sequence(run_info.clone());
                }

                _ => unreachable!(),
            }
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum ChildTermination {
    Signal(nix::sys::signal::Signal),
    Exit(i32),
}

impl std::fmt::Display for ChildTermination {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Signal(sig) => write!(f, "signal {sig}"),
            Self::Exit(code) => write!(f, "exit code {code}"),
        }
    }
}

impl ChildTermination {
    #[must_use]
    pub const fn success(&self) -> bool {
        match self {
            Self::Signal(_) => false,
            Self::Exit(code) => *code == 0,
        }
    }
}

type ChildIterElem = Result<(nix::unistd::Pid, ChildTermination), nix::Error>;

fn get_next_exited_child() -> Option<ChildIterElem> {
    let wait_any_pid = nix::unistd::Pid::from_raw(-1);
    let wait_flags = nix::sys::wait::WaitPidFlag::WNOHANG;
    match nix::sys::wait::waitpid(wait_any_pid, Some(wait_flags)) {
        Ok(exit_status) => match exit_status {
            nix::sys::wait::WaitStatus::Exited(pid, code) => {
                Some(Ok((pid, ChildTermination::Exit(code))))
            }
            nix::sys::wait::WaitStatus::Signaled(pid, signal, _dumped_core) => {
                // signals get handed to the parent if the child got killed by it but didnt handle the
                // signal itself
                // we dont care if the service dumped it's core
                Some(Ok((pid, ChildTermination::Signal(signal))))
            }
            nix::sys::wait::WaitStatus::StillAlive => {
                trace!("No more state changes to poll");
                None
            }
            _ => {
                trace!("Ignored child signal received with code: {exit_status:?}");
                // return next child, we dont care about other events like stop/continue of children
                get_next_exited_child()
            }
        },
        Err(e) => {
            if e == nix::Error::ECHILD {
            } else {
                trace!("Error while waiting: {e}");
            }
            Some(Err(e))
        }
    }
}
