use log::{trace, warn};

use crate::units::{
    map_tuples_to_second, parse_install_section, parse_unit_section, string_to_bool, Commandline,
    CommandlinePrefix, Delegate, KillMode, NotifyKind, ParsedCommonConfig, ParsedFile,
    ParsedSection, ParsedServiceConfig, ParsedServiceSection, ParsingErrorReason, RLimitValue,
    ResourceLimit, ServiceRestart, ServiceType, TasksMax, Timeout,
};
use std::collections::HashMap;
use std::path::PathBuf;

pub fn parse_service(
    parsed_file: ParsedFile,
    path: &PathBuf,
) -> Result<ParsedServiceConfig, ParsingErrorReason> {
    let mut service_config = None;
    let mut install_config = None;
    let mut unit_config = None;

    for (name, section) in parsed_file {
        match name.as_str() {
            "[Service]" => {
                service_config = Some(parse_service_section(section)?);
            }
            "[Unit]" => {
                unit_config = Some(parse_unit_section(section)?);
            }
            "[Install]" => {
                install_config = Some(parse_install_section(section)?);
            }

            _ => {
                warn!("Ignoring unknown section in service unit {path:?}: {name}");
            }
        }
    }

    // systemd allows .service files without a [Service] section (e.g.
    // systemd-reboot.service which only has a [Unit] with SuccessAction=).
    // Treat them as oneshot services with no ExecStart.
    let service_config = match service_config {
        Some(c) => c,
        None => {
            trace!("Service unit {path:?} has no [Service] section, treating as oneshot with no ExecStart");
            let empty_section: ParsedSection = HashMap::new();
            parse_service_section(empty_section)?
        }
    };

    Ok(ParsedServiceConfig {
        common: ParsedCommonConfig {
            name: path.file_name().unwrap().to_str().unwrap().to_owned(),
            unit: unit_config.unwrap_or_else(Default::default),
            install: install_config.unwrap_or_else(Default::default),
        },
        srvc: service_config,
    })
}

fn parse_timeout(descr: &str) -> Timeout {
    if descr.to_uppercase() == "INFINITY" {
        Timeout::Infinity
    } else if let Ok(secs) = descr.parse::<u64>() {
        Timeout::Duration(std::time::Duration::from_secs(secs))
    } else {
        let mut sum = 0;
        let split = descr.split(' ').collect::<Vec<_>>();
        for t in &split {
            if t.ends_with("min") {
                let mins = t[0..t.len() - 3].parse::<u64>().unwrap();
                sum += mins * 60;
            } else if t.ends_with("hrs") {
                let hrs = t[0..t.len() - 3].parse::<u64>().unwrap();
                sum += hrs * 60 * 60;
            } else if t.ends_with('s') {
                let secs = t[0..t.len() - 1].parse::<u64>().unwrap();
                sum += secs;
            }
        }
        Timeout::Duration(std::time::Duration::from_secs(sum))
    }
}

fn parse_cmdlines(raw_lines: &Vec<(u32, String)>) -> Result<Vec<Commandline>, ParsingErrorReason> {
    let mut cmdlines = Vec::new();
    for (_line, cmdline) in raw_lines {
        cmdlines.push(parse_cmdline(cmdline)?);
    }
    Ok(cmdlines)
}

fn parse_cmdline(raw_line: &str) -> Result<Commandline, ParsingErrorReason> {
    let mut split = shlex::split(raw_line).ok_or(ParsingErrorReason::Generic(format!(
        "Could not parse cmdline: {raw_line}"
    )))?;
    if split.is_empty() {
        return Err(ParsingErrorReason::Generic(format!(
            "Empty command line: {raw_line}"
        )));
    }
    let mut cmd = split.remove(0);

    let mut prefixes = Vec::new();
    loop {
        let prefix = match &cmd[..1] {
            "-" => {
                cmd = cmd[1..].to_owned();
                CommandlinePrefix::Minus
            }
            "+" => {
                return Err(ParsingErrorReason::UnsupportedSetting(
                    "The prefix '+' for cmdlines is currently not supported".into(),
                ));
                //cmd = cmd[1..].to_owned();
                //CommandlinePrefix::Plus
            }
            "@" => {
                cmd = cmd[1..].to_owned();
                CommandlinePrefix::AtSign
            }
            ":" => {
                return Err(ParsingErrorReason::UnsupportedSetting(
                    "The prefix ':' for cmdlines is currently not supported".into(),
                ));
                //cmd = cmd[1..].to_owned();
                //CommandlinePrefix::Colon
            }
            "!" => match &cmd[1..2] {
                "!" => {
                    return Err(ParsingErrorReason::UnsupportedSetting(
                        "The prefix '!!' for cmdlines is currently not supported".into(),
                    ));
                    //cmd = cmd[2..].to_owned();
                    //CommandlinePrefix::DoubleExclamation
                }
                _ => {
                    return Err(ParsingErrorReason::UnsupportedSetting(
                        "The prefix '!' for cmdlines is currently not supported".into(),
                    ));
                    //cmd = cmd[1..].to_owned();
                    //CommandlinePrefix::Exclamation
                }
            },
            _ => break,
        };
        prefixes.push(prefix);
    }
    Ok(Commandline {
        cmd,
        prefixes,
        args: split,
    })
}

fn parse_service_section(
    mut section: ParsedSection,
) -> Result<ParsedServiceSection, ParsingErrorReason> {
    let exec = section.remove("EXECSTART");
    let stop = section.remove("EXECSTOP");
    let stoppost = section.remove("EXECSTOPPOST");
    let startpre = section.remove("EXECSTARTPRE");
    let startpost = section.remove("EXECSTARTPOST");
    let starttimeout = section.remove("TIMEOUTSTARTSEC");
    let stoptimeout = section.remove("TIMEOUTSTOPSEC");
    let generaltimeout = section.remove("TIMEOUTSEC");

    let restart = section.remove("RESTART");
    let restart_sec = section.remove("RESTARTSEC");
    let kill_mode = section.remove("KILLMODE");
    let delegate = section.remove("DELEGATE");
    let tasks_max = section.remove("TASKSMAX");
    let limit_nofile = section.remove("LIMITNOFILE");
    let sockets = section.remove("SOCKETS");
    let notify_access = section.remove("NOTIFYACCESS");
    let srcv_type = section.remove("TYPE");
    let accept = section.remove("ACCEPT");
    let dbus_name = section.remove("BUSNAME");
    let pid_file = section.remove("PIDFILE");
    let slice = section.remove("SLICE");
    let remain_after_exit = section.remove("REMAINAFTEREXIT");

    let exec_config = super::parse_exec_section(&mut section)?;

    for key in section.keys() {
        warn!("Ignoring unsupported setting in [Service] section: {key}");
    }

    let limit_nofile = match limit_nofile {
        Some(vec) => {
            if vec.len() == 1 {
                let val = vec[0].1.trim();
                if val.to_uppercase() == "INFINITY" {
                    Some(ResourceLimit {
                        soft: RLimitValue::Infinity,
                        hard: RLimitValue::Infinity,
                    })
                } else if let Some((soft_str, hard_str)) = val.split_once(':') {
                    let soft = if soft_str.trim().to_uppercase() == "INFINITY" {
                        RLimitValue::Infinity
                    } else {
                        RLimitValue::Value(soft_str.trim().parse::<u64>().map_err(|_| {
                            ParsingErrorReason::Generic(format!(
                                "LimitNOFILE soft value is not valid: {soft_str}"
                            ))
                        })?)
                    };
                    let hard = if hard_str.trim().to_uppercase() == "INFINITY" {
                        RLimitValue::Infinity
                    } else {
                        RLimitValue::Value(hard_str.trim().parse::<u64>().map_err(|_| {
                            ParsingErrorReason::Generic(format!(
                                "LimitNOFILE hard value is not valid: {hard_str}"
                            ))
                        })?)
                    };
                    Some(ResourceLimit { soft, hard })
                } else {
                    let num = val.parse::<u64>().map_err(|_| {
                        ParsingErrorReason::Generic(format!(
                            "LimitNOFILE is not a valid value: {val}"
                        ))
                    })?;
                    Some(ResourceLimit {
                        soft: RLimitValue::Value(num),
                        hard: RLimitValue::Value(num),
                    })
                }
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "LimitNOFILE".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => None,
    };

    let tasks_max = match tasks_max {
        Some(vec) => {
            if vec.len() == 1 {
                let val = vec[0].1.trim();
                if val.to_uppercase() == "INFINITY" {
                    Some(TasksMax::Infinity)
                } else if let Some(pct) = val.strip_suffix('%') {
                    let pct_val = pct.trim().parse::<u64>().map_err(|_| {
                        ParsingErrorReason::Generic(format!(
                            "TasksMax percentage is not a valid number: {val}"
                        ))
                    })?;
                    Some(TasksMax::Percent(pct_val))
                } else {
                    let num = val.parse::<u64>().map_err(|_| {
                        ParsingErrorReason::Generic(format!("TasksMax is not a valid value: {val}"))
                    })?;
                    Some(TasksMax::Value(num))
                }
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "TasksMax".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => None,
    };

    let delegate = match delegate {
        Some(vec) => {
            if vec.len() == 1 {
                let val = &vec[0].1;
                if string_to_bool(val) {
                    Delegate::Yes
                } else if val.to_uppercase() == "NO"
                    || val.to_uppercase() == "FALSE"
                    || val == "0"
                    || val.is_empty()
                {
                    Delegate::No
                } else {
                    // Treat as a space-separated list of controller names
                    let controllers: Vec<String> =
                        val.split_whitespace().map(|s| s.to_owned()).collect();
                    if controllers.is_empty() {
                        Delegate::No
                    } else {
                        Delegate::Controllers(controllers)
                    }
                }
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "Delegate".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => Delegate::default(),
    };

    let kill_mode = match kill_mode {
        Some(vec) => {
            if vec.len() == 1 {
                match vec[0].1.to_lowercase().replace('-', "").as_str() {
                    "controlgroup" => KillMode::ControlGroup,
                    "process" => KillMode::Process,
                    "mixed" => KillMode::Mixed,
                    "none" => KillMode::None,
                    name => {
                        return Err(ParsingErrorReason::UnknownSetting(
                            "KillMode".to_owned(),
                            name.to_owned(),
                        ))
                    }
                }
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "KillMode".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => KillMode::default(),
    };

    let restart_sec = match restart_sec {
        Some(vec) => {
            if vec.len() == 1 {
                Some(parse_timeout(&vec[0].1))
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "RestartSec".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => None,
    };

    let starttimeout = match starttimeout {
        Some(vec) => {
            if vec.len() == 1 {
                Some(parse_timeout(&vec[0].1))
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "TimeoutStartSec".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => None,
    };
    let stoptimeout = match stoptimeout {
        Some(vec) => {
            if vec.len() == 1 {
                Some(parse_timeout(&vec[0].1))
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "TimeoutStopSec".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => None,
    };
    let generaltimeout = match generaltimeout {
        Some(vec) => {
            if vec.len() == 1 {
                Some(parse_timeout(&vec[0].1))
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "TimeoutSec".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => None,
    };

    let exec = match exec {
        Some(mut vec) => {
            if vec.len() == 1 {
                Some(parse_cmdline(&vec.remove(0).1)?)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "ExecStart".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => None,
    };

    let srcv_type = match srcv_type {
        Some(vec) => {
            if vec.len() == 1 {
                match vec[0].1.as_str() {
                    "simple" => ServiceType::Simple,
                    "notify" => ServiceType::Notify,
                    "notify-reload" => ServiceType::NotifyReload,
                    "oneshot" => ServiceType::OneShot,
                    "forking" => ServiceType::Forking,
                    "idle" => ServiceType::Idle,
                    "dbus" => {
                        if cfg!(feature = "dbus_support") {
                            ServiceType::Dbus
                        } else {
                            return Err(ParsingErrorReason::UnsupportedSetting(
                                "Type=dbus".to_owned(),
                            ));
                        }
                    }
                    name => {
                        return Err(ParsingErrorReason::UnknownSetting(
                            "Type".to_owned(),
                            name.to_owned(),
                        ))
                    }
                }
            } else if vec.is_empty() {
                return Err(ParsingErrorReason::MissingSetting("Type".to_owned()));
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "Type".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        // When no Type= is specified and there is no ExecStart=, default to
        // oneshot (matches systemd behavior for exec-less service units like
        // systemd-reboot.service).
        None => {
            if exec.is_none() {
                ServiceType::OneShot
            } else {
                ServiceType::Simple
            }
        }
    };

    let notifyaccess = match notify_access {
        Some(vec) => {
            if vec.len() == 1 {
                match vec[0].1.as_str() {
                    "all" => NotifyKind::All,
                    "main" => NotifyKind::Main,
                    "exec" => NotifyKind::Exec,
                    "none" => NotifyKind::None,
                    name => {
                        return Err(ParsingErrorReason::UnknownSetting(
                            "NotifyAccess".to_owned(),
                            name.to_owned(),
                        ))
                    }
                }
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "NotifyAccess".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => NotifyKind::Main,
    };

    let stop = match stop {
        Some(vec) => parse_cmdlines(&vec)?,
        None => Vec::new(),
    };
    let stoppost = match stoppost {
        Some(vec) => parse_cmdlines(&vec)?,
        None => Vec::new(),
    };
    let startpre = match startpre {
        Some(vec) => parse_cmdlines(&vec)?,
        None => Vec::new(),
    };
    let startpost = match startpost {
        Some(vec) => parse_cmdlines(&vec)?,
        None => Vec::new(),
    };

    let restart = match restart {
        Some(vec) => {
            if vec.len() == 1 {
                match vec[0].1.to_uppercase().replace('-', "").as_str() {
                    "ALWAYS" => ServiceRestart::Always,
                    "NO" => ServiceRestart::No,
                    "ONSUCCESS" => ServiceRestart::OnSuccess,
                    "ONFAILURE" => ServiceRestart::OnFailure,
                    "ONABNORMAL" => ServiceRestart::OnAbnormal,
                    "ONABORT" => ServiceRestart::OnAbort,
                    "ONWATCHDOG" => ServiceRestart::OnWatchdog,

                    name => {
                        return Err(ParsingErrorReason::UnknownSetting(
                            "Restart".to_owned(),
                            name.to_owned(),
                        ))
                    }
                }
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "Restart".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => ServiceRestart::No,
    };
    let accept = match accept {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "Accept".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => false,
    };
    let dbus_name = match dbus_name {
        Some(vec) => {
            if vec.len() == 1 {
                Some(vec[0].1.clone())
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "BusName".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => None,
    };

    if srcv_type == ServiceType::Dbus && dbus_name.is_none() {
        return Err(ParsingErrorReason::MissingSetting("BusName".to_owned()));
    }

    let pid_file = match pid_file {
        None => None,
        Some(mut vec) => {
            if vec.len() == 1 {
                Some(std::path::PathBuf::from(vec.remove(0).1))
            } else if vec.len() > 1 {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "PIDFile".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            } else {
                None
            }
        }
    };

    Ok(ParsedServiceSection {
        srcv_type,
        notifyaccess,
        restart,
        restart_sec,
        kill_mode,
        delegate,
        tasks_max,
        limit_nofile,
        accept,
        dbus_name,
        pid_file,
        exec,
        stop,
        stoppost,
        startpre,
        startpost,
        starttimeout,
        stoptimeout,
        generaltimeout,
        sockets: map_tuples_to_second(super::split_list_values(sockets.unwrap_or_default())),
        slice: slice.and_then(|vec| {
            if vec.len() == 1 {
                Some(vec[0].1.clone())
            } else {
                None
            }
        }),
        remain_after_exit: remain_after_exit
            .map(|vec| {
                if vec.len() == 1 {
                    string_to_bool(&vec[0].1)
                } else {
                    false
                }
            })
            .unwrap_or(false),
        exec_section: exec_config,
    })
}
