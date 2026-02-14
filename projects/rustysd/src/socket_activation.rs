//! Wait for sockets to activate their respective services
use log::error;
use log::trace;

use crate::runtime_info::ArcMutRuntimeInfo;
use crate::units::{ActivationSource, Specific, StatusStarted, UnitId, UnitStatus};
use std::os::unix::io::BorrowedFd;

/// Helper to create a BorrowedFd from a raw fd.
///
/// # Safety
/// The caller must ensure the fd is valid and will outlive the returned BorrowedFd.
unsafe fn borrow_fd(fd: i32) -> BorrowedFd<'static> {
    BorrowedFd::borrow_raw(fd)
}

pub fn start_socketactivation_thread(run_info: ArcMutRuntimeInfo) {
    std::thread::spawn(move || loop {
        let wait_result = wait_for_socket(run_info.clone());
        match wait_result {
            Ok(ids) => {
                let run_info = run_info.read().unwrap();
                let unit_table = &run_info.unit_table;
                for socket_id in ids {
                    {
                        // search the service this socket belongs to.
                        // Note that this differs from systemd behaviour where one socket may belong to multiple services
                        let mut srvc_unit = None;
                        for unit in unit_table.values() {
                            if let crate::units::Specific::Service(specific) = &unit.specific {
                                if specific.has_socket(&socket_id.name) {
                                    srvc_unit = Some(unit);
                                    trace!("Start service {} by socket activation", unit.id.name);
                                    break;
                                }
                            }
                        }

                        // mark socket as activated, removing it from the set of
                        // fds rustysd is actively listening on
                        let sock_unit = unit_table.get(&socket_id).unwrap();
                        if let Specific::Socket(specific) = &sock_unit.specific {
                            let mut_state = &mut *specific.state.write().unwrap();
                            mut_state.sock.activated = true;
                        }
                        if srvc_unit.is_none() {
                            error!(
                                "Socket unit {socket_id:?} activated, but the service could not be found"
                            );
                        }
                        if let Some(srvc_unit) = srvc_unit {
                            let srvc_status = {
                                let status_locked = &*srvc_unit.common.status.read().unwrap();
                                status_locked.clone()
                            };

                            if srvc_status == UnitStatus::Started(StatusStarted::WaitingForSocket) {
                                // the service unit gets activated
                                match crate::units::activate_unit(
                                    srvc_unit.id.clone(),
                                    &run_info,
                                    ActivationSource::SocketActivation,
                                ) {
                                    Ok(_) => {
                                        trace!(
                                            "New status after socket activation: {:?}",
                                            *unit_table
                                                .get(&srvc_unit.id)
                                                .unwrap()
                                                .common
                                                .status
                                                .read()
                                                .unwrap()
                                        );
                                    }
                                    Err(e) => {
                                        error!(
                                                "Error while starting service from socket activation: {e}"
                                            );
                                    }
                                }
                            } else {
                                // This should not happen too often because the sockets of a service
                                // should only be listened on if the service is currently waiting on socket activation
                                trace!(
                                    "Ignore socket activation. Service has status: {srvc_status:?}"
                                );
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error in socket activation loop: {e}");
                break;
            }
        }
    });
}

pub fn wait_for_socket(run_info: ArcMutRuntimeInfo) -> Result<Vec<UnitId>, String> {
    let eventfd = { run_info.read().unwrap().socket_activation_eventfd };
    let (mut fdset, fd_to_sock_id) = {
        let run_info_locked = &*run_info.read().unwrap();

        let fd_to_sock_id = run_info_locked.fd_store.read().unwrap().global_fds_to_ids();
        let mut fdset = nix::sys::select::FdSet::new();
        {
            let unit_table_locked = &run_info_locked.unit_table;
            for (fd, id) in &fd_to_sock_id {
                let unit = unit_table_locked.get(id).unwrap();
                if let Specific::Socket(specific) = &unit.specific {
                    let mut_state = &*specific.state.read().unwrap();
                    if !mut_state.sock.activated {
                        fdset.insert(unsafe { borrow_fd(*fd) });
                    }
                }
            }
            fdset.insert(unsafe { borrow_fd(eventfd.read_end()) });
        }
        (fdset, fd_to_sock_id)
    };

    let result = nix::sys::select::select(None, Some(&mut fdset), None, None, None);
    match result {
        Ok(_) => {
            let mut activated_ids = Vec::new();
            if fdset.contains(unsafe { borrow_fd(eventfd.read_end()) }) {
                trace!("Interrupted socketactivation select because the eventfd fired");
                crate::platform::reset_event_fd(eventfd);
                trace!("Reset eventfd value");
            } else {
                for (fd, id) in &fd_to_sock_id {
                    if fdset.contains(unsafe { borrow_fd(*fd) }) {
                        activated_ids.push(id.clone());
                    }
                }
            }
            Ok(activated_ids)
        }
        Err(e) => {
            if e == nix::Error::EINTR {
                Ok(Vec::new())
            } else {
                Err(format!("Error while selecting: {e}"))
            }
        }
    }
}
