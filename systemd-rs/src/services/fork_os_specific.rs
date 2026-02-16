use crate::units::PlatformSpecificServiceFields;
use crate::units::ServiceConfig;

#[cfg(feature = "cgroups")]
use crate::platform::cgroups;
#[cfg(feature = "cgroups")]
use crate::units::Delegate;
#[cfg(feature = "cgroups")]
use crate::units::TasksMax;

/// This is the place to do anything that is not standard unix but specific to one os. Like cgroups
pub fn pre_fork_os_specific(srvc: &ServiceConfig) -> Result<(), String> {
    #[cfg(feature = "cgroups")]
    {
        use log::trace;

        std::fs::create_dir_all(&srvc.platform_specific.cgroup_path).map_err(|e| {
            format!(
                "Couldnt create service cgroup ({:?}): {}",
                srvc.platform_specific.cgroup_path, e
            )
        })?;

        // When Delegate is enabled, chown the cgroup directory to the service user
        // so the service process can manage its own sub-cgroup hierarchy.
        if srvc.delegate != Delegate::No {
            let uid = super::start_service::resolve_uid(&srvc.exec_config.user)
                .map_err(|e| format!("Couldn't resolve user for cgroup delegation: {e}"))?;
            let uid = nix::unistd::Uid::from_raw(uid);
            let gid = super::start_service::resolve_gid(&srvc.exec_config.group)
                .map_err(|e| format!("Couldn't resolve group for cgroup delegation: {e}"))?;
            let gid = nix::unistd::Gid::from_raw(gid);
            trace!(
                "Delegating cgroup {:?} to uid={} gid={}",
                &srvc.platform_specific.cgroup_path,
                uid,
                gid
            );
            nix::unistd::chown(&srvc.platform_specific.cgroup_path, Some(uid), Some(gid)).map_err(
                |e| {
                    format!(
                        "Couldnt chown service cgroup ({:?}) to uid={} gid={}: {}",
                        srvc.platform_specific.cgroup_path, uid, gid, e
                    )
                },
            )?;
        }

        // Apply TasksMax limit via the pids cgroup controller
        if let Some(ref tasks_max) = srvc.tasks_max {
            let pids_max_path = srvc.platform_specific.cgroup_path.join("pids.max");
            if pids_max_path.exists() || pids_max_path.parent().map_or(false, |p| p.exists()) {
                let value = match tasks_max {
                    TasksMax::Value(n) => n.to_string(),
                    TasksMax::Percent(pct) => {
                        // Read the system-wide pid limit and compute the percentage
                        let pid_max = std::fs::read_to_string("/proc/sys/kernel/pid_max")
                            .unwrap_or_else(|_| "32768".to_owned());
                        let pid_max: u64 = pid_max.trim().parse().unwrap_or(32768);
                        let limit = (pid_max * pct / 100).max(1);
                        limit.to_string()
                    }
                    TasksMax::Infinity => "max".to_owned(),
                };
                trace!(
                    "Setting TasksMax={} for cgroup {:?}",
                    value,
                    &srvc.platform_specific.cgroup_path
                );
                if let Err(e) = std::fs::write(&pids_max_path, &value) {
                    trace!(
                        "Could not write pids.max for service cgroup ({:?}): {}",
                        pids_max_path,
                        e
                    );
                }
            } else {
                trace!(
                    "pids.max not available for cgroup {:?}, skipping TasksMax",
                    &srvc.platform_specific.cgroup_path
                );
            }
        }
    }
    let _ = srvc;
    Ok(())
}

pub const fn post_fork_os_specific(conf: &PlatformSpecificServiceFields) -> Result<(), String> {
    #[cfg(feature = "cgroups")]
    {
        use log::trace;
        trace!("Move service to cgroup: {:?}", &conf.cgroup_path);
        cgroups::move_self_to_cgroup(&conf.cgroup_path)
            .map_err(|e| format!("postfork os specific: {}", e))?;
    }
    let _ = conf;
    Ok(())
}
