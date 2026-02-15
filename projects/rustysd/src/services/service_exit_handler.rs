use log::{error, info, trace};

use crate::runtime_info::{ArcMutRuntimeInfo, PidEntry, RuntimeInfo};
use crate::signal_handler::ChildTermination;
use crate::units::{
    ServiceRestart, ServiceType, Specific, Timeout, UnitAction, UnitOperationErrorReason,
    UnitStatus,
};

/// Check whether a service has `RemainAfterExit=yes` configured.
fn has_remain_after_exit(unit: &crate::units::Unit) -> bool {
    if let Specific::Service(srvc) = &unit.specific {
        srvc.conf.remain_after_exit
    } else {
        false
    }
}

/// Determine whether a service should be restarted given its `Restart=`
/// policy and the way it terminated.
///
/// The logic mirrors systemd's restart table
/// (<https://www.freedesktop.org/software/systemd/man/systemd.service.html#Restart=>):
///
/// | Exit reason        | no | always | on-success | on-failure | on-abnormal | on-abort | on-watchdog |
/// |--------------------|----|--------|------------|------------|-------------|----------|-------------|
/// | Clean exit (0)     |    |   X    |     X      |            |             |          |             |
/// | Unclean exit (!=0) |    |   X    |            |     X      |             |          |             |
/// | Clean signal       |    |   X    |     X      |            |             |          |             |
/// | Unclean signal     |    |   X    |            |     X      |      X      |    X     |             |
/// | Timeout            |    |   X    |            |     X      |      X      |          |             |
/// | Watchdog           |    |   X    |            |     X      |      X      |          |      X      |
///
/// "Clean" signals are SIGHUP, SIGINT, SIGTERM, and SIGPIPE (matching systemd defaults).
fn should_restart(policy: &ServiceRestart, termination: &ChildTermination) -> bool {
    match policy {
        ServiceRestart::No => false,
        ServiceRestart::Always => true,
        ServiceRestart::OnSuccess => termination.success() || is_clean_signal(termination),
        ServiceRestart::OnFailure => {
            // Non-zero exit, unclean signal, or timeout (timeout is not
            // currently tracked separately – a killed-by-signal counts).
            match termination {
                ChildTermination::Exit(code) => *code != 0,
                ChildTermination::Signal(sig) => !is_clean_signal_value(*sig),
            }
        }
        ServiceRestart::OnAbnormal => {
            // Unclean signal or timeout – not on any exit code.
            match termination {
                ChildTermination::Exit(_) => false,
                ChildTermination::Signal(sig) => !is_clean_signal_value(*sig),
            }
        }
        ServiceRestart::OnAbort => {
            // Unclean signal only.
            match termination {
                ChildTermination::Exit(_) => false,
                ChildTermination::Signal(sig) => !is_clean_signal_value(*sig),
            }
        }
        ServiceRestart::OnWatchdog => {
            // Watchdog timeout only – rustysd does not implement watchdog yet,
            // so this never triggers a restart.
            false
        }
    }
}

/// Returns `true` if the termination was one of the "clean" signals that
/// systemd treats as a successful exit: SIGHUP, SIGINT, SIGTERM, SIGPIPE.
fn is_clean_signal(termination: &ChildTermination) -> bool {
    match termination {
        ChildTermination::Signal(sig) => is_clean_signal_value(*sig),
        ChildTermination::Exit(_) => false,
    }
}

fn is_clean_signal_value(sig: nix::sys::signal::Signal) -> bool {
    use nix::sys::signal::Signal;
    matches!(
        sig,
        Signal::SIGHUP | Signal::SIGINT | Signal::SIGTERM | Signal::SIGPIPE
    )
}

pub fn service_exit_handler_new_thread(
    pid: nix::unistd::Pid,
    code: ChildTermination,
    run_info: ArcMutRuntimeInfo,
) {
    std::thread::spawn(move || {
        if let Err(e) = service_exit_handler(pid, code, &run_info.read().unwrap()) {
            error!("{e}");
        }
    });
}

pub fn service_exit_handler(
    pid: nix::unistd::Pid,
    code: ChildTermination,
    run_info: &RuntimeInfo,
) -> Result<(), String> {
    trace!("Exit handler with pid: {pid}");

    // Handle exiting of helper processes and oneshot processes
    {
        let pid_table_locked = &mut *run_info.pid_table.lock().unwrap();
        let entry = pid_table_locked.get(&pid);
        if let Some(entry) = entry {
            match entry {
                PidEntry::Service(_id, _srvctype) => {
                    // ignore at this point, will be handled below
                }
                PidEntry::Helper(_id, srvc_name) => {
                    trace!("Helper process for service: {srvc_name} exited with: {code:?}");
                    // this will be collected by the thread that waits for the helper process to exit
                    pid_table_locked.insert(pid, PidEntry::HelperExited(code));
                    return Ok(());
                }
                PidEntry::HelperExited(_) | PidEntry::ServiceExited(_) => {
                    // TODO is this sensible? How do we handle this?
                    error!("Pid exited that was already saved as exited");
                    return Ok(());
                }
            }
        } else {
            trace!(
                "All processes spawned by rustysd have a pid entry. This did not: {pid}. Probably a rerooted orphan that got killed."
            );
            return Ok(());
        }
    }

    // find out which service exited and if it was a oneshot service save an entry in the pid table that marks the service as exited
    let srvc_id = {
        let pid_table_locked = &mut *run_info.pid_table.lock().unwrap();
        let entry = pid_table_locked.remove(&pid);
        match entry {
            Some(entry) => match entry {
                PidEntry::Service(id, _srvctype) => {
                    trace!("Save service as exited. PID: {pid}");
                    pid_table_locked.insert(pid, PidEntry::ServiceExited(code));
                    id
                }
                PidEntry::Helper(..) | PidEntry::HelperExited(_) | PidEntry::ServiceExited(_) => {
                    unreachable!();
                }
            },
            None => {
                unreachable!();
            }
        }
    };

    let Some(unit) = run_info.unit_table.get(&srvc_id) else {
        panic!("Tried to run a unit that has been removed from the map");
    };

    // kill oneshot service processes. There should be none but just in case...
    {
        if let Specific::Service(srvc) = &unit.specific {
            if srvc.conf.srcv_type == ServiceType::OneShot {
                let mut_state = &mut *srvc.state.write().unwrap();
                mut_state
                    .srvc
                    .kill_all_remaining_processes(&srvc.conf, &unit.id.name);

                // RemainAfterExit=yes: keep the unit in Started status after a
                // clean exit, matching systemd's behaviour for oneshot services
                // that perform setup tasks.
                if srvc.conf.remain_after_exit && code.success() {
                    trace!(
                        "Oneshot service {} exited cleanly with RemainAfterExit=yes, staying active",
                        unit.id.name
                    );
                }
                return Ok(());
            }
        }
    }

    // Determine SuccessAction / FailureAction for this unit and whether
    // the exit counts as success or failure.
    let success_action = &unit.common.unit.success_action;
    let failure_action = &unit.common.unit.failure_action;

    if code.success() {
        if *success_action != UnitAction::None {
            info!(
                "Service {} exited successfully, triggering SuccessAction={:?}",
                unit.id.name, success_action
            );
            crate::units::execute_unit_action(success_action, &unit.id.name);
        }
    } else if *failure_action != UnitAction::None {
        info!(
            "Service {} failed ({:?}), triggering FailureAction={:?}",
            unit.id.name, code, failure_action
        );
        crate::units::execute_unit_action(failure_action, &unit.id.name);
    }

    trace!("Check if we want to restart the unit");
    let name = &unit.id.name;
    let (restart_unit, restart_sec) = {
        if let Specific::Service(srvc) = &unit.specific {
            trace!(
                "Service with id: {:?}, name: {} pid: {} exited with: {:?}",
                srvc_id,
                unit.id.name,
                pid,
                code
            );

            (
                should_restart(&srvc.conf.restart, &code),
                srvc.conf.restart_sec.clone(),
            )
        } else {
            (false, None)
        }
    };

    // check that the status is "Started". If thats not the case this service got killed by something else (control interface for example) so dont interfere
    {
        let status_locked = &*unit.common.status.read().unwrap();
        if !(status_locked.is_started() || *status_locked == UnitStatus::Starting) {
            trace!("Exit handler ignores exit of service {}. Its status is not 'Started'/'Starting', it is: {:?}", name, *status_locked);
            return Ok(());
        }
    }

    // RemainAfterExit=yes: if the process exited cleanly, keep the service
    // in its current active state — do not restart, do not deactivate.
    if has_remain_after_exit(unit) && code.success() {
        trace!("Service {name} exited cleanly with RemainAfterExit=yes, staying active");
        return Ok(());
    }

    if restart_unit {
        if let Some(ref timeout) = restart_sec {
            match timeout {
                Timeout::Duration(dur) => {
                    trace!(
                        "Waiting {:?} (RestartSec) before restarting service {name}",
                        dur
                    );
                    std::thread::sleep(*dur);
                }
                Timeout::Infinity => {
                    trace!("RestartSec=infinity for service {name}, not restarting");
                    return Ok(());
                }
            }
        }
        trace!("Restart service {name} after it died");
        crate::units::reactivate_unit(srvc_id, run_info).map_err(|e| format!("{e}"))?;
    } else {
        trace!("Recursively killing all services requiring service {name}");
        loop {
            let res = crate::units::deactivate_unit_recursive(&srvc_id, run_info);
            let retry = if let Err(e) = &res {
                if let UnitOperationErrorReason::DependencyError(_) = e.reason {
                    // Only retry if this is the case. This only occurs if, while the units are being deactivated,
                    // another unit got activated that would not be able to run with this unit deactivated.
                    // This should generally be pretty rare but it should be handled properly.
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if !retry {
                res.map_err(|e| format!("{e}"))?;
            }
        }
    }
    Ok(())
}
