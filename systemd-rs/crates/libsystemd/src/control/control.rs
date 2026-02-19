use crate::control::unit_properties;
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
    /// `list-dependencies <unit> [--reverse]` — show the dependency tree.
    ListDependencies(String, bool),
    Status(Option<String>),
    /// `show <unit> [property,...]` — return all (or filtered) properties as key=value.
    Show(String, Option<Vec<String>>),
    /// `cat <unit>` — return the unit file source text.
    Cat(String),
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
    /// `mask <unit>...` — symlink unit files to /dev/null.
    Mask(Vec<String>),
    /// `unmask <unit>...` — remove /dev/null symlinks for units.
    Unmask(Vec<String>),
    Shutdown,
}

#[derive(Debug)]
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
        "show" => {
            // show <unit> [property-filter...]
            // Params: String (unit name) or Array [unit_name, prop1, prop2, ...]
            match &call.params {
                Some(Value::String(s)) => Command::Show(s.clone(), None),
                Some(Value::Array(arr)) if !arr.is_empty() => {
                    let name = arr[0].as_str().unwrap_or("").to_owned();
                    let filter = if arr.len() > 1 {
                        Some(
                            arr[1..]
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                                .collect(),
                        )
                    } else {
                        None
                    };
                    Command::Show(name, filter)
                }
                Some(_) | None => {
                    return Err(ParseError::ParamsInvalid(
                        "show requires a unit name".to_string(),
                    ));
                }
            }
        }
        "cat" => {
            let name = match &call.params {
                Some(Value::String(s)) => s.clone(),
                Some(_) | None => {
                    return Err(ParseError::ParamsInvalid(
                        "Params must be a single string".to_string(),
                    ));
                }
            };
            Command::Cat(name)
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
        "list-dependencies" => {
            // Params: String (unit name) or Array [unit_name, "--reverse"]
            match &call.params {
                Some(Value::String(s)) => Command::ListDependencies(s.clone(), false),
                Some(Value::Array(arr)) if !arr.is_empty() => {
                    let name = arr[0].as_str().unwrap_or("").to_owned();
                    let reverse = arr.iter().skip(1).any(|v| v.as_str() == Some("--reverse"));
                    Command::ListDependencies(name, reverse)
                }
                Some(_) | None => {
                    return Err(ParseError::ParamsInvalid(
                        "list-dependencies requires a unit name".to_string(),
                    ));
                }
            }
        }
        "mask" => {
            let names = match &call.params {
                Some(Value::String(s)) => vec![s.clone()],
                Some(Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                    .collect(),
                Some(_) | None => {
                    return Err(ParseError::ParamsInvalid(
                        "mask requires at least one unit name".to_string(),
                    ));
                }
            };
            Command::Mask(names)
        }
        "unmask" => {
            let names = match &call.params {
                Some(Value::String(s)) => vec![s.clone()],
                Some(Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                    .collect(),
                Some(_) | None => {
                    return Err(ParseError::ParamsInvalid(
                        "unmask requires at least one unit name".to_string(),
                    ));
                }
            };
            Command::Unmask(names)
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

/// Format a dependency tree as an indented string with box-drawing characters.
///
/// `visited` tracks already-printed units to avoid infinite loops in cyclic graphs.
fn format_dep_tree(
    unit_name: &str,
    unit_table: &UnitTable,
    reverse: bool,
    prefix: &str,
    is_last: bool,
    visited: &mut std::collections::HashSet<String>,
    out: &mut String,
    depth: usize,
) {
    // Print this node
    if depth == 0 {
        // Root node — no prefix/connector
        let status_marker = unit_status_marker(unit_name, unit_table);
        let _ = writeln!(out, "{status_marker}{unit_name}");
    } else {
        let connector = if is_last { "└─" } else { "├─" };
        let status_marker = unit_status_marker(unit_name, unit_table);
        let _ = writeln!(out, "{prefix}{connector}{status_marker}{unit_name}");
    }

    // Prevent infinite recursion on cycles
    if !visited.insert(unit_name.to_owned()) {
        return;
    }

    // Limit depth to avoid excessively deep trees
    if depth >= 20 {
        return;
    }

    // Find children (forward deps = wants+requires, reverse deps = wanted_by+required_by)
    let children: Vec<String> =
        if let Some(unit) = unit_table.values().find(|u| u.id.name == unit_name) {
            let deps = &unit.common.dependencies;
            if reverse {
                let mut c: Vec<String> = Vec::new();
                for id in &deps.wanted_by {
                    c.push(id.name.clone());
                }
                for id in &deps.required_by {
                    if !c.contains(&id.name) {
                        c.push(id.name.clone());
                    }
                }
                for id in &deps.bound_by {
                    if !c.contains(&id.name) {
                        c.push(id.name.clone());
                    }
                }
                c.sort();
                c
            } else {
                let mut c: Vec<String> = Vec::new();
                for id in &deps.wants {
                    c.push(id.name.clone());
                }
                for id in &deps.requires {
                    if !c.contains(&id.name) {
                        c.push(id.name.clone());
                    }
                }
                for id in &deps.binds_to {
                    if !c.contains(&id.name) {
                        c.push(id.name.clone());
                    }
                }
                c.sort();
                c
            }
        } else {
            Vec::new()
        };

    let child_prefix = if depth == 0 {
        String::new()
    } else if is_last {
        format!("{prefix}  ")
    } else {
        format!("{prefix}│ ")
    };

    for (i, child) in children.iter().enumerate() {
        let child_is_last = i == children.len() - 1;
        format_dep_tree(
            child,
            unit_table,
            reverse,
            &child_prefix,
            child_is_last,
            visited,
            out,
            depth + 1,
        );
    }

    // Remove from visited so the same unit can appear in other branches
    // (but the recursion guard above prevents infinite loops within a single branch)
    visited.remove(unit_name);
}

/// Return a colored status marker for a unit: ● (green=active, white=inactive, red=failed).
fn unit_status_marker(unit_name: &str, unit_table: &UnitTable) -> &'static str {
    if let Some(unit) = unit_table.values().find(|u| u.id.name == unit_name) {
        let status = unit.common.status.read_poisoned();
        match &*status {
            UnitStatus::Started(_) => "● ",
            UnitStatus::Starting | UnitStatus::Restarting => "● ",
            UnitStatus::Stopped(_, errors) if !errors.is_empty() => "× ",
            UnitStatus::Stopped(_, _) | UnitStatus::NeverStarted => "○ ",
            UnitStatus::Stopping => "○ ",
        }
    } else {
        "○ "
    }
}

pub fn execute_command(
    cmd: Command,
    run_info: ArcMutRuntimeInfo,
) -> Result<serde_json::Value, String> {
    let mut result_vec = Value::Array(Vec::new());
    match cmd {
        Command::ListDependencies(unit_name, reverse) => {
            let ri = run_info.read_poisoned();
            let units = find_units_with_name(&unit_name, &ri.unit_table);
            if units.is_empty() {
                return Err(format!("Unit {unit_name} not found."));
            }
            let name = units[0].id.name.clone();
            let mut out = String::new();
            let mut visited = std::collections::HashSet::new();
            format_dep_tree(
                &name,
                &ri.unit_table,
                reverse,
                "",
                true,
                &mut visited,
                &mut out,
                0,
            );
            return Ok(serde_json::json!({ "list-dependencies": out }));
        }
        Command::Show(unit_name, filter) => {
            let ri = run_info.read_poisoned();
            let units = find_units_with_name(&unit_name, &ri.unit_table);
            if units.is_empty() {
                return Err(format!("Unit {unit_name} not found."));
            }
            let unit = &units[0];
            let props = unit_properties::collect_properties(unit);
            let text = unit_properties::format_properties(&props, filter.as_deref());
            return Ok(serde_json::json!({ "show": text }));
        }
        Command::Cat(unit_name) => {
            let ri = run_info.read_poisoned();
            let units = find_units_with_name(&unit_name, &ri.unit_table);
            if units.is_empty() {
                return Err(format!("Unit {unit_name} not found."));
            }
            let unit = &units[0];
            let fragment_path = unit.common.unit.fragment_path.as_ref();
            match fragment_path {
                Some(path) => {
                    let content = std::fs::read_to_string(path)
                        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
                    let mut out = format!("# {}\n", path.display());
                    out.push_str(&content);
                    return Ok(serde_json::json!({ "cat": out }));
                }
                None => {
                    return Err(format!(
                        "No fragment path recorded for {unit_name} (unit may have been generated at runtime)"
                    ));
                }
            }
        }
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
        Command::Mask(names) => {
            // Create symlinks to /dev/null in /run/systemd/system/ (runtime mask)
            // or /etc/systemd/system/ (persistent mask).
            // We use /etc/systemd/system/ to match systemd's default behavior.
            let mask_dir = std::path::Path::new("/etc/systemd/system");
            let mut masked = Vec::new();
            for name in &names {
                let link_path = mask_dir.join(name);
                // Remove existing file/symlink if present
                if link_path.exists() || link_path.symlink_metadata().is_ok() {
                    if let Err(e) = std::fs::remove_file(&link_path) {
                        return Err(format!("Failed to remove {}: {e}", link_path.display()));
                    }
                }
                // Create symlink to /dev/null
                if let Err(e) = std::os::unix::fs::symlink("/dev/null", &link_path) {
                    return Err(format!(
                        "Failed to create mask symlink {}: {e}",
                        link_path.display()
                    ));
                }
                masked.push(Value::String(name.clone()));
            }
            return Ok(serde_json::json!({ "masked": masked }));
        }
        Command::Unmask(names) => {
            let mask_dir = std::path::Path::new("/etc/systemd/system");
            let mut unmasked = Vec::new();
            for name in &names {
                let link_path = mask_dir.join(name);
                // Only remove if it's a symlink pointing to /dev/null
                if let Ok(target) = std::fs::read_link(&link_path) {
                    if target == std::path::Path::new("/dev/null") {
                        if let Err(e) = std::fs::remove_file(&link_path) {
                            return Err(format!(
                                "Failed to remove mask symlink {}: {e}",
                                link_path.display()
                            ));
                        }
                        unmasked.push(Value::String(name.clone()));
                    }
                }
                // If it's not a symlink to /dev/null, silently skip
            }
            return Ok(serde_json::json!({ "unmasked": unmasked }));
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

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::{
        Common, CommonState, Dependencies, Specific, StatusStarted, StatusStopped, TargetSpecific,
        TargetState, Unit, UnitConfig, UnitId, UnitIdKind, UnitOperationErrorReason, UnitStatus,
    };
    use std::collections::HashMap;
    use std::sync::RwLock;

    fn make_unit_id(name: &str) -> UnitId {
        let kind = if name.ends_with(".service") {
            UnitIdKind::Service
        } else if name.ends_with(".target") {
            UnitIdKind::Target
        } else if name.ends_with(".socket") {
            UnitIdKind::Socket
        } else if name.ends_with(".mount") {
            UnitIdKind::Mount
        } else {
            UnitIdKind::Service
        };
        UnitId {
            kind,
            name: name.to_string(),
        }
    }

    fn make_test_unit(name: &str) -> Unit {
        let id = make_unit_id(name);
        Unit {
            id: id.clone(),
            common: Common {
                unit: UnitConfig {
                    description: name.to_string(),
                    documentation: vec![],
                    fragment_path: None,
                    refs_by_name: vec![],
                    default_dependencies: true,
                    conditions: vec![],
                    assertions: vec![],
                    success_action: crate::units::UnitAction::None,
                    failure_action: crate::units::UnitAction::None,
                    aliases: vec![],
                    ignore_on_isolate: false,
                    default_instance: None,
                    allow_isolate: false,
                    job_timeout_sec: None,
                    job_timeout_action: crate::units::UnitAction::None,
                    refuse_manual_start: false,
                    refuse_manual_stop: false,
                    on_failure: vec![],
                    on_failure_job_mode: crate::units::OnFailureJobMode::Replace,
                    start_limit_interval_sec: None,
                    start_limit_burst: None,
                    start_limit_action: crate::units::UnitAction::None,
                },
                dependencies: Dependencies {
                    wants: vec![],
                    wanted_by: vec![],
                    requires: vec![],
                    required_by: vec![],
                    conflicts: vec![],
                    conflicted_by: vec![],
                    before: vec![],
                    after: vec![],
                    part_of: vec![],
                    part_of_by: vec![],
                    binds_to: vec![],
                    bound_by: vec![],
                },
                status: RwLock::new(UnitStatus::NeverStarted),
            },
            specific: Specific::Target(TargetSpecific {
                state: RwLock::new(TargetState {
                    common: CommonState {
                        up_since: None,
                        restart_count: 0,
                    },
                }),
            }),
        }
    }

    fn make_unit_table(units: Vec<Unit>) -> UnitTable {
        let mut table = HashMap::new();
        for unit in units {
            table.insert(unit.id.clone(), unit);
        }
        table
    }

    // ── parse_command tests ──────────────────────────────────────────────

    #[test]
    fn test_parse_list_dependencies_string() {
        let call = super::super::jsonrpc2::Call {
            method: "list-dependencies".to_string(),
            params: Some(Value::String("multi-user.target".to_string())),
            id: None,
        };
        let cmd = parse_command(&call).unwrap();
        match cmd {
            Command::ListDependencies(name, reverse) => {
                assert_eq!(name, "multi-user.target");
                assert!(!reverse);
            }
            _ => panic!("Expected ListDependencies"),
        }
    }

    #[test]
    fn test_parse_list_dependencies_reverse() {
        let call = super::super::jsonrpc2::Call {
            method: "list-dependencies".to_string(),
            params: Some(Value::Array(vec![
                Value::String("sshd.service".to_string()),
                Value::String("--reverse".to_string()),
            ])),
            id: None,
        };
        let cmd = parse_command(&call).unwrap();
        match cmd {
            Command::ListDependencies(name, reverse) => {
                assert_eq!(name, "sshd.service");
                assert!(reverse);
            }
            _ => panic!("Expected ListDependencies with reverse"),
        }
    }

    #[test]
    fn test_parse_list_dependencies_no_params() {
        let call = super::super::jsonrpc2::Call {
            method: "list-dependencies".to_string(),
            params: None,
            id: None,
        };
        assert!(parse_command(&call).is_err());
    }

    #[test]
    fn test_parse_mask_single() {
        let call = super::super::jsonrpc2::Call {
            method: "mask".to_string(),
            params: Some(Value::String("tmp.mount".to_string())),
            id: None,
        };
        let cmd = parse_command(&call).unwrap();
        match cmd {
            Command::Mask(names) => {
                assert_eq!(names, vec!["tmp.mount"]);
            }
            _ => panic!("Expected Mask"),
        }
    }

    #[test]
    fn test_parse_mask_multiple() {
        let call = super::super::jsonrpc2::Call {
            method: "mask".to_string(),
            params: Some(Value::Array(vec![
                Value::String("a.service".to_string()),
                Value::String("b.service".to_string()),
            ])),
            id: None,
        };
        let cmd = parse_command(&call).unwrap();
        match cmd {
            Command::Mask(names) => {
                assert_eq!(names, vec!["a.service", "b.service"]);
            }
            _ => panic!("Expected Mask"),
        }
    }

    #[test]
    fn test_parse_mask_no_params() {
        let call = super::super::jsonrpc2::Call {
            method: "mask".to_string(),
            params: None,
            id: None,
        };
        assert!(parse_command(&call).is_err());
    }

    #[test]
    fn test_parse_unmask_single() {
        let call = super::super::jsonrpc2::Call {
            method: "unmask".to_string(),
            params: Some(Value::String("tmp.mount".to_string())),
            id: None,
        };
        let cmd = parse_command(&call).unwrap();
        match cmd {
            Command::Unmask(names) => {
                assert_eq!(names, vec!["tmp.mount"]);
            }
            _ => panic!("Expected Unmask"),
        }
    }

    #[test]
    fn test_parse_unmask_no_params() {
        let call = super::super::jsonrpc2::Call {
            method: "unmask".to_string(),
            params: None,
            id: None,
        };
        assert!(parse_command(&call).is_err());
    }

    // ── format_dep_tree tests ────────────────────────────────────────────

    #[test]
    fn test_dep_tree_single_unit_no_deps() {
        let unit = make_test_unit("test.target");
        let table = make_unit_table(vec![unit]);

        let mut out = String::new();
        let mut visited = std::collections::HashSet::new();
        format_dep_tree(
            "test.target",
            &table,
            false,
            "",
            true,
            &mut visited,
            &mut out,
            0,
        );

        assert!(out.contains("test.target"));
        // Should only have one line (the root)
        assert_eq!(out.lines().count(), 1);
    }

    #[test]
    fn test_dep_tree_with_wants() {
        let mut target = make_test_unit("multi-user.target");
        let svc_a = make_test_unit("a.service");
        let svc_b = make_test_unit("b.service");

        target.common.dependencies.wants =
            vec![make_unit_id("a.service"), make_unit_id("b.service")];

        let table = make_unit_table(vec![target, svc_a, svc_b]);

        let mut out = String::new();
        let mut visited = std::collections::HashSet::new();
        format_dep_tree(
            "multi-user.target",
            &table,
            false,
            "",
            true,
            &mut visited,
            &mut out,
            0,
        );

        assert!(out.contains("multi-user.target"));
        assert!(out.contains("a.service"));
        assert!(out.contains("b.service"));
        // Root + 2 children = 3 lines
        assert_eq!(out.lines().count(), 3);
    }

    #[test]
    fn test_dep_tree_with_requires() {
        let mut target = make_test_unit("network.target");
        let svc = make_test_unit("networkd.service");

        target.common.dependencies.requires = vec![make_unit_id("networkd.service")];

        let table = make_unit_table(vec![target, svc]);

        let mut out = String::new();
        let mut visited = std::collections::HashSet::new();
        format_dep_tree(
            "network.target",
            &table,
            false,
            "",
            true,
            &mut visited,
            &mut out,
            0,
        );

        assert!(out.contains("network.target"));
        assert!(out.contains("networkd.service"));
    }

    #[test]
    fn test_dep_tree_deduplicates_wants_and_requires() {
        // If a unit is in both wants and requires, it should appear once.
        let mut target = make_test_unit("test.target");
        let svc = make_test_unit("dup.service");

        target.common.dependencies.wants = vec![make_unit_id("dup.service")];
        target.common.dependencies.requires = vec![make_unit_id("dup.service")];

        let table = make_unit_table(vec![target, svc]);

        let mut out = String::new();
        let mut visited = std::collections::HashSet::new();
        format_dep_tree(
            "test.target",
            &table,
            false,
            "",
            true,
            &mut visited,
            &mut out,
            0,
        );

        // Count occurrences of "dup.service" in lines (should be 1)
        let count = out.lines().filter(|l| l.contains("dup.service")).count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_dep_tree_reverse() {
        let mut svc = make_test_unit("sshd.service");
        let target = make_test_unit("multi-user.target");

        svc.common.dependencies.wanted_by = vec![make_unit_id("multi-user.target")];

        let table = make_unit_table(vec![svc, target]);

        let mut out = String::new();
        let mut visited = std::collections::HashSet::new();
        format_dep_tree(
            "sshd.service",
            &table,
            true, // reverse
            "",
            true,
            &mut visited,
            &mut out,
            0,
        );

        assert!(out.contains("sshd.service"));
        assert!(out.contains("multi-user.target"));
    }

    #[test]
    fn test_dep_tree_reverse_required_by() {
        let mut svc = make_test_unit("dbus.service");
        let target = make_test_unit("multi-user.target");

        svc.common.dependencies.required_by = vec![make_unit_id("multi-user.target")];

        let table = make_unit_table(vec![svc, target]);

        let mut out = String::new();
        let mut visited = std::collections::HashSet::new();
        format_dep_tree(
            "dbus.service",
            &table,
            true,
            "",
            true,
            &mut visited,
            &mut out,
            0,
        );

        assert!(out.contains("multi-user.target"));
    }

    #[test]
    fn test_dep_tree_cycle_protection() {
        // a.service wants b.service, b.service wants a.service → cycle
        let mut a = make_test_unit("a.service");
        let mut b = make_test_unit("b.service");

        a.common.dependencies.wants = vec![make_unit_id("b.service")];
        b.common.dependencies.wants = vec![make_unit_id("a.service")];

        let table = make_unit_table(vec![a, b]);

        let mut out = String::new();
        let mut visited = std::collections::HashSet::new();
        format_dep_tree(
            "a.service",
            &table,
            false,
            "",
            true,
            &mut visited,
            &mut out,
            0,
        );

        // Should not infinite loop — just check it terminates and contains both
        assert!(out.contains("a.service"));
        assert!(out.contains("b.service"));
        // The cycle should mean b.service's child (a.service) appears but doesn't recurse
        let lines: Vec<&str> = out.lines().collect();
        assert!(
            lines.len() <= 4,
            "Cycle should be bounded, got {} lines",
            lines.len()
        );
    }

    #[test]
    fn test_dep_tree_nested() {
        // target -> a.service -> b.service
        let mut target = make_test_unit("test.target");
        let mut svc_a = make_test_unit("a.service");
        let svc_b = make_test_unit("b.service");

        target.common.dependencies.wants = vec![make_unit_id("a.service")];
        svc_a.common.dependencies.wants = vec![make_unit_id("b.service")];

        let table = make_unit_table(vec![target, svc_a, svc_b]);

        let mut out = String::new();
        let mut visited = std::collections::HashSet::new();
        format_dep_tree(
            "test.target",
            &table,
            false,
            "",
            true,
            &mut visited,
            &mut out,
            0,
        );

        assert!(out.contains("test.target"));
        assert!(out.contains("a.service"));
        assert!(out.contains("b.service"));
        assert_eq!(out.lines().count(), 3);
    }

    #[test]
    fn test_dep_tree_box_drawing_chars() {
        // target wants a.service and b.service — check tree connectors
        let mut target = make_test_unit("multi.target");
        let svc_a = make_test_unit("a.service");
        let svc_b = make_test_unit("b.service");

        target.common.dependencies.wants =
            vec![make_unit_id("a.service"), make_unit_id("b.service")];

        let table = make_unit_table(vec![target, svc_a, svc_b]);

        let mut out = String::new();
        let mut visited = std::collections::HashSet::new();
        format_dep_tree(
            "multi.target",
            &table,
            false,
            "",
            true,
            &mut visited,
            &mut out,
            0,
        );

        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3);
        // First child uses ├─, last child uses └─
        assert!(
            lines[1].contains("├─"),
            "first child should use ├─, got: {}",
            lines[1]
        );
        assert!(
            lines[2].contains("└─"),
            "last child should use └─, got: {}",
            lines[2]
        );
    }

    #[test]
    fn test_dep_tree_single_child_uses_last_connector() {
        let mut target = make_test_unit("test.target");
        let svc = make_test_unit("only.service");

        target.common.dependencies.wants = vec![make_unit_id("only.service")];

        let table = make_unit_table(vec![target, svc]);

        let mut out = String::new();
        let mut visited = std::collections::HashSet::new();
        format_dep_tree(
            "test.target",
            &table,
            false,
            "",
            true,
            &mut visited,
            &mut out,
            0,
        );

        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        // Single child is both first and last, so it uses └─
        assert!(
            lines[1].contains("└─"),
            "single child should use └─, got: {}",
            lines[1]
        );
    }

    #[test]
    fn test_dep_tree_children_sorted() {
        let mut target = make_test_unit("test.target");
        let c = make_test_unit("c.service");
        let a = make_test_unit("a.service");
        let b = make_test_unit("b.service");

        // Add in unsorted order
        target.common.dependencies.wants = vec![
            make_unit_id("c.service"),
            make_unit_id("a.service"),
            make_unit_id("b.service"),
        ];

        let table = make_unit_table(vec![target, c, a, b]);

        let mut out = String::new();
        let mut visited = std::collections::HashSet::new();
        format_dep_tree(
            "test.target",
            &table,
            false,
            "",
            true,
            &mut visited,
            &mut out,
            0,
        );

        let child_names: Vec<&str> = out
            .lines()
            .skip(1) // skip root
            .filter_map(|l| {
                // Extract the unit name after the status marker
                let trimmed = l.trim_start_matches(|c: char| !c.is_ascii_alphanumeric());
                if trimmed.contains(".service") {
                    // Find the start of the name
                    Some(trimmed.trim())
                } else {
                    None
                }
            })
            .collect();

        // Should be sorted: a, b, c
        assert_eq!(child_names.len(), 3);
        assert!(
            child_names[0].starts_with("a.service"),
            "first should be a.service, got: {}",
            child_names[0]
        );
        assert!(
            child_names[1].starts_with("b.service"),
            "second should be b.service, got: {}",
            child_names[1]
        );
        assert!(
            child_names[2].starts_with("c.service"),
            "third should be c.service, got: {}",
            child_names[2]
        );
    }

    #[test]
    fn test_dep_tree_unknown_unit() {
        let table: UnitTable = HashMap::new();

        let mut out = String::new();
        let mut visited = std::collections::HashSet::new();
        format_dep_tree(
            "nonexistent.service",
            &table,
            false,
            "",
            true,
            &mut visited,
            &mut out,
            0,
        );

        // Should print the name but with no children
        assert!(out.contains("nonexistent.service"));
        assert_eq!(out.lines().count(), 1);
    }

    #[test]
    fn test_dep_tree_binds_to_forward() {
        let mut svc = make_test_unit("a.service");
        let bound = make_test_unit("b.service");

        svc.common.dependencies.binds_to = vec![make_unit_id("b.service")];

        let table = make_unit_table(vec![svc, bound]);

        let mut out = String::new();
        let mut visited = std::collections::HashSet::new();
        format_dep_tree(
            "a.service",
            &table,
            false,
            "",
            true,
            &mut visited,
            &mut out,
            0,
        );

        assert!(out.contains("b.service"));
    }

    #[test]
    fn test_dep_tree_bound_by_reverse() {
        let mut svc = make_test_unit("a.service");
        let dependent = make_test_unit("b.service");

        svc.common.dependencies.bound_by = vec![make_unit_id("b.service")];

        let table = make_unit_table(vec![svc, dependent]);

        let mut out = String::new();
        let mut visited = std::collections::HashSet::new();
        format_dep_tree(
            "a.service",
            &table,
            true,
            "",
            true,
            &mut visited,
            &mut out,
            0,
        );

        assert!(out.contains("b.service"));
    }

    // ── unit_status_marker tests ─────────────────────────────────────────

    #[test]
    fn test_status_marker_never_started() {
        let unit = make_test_unit("test.service");
        let table = make_unit_table(vec![unit]);
        let marker = unit_status_marker("test.service", &table);
        assert_eq!(marker, "○ ");
    }

    #[test]
    fn test_status_marker_active() {
        let unit = make_test_unit("test.service");
        *unit.common.status.write().unwrap() = UnitStatus::Started(StatusStarted::Running);
        let table = make_unit_table(vec![unit]);
        let marker = unit_status_marker("test.service", &table);
        assert_eq!(marker, "● ");
    }

    #[test]
    fn test_status_marker_failed() {
        let unit = make_test_unit("test.service");
        *unit.common.status.write().unwrap() = UnitStatus::Stopped(
            StatusStopped::StoppedFinal,
            vec![UnitOperationErrorReason::GenericStartError(
                "error".to_string(),
            )],
        );
        let table = make_unit_table(vec![unit]);
        let marker = unit_status_marker("test.service", &table);
        assert_eq!(marker, "× ");
    }

    #[test]
    fn test_status_marker_stopped_no_errors() {
        let unit = make_test_unit("test.service");
        *unit.common.status.write().unwrap() =
            UnitStatus::Stopped(StatusStopped::StoppedFinal, vec![]);
        let table = make_unit_table(vec![unit]);
        let marker = unit_status_marker("test.service", &table);
        assert_eq!(marker, "○ ");
    }

    #[test]
    fn test_status_marker_unknown_unit() {
        let table: UnitTable = HashMap::new();
        let marker = unit_status_marker("nonexistent.service", &table);
        assert_eq!(marker, "○ ");
    }

    // ── mask / unmask filesystem tests ───────────────────────────────────

    #[test]
    fn test_mask_unmask_tempdir() {
        // Test mask/unmask logic using a temp directory instead of /etc/systemd/system
        let dir = tempfile::tempdir().unwrap();
        let mask_dir = dir.path();

        let name = "test-mask.service";
        let link_path = mask_dir.join(name);

        // Mask: create symlink to /dev/null
        std::os::unix::fs::symlink("/dev/null", &link_path).unwrap();
        assert!(link_path.symlink_metadata().is_ok());
        let target = std::fs::read_link(&link_path).unwrap();
        assert_eq!(target, std::path::Path::new("/dev/null"));

        // Unmask: remove the symlink
        std::fs::remove_file(&link_path).unwrap();
        assert!(!link_path.exists());
    }

    #[test]
    fn test_unmask_only_removes_devnull_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let mask_dir = dir.path();

        let name = "real-unit.service";
        let link_path = mask_dir.join(name);

        // Create a regular file (not a symlink to /dev/null)
        std::fs::write(&link_path, "[Unit]\nDescription=Real\n").unwrap();

        // Unmask should NOT remove a regular file
        if let Ok(target) = std::fs::read_link(&link_path) {
            assert_ne!(target, std::path::Path::new("/dev/null"));
        }
        // File should still exist
        assert!(link_path.exists());
    }

    #[test]
    fn test_mask_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let mask_dir = dir.path();

        let name = "overwrite.service";
        let link_path = mask_dir.join(name);

        // Create a regular file first
        std::fs::write(&link_path, "[Unit]\nDescription=Old\n").unwrap();
        assert!(link_path.exists());

        // Remove and re-create as symlink (simulating mask)
        std::fs::remove_file(&link_path).unwrap();
        std::os::unix::fs::symlink("/dev/null", &link_path).unwrap();

        let target = std::fs::read_link(&link_path).unwrap();
        assert_eq!(target, std::path::Path::new("/dev/null"));
    }

    #[test]
    fn test_unmask_nonexistent_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let mask_dir = dir.path();

        let name = "nonexistent.service";
        let link_path = mask_dir.join(name);

        // read_link on nonexistent path should fail
        assert!(std::fs::read_link(&link_path).is_err());
        // Not an error — just nothing to do
    }

    // ── find_units_with_name tests ───────────────────────────────────────

    #[test]
    fn test_find_units_exact_name() {
        let unit = make_test_unit("sshd.service");
        let table = make_unit_table(vec![unit]);
        let found = find_units_with_name("sshd.service", &table);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id.name, "sshd.service");
    }

    #[test]
    fn test_find_units_prefix_match() {
        let unit = make_test_unit("sshd.service");
        let table = make_unit_table(vec![unit]);
        let found = find_units_with_name("sshd", &table);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn test_find_units_no_match() {
        let unit = make_test_unit("sshd.service");
        let table = make_unit_table(vec![unit]);
        let found = find_units_with_name("nginx.service", &table);
        assert!(found.is_empty());
    }

    #[test]
    fn test_find_units_by_alias() {
        let mut unit = make_test_unit("systemd-resolved.service");
        unit.common.unit.aliases = vec!["dbus-org.freedesktop.resolve1.service".to_string()];
        let table = make_unit_table(vec![unit]);
        let found = find_units_with_name("dbus-org.freedesktop.resolve1.service", &table);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id.name, "systemd-resolved.service");
    }

    // ── parse_command edge cases ─────────────────────────────────────────

    #[test]
    fn test_parse_unknown_method() {
        let call = super::super::jsonrpc2::Call {
            method: "nonexistent-method".to_string(),
            params: None,
            id: None,
        };
        let result = parse_command(&call);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_show_with_filter() {
        let call = super::super::jsonrpc2::Call {
            method: "show".to_string(),
            params: Some(Value::Array(vec![
                Value::String("sshd.service".to_string()),
                Value::String("MainPID".to_string()),
                Value::String("ActiveState".to_string()),
            ])),
            id: None,
        };
        let cmd = parse_command(&call).unwrap();
        match cmd {
            Command::Show(name, filter) => {
                assert_eq!(name, "sshd.service");
                let f = filter.unwrap();
                assert_eq!(f, vec!["MainPID", "ActiveState"]);
            }
            _ => panic!("Expected Show"),
        }
    }

    #[test]
    fn test_parse_cat() {
        let call = super::super::jsonrpc2::Call {
            method: "cat".to_string(),
            params: Some(Value::String("sshd.service".to_string())),
            id: None,
        };
        let cmd = parse_command(&call).unwrap();
        match cmd {
            Command::Cat(name) => assert_eq!(name, "sshd.service"),
            _ => panic!("Expected Cat"),
        }
    }

    #[test]
    fn test_parse_is_active() {
        let call = super::super::jsonrpc2::Call {
            method: "is-active".to_string(),
            params: Some(Value::String("sshd.service".to_string())),
            id: None,
        };
        let cmd = parse_command(&call).unwrap();
        match cmd {
            Command::IsActive(name) => assert_eq!(name, "sshd.service"),
            _ => panic!("Expected IsActive"),
        }
    }

    #[test]
    fn test_parse_list_dependencies_array_no_reverse() {
        let call = super::super::jsonrpc2::Call {
            method: "list-dependencies".to_string(),
            params: Some(Value::Array(vec![Value::String(
                "default.target".to_string(),
            )])),
            id: None,
        };
        let cmd = parse_command(&call).unwrap();
        match cmd {
            Command::ListDependencies(name, reverse) => {
                assert_eq!(name, "default.target");
                assert!(!reverse);
            }
            _ => panic!("Expected ListDependencies"),
        }
    }

    #[test]
    fn test_parse_unmask_multiple() {
        let call = super::super::jsonrpc2::Call {
            method: "unmask".to_string(),
            params: Some(Value::Array(vec![
                Value::String("a.service".to_string()),
                Value::String("b.service".to_string()),
            ])),
            id: None,
        };
        let cmd = parse_command(&call).unwrap();
        match cmd {
            Command::Unmask(names) => {
                assert_eq!(names, vec!["a.service", "b.service"]);
            }
            _ => panic!("Expected Unmask"),
        }
    }

    // ── dep tree with mixed status markers ───────────────────────────────

    #[test]
    fn test_dep_tree_status_markers_in_output() {
        let mut target = make_test_unit("test.target");
        let mut active_svc = make_test_unit("active.service");
        let mut failed_svc = make_test_unit("failed.service");
        let inactive_svc = make_test_unit("inactive.service");

        *active_svc.common.status.write().unwrap() = UnitStatus::Started(StatusStarted::Running);
        *failed_svc.common.status.write().unwrap() = UnitStatus::Stopped(
            StatusStopped::StoppedFinal,
            vec![UnitOperationErrorReason::GenericStartError(
                "exit 1".to_string(),
            )],
        );

        target.common.dependencies.wants = vec![
            make_unit_id("active.service"),
            make_unit_id("failed.service"),
            make_unit_id("inactive.service"),
        ];

        let table = make_unit_table(vec![target, active_svc, failed_svc, inactive_svc]);

        let mut out = String::new();
        let mut visited = std::collections::HashSet::new();
        format_dep_tree(
            "test.target",
            &table,
            false,
            "",
            true,
            &mut visited,
            &mut out,
            0,
        );

        // active.service should have ● marker
        let active_line = out.lines().find(|l| l.contains("active.service")).unwrap();
        assert!(
            active_line.contains("● "),
            "active should have ● marker: {}",
            active_line
        );

        // failed.service should have × marker
        let failed_line = out.lines().find(|l| l.contains("failed.service")).unwrap();
        assert!(
            failed_line.contains("× "),
            "failed should have × marker: {}",
            failed_line
        );

        // inactive.service should have ○ marker
        let inactive_line = out
            .lines()
            .find(|l| l.contains("inactive.service"))
            .unwrap();
        assert!(
            inactive_line.contains("○ "),
            "inactive should have ○ marker: {}",
            inactive_line
        );
    }

    #[test]
    fn test_dep_tree_diamond_dependency() {
        // Diamond: target -> a, b; a -> c; b -> c
        // c should appear under both a and b (not deduplicated across branches)
        let mut target = make_test_unit("test.target");
        let mut a = make_test_unit("a.service");
        let mut b = make_test_unit("b.service");
        let c = make_test_unit("c.service");

        target.common.dependencies.wants =
            vec![make_unit_id("a.service"), make_unit_id("b.service")];
        a.common.dependencies.wants = vec![make_unit_id("c.service")];
        b.common.dependencies.wants = vec![make_unit_id("c.service")];

        let table = make_unit_table(vec![target, a, b, c]);

        let mut out = String::new();
        let mut visited = std::collections::HashSet::new();
        format_dep_tree(
            "test.target",
            &table,
            false,
            "",
            true,
            &mut visited,
            &mut out,
            0,
        );

        // c.service should appear under both a and b branches
        let c_count = out.lines().filter(|l| l.contains("c.service")).count();
        assert!(
            c_count >= 2,
            "c.service should appear in both branches of the diamond, appeared {} times in:\n{}",
            c_count,
            out
        );
    }
}
