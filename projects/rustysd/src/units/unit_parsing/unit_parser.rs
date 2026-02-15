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
            "SuccessAction/FailureAction".to_owned(),
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
    let success_action = section.remove("SUCCESSACTION");
    let failure_action = section.remove("FAILUREACTION");

    for key in section.keys() {
        warn!("Ignoring unsupported setting in [Unit] section: {key}");
    }

    let default_dependencies = default_dependencies
        .map(|x| string_to_bool(&x[0].1))
        .unwrap_or(true);

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

    Ok(ParsedUnitSection {
        description: description.map(|x| (x[0]).1.clone()).unwrap_or_default(),
        documentation: map_tuples_to_second(split_list_values(documentation.unwrap_or_default())),
        wants: map_tuples_to_second(split_list_values(wants.unwrap_or_default())),
        requires: map_tuples_to_second(split_list_values(requires.unwrap_or_default())),
        conflicts: map_tuples_to_second(split_list_values(conflicts.unwrap_or_default())),
        after: map_tuples_to_second(split_list_values(after.unwrap_or_default())),
        before: map_tuples_to_second(split_list_values(before.unwrap_or_default())),
        default_dependencies,
        conditions,
        success_action,
        failure_action,
    })
}

fn make_stdio_option(setting: &str) -> Result<StdIoOption, ParsingErrorReason> {
    match setting.to_lowercase().as_str() {
        "null" | "" => Ok(StdIoOption::Null),
        "inherit" => Ok(StdIoOption::Inherit),
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
    let tty_path = section.remove("TTYPATH");
    let tty_reset = section.remove("TTYRESET");
    let tty_vhangup = section.remove("TTYVHANGUP");
    let tty_vt_disallocate = section.remove("TTYVTDISALLOCATE");

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
        tty_path,
        tty_reset,
        tty_vhangup,
        tty_vt_disallocate,
    })
}

pub fn parse_install_section(
    mut section: ParsedSection,
) -> Result<ParsedInstallSection, ParsingErrorReason> {
    let wantedby = section.remove("WANTEDBY");
    let requiredby = section.remove("REQUIREDBY");
    let also = section.remove("ALSO");

    for key in section.keys() {
        warn!("Ignoring unsupported setting in [Install] section: {key}");
    }

    Ok(ParsedInstallSection {
        wanted_by: map_tuples_to_second(split_list_values(wantedby.unwrap_or_default())),
        required_by: map_tuples_to_second(split_list_values(requiredby.unwrap_or_default())),
        also: map_tuples_to_second(split_list_values(also.unwrap_or_default())),
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
