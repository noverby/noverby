mod dependency_resolving;
pub use dependency_resolving::*;
use log::{trace, warn};

use crate::runtime_info::UnitTable;
use crate::units::{
    get_file_list, parse_file, parse_service, parse_socket, parse_target, ParsingError,
    ParsingErrorReason, Specific, Unit, UnitId,
};

use std::collections::HashMap;
use std::convert::TryInto;
use std::path::PathBuf;

#[derive(Debug)]
pub enum LoadingError {
    Parsing(ParsingError),
    Dependency(DependencyError),
}

#[derive(Debug)]
pub struct DependencyError {
    msg: String,
}

impl std::convert::From<String> for DependencyError {
    fn from(s: String) -> Self {
        Self { msg: s }
    }
}

impl std::fmt::Display for DependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Dependency resolving error: {}", self.msg)
    }
}

impl std::convert::From<DependencyError> for LoadingError {
    fn from(s: DependencyError) -> Self {
        Self::Dependency(s)
    }
}

impl std::convert::From<ParsingError> for LoadingError {
    fn from(s: ParsingError) -> Self {
        Self::Parsing(s)
    }
}

pub fn load_all_units(
    paths: &[PathBuf],
    target_unit: &str,
) -> Result<HashMap<UnitId, Unit>, LoadingError> {
    let mut service_unit_table = HashMap::new();
    let mut socket_unit_table = HashMap::new();
    let mut target_unit_table = HashMap::new();
    for path in paths {
        parse_all_units(
            &mut service_unit_table,
            &mut socket_unit_table,
            &mut target_unit_table,
            path,
        )?;
    }

    let mut unit_table = std::collections::HashMap::new();
    unit_table.extend(service_unit_table);
    unit_table.extend(socket_unit_table);
    unit_table.extend(target_unit_table);

    trace!("Units found: {}", unit_table.len());

    fill_dependencies(&mut unit_table).map_err(|e| LoadingError::Dependency(e.into()))?;

    prune_units(target_unit, &mut unit_table).unwrap();
    trace!("Finished pruning units");

    let removed_ids = prune_unused_sockets(&mut unit_table);
    trace!("Finished pruning sockets");

    cleanup_removed_ids(&mut unit_table, &removed_ids);

    Ok(unit_table)
}

fn cleanup_removed_ids(
    units: &mut std::collections::HashMap<UnitId, Unit>,
    removed_ids: &Vec<UnitId>,
) {
    for unit in units.values_mut() {
        for id in removed_ids {
            unit.common.dependencies.remove_id(id);
        }
    }
}

fn prune_unused_sockets(sockets: &mut UnitTable) -> Vec<UnitId> {
    let mut ids_to_remove = Vec::new();
    for unit in sockets.values() {
        if let Specific::Socket(sock) = &unit.specific {
            if sock.conf.services.is_empty() {
                trace!(
                    "Prune socket {} because it was not added to any service",
                    unit.id.name
                );
                ids_to_remove.push(unit.id.clone());
            }
        }
    }
    for id in &ids_to_remove {
        sockets.remove(id);
    }
    ids_to_remove
}

fn parse_all_units(
    services: &mut std::collections::HashMap<UnitId, Unit>,
    sockets: &mut std::collections::HashMap<UnitId, Unit>,
    targets: &mut std::collections::HashMap<UnitId, Unit>,
    path: &PathBuf,
) -> Result<(), ParsingError> {
    let files = get_file_list(path).map_err(|e| ParsingError::new(e, path.clone()))?;
    for entry in files {
        if entry.path().is_dir() {
            parse_all_units(services, sockets, targets, &entry.path())?;
        } else {
            let raw = match std::fs::read_to_string(entry.path()) {
                Ok(raw) => raw,
                Err(e) => {
                    warn!(
                        "Skipping unit {:?}: could not read file: {}",
                        entry.path(),
                        e
                    );
                    continue;
                }
            };

            let parsed_file = match parse_file(&raw) {
                Ok(pf) => pf,
                Err(e) => {
                    warn!(
                        "Skipping unit {:?}: could not parse file: {:?}",
                        entry.path(),
                        e
                    );
                    continue;
                }
            };

            if entry.path().to_str().unwrap().ends_with(".service") {
                trace!("Service found: {:?}", entry.path());
                match parse_service(parsed_file, &entry.path()).and_then(|parsed| {
                    TryInto::<Unit>::try_into(parsed).map_err(ParsingErrorReason::Generic)
                }) {
                    Ok(unit) => {
                        services.insert(unit.id.clone(), unit);
                    }
                    Err(e) => {
                        warn!("Skipping service {:?}: {:?}", entry.path(), e);
                    }
                }
            } else if entry.path().to_str().unwrap().ends_with(".socket") {
                trace!("Socket found: {:?}", entry.path());
                match parse_socket(parsed_file, &entry.path()).and_then(|parsed| {
                    TryInto::<Unit>::try_into(parsed).map_err(ParsingErrorReason::Generic)
                }) {
                    Ok(unit) => {
                        sockets.insert(unit.id.clone(), unit);
                    }
                    Err(e) => {
                        warn!("Skipping socket {:?}: {:?}", entry.path(), e);
                    }
                }
            } else if entry.path().to_str().unwrap().ends_with(".target") {
                trace!("Target found: {:?}", entry.path());
                match parse_target(parsed_file, &entry.path()).and_then(|parsed| {
                    TryInto::<Unit>::try_into(parsed).map_err(ParsingErrorReason::Generic)
                }) {
                    Ok(unit) => {
                        targets.insert(unit.id.clone(), unit);
                    }
                    Err(e) => {
                        warn!("Skipping target {:?}: {:?}", entry.path(), e);
                    }
                }
            }
        }
    }
    Ok(())
}
