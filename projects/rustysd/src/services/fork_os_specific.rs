use crate::units::PlatformSpecificServiceFields;
use crate::units::ServiceConfig;

#[cfg(feature = "cgroups")]
use crate::platform::cgroups;
#[cfg(feature = "cgroups")]
use crate::units::Delegate;

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
            let uid = srvc.exec_config.user;
            let gid = srvc.exec_config.group;
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
