//! Parse all supported unit types / options for these and do needed operations like matching services <-> sockets and adding implicit dependencies like
//! all sockets to socket.target

use log::{debug, warn};

use crate::units::{
    EnvVars, ParsedExecSection, ParsedInstallSection, ParsedUnitSection, ParsingErrorReason,
    StdIoOption, UnitAction,
};
use std::collections::HashMap;
use std::path::PathBuf;

pub type ParsedSection = HashMap<String, Vec<(u32, String)>>;
pub type ParsedFile = HashMap<String, ParsedSection>;

pub fn parse_file(content: &str) -> Result<ParsedFile, ParsingErrorReason> {
    let mut sections = HashMap::new();
    let lines: Vec<&str> = content.split('\n').collect();
    let lines: Vec<_> = lines.iter().map(|s| s.trim()).collect();

    let mut lines_left = &lines[..];

    // remove lines before the first section
    while !lines_left.is_empty() && !lines_left[0].starts_with('[') {
        lines_left = &lines_left[1..];
    }
    if lines_left.is_empty() {
        return Ok(sections);
    }
    let mut current_section_name: String = lines_left[0].into();
    let mut current_section_lines = Vec::new();

    lines_left = &lines_left[1..];

    while !lines_left.is_empty() {
        let line = lines_left[0];

        if line.starts_with('[') {
            if sections.contains_key(&current_section_name) {
                return Err(ParsingErrorReason::SectionTooOften(current_section_name));
            }
            sections.insert(
                current_section_name.clone(),
                parse_section(&current_section_lines),
            );
            current_section_name = line.into();
            current_section_lines.clear();
        } else {
            current_section_lines.push(line);
        }
        lines_left = &lines_left[1..];
    }

    // insert last section
    if let std::collections::hash_map::Entry::Vacant(e) =
        sections.entry(current_section_name.clone())
    {
        e.insert(parse_section(&current_section_lines));
    } else {
        return Err(ParsingErrorReason::SectionTooOften(current_section_name));
    }

    Ok(sections)
}

#[must_use]
pub fn map_tuples_to_second<X, Y: Clone>(v: Vec<(X, Y)>) -> Vec<Y> {
    v.iter().map(|(_, scnd)| scnd.clone()).collect()
}

/// Split comma-separated and space-separated values in list-type fields into
/// individual entries. In systemd, dependency fields like After=, Before=,
/// Wants=, Requires=, Conflicts= accept both comma-separated and
/// space-separated values. This function handles both kinds of splitting for
/// list-type fields. It must NOT be used for command-line fields (ExecStart=,
/// etc.) where commas can be part of arguments.
pub(crate) fn split_list_values(tuples: Vec<(u32, String)>) -> Vec<(u32, String)> {
    let mut result = Vec::new();
    for (idx, value) in tuples {
        for part in value.split(|c: char| c == ',' || c.is_whitespace()) {
            let part = part.trim();
            if !part.is_empty() {
                result.push((idx, part.to_string()));
            }
        }
    }
    result
}

#[must_use]
pub fn string_to_bool(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let s_upper = &s.to_uppercase();
    let c: char = s_upper.chars().next().unwrap();

    let is_num_and_one = s.len() == 1 && c == '1';
    *s_upper == *"YES" || *s_upper == *"TRUE" || is_num_and_one
}

fn parse_environment(raw_line: &str) -> Result<EnvVars, ParsingErrorReason> {
    debug!("raw line: {raw_line}");
    let split = shlex::split(raw_line).ok_or(ParsingErrorReason::Generic(format!(
        "Could not parse cmdline: {raw_line}"
    )))?;
    debug!("split: {split:?}");
    let mut vars: Vec<(String, String)> = Vec::new();

    for pair in split {
        let p: Vec<&str> = pair.splitn(2, '=').collect();
        let key = p[0].to_owned();
        let val = if p.len() > 1 { p[1] } else { "" };
        vars.push((key, val.to_owned()));
    }

    Ok(EnvVars { vars })
}

/// Convert an absolute path to a systemd-style mount unit name.
///
/// Examples:
/// - `/` → `-.mount`
/// - `/var` → `var.mount`
/// - `/var/log` → `var-log.mount`
#[allow(dead_code)]
pub(crate) fn path_to_mount_unit_name(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        "-.mount".to_owned()
    } else {
        format!("{}.mount", trimmed.replace('/', "-"))
    }
}

/// Return mount unit names for every prefix of the given absolute path,
/// from `/` down to the path itself.
///
/// For `/var/log/myapp` this returns:
/// `["-.mount", "var.mount", "var-log.mount", "var-log-myapp.mount"]`
pub(crate) fn mount_units_for_path(path: &str) -> Vec<String> {
    let mut units = vec!["-.mount".to_owned()];
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return units;
    }
    let mut accumulated = String::new();
    for component in trimmed.split('/') {
        if !accumulated.is_empty() {
            accumulated.push('-');
        }
        accumulated.push_str(component);
        let name = format!("{accumulated}.mount");
        if !units.contains(&name) {
            units.push(name);
        }
    }
    units
}

fn parse_unit_action(value: &str) -> Result<UnitAction, ParsingErrorReason> {
    match value.to_lowercase().replace('-', "").as_str() {
        "none" => Ok(UnitAction::None),
        "exit" => Ok(UnitAction::Exit),
        "exitforce" => Ok(UnitAction::ExitForce),
        "reboot" => Ok(UnitAction::Reboot),
        "rebootforce" => Ok(UnitAction::RebootForce),
        "rebootimmediate" => Ok(UnitAction::RebootImmediate),
        "poweroff" => Ok(UnitAction::Poweroff),
        "poweroffforce" => Ok(UnitAction::PoweroffForce),
        "poweroffimmediate" => Ok(UnitAction::PoweroffImmediate),
        "halt" => Ok(UnitAction::Halt),
        "haltforce" => Ok(UnitAction::HaltForce),
        "haltimmediate" => Ok(UnitAction::HaltImmediate),
        "kexec" => Ok(UnitAction::Kexec),
        "kexecforce" => Ok(UnitAction::KexecForce),
        "kexecimmediate" => Ok(UnitAction::KexecImmediate),
        other => Err(ParsingErrorReason::UnknownSetting(
            "SuccessAction/FailureAction/JobTimeoutAction".to_owned(),
            other.to_owned(),
        )),
    }
}

pub fn parse_unit_section(
    mut section: ParsedSection,
) -> Result<ParsedUnitSection, ParsingErrorReason> {
    let wants = section.remove("WANTS");
    let requires = section.remove("REQUIRES");
    let conflicts = section.remove("CONFLICTS");
    let after = section.remove("AFTER");
    let before = section.remove("BEFORE");
    let description = section.remove("DESCRIPTION");
    let documentation = section.remove("DOCUMENTATION");
    let default_dependencies = section.remove("DEFAULTDEPENDENCIES");
    let condition_path_exists = section.remove("CONDITIONPATHEXISTS");
    let condition_path_is_directory = section.remove("CONDITIONPATHISDIRECTORY");
    let condition_virtualization = section.remove("CONDITIONVIRTUALIZATION");
    let condition_capability = section.remove("CONDITIONCAPABILITY");
    let success_action = section.remove("SUCCESSACTION");
    let failure_action = section.remove("FAILUREACTION");
    let part_of = section.remove("PARTOF");
    let ignore_on_isolate = section.remove("IGNOREONISOLATE");
    let requires_mounts_for = section.remove("REQUIRESMOUNTSFOR");
    let stop_when_unneeded = section.remove("STOPWHENUNNEEDED");
    let allow_isolate = section.remove("ALLOWISOLATE");
    let job_timeout_sec = section.remove("JOBTIMEOUTSEC");
    let job_timeout_action = section.remove("JOBTIMEOUTACTION");

    for key in section.keys() {
        warn!("Ignoring unsupported setting in [Unit] section: {key}");
    }

    // Parse RequiresMountsFor= paths and generate implicit mount unit deps
    let requires_mounts_for_paths: Vec<String> =
        map_tuples_to_second(split_list_values(requires_mounts_for.unwrap_or_default()));

    let mut mount_unit_requires: Vec<String> = Vec::new();
    let mut mount_unit_after: Vec<String> = Vec::new();
    for path in &requires_mounts_for_paths {
        for unit_name in mount_units_for_path(path) {
            if !mount_unit_requires.contains(&unit_name) {
                mount_unit_requires.push(unit_name.clone());
            }
            if !mount_unit_after.contains(&unit_name) {
                mount_unit_after.push(unit_name);
            }
        }
    }

    let default_dependencies = default_dependencies
        .map(|x| string_to_bool(&x[0].1))
        .unwrap_or(true);

    let ignore_on_isolate = ignore_on_isolate
        .map(|x| string_to_bool(&x[0].1))
        .unwrap_or(false);

    let stop_when_unneeded = stop_when_unneeded
        .map(|x| string_to_bool(&x[0].1))
        .unwrap_or(false);

    let allow_isolate = allow_isolate
        .map(|x| string_to_bool(&x[0].1))
        .unwrap_or(false);

    let mut conditions = Vec::new();
    for (_, value) in condition_path_exists.unwrap_or_default() {
        let (path, negate) = if let Some(stripped) = value.strip_prefix('!') {
            (stripped.to_string(), true)
        } else {
            (value, false)
        };
        conditions.push(super::UnitCondition::PathExists { path, negate });
    }
    for (_, value) in condition_path_is_directory.unwrap_or_default() {
        let (path, negate) = if let Some(stripped) = value.strip_prefix('!') {
            (stripped.to_string(), true)
        } else {
            (value, false)
        };
        conditions.push(super::UnitCondition::PathIsDirectory { path, negate });
    }
    for (_, raw) in condition_virtualization.unwrap_or_default() {
        let trimmed = raw.trim();
        let (value, negate) = if let Some(stripped) = trimmed.strip_prefix('!') {
            (stripped.to_lowercase(), true)
        } else {
            (trimmed.to_lowercase(), false)
        };
        if !value.is_empty() {
            conditions.push(super::UnitCondition::Virtualization { value, negate });
        }
    }
    for (_, raw) in condition_capability.unwrap_or_default() {
        let trimmed = raw.trim();
        let (capability, negate) = if let Some(stripped) = trimmed.strip_prefix('!') {
            (stripped.trim().to_owned(), true)
        } else {
            (trimmed.to_owned(), false)
        };
        if !capability.is_empty() {
            conditions.push(super::UnitCondition::Capability { capability, negate });
        }
    }

    let success_action = match success_action {
        Some(vec) => {
            if vec.len() == 1 {
                parse_unit_action(&vec[0].1)?
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "SuccessAction".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => UnitAction::default(),
    };

    let failure_action = match failure_action {
        Some(vec) => {
            if vec.len() == 1 {
                parse_unit_action(&vec[0].1)?
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "FailureAction".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => UnitAction::default(),
    };

    let job_timeout_sec = match job_timeout_sec {
        Some(vec) => {
            if vec.len() == 1 {
                Some(super::service_unit::parse_timeout(&vec[0].1))
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "JobTimeoutSec".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => None,
    };

    let job_timeout_action = match job_timeout_action {
        Some(vec) => {
            if vec.len() == 1 {
                parse_unit_action(&vec[0].1)?
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "JobTimeoutAction".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => UnitAction::default(),
    };

    // Merge explicit deps with implicit mount deps from RequiresMountsFor=
    let mut requires_list = map_tuples_to_second(split_list_values(requires.unwrap_or_default()));
    for name in mount_unit_requires {
        if !requires_list.contains(&name) {
            requires_list.push(name);
        }
    }

    let mut after_list = map_tuples_to_second(split_list_values(after.unwrap_or_default()));
    for name in mount_unit_after {
        if !after_list.contains(&name) {
            after_list.push(name);
        }
    }

    Ok(ParsedUnitSection {
        description: description.map(|x| (x[0]).1.clone()).unwrap_or_default(),
        documentation: map_tuples_to_second(split_list_values(documentation.unwrap_or_default())),
        wants: map_tuples_to_second(split_list_values(wants.unwrap_or_default())),
        requires: requires_list,
        conflicts: map_tuples_to_second(split_list_values(conflicts.unwrap_or_default())),
        after: after_list,
        before: map_tuples_to_second(split_list_values(before.unwrap_or_default())),
        part_of: map_tuples_to_second(split_list_values(part_of.unwrap_or_default())),
        default_dependencies,
        ignore_on_isolate,
        conditions,
        success_action,
        failure_action,
        requires_mounts_for: requires_mounts_for_paths,
        stop_when_unneeded,
        allow_isolate,
        job_timeout_sec,
        job_timeout_action,
    })
}

fn make_stdio_option(setting: &str) -> Result<StdIoOption, ParsingErrorReason> {
    match setting.to_lowercase().as_str() {
        "null" | "" => Ok(StdIoOption::Null),
        "inherit" => Ok(StdIoOption::Inherit),
        "tty" => Ok(StdIoOption::Tty),
        "journal" | "syslog" | "journal+console" | "syslog+console" => Ok(StdIoOption::Journal),
        "kmsg" | "kmsg+console" => Ok(StdIoOption::Kmsg),
        _ if setting.starts_with("file:") => {
            let p = setting.trim_start_matches("file:");
            Ok(StdIoOption::File(p.into()))
        }
        _ if setting.starts_with("append:") => {
            let p = setting.trim_start_matches("append:");
            Ok(StdIoOption::AppendFile(p.into()))
        }
        _ => {
            warn!(
                "Unsupported StandardOutput/StandardError={}, treating as inherit",
                setting
            );
            Ok(StdIoOption::Inherit)
        }
    }
}

pub fn parse_exec_section(
    section: &mut ParsedSection,
) -> Result<ParsedExecSection, ParsingErrorReason> {
    let user = section.remove("USER");
    let group = section.remove("GROUP");
    let stdin = section.remove("STANDARDINPUT");
    let stdout = section.remove("STANDARDOUTPUT");
    let stderr = section.remove("STANDARDERROR");
    let supplementary_groups = section.remove("SUPPLEMENTARYGROUPS");
    let environment = section.remove("ENVIRONMENT");
    let environment_file = section.remove("ENVIRONMENTFILE");
    let working_directory = section.remove("WORKINGDIRECTORY");
    let state_directory = section.remove("STATEDIRECTORY");
    let runtime_directory = section.remove("RUNTIMEDIRECTORY");
    let runtime_directory_preserve = section.remove("RUNTIMEDIRECTORYPRESERVE");
    let tty_path = section.remove("TTYPATH");
    let tty_reset = section.remove("TTYRESET");
    let tty_vhangup = section.remove("TTYVHANGUP");
    let tty_vt_disallocate = section.remove("TTYVTDISALLOCATE");
    let ignore_sigpipe = section.remove("IGNORESIGPIPE");
    let utmp_identifier = section.remove("UTMPIDENTIFIER");
    let utmp_mode = section.remove("UTMPMODE");
    let import_credential = section.remove("IMPORTCREDENTIAL");
    let unset_environment = section.remove("UNSETENVIRONMENT");
    let oom_score_adjust = section.remove("OOMSCOREADJUST");
    let log_extra_fields = section.remove("LOGEXTRAFIELDS");
    let dynamic_user = section.remove("DYNAMICUSER");
    let system_call_filter = section.remove("SYSTEMCALLFILTER");
    let protect_system = section.remove("PROTECTSYSTEM");
    let restrict_namespaces = section.remove("RESTRICTNAMESPACES");
    let restrict_realtime = section.remove("RESTRICTREALTIME");
    let restrict_address_families = section.remove("RESTRICTADDRESSFAMILIES");
    let system_call_error_number = section.remove("SYSTEMCALLERRORNUMBER");
    let no_new_privileges = section.remove("NONEWPRIVILEGES");
    let protect_control_groups = section.remove("PROTECTCONTROLGROUPS");
    let protect_kernel_modules = section.remove("PROTECTKERNELMODULES");
    let restrict_suid_sgid = section.remove("RESTRICTSUIDSGID");
    let protect_kernel_logs = section.remove("PROTECTKERNELLOGS");
    let capability_bounding_set = section.remove("CAPABILITYBOUNDINGSET");
    let protect_clock = section.remove("PROTECTCLOCK");
    let protect_home = section.remove("PROTECTHOME");
    let protect_hostname = section.remove("PROTECTHOSTNAME");
    let system_call_architectures = section.remove("SYSTEMCALLARCHITECTURES");
    let read_write_paths = section.remove("READWRITEPATHS");
    let memory_deny_write_execute = section.remove("MEMORYDENYWRITEEXECUTE");
    let lock_personality = section.remove("LOCKPERSONALITY");
    let protect_proc = section.remove("PROTECTPROC");
    let private_tmp = section.remove("PRIVATETMP");

    let user = match user {
        None => None,
        Some(mut vec) => {
            if vec.len() == 1 {
                Some(vec.remove(0).1)
            } else if vec.len() > 1 {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "User".into(),
                    super::map_tuples_to_second(vec),
                ));
            } else {
                None
            }
        }
    };

    let group = match group {
        None => None,
        Some(mut vec) => {
            if vec.len() == 1 {
                Some(vec.remove(0).1)
            } else if vec.len() > 1 {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "Group".into(),
                    super::map_tuples_to_second(vec),
                ));
            } else {
                None
            }
        }
    };
    let stdin_option = match stdin {
        None => super::StandardInput::Null,
        Some(mut vec) => {
            if vec.len() == 1 {
                match vec.remove(0).1.to_lowercase().as_str() {
                    "null" | "" => super::StandardInput::Null,
                    "tty" => super::StandardInput::Tty,
                    "tty-force" => super::StandardInput::TtyForce,
                    "tty-fail" => super::StandardInput::TtyFail,
                    other => {
                        warn!("Unsupported StandardInput={}, falling back to null", other);
                        super::StandardInput::Null
                    }
                }
            } else if vec.len() > 1 {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "StandardInput".into(),
                    super::map_tuples_to_second(vec),
                ));
            } else {
                super::StandardInput::Null
            }
        }
    };

    let tty_path = match tty_path {
        None => None,
        Some(mut vec) => {
            if vec.len() == 1 {
                Some(std::path::PathBuf::from(vec.remove(0).1))
            } else if vec.len() > 1 {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "TTYPath".into(),
                    super::map_tuples_to_second(vec),
                ));
            } else {
                None
            }
        }
    };

    let tty_reset = match tty_reset {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "TTYReset".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => false,
    };

    let tty_vhangup = match tty_vhangup {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "TTYVHangup".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => false,
    };

    let tty_vt_disallocate = match tty_vt_disallocate {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "TTYVTDisallocate".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => false,
    };

    let ignore_sigpipe = match ignore_sigpipe {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "IgnoreSIGPIPE".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        // systemd default: true — SIGPIPE is ignored
        None => true,
    };

    let dynamic_user = match dynamic_user {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "DynamicUser".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        // systemd default: false
        None => false,
    };

    let restrict_realtime = match restrict_realtime {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "RestrictRealtime".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        // systemd default: false
        None => false,
    };

    let no_new_privileges = match no_new_privileges {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "NoNewPrivileges".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        // systemd default: false
        None => false,
    };

    let protect_kernel_modules = match protect_kernel_modules {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "ProtectKernelModules".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        // systemd default: false
        None => false,
    };

    let restrict_suid_sgid = match restrict_suid_sgid {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "RestrictSUIDSGID".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        // systemd default: false
        None => false,
    };

    let protect_kernel_logs = match protect_kernel_logs {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "ProtectKernelLogs".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        // systemd default: false
        None => false,
    };

    let protect_clock = match protect_clock {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "ProtectClock".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        // systemd default: false
        None => false,
    };

    let protect_hostname = match protect_hostname {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "ProtectHostname".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        // systemd default: false
        None => false,
    };

    let memory_deny_write_execute = match memory_deny_write_execute {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "MemoryDenyWriteExecute".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        // systemd default: false
        None => false,
    };

    let lock_personality = match lock_personality {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "LockPersonality".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        // systemd default: false
        None => false,
    };

    let private_tmp = match private_tmp {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "PrivateTmp".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        // systemd default: false
        None => false,
    };

    let protect_control_groups = match protect_control_groups {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "ProtectControlGroups".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        // systemd default: false
        None => false,
    };

    let stdout_path = match stdout {
        None => None,
        Some(mut vec) => {
            if vec.len() == 1 {
                Some(vec.remove(0).1)
            } else if vec.len() > 1 {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "Standardoutput".into(),
                    super::map_tuples_to_second(vec),
                ));
            } else {
                None
            }
        }
    };
    let stdout_path = if let Some(p) = stdout_path {
        Some(make_stdio_option(&p)?)
    } else {
        None
    };

    let stderr_path = match stderr {
        None => None,
        Some(mut vec) => {
            if vec.len() == 1 {
                Some(vec.remove(0).1)
            } else if vec.len() > 1 {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "Standarderror".into(),
                    super::map_tuples_to_second(vec),
                ));
            } else {
                None
            }
        }
    };
    let stderr_path = if let Some(p) = stderr_path {
        Some(make_stdio_option(&p)?)
    } else {
        None
    };

    let supplementary_groups = match supplementary_groups {
        None => Vec::new(),
        Some(vec) => vec.iter().fold(Vec::new(), |mut acc, (_id, list)| {
            acc.extend(list.split(' ').map(std::string::ToString::to_string));
            acc
        }),
    };

    let environment = match environment {
        Some(vec) => {
            debug!("Env vec: {vec:?}");
            let mut all_vars = Vec::new();
            for (_idx, line) in &vec {
                let parsed = parse_environment(line)?;
                all_vars.extend(parsed.vars);
            }
            Some(EnvVars { vars: all_vars })
        }
        None => None,
    };

    // Parse EnvironmentFile= directives. A leading '-' on the path means the
    // file is optional (no error if it doesn't exist). Multiple directives are
    // allowed and each can list one file.
    let environment_files: Vec<(PathBuf, bool)> = match environment_file {
        Some(vec) => vec
            .into_iter()
            .map(|(_, val)| {
                let val = val.trim().to_string();
                if let Some(stripped) = val.strip_prefix('-') {
                    (PathBuf::from(stripped), true)
                } else {
                    (PathBuf::from(val), false)
                }
            })
            .collect(),
        None => Vec::new(),
    };

    let working_directory = match working_directory {
        None => None,
        Some(mut vec) => {
            if vec.len() == 1 {
                let dir = vec.remove(0).1;
                // Strip leading '-' prefix (makes it non-fatal if directory doesn't exist)
                let dir = dir.strip_prefix('-').unwrap_or(&dir);
                if dir == "~" {
                    // Home directory of the user; resolved later when user is known
                    Some(std::path::PathBuf::from("~"))
                } else {
                    Some(std::path::PathBuf::from(dir))
                }
            } else if vec.len() > 1 {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "WorkingDirectory".into(),
                    super::map_tuples_to_second(vec),
                ));
            } else {
                None
            }
        }
    };

    let state_directory = match state_directory {
        None => Vec::new(),
        Some(vec) => vec
            .into_iter()
            .flat_map(|(_, val)| {
                val.split_whitespace()
                    .map(|s| s.to_owned())
                    .collect::<Vec<_>>()
            })
            .collect(),
    };

    let runtime_directory = match runtime_directory {
        None => Vec::new(),
        Some(vec) => vec
            .into_iter()
            .flat_map(|(_, val)| {
                val.split_whitespace()
                    .map(|s| s.to_owned())
                    .collect::<Vec<_>>()
            })
            .collect(),
    };

    Ok(ParsedExecSection {
        user,
        group,
        stdin_option,
        stdout_path,
        stderr_path,
        supplementary_groups,
        environment,
        environment_files,
        working_directory,
        state_directory,
        runtime_directory,
        runtime_directory_preserve: match runtime_directory_preserve {
            Some(vec) => {
                if vec.len() == 1 {
                    match vec[0].1.trim().to_lowercase().as_str() {
                        "no" | "false" | "0" => super::RuntimeDirectoryPreserve::No,
                        "yes" | "true" | "1" => super::RuntimeDirectoryPreserve::Yes,
                        "restart" => super::RuntimeDirectoryPreserve::Restart,
                        other => {
                            return Err(ParsingErrorReason::UnknownSetting(
                                "RuntimeDirectoryPreserve".to_owned(),
                                other.to_owned(),
                            ))
                        }
                    }
                } else {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "RuntimeDirectoryPreserve".to_owned(),
                        super::map_tuples_to_second(vec),
                    ));
                }
            }
            None => super::RuntimeDirectoryPreserve::default(),
        },
        oom_score_adjust: match oom_score_adjust {
            None => None,
            Some(vec) => {
                if vec.len() == 1 {
                    let val: i32 = vec[0].1.trim().parse().map_err(|_| {
                        ParsingErrorReason::UnknownSetting(
                            "OOMScoreAdjust".to_owned(),
                            vec[0].1.clone(),
                        )
                    })?;
                    Some(val.clamp(-1000, 1000))
                } else if vec.len() > 1 {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "OOMScoreAdjust".into(),
                        super::map_tuples_to_second(vec),
                    ));
                } else {
                    None
                }
            }
        },
        tty_path,
        tty_reset,
        tty_vhangup,
        tty_vt_disallocate,
        ignore_sigpipe,
        dynamic_user,
        utmp_identifier: match utmp_identifier {
            None => None,
            Some(mut vec) => {
                if vec.len() == 1 {
                    Some(vec.remove(0).1)
                } else if vec.len() > 1 {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "UtmpIdentifier".into(),
                        super::map_tuples_to_second(vec),
                    ));
                } else {
                    None
                }
            }
        },
        utmp_mode: match utmp_mode {
            Some(vec) => {
                if vec.len() == 1 {
                    match vec[0].1.to_lowercase().as_str() {
                        "init" => super::UtmpMode::Init,
                        "login" => super::UtmpMode::Login,
                        "user" => super::UtmpMode::User,
                        name => {
                            return Err(ParsingErrorReason::UnknownSetting(
                                "UtmpMode".to_owned(),
                                name.to_owned(),
                            ))
                        }
                    }
                } else {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "UtmpMode".to_owned(),
                        super::map_tuples_to_second(vec),
                    ));
                }
            }
            None => super::UtmpMode::default(),
        },
        import_credentials: match import_credential {
            Some(vec) => vec
                .into_iter()
                .flat_map(|(_, val)| {
                    val.split_whitespace()
                        .map(|s| s.to_owned())
                        .collect::<Vec<_>>()
                })
                .collect(),
            None => Vec::new(),
        },
        unset_environment: match unset_environment {
            Some(vec) => {
                // Each directive is a space-separated list of variable names
                // or VAR=VALUE assignments. Multiple directives accumulate.
                // Tokens may be optionally quoted with double quotes (same
                // rules as Environment=). If an empty string is assigned,
                // the list is reset.
                let mut entries = Vec::new();
                for (_idx, line) in &vec {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        // Empty string resets the list
                        entries.clear();
                        continue;
                    }
                    // Split on whitespace, respecting double-quoted tokens
                    let mut chars = trimmed.chars().peekable();
                    while chars.peek().is_some() {
                        // skip whitespace
                        while chars.peek().map_or(false, |c| c.is_whitespace()) {
                            chars.next();
                        }
                        if chars.peek().is_none() {
                            break;
                        }
                        let mut token = String::new();
                        if chars.peek() == Some(&'"') {
                            // quoted token — consume until closing quote
                            chars.next(); // skip opening quote
                            while let Some(&c) = chars.peek() {
                                if c == '"' {
                                    chars.next();
                                    break;
                                }
                                token.push(c);
                                chars.next();
                            }
                        } else {
                            // unquoted token — consume until whitespace
                            while let Some(&c) = chars.peek() {
                                if c.is_whitespace() {
                                    break;
                                }
                                token.push(c);
                                chars.next();
                            }
                        }
                        if !token.is_empty() {
                            entries.push(token);
                        }
                    }
                }
                entries
            }
            None => Vec::new(),
        },
        log_extra_fields: match log_extra_fields {
            Some(vec) => vec.into_iter().map(|(_, val)| val).collect(),
            None => Vec::new(),
        },
        protect_system: match protect_system {
            Some(vec) => {
                if vec.len() == 1 {
                    match vec[0].1.trim().to_lowercase().as_str() {
                        "no" | "false" | "0" => super::ProtectSystem::No,
                        "yes" | "true" | "1" => super::ProtectSystem::Yes,
                        "full" => super::ProtectSystem::Full,
                        "strict" => super::ProtectSystem::Strict,
                        other => {
                            return Err(ParsingErrorReason::UnknownSetting(
                                "ProtectSystem".to_owned(),
                                other.to_owned(),
                            ))
                        }
                    }
                } else {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "ProtectSystem".to_owned(),
                        super::map_tuples_to_second(vec),
                    ));
                }
            }
            None => super::ProtectSystem::default(),
        },
        restrict_namespaces: match restrict_namespaces {
            Some(vec) => {
                if vec.len() == 1 {
                    let raw = vec[0].1.trim();
                    match raw.to_lowercase().as_str() {
                        "yes" | "true" | "1" => super::RestrictNamespaces::Yes,
                        "no" | "false" | "0" | "" => super::RestrictNamespaces::No,
                        _ => {
                            // Space-separated list of namespace types, optionally
                            // prefixed with ~ for deny-list mode.
                            let trimmed = raw;
                            let (deny, rest) = if let Some(stripped) = trimmed.strip_prefix('~') {
                                (true, stripped.trim_start())
                            } else {
                                (false, trimmed)
                            };
                            let names: Vec<String> =
                                rest.split_whitespace().map(|s| s.to_lowercase()).collect();
                            if deny {
                                super::RestrictNamespaces::Deny(names)
                            } else {
                                super::RestrictNamespaces::Allow(names)
                            }
                        }
                    }
                } else {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "RestrictNamespaces".to_owned(),
                        super::map_tuples_to_second(vec),
                    ));
                }
            }
            None => super::RestrictNamespaces::default(),
        },
        restrict_realtime,
        restrict_address_families: match restrict_address_families {
            Some(vec) => {
                // Each directive is a space-separated list of address family
                // names (e.g. AF_UNIX, AF_INET, AF_INET6). A leading ~ on
                // the whole value means deny-list mode. Multiple directives
                // accumulate. An empty assignment resets the list.
                let mut entries = Vec::new();
                for (_idx, line) in &vec {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        // Empty string resets the list
                        entries.clear();
                        continue;
                    }
                    // Split on whitespace, respecting double-quoted tokens
                    let mut chars = trimmed.chars().peekable();
                    while chars.peek().is_some() {
                        // skip whitespace
                        while chars.peek().map_or(false, |c| c.is_whitespace()) {
                            chars.next();
                        }
                        if chars.peek().is_none() {
                            break;
                        }
                        let mut token = String::new();
                        if chars.peek() == Some(&'"') {
                            // quoted token — consume until closing quote
                            chars.next(); // skip opening quote
                            while let Some(&c) = chars.peek() {
                                if c == '"' {
                                    chars.next();
                                    break;
                                }
                                token.push(c);
                                chars.next();
                            }
                        } else {
                            // unquoted token — consume until whitespace
                            while let Some(&c) = chars.peek() {
                                if c.is_whitespace() {
                                    break;
                                }
                                token.push(c);
                                chars.next();
                            }
                        }
                        if !token.is_empty() {
                            entries.push(token);
                        }
                    }
                }
                entries
            }
            None => Vec::new(),
        },
        no_new_privileges,
        protect_control_groups,
        protect_kernel_modules,
        restrict_suid_sgid,
        protect_kernel_logs,
        protect_clock,
        capability_bounding_set: match capability_bounding_set {
            Some(vec) => {
                // Each directive is a space-separated list of capability names
                // (e.g. CAP_NET_ADMIN, CAP_SYS_PTRACE). A leading ~ on the
                // whole value means deny-list mode. Multiple directives
                // accumulate. An empty assignment resets the list.
                let mut entries = Vec::new();
                for (_idx, line) in &vec {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        // Empty string resets the list
                        entries.clear();
                        continue;
                    }
                    // Split on whitespace, respecting double-quoted tokens
                    let mut chars = trimmed.chars().peekable();
                    while chars.peek().is_some() {
                        // skip whitespace
                        while chars.peek().map_or(false, |c| c.is_whitespace()) {
                            chars.next();
                        }
                        if chars.peek().is_none() {
                            break;
                        }
                        let mut token = String::new();
                        if chars.peek() == Some(&'"') {
                            // quoted token — consume until closing quote
                            chars.next(); // skip opening quote
                            while let Some(&c) = chars.peek() {
                                if c == '"' {
                                    chars.next();
                                    break;
                                }
                                token.push(c);
                                chars.next();
                            }
                        } else {
                            // unquoted token — consume until whitespace
                            while let Some(&c) = chars.peek() {
                                if c.is_whitespace() {
                                    break;
                                }
                                token.push(c);
                                chars.next();
                            }
                        }
                        if !token.is_empty() {
                            entries.push(token);
                        }
                    }
                }
                entries
            }
            None => Vec::new(),
        },
        system_call_error_number: match system_call_error_number {
            None => None,
            Some(mut vec) => {
                if vec.len() == 1 {
                    let val = vec.remove(0).1;
                    let trimmed = val.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_owned())
                    }
                } else if vec.len() > 1 {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "SystemCallErrorNumber".into(),
                        super::map_tuples_to_second(vec),
                    ));
                } else {
                    None
                }
            }
        },
        system_call_filter: match system_call_filter {
            Some(vec) => {
                // Each directive is a space-separated list of syscall names
                // or @group names. A leading ~ on the whole value means
                // deny-list mode; without it, allow-list mode. Multiple
                // directives accumulate. An empty assignment resets the list.
                let mut entries = Vec::new();
                for (_idx, line) in &vec {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        // Empty string resets the list
                        entries.clear();
                        continue;
                    }
                    // Split on whitespace, respecting double-quoted tokens
                    let mut chars = trimmed.chars().peekable();
                    while chars.peek().is_some() {
                        // skip whitespace
                        while chars.peek().map_or(false, |c| c.is_whitespace()) {
                            chars.next();
                        }
                        if chars.peek().is_none() {
                            break;
                        }
                        let mut token = String::new();
                        if chars.peek() == Some(&'"') {
                            // quoted token — consume until closing quote
                            chars.next(); // skip opening quote
                            while let Some(&c) = chars.peek() {
                                if c == '"' {
                                    chars.next();
                                    break;
                                }
                                token.push(c);
                                chars.next();
                            }
                        } else {
                            // unquoted token — consume until whitespace
                            while let Some(&c) = chars.peek() {
                                if c.is_whitespace() {
                                    break;
                                }
                                token.push(c);
                                chars.next();
                            }
                        }
                        if !token.is_empty() {
                            entries.push(token);
                        }
                    }
                }
                entries
            }
            None => Vec::new(),
        },
        protect_home: match protect_home {
            Some(vec) => {
                if vec.len() == 1 {
                    match vec[0].1.trim().to_lowercase().as_str() {
                        "no" | "false" | "0" => super::ProtectHome::No,
                        "yes" | "true" | "1" => super::ProtectHome::Yes,
                        "read-only" => super::ProtectHome::ReadOnly,
                        "tmpfs" => super::ProtectHome::Tmpfs,
                        other => {
                            return Err(ParsingErrorReason::UnknownSetting(
                                "ProtectHome".to_owned(),
                                other.to_owned(),
                            ))
                        }
                    }
                } else {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "ProtectHome".to_owned(),
                        super::map_tuples_to_second(vec),
                    ));
                }
            }
            None => super::ProtectHome::default(),
        },
        protect_hostname,
        system_call_architectures: match system_call_architectures {
            Some(vec) => {
                let mut entries = Vec::new();
                for (_idx, line) in &vec {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        // Empty string resets the list
                        entries.clear();
                        continue;
                    }
                    for token in trimmed.split_whitespace() {
                        entries.push(token.to_owned());
                    }
                }
                entries
            }
            None => Vec::new(),
        },
        read_write_paths: match read_write_paths {
            Some(vec) => {
                let mut entries = Vec::new();
                for (_idx, line) in &vec {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        // Empty string resets the list
                        entries.clear();
                        continue;
                    }
                    for token in trimmed.split_whitespace() {
                        entries.push(token.to_owned());
                    }
                }
                entries
            }
            None => Vec::new(),
        },
        memory_deny_write_execute,
        lock_personality,
        protect_proc: match protect_proc {
            Some(vec) => {
                if vec.len() == 1 {
                    match vec[0].1.trim().to_lowercase().as_str() {
                        "default" | "" => super::ProtectProc::Default,
                        "noaccess" => super::ProtectProc::Noaccess,
                        "invisible" => super::ProtectProc::Invisible,
                        "ptraceable" => super::ProtectProc::Ptraceable,
                        other => {
                            return Err(ParsingErrorReason::UnknownSetting(
                                "ProtectProc".to_owned(),
                                other.to_owned(),
                            ))
                        }
                    }
                } else {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "ProtectProc".to_owned(),
                        super::map_tuples_to_second(vec),
                    ));
                }
            }
            None => super::ProtectProc::default(),
        },
        private_tmp,
    })
}

pub fn parse_install_section(
    mut section: ParsedSection,
) -> Result<ParsedInstallSection, ParsingErrorReason> {
    let wantedby = section.remove("WANTEDBY");
    let requiredby = section.remove("REQUIREDBY");
    let also = section.remove("ALSO");
    let alias = section.remove("ALIAS");
    let default_instance = section.remove("DEFAULTINSTANCE");

    for key in section.keys() {
        warn!("Ignoring unsupported setting in [Install] section: {key}");
    }

    Ok(ParsedInstallSection {
        wanted_by: map_tuples_to_second(split_list_values(wantedby.unwrap_or_default())),
        required_by: map_tuples_to_second(split_list_values(requiredby.unwrap_or_default())),
        also: map_tuples_to_second(split_list_values(also.unwrap_or_default())),
        alias: map_tuples_to_second(split_list_values(alias.unwrap_or_default())),
        default_instance: default_instance.map(|x| x[0].1.clone()),
    })
}

pub fn get_file_list(path: &PathBuf) -> Result<Vec<std::fs::DirEntry>, ParsingErrorReason> {
    if !path.exists() {
        return Err(ParsingErrorReason::Generic(format!(
            "Path to services does not exist: {path:?}"
        )));
    }
    if !path.is_dir() {
        return Err(ParsingErrorReason::Generic(format!(
            "Path to services does not exist: {path:?}"
        )));
    }
    let mut files: Vec<_> = match std::fs::read_dir(path) {
        Ok(iter) => iter
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ParsingErrorReason::FileError(Box::new(e)))?,
        Err(e) => return Err(ParsingErrorReason::FileError(Box::new(e))),
    };
    files.sort_by_key(std::fs::DirEntry::path);

    Ok(files)
}

#[must_use]
pub fn parse_section(lines: &[&str]) -> ParsedSection {
    let mut entries: ParsedSection = HashMap::new();

    let mut entry_number = 0;
    for line in lines {
        //ignore comments
        if line.starts_with('#') {
            continue;
        }

        //check if this is a key value pair
        let Some(pos) = line.find('=') else {
            continue;
        };
        let (name, value) = line.split_at(pos);

        let value = value.trim_start_matches('=');
        let value = value.trim();
        let name = name.trim().to_uppercase();

        let vec = entries.entry(name).or_default();
        vec.push((entry_number, value.to_string()));
        entry_number += 1;
    }

    entries
}
