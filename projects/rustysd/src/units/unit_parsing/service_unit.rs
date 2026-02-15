use log::{trace, warn};

use crate::units::{
    map_tuples_to_second, parse_install_section, parse_unit_section, string_to_bool, Commandline,
    CommandlinePrefix, Delegate, KeyringMode, KillMode, MemoryPressureWatch, NotifyKind,
    ParsedCommonConfig, ParsedFile, ParsedSection, ParsedServiceConfig, ParsedServiceSection,
    ParsingErrorReason, RLimitValue, ResourceLimit, ServiceRestart, ServiceType, SuccessExitStatus,
    TasksMax, Timeout,
};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::PathBuf;

/// Parse a signal name string (with or without `SIG` prefix) into a
/// `nix::sys::signal::Signal`.  Accepts e.g. `SIGTERM`, `TERM`, `SIGHUP`,
/// `HUP`, etc.  Returns `None` for unrecognised names.
fn parse_signal_name(s: &str) -> Option<nix::sys::signal::Signal> {
    use nix::sys::signal::Signal;
    let upper = s.to_uppercase();
    let name = upper.strip_prefix("SIG").unwrap_or(&upper);
    match name {
        "HUP" => Some(Signal::SIGHUP),
        "INT" => Some(Signal::SIGINT),
        "QUIT" => Some(Signal::SIGQUIT),
        "ILL" => Some(Signal::SIGILL),
        "TRAP" => Some(Signal::SIGTRAP),
        "ABRT" | "IOT" => Some(Signal::SIGABRT),
        "BUS" => Some(Signal::SIGBUS),
        "FPE" => Some(Signal::SIGFPE),
        "KILL" => Some(Signal::SIGKILL),
        "USR1" => Some(Signal::SIGUSR1),
        "SEGV" => Some(Signal::SIGSEGV),
        "USR2" => Some(Signal::SIGUSR2),
        "PIPE" => Some(Signal::SIGPIPE),
        "ALRM" => Some(Signal::SIGALRM),
        "TERM" => Some(Signal::SIGTERM),
        "CHLD" => Some(Signal::SIGCHLD),
        "CONT" => Some(Signal::SIGCONT),
        "STOP" => Some(Signal::SIGSTOP),
        "TSTP" => Some(Signal::SIGTSTP),
        "TTIN" => Some(Signal::SIGTTIN),
        "TTOU" => Some(Signal::SIGTTOU),
        "URG" => Some(Signal::SIGURG),
        "XCPU" => Some(Signal::SIGXCPU),
        "XFSZ" => Some(Signal::SIGXFSZ),
        "VTALRM" => Some(Signal::SIGVTALRM),
        "PROF" => Some(Signal::SIGPROF),
        "WINCH" => Some(Signal::SIGWINCH),
        "IO" => Some(Signal::SIGIO),
        "SYS" => Some(Signal::SIGSYS),
        "PWR" => Some(Signal::SIGPWR),
        _ => None,
    }
}

/// Parse a `SuccessExitStatus=` value.  The value is a space-separated list
/// of tokens.  Numeric tokens are treated as exit codes; everything else is
/// tried as a signal name.
fn parse_success_exit_status(raw: &str) -> SuccessExitStatus {
    let mut exit_codes = Vec::new();
    let mut signals = Vec::new();
    for token in raw.split_whitespace() {
        if let Ok(code) = token.parse::<i32>() {
            exit_codes.push(code);
        } else if let Some(sig) = parse_signal_name(token) {
            signals.push(sig);
        } else {
            warn!("SuccessExitStatus: ignoring unrecognised token: {token}");
        }
    }
    SuccessExitStatus {
        exit_codes,
        signals,
    }
}

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

            _ if name.starts_with("[X-") || name.starts_with("[x-") => {
                trace!(
                    "Silently ignoring vendor extension section in service unit {path:?}: {name}"
                );
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

pub(super) fn parse_timeout(descr: &str) -> Timeout {
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
    let reload = section.remove("EXECRELOAD");
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
    let success_exit_status = section.remove("SUCCESSEXITSTATUS");
    let send_sighup = section.remove("SENDSIGHUP");
    let memory_pressure_watch = section.remove("MEMORYPRESSUREWATCH");
    let reload_signal = section.remove("RELOADSIGNAL");
    let delegate_subgroup = section.remove("DELEGATESUBGROUP");
    let keyring_mode = section.remove("KEYRINGMODE");
    let device_allow = section.remove("DEVICEALLOW");
    let device_policy = section.remove("DEVICEPOLICY");
    let watchdog_sec = section.remove("WATCHDOGSEC");
    let ip_address_allow = section.remove("IPADDRESSALLOW");
    let ip_address_deny = section.remove("IPADDRESSDENY");
    let file_descriptor_store_max = section.remove("FILEDESCRIPTORSTOREMAX");
    let kill_signal = section.remove("KILLSIGNAL");

    let exec_config = super::parse_exec_section(&mut section)?;

    for key in section.keys() {
        if key.starts_with("X-") {
            trace!("Silently ignoring vendor extension in [Service] section: {key}");
            continue;
        }
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

    let watchdog_sec = match watchdog_sec {
        Some(vec) => {
            if vec.len() == 1 {
                let t = parse_timeout(&vec[0].1);
                match t {
                    // A zero duration means "disabled" — store as None
                    Timeout::Duration(d) if d.is_zero() => None,
                    other => Some(other),
                }
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "WatchdogSec".to_owned(),
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

    let reload = match reload {
        Some(vec) => parse_cmdlines(&vec)?,
        None => Vec::new(),
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

    let send_sighup = match send_sighup {
        Some(vec) => {
            if vec.len() == 1 {
                string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "SendSIGHUP".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => false,
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
        reload,
        stop,
        stoppost,
        startpre,
        startpost,
        starttimeout,
        stoptimeout,
        generaltimeout,
        send_sighup,
        memory_pressure_watch: match memory_pressure_watch {
            Some(vec) => {
                if vec.len() == 1 {
                    match vec[0].1.to_lowercase().as_str() {
                        "auto" | "" => MemoryPressureWatch::Auto,
                        "on" | "yes" | "true" | "1" => MemoryPressureWatch::On,
                        "off" | "no" | "false" | "0" => MemoryPressureWatch::Off,
                        "skip" => MemoryPressureWatch::Skip,
                        other => {
                            return Err(ParsingErrorReason::UnknownSetting(
                                "MemoryPressureWatch".to_owned(),
                                other.to_owned(),
                            ))
                        }
                    }
                } else {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "MemoryPressureWatch".to_owned(),
                        map_tuples_to_second(vec),
                    ));
                }
            }
            None => MemoryPressureWatch::default(),
        },
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
        success_exit_status: success_exit_status
            .map(|vec| {
                // Merge all entries — systemd allows multiple SuccessExitStatus=
                // lines and they accumulate.
                let combined: Vec<String> = vec.into_iter().map(|(_, v)| v).collect();
                parse_success_exit_status(&combined.join(" "))
            })
            .unwrap_or_default(),
        reload_signal: match reload_signal {
            Some(vec) => {
                if vec.len() == 1 {
                    let raw = vec[0].1.trim();
                    if raw.is_empty() {
                        None
                    } else {
                        match parse_signal_name(raw) {
                            Some(sig) => Some(sig),
                            None => {
                                // Also try parsing as a plain integer signal number
                                if let Ok(num) = raw.parse::<i32>() {
                                    nix::sys::signal::Signal::try_from(num).ok()
                                } else {
                                    return Err(ParsingErrorReason::UnknownSetting(
                                        "ReloadSignal".to_owned(),
                                        raw.to_owned(),
                                    ));
                                }
                            }
                        }
                    }
                } else {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "ReloadSignal".to_owned(),
                        map_tuples_to_second(vec),
                    ));
                }
            }
            None => None,
        },
        kill_signal: match kill_signal {
            Some(vec) => {
                if vec.len() == 1 {
                    let raw = vec[0].1.trim();
                    if raw.is_empty() {
                        None
                    } else {
                        match parse_signal_name(raw) {
                            Some(sig) => Some(sig),
                            None => {
                                if let Ok(num) = raw.parse::<i32>() {
                                    nix::sys::signal::Signal::try_from(num).ok()
                                } else {
                                    return Err(ParsingErrorReason::UnknownSetting(
                                        "KillSignal".to_owned(),
                                        raw.to_owned(),
                                    ));
                                }
                            }
                        }
                    }
                } else {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "KillSignal".to_owned(),
                        map_tuples_to_second(vec),
                    ));
                }
            }
            None => None,
        },
        delegate_subgroup: match delegate_subgroup {
            Some(vec) => {
                if vec.len() == 1 {
                    let raw = vec[0].1.trim();
                    if raw.is_empty() {
                        None
                    } else {
                        Some(raw.to_owned())
                    }
                } else {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "DelegateSubgroup".to_owned(),
                        map_tuples_to_second(vec),
                    ));
                }
            }
            None => None,
        },
        keyring_mode: match keyring_mode {
            Some(vec) => {
                if vec.len() == 1 {
                    match vec[0].1.trim().to_lowercase().as_str() {
                        "inherit" => KeyringMode::Inherit,
                        "private" => KeyringMode::Private,
                        "shared" => KeyringMode::Shared,
                        other => {
                            return Err(ParsingErrorReason::UnknownSetting(
                                "KeyringMode".to_owned(),
                                other.to_owned(),
                            ))
                        }
                    }
                } else {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "KeyringMode".to_owned(),
                        map_tuples_to_second(vec),
                    ));
                }
            }
            None => KeyringMode::default(),
        },
        device_allow: match device_allow {
            Some(vec) => {
                // Each directive is a device node path (or class like "char-*",
                // "block-*") followed by optional access characters (r, w, m).
                // Multiple directives accumulate. An empty assignment resets
                // the list.
                let mut entries = Vec::new();
                for (_idx, line) in &vec {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        // Empty string resets the list
                        entries.clear();
                        continue;
                    }
                    entries.push(trimmed.to_owned());
                }
                entries
            }
            None => Vec::new(),
        },
        device_policy: match device_policy {
            Some(vec) => {
                if vec.len() == 1 {
                    match vec[0].1.trim().to_lowercase().as_str() {
                        "auto" | "" => super::DevicePolicy::Auto,
                        "closed" => super::DevicePolicy::Closed,
                        "strict" => super::DevicePolicy::Strict,
                        other => {
                            return Err(ParsingErrorReason::UnknownSetting(
                                "DevicePolicy".to_owned(),
                                other.to_owned(),
                            ))
                        }
                    }
                } else {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "DevicePolicy".to_owned(),
                        super::map_tuples_to_second(vec),
                    ));
                }
            }
            None => super::DevicePolicy::default(),
        },
        watchdog_sec,
        ip_address_allow: match ip_address_allow {
            Some(vec) => {
                let mut entries = Vec::new();
                for (_idx, line) in &vec {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        // Empty string resets the list
                        entries.clear();
                        continue;
                    }
                    // Each directive may contain space-separated addresses
                    for addr in trimmed.split_whitespace() {
                        entries.push(addr.to_owned());
                    }
                }
                entries
            }
            None => Vec::new(),
        },
        ip_address_deny: match ip_address_deny {
            Some(vec) => {
                let mut entries = Vec::new();
                for (_idx, line) in &vec {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        // Empty string resets the list
                        entries.clear();
                        continue;
                    }
                    // Each directive may contain space-separated addresses
                    for addr in trimmed.split_whitespace() {
                        entries.push(addr.to_owned());
                    }
                }
                entries
            }
            None => Vec::new(),
        },
        file_descriptor_store_max: match file_descriptor_store_max {
            Some(vec) => {
                if vec.len() == 1 {
                    let val = vec[0].1.trim();
                    if val.is_empty() {
                        0
                    } else {
                        val.parse::<u64>().map_err(|_| {
                            ParsingErrorReason::Generic(format!(
                                "FileDescriptorStoreMax is not a valid non-negative integer: {val}"
                            ))
                        })?
                    }
                } else {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "FileDescriptorStoreMax".to_owned(),
                        super::map_tuples_to_second(vec),
                    ));
                }
            }
            None => 0,
        },
        exec_section: exec_config,
    })
}
