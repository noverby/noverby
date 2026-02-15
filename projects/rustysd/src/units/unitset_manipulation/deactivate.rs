use log::trace;

use crate::runtime_info::RuntimeInfo;
use crate::units::{UnitId, UnitOperationError, UnitOperationErrorReason};

pub fn deactivate_unit_recursive(
    id_to_kill: &UnitId,
    run_info: &RuntimeInfo,
) -> Result<(), UnitOperationError> {
    let Some(unit) = run_info.unit_table.get(id_to_kill) else {
        // If this occurs, there is a flaw in the handling of dependencies
        // IDs should be purged globally when units get removed
        return Err(UnitOperationError {
            reason: UnitOperationErrorReason::GenericStartError(
                "Tried to activate a unit that can not be found".into(),
            ),
            unit_name: id_to_kill.name.clone(),
            unit_id: id_to_kill.clone(),
        });
    };

    deactivate_units_recursive(&unit.common.dependencies.required_by, run_info)?;
    // PartOf= stop propagation: units that declared PartOf= this unit
    // should also be stopped when this unit is stopped.
    deactivate_units_recursive(&unit.common.dependencies.part_of_by, run_info)?;

    deactivate_unit(id_to_kill, run_info)
}

pub fn deactivate_unit(
    id_to_kill: &UnitId,
    run_info: &RuntimeInfo,
) -> Result<(), UnitOperationError> {
    let Some(unit) = run_info.unit_table.get(id_to_kill) else {
        // If this occurs, there is a flaw in the handling of dependencies
        // IDs should be purged globally when units get removed
        return Err(UnitOperationError {
            reason: UnitOperationErrorReason::GenericStartError(
                "Tried to activate a unit that can not be found".into(),
            ),
            unit_name: id_to_kill.name.clone(),
            unit_id: id_to_kill.clone(),
        });
    };
    unit.deactivate(run_info)?;

    Ok(())
}

pub fn deactivate_units_recursive(
    ids_to_kill: &[UnitId],
    run_info: &RuntimeInfo,
) -> Result<(), UnitOperationError> {
    for id in ids_to_kill {
        deactivate_unit_recursive(id, run_info)?;
    }
    Ok(())
}

pub fn deactivate_units(
    ids_to_kill: &[UnitId],
    run_info: &RuntimeInfo,
) -> Result<(), UnitOperationError> {
    for id in ids_to_kill {
        deactivate_unit(id, run_info)?;
    }
    Ok(())
}

pub fn reactivate_unit(
    id_to_restart: UnitId,
    run_info: &RuntimeInfo,
) -> std::result::Result<(), UnitOperationError> {
    trace!("Reactivation of unit: {id_to_restart:?}. Deactivate");
    let Some(unit) = run_info.unit_table.get(&id_to_restart) else {
        // If this occurs, there is a flaw in the handling of dependencies
        // IDs should be purged globally when units get removed
        return Err(UnitOperationError {
            reason: UnitOperationErrorReason::GenericStartError(
                "Tried to activate a unit that can not be found".into(),
            ),
            unit_name: id_to_restart.name.clone(),
            unit_id: id_to_restart,
        });
    };
    unit.reactivate(run_info, crate::units::ActivationSource::Regular)
}
