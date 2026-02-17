//! collect the different streams from the services
//! Stdout and stderr get redirected to the normal stdout/err but are prefixed with a unique string to identify their output
//! streams from the notification sockets get parsed and applied to the respective service

use log::trace;
use log::warn;

use crate::platform::reset_event_fd;
use crate::runtime_info::ArcMutRuntimeInfo;
use crate::services::Service;
use crate::services::StdIo;
use crate::units::{Specific, UnitId};
use std::os::unix::io::BorrowedFd;
use std::{collections::HashMap, os::unix::io::AsRawFd};

/// Helper to create a BorrowedFd from a raw fd.
///
/// # Safety
/// The caller must ensure the fd is valid and will outlive the returned BorrowedFd.
unsafe fn borrow_fd(fd: i32) -> BorrowedFd<'static> {
    unsafe { BorrowedFd::borrow_raw(fd) }
}

fn collect_from_srvc<F>(run_info: ArcMutRuntimeInfo, f: F) -> HashMap<i32, UnitId>
where
    F: Fn(&mut HashMap<i32, UnitId>, &Service, UnitId),
{
    let run_info_locked = run_info.read().unwrap();
    let unit_table = &run_info_locked.unit_table;
    unit_table
        .iter()
        .fold(HashMap::new(), |mut map, (id, srvc_unit)| {
            if let Specific::Service(srvc) = &srvc_unit.specific {
                let state = &*srvc.state.read().unwrap();
                f(&mut map, &state.srvc, id.clone());
            }
            map
        })
}

pub fn handle_all_streams(run_info: ArcMutRuntimeInfo) {
    let eventfd = { run_info.read().unwrap().notification_eventfd };
    loop {
        let fd_to_srvc_id = collect_from_srvc(run_info.clone(), |map, srvc, id| {
            if let Some(socket) = &srvc.notifications {
                map.insert(socket.as_raw_fd(), id);
            }
        });

        let mut fdset = nix::sys::select::FdSet::new();
        for fd in fd_to_srvc_id.keys() {
            fdset.insert(unsafe { borrow_fd(*fd) });
        }
        fdset.insert(unsafe { borrow_fd(eventfd.read_end()) });

        let result = nix::sys::select::select(None, Some(&mut fdset), None, None, None);

        let run_info_locked = run_info.read().unwrap();
        let unit_table = &run_info_locked.unit_table;
        match result {
            Ok(_) => {
                if fdset.contains(unsafe { borrow_fd(eventfd.read_end()) }) {
                    trace!("Interrupted notification select because the eventfd fired");
                    reset_event_fd(eventfd);
                    trace!("Reset eventfd value");
                }
                let mut buf = [0u8; 512];
                for (fd, id) in &fd_to_srvc_id {
                    if fdset.contains(unsafe { borrow_fd(*fd) }) {
                        if let Some(srvc_unit) = unit_table.get(id) {
                            if let Specific::Service(srvc) = &srvc_unit.specific {
                                let mut_state = &mut *srvc.state.write().unwrap();
                                if let Some(socket) = &mut_state.srvc.notifications {
                                    let old_flags = nix::fcntl::fcntl(
                                        unsafe { borrow_fd(*fd) },
                                        nix::fcntl::FcntlArg::F_GETFL,
                                    )
                                    .unwrap();

                                    let old_flags =
                                        nix::fcntl::OFlag::from_bits(old_flags).unwrap();
                                    let mut new_flags = old_flags;
                                    new_flags.insert(nix::fcntl::OFlag::O_NONBLOCK);
                                    nix::fcntl::fcntl(
                                        unsafe { borrow_fd(*fd) },
                                        nix::fcntl::FcntlArg::F_SETFL(new_flags),
                                    )
                                    .unwrap();
                                    let bytes = {
                                        match socket.recv(&mut buf[..]) {
                                            Ok(b) => b,
                                            Err(e) => match e.kind() {
                                                std::io::ErrorKind::WouldBlock => 0,
                                                _ => panic!("{}", e),
                                            },
                                        }
                                    };
                                    nix::fcntl::fcntl(
                                        unsafe { borrow_fd(*fd) },
                                        nix::fcntl::FcntlArg::F_SETFL(old_flags),
                                    )
                                    .unwrap();
                                    let note_str =
                                        String::from_utf8(buf[..bytes].to_vec()).unwrap();
                                    mut_state.srvc.notifications_buffer.push_str(&note_str);
                                    // Each recv() returns a complete datagram from sd_notify.
                                    // Datagrams use '\n' to separate key=value pairs internally,
                                    // but may not end with '\n'.  If we don't add a separator,
                                    // the last key=value of one datagram merges with the first
                                    // key=value of the next (e.g. "FDNAME=inotifyREADY=1"),
                                    // causing READY=1 to never be parsed.
                                    if bytes > 0 && !note_str.ends_with('\n') {
                                        mut_state.srvc.notifications_buffer.push('\n');
                                    }
                                    crate::notification_handler::handle_notifications_from_buffer(
                                        &mut mut_state.srvc,
                                        &srvc_unit.id.name,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Error while selecting: {e}");
            }
        }
    }
}

pub fn handle_all_std_out(run_info: ArcMutRuntimeInfo) {
    let eventfd = { run_info.read().unwrap().stdout_eventfd };
    loop {
        let fd_to_srvc_id = collect_from_srvc(run_info.clone(), |map, srvc, id| {
            if let Some(StdIo::Piped(r, _w)) = &srvc.stdout {
                map.insert(*r, id);
            }
        });

        let mut fdset = nix::sys::select::FdSet::new();
        for fd in fd_to_srvc_id.keys() {
            fdset.insert(unsafe { borrow_fd(*fd) });
        }
        fdset.insert(unsafe { borrow_fd(eventfd.read_end()) });

        let result = nix::sys::select::select(None, Some(&mut fdset), None, None, None);

        let run_info_locked = run_info.read().unwrap();
        let unit_table = &run_info_locked.unit_table;
        match result {
            Ok(_) => {
                if fdset.contains(unsafe { borrow_fd(eventfd.read_end()) }) {
                    trace!("Interrupted stdout select because the eventfd fired");
                    reset_event_fd(eventfd);
                    trace!("Reset eventfd value");
                }
                let mut buf = [0u8; 512];
                let mut eof_ids = Vec::new();
                for (fd, id) in &fd_to_srvc_id {
                    if fdset.contains(unsafe { borrow_fd(*fd) }) {
                        if let Some(srvc_unit) = unit_table.get(id) {
                            let name = srvc_unit.id.name.clone();
                            if let Specific::Service(srvc) = &srvc_unit.specific {
                                let mut_state = &mut *srvc.state.write().unwrap();
                                let status = srvc_unit.common.status.read().unwrap();

                                let old_flags = nix::fcntl::fcntl(
                                    unsafe { borrow_fd(*fd) },
                                    nix::fcntl::FcntlArg::F_GETFL,
                                )
                                .unwrap();
                                let old_flags = nix::fcntl::OFlag::from_bits(old_flags).unwrap();
                                let mut new_flags = old_flags;
                                new_flags.insert(nix::fcntl::OFlag::O_NONBLOCK);
                                nix::fcntl::fcntl(
                                    unsafe { borrow_fd(*fd) },
                                    nix::fcntl::FcntlArg::F_SETFL(new_flags),
                                )
                                .unwrap();

                                ////
                                let bytes = match nix::unistd::read(
                                    unsafe { borrow_fd(*fd) },
                                    &mut buf[..],
                                ) {
                                    Ok(b) => b,
                                    Err(nix::Error::EWOULDBLOCK) => 0,
                                    Err(e) => panic!("{}", e),
                                };
                                ////

                                nix::fcntl::fcntl(
                                    unsafe { borrow_fd(*fd) },
                                    nix::fcntl::FcntlArg::F_SETFL(old_flags),
                                )
                                .unwrap();

                                if bytes == 0 {
                                    // EOF: the write end of the pipe was closed.
                                    // This happens when a service redirects its
                                    // stdout to a TTY or file (e.g. debug-shell).
                                    // Mark for cleanup to avoid a busy select loop.
                                    eof_ids.push((id.clone(), name));
                                } else {
                                    mut_state.srvc.stdout_buffer.extend(&buf[..bytes]);
                                    mut_state.srvc.log_stdout_lines(&name, &status).unwrap();
                                }
                            }
                        }
                    }
                }
                // Close pipes that hit EOF so they are no longer selected.
                for (id, name) in eof_ids {
                    if let Some(srvc_unit) = unit_table.get(&id) {
                        if let Specific::Service(srvc) = &srvc_unit.specific {
                            let mut_state = &mut *srvc.state.write().unwrap();
                            if let Some(StdIo::Piped(r, _w)) = &mut_state.srvc.stdout {
                                trace!("stdout pipe EOF for service {name}, closing read end");
                                let _ = nix::unistd::close(*r);
                            }
                            mut_state.srvc.stdout = None;
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Error while selecting: {e}");
            }
        }
    }
}

pub fn handle_all_std_err(run_info: ArcMutRuntimeInfo) {
    let eventfd = { run_info.read().unwrap().stderr_eventfd };
    loop {
        let fd_to_srvc_id = collect_from_srvc(run_info.clone(), |map, srvc, id| {
            if let Some(StdIo::Piped(r, _w)) = &srvc.stderr {
                map.insert(*r, id);
            }
        });

        let mut fdset = nix::sys::select::FdSet::new();
        for fd in fd_to_srvc_id.keys() {
            fdset.insert(unsafe { borrow_fd(*fd) });
        }
        fdset.insert(unsafe { borrow_fd(eventfd.read_end()) });

        let result = nix::sys::select::select(None, Some(&mut fdset), None, None, None);
        let run_info_locked = run_info.read().unwrap();
        let unit_table = &run_info_locked.unit_table;

        match result {
            Ok(_) => {
                if fdset.contains(unsafe { borrow_fd(eventfd.read_end()) }) {
                    trace!("Interrupted stderr select because the eventfd fired");
                    reset_event_fd(eventfd);
                    trace!("Reset eventfd value");
                }
                let mut buf = [0u8; 512];
                let mut eof_ids = Vec::new();
                for (fd, id) in &fd_to_srvc_id {
                    if fdset.contains(unsafe { borrow_fd(*fd) }) {
                        if let Some(srvc_unit) = unit_table.get(id) {
                            let name = srvc_unit.id.name.clone();
                            if let Specific::Service(srvc) = &srvc_unit.specific {
                                let mut_state = &mut *srvc.state.write().unwrap();
                                let status = srvc_unit.common.status.read().unwrap();

                                let old_flags = nix::fcntl::fcntl(
                                    unsafe { borrow_fd(*fd) },
                                    nix::fcntl::FcntlArg::F_GETFL,
                                )
                                .unwrap();
                                let old_flags = nix::fcntl::OFlag::from_bits(old_flags).unwrap();
                                let mut new_flags = old_flags;
                                new_flags.insert(nix::fcntl::OFlag::O_NONBLOCK);
                                nix::fcntl::fcntl(
                                    unsafe { borrow_fd(*fd) },
                                    nix::fcntl::FcntlArg::F_SETFL(new_flags),
                                )
                                .unwrap();

                                ////
                                let bytes = match nix::unistd::read(
                                    unsafe { borrow_fd(*fd) },
                                    &mut buf[..],
                                ) {
                                    Ok(b) => b,
                                    Err(nix::Error::EWOULDBLOCK) => 0,
                                    Err(e) => panic!("{}", e),
                                };
                                ////
                                nix::fcntl::fcntl(
                                    unsafe { borrow_fd(*fd) },
                                    nix::fcntl::FcntlArg::F_SETFL(old_flags),
                                )
                                .unwrap();

                                if bytes == 0 {
                                    // EOF: the write end of the pipe was closed.
                                    // This happens when a service redirects its
                                    // stderr to a TTY or file (e.g. debug-shell).
                                    // Mark for cleanup to avoid a busy select loop.
                                    eof_ids.push((id.clone(), name));
                                } else {
                                    mut_state.srvc.stderr_buffer.extend(&buf[..bytes]);
                                    mut_state.srvc.log_stderr_lines(&name, &status).unwrap();
                                }
                            }
                        }
                    }
                }
                // Close pipes that hit EOF so they are no longer selected.
                for (id, name) in eof_ids {
                    if let Some(srvc_unit) = unit_table.get(&id) {
                        if let Specific::Service(srvc) = &srvc_unit.specific {
                            let mut_state = &mut *srvc.state.write().unwrap();
                            if let Some(StdIo::Piped(r, _w)) = &mut_state.srvc.stderr {
                                trace!("stderr pipe EOF for service {name}, closing read end");
                                let _ = nix::unistd::close(*r);
                            }
                            mut_state.srvc.stderr = None;
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Error while selecting: {e}");
            }
        }
    }
}

pub fn handle_notification_message(msg: &str, srvc: &mut Service, name: &str) {
    let split: Vec<_> = msg.splitn(2, '=').collect();
    if split.is_empty() {
        return;
    }
    match split[0] {
        "STATUS" => {
            if split.len() > 1 {
                srvc.status_msgs.push(split[1].to_owned());
                trace!(
                    "New status message pushed from service {}: {}",
                    name,
                    srvc.status_msgs.last().unwrap()
                );
            }
        }
        "READY" => {
            srvc.signaled_ready = true;
        }
        // Known sd_notify fields we accept but don't fully implement yet.
        // Logged at trace level to avoid spamming warnings during normal operation.
        "FDSTORE" | "FDSTOREREMOVE" | "FDNAME" => {
            trace!("Service {name}: sd_notify {msg} (fd store not fully implemented)");
        }
        "MAINPID" => {
            trace!("Service {name}: sd_notify MAINPID notification: {msg}");
        }
        "WATCHDOG" | "WATCHDOG_USEC" => {
            trace!("Service {name}: sd_notify watchdog notification: {msg}");
        }
        "RELOADING" | "STOPPING" => {
            trace!("Service {name}: sd_notify state transition: {msg}");
        }
        "ERRNO" | "BUSERROR" | "EXIT_STATUS" => {
            trace!("Service {name}: sd_notify error/exit info: {msg}");
        }
        "NOTIFYACCESS" | "MONOTONIC_USEC" | "INVOCATION_ID" => {
            trace!("Service {name}: sd_notify metadata: {msg}");
        }
        _ => {
            warn!("Unknown notification from service {name}: {msg}");
        }
    }
}

pub fn handle_notifications_from_buffer(srvc: &mut Service, name: &str) {
    while srvc.notifications_buffer.contains('\n') {
        let (line, rest) = srvc
            .notifications_buffer
            .split_at(srvc.notifications_buffer.find('\n').unwrap());
        let line = line.to_owned();
        srvc.notifications_buffer = rest[1..].to_owned();

        handle_notification_message(&line, srvc, name);
    }
}
