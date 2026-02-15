use log::error;
use log::trace;
use log::warn;
use which::which;

use super::fork_child;
use crate::fd_store::FDStore;
use crate::services::RunCmdError;
use crate::services::Service;
use crate::units::{CommandlinePrefix, ServiceConfig, StdIoOption};

use std::path::Path;

/// Resolve a User= value (name or numeric UID) to a raw `uid_t`.
/// Falls back to the current process UID if `user` is `None`.
pub fn resolve_uid(user: &Option<String>) -> Result<libc::uid_t, String> {
    match user {
        Some(user_str) => {
            if let Ok(uid) = user_str.parse::<u32>() {
                Ok(uid)
            } else {
                let pwentry = crate::platform::pwnam::getpwnam_r(user_str)
                    .map_err(|_| format!("Couldn't resolve uid for username: {user_str}"))?;
                Ok(pwentry.uid.as_raw())
            }
        }
        None => Ok(nix::unistd::getuid().as_raw()),
    }
}

/// Resolve a Group= value (name or numeric GID) to a raw `gid_t`.
/// Falls back to the current process GID if `group` is `None`.
pub fn resolve_gid(group: &Option<String>) -> Result<libc::gid_t, String> {
    match group {
        Some(group_str) => {
            if let Ok(gid) = group_str.parse::<u32>() {
                Ok(gid)
            } else {
                let grentry = crate::platform::grnam::getgrnam_r(group_str)
                    .map_err(|_| format!("Couldn't resolve gid for groupname: {group_str}"))?;
                Ok(grentry.gid.as_raw())
            }
        }
        None => Ok(nix::unistd::getgid().as_raw()),
    }
}

/// Resolve a list of SupplementaryGroups= values to raw `gid_t` values.
fn resolve_supplementary_gids(groups: &[String]) -> Result<Vec<libc::gid_t>, String> {
    groups
        .iter()
        .map(|g| {
            if let Ok(gid) = g.parse::<u32>() {
                Ok(gid)
            } else {
                let grentry = crate::platform::grnam::getgrnam_r(g)
                    .map_err(|_| format!("Couldn't resolve gid for supplementary group: {g}"))?;
                Ok(grentry.gid.as_raw())
            }
        })
        .collect()
}

fn start_service_with_filedescriptors(
    self_path: &Path,
    srvc: &mut Service,
    conf: &ServiceConfig,
    name: &str,
    fd_store: &FDStore,
) -> Result<(), RunCmdError> {
    let exec = conf.exec.as_ref().ok_or_else(|| {
        RunCmdError::SpawnError(
            name.to_owned(),
            "Service has no ExecStart command".to_string(),
        )
    })?;

    // check if executable even exists
    let cmd = which(&exec.cmd).map_err(|err| {
        RunCmdError::SpawnError(
            name.to_owned(),
            format!("Could not resolve command to an executable file: {err:?}"),
        )
    })?;
    if !cmd.exists() {
        error!(
            "The service {} specified an executable that does not exist: {:?}",
            name, &exec.cmd
        );
        return Err(RunCmdError::SpawnError(
            exec.cmd.clone(),
            "Executable does not exist".to_string(),
        ));
    }
    if !cmd.is_file() {
        error!(
            "The service {} specified an executable that is not a file: {:?}",
            name, &cmd
        );
        return Err(RunCmdError::SpawnError(
            exec.cmd.clone(),
            "Executable does not exist (is a directory)".to_string(),
        ));
    }

    // 1. fork
    // 1. in fork use dup2 to map all relevant file desrciptors to 3..x
    // 1. in fork mark all other file descriptors with FD_CLOEXEC
    // 1. in fork set relevant env variables $LISTEN_FDS $LISTEN_PID
    // 1. in fork execve the cmd with the args
    // 1. in parent set pid and return. Waiting will be done afterwards if necessary

    let notifications_path = {
        if let Some(p) = &srvc.notifications_path {
            p.to_str().unwrap().to_owned()
        } else {
            return Err(RunCmdError::Generic(format!(
                "Tried to start service: {name} without a notifications path",
            )));
        }
    };

    super::fork_os_specific::pre_fork_os_specific(conf).map_err(RunCmdError::Generic)?;

    let mut fds = Vec::new();
    let mut names = Vec::new();

    for socket in &conf.sockets {
        let sock_fds = fd_store
            .get_global(&socket.name)
            .unwrap()
            .iter()
            .map(|(_, _, fd)| fd.as_raw_fd())
            .collect::<Vec<_>>();

        let sock_names = fd_store
            .get_global(&socket.name)
            .unwrap()
            .iter()
            .map(|(_, name, _)| name.clone())
            .collect::<Vec<_>>();

        fds.extend(sock_fds);
        names.extend(sock_names);
    }

    // We first exec into our own executable again and apply this config
    // We transfer the config via a anonymous shared memory file
    let exec_helper_conf = crate::entrypoints::ExecHelperConfig {
        name: name.to_owned(),
        cmd,
        args: exec.args.clone(),
        use_first_arg_as_argv0: exec.prefixes.contains(&CommandlinePrefix::AtSign),
        env: {
            let default_path = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
            let mut env = vec![
                // Use the inherited PATH if available (important for NixOS where
                // executables live in /nix/store and are reachable via
                // /run/current-system/sw/bin, /run/wrappers/bin, etc.).
                // Fall back to the systemd default FHS PATH (see man systemd.exec).
                (
                    "PATH".to_owned(),
                    std::env::var("PATH").unwrap_or_else(|_| default_path.to_owned()),
                ),
            ];
            // Load EnvironmentFile= files first (lower priority than Environment=).
            // Each file contains lines of KEY=VALUE pairs.
            for (path, optional) in &conf.exec_config.environment_files {
                match std::fs::read_to_string(path) {
                    Ok(contents) => {
                        for line in contents.lines() {
                            let line = line.trim();
                            // Skip comments and empty lines
                            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                                continue;
                            }
                            if let Some((key, value)) = line.split_once('=') {
                                let key = key.trim().to_owned();
                                // Strip optional surrounding quotes from the value
                                let value = value.trim();
                                let value = if (value.starts_with('"') && value.ends_with('"'))
                                    || (value.starts_with('\'') && value.ends_with('\''))
                                {
                                    value[1..value.len() - 1].to_owned()
                                } else {
                                    value.to_owned()
                                };
                                env.retain(|(ek, _)| *ek != key);
                                env.push((key, value));
                            }
                        }
                    }
                    Err(e) => {
                        if *optional {
                            trace!(
                                "Optional EnvironmentFile not found for {}: {:?}: {}",
                                name,
                                path,
                                e
                            );
                        } else {
                            warn!(
                                "Failed to read EnvironmentFile for {}: {:?}: {}",
                                name, path, e
                            );
                        }
                    }
                }
            }
            // Apply the service's Environment= settings (may override PATH
            // and EnvironmentFile= values)
            if let Some(ref vars) = conf.exec_config.environment {
                for (k, v) in &vars.vars {
                    // Remove any prior entry with the same key so the unit's
                    // value takes precedence over the defaults above.
                    env.retain(|(ek, _)| ek != k);
                    env.push((k.clone(), v.clone()));
                }
            }
            // Internal variables added last — these must not be overridden
            env.push(("LISTEN_FDS".to_owned(), format!("{}", names.len())));
            env.push(("LISTEN_FDNAMES".to_owned(), names.join(":")));
            env.push(("NOTIFY_SOCKET".to_owned(), notifications_path));
            env
        },
        group: resolve_gid(&conf.exec_config.group)
            .map_err(|e| RunCmdError::SpawnError(name.to_owned(), e))?,
        supplementary_groups: resolve_supplementary_gids(&conf.exec_config.supplementary_groups)
            .map_err(|e| RunCmdError::SpawnError(name.to_owned(), e))?,
        user: resolve_uid(&conf.exec_config.user)
            .map_err(|e| RunCmdError::SpawnError(name.to_owned(), e))?,

        working_directory: conf.exec_config.working_directory.clone(),
        state_directory: conf.exec_config.state_directory.clone(),

        platform_specific: conf.platform_specific.clone(),

        limit_nofile: conf.limit_nofile,

        stdin_option: conf.exec_config.stdin_option.clone(),
        tty_path: conf.exec_config.tty_path.clone(),
        tty_reset: conf.exec_config.tty_reset,
        tty_vhangup: conf.exec_config.tty_vhangup,
        tty_vt_disallocate: conf.exec_config.tty_vt_disallocate,
        stdout_is_inherit: matches!(
            conf.exec_config.stdout_path,
            None | Some(StdIoOption::Inherit)
                | Some(StdIoOption::Journal)
                | Some(StdIoOption::Kmsg)
                | Some(StdIoOption::Tty)
        ),
        stderr_is_inherit: matches!(
            conf.exec_config.stderr_path,
            None | Some(StdIoOption::Inherit)
                | Some(StdIoOption::Journal)
                | Some(StdIoOption::Kmsg)
                | Some(StdIoOption::Tty)
        ),
        stdout_is_tty: matches!(conf.exec_config.stdout_path, Some(StdIoOption::Tty)),
        stderr_is_tty: matches!(conf.exec_config.stderr_path, Some(StdIoOption::Tty)),
    };

    let marshalled_config = serde_json::to_string(&exec_helper_conf).unwrap();

    // crate the shared memory file
    let exec_helper_conf_fd = shmemfdrs::create_shmem(
        std::ffi::CString::new(name).unwrap(),
        marshalled_config.len() + 1,
    );
    if exec_helper_conf_fd < 0 {
        return Err(RunCmdError::CreatingShmemFailed(
            name.to_owned(),
            std::io::Error::from_raw_os_error(exec_helper_conf_fd).kind(),
        ));
    }
    use std::os::unix::io::FromRawFd;
    let mut exec_helper_conf_file = unsafe { std::fs::File::from_raw_fd(exec_helper_conf_fd) };

    // write the config to the file
    use std::io::Write;
    exec_helper_conf_file
        .write_all(marshalled_config.as_bytes())
        .unwrap();
    exec_helper_conf_file.write_all(b"\n").unwrap();
    use std::io::Seek;
    exec_helper_conf_file
        .seek(std::io::SeekFrom::Start(0))
        .unwrap();

    // need to allocate this before forking. Currently this is just static info, we could only do this once...
    let self_path_cstr = std::ffi::CString::new(self_path.to_str().unwrap()).unwrap();
    let name_arg = std::ffi::CString::new("exec_helper").unwrap();
    let self_args = [name_arg.as_ptr(), std::ptr::null()];

    trace!(
        "Start main executable for service: {name}: {:?} {:?}",
        exec_helper_conf.cmd,
        exec_helper_conf.args
    );
    match unsafe { nix::unistd::fork() } {
        Ok(nix::unistd::ForkResult::Parent { child, .. }) => {
            // make sure the file exists until after we fork before closing it
            drop(exec_helper_conf_file);
            srvc.pid = Some(child);
            srvc.process_group = Some(nix::unistd::Pid::from_raw(-child.as_raw()));
        }
        Ok(nix::unistd::ForkResult::Child) => {
            let stdout = {
                if let Some(stdio) = &srvc.stdout {
                    stdio.write_fd()
                } else {
                    unreachable!();
                }
            };
            let stderr = {
                if let Some(stdio) = &srvc.stderr {
                    stdio.write_fd()
                } else {
                    unreachable!();
                }
            };

            fork_child::after_fork_child(
                &self_path_cstr,
                self_args.as_slice(),
                &mut fds,
                stdout,
                stderr,
                exec_helper_conf_fd,
            );
        }
        Err(e) => error!("Fork for service: {name} failed with: {e}"),
    }
    Ok(())
}

pub fn start_service(
    self_path: &Path,
    srvc: &mut Service,
    conf: &ServiceConfig,
    name: &str,
    fd_store: &FDStore,
) -> Result<(), super::RunCmdError> {
    start_service_with_filedescriptors(self_path, srvc, conf, name, fd_store)?;
    Ok(())
}
