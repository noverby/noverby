//! collect the different streams from the services
//! Stdout and stderr get redirected to the normal stdout/err but are prefixed with a unique string to identify their output
//! streams from the notification sockets get parsed and applied to the respective service

use log::trace;
use log::warn;

use crate::lock_ext::RwLockExt;
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
    let run_info_locked = run_info.read_poisoned();
    let unit_table = &run_info_locked.unit_table;
    unit_table
        .iter()
        .fold(HashMap::new(), |mut map, (id, srvc_unit)| {
            if let Specific::Service(srvc) = &srvc_unit.specific {
                let state = &*srvc.state.read_poisoned();
                f(&mut map, &state.srvc, id.clone());
            }
            map
        })
}

pub fn handle_all_streams(run_info: ArcMutRuntimeInfo) {
    let eventfd = { run_info.read_poisoned().notification_eventfd };
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

        let run_info_locked = run_info.read_poisoned();
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
                    if fdset.contains(unsafe { borrow_fd(*fd) })
                        && let Some(srvc_unit) = unit_table.get(id)
                        && let Specific::Service(srvc) = &srvc_unit.specific
                    {
                        let mut_state = &mut *srvc.state.write_poisoned();
                        if let Some(socket) = &mut_state.srvc.notifications {
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
                            let note_str = String::from_utf8(buf[..bytes].to_vec()).unwrap();
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
            Err(e) => {
                warn!("Error while selecting: {e}");
            }
        }
    }
}

pub fn handle_all_std_out(run_info: ArcMutRuntimeInfo) {
    let eventfd = { run_info.read_poisoned().stdout_eventfd };
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

        let run_info_locked = run_info.read_poisoned();
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
                    if fdset.contains(unsafe { borrow_fd(*fd) })
                        && let Some(srvc_unit) = unit_table.get(id)
                    {
                        let name = srvc_unit.id.name.clone();
                        if let Specific::Service(srvc) = &srvc_unit.specific {
                            let mut_state = &mut *srvc.state.write_poisoned();
                            let status = srvc_unit.common.status.read_poisoned();

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
                            let bytes =
                                match nix::unistd::read(unsafe { borrow_fd(*fd) }, &mut buf[..]) {
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
                // Close pipes that hit EOF so they are no longer selected.
                for (id, name) in eof_ids {
                    if let Some(srvc_unit) = unit_table.get(&id)
                        && let Specific::Service(srvc) = &srvc_unit.specific
                    {
                        let mut_state = &mut *srvc.state.write_poisoned();
                        if let Some(StdIo::Piped(r, _w)) = &mut_state.srvc.stdout {
                            trace!("stdout pipe EOF for service {name}, closing read end");
                            let _ = nix::unistd::close(*r);
                        }
                        mut_state.srvc.stdout = None;
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
    let eventfd = { run_info.read_poisoned().stderr_eventfd };
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
        let run_info_locked = run_info.read_poisoned();
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
                    if fdset.contains(unsafe { borrow_fd(*fd) })
                        && let Some(srvc_unit) = unit_table.get(id)
                    {
                        let name = srvc_unit.id.name.clone();
                        if let Specific::Service(srvc) = &srvc_unit.specific {
                            let mut_state = &mut *srvc.state.write_poisoned();
                            let status = srvc_unit.common.status.read_poisoned();

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
                            let bytes =
                                match nix::unistd::read(unsafe { borrow_fd(*fd) }, &mut buf[..]) {
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
                // Close pipes that hit EOF so they are no longer selected.
                for (id, name) in eof_ids {
                    if let Some(srvc_unit) = unit_table.get(&id)
                        && let Specific::Service(srvc) = &srvc_unit.specific
                    {
                        let mut_state = &mut *srvc.state.write_poisoned();
                        if let Some(StdIo::Piped(r, _w)) = &mut_state.srvc.stderr {
                            trace!("stderr pipe EOF for service {name}, closing read end");
                            let _ = nix::unistd::close(*r);
                        }
                        mut_state.srvc.stderr = None;
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
            // READY=1 after RELOADING=1 means reload is complete
            if srvc.reloading {
                srvc.reloading = false;
                trace!("Service {name}: reload complete (READY=1 after RELOADING=1)");
            }
        }
        "RELOADING" => {
            if split.len() > 1 && split[1] == "1" {
                srvc.reloading = true;
                // Per sd_notify(3), RELOADING=1 implies the service is not
                // ready during reload. The service will send READY=1 again
                // when reload completes.
                srvc.signaled_ready = false;
                trace!("Service {name}: entering reload state (RELOADING=1)");
            }
        }
        "STOPPING" => {
            if split.len() > 1 && split[1] == "1" {
                srvc.stopping = true;
                trace!("Service {name}: entering stopping state (STOPPING=1)");
            }
        }
        "MAINPID" => {
            if split.len() > 1 {
                match split[1].parse::<i32>() {
                    Ok(pid) if pid > 0 => {
                        let new_pid = nix::unistd::Pid::from_raw(pid);
                        trace!(
                            "Service {name}: MAINPID updated to {} (was {:?})",
                            pid, srvc.main_pid
                        );
                        srvc.main_pid = Some(new_pid);
                    }
                    Ok(pid) => {
                        trace!("Service {name}: ignoring invalid MAINPID={pid}");
                    }
                    Err(e) => {
                        trace!(
                            "Service {name}: ignoring unparsable MAINPID={}: {e}",
                            split[1]
                        );
                    }
                }
            }
        }
        "WATCHDOG" => {
            if split.len() > 1 {
                match split[1] {
                    "1" => {
                        srvc.watchdog_last_ping = Some(std::time::Instant::now());
                        trace!("Service {name}: watchdog ping received (WATCHDOG=1)");
                    }
                    "trigger" => {
                        // The service is requesting that the watchdog action be
                        // triggered immediately (e.g. because it detected an
                        // internal error). We log this; the watchdog checker
                        // thread will act on it.
                        warn!("Service {name}: watchdog trigger requested (WATCHDOG=trigger)");
                        // Clear the last ping so the watchdog checker sees it
                        // as expired immediately.
                        srvc.watchdog_last_ping = None;
                    }
                    other => {
                        trace!("Service {name}: ignoring unknown WATCHDOG={other}");
                    }
                }
            }
        }
        "WATCHDOG_USEC" => {
            if split.len() > 1 {
                // The service is requesting a change to its watchdog timeout.
                // We log this for now; a full implementation would update
                // WatchdogSec= dynamically.
                trace!(
                    "Service {name}: WATCHDOG_USEC={} (dynamic timeout change noted)",
                    split[1]
                );
            }
        }
        // Known sd_notify fields we accept but don't fully implement yet.
        // Logged at trace level to avoid spamming warnings during normal operation.
        "FDSTORE" | "FDSTOREREMOVE" | "FDNAME" => {
            trace!("Service {name}: sd_notify {msg} (fd store not fully implemented)");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::Service;

    fn make_test_service() -> Service {
        Service {
            pid: None,
            main_pid: None,
            status_msgs: Vec::new(),
            process_group: None,
            signaled_ready: false,
            reloading: false,
            stopping: false,
            watchdog_last_ping: None,
            notifications: None,
            notifications_path: None,
            stdout: None,
            stderr: None,
            notifications_buffer: String::new(),
            stdout_buffer: Vec::new(),
            stderr_buffer: Vec::new(),
        }
    }

    #[test]
    fn test_ready_sets_signaled_ready() {
        let mut srvc = make_test_service();
        handle_notification_message("READY=1", &mut srvc, "test.service");
        assert!(srvc.signaled_ready);
    }

    #[test]
    fn test_status_pushes_message() {
        let mut srvc = make_test_service();
        handle_notification_message("STATUS=Starting up...", &mut srvc, "test.service");
        assert_eq!(srvc.status_msgs.len(), 1);
        assert_eq!(srvc.status_msgs[0], "Starting up...");
    }

    #[test]
    fn test_status_multiple_messages() {
        let mut srvc = make_test_service();
        handle_notification_message("STATUS=Starting", &mut srvc, "test.service");
        handle_notification_message("STATUS=Ready", &mut srvc, "test.service");
        assert_eq!(srvc.status_msgs.len(), 2);
        assert_eq!(srvc.status_msgs[0], "Starting");
        assert_eq!(srvc.status_msgs[1], "Ready");
    }

    #[test]
    fn test_mainpid_valid() {
        let mut srvc = make_test_service();
        handle_notification_message("MAINPID=12345", &mut srvc, "test.service");
        assert_eq!(srvc.main_pid, Some(nix::unistd::Pid::from_raw(12345)));
    }

    #[test]
    fn test_mainpid_updates_existing() {
        let mut srvc = make_test_service();
        srvc.main_pid = Some(nix::unistd::Pid::from_raw(100));
        handle_notification_message("MAINPID=200", &mut srvc, "test.service");
        assert_eq!(srvc.main_pid, Some(nix::unistd::Pid::from_raw(200)));
    }

    #[test]
    fn test_mainpid_zero_ignored() {
        let mut srvc = make_test_service();
        handle_notification_message("MAINPID=0", &mut srvc, "test.service");
        assert_eq!(srvc.main_pid, None);
    }

    #[test]
    fn test_mainpid_negative_ignored() {
        let mut srvc = make_test_service();
        handle_notification_message("MAINPID=-1", &mut srvc, "test.service");
        assert_eq!(srvc.main_pid, None);
    }

    #[test]
    fn test_mainpid_invalid_ignored() {
        let mut srvc = make_test_service();
        handle_notification_message("MAINPID=notanumber", &mut srvc, "test.service");
        assert_eq!(srvc.main_pid, None);
    }

    #[test]
    fn test_mainpid_no_value_ignored() {
        let mut srvc = make_test_service();
        handle_notification_message("MAINPID", &mut srvc, "test.service");
        assert_eq!(srvc.main_pid, None);
    }

    #[test]
    fn test_watchdog_ping() {
        let mut srvc = make_test_service();
        assert!(srvc.watchdog_last_ping.is_none());
        handle_notification_message("WATCHDOG=1", &mut srvc, "test.service");
        assert!(srvc.watchdog_last_ping.is_some());
    }

    #[test]
    fn test_watchdog_trigger_clears_ping() {
        let mut srvc = make_test_service();
        srvc.watchdog_last_ping = Some(std::time::Instant::now());
        handle_notification_message("WATCHDOG=trigger", &mut srvc, "test.service");
        assert!(srvc.watchdog_last_ping.is_none());
    }

    #[test]
    fn test_watchdog_unknown_value_ignored() {
        let mut srvc = make_test_service();
        handle_notification_message("WATCHDOG=0", &mut srvc, "test.service");
        assert!(srvc.watchdog_last_ping.is_none());
    }

    #[test]
    fn test_reloading_sets_flag() {
        let mut srvc = make_test_service();
        srvc.signaled_ready = true;
        handle_notification_message("RELOADING=1", &mut srvc, "test.service");
        assert!(srvc.reloading);
        // RELOADING=1 clears signaled_ready
        assert!(!srvc.signaled_ready);
    }

    #[test]
    fn test_reloading_then_ready_clears_reload() {
        let mut srvc = make_test_service();
        handle_notification_message("RELOADING=1", &mut srvc, "test.service");
        assert!(srvc.reloading);
        assert!(!srvc.signaled_ready);

        handle_notification_message("READY=1", &mut srvc, "test.service");
        assert!(!srvc.reloading);
        assert!(srvc.signaled_ready);
    }

    #[test]
    fn test_stopping_sets_flag() {
        let mut srvc = make_test_service();
        handle_notification_message("STOPPING=1", &mut srvc, "test.service");
        assert!(srvc.stopping);
    }

    #[test]
    fn test_stopping_zero_ignored() {
        let mut srvc = make_test_service();
        handle_notification_message("STOPPING=0", &mut srvc, "test.service");
        assert!(!srvc.stopping);
    }

    #[test]
    fn test_watchdog_usec_does_not_crash() {
        let mut srvc = make_test_service();
        handle_notification_message("WATCHDOG_USEC=30000000", &mut srvc, "test.service");
        // Just verifying no panic or crash
    }

    #[test]
    fn test_unknown_notification_does_not_crash() {
        let mut srvc = make_test_service();
        handle_notification_message("CUSTOM_FIELD=value", &mut srvc, "test.service");
        // Should not crash, just warn
    }

    #[test]
    fn test_empty_message_does_not_crash() {
        let mut srvc = make_test_service();
        handle_notification_message("", &mut srvc, "test.service");
    }

    #[test]
    fn test_fdstore_does_not_crash() {
        let mut srvc = make_test_service();
        handle_notification_message("FDSTORE=1", &mut srvc, "test.service");
        handle_notification_message("FDNAME=myfd", &mut srvc, "test.service");
        handle_notification_message("FDSTOREREMOVE=1", &mut srvc, "test.service");
    }

    #[test]
    fn test_errno_does_not_crash() {
        let mut srvc = make_test_service();
        handle_notification_message("ERRNO=2", &mut srvc, "test.service");
        handle_notification_message(
            "BUSERROR=org.freedesktop.DBus.Error.Failed",
            &mut srvc,
            "test.service",
        );
        handle_notification_message("EXIT_STATUS=1", &mut srvc, "test.service");
    }

    #[test]
    fn test_handle_notifications_from_buffer() {
        let mut srvc = make_test_service();
        srvc.notifications_buffer = "READY=1\nSTATUS=Running\n".to_owned();
        handle_notifications_from_buffer(&mut srvc, "test.service");
        assert!(srvc.signaled_ready);
        assert_eq!(srvc.status_msgs.len(), 1);
        assert_eq!(srvc.status_msgs[0], "Running");
        assert!(srvc.notifications_buffer.is_empty());
    }

    #[test]
    fn test_handle_notifications_from_buffer_partial() {
        let mut srvc = make_test_service();
        // Buffer with a complete line and an incomplete one
        srvc.notifications_buffer = "READY=1\nSTAT".to_owned();
        handle_notifications_from_buffer(&mut srvc, "test.service");
        assert!(srvc.signaled_ready);
        // The incomplete "STAT" should remain in the buffer
        assert_eq!(srvc.notifications_buffer, "STAT");
    }

    #[test]
    fn test_handle_notifications_from_buffer_mainpid_and_watchdog() {
        let mut srvc = make_test_service();
        srvc.notifications_buffer = "MAINPID=42\nWATCHDOG=1\n".to_owned();
        handle_notifications_from_buffer(&mut srvc, "test.service");
        assert_eq!(srvc.main_pid, Some(nix::unistd::Pid::from_raw(42)));
        assert!(srvc.watchdog_last_ping.is_some());
    }

    #[test]
    fn test_full_lifecycle_notify_reload_ready() {
        let mut srvc = make_test_service();

        // Service starts and signals ready
        handle_notification_message("READY=1", &mut srvc, "test.service");
        assert!(srvc.signaled_ready);
        assert!(!srvc.reloading);

        // Service begins reload
        handle_notification_message("RELOADING=1", &mut srvc, "test.service");
        assert!(srvc.reloading);
        assert!(!srvc.signaled_ready);

        // Service finishes reload
        handle_notification_message("READY=1", &mut srvc, "test.service");
        assert!(!srvc.reloading);
        assert!(srvc.signaled_ready);

        // Service begins stopping
        handle_notification_message("STOPPING=1", &mut srvc, "test.service");
        assert!(srvc.stopping);
    }

    #[test]
    fn test_watchdog_multiple_pings() {
        let mut srvc = make_test_service();

        handle_notification_message("WATCHDOG=1", &mut srvc, "test.service");
        let first_ping = srvc.watchdog_last_ping.unwrap();

        // Small delay to ensure distinct timestamps
        std::thread::sleep(std::time::Duration::from_millis(10));

        handle_notification_message("WATCHDOG=1", &mut srvc, "test.service");
        let second_ping = srvc.watchdog_last_ping.unwrap();

        assert!(second_ping >= first_ping);
    }
}
