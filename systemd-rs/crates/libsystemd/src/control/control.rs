use crate::lock_ext::RwLockExt;
use crate::runtime_info::{ArcMutRuntimeInfo, UnitTable};
use crate::units::{
    ActivationSource, Specific, Unit, UnitIdKind, UnitStatus, insert_new_units, load_all_units,
    load_new_unit,
};

use std::fmt::Write as _;

use log::trace;
use serde_json::Value;

pub fn open_all_sockets(run_info: ArcMutRuntimeInfo, conf: &crate::config::Config) {
    // TODO make configurable
    let control_sock_path = {
        run_info
            .read()
            .unwrap()
            .config
            .notification_sockets_dir
            .join("control.socket")
    };
    if control_sock_path.exists() {
        std::fs::remove_file(&control_sock_path).unwrap();
    }
    use std::os::unix::net::UnixListener;
    std::fs::create_dir_all(&conf.notification_sockets_dir).unwrap();
    let unixsock = UnixListener::bind(&control_sock_path).unwrap();
    accept_control_connections_unix_socket(run_info, unixsock);
    //let tcpsock = std::net::TcpListener::bind("127.0.0.1:8080").unwrap();
    //accept_control_connections_tcp(
    //    run_info.clone(),
    //    conf.notification_sockets_dir.clone(),
    //    tcpsock,
    //);
}

#[derive(Debug)]
pub enum Command {
    ListUnits(Option<UnitIdKind>),
    Status(Option<String>),
    LoadNew(Vec<String>),
    LoadAllNew,
    LoadAllNewDry,
    Remove(String),
    Restart(String),
    TryRestart(String),
    ReloadOrRestart(String),
    Start(String),
    StartAll(String),
    Stop(String),
    StopAll(String),
    IsActive(String),
    IsEnabled(String),
    IsFailed(String),
    Shutdown,
}

enum ParseError {
    MethodNotFound(String),
    ParamsInvalid(String),
}

fn parse_command(call: &super::jsonrpc2::Call) -> Result<Command, ParseError> {
    let command = match call.method.as_str() {
        "status" => {
            let name = match &call.params {
                Some(Value::String(s)) => Some(s.clone()),
                Some(_) => {
                    return Err(ParseError::ParamsInvalid(
                        "Params must be either none or a single string".to_string(),
                    ));
                }
                None => None,
            };
            Command::Status(name)
        }
        "restart" => {
            let name = match &call.params {
                Some(Value::String(s)) => s.clone(),
                Some(_) | None => {
                    return Err(ParseError::ParamsInvalid(
                        "Params must be a single string".to_string(),
                    ));
                }
            };
            Command::Restart(name)
        }
        "try-restart" | "condrestart" => {
            let name = match &call.params {
                Some(Value::String(s)) => s.clone(),
                Some(_) | None => {
                    return Err(ParseError::ParamsInvalid(
                        "Params must be a single string".to_string(),
                    ));
                }
            };
            Command::TryRestart(name)
        }
        "reload-or-restart" | "reload-or-try-restart" | "force-reload" => {
            let name = match &call.params {
                Some(Value::String(s)) => s.clone(),
                Some(_) | None => {
                    return Err(ParseError::ParamsInvalid(
                        "Params must be a single string".to_string(),
                    ));
                }
            };
            Command::ReloadOrRestart(name)
        }
        "is-active" => {
            let name = match &call.params {
                Some(Value::String(s)) => s.clone(),
                Some(_) | None => {
                    return Err(ParseError::ParamsInvalid(
                        "Params must be a single string".to_string(),
                    ));
                }
            };
            Command::IsActive(name)
        }
        "is-enabled" => {
            let name = match &call.params {
                Some(Value::String(s)) => s.clone(),
                Some(_) | None => {
                    return Err(ParseError::ParamsInvalid(
                        "Params must be a single string".to_string(),
                    ));
                }
            };
            Command::IsEnabled(name)
        }
        "is-failed" => {
            let name = match &call.params {
                Some(Value::String(s)) => s.clone(),
                Some(_) | None => {
                    return Err(ParseError::ParamsInvalid(
                        "Params must be a single string".to_string(),
                    ));
                }
            };
            Command::IsFailed(name)
        }
        "start" => {
            let name = match &call.params {
                Some(Value::String(s)) => s.clone(),
                Some(_) | None => {
                    return Err(ParseError::ParamsInvalid(
                        "Params must be a single string".to_string(),
                    ));
                }
            };
            Command::Start(name)
        }
        "start-all" => {
            let name = match &call.params {
                Some(Value::String(s)) => s.clone(),
                Some(_) | None => {
                    return Err(ParseError::ParamsInvalid(
                        "Params must be a single string".to_string(),
                    ));
                }
            };
            Command::StartAll(name)
        }
        "remove" => {
            let name = match &call.params {
                Some(Value::String(s)) => s.clone(),
                Some(_) | None => {
                    return Err(ParseError::ParamsInvalid(
                        "Params must be a single string".to_string(),
                    ));
                }
            };
            Command::Remove(name)
        }
        "stop" => {
            let name = match &call.params {
                Some(Value::String(s)) => s.clone(),
                Some(_) | None => {
                    return Err(ParseError::ParamsInvalid(
                        "Params must be a single string".to_string(),
                    ));
                }
            };
            Command::Stop(name)
        }
        "stop-all" => {
            let name = match &call.params {
                Some(Value::String(s)) => s.clone(),
                Some(_) | None => {
                    return Err(ParseError::ParamsInvalid(
                        "Params must be a single string".to_string(),
                    ));
                }
            };
            Command::StopAll(name)
        }

        "list-units" => {
            let kind = match &call.params {
                Some(params) => match params {
                    Value::String(s) => {
                        let kind = match s.as_str() {
                            "target" => UnitIdKind::Target,
                            "socket" => UnitIdKind::Socket,
                            "service" => UnitIdKind::Service,
                            "slice" => UnitIdKind::Slice,
                            "mount" => UnitIdKind::Mount,
                            "device" => UnitIdKind::Device,
                            _ => {
                                return Err(ParseError::ParamsInvalid(format!(
                                    "Kind not recognized: {s}"
                                )));
                            }
                        };
                        Some(kind)
                    }
                    _ => {
                        return Err(ParseError::ParamsInvalid(
                            "Params must be a single string".to_string(),
                        ));
                    }
                },
                None => None,
            };
            Command::ListUnits(kind)
        }
        "shutdown" => Command::Shutdown,
        "reload" | "daemon-reload" | "daemon-reexec" => Command::LoadAllNew,
        "reload-dry" => Command::LoadAllNewDry,
        "enable" => {
            let names = match &call.params {
                Some(params) => match params {
                    Value::String(s) => vec![s.clone()],
                    Value::Array(names) => {
                        let mut str_names = Vec::new();
                        for name in names {
                            if let Value::String(name) = name {
                                str_names.push(name.clone());
                            } else {
                                return Err(ParseError::ParamsInvalid(
                                    "Params must be at least one string".to_string(),
                                ));
                            }
                        }
                        str_names
                    }
                    _ => {
                        return Err(ParseError::ParamsInvalid(
                            "Params must be at least one string".to_string(),
                        ));
                    }
                },
                None => {
                    return Err(ParseError::ParamsInvalid(
                        "Params must be at least one string".to_string(),
                    ));
                }
            };
            Command::LoadNew(names)
        }
        _ => {
            return Err(ParseError::MethodNotFound(format!(
                "Unknown method: {}",
                call.method
            )));
        }
    };

    Ok(command)
}

pub fn format_socket(socket_unit: &Unit, status: UnitStatus) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("Name".into(), Value::String(socket_unit.id.name.clone()));
    map.insert("Status".into(), Value::String(format!("{status:?}")));

    if let Specific::Socket(sock) = &socket_unit.specific {
        map.insert(
            "FileDescriptorname".into(),
            Value::String(socket_unit.id.name.clone()),
        );
        map.insert(
            "FileDescriptors".into(),
            Value::Array(
                sock.conf
                    .sockets
                    .iter()
                    .map(|sock_conf| Value::String(format!("{:?}", sock_conf.specialized)))
                    .collect(),
            ),
        );
    }

    Value::Object(map)
}

pub fn format_target(socket_unit: &Unit, status: UnitStatus) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("Name".into(), Value::String(socket_unit.id.name.clone()));
    map.insert("Status".into(), Value::String(format!("{status:?}")));
    Value::Object(map)
}

pub fn format_service(srvc_unit: &Unit, status: UnitStatus) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("Name".into(), Value::String(srvc_unit.id.name.clone()));
    map.insert("Status".into(), Value::String(format!("{status:?}")));
    if let Specific::Service(srvc) = &srvc_unit.specific {
        map.insert(
            "Sockets".into(),
            Value::Array(
                srvc.conf
                    .sockets
                    .iter()
                    .map(|x| Value::String(x.name.clone()))
                    .collect(),
            ),
        );
        if let Some(instant) = srvc.state.read_poisoned().common.up_since {
            map.insert(
                "UpSince".into(),
                Value::String(format!("{:?}", instant.elapsed())),
            );
        }
        map.insert(
            "Restarted".into(),
            Value::String(format!(
                "{:?}",
                srvc.state.read_poisoned().common.restart_count
            )),
        );
    }
    Value::Object(map)
}

fn find_units_with_name<'a>(unit_name: &str, unit_table: &'a UnitTable) -> Vec<&'a Unit> {
    trace!("Find unit for name: {unit_name}");
    unit_table
        .values()
        .filter(|unit| {
            let name = unit.id.name.clone();
            name.starts_with(unit_name)
                || unit
                    .common
                    .unit
                    .aliases
                    .iter()
                    .any(|alias| alias.starts_with(unit_name))
        })
        .collect()
}

/// Try to find a unit by name in the unit table. If not found, attempt to load
/// it from disk (searching the configured unit directories) and insert it into
/// the unit table. Returns the UnitId on success.
///
/// This enables commands like `systemctl restart <unit>` to work even when the
/// unit wasn't part of the initial boot dependency graph (e.g. units triggered
/// on-demand by udev rules).
fn find_or_load_unit(
    unit_name: &str,
    run_info: &ArcMutRuntimeInfo,
) -> Result<crate::units::UnitId, String> {
    // First, try to find under a read lock.
    {
        let ri = run_info.read_poisoned();
        let units = find_units_with_name(unit_name, &ri.unit_table);
        if units.len() > 1 {
            let names: Vec<_> = units.iter().map(|unit| unit.id.name.clone()).collect();
            return Err(format!(
                "More than one unit found with name: {unit_name}: {names:?}"
            ));
        }
        if let Some(unit) = units.first() {
            return Ok(unit.id.clone());
        }
    }
    // Not found — try to load from disk under a write lock.
    {
        let mut ri = run_info.write_poisoned();
        // Re-check after acquiring write lock (another thread may have loaded it).
        let already = find_units_with_name(unit_name, &ri.unit_table);
        if let Some(unit) = already.first() {
            return Ok(unit.id.clone());
        }
        let unit = load_new_unit(&ri.config.unit_dirs, unit_name).map_err(|e| {
            format!("No unit found with name: {unit_name} (also failed to load from disk: {e})")
        })?;
        let id = unit.id.clone();
        // Use lenient insertion: the unit may reference dependencies that aren't
        // in the unit table (e.g. it wasn't part of the boot dependency graph).
        // Missing deps are silently ignored — the unit is wired up to whatever
        // is already present.
        crate::units::insert_new_unit_lenient(unit, &mut ri);
        trace!("Auto-loaded unit {unit_name} from disk");
        Ok(id)
    }
}

// TODO make this some kind of regex pattern matching
fn find_units_with_pattern<'a>(
    name_pattern: &str,
    unit_table_locked: &'a UnitTable,
) -> Vec<&'a Unit> {
    trace!("Find units matching pattern: {name_pattern}");
    let units: Vec<_> = unit_table_locked
        .values()
        .filter(|unit| {
            let name = unit.id.name.clone();
            name.starts_with(name_pattern)
                || unit
                    .common
                    .unit
                    .aliases
                    .iter()
                    .any(|alias| alias.starts_with(name_pattern))
        })
        .collect();
    units
}

pub fn execute_command(
    cmd: Command,
    run_info: ArcMutRuntimeInfo,
) -> Result<serde_json::Value, String> {
    let mut result_vec = Value::Array(Vec::new());
    match cmd {
        Command::Shutdown => {
            crate::shutdown::shutdown_sequence(run_info);
        }
        Command::Restart(unit_name) => {
            let id = find_or_load_unit(&unit_name, &run_info)?;
            let ri = run_info.read_poisoned();
            crate::units::reactivate_unit(id, &ri).map_err(|e| format!("{e}"))?;
        }
        Command::TryRestart(unit_name) => {
            // try-restart: restart the unit only if it is currently active.
            // If the unit is not active, do nothing (success).
            let ri = run_info.read_poisoned();
            let units = find_units_with_name(&unit_name, &ri.unit_table);
            if units.is_empty() {
                // Unit not found — nothing to restart, not an error for try-restart.
                return Ok(serde_json::json!(null));
            }
            if units.len() > 1 {
                let names: Vec<_> = units.iter().map(|unit| unit.id.name.clone()).collect();
                return Err(format!(
                    "More than one unit found with name: {unit_name}: {names:?}"
                ));
            }

            let id = units[0].id.clone();
            // Check if the unit is currently active.
            let is_active = ri
                .unit_table
                .get(&id)
                .map(|unit| {
                    let status_locked = unit.common.status.read_poisoned();
                    matches!(
                        *status_locked,
                        crate::units::UnitStatus::Started(_) | crate::units::UnitStatus::Starting
                    )
                })
                .unwrap_or(false);

            if is_active {
                crate::units::reactivate_unit(id, &ri).map_err(|e| format!("{e}"))?;
            }
            // If not active, silently succeed.
        }
        Command::ReloadOrRestart(unit_name) => {
            // reload-or-restart: try to reload, fall back to restart.
            // Since we don't support reload yet, just restart.
            let id = find_or_load_unit(&unit_name, &run_info)?;
            let ri = run_info.read_poisoned();
            crate::units::reactivate_unit(id, &ri).map_err(|e| format!("{e}"))?;
        }
        Command::IsActive(unit_name) => {
            let run_info = &*run_info.read_poisoned();
            let unit_table = &run_info.unit_table;
            let units = find_units_with_name(&unit_name, unit_table);
            if units.is_empty() {
                return Ok(serde_json::json!("inactive"));
            }
            let unit = &units[0];
            let status_locked = unit.common.status.read_poisoned();
            let state = match &*status_locked {
                crate::units::UnitStatus::Started(_) => "active",
                crate::units::UnitStatus::Starting => "activating",
                crate::units::UnitStatus::Stopping => "deactivating",
                crate::units::UnitStatus::Restarting => "activating",
                crate::units::UnitStatus::Stopped(_, errors) if !errors.is_empty() => "failed",
                crate::units::UnitStatus::Stopped(_, _) => "inactive",
                crate::units::UnitStatus::NeverStarted => "inactive",
            };
            return Ok(serde_json::json!(state));
        }
        Command::IsEnabled(unit_name) => {
            // Check if the unit is enabled (i.e. has an [Install] section
            // and is linked in the target wants).
            // For now, if the unit is loaded at all, report "enabled".
            let run_info = &*run_info.read_poisoned();
            let unit_table = &run_info.unit_table;
            let units = find_units_with_name(&unit_name, unit_table);
            if units.is_empty() {
                return Ok(serde_json::json!("disabled"));
            }
            return Ok(serde_json::json!("enabled"));
        }
        Command::IsFailed(unit_name) => {
            let run_info = &*run_info.read_poisoned();
            let unit_table = &run_info.unit_table;
            let units = find_units_with_name(&unit_name, unit_table);
            if units.is_empty() {
                return Ok(serde_json::json!("inactive"));
            }
            let unit = &units[0];
            let status_locked = unit.common.status.read_poisoned();
            let state = match &*status_locked {
                crate::units::UnitStatus::Stopped(_, errors) if !errors.is_empty() => "failed",
                _ => "inactive",
            };
            return Ok(serde_json::json!(state));
        }
        Command::Start(unit_name) => {
            let id = find_or_load_unit(&unit_name, &run_info)?;
            let ri = run_info.read_poisoned();
            crate::units::activate_unit(id, &ri, ActivationSource::Regular)
                .map_err(|e| format!("{e}"))?;
            // Happy
        }
        Command::StartAll(unit_name) => {
            let id = {
                let run_info_locked = &*run_info.read_poisoned();
                let unit_table = &run_info_locked.unit_table;
                let units = find_units_with_name(&unit_name, unit_table);
                if units.len() > 1 {
                    let names: Vec<_> = units.iter().map(|unit| unit.id.name.clone()).collect();
                    return Err(format!(
                        "More than one unit found with name: {unit_name}: {names:?}"
                    ));
                }
                if units.is_empty() {
                    return Err(format!("No unit found with name: {unit_name}"));
                }

                units[0].id.clone()
            };

            let errs = crate::units::activate_needed_units(id, run_info);
            if !errs.is_empty() {
                let mut errstr = String::from("Errors while starting the units:");
                for err in errs {
                    let _ = write!(errstr, "\n{err:?}");
                }
                return Err(errstr);
            }
        }
        Command::Remove(unit_name) => {
            let run_info = &mut *run_info.write_poisoned();
            let id = {
                let units = find_units_with_name(&unit_name, &run_info.unit_table);
                if units.len() > 1 {
                    let names: Vec<_> = units.iter().map(|unit| unit.id.name.clone()).collect();
                    return Err(format!(
                        "More than one unit found with name: {unit_name}: {names:?}"
                    ));
                }
                if units.is_empty() {
                    return Err(format!("No unit found with name: {unit_name}"));
                }

                units[0].id.clone()
            };

            crate::units::remove_unit_with_dependencies(id, run_info)?;
        }
        Command::Stop(unit_name) => {
            let run_info = &*run_info.read_poisoned();
            let id = {
                let units = find_units_with_name(&unit_name, &run_info.unit_table);
                if units.len() > 1 {
                    let names: Vec<_> = units.iter().map(|unit| unit.id.name.clone()).collect();
                    return Err(format!(
                        "More than one unit found with name: {unit_name}: {names:?}"
                    ));
                }
                if units.is_empty() {
                    return Err(format!("No unit found with name: {unit_name}"));
                }

                units[0].id.clone()
            };

            crate::units::deactivate_unit(&id, run_info).map_err(|e| format!("{e}"))?;
            // Happy
        }
        Command::StopAll(unit_name) => {
            let run_info = &*run_info.read_poisoned();
            let id = {
                let units = find_units_with_name(&unit_name, &run_info.unit_table);
                if units.len() > 1 {
                    let names: Vec<_> = units.iter().map(|unit| unit.id.name.clone()).collect();
                    return Err(format!(
                        "More than one unit found with name: {unit_name}: {names:?}"
                    ));
                }
                if units.is_empty() {
                    return Err(format!("No unit found with name: {unit_name}"));
                }

                units[0].id.clone()
            };

            crate::units::deactivate_unit_recursive(&id, run_info).map_err(|e| format!("{e}"))?;
            // Happy
        }
        Command::Status(unit_name) => {
            let run_info = &*run_info.read_poisoned();
            let unit_table = &run_info.unit_table;
            if let Some(name) = unit_name {
                //list specific
                let units = find_units_with_pattern(&name, unit_table);
                for unit in units {
                    let status = { unit.common.status.read_poisoned().clone() };
                    if name.ends_with(".service") {
                        result_vec
                            .as_array_mut()
                            .unwrap()
                            .push(format_service(unit, status));
                    } else if name.ends_with(".socket") {
                        result_vec
                            .as_array_mut()
                            .unwrap()
                            .push(format_socket(unit, status));
                    } else if name.ends_with(".target") || name.ends_with(".slice") {
                        result_vec
                            .as_array_mut()
                            .unwrap()
                            .push(format_target(unit, status));
                    } else {
                        return Err("Name suffix not recognized".into());
                    }
                }
            } else {
                //list all
                let strings: Vec<_> = unit_table
                    .values()
                    .map(|unit| {
                        let status = { unit.common.status.read_poisoned().clone() };
                        match unit.specific {
                            Specific::Socket(_) => format_socket(unit, status),
                            Specific::Service(_) => format_service(unit, status),
                            Specific::Target(_) | Specific::Slice(_) | Specific::Mount(_) => {
                                format_target(unit, status)
                            }
                        }
                    })
                    .collect();
                for s in strings {
                    result_vec.as_array_mut().unwrap().push(s);
                }
            }
        }
        Command::ListUnits(kind) => {
            let run_info = &*run_info.read_poisoned();
            let unit_table = &run_info.unit_table;
            for (id, unit) in unit_table {
                let include = if let Some(kind) = kind {
                    id.kind == kind
                } else {
                    true
                };
                if include {
                    result_vec
                        .as_array_mut()
                        .unwrap()
                        .push(Value::String(unit.id.name.clone()));
                }
            }
        }
        Command::LoadNew(names) => {
            let run_info = &mut *run_info.write_poisoned();
            let mut map = std::collections::HashMap::new();
            for name in &names {
                let unit = load_new_unit(&run_info.config.unit_dirs, name)?;
                map.insert(unit.id.clone(), unit);
            }
            insert_new_units(map, run_info)?;
        }
        Command::LoadAllNew => {
            let run_info = &mut *run_info.write_poisoned();
            let unit_table = &run_info.unit_table;
            // get all units there are
            let units = load_all_units(&run_info.config.unit_dirs, &run_info.config.target_unit)
                .map_err(|e| format!("Error while loading unit definitions: {e:?}"))?;

            // collect all names
            let existing_names = unit_table
                .values()
                .map(|unit| unit.id.name.clone())
                .collect::<Vec<_>>();

            // filter out existing units
            let mut ignored_units_names = Vec::new();
            let mut new_units_names = Vec::new();
            let mut new_units = std::collections::HashMap::new();
            for (id, unit) in units {
                if existing_names.contains(&unit.id.name) {
                    ignored_units_names.push(Value::String(unit.id.name.clone()));
                } else {
                    new_units_names.push(Value::String(unit.id.name.clone()));
                    new_units.insert(id, unit);
                }
            }

            let mut response_object = serde_json::Map::new();
            insert_new_units(new_units, run_info)?;
            response_object.insert("Added".into(), serde_json::Value::Array(new_units_names));
            response_object.insert(
                "Ignored".into(),
                serde_json::Value::Array(ignored_units_names),
            );
            result_vec
                .as_array_mut()
                .unwrap()
                .push(Value::Object(response_object));
        }
        Command::LoadAllNewDry => {
            let run_info = &mut *run_info.write_poisoned();
            let unit_table = &run_info.unit_table;
            // get all units there are
            let units = load_all_units(&run_info.config.unit_dirs, &run_info.config.target_unit)
                .map_err(|e| format!("Error while loading unit definitions: {e:?}"))?;

            // collect all names
            let existing_names = unit_table
                .values()
                .map(|unit| unit.id.name.clone())
                .collect::<Vec<_>>();

            // filter out existing units
            let mut ignored_units_names = Vec::new();
            let mut new_units_names = Vec::new();
            for (_id, unit) in units {
                if existing_names.contains(&unit.id.name) {
                    ignored_units_names.push(Value::String(unit.id.name.clone()));
                } else {
                    new_units_names.push(Value::String(unit.id.name.clone()));
                }
            }

            let mut response_object = serde_json::Map::new();
            response_object.insert(
                "Would add".into(),
                serde_json::Value::Array(new_units_names),
            );
            response_object.insert(
                "Would ignore".into(),
                serde_json::Value::Array(ignored_units_names),
            );
            result_vec
                .as_array_mut()
                .unwrap()
                .push(Value::Object(response_object));
        }
    }

    Ok(result_vec)
}

use std::io::Read;
use std::io::Write as IoWrite;
pub fn listen_on_commands<T: 'static + Read + IoWrite + Send>(
    mut source: Box<T>,
    run_info: ArcMutRuntimeInfo,
) {
    std::thread::spawn(move || {
        loop {
            match super::jsonrpc2::get_next_call(source.as_mut()) {
                Err(e) => {
                    if e.classify() == serde_json::error::Category::Eof {
                        // ignore, just stop reading
                    } else {
                        let err = super::jsonrpc2::make_error(
                            super::jsonrpc2::PARSE_ERROR,
                            format!("{e}"),
                            None,
                        );
                        let msg = super::jsonrpc2::make_error_response(None, err);
                        let response_string = serde_json::to_string_pretty(&msg).unwrap();
                        source.write_all(response_string.as_bytes()).unwrap();
                    }
                    return;
                }
                Ok(call) => {
                    match call {
                        Err(e) => {
                            let err = super::jsonrpc2::make_error(
                                super::jsonrpc2::INVALID_REQUEST_ERROR,
                                e,
                                None,
                            );
                            let msg = super::jsonrpc2::make_error_response(None, err);
                            let response_string = serde_json::to_string_pretty(&msg).unwrap();
                            source.write_all(response_string.as_bytes()).unwrap();
                        }
                        Ok(call) => {
                            match parse_command(&call) {
                                Err(e) => {
                                    // TODO invalid arguments error
                                    let (code, err_msg) = match e {
                                        ParseError::ParamsInvalid(s) => {
                                            (super::jsonrpc2::INVALID_PARAMS_ERROR, s)
                                        }
                                        ParseError::MethodNotFound(s) => {
                                            (super::jsonrpc2::METHOD_NOT_FOUND_ERROR, s)
                                        }
                                    };
                                    let err = super::jsonrpc2::make_error(code, err_msg, None);
                                    let msg = super::jsonrpc2::make_error_response(call.id, err);
                                    let response_string =
                                        serde_json::to_string_pretty(&msg).unwrap();
                                    source.write_all(response_string.as_bytes()).unwrap();
                                }
                                Ok(cmd) => {
                                    trace!("Execute command: {cmd:?}");
                                    let msg = match execute_command(cmd, run_info.clone()) {
                                        Err(e) => {
                                            let err = super::jsonrpc2::make_error(
                                                super::jsonrpc2::SERVER_ERROR,
                                                e,
                                                None,
                                            );
                                            super::jsonrpc2::make_error_response(call.id, err)
                                        }
                                        Ok(result) => {
                                            super::jsonrpc2::make_result_response(call.id, result)
                                        }
                                    };
                                    let response_string =
                                        serde_json::to_string_pretty(&msg).unwrap();
                                    source.write_all(response_string.as_bytes()).unwrap();
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

pub fn accept_control_connections_unix_socket(
    run_info: ArcMutRuntimeInfo,
    source: std::os::unix::net::UnixListener,
) {
    std::thread::spawn(move || {
        loop {
            let stream = Box::new(source.accept().unwrap().0);
            listen_on_commands(stream, run_info.clone());
        }
    });
}

pub fn accept_control_connections_tcp(run_info: ArcMutRuntimeInfo, source: std::net::TcpListener) {
    std::thread::spawn(move || {
        loop {
            let stream = Box::new(source.accept().unwrap().0);
            listen_on_commands(stream, run_info.clone());
        }
    });
}
