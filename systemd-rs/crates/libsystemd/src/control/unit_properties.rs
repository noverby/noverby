//! Extract unit properties in key=value format for `systemctl show`.
//!
//! Real systemd exposes hundreds of properties per unit via D-Bus.  We expose
//! the most commonly used subset, formatted exactly as `systemctl show` would
//! print them (one `Key=Value` per line, lists space-separated, booleans as
//! `yes`/`no`, timestamps as `n/a` when unavailable).

use crate::lock_ext::RwLockExt;
use crate::units::{
    Commandline, ExecConfig, KillMode, MountConfig, NotifyKind, ServiceConfig, ServiceRestart,
    ServiceType, SocketConfig, Specific, Timeout, Unit, UnitConfig, UnitStatus,
};

use std::collections::BTreeMap;

/// Collect all properties of a unit into an ordered map.
///
/// The returned `BTreeMap` keeps keys in alphabetical order, matching the
/// default output of `systemctl show` (which lists properties sorted).
pub fn collect_properties(unit: &Unit) -> BTreeMap<String, String> {
    let mut props = BTreeMap::new();

    // ── Identity ──────────────────────────────────────────────────────
    insert(&mut props, "Id", &unit.id.name);
    insert(&mut props, "Names", &unit.id.name);

    // ── [Unit] section ────────────────────────────────────────────────
    insert_unit_config(&mut props, &unit.common.unit);

    // ── Dependencies ──────────────────────────────────────────────────
    insert_dep_list(&mut props, "Wants", &unit.common.dependencies.wants);
    insert_dep_list(&mut props, "WantedBy", &unit.common.dependencies.wanted_by);
    insert_dep_list(&mut props, "Requires", &unit.common.dependencies.requires);
    insert_dep_list(
        &mut props,
        "RequiredBy",
        &unit.common.dependencies.required_by,
    );
    insert_dep_list(&mut props, "Conflicts", &unit.common.dependencies.conflicts);
    insert_dep_list(
        &mut props,
        "ConflictedBy",
        &unit.common.dependencies.conflicted_by,
    );
    insert_dep_list(&mut props, "Before", &unit.common.dependencies.before);
    insert_dep_list(&mut props, "After", &unit.common.dependencies.after);
    insert_dep_list(&mut props, "PartOf", &unit.common.dependencies.part_of);
    insert_dep_list(&mut props, "PartOfBy", &unit.common.dependencies.part_of_by);
    insert_dep_list(&mut props, "BindsTo", &unit.common.dependencies.binds_to);
    insert_dep_list(&mut props, "BoundBy", &unit.common.dependencies.bound_by);

    // ── Status ────────────────────────────────────────────────────────
    let status = unit.common.status.read_poisoned();
    insert_status(&mut props, &status);

    // ── Type-specific properties ──────────────────────────────────────
    match &unit.specific {
        Specific::Service(svc) => {
            insert(&mut props, "Type", &format_service_type(svc.conf.srcv_type));
            insert_service_config(&mut props, &svc.conf);
            insert_exec_config(&mut props, &svc.conf.exec_config);

            // MainPID — read from state if available
            let state = svc.state.read_poisoned();
            if let Some(pid) = state.srvc.pid {
                insert(&mut props, "MainPID", &pid.to_string());
            } else {
                insert(&mut props, "MainPID", "0");
            }

            insert(
                &mut props,
                "RestartCount",
                &state.common.restart_count.to_string(),
            );
        }
        Specific::Socket(sock) => {
            insert_socket_config(&mut props, &sock.conf);
            insert_exec_config(&mut props, &sock.conf.exec_config);
        }
        Specific::Target(_) => {
            // Targets have no type-specific properties beyond [Unit].
        }
        Specific::Slice(_) => {
            // Slices have no type-specific properties beyond [Unit].
        }
        Specific::Mount(mnt) => {
            insert_mount_config(&mut props, &mnt.conf);
        }
        Specific::Timer(tmr) => {
            // Timer-specific properties
            insert(&mut props, "Unit", &tmr.conf.unit);
            insert(
                &mut props,
                "Persistent",
                if tmr.conf.persistent { "yes" } else { "no" },
            );
            insert(
                &mut props,
                "RemainAfterElapse",
                if tmr.conf.remain_after_elapse {
                    "yes"
                } else {
                    "no"
                },
            );
            insert(
                &mut props,
                "WakeSystem",
                if tmr.conf.wake_system { "yes" } else { "no" },
            );
        }
    }

    // ── LoadState / UnitFileState (synthetic) ─────────────────────────
    insert(&mut props, "LoadState", "loaded");
    insert(&mut props, "UnitFileState", "enabled");

    props
}

/// Format properties as `Key=Value\n` lines, optionally filtered to a set of
/// property names.
pub fn format_properties(props: &BTreeMap<String, String>, filter: Option<&[String]>) -> String {
    let mut out = String::new();
    for (key, value) in props {
        let include = match filter {
            Some(keys) => keys.iter().any(|k| k.eq_ignore_ascii_case(key)),
            None => true,
        };
        if include {
            out.push_str(key);
            out.push('=');
            out.push_str(value);
            out.push('\n');
        }
    }
    out
}

/// Format properties as a JSON object (for the JSON-RPC transport).
pub fn properties_to_json(props: &BTreeMap<String, String>) -> serde_json::Value {
    let map: serde_json::Map<String, serde_json::Value> = props
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
        .collect();
    serde_json::Value::Object(map)
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn insert(props: &mut BTreeMap<String, String>, key: &str, value: &str) {
    props.insert(key.to_owned(), value.to_owned());
}

fn insert_bool(props: &mut BTreeMap<String, String>, key: &str, value: bool) {
    props.insert(
        key.to_owned(),
        if value {
            "yes".to_owned()
        } else {
            "no".to_owned()
        },
    );
}

fn insert_dep_list(props: &mut BTreeMap<String, String>, key: &str, ids: &[crate::units::UnitId]) {
    let value: String = ids
        .iter()
        .map(|id| id.name.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    props.insert(key.to_owned(), value);
}

fn insert_string_list(props: &mut BTreeMap<String, String>, key: &str, items: &[String]) {
    props.insert(key.to_owned(), items.join(" "));
}

fn insert_timeout(props: &mut BTreeMap<String, String>, key: &str, timeout: &Option<Timeout>) {
    let value = match timeout {
        Some(Timeout::Duration(d)) => format!("{}us", d.as_micros()),
        Some(Timeout::Infinity) => "infinity".to_owned(),
        None => "infinity".to_owned(),
    };
    props.insert(key.to_owned(), value);
}

fn insert_commandlines(props: &mut BTreeMap<String, String>, key: &str, cmds: &[Commandline]) {
    if cmds.is_empty() {
        props.insert(key.to_owned(), String::new());
        return;
    }
    // systemctl show formats multi-command ExecStart as
    //   { path=/bin/foo ; argv[]=/bin/foo arg1 ; ... }
    // We use a simpler but compatible format.
    let parts: Vec<String> = cmds
        .iter()
        .map(|cmd| {
            let mut s = String::new();
            s.push_str("{ path=");
            s.push_str(&cmd.cmd);
            s.push_str(" ; argv[]=");
            s.push_str(&cmd.cmd);
            for arg in &cmd.args {
                s.push(' ');
                s.push_str(arg);
            }
            s.push_str(" ; }");
            s
        })
        .collect();
    props.insert(key.to_owned(), parts.join(" ; "));
}

fn insert_optional(props: &mut BTreeMap<String, String>, key: &str, value: &Option<String>) {
    match value {
        Some(v) => props.insert(key.to_owned(), v.clone()),
        None => props.insert(key.to_owned(), String::new()),
    };
}

// ── Section inserters ────────────────────────────────────────────────────

fn insert_unit_config(props: &mut BTreeMap<String, String>, conf: &UnitConfig) {
    insert(props, "Description", &conf.description);
    insert_string_list(props, "Documentation", &conf.documentation);

    match &conf.fragment_path {
        Some(p) => insert(props, "FragmentPath", &p.display().to_string()),
        None => insert(props, "FragmentPath", ""),
    }

    insert_bool(props, "DefaultDependencies", conf.default_dependencies);
    insert_bool(props, "RefuseManualStart", conf.refuse_manual_start);
    insert_bool(props, "RefuseManualStop", conf.refuse_manual_stop);
    insert_bool(props, "IgnoreOnIsolate", conf.ignore_on_isolate);
    insert_bool(props, "AllowIsolate", conf.allow_isolate);

    insert(
        props,
        "SuccessAction",
        &format!("{:?}", conf.success_action),
    );
    insert(
        props,
        "FailureAction",
        &format!("{:?}", conf.failure_action),
    );

    insert_timeout(props, "JobTimeoutUSec", &conf.job_timeout_sec);
    insert(
        props,
        "JobTimeoutAction",
        &format!("{:?}", conf.job_timeout_action),
    );

    if !conf.on_failure.is_empty() {
        insert_string_list(props, "OnFailure", &conf.on_failure);
    } else {
        insert(props, "OnFailure", "");
    }

    insert_timeout(
        props,
        "StartLimitIntervalUSec",
        &conf.start_limit_interval_sec,
    );
    match conf.start_limit_burst {
        Some(n) => insert(props, "StartLimitBurst", &n.to_string()),
        None => insert(props, "StartLimitBurst", "5"),
    }
    insert(
        props,
        "StartLimitAction",
        &format!("{:?}", conf.start_limit_action),
    );

    if !conf.conditions.is_empty() {
        let cond_str: Vec<String> = conf.conditions.iter().map(|c| format!("{c:?}")).collect();
        insert(props, "Conditions", &cond_str.join(" "));
    }

    if !conf.assertions.is_empty() {
        let assert_str: Vec<String> = conf.assertions.iter().map(|a| format!("{a:?}")).collect();
        insert(props, "Asserts", &assert_str.join(" "));
    }
}

fn insert_status(props: &mut BTreeMap<String, String>, status: &UnitStatus) {
    let (active_state, sub_state) = match status {
        UnitStatus::NeverStarted => ("inactive", "dead"),
        UnitStatus::Starting => ("activating", "start"),
        UnitStatus::Stopping => ("deactivating", "stop"),
        UnitStatus::Restarting => ("activating", "auto-restart"),
        UnitStatus::Started(s) => {
            let sub = match s {
                crate::units::StatusStarted::Running => "running",
                crate::units::StatusStarted::WaitingForSocket => "waiting",
            };
            ("active", sub)
        }
        UnitStatus::Stopped(_, errors) => {
            if errors.is_empty() {
                ("inactive", "dead")
            } else {
                ("failed", "failed")
            }
        }
    };
    insert(props, "ActiveState", active_state);
    insert(props, "SubState", sub_state);
}

fn insert_service_config(props: &mut BTreeMap<String, String>, conf: &ServiceConfig) {
    // Restart policy
    insert(props, "Restart", &format_restart(&conf.restart));
    match &conf.restart_sec {
        Some(Timeout::Duration(d)) => insert(props, "RestartUSec", &format!("{}us", d.as_micros())),
        Some(Timeout::Infinity) => insert(props, "RestartUSec", "infinity"),
        None => insert(props, "RestartUSec", "100000us"),
    }

    // Exec lines
    match &conf.exec {
        Some(cmd) => insert_commandlines(props, "ExecStart", &[cmd.clone()]),
        None => insert_commandlines(props, "ExecStart", &[]),
    }
    insert_commandlines(props, "ExecStartPre", &conf.startpre);
    insert_commandlines(props, "ExecStartPost", &conf.startpost);
    insert_commandlines(props, "ExecReload", &conf.reload);
    insert_commandlines(props, "ExecStop", &conf.stop);
    insert_commandlines(props, "ExecStopPost", &conf.stoppost);

    // Kill mode / signal
    insert(props, "KillMode", &format_kill_mode(conf.kill_mode));

    // Notify access
    insert(
        props,
        "NotifyAccess",
        &format_notify_access(conf.notifyaccess),
    );

    // Timeouts
    insert_timeout(props, "TimeoutStartUSec", &conf.starttimeout);
    insert_timeout(props, "TimeoutStopUSec", &conf.stoptimeout);

    // PID file
    match &conf.pid_file {
        Some(p) => insert(props, "PIDFile", &p.display().to_string()),
        None => insert(props, "PIDFile", ""),
    }

    // Sockets
    if !conf.sockets.is_empty() {
        let sock_names: Vec<String> = conf.sockets.iter().map(|s| s.name.clone()).collect();
        insert(props, "Sockets", &sock_names.join(" "));
    }

    // Slice
    insert_optional(props, "Slice", &conf.slice);

    // Remain after exit
    insert_bool(props, "RemainAfterExit", conf.remain_after_exit);

    // D-Bus name
    insert_optional(props, "BusName", &conf.dbus_name);

    // Delegate
    insert_bool(
        props,
        "Delegate",
        !matches!(conf.delegate, crate::units::Delegate::No),
    );

    // Watchdog
    insert_timeout(props, "WatchdogUSec", &conf.watchdog_sec);

    // RuntimeMaxUSec
    insert_timeout(props, "RuntimeMaxUSec", &conf.runtime_max_sec);

    // TasksMax
    match &conf.tasks_max {
        Some(crate::units::TasksMax::Value(n)) => insert(props, "TasksMax", &n.to_string()),
        Some(crate::units::TasksMax::Infinity) => insert(props, "TasksMax", "infinity"),
        Some(crate::units::TasksMax::Percent(p)) => insert(props, "TasksMax", &format!("{p}%")),
        None => insert(props, "TasksMax", "infinity"),
    }
}

fn insert_exec_config(props: &mut BTreeMap<String, String>, conf: &ExecConfig) {
    insert_optional(props, "User", &conf.user);
    insert_optional(props, "Group", &conf.group);

    if !conf.supplementary_groups.is_empty() {
        insert(
            props,
            "SupplementaryGroups",
            &conf.supplementary_groups.join(" "),
        );
    }

    match &conf.working_directory {
        Some(p) => insert(props, "WorkingDirectory", &p.display().to_string()),
        None => insert(props, "WorkingDirectory", ""),
    }

    // Environment variables
    if let Some(env) = &conf.environment {
        if !env.vars.is_empty() {
            let env_strs: Vec<String> = env.vars.iter().map(|(k, v)| format!("{k}={v}")).collect();
            insert(props, "Environment", &env_strs.join(" "));
        }
    }

    // Environment files
    if !conf.environment_files.is_empty() {
        insert(
            props,
            "EnvironmentFile",
            &conf
                .environment_files
                .iter()
                .map(|(path, optional)| {
                    if *optional {
                        format!("-{}", path.display())
                    } else {
                        path.display().to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
        );
    }

    // Directories
    if !conf.state_directory.is_empty() {
        insert(props, "StateDirectory", &conf.state_directory.join(" "));
    }
    if !conf.logs_directory.is_empty() {
        insert(props, "LogsDirectory", &conf.logs_directory.join(" "));
    }
    if !conf.runtime_directory.is_empty() {
        insert(props, "RuntimeDirectory", &conf.runtime_directory.join(" "));
    }

    // Security settings
    insert_bool(props, "NoNewPrivileges", conf.no_new_privileges);
    insert_bool(props, "ProtectKernelModules", conf.protect_kernel_modules);
    insert_bool(props, "ProtectKernelTunables", conf.protect_kernel_tunables);
    insert_bool(props, "ProtectKernelLogs", conf.protect_kernel_logs);
    insert_bool(props, "ProtectControlGroups", conf.protect_control_groups);
    insert_bool(props, "ProtectClock", conf.protect_clock);
    insert_bool(props, "ProtectHostname", conf.protect_hostname);
    insert_bool(props, "PrivateTmp", conf.private_tmp);
    insert_bool(props, "PrivateDevices", conf.private_devices);
    insert_bool(props, "PrivateNetwork", conf.private_network);
    insert_bool(props, "PrivateUsers", conf.private_users);
    insert_bool(props, "PrivateMounts", conf.private_mounts);
    insert_bool(
        props,
        "MemoryDenyWriteExecute",
        conf.memory_deny_write_execute,
    );
    insert_bool(props, "LockPersonality", conf.lock_personality);
    insert_bool(props, "RestrictSUIDSGID", conf.restrict_suid_sgid);
    insert_bool(props, "RestrictRealtime", conf.restrict_realtime);
    insert_bool(props, "RemoveIPC", conf.remove_ipc);
    insert_bool(props, "DynamicUser", conf.dynamic_user);

    match &conf.protect_system {
        crate::units::ProtectSystem::No => insert(props, "ProtectSystem", "no"),
        crate::units::ProtectSystem::Yes => insert(props, "ProtectSystem", "yes"),
        crate::units::ProtectSystem::Full => insert(props, "ProtectSystem", "full"),
        crate::units::ProtectSystem::Strict => insert(props, "ProtectSystem", "strict"),
    }

    match &conf.protect_home {
        crate::units::ProtectHome::No => insert(props, "ProtectHome", "no"),
        crate::units::ProtectHome::Yes => insert(props, "ProtectHome", "yes"),
        crate::units::ProtectHome::ReadOnly => insert(props, "ProtectHome", "read-only"),
        crate::units::ProtectHome::Tmpfs => insert(props, "ProtectHome", "tmpfs"),
    }

    // OOMScoreAdjust
    if let Some(adj) = conf.oom_score_adjust {
        insert(props, "OOMScoreAdjust", &adj.to_string());
    }

    // Nice
    if let Some(nice) = conf.nice {
        insert(props, "Nice", &nice.to_string());
    }

    // UMask
    if let Some(umask) = conf.umask {
        insert(props, "UMask", &format!("{:04o}", umask));
    }

    // PAM name
    insert_optional(props, "PAMName", &conf.pam_name);

    // Credential directives
    if !conf.import_credentials.is_empty() {
        insert(
            props,
            "ImportCredential",
            &conf.import_credentials.join(" "),
        );
    }
    if !conf.load_credentials.is_empty() {
        let lc: Vec<String> = conf
            .load_credentials
            .iter()
            .map(|(id, path)| format!("{id}:{path}"))
            .collect();
        insert(props, "LoadCredential", &lc.join(" "));
    }
    if !conf.set_credentials.is_empty() {
        let sc: Vec<String> = conf
            .set_credentials
            .iter()
            .map(|(id, _data)| id.clone())
            .collect();
        insert(props, "SetCredential", &sc.join(" "));
    }

    // Capability sets
    if !conf.capability_bounding_set.is_empty() {
        insert(
            props,
            "CapabilityBoundingSet",
            &conf.capability_bounding_set.join(" "),
        );
    }
    if !conf.ambient_capabilities.is_empty() {
        insert(
            props,
            "AmbientCapabilities",
            &conf.ambient_capabilities.join(" "),
        );
    }

    // System call filter
    if !conf.system_call_filter.is_empty() {
        insert(
            props,
            "SystemCallFilter",
            &conf.system_call_filter.join(" "),
        );
    }
    if !conf.system_call_architectures.is_empty() {
        insert(
            props,
            "SystemCallArchitectures",
            &conf.system_call_architectures.join(" "),
        );
    }

    // Read-write paths
    if !conf.read_write_paths.is_empty() {
        insert(props, "ReadWritePaths", &conf.read_write_paths.join(" "));
    }

    // Restrict address families
    if !conf.restrict_address_families.is_empty() {
        insert(
            props,
            "RestrictAddressFamilies",
            &conf.restrict_address_families.join(" "),
        );
    }

    // IP address allow/deny (from ServiceConfig but stored in ExecConfig vicinity)
    // These are on ServiceConfig, not ExecConfig, so handled in insert_service_config.
}

fn insert_socket_config(props: &mut BTreeMap<String, String>, conf: &SocketConfig) {
    // List listen addresses
    let mut listen_items = Vec::new();
    for sock in &conf.sockets {
        let desc = match &sock.specialized {
            crate::sockets::SpecializedSocketConfig::UnixSocket(unix) => {
                let addr = match unix {
                    crate::sockets::UnixSocketConfig::Stream(s) => s,
                    crate::sockets::UnixSocketConfig::Sequential(s) => s,
                    crate::sockets::UnixSocketConfig::Datagram(s) => s,
                };
                format!("unix:{addr}")
            }
            crate::sockets::SpecializedSocketConfig::Fifo(fifo) => {
                format!("fifo:{}", fifo.path.display())
            }
            crate::sockets::SpecializedSocketConfig::TcpSocket(tcp) => {
                format!("tcp:{}", tcp.addr)
            }
            crate::sockets::SpecializedSocketConfig::UdpSocket(udp) => {
                format!("udp:{}", udp.addr)
            }
            crate::sockets::SpecializedSocketConfig::NetlinkSocket(nl) => {
                format!("netlink:{}", nl.family)
            }
            crate::sockets::SpecializedSocketConfig::SpecialFile(sp) => {
                format!("special:{}", sp.path.display())
            }
        };
        listen_items.push(desc);
    }
    if !listen_items.is_empty() {
        insert(props, "Listen", &listen_items.join(" "));
    }

    // Services triggered by this socket
    let srvc_names: Vec<String> = conf.services.iter().map(|s| s.name.clone()).collect();
    if !srvc_names.is_empty() {
        insert(props, "TriggerService", &srvc_names.join(" "));
    }

    insert_bool(props, "Accept", conf.accept);
    insert(props, "MaxConnections", &conf.max_connections.to_string());
    insert(
        props,
        "MaxConnectionsPerSource",
        &conf.max_connections_per_source.to_string(),
    );
    match conf.socket_mode {
        Some(mode) => insert(props, "SocketMode", &format!("{mode:04o}")),
        None => insert(props, "SocketMode", "0666"),
    }
    match conf.directory_mode {
        Some(mode) => insert(props, "DirectoryMode", &format!("{mode:04o}")),
        None => insert(props, "DirectoryMode", "0755"),
    }
    insert_bool(props, "PassCredentials", conf.pass_credentials);
    insert_bool(props, "PassSecurity", conf.pass_security);
    insert_bool(props, "RemoveOnStop", conf.remove_on_stop);
    insert_bool(props, "Writable", conf.writable);
    if !conf.filedesc_name.is_empty() {
        insert(props, "FileDescriptorName", &conf.filedesc_name);
    }
}

fn insert_mount_config(props: &mut BTreeMap<String, String>, conf: &MountConfig) {
    insert(props, "What", &conf.what);
    insert(props, "Where", &conf.where_);
    insert_optional(props, "Type", &conf.fs_type);
    insert_optional(props, "Options", &conf.options);
    insert_bool(props, "SloppyOptions", conf.sloppy_options);
    insert_bool(props, "LazyUnmount", conf.lazy_unmount);
    insert_bool(props, "ReadWriteOnly", conf.read_write_only);
    insert_bool(props, "ForceUnmount", conf.force_unmount);
    insert(
        props,
        "DirectoryMode",
        &format!("{:04o}", conf.directory_mode),
    );
    match conf.timeout_sec {
        Some(secs) => insert(props, "TimeoutUSec", &format!("{}us", secs * 1_000_000)),
        None => insert(props, "TimeoutUSec", "infinity"),
    }
}

// ── Enum formatters ──────────────────────────────────────────────────────

fn format_service_type(t: ServiceType) -> String {
    match t {
        ServiceType::Simple => "simple",
        ServiceType::Exec => "exec",
        ServiceType::Forking => "forking",
        ServiceType::OneShot => "oneshot",
        ServiceType::Dbus => "dbus",
        ServiceType::Notify => "notify",
        ServiceType::NotifyReload => "notify-reload",
        ServiceType::Idle => "idle",
    }
    .to_owned()
}

fn format_restart(r: &ServiceRestart) -> String {
    match r {
        ServiceRestart::Always => "always",
        ServiceRestart::No => "no",
        ServiceRestart::OnFailure => "on-failure",
        ServiceRestart::OnSuccess => "on-success",
        ServiceRestart::OnWatchdog => "on-watchdog",
        ServiceRestart::OnAbnormal => "on-abnormal",
        ServiceRestart::OnAbort => "on-abort",
    }
    .to_owned()
}

fn format_kill_mode(m: KillMode) -> String {
    match m {
        KillMode::ControlGroup => "control-group",
        KillMode::Process => "process",
        KillMode::Mixed => "mixed",
        KillMode::None => "none",
    }
    .to_owned()
}

fn format_notify_access(n: NotifyKind) -> String {
    match n {
        NotifyKind::None => "none",
        NotifyKind::Main => "main",
        NotifyKind::All => "all",
        NotifyKind::Exec => "exec",
    }
    .to_owned()
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_properties_all() {
        let mut props = BTreeMap::new();
        props.insert("ActiveState".to_owned(), "active".to_owned());
        props.insert("Description".to_owned(), "Test Unit".to_owned());
        props.insert("Id".to_owned(), "test.service".to_owned());

        let output = format_properties(&props, None);
        assert!(output.contains("ActiveState=active\n"));
        assert!(output.contains("Description=Test Unit\n"));
        assert!(output.contains("Id=test.service\n"));
    }

    #[test]
    fn test_format_properties_filtered() {
        let mut props = BTreeMap::new();
        props.insert("ActiveState".to_owned(), "active".to_owned());
        props.insert("Description".to_owned(), "Test Unit".to_owned());
        props.insert("Id".to_owned(), "test.service".to_owned());

        let filter = vec!["Id".to_owned(), "ActiveState".to_owned()];
        let output = format_properties(&props, Some(&filter));
        assert!(output.contains("ActiveState=active\n"));
        assert!(output.contains("Id=test.service\n"));
        assert!(!output.contains("Description="));
    }

    #[test]
    fn test_format_properties_filter_case_insensitive() {
        let mut props = BTreeMap::new();
        props.insert("ActiveState".to_owned(), "active".to_owned());

        let filter = vec!["activestate".to_owned()];
        let output = format_properties(&props, Some(&filter));
        assert!(output.contains("ActiveState=active\n"));
    }

    #[test]
    fn test_format_properties_empty_filter() {
        let mut props = BTreeMap::new();
        props.insert("Id".to_owned(), "test.service".to_owned());

        let filter: Vec<String> = vec![];
        let output = format_properties(&props, Some(&filter));
        assert!(output.is_empty());
    }

    #[test]
    fn test_format_service_type() {
        assert_eq!(format_service_type(ServiceType::Simple), "simple");
        assert_eq!(format_service_type(ServiceType::Forking), "forking");
        assert_eq!(format_service_type(ServiceType::OneShot), "oneshot");
        assert_eq!(format_service_type(ServiceType::Notify), "notify");
        assert_eq!(format_service_type(ServiceType::Dbus), "dbus");
        assert_eq!(format_service_type(ServiceType::Idle), "idle");
        assert_eq!(format_service_type(ServiceType::Exec), "exec");
        assert_eq!(
            format_service_type(ServiceType::NotifyReload),
            "notify-reload"
        );
    }

    #[test]
    fn test_format_restart() {
        assert_eq!(format_restart(&ServiceRestart::Always), "always");
        assert_eq!(format_restart(&ServiceRestart::No), "no");
        assert_eq!(format_restart(&ServiceRestart::OnFailure), "on-failure");
    }

    #[test]
    fn test_format_kill_mode() {
        assert_eq!(format_kill_mode(KillMode::ControlGroup), "control-group");
        assert_eq!(format_kill_mode(KillMode::Process), "process");
        assert_eq!(format_kill_mode(KillMode::Mixed), "mixed");
        assert_eq!(format_kill_mode(KillMode::None), "none");
    }

    #[test]
    fn test_format_notify_access() {
        assert_eq!(format_notify_access(NotifyKind::None), "none");
        assert_eq!(format_notify_access(NotifyKind::Main), "main");
        assert_eq!(format_notify_access(NotifyKind::All), "all");
        assert_eq!(format_notify_access(NotifyKind::Exec), "exec");
    }

    #[test]
    fn test_insert_bool() {
        let mut props = BTreeMap::new();
        insert_bool(&mut props, "TestYes", true);
        insert_bool(&mut props, "TestNo", false);
        assert_eq!(props.get("TestYes").unwrap(), "yes");
        assert_eq!(props.get("TestNo").unwrap(), "no");
    }

    #[test]
    fn test_insert_timeout_duration() {
        let mut props = BTreeMap::new();
        insert_timeout(
            &mut props,
            "TestTimeout",
            &Some(Timeout::Duration(std::time::Duration::from_secs(30))),
        );
        assert_eq!(props.get("TestTimeout").unwrap(), "30000000us");
    }

    #[test]
    fn test_insert_timeout_infinity() {
        let mut props = BTreeMap::new();
        insert_timeout(&mut props, "TestTimeout", &Some(Timeout::Infinity));
        assert_eq!(props.get("TestTimeout").unwrap(), "infinity");
    }

    #[test]
    fn test_insert_timeout_none() {
        let mut props = BTreeMap::new();
        insert_timeout(&mut props, "TestTimeout", &None);
        assert_eq!(props.get("TestTimeout").unwrap(), "infinity");
    }

    #[test]
    fn test_insert_dep_list() {
        use crate::units::{UnitId, UnitIdKind};
        let mut props = BTreeMap::new();
        let deps = vec![
            UnitId {
                kind: UnitIdKind::Target,
                name: "multi-user.target".to_owned(),
            },
            UnitId {
                kind: UnitIdKind::Service,
                name: "sshd.service".to_owned(),
            },
        ];
        insert_dep_list(&mut props, "After", &deps);
        assert_eq!(
            props.get("After").unwrap(),
            "multi-user.target sshd.service"
        );
    }

    #[test]
    fn test_insert_dep_list_empty() {
        let mut props = BTreeMap::new();
        insert_dep_list(&mut props, "Before", &[]);
        assert_eq!(props.get("Before").unwrap(), "");
    }

    #[test]
    fn test_properties_to_json() {
        let mut props = BTreeMap::new();
        props.insert("Id".to_owned(), "test.service".to_owned());
        props.insert("ActiveState".to_owned(), "active".to_owned());

        let json = properties_to_json(&props);
        assert_eq!(json["Id"], "test.service");
        assert_eq!(json["ActiveState"], "active");
    }

    #[test]
    fn test_insert_commandlines_empty() {
        let mut props = BTreeMap::new();
        insert_commandlines(&mut props, "ExecStart", &[]);
        assert_eq!(props.get("ExecStart").unwrap(), "");
    }

    #[test]
    fn test_insert_commandlines_single() {
        let mut props = BTreeMap::new();
        let cmds = vec![Commandline {
            cmd: "/usr/bin/foo".to_owned(),
            args: vec!["--bar".to_owned(), "baz".to_owned()],
            prefixes: vec![],
        }];
        insert_commandlines(&mut props, "ExecStart", &cmds);
        let val = props.get("ExecStart").unwrap();
        assert!(val.contains("path=/usr/bin/foo"));
        assert!(val.contains("--bar baz"));
    }

    #[test]
    fn test_insert_optional_some() {
        let mut props = BTreeMap::new();
        insert_optional(&mut props, "PIDFile", &Some("/run/foo.pid".to_owned()));
        assert_eq!(props.get("PIDFile").unwrap(), "/run/foo.pid");
    }

    #[test]
    fn test_insert_optional_none() {
        let mut props = BTreeMap::new();
        insert_optional(&mut props, "PIDFile", &None);
        assert_eq!(props.get("PIDFile").unwrap(), "");
    }

    #[test]
    fn test_format_properties_ordering() {
        let mut props = BTreeMap::new();
        props.insert("Zebra".to_owned(), "z".to_owned());
        props.insert("Alpha".to_owned(), "a".to_owned());
        props.insert("Middle".to_owned(), "m".to_owned());

        let output = format_properties(&props, None);
        let lines: Vec<&str> = output.trim().split('\n').collect();
        assert_eq!(lines[0], "Alpha=a");
        assert_eq!(lines[1], "Middle=m");
        assert_eq!(lines[2], "Zebra=z");
    }
}
