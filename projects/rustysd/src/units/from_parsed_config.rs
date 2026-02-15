use crate::services::Service;
use crate::sockets::Socket;
use crate::units::{
    Common, CommonState, Dependencies, ExecConfig, ParsedExecSection, ParsedInstallSection,
    ParsedServiceConfig, ParsedSingleSocketConfig, ParsedSocketConfig, ParsedTargetConfig,
    ParsedUnitSection, PlatformSpecificServiceFields, ServiceConfig, ServiceSpecific, ServiceState,
    SingleSocketConfig, SocketConfig, SocketSpecific, SocketState, Specific, TargetSpecific,
    TargetState, Unit, UnitConfig, UnitId, UnitIdKind, UnitStatus,
};

#[cfg(feature = "cgroups")]
use log::trace;
use log::warn;

use std::convert::TryInto;
use std::path::PathBuf;
use std::sync::RwLock;

#[cfg(feature = "cgroups")]
fn make_cgroup_path(srvc_name: &str) -> Result<PathBuf, String> {
    let rustysd_cgroup =
        crate::platform::cgroups::get_own_freezer(&PathBuf::from("/sys/fs/cgroup"))
            .map_err(|e| format!("Couldnt get own cgroup: {}", e))?;
    let service_cgroup = rustysd_cgroup.join(srvc_name);
    trace!(
        "Service {} will be moved into cgroup: {:?}",
        srvc_name,
        service_cgroup
    );
    Ok(service_cgroup)
}

#[cfg(not(feature = "cgroups"))]
fn make_cgroup_path(_srvc_name: &str) -> Result<PathBuf, String> {
    // doesnt matter, wont be used anyways
    Ok(PathBuf::from("/ree"))
}

pub fn unit_from_parsed_service(conf: ParsedServiceConfig) -> Result<Unit, String> {
    // TODO make the cgroup path dynamic so multiple rustysd instances can exist
    let platform_specific = PlatformSpecificServiceFields {
        #[cfg(target_os = "linux")]
        cgroup_path: make_cgroup_path(&conf.common.name)?,
    };

    let mut sockets: Vec<UnitId> = Vec::new();
    for sock in conf.srvc.sockets {
        sockets.push(sock.as_str().try_into()?);
    }

    let mut common = make_common_from_parsed(conf.common.unit, conf.common.install)?;
    common.unit.refs_by_name.extend(sockets.iter().cloned());

    Ok(Unit {
        id: UnitId {
            kind: UnitIdKind::Service,
            name: conf.common.name,
        },
        common,
        specific: Specific::Service(ServiceSpecific {
            conf: ServiceConfig {
                exec_config: conf.srvc.exec_section.try_into()?,
                sockets,
                accept: conf.srvc.accept,
                dbus_name: conf.srvc.dbus_name,
                pid_file: conf.srvc.pid_file,
                restart: conf.srvc.restart,
                restart_sec: conf.srvc.restart_sec,
                kill_mode: conf.srvc.kill_mode,
                send_sighup: conf.srvc.send_sighup,
                delegate: conf.srvc.delegate,
                tasks_max: conf.srvc.tasks_max,
                limit_nofile: conf.srvc.limit_nofile,
                notifyaccess: conf.srvc.notifyaccess,
                exec: conf.srvc.exec,
                reload: conf.srvc.reload,
                startpre: conf.srvc.startpre,
                startpost: conf.srvc.startpost,
                stop: conf.srvc.stop,
                stoppost: conf.srvc.stoppost,
                srcv_type: conf.srvc.srcv_type,
                starttimeout: conf.srvc.starttimeout,
                stoptimeout: conf.srvc.stoptimeout,
                generaltimeout: conf.srvc.generaltimeout,
                platform_specific,
                slice: conf.srvc.slice,
                remain_after_exit: conf.srvc.remain_after_exit,
                success_exit_status: conf.srvc.success_exit_status,
                memory_pressure_watch: conf.srvc.memory_pressure_watch,
                reload_signal: conf.srvc.reload_signal,
                delegate_subgroup: conf.srvc.delegate_subgroup,
                keyring_mode: conf.srvc.keyring_mode,
                device_allow: conf.srvc.device_allow,
                watchdog_sec: conf.srvc.watchdog_sec,
                ip_address_allow: conf.srvc.ip_address_allow,
                ip_address_deny: conf.srvc.ip_address_deny,
                file_descriptor_store_max: conf.srvc.file_descriptor_store_max,
            },
            state: RwLock::new(ServiceState {
                common: CommonState::default(),
                srvc: Service {
                    pid: None,
                    status_msgs: Vec::new(),
                    process_group: None,
                    signaled_ready: false,
                    notifications: None,
                    notifications_path: None,
                    stdout: None,
                    stderr: None,
                    notifications_buffer: String::new(),
                    stdout_buffer: Vec::new(),
                    stderr_buffer: Vec::new(),
                },
            }),
        }),
    })
}

pub fn unit_from_parsed_socket(conf: ParsedSocketConfig) -> Result<Unit, String> {
    let mut services: Vec<UnitId> = Vec::new();
    for srvc in conf.sock.services {
        services.push(srvc.as_str().try_into()?);
    }

    let mut common = make_common_from_parsed(conf.common.unit, conf.common.install)?;
    common.unit.refs_by_name.extend(services.iter().cloned());

    Ok(Unit {
        id: UnitId {
            kind: UnitIdKind::Socket,
            name: conf.common.name,
        },
        common,
        specific: Specific::Socket(SocketSpecific {
            conf: SocketConfig {
                exec_config: conf.sock.exec_section.try_into()?,
                filedesc_name: conf
                    .sock
                    .filedesc_name
                    .unwrap_or_else(|| "unknown".to_owned()),
                services,
                sockets: conf.sock.sockets.into_iter().map(Into::into).collect(),
            },
            state: RwLock::new(SocketState {
                common: CommonState::default(),
                sock: Socket { activated: false },
            }),
        }),
    })
}
pub fn unit_from_parsed_target(conf: ParsedTargetConfig) -> Result<Unit, String> {
    Ok(Unit {
        id: UnitId {
            kind: UnitIdKind::Target,
            name: conf.common.name,
        },
        common: make_common_from_parsed(conf.common.unit, conf.common.install)?,
        specific: Specific::Target(TargetSpecific {
            state: RwLock::new(TargetState {
                common: CommonState::default(),
            }),
        }),
    })
}

impl From<ParsedSingleSocketConfig> for SingleSocketConfig {
    fn from(parsed: ParsedSingleSocketConfig) -> Self {
        Self {
            kind: parsed.kind,
            specialized: parsed.specialized,
        }
    }
}

impl std::convert::TryFrom<ParsedExecSection> for ExecConfig {
    type Error = String;
    fn try_from(parsed: ParsedExecSection) -> Result<Self, String> {
        // Store raw user/group strings and defer resolution to exec time.
        // This matches systemd's behavior: users/groups that don't exist yet
        // at unit-load time (e.g. created by systemd-sysusers) are resolved
        // just before the service process is spawned.
        Ok(Self {
            user: parsed.user,
            group: parsed.group,
            supplementary_groups: parsed.supplementary_groups,
            stdin_option: parsed.stdin_option,
            stderr_path: parsed.stderr_path,
            stdout_path: parsed.stdout_path,
            environment: parsed.environment,
            environment_files: parsed.environment_files,
            working_directory: parsed.working_directory,
            state_directory: parsed.state_directory,
            runtime_directory: parsed.runtime_directory,
            runtime_directory_preserve: parsed.runtime_directory_preserve,
            tty_path: parsed.tty_path,
            tty_reset: parsed.tty_reset,
            tty_vhangup: parsed.tty_vhangup,
            tty_vt_disallocate: parsed.tty_vt_disallocate,
            ignore_sigpipe: parsed.ignore_sigpipe,
            utmp_identifier: parsed.utmp_identifier,
            utmp_mode: parsed.utmp_mode,
            import_credentials: parsed.import_credentials,
            unset_environment: parsed.unset_environment,
            oom_score_adjust: parsed.oom_score_adjust,
            log_extra_fields: parsed.log_extra_fields,
            dynamic_user: parsed.dynamic_user,
            system_call_filter: parsed.system_call_filter,
            protect_system: parsed.protect_system,
            restrict_namespaces: parsed.restrict_namespaces,
            restrict_realtime: parsed.restrict_realtime,
            restrict_address_families: parsed.restrict_address_families,
            system_call_error_number: parsed.system_call_error_number,
            no_new_privileges: parsed.no_new_privileges,
            protect_control_groups: parsed.protect_control_groups,
            protect_kernel_modules: parsed.protect_kernel_modules,
            restrict_suid_sgid: parsed.restrict_suid_sgid,
            protect_kernel_logs: parsed.protect_kernel_logs,
            protect_clock: parsed.protect_clock,
            capability_bounding_set: parsed.capability_bounding_set,
            protect_home: parsed.protect_home,
            protect_hostname: parsed.protect_hostname,
            system_call_architectures: parsed.system_call_architectures,
            read_write_paths: parsed.read_write_paths,
            memory_deny_write_execute: parsed.memory_deny_write_execute,
            lock_personality: parsed.lock_personality,
            protect_proc: parsed.protect_proc,
            private_tmp: parsed.private_tmp,
        })
    }
}

/// Convert a list of unit name strings into UnitIds, skipping any with
/// unsupported suffixes (e.g. .device, .path, .timer, .swap, .scope).
/// This matches systemd's behavior of silently ignoring unit types it
/// doesn't manage in dependency lists, rather than rejecting the
/// entire unit file.
fn collect_supported_unit_ids(names: Vec<String>) -> Vec<UnitId> {
    let mut ids = Vec::new();
    for name in names {
        match <&str as TryInto<UnitId>>::try_into(name.as_str()) {
            Ok(id) => ids.push(id),
            Err(_) => {
                warn!(
                    "Skipping unsupported unit type in dependency list: {}",
                    name
                );
            }
        }
    }
    ids
}

fn make_common_from_parsed(
    unit: ParsedUnitSection,
    install: ParsedInstallSection,
) -> Result<Common, String> {
    let mut wants = collect_supported_unit_ids(unit.wants);
    // Also= in [Install] is treated as a soft (wants) dependency
    wants.extend(collect_supported_unit_ids(install.also));
    let requires = collect_supported_unit_ids(unit.requires);
    let part_of = collect_supported_unit_ids(unit.part_of);
    let conflicts = collect_supported_unit_ids(unit.conflicts);
    let wanted_by = collect_supported_unit_ids(install.wanted_by);
    let required_by = collect_supported_unit_ids(install.required_by);
    let after = collect_supported_unit_ids(unit.after);
    let before = collect_supported_unit_ids(unit.before);

    let mut refs_by_name = Vec::new();
    refs_by_name.extend(wants.iter().cloned());
    refs_by_name.extend(wanted_by.iter().cloned());
    refs_by_name.extend(requires.iter().cloned());
    refs_by_name.extend(required_by.iter().cloned());
    refs_by_name.extend(part_of.iter().cloned());
    refs_by_name.extend(conflicts.iter().cloned());
    refs_by_name.extend(before.iter().cloned());
    refs_by_name.extend(after.iter().cloned());

    Ok(Common {
        status: RwLock::new(UnitStatus::NeverStarted),
        unit: UnitConfig {
            description: unit.description,
            documentation: unit.documentation,
            refs_by_name,
            default_dependencies: unit.default_dependencies,
            ignore_on_isolate: unit.ignore_on_isolate,
            conditions: unit.conditions,
            success_action: unit.success_action,
            failure_action: unit.failure_action,
            job_timeout_action: unit.job_timeout_action,
            job_timeout_sec: unit.job_timeout_sec,
            allow_isolate: unit.allow_isolate,
            aliases: install.alias,
            default_instance: install.default_instance,
        },
        dependencies: Dependencies {
            wants,
            wanted_by,
            requires,
            required_by,
            conflicts,
            conflicted_by: Vec::new(),
            before,
            after,
            part_of,
            part_of_by: Vec::new(),
        },
    })
}

impl std::convert::TryInto<UnitId> for &str {
    type Error = String;
    fn try_into(self) -> Result<UnitId, String> {
        if self.ends_with(".target") {
            Ok(UnitId {
                name: self.to_owned(),
                kind: UnitIdKind::Target,
            })
        } else if self.ends_with(".service") {
            Ok(UnitId {
                name: self.to_owned(),
                kind: UnitIdKind::Service,
            })
        } else if self.ends_with(".socket") {
            Ok(UnitId {
                name: self.to_owned(),
                kind: UnitIdKind::Socket,
            })
        } else if self.ends_with(".slice") {
            Ok(UnitId {
                name: self.to_owned(),
                kind: UnitIdKind::Slice,
            })
        } else if self.ends_with(".mount") {
            Ok(UnitId {
                name: self.to_owned(),
                kind: UnitIdKind::Mount,
            })
        } else {
            Err(format!(
                "{self} is not a valid unit name. The suffix is not supported."
            ))
        }
    }
}

impl std::convert::TryFrom<ParsedServiceConfig> for Unit {
    type Error = String;
    fn try_from(conf: ParsedServiceConfig) -> Result<Self, String> {
        unit_from_parsed_service(conf)
    }
}
impl std::convert::TryFrom<ParsedSocketConfig> for Unit {
    type Error = String;
    fn try_from(conf: ParsedSocketConfig) -> Result<Self, String> {
        unit_from_parsed_socket(conf)
    }
}
impl std::convert::TryFrom<ParsedTargetConfig> for Unit {
    type Error = String;
    fn try_from(conf: ParsedTargetConfig) -> Result<Self, String> {
        unit_from_parsed_target(conf)
    }
}
