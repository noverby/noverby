use log::{error, info, trace};

use crate::runtime_info::RuntimeInfo;
use crate::services::Service;
use crate::sockets::{Socket, SocketKind, SpecializedSocketConfig};
use crate::units::{
    ActivationSource, Commandline, DeferTrigger, Delegate, DevicePolicy, EnvVars,
    FileDescriptorStorePreserve, IOSchedulingClass, KeyringMode, KillMode, MemoryLimit,
    MemoryPressureWatch, NotifyKind, OnFailureJobMode, ParsedMountSection, ProcSubset, ProtectHome,
    ProtectProc, ProtectSystem, ResourceLimit, RestrictNamespaces, RuntimeDirectoryPreserve,
    ServiceRestart, ServiceType, StandardInput, StatusStarted, StatusStopped, StdIoOption,
    TasksMax, Timeout, Timestamping, UnitAction, UnitCondition, UnitId, UnitIdKind,
    UnitOperationError, UnitOperationErrorReason, UnitStatus, UtmpMode, acquire_locks,
};

use std::sync::RwLock;

/// A units has a common part that all units share, like dependencies and a description. The specific part containbs mutable state and
/// the unit-type specific configs
pub struct Unit {
    pub id: UnitId,
    pub common: Common,
    pub specific: Specific,
}

/// Common attributes of units
pub struct Common {
    pub unit: UnitConfig,
    pub dependencies: Dependencies,
    pub status: RwLock<UnitStatus>,
}

/// Different unit-types have different configs and state
pub enum Specific {
    Service(ServiceSpecific),
    Socket(SocketSpecific),
    Target(TargetSpecific),
    Slice(SliceSpecific),
    Mount(MountSpecific),
}

pub struct ServiceSpecific {
    pub conf: ServiceConfig,
    pub state: RwLock<ServiceState>,
}

impl SocketState {
    fn activate(
        &mut self,
        id: &UnitId,
        conf: &SocketConfig,
        status: &RwLock<UnitStatus>,
        run_info: &RuntimeInfo,
    ) -> Result<UnitStatus, UnitOperationError> {
        let open_res = self
            .sock
            .open_all(
                conf,
                id.name.clone(),
                id.clone(),
                &mut run_info.fd_store.write().unwrap(),
            )
            .map_err(|e| UnitOperationError {
                unit_name: id.name.clone(),
                unit_id: id.clone(),
                reason: UnitOperationErrorReason::SocketOpenError(format!("{e}")),
            });
        match open_res {
            Ok(()) => {
                let mut status = status.write().unwrap();
                *status = UnitStatus::Started(StatusStarted::Running);
                run_info.notify_eventfds();
                Ok(UnitStatus::Started(StatusStarted::Running))
            }
            Err(e) => {
                let mut status = status.write().unwrap();
                *status =
                    UnitStatus::Stopped(StatusStopped::StoppedUnexpected, vec![e.reason.clone()]);
                Err(e)
            }
        }
    }

    fn deactivate(
        &mut self,
        id: &UnitId,
        conf: &SocketConfig,
        status: &RwLock<UnitStatus>,
        run_info: &RuntimeInfo,
    ) -> Result<(), UnitOperationError> {
        let close_result = self
            .sock
            .close_all(
                conf,
                id.name.clone(),
                &mut run_info.fd_store.write().unwrap(),
            )
            .map_err(|e| UnitOperationError {
                unit_name: id.name.clone(),
                unit_id: id.clone(),
                reason: UnitOperationErrorReason::SocketCloseError(e),
            });
        match &close_result {
            Ok(()) => {
                let mut status = status.write().unwrap();
                *status = UnitStatus::Stopped(StatusStopped::StoppedFinal, vec![]);
            }
            Err(e) => {
                let mut status = status.write().unwrap();
                *status = UnitStatus::Stopped(StatusStopped::StoppedFinal, vec![e.reason.clone()]);
            }
        }
        close_result
    }

    fn reactivate(
        &mut self,
        id: &UnitId,
        conf: &SocketConfig,
        status: &RwLock<UnitStatus>,
        run_info: &RuntimeInfo,
    ) -> Result<(), UnitOperationError> {
        let close_result = self
            .sock
            .close_all(
                conf,
                id.name.clone(),
                &mut run_info.fd_store.write().unwrap(),
            )
            .map_err(|e| UnitOperationError {
                unit_name: id.name.clone(),
                unit_id: id.clone(),
                reason: UnitOperationErrorReason::SocketCloseError(e),
            });

        // If closing failed, dont try to restart but fail early
        if let Err(error) = close_result {
            let mut status = status.write().unwrap();
            *status = UnitStatus::Stopped(StatusStopped::StoppedFinal, vec![error.reason.clone()]);
            return Err(error);
        }

        // Reopen and set the status according to the result
        let open_res = self
            .sock
            .open_all(
                conf,
                id.name.clone(),
                id.clone(),
                &mut run_info.fd_store.write().unwrap(),
            )
            .map_err(|e| UnitOperationError {
                unit_name: id.name.clone(),
                unit_id: id.clone(),
                reason: UnitOperationErrorReason::SocketOpenError(format!("{e}")),
            });
        match open_res {
            Ok(()) => {
                let mut status = status.write().unwrap();
                *status = UnitStatus::Started(StatusStarted::Running);
                run_info.notify_eventfds();
                Ok(())
            }
            Err(e) => {
                let mut status = status.write().unwrap();
                *status =
                    UnitStatus::Stopped(StatusStopped::StoppedUnexpected, vec![e.reason.clone()]);
                Err(e)
            }
        }
    }
}

impl ServiceState {
    fn activate(
        &mut self,
        id: &UnitId,
        conf: &ServiceConfig,
        status: &RwLock<UnitStatus>,
        run_info: &RuntimeInfo,
        source: ActivationSource,
    ) -> Result<UnitStatus, UnitOperationError> {
        let start_res = self
            .srvc
            .start(conf, id.clone(), &id.name, run_info, source)
            .map_err(|e| UnitOperationError {
                unit_name: id.name.clone(),
                unit_id: id.clone(),
                reason: UnitOperationErrorReason::ServiceStartError(e),
            });
        match start_res {
            Ok(crate::services::StartResult::Started) => {
                {
                    let mut status = status.write().unwrap();
                    *status = UnitStatus::Started(StatusStarted::Running);
                }
                Ok(UnitStatus::Started(StatusStarted::Running))
            }
            Ok(crate::services::StartResult::WaitingForSocket) => {
                {
                    let mut status = status.write().unwrap();
                    *status = UnitStatus::Started(StatusStarted::WaitingForSocket);
                }
                // tell socket activation to listen to these sockets again
                for socket_id in &conf.sockets {
                    if let Some(unit) = run_info.unit_table.get(socket_id) {
                        if let Specific::Socket(sock) = &unit.specific {
                            let mut_state = &mut *sock.state.write().unwrap();
                            mut_state.sock.activated = false;
                        }
                    }
                }
                run_info.notify_eventfds();
                Ok(UnitStatus::Started(StatusStarted::WaitingForSocket))
            }
            Err(e) => {
                let mut status = status.write().unwrap();
                *status =
                    UnitStatus::Stopped(StatusStopped::StoppedUnexpected, vec![e.reason.clone()]);
                Err(e)
            }
        }
    }

    fn deactivate(
        &mut self,
        id: &UnitId,
        conf: &ServiceConfig,
        status: &RwLock<UnitStatus>,
        run_info: &RuntimeInfo,
    ) -> Result<(), UnitOperationError> {
        let kill_result = self
            .srvc
            .kill(conf, id.clone(), &id.name, run_info)
            .map_err(|e| UnitOperationError {
                unit_name: id.name.clone(),
                unit_id: id.clone(),
                reason: UnitOperationErrorReason::ServiceStopError(e),
            });
        match &kill_result {
            Ok(()) => {
                let mut status = status.write().unwrap();
                *status = UnitStatus::Stopped(StatusStopped::StoppedFinal, vec![]);
            }
            Err(e) => {
                let mut status = status.write().unwrap();
                *status = UnitStatus::Stopped(StatusStopped::StoppedFinal, vec![e.reason.clone()]);
            }
        }
        kill_result
    }
    fn reactivate(
        &mut self,
        id: &UnitId,
        conf: &ServiceConfig,
        status: &RwLock<UnitStatus>,
        run_info: &RuntimeInfo,
        source: ActivationSource,
    ) -> Result<(), UnitOperationError> {
        let kill_result = self
            .srvc
            .kill(conf, id.clone(), &id.name, run_info)
            .map_err(|e| UnitOperationError {
                unit_name: id.name.clone(),
                unit_id: id.clone(),
                reason: UnitOperationErrorReason::ServiceStopError(e),
            });

        // If killing failed, dont try to restart but fail early
        if let Err(error) = kill_result {
            let mut status = status.write().unwrap();
            *status = UnitStatus::Stopped(StatusStopped::StoppedFinal, vec![error.reason.clone()]);
            return Err(error);
        }

        // Restart and set the status according to the result
        let start_res = self
            .srvc
            .start(conf, id.clone(), &id.name, run_info, source)
            .map_err(|e| UnitOperationError {
                unit_name: id.name.clone(),
                unit_id: id.clone(),
                reason: UnitOperationErrorReason::ServiceStartError(e),
            });
        match start_res {
            Ok(crate::services::StartResult::Started) => {
                {
                    let mut status = status.write().unwrap();
                    *status = UnitStatus::Started(StatusStarted::Running);
                }
                Ok(())
            }
            Ok(crate::services::StartResult::WaitingForSocket) => {
                {
                    let mut status = status.write().unwrap();
                    *status = UnitStatus::Started(StatusStarted::WaitingForSocket);
                }
                // tell socket activation to listen to these sockets again
                for socket_id in &conf.sockets {
                    if let Some(unit) = run_info.unit_table.get(socket_id) {
                        if let Specific::Socket(sock) = &unit.specific {
                            let mut_state = &mut *sock.state.write().unwrap();
                            mut_state.sock.activated = false;
                        }
                    }
                }
                run_info.notify_eventfds();
                Ok(())
            }
            Err(e) => {
                let mut status = status.write().unwrap();
                *status =
                    UnitStatus::Stopped(StatusStopped::StoppedUnexpected, vec![e.reason.clone()]);
                Err(e)
            }
        }
    }
}

impl ServiceSpecific {
    pub fn has_socket(&self, socket: &str) -> bool {
        self.conf.sockets.iter().any(|id| id.eq(socket))
    }
}

pub struct SocketSpecific {
    pub conf: SocketConfig,
    pub state: RwLock<SocketState>,
}

impl SocketSpecific {
    pub fn belongs_to_service(&self, service: &str) -> bool {
        self.conf.services.iter().any(|id| id.eq(service))
    }
}

pub struct TargetSpecific {
    pub state: RwLock<TargetState>,
}

pub struct SliceSpecific {
    pub state: RwLock<SliceState>,
}

pub struct MountSpecific {
    pub conf: MountConfig,
    pub state: RwLock<MountState>,
}

#[derive(Default)]
/// All units have some common mutable state
pub struct CommonState {
    pub up_since: Option<std::time::Instant>,
    pub restart_count: u64,
}

pub struct ServiceState {
    pub common: CommonState,
    pub srvc: Service,
}
pub struct SocketState {
    pub common: CommonState,
    pub sock: Socket,
}
pub struct TargetState {
    pub common: CommonState,
}
pub struct SliceState {
    pub common: CommonState,
}

pub struct MountState {
    pub common: CommonState,
}

/// Configuration for a `.mount` unit, derived from the parsed `[Mount]` section.
#[derive(Debug, Clone)]
pub struct MountConfig {
    /// What= — the device, file, or resource to mount.
    pub what: String,
    /// Where= — the mount point path.
    pub where_: String,
    /// Type= — filesystem type (e.g. "ext4", "tmpfs").
    pub fs_type: Option<String>,
    /// Options= — comma-separated mount options.
    pub options: Option<String>,
    /// SloppyOptions= — tolerate unknown mount options.
    pub sloppy_options: bool,
    /// LazyUnmount= — use MNT_DETACH when unmounting.
    pub lazy_unmount: bool,
    /// ReadWriteOnly= — fail if can't mount read-write.
    pub read_write_only: bool,
    /// ForceUnmount= — use MNT_FORCE when unmounting.
    pub force_unmount: bool,
    /// DirectoryMode= — mode for auto-created mount point directory.
    pub directory_mode: u32,
    /// TimeoutSec= — mount operation timeout.
    pub timeout_sec: Option<u64>,
}

impl From<ParsedMountSection> for MountConfig {
    fn from(parsed: ParsedMountSection) -> Self {
        Self {
            what: parsed.what,
            where_: parsed.where_,
            fs_type: parsed.fs_type,
            options: parsed.options,
            sloppy_options: parsed.sloppy_options,
            lazy_unmount: parsed.lazy_unmount,
            read_write_only: parsed.read_write_only,
            force_unmount: parsed.force_unmount,
            directory_mode: parsed.directory_mode,
            timeout_sec: parsed.timeout_sec,
        }
    }
}

// Fields are held to keep RwLockWriteGuards alive during activate/deactivate/reactivate
#[allow(dead_code)]
enum LockedState<'a> {
    Service(
        std::sync::RwLockWriteGuard<'a, ServiceState>,
        &'a ServiceConfig,
    ),
    Socket(
        std::sync::RwLockWriteGuard<'a, SocketState>,
        &'a SocketConfig,
    ),
    Target(std::sync::RwLockWriteGuard<'a, TargetState>),
    Slice(std::sync::RwLockWriteGuard<'a, SliceState>),
    Mount(std::sync::RwLockWriteGuard<'a, MountState>, &'a MountConfig),
}

impl Unit {
    pub const fn is_service(&self) -> bool {
        matches!(self.id.kind, UnitIdKind::Service)
    }
    pub const fn is_socket(&self) -> bool {
        matches!(self.id.kind, UnitIdKind::Socket)
    }
    pub const fn is_target(&self) -> bool {
        matches!(self.id.kind, UnitIdKind::Target)
    }
    pub const fn is_slice(&self) -> bool {
        matches!(self.id.kind, UnitIdKind::Slice)
    }
    pub const fn is_mount(&self) -> bool {
        matches!(self.id.kind, UnitIdKind::Mount)
    }
    pub const fn is_device(&self) -> bool {
        matches!(self.id.kind, UnitIdKind::Device)
    }

    pub fn name_without_suffix(&self) -> String {
        let split: Vec<_> = self.id.name.split('.').collect();
        split[0..split.len() - 1].join(".")
    }

    pub fn dedup_dependencies(&mut self) {
        self.common.dependencies.dedup();
    }

    /// Check if the transition to state 'Starting' can be done
    ///
    /// This is the case if:
    /// 1. All units that have a before relation to this unit have been run at least once
    /// 1. All of the above that are required by this unit are in the state 'Started'
    fn state_transition_starting(&self, run_info: &RuntimeInfo) -> Result<(), Vec<UnitId>> {
        let (mut self_lock, others) = acquire_locks(
            vec![self.id.clone()],
            self.common.dependencies.after.clone(),
            &run_info.unit_table,
        );

        let unstarted_deps = others
            .iter()
            .fold(Vec::new(), |mut acc, (id, status_locked)| {
                let required = self.common.dependencies.requires.contains(id)
                    || self.common.dependencies.binds_to.contains(id);
                let pulled = self.common.dependencies.wants.contains(id);
                let is_pull_dep = required || pulled;
                let ready = if required {
                    status_locked.is_started()
                } else if is_pull_dep {
                    **status_locked != UnitStatus::NeverStarted
                } else {
                    // Pure ordering dep (After= without pull-dep):
                    // NeverStarted means it's not being activated — treat as ready.
                    // Starting means it's actively starting — wait for it.
                    match &**status_locked {
                        UnitStatus::NeverStarted => true,
                        UnitStatus::Starting => false,
                        _ => true,
                    }
                };

                if !ready {
                    acc.push(id.clone());
                }
                acc
            });

        if unstarted_deps.is_empty() {
            **self_lock.get_mut(&self.id).unwrap() = UnitStatus::Starting;
            Ok(())
        } else {
            Err(unstarted_deps)
        }
        // All locks are released again here
    }

    /// Check if the transition to state 'Restarting' can be done. Returns whether the status before was
    /// Started, which requires a full restart.
    ///
    /// This is the case if:
    /// 1. All units that have a before relation to this unit have been run at least once
    /// 1. All of the above that are required by this unit are in the state 'Started'
    fn state_transition_restarting(&self, run_info: &RuntimeInfo) -> Result<bool, Vec<UnitId>> {
        let (mut self_lock, others) = acquire_locks(
            vec![self.id.clone()],
            self.common.dependencies.after.clone(),
            &run_info.unit_table,
        );

        let unstarted_deps = others
            .iter()
            .fold(Vec::new(), |mut acc, (id, status_locked)| {
                let required = self.common.dependencies.requires.contains(id)
                    || self.common.dependencies.binds_to.contains(id);
                let pulled = self.common.dependencies.wants.contains(id);
                let is_pull_dep = required || pulled;
                let ready = if required {
                    status_locked.is_started()
                } else if is_pull_dep {
                    **status_locked != UnitStatus::NeverStarted
                } else {
                    // Pure ordering dep: treat NeverStarted as ready
                    match &**status_locked {
                        UnitStatus::NeverStarted => true,
                        UnitStatus::Starting => false,
                        _ => true,
                    }
                };

                if !ready {
                    acc.push(id.clone());
                }
                acc
            });

        if unstarted_deps.is_empty() {
            let need_full_restart = self_lock.get_mut(&self.id).unwrap().is_started();
            **self_lock.get_mut(&self.id).unwrap() = UnitStatus::Restarting;
            Ok(need_full_restart)
        } else {
            Err(unstarted_deps)
        }
        // All locks are released again here
    }

    /// Check if the transition to state 'Stopping' can be done
    ///
    /// This is the case if:
    /// 1. All units that have a requires relation to this unit have been stopped
    fn state_transition_stopping(&self, run_info: &RuntimeInfo) -> Result<(), Vec<UnitId>> {
        let (mut self_lock, others) = acquire_locks(
            vec![self.id.clone()],
            self.common.dependencies.kill_before_this(),
            &run_info.unit_table,
        );

        let unkilled_depending = others
            .iter()
            .fold(Vec::new(), |mut acc, (id, status_locked)| {
                if status_locked.is_started() {
                    acc.push(id.clone());
                }
                acc
            });

        if unkilled_depending.is_empty() {
            **self_lock.get_mut(&self.id).unwrap() = UnitStatus::Stopping;
            Ok(())
        } else {
            Err(unkilled_depending)
        }
        // All locks are released again here
    }

    /// This activates the unit and manages the state transitions. It reports back the new unit status or any
    /// errors encountered while starting the unit. Note that these errors are also recorded in the units status.
    pub fn activate(
        &self,
        run_info: &RuntimeInfo,
        source: ActivationSource,
    ) -> Result<UnitStatus, UnitOperationError> {
        let state = match &self.specific {
            Specific::Service(specific) => {
                LockedState::Service(specific.state.write().unwrap(), &specific.conf)
            }
            Specific::Socket(specific) => {
                LockedState::Socket(specific.state.write().unwrap(), &specific.conf)
            }
            Specific::Target(specific) => LockedState::Target(specific.state.write().unwrap()),
            Specific::Slice(specific) => LockedState::Slice(specific.state.write().unwrap()),
            Specific::Mount(specific) => {
                LockedState::Mount(specific.state.write().unwrap(), &specific.conf)
            }
        };

        {
            let self_status = &*self.common.status.read().unwrap();
            match self_status {
                UnitStatus::Started(StatusStarted::WaitingForSocket) => {
                    if source == ActivationSource::SocketActivation {
                        // Need activation
                    } else {
                        // Dont need activation
                        return Ok(self_status.clone());
                    }
                }
                UnitStatus::Started(_) => {
                    // Dont need activation
                    return Ok(self_status.clone());
                }
                UnitStatus::Stopped(StatusStopped::StoppedUnexpected, _) => {
                    // Unit already tried to start and failed.  Don't retry
                    // during the initial activation graph walk — only the
                    // restart mechanism (service_exit_handler / reactivate)
                    // should retry failed units if Restart= policy allows it.
                    return Ok(self_status.clone());
                }
                UnitStatus::Stopped(_, _) => {
                    if source == ActivationSource::SocketActivation {
                        // Dont need activation
                        return Ok(self_status.clone());
                    }
                    // Need activation
                }
                _ => {
                    // Need activation
                }
            }
        }

        self.state_transition_starting(run_info).map_err(|bad_ids| {
            trace!(
                "Unit: {} ignores activation. Not all dependencies have been started (still waiting for: {:?})",
                self.id.name,
                bad_ids,
            );
            UnitOperationError {
                reason: UnitOperationErrorReason::DependencyError(bad_ids),
                unit_name: self.id.name.clone(),
                unit_id: self.id.clone(),
            }
        })?;

        match state {
            LockedState::Target(_) | LockedState::Slice(_) => {
                {
                    let mut status = self.common.status.write().unwrap();
                    if status.is_started() {
                        return Ok(status.clone());
                    }
                    *status = UnitStatus::Started(StatusStarted::Running);
                }
                trace!("Reached target {}", self.id.name);
                Ok(UnitStatus::Started(StatusStarted::Running))
            }
            LockedState::Socket(mut state, conf) => {
                let state = &mut *state;
                state.activate(&self.id, conf, &self.common.status, run_info)
            }
            LockedState::Service(mut state, conf) => {
                let state = &mut *state;
                state.activate(&self.id, conf, &self.common.status, run_info, source)
            }
            LockedState::Mount(_, conf) => activate_mount(&self.id, conf, &self.common.status),
        }
    }

    /// This dectivates the unit and manages the state transitions. It reports back any
    /// errors encountered while stopping the unit
    pub fn deactivate(&self, run_info: &RuntimeInfo) -> Result<(), UnitOperationError> {
        let state = match &self.specific {
            Specific::Service(specific) => {
                LockedState::Service(specific.state.write().unwrap(), &specific.conf)
            }
            Specific::Socket(specific) => {
                LockedState::Socket(specific.state.write().unwrap(), &specific.conf)
            }
            Specific::Target(specific) => LockedState::Target(specific.state.write().unwrap()),
            Specific::Slice(specific) => LockedState::Slice(specific.state.write().unwrap()),
            Specific::Mount(specific) => {
                LockedState::Mount(specific.state.write().unwrap(), &specific.conf)
            }
        };

        {
            let self_status = &*self.common.status.read().unwrap();
            if let UnitStatus::Stopped(_, _) = self_status {
                return Ok(());
            }
            // Need deactivation
        }

        self.state_transition_stopping(run_info).map_err(|bad_ids| {
            trace!(
                "Unit: {} ignores deactivation. Not all units depending on this unit have been started (still waiting for: {:?})",
                self.id.name,
                bad_ids,
            );
            UnitOperationError {
                reason: UnitOperationErrorReason::DependencyError(bad_ids),
                unit_name: self.id.name.clone(),
                unit_id: self.id.clone(),
            }
        })?;

        trace!("Deactivate unit: {}", self.id.name);
        match state {
            LockedState::Target(_) | LockedState::Slice(_) => {
                let mut status = self.common.status.write().unwrap();
                *status = UnitStatus::Stopped(StatusStopped::StoppedFinal, vec![]);
                Ok(())
            }
            LockedState::Socket(mut state, conf) => {
                let state = &mut *state;
                state.deactivate(&self.id, conf, &self.common.status, run_info)
            }
            LockedState::Service(mut state, conf) => {
                let state = &mut *state;
                state.deactivate(&self.id, conf, &self.common.status, run_info)
            }
            LockedState::Mount(_, conf) => deactivate_mount(&self.id, conf, &self.common.status),
        }
    }

    /// This rectivates the unit and manages the state transitions. It reports back any
    /// errors encountered while stopping the unit.
    ///
    /// If the unit was stopped this just calls activate.
    pub fn reactivate(
        &self,
        run_info: &RuntimeInfo,
        source: ActivationSource,
    ) -> Result<(), UnitOperationError> {
        trace!("Reactivate unit: {}", self.id.name);

        let state = match &self.specific {
            Specific::Service(specific) => {
                LockedState::Service(specific.state.write().unwrap(), &specific.conf)
            }
            Specific::Socket(specific) => {
                LockedState::Socket(specific.state.write().unwrap(), &specific.conf)
            }
            Specific::Target(specific) => LockedState::Target(specific.state.write().unwrap()),
            Specific::Slice(specific) => LockedState::Slice(specific.state.write().unwrap()),
            Specific::Mount(specific) => {
                LockedState::Mount(specific.state.write().unwrap(), &specific.conf)
            }
        };

        let need_full_restart = self.state_transition_restarting(run_info).map_err(|bad_ids| {
            trace!(
                "Unit: {} ignores deactivation. Not all units depending on this unit have been started (still waiting for: {:?})",
                self.id.name,
                bad_ids,
            );
            UnitOperationError {
                reason: UnitOperationErrorReason::DependencyError(bad_ids),
                unit_name: self.id.name.clone(),
                unit_id: self.id.clone(),
            }
        })?;

        if need_full_restart {
            match state {
                LockedState::Target(_) | LockedState::Slice(_) => {
                    let mut status = self.common.status.write().unwrap();
                    *status = UnitStatus::Started(StatusStarted::Running);
                    Ok(())
                }
                LockedState::Socket(mut state, conf) => {
                    let state = &mut *state;
                    state.reactivate(&self.id, conf, &self.common.status, run_info)
                }
                LockedState::Service(mut state, conf) => {
                    let state = &mut *state;
                    state.reactivate(&self.id, conf, &self.common.status, run_info, source)
                }
                LockedState::Mount(_, conf) => {
                    deactivate_mount(&self.id, conf, &self.common.status).ok();
                    activate_mount(&self.id, conf, &self.common.status).map(|_| ())
                }
            }
        } else {
            match state {
                LockedState::Target(_) | LockedState::Slice(_) => {
                    let mut status = self.common.status.write().unwrap();
                    *status = UnitStatus::Started(StatusStarted::Running);
                    Ok(())
                }
                LockedState::Socket(mut state, conf) => {
                    let state = &mut *state;
                    state
                        .activate(&self.id, conf, &self.common.status, run_info)
                        .map(|_| ())
                }
                LockedState::Service(mut state, conf) => {
                    let state = &mut *state;
                    state
                        .activate(&self.id, conf, &self.common.status, run_info, source)
                        .map(|_| ())
                }
                LockedState::Mount(_, conf) => {
                    activate_mount(&self.id, conf, &self.common.status).map(|_| ())
                }
            }
        }
    }
}

/// Perform the mount(2) syscall for a mount unit.
///
/// This creates the mount point directory if needed, then calls mount(2)
/// with the parameters from the `[Mount]` section. On success the unit
/// status is set to `Started(Running)`; on failure it is set to
/// `Stopped(StoppedUnexpected)`.
#[cfg(target_os = "linux")]
fn activate_mount(
    id: &UnitId,
    conf: &MountConfig,
    status: &RwLock<UnitStatus>,
) -> Result<UnitStatus, UnitOperationError> {
    let where_path = std::path::Path::new(&conf.where_);

    // Check if already mounted by reading /proc/mounts
    if is_already_mounted(&conf.where_) {
        info!(
            "Mount point {} is already mounted, marking as active",
            conf.where_
        );
        let mut status = status.write().unwrap();
        *status = UnitStatus::Started(StatusStarted::Running);
        return Ok(UnitStatus::Started(StatusStarted::Running));
    }

    // Create mount point directory if it doesn't exist
    if !where_path.exists() {
        let mode = conf.directory_mode;
        trace!(
            "Creating mount point directory {} with mode {:o}",
            conf.where_, mode
        );
        if let Err(e) = std::fs::create_dir_all(where_path) {
            let reason = UnitOperationErrorReason::GenericStartError(format!(
                "Failed to create mount point {}: {}",
                conf.where_, e
            ));
            let mut status = status.write().unwrap();
            *status = UnitStatus::Stopped(StatusStopped::StoppedUnexpected, vec![reason.clone()]);
            return Err(UnitOperationError {
                reason,
                unit_name: id.name.clone(),
                unit_id: id.clone(),
            });
        }
        // Set directory permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(where_path, std::fs::Permissions::from_mode(mode));
        }
    }

    // Build mount flags from Options=
    let mut flags = nix::mount::MsFlags::empty();
    let mut filtered_options: Vec<String> = Vec::new();

    if let Some(ref options) = conf.options {
        for opt in options.split(',') {
            let opt = opt.trim();
            match opt {
                "ro" | "rdonly" => flags |= nix::mount::MsFlags::MS_RDONLY,
                "rw" => flags &= !nix::mount::MsFlags::MS_RDONLY,
                "nosuid" => flags |= nix::mount::MsFlags::MS_NOSUID,
                "suid" => flags &= !nix::mount::MsFlags::MS_NOSUID,
                "nodev" => flags |= nix::mount::MsFlags::MS_NODEV,
                "dev" => flags &= !nix::mount::MsFlags::MS_NODEV,
                "noexec" => flags |= nix::mount::MsFlags::MS_NOEXEC,
                "exec" => flags &= !nix::mount::MsFlags::MS_NOEXEC,
                "sync" => flags |= nix::mount::MsFlags::MS_SYNCHRONOUS,
                "async" => flags &= !nix::mount::MsFlags::MS_SYNCHRONOUS,
                "remount" => flags |= nix::mount::MsFlags::MS_REMOUNT,
                "bind" => flags |= nix::mount::MsFlags::MS_BIND,
                "rbind" => flags |= nix::mount::MsFlags::MS_BIND | nix::mount::MsFlags::MS_REC,
                "move" => flags |= nix::mount::MsFlags::MS_MOVE,
                "noatime" => flags |= nix::mount::MsFlags::MS_NOATIME,
                "nodiratime" => flags |= nix::mount::MsFlags::MS_NODIRATIME,
                "relatime" => flags |= nix::mount::MsFlags::MS_RELATIME,
                "strictatime" => flags |= nix::mount::MsFlags::MS_STRICTATIME,
                "lazytime" => flags |= nix::mount::MsFlags::MS_LAZYTIME,
                "silent" => flags |= nix::mount::MsFlags::MS_SILENT,
                "loud" => flags &= !nix::mount::MsFlags::MS_SILENT,
                "dirsync" => flags |= nix::mount::MsFlags::MS_DIRSYNC,
                "mand" => flags |= nix::mount::MsFlags::MS_MANDLOCK,
                "nomand" => flags &= !nix::mount::MsFlags::MS_MANDLOCK,
                "private" => flags |= nix::mount::MsFlags::MS_PRIVATE,
                "rprivate" => {
                    flags |= nix::mount::MsFlags::MS_PRIVATE | nix::mount::MsFlags::MS_REC
                }
                "shared" => flags |= nix::mount::MsFlags::MS_SHARED,
                "rshared" => flags |= nix::mount::MsFlags::MS_SHARED | nix::mount::MsFlags::MS_REC,
                "slave" => flags |= nix::mount::MsFlags::MS_SLAVE,
                "rslave" => flags |= nix::mount::MsFlags::MS_SLAVE | nix::mount::MsFlags::MS_REC,
                "unbindable" => flags |= nix::mount::MsFlags::MS_UNBINDABLE,
                "runbindable" => {
                    flags |= nix::mount::MsFlags::MS_UNBINDABLE | nix::mount::MsFlags::MS_REC
                }
                "defaults" => { /* defaults = rw,suid,dev,exec,auto,nouser,async */ }
                "auto"
                | "noauto"
                | "user"
                | "nouser"
                | "users"
                | "group"
                | "_netdev"
                | "comment"
                | "x-systemd.automount"
                | "nofail" => {
                    // These are fstab-only options, not passed to mount(2)
                }
                _ => {
                    // Pass unknown options through as data for the filesystem driver
                    filtered_options.push(opt.to_owned());
                }
            }
        }
    }

    if conf.read_write_only {
        flags &= !nix::mount::MsFlags::MS_RDONLY;
    }

    let data_str = if filtered_options.is_empty() {
        None
    } else {
        Some(filtered_options.join(","))
    };

    let what: Option<&str> = if conf.what.is_empty() {
        None
    } else {
        Some(&conf.what)
    };
    let fs_type: Option<&str> = conf.fs_type.as_deref();
    let data: Option<&str> = data_str.as_deref();

    info!(
        "Mounting {} on {} (type={}, flags={:?}, data={:?})",
        conf.what,
        conf.where_,
        fs_type.unwrap_or("auto"),
        flags,
        data
    );

    match nix::mount::mount(what, conf.where_.as_str(), fs_type, flags, data) {
        Ok(()) => {
            info!("Successfully mounted {} on {}", conf.what, conf.where_);
            let mut status = status.write().unwrap();
            *status = UnitStatus::Started(StatusStarted::Running);
            Ok(UnitStatus::Started(StatusStarted::Running))
        }
        Err(e) => {
            error!("Failed to mount {} on {}: {}", conf.what, conf.where_, e);
            let reason = UnitOperationErrorReason::GenericStartError(format!(
                "mount({}, {}, {:?}): {}",
                conf.what, conf.where_, fs_type, e
            ));
            let mut status = status.write().unwrap();
            *status = UnitStatus::Stopped(StatusStopped::StoppedUnexpected, vec![reason.clone()]);
            Err(UnitOperationError {
                reason,
                unit_name: id.name.clone(),
                unit_id: id.clone(),
            })
        }
    }
}

/// Non-Linux stub for mount activation — always succeeds and marks the unit
/// as started.
#[cfg(not(target_os = "linux"))]
fn activate_mount(
    id: &UnitId,
    _conf: &MountConfig,
    status: &RwLock<UnitStatus>,
) -> Result<UnitStatus, UnitOperationError> {
    trace!("Mount activation is a no-op on non-Linux ({})", id.name);
    let mut status = status.write().unwrap();
    *status = UnitStatus::Started(StatusStarted::Running);
    Ok(UnitStatus::Started(StatusStarted::Running))
}

/// Perform the umount(2) syscall for a mount unit.
#[cfg(target_os = "linux")]
fn deactivate_mount(
    id: &UnitId,
    conf: &MountConfig,
    status: &RwLock<UnitStatus>,
) -> Result<(), UnitOperationError> {
    // If not currently mounted, just mark as stopped
    if !is_already_mounted(&conf.where_) {
        trace!(
            "Mount point {} is not mounted, nothing to unmount",
            conf.where_
        );
        let mut status = status.write().unwrap();
        *status = UnitStatus::Stopped(StatusStopped::StoppedFinal, vec![]);
        return Ok(());
    }

    let mut umount_flags = nix::mount::MntFlags::empty();
    if conf.lazy_unmount {
        umount_flags |= nix::mount::MntFlags::MNT_DETACH;
    }
    if conf.force_unmount {
        umount_flags |= nix::mount::MntFlags::MNT_FORCE;
    }

    info!("Unmounting {} (flags={:?})", conf.where_, umount_flags);

    match nix::mount::umount2(conf.where_.as_str(), umount_flags) {
        Ok(()) => {
            info!("Successfully unmounted {}", conf.where_);
            let mut status = status.write().unwrap();
            *status = UnitStatus::Stopped(StatusStopped::StoppedFinal, vec![]);
            Ok(())
        }
        Err(e) => {
            error!("Failed to unmount {}: {}", conf.where_, e);
            let reason = UnitOperationErrorReason::GenericStopError(format!(
                "umount({}): {}",
                conf.where_, e
            ));
            let mut status = status.write().unwrap();
            *status = UnitStatus::Stopped(StatusStopped::StoppedFinal, vec![reason.clone()]);
            Err(UnitOperationError {
                reason,
                unit_name: id.name.clone(),
                unit_id: id.clone(),
            })
        }
    }
}

/// Non-Linux stub for mount deactivation.
#[cfg(not(target_os = "linux"))]
fn deactivate_mount(
    id: &UnitId,
    _conf: &MountConfig,
    status: &RwLock<UnitStatus>,
) -> Result<(), UnitOperationError> {
    trace!("Mount deactivation is a no-op on non-Linux ({})", id.name);
    let mut status = status.write().unwrap();
    *status = UnitStatus::Stopped(StatusStopped::StoppedFinal, vec![]);
    Ok(())
}

/// Check whether a path is already mounted by reading /proc/mounts.
#[cfg(target_os = "linux")]
fn is_already_mounted(path: &str) -> bool {
    let normalized = path.trim_end_matches('/');
    let check_path = if normalized.is_empty() {
        "/"
    } else {
        normalized
    };

    match std::fs::read_to_string("/proc/mounts") {
        Ok(contents) => {
            for line in contents.lines() {
                // /proc/mounts format: device mountpoint fstype options dump pass
                let mut fields = line.split_whitespace();
                if let Some(_device) = fields.next() {
                    if let Some(mountpoint) = fields.next() {
                        let mp = mountpoint.trim_end_matches('/');
                        let mp = if mp.is_empty() { "/" } else { mp };
                        if mp == check_path {
                            return true;
                        }
                    }
                }
            }
            false
        }
        Err(_) => false,
    }
}

#[derive(Debug, Clone)]
pub struct UnitConfig {
    pub description: String,
    pub documentation: Vec<String>,

    /// This is needed for adding/removing units. All units in this set must be present
    /// or this unit is considered invalid os it has to be removed too / cannot be added.
    pub refs_by_name: Vec<UnitId>,

    /// Whether to add implicit default dependencies (e.g. on sysinit.target / shutdown.target).
    /// Defaults to true, matching systemd behavior.
    pub default_dependencies: bool,

    /// Conditions that must all be true for the unit to activate.
    /// If any condition fails, the unit is skipped (not treated as an error).
    /// Matches systemd's ConditionPathExists=, ConditionPathIsDirectory=, etc.
    pub conditions: Vec<UnitCondition>,

    /// Assertions that must all be true for the unit to activate.
    /// Unlike conditions, if any assertion fails the unit enters a **failed**
    /// state (not silently skipped). Uses the same `UnitCondition` type since
    /// the check logic is identical — only the failure semantics differ.
    /// Matches systemd's AssertPathExists=, AssertPathIsDirectory=, etc.
    pub assertions: Vec<UnitCondition>,

    /// Action to take when the unit finishes successfully.
    /// Matches systemd's `SuccessAction=` setting.
    pub success_action: UnitAction,

    /// Action to take when the unit fails.
    /// Matches systemd's `FailureAction=` setting.
    pub failure_action: UnitAction,

    /// Alternative names for this unit from `Alias=` in the `[Install]` section.
    /// In systemd, these create symlinks when the unit is enabled.
    /// In systemd-rs, units can be looked up by any of their aliases.
    pub aliases: Vec<String>,

    /// If true, this unit will not be stopped when isolating to another target.
    /// Defaults to false, matching systemd's `IgnoreOnIsolate=` setting.
    pub ignore_on_isolate: bool,

    /// Default instance name for template units (e.g. `foo@.service`).
    /// When a template is enabled without an explicit instance, this value is used.
    /// Matches systemd's `DefaultInstance=` setting in the `[Install]` section.
    pub default_instance: Option<String>,

    /// If true, this unit may be used with `systemctl isolate`.
    /// Defaults to false, matching systemd's `AllowIsolate=` setting.
    /// Parsed and stored; no runtime enforcement yet.
    pub allow_isolate: bool,

    /// Timeout before a job for this unit is cancelled.
    /// Matches systemd's `JobTimeoutSec=` setting.
    /// Parsed and stored; no runtime enforcement yet.
    pub job_timeout_sec: Option<Timeout>,

    /// Action to take when a job for this unit times out.
    /// Matches systemd's `JobTimeoutAction=` setting.
    /// Uses the same action values as `SuccessAction=`/`FailureAction=`.
    /// Parsed and stored; no runtime enforcement yet.
    pub job_timeout_action: UnitAction,

    /// If true, this unit may not be started manually (e.g. via `systemctl start`).
    /// It can only be started as a dependency of another unit.
    /// Defaults to false, matching systemd's `RefuseManualStart=` setting.
    /// Parsed and stored; no runtime enforcement yet.
    pub refuse_manual_start: bool,

    /// If true, this unit may not be stopped manually (e.g. via `systemctl stop`).
    /// It can only be stopped as a dependency of another unit.
    /// Defaults to false, matching systemd's `RefuseManualStop=` setting.
    /// Parsed and stored; no runtime enforcement yet.
    pub refuse_manual_stop: bool,

    /// Units to activate when this unit enters the "failed" state.
    /// Matches systemd's `OnFailure=` setting.
    /// Parsed and stored; no runtime triggering enforcement yet.
    pub on_failure: Vec<String>,

    /// Job mode for enqueuing OnFailure= units.
    /// Defaults to `Replace`, matching systemd's default.
    /// Matches systemd's `OnFailureJobMode=` setting.
    /// Parsed and stored; no runtime enforcement yet.
    pub on_failure_job_mode: OnFailureJobMode,

    /// Time interval for rate limiting unit starts.
    /// If the unit is started more than `start_limit_burst` times within this interval,
    /// further starts are denied.
    /// Matches systemd's `StartLimitIntervalSec=` setting.
    /// Parsed and stored; no runtime enforcement yet.
    pub start_limit_interval_sec: Option<Timeout>,

    /// Number of starts allowed within the `start_limit_interval_sec` interval.
    /// Defaults to `None` (use manager default, typically 5).
    /// Matches systemd's `StartLimitBurst=` setting.
    /// Parsed and stored; no runtime enforcement yet.
    pub start_limit_burst: Option<u32>,

    /// Action to take when the start rate limit is hit.
    /// Matches systemd's `StartLimitAction=` setting.
    /// Uses the same action values as `SuccessAction=`/`FailureAction=`.
    /// Parsed and stored; no runtime enforcement yet.
    pub start_limit_action: UnitAction,
}

#[derive(Debug, Clone)]
/// This are the runtime dependencies. They are extended when the unit is added into the unit set
/// so all dependencies go both ways.
///
/// These vecs are meant like this:
/// `Dependencies::after`: this unit should start after these units have been started
/// `Dependencies::before`: this unit should start before these units have been started
/// ....
pub struct Dependencies {
    pub wants: Vec<UnitId>,
    pub wanted_by: Vec<UnitId>,
    pub requires: Vec<UnitId>,
    pub required_by: Vec<UnitId>,
    pub conflicts: Vec<UnitId>,
    pub conflicted_by: Vec<UnitId>,
    pub before: Vec<UnitId>,
    pub after: Vec<UnitId>,

    /// Units this unit is "part of". When the listed units are stopped or
    /// restarted, this unit is also stopped or restarted.
    /// Matches systemd's `PartOf=` setting.
    pub part_of: Vec<UnitId>,
    /// Reverse of `part_of`: units that declared `PartOf=` pointing to this unit.
    /// When this unit is stopped or restarted, all `part_of_by` units are too.
    pub part_of_by: Vec<UnitId>,

    /// Units this unit "binds to". Similar to `Requires=`, but additionally
    /// causes this unit to stop when the listed units stop (even cleanly).
    /// Matches systemd's `BindsTo=` setting.
    /// Parsed and stored; stop-propagation not yet enforced at runtime.
    pub binds_to: Vec<UnitId>,
    /// Reverse of `binds_to`: units that declared `BindsTo=` pointing to this unit.
    /// When this unit stops, all `bound_by` units should also stop.
    pub bound_by: Vec<UnitId>,
}

impl Dependencies {
    pub fn dedup(&mut self) {
        self.wants.sort();
        self.wanted_by.sort();
        self.required_by.sort();
        self.conflicts.sort();
        self.conflicted_by.sort();
        self.before.sort();
        self.after.sort();
        self.requires.sort();
        self.part_of.sort();
        self.part_of_by.sort();
        self.binds_to.sort();
        self.bound_by.sort();
        // dedup after sorting
        self.wants.dedup();
        self.requires.dedup();
        self.wanted_by.dedup();
        self.required_by.dedup();
        self.conflicts.dedup();
        self.conflicted_by.dedup();
        self.before.dedup();
        self.after.dedup();
        self.part_of.dedup();
        self.part_of_by.dedup();
        self.binds_to.dedup();
        self.bound_by.dedup();
    }

    #[must_use]
    pub fn kill_before_this(&self) -> Vec<UnitId> {
        let mut ids = Vec::new();
        ids.extend(self.required_by.iter().cloned());
        // Units that declared PartOf= this unit should also stop when this unit stops
        ids.extend(self.part_of_by.iter().cloned());
        // Units that declared BindsTo= this unit should also stop when this unit stops
        ids.extend(self.bound_by.iter().cloned());
        ids
    }
    /// Return units that must be started before this one AND that this unit
    /// actually pulls in (via Wants=/Requires=/BindsTo=).
    ///
    /// Pure `After=` ordering without a pull-dep does NOT cause a unit to be
    /// included in the activation subgraph — it only affects ordering IF
    /// both units happen to be activated.  This matches real systemd behavior
    /// where `After=rescue.target` on `multi-user.target` does NOT cause
    /// `rescue.target` to be activated during normal boot.
    #[must_use]
    pub fn start_before_this(&self) -> Vec<UnitId> {
        // Only return After= deps that are also pull-deps.
        // Units that are only in `after` (pure ordering) are NOT included —
        // they don't need to be started, only ordered if they happen to start.
        self.after
            .iter()
            .filter(|id| {
                self.wants.contains(id) || self.requires.contains(id) || self.binds_to.contains(id)
            })
            .cloned()
            .collect()
    }
    #[must_use]
    pub fn start_concurrently_with_this(&self) -> Vec<UnitId> {
        let mut ids = Vec::new();
        ids.extend(self.wants.iter().cloned());
        ids.extend(self.requires.iter().cloned());
        // BindsTo= implies the same start dependency as Requires=
        ids.extend(self.binds_to.iter().cloned());

        ids.into_iter()
            .filter(|id| !self.after.contains(id))
            .collect()
    }

    /// Remove all occurrences of this id from the vec
    fn remove_from_vec(ids: &mut Vec<UnitId>, id: &UnitId) {
        while let Some(idx) = ids.iter().position(|e| *e == *id) {
            ids.remove(idx);
        }
    }

    pub fn remove_id(&mut self, id: &UnitId) {
        Self::remove_from_vec(&mut self.wants, id);
        Self::remove_from_vec(&mut self.wanted_by, id);
        Self::remove_from_vec(&mut self.requires, id);
        Self::remove_from_vec(&mut self.required_by, id);
        Self::remove_from_vec(&mut self.conflicts, id);
        Self::remove_from_vec(&mut self.conflicted_by, id);
        Self::remove_from_vec(&mut self.before, id);
        Self::remove_from_vec(&mut self.after, id);
        Self::remove_from_vec(&mut self.part_of, id);
        Self::remove_from_vec(&mut self.part_of_by, id);
        Self::remove_from_vec(&mut self.binds_to, id);
        Self::remove_from_vec(&mut self.bound_by, id);
    }

    #[must_use]
    pub fn comes_after(&self, name: &str) -> bool {
        for id in &self.after {
            if id.eq(name) {
                return true;
            }
        }
        false
    }
    #[must_use]
    pub fn comes_before(&self, name: &str) -> bool {
        for id in &self.before {
            if id.eq(name) {
                return true;
            }
        }
        false
    }
    #[must_use]
    pub fn requires(&self, name: &str) -> bool {
        for id in &self.requires {
            if id.eq(name) {
                return true;
            }
        }
        false
    }
    #[must_use]
    pub fn required_by(&self, name: &str) -> bool {
        for id in &self.required_by {
            if id.eq(name) {
                return true;
            }
        }
        false
    }
    #[must_use]
    pub fn wants(&self, name: &str) -> bool {
        for id in &self.wants {
            if id.eq(name) {
                return true;
            }
        }
        false
    }
    #[must_use]
    pub fn wanted_by(&self, name: &str) -> bool {
        for id in &self.wanted_by {
            if id.eq(name) {
                return true;
            }
        }
        false
    }
}

/// Describes a single socket that should be opened. One Socket unit may contain multiple of these
#[derive(Clone, Debug)]
pub struct SingleSocketConfig {
    pub kind: SocketKind,
    pub specialized: SpecializedSocketConfig,
}

/// All settings from the Exec section of a unit
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ExecConfig {
    /// Raw user name or numeric UID from User= directive. Resolved at exec time.
    pub user: Option<String>,
    /// Raw group name or numeric GID from Group= directive. Resolved at exec time.
    pub group: Option<String>,
    /// Raw supplementary group names or numeric GIDs. Resolved at exec time.
    pub supplementary_groups: Vec<String>,
    pub stdin_option: StandardInput,
    pub stdout_path: Option<StdIoOption>,
    pub stderr_path: Option<StdIoOption>,
    pub environment: Option<EnvVars>,
    /// Paths from EnvironmentFile= directives. Each entry is (path, optional)
    /// where optional=true means a leading '-' was present (file may not exist).
    pub environment_files: Vec<(std::path::PathBuf, bool)>,
    pub working_directory: Option<std::path::PathBuf>,
    pub state_directory: Vec<String>,
    /// LogsDirectory= — directories to create under /var/log/ before the
    /// service starts. Ownership is set to the service user/group and the
    /// LOGS_DIRECTORY environment variable is set to a colon-separated
    /// list of the absolute paths. Matches systemd.exec(5).
    pub logs_directory: Vec<String>,
    /// LogsDirectoryMode= — the file system access mode to use when creating
    /// the logs directories specified with LogsDirectory=. Takes an access
    /// mode in octal notation (e.g. 0755). Defaults to 0755.
    /// Parsed and stored; runtime enforcement applies the mode at directory
    /// creation time. See systemd.exec(5).
    pub logs_directory_mode: Option<u32>,
    /// RuntimeDirectory= — directories to create under /run/ before the
    /// service starts. Ownership is set to the service user/group and the
    /// RUNTIME_DIRECTORY environment variable is set to a colon-separated
    /// list of the absolute paths. Matches systemd.exec(5).
    pub runtime_directory: Vec<String>,
    /// RuntimeDirectoryPreserve= — controls whether runtime directories
    /// (created by `RuntimeDirectory=`) are removed when the service stops.
    /// Can be `no` (default, always remove), `yes` (always preserve), or
    /// `restart` (preserve across restarts, remove on full stop). Parsed and
    /// stored; no runtime enforcement yet. See systemd.exec(5).
    pub runtime_directory_preserve: RuntimeDirectoryPreserve,
    pub tty_path: Option<std::path::PathBuf>,
    /// TTYReset= — reset the TTY to sane defaults before use.
    /// Matches systemd: resets termios, keyboard mode, switches to text mode.
    pub tty_reset: bool,
    /// TTYVHangup= — send TIOCVHANGUP to the TTY before use.
    /// Disconnects prior sessions so the new service gets a clean terminal.
    pub tty_vhangup: bool,
    /// TTYVTDisallocate= — deallocate or clear the VT before use.
    pub tty_vt_disallocate: bool,
    /// IgnoreSIGPIPE= — if true (the default), SIGPIPE is set to SIG_IGN before
    /// exec'ing the service binary. When false, the default SIGPIPE disposition
    /// (terminate) is left in place. Matches systemd.exec(5).
    pub ignore_sigpipe: bool,
    /// UtmpIdentifier= — the 4-character identifier string to write to the utmp
    /// and wtmp entries when the service runs on a TTY. Defaults to the TTY
    /// basename when unset. See systemd.exec(5).
    pub utmp_identifier: Option<String>,
    /// UtmpMode= — the type of utmp/wtmp record to write. Defaults to `Init`.
    /// See systemd.exec(5).
    pub utmp_mode: UtmpMode,
    /// ImportCredential= — glob patterns for credentials to import from the
    /// system credential store into the service's credential directory.
    /// Multiple patterns may be specified (the setting accumulates).
    /// See systemd.exec(5).
    pub import_credentials: Vec<String>,
    /// PassEnvironment= — a list of environment variable names to import from
    /// the system manager's (PID 1) environment into the service's execution
    /// environment. Only variables that are actually set in the manager's
    /// environment are passed; unset variables are silently ignored. Multiple
    /// directives accumulate; an empty assignment resets the list. Applied
    /// after Environment= but before internal variables and UnsetEnvironment=.
    /// See systemd.exec(5).
    pub pass_environment: Vec<String>,
    /// UnsetEnvironment= — a list of environment variable names or variable
    /// assignments (VAR=VALUE) to remove from the final environment passed to
    /// executed processes. If a plain name is given, any assignment with that
    /// name is removed regardless of value. If a VAR=VALUE assignment is given,
    /// only an exact match is removed. Applied as the final step when
    /// compiling the environment block. See systemd.exec(5).
    pub unset_environment: Vec<String>,
    /// OOMScoreAdjust= — sets the OOM score adjustment for executed processes.
    /// Takes an integer between -1000 (least likely to be killed) and 1000
    /// (most likely to be killed). Written to /proc/self/oom_score_adj before
    /// exec. See systemd.exec(5).
    pub oom_score_adjust: Option<i32>,
    /// LogExtraFields= — additional journal fields to include in log entries
    /// for this unit. Each entry is a KEY=VALUE string. Multiple directives
    /// accumulate. Parsed and stored; not yet used at runtime. See systemd.exec(5).
    pub log_extra_fields: Vec<String>,
    /// DynamicUser= — if true, a UNIX user and group pair is dynamically
    /// allocated for this unit at runtime and released when the unit is stopped.
    /// Defaults to false. Parsed and stored; no runtime enforcement yet.
    /// See systemd.exec(5).
    pub dynamic_user: bool,
    /// SystemCallFilter= — a list of syscall names or `@group` names for
    /// seccomp-based system-call filtering. Entries prefixed with `~` form a
    /// deny-list; without the prefix they form an allow-list. Multiple
    /// directives accumulate; an empty assignment resets the list. Parsed and
    /// stored; no runtime enforcement yet. See systemd.exec(5).
    pub system_call_filter: Vec<String>,
    /// ProtectSystem= — controls whether the service has read-only access to
    /// the OS file system hierarchy. Parsed and stored; no runtime enforcement
    /// yet (requires mount namespace support). See systemd.exec(5).
    pub protect_system: ProtectSystem,
    /// RestrictNamespaces= — restricts access to Linux namespace types for the
    /// service. Can be a boolean (`yes` restricts all, `no` allows all) or a
    /// space-separated list of namespace type identifiers (cgroup, ipc, net,
    /// mnt, pid, user, uts). A `~` prefix inverts the list. Parsed and stored;
    /// no runtime seccomp enforcement yet. See systemd.exec(5).
    pub restrict_namespaces: RestrictNamespaces,
    /// RestrictRealtime= — if true, any attempts to enable realtime scheduling
    /// in a process of the unit are refused via seccomp. Defaults to false.
    /// Parsed and stored; no runtime seccomp enforcement yet. See systemd.exec(5).
    pub restrict_realtime: bool,
    /// RestrictAddressFamilies= — a list of address family names (e.g.
    /// AF_UNIX, AF_INET, AF_INET6) for seccomp-based socket address family
    /// filtering. Entries prefixed with `~` form a deny-list; without the
    /// prefix they form an allow-list. Multiple directives accumulate; an
    /// empty assignment resets the list. Parsed and stored; no runtime seccomp
    /// enforcement yet. See systemd.exec(5).
    pub restrict_address_families: Vec<String>,
    /// SystemCallErrorNumber= — the errno to return when a system call is
    /// blocked by `SystemCallFilter=`. Takes an errno name such as `EPERM`
    /// or `EACCES`. When not set, blocked calls result in SIGSYS (process
    /// kill). Parsed and stored; no runtime seccomp enforcement yet.
    /// See systemd.exec(5).
    pub system_call_error_number: Option<String>,
    /// NoNewPrivileges= — if true, ensures that the service process and all
    /// its children can never gain new privileges through execve() (e.g.
    /// via setuid/setgid bits or file capabilities). Defaults to false.
    /// Parsed and stored; no runtime enforcement yet. See systemd.exec(5).
    pub no_new_privileges: bool,
    /// ProtectControlGroups= — if true, the Linux Control Groups (cgroups)
    /// hierarchies accessible through /sys/fs/cgroup/ will be made read-only
    /// to all processes of the unit. Defaults to false. Parsed and stored;
    /// no runtime enforcement yet (requires mount namespace support).
    /// See systemd.exec(5).
    pub protect_control_groups: bool,
    /// ProtectKernelModules= — if true, explicit kernel module loading and
    /// unloading is denied. This also makes /usr/lib/modules/ inaccessible.
    /// Defaults to false. Parsed and stored; no runtime enforcement yet
    /// (requires mount namespace and seccomp support). See systemd.exec(5).
    pub protect_kernel_modules: bool,
    /// RestrictSUIDSGID= — if true, any attempts to set the set-user-ID
    /// (SUID) or set-group-ID (SGID) bits on files or directories will be
    /// denied. Defaults to false. Parsed and stored; no runtime enforcement
    /// yet (requires seccomp support). See systemd.exec(5).
    pub restrict_suid_sgid: bool,
    /// ProtectKernelLogs= — if true, access to the kernel log ring buffer
    /// (/dev/kmsg, /proc/kmsg, dmesg) is denied. Defaults to false. Parsed
    /// and stored; no runtime enforcement yet (requires mount namespace and
    /// seccomp support). See systemd.exec(5).
    pub protect_kernel_logs: bool,
    /// ProtectKernelTunables= — if true, kernel variables accessible through
    /// /proc/sys/, /sys/, /proc/sysrq-trigger, /proc/latency_stats,
    /// /proc/acpi, /proc/timer_stats, /proc/fs, and /proc/irq will be made
    /// read-only to all processes of the unit. Defaults to false. Parsed and
    /// stored; no runtime enforcement yet (requires mount namespace support).
    /// See systemd.exec(5).
    pub protect_kernel_tunables: bool,
    /// ProtectClock= — if true, writes to the system and hardware clock are
    /// denied. Defaults to false. Parsed and stored; no runtime enforcement
    /// yet (requires seccomp and device access restrictions).
    /// See systemd.exec(5).
    pub protect_clock: bool,
    /// CapabilityBoundingSet= — a list of Linux capability names (e.g.
    /// CAP_NET_ADMIN, CAP_SYS_PTRACE) controlling the capability bounding
    /// set for executed processes. Entries prefixed with `~` form a deny-list;
    /// without the prefix they form an allow-list. Multiple directives
    /// accumulate; an empty assignment resets the list. Parsed and stored;
    /// no runtime enforcement yet. See systemd.exec(5).
    pub capability_bounding_set: Vec<String>,
    /// AmbientCapabilities= — a list of Linux capability names (e.g.
    /// CAP_NET_BIND_SERVICE, CAP_SYS_NICE) controlling the ambient
    /// capability set for executed processes. Ambient capabilities are
    /// inherited across execve() even without file capabilities. Entries
    /// prefixed with `~` form a deny-list; without the prefix they form an
    /// allow-list. Multiple directives accumulate; an empty assignment resets
    /// the list. Parsed and stored; no runtime enforcement yet.
    /// See systemd.exec(5).
    pub ambient_capabilities: Vec<String>,
    /// ProtectHome= — controls whether /home, /root, and /run/user are
    /// accessible to the service. Can be `no` (default), `yes` (inaccessible),
    /// `read-only`, or `tmpfs`. Parsed and stored; no runtime enforcement yet
    /// (requires mount namespace support). See systemd.exec(5).
    pub protect_home: ProtectHome,
    /// ProtectHostname= — if true, sets up a new UTS namespace for the
    /// service and prevents it from changing the hostname or domainname.
    /// Defaults to false. Parsed and stored; no runtime enforcement yet
    /// (requires UTS namespace and seccomp support). See systemd.exec(5).
    pub protect_hostname: bool,
    /// SystemCallArchitectures= — a space-separated list of architecture
    /// identifiers (e.g. `native`, `x86`, `x86-64`) restricting which
    /// architectures system calls may be invoked from. Multiple directives
    /// accumulate; an empty assignment resets the list. Parsed and stored;
    /// no runtime seccomp enforcement yet. See systemd.exec(5).
    pub system_call_architectures: Vec<String>,
    /// ReadWritePaths= — a space-separated list of file system paths that
    /// should be made accessible read-write for the service, even if other
    /// settings (like `ProtectSystem=strict`) would otherwise make them
    /// read-only. Multiple directives accumulate; an empty assignment resets
    /// the list. Parsed and stored; no runtime mount-namespace enforcement
    /// yet. See systemd.exec(5).
    pub read_write_paths: Vec<String>,
    /// MemoryDenyWriteExecute= — if true, attempts to create memory mappings
    /// that are both writable and executable, or to change existing writable
    /// mappings to executable, are prohibited. Defaults to false. Parsed and
    /// stored; no runtime seccomp enforcement yet. See systemd.exec(5).
    pub memory_deny_write_execute: bool,
    /// LockPersonality= — if true, locks down the personality(2) system call
    /// so that the kernel execution domain may not be changed from the default
    /// or the personality selected with Personality=. Defaults to false.
    /// Parsed and stored; no runtime seccomp enforcement yet. See systemd.exec(5).
    pub lock_personality: bool,
    /// ProtectProc= — controls the `hidepid=` mount option of the procfs
    /// instance for the unit. Takes one of "default", "noaccess", "invisible",
    /// or "ptraceable". Defaults to "default". Parsed and stored; no runtime
    /// mount-namespace enforcement yet. See systemd.exec(5).
    pub protect_proc: ProtectProc,
    /// PrivateTmp= — if true, sets up a private /tmp and /var/tmp namespace
    /// for the executed processes. Files in these directories are not visible
    /// to other processes and vice versa. Defaults to false. Parsed and
    /// stored; no runtime mount-namespace enforcement yet. See systemd.exec(5).
    pub private_tmp: bool,
    /// PrivateDevices= — if true, sets up a private /dev/ namespace for the
    /// executed processes with only pseudo-devices such as /dev/null,
    /// /dev/zero, /dev/random available. Physical devices are not accessible.
    /// Defaults to false. Parsed and stored; no runtime mount-namespace
    /// enforcement yet. See systemd.exec(5).
    pub private_devices: bool,
    /// PrivateNetwork= — if true, sets up a new network namespace for the
    /// executed processes with only the loopback device "lo" (and no
    /// external network connectivity). Defaults to false. Parsed and stored;
    /// no runtime namespace enforcement yet. See systemd.exec(5).
    pub private_network: bool,
    /// PrivateUsers= — if true, sets up a new user namespace for the
    /// executed processes and configures a minimal user/group mapping:
    /// the root user/group as well as the unit's own user/group are
    /// mapped to themselves, everything else is mapped to the "nobody"
    /// user/group. Defaults to false. Parsed and stored; no runtime
    /// user-namespace enforcement yet. See systemd.exec(5).
    pub private_users: bool,
    /// PrivateMounts= — if true, the processes of this unit will be run in
    /// their own private file system (mount) namespace with all mount
    /// propagation from the processes towards the host's main file system
    /// namespace turned off. File system mount points established or removed
    /// on the host will still be propagated to the unit's processes.
    /// Defaults to false. Parsed and stored; no runtime mount-namespace
    /// enforcement yet. See systemd.exec(5).
    pub private_mounts: bool,
    /// IOSchedulingClass= — sets the I/O scheduling class for executed
    /// processes. Takes one of "none" (or "0"), "realtime" (or "1"),
    /// "best-effort" (or "2"), or "idle" (or "3"). Defaults to None
    /// (kernel default, which is best-effort). Parsed and stored; no
    /// runtime ioprio_set() enforcement yet. See systemd.exec(5).
    pub io_scheduling_class: IOSchedulingClass,
    /// IOSchedulingPriority= — sets the I/O scheduling priority for executed
    /// processes. Takes an integer between 0 (highest priority) and 7
    /// (lowest priority). The default priority for the best-effort scheduling
    /// class is 4. Parsed and stored; no runtime enforcement yet.
    /// See systemd.exec(5).
    pub io_scheduling_priority: Option<u8>,
    /// UMask= — sets the file mode creation mask (umask) for executed
    /// processes. Takes an octal value (e.g. 0022, 0077). Defaults to 0022.
    /// Parsed and stored; no runtime enforcement yet. See systemd.exec(5).
    pub umask: Option<u32>,
    /// ProcSubset= — controls which subset of /proc/ is mounted for the
    /// unit. Takes one of "all" (full /proc, default) or "pid" (only
    /// process-specific subdirectories). Parsed and stored; no runtime
    /// mount-namespace enforcement yet. See systemd.exec(5).
    pub proc_subset: ProcSubset,
    /// Nice= — sets the default nice level (scheduling priority) for
    /// executed processes. Takes an integer between -20 (highest priority)
    /// and 19 (lowest priority). Parsed and stored; no runtime enforcement
    /// yet. See systemd.exec(5).
    pub nice: Option<i32>,
    /// RemoveIPC= — if true, all System V and POSIX IPC objects owned by
    /// the user and group of the executed processes are removed when the
    /// unit is stopped. Defaults to false. Parsed and stored; no runtime
    /// enforcement yet. See systemd.exec(5).
    pub remove_ipc: bool,
    /// PAMName= — sets the PAM service name to set up a session as.
    /// If set, PAM authentication, account management, and session open/close
    /// calls are performed for the executed processes. Takes a string such as
    /// "login", "systemd-user", or "sshd". Defaults to None (no PAM session).
    /// Parsed and stored; no runtime PAM enforcement yet. See systemd.exec(5).
    pub pam_name: Option<String>,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct PlatformSpecificServiceFields {
    pub cgroup_path: std::path::PathBuf,
}

#[cfg(not(target_os = "linux"))]
#[derive(Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct PlatformSpecificServiceFields {}

/// Additional exit codes and signals that should be considered a successful
/// termination, as configured by `SuccessExitStatus=` in the `[Service]` section.
///
/// By default only exit code 0 and the "clean" signals (SIGHUP, SIGINT,
/// SIGTERM, SIGPIPE) count as success.  This struct extends that set on a
/// per-service basis.
#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub struct SuccessExitStatus {
    pub exit_codes: Vec<i32>,
    pub signals: Vec<nix::sys::signal::Signal>,
}

impl SuccessExitStatus {
    /// Returns `true` when `termination` should be treated as a successful
    /// exit, considering both the built-in rules (exit 0) and any extra
    /// codes/signals configured via `SuccessExitStatus=`.
    pub fn is_success(&self, termination: &crate::signal_handler::ChildTermination) -> bool {
        match termination {
            crate::signal_handler::ChildTermination::Exit(code) => {
                *code == 0 || self.exit_codes.contains(code)
            }
            crate::signal_handler::ChildTermination::Signal(sig) => self.signals.contains(sig),
        }
    }

    /// Like `is_clean_signal` but also considers extra signals from this
    /// config as "clean".
    pub fn is_clean_signal(&self, termination: &crate::signal_handler::ChildTermination) -> bool {
        use nix::sys::signal::Signal;
        match termination {
            crate::signal_handler::ChildTermination::Signal(sig) => {
                matches!(
                    sig,
                    Signal::SIGHUP | Signal::SIGINT | Signal::SIGTERM | Signal::SIGPIPE
                ) || self.signals.contains(sig)
            }
            crate::signal_handler::ChildTermination::Exit(_) => false,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
/// The immutable config of a service unit
pub struct ServiceConfig {
    pub restart: ServiceRestart,
    pub restart_sec: Option<Timeout>,
    pub kill_mode: KillMode,
    pub delegate: Delegate,
    pub tasks_max: Option<TasksMax>,
    pub limit_nofile: Option<ResourceLimit>,
    pub accept: bool,
    pub notifyaccess: NotifyKind,
    pub exec: Option<Commandline>,
    pub reload: Vec<Commandline>,
    pub stop: Vec<Commandline>,
    pub stoppost: Vec<Commandline>,
    pub startpre: Vec<Commandline>,
    pub startpost: Vec<Commandline>,
    pub srcv_type: ServiceType,
    pub starttimeout: Option<Timeout>,
    pub stoptimeout: Option<Timeout>,
    pub generaltimeout: Option<Timeout>,
    pub exec_config: ExecConfig,
    pub platform_specific: PlatformSpecificServiceFields,
    pub dbus_name: Option<String>,
    /// PIDFile= — path to a file that contains the PID of the main daemon
    /// process after a Type=forking service has started.
    pub pid_file: Option<std::path::PathBuf>,
    pub sockets: Vec<UnitId>,
    /// Slice= — the slice unit to place this service in for resource management
    pub slice: Option<String>,
    /// RemainAfterExit= — whether the service is considered active even after
    /// the main process exits. Defaults to false. Commonly used with Type=oneshot.
    pub remain_after_exit: bool,
    /// SuccessExitStatus= — additional exit codes and signals that are
    /// considered a successful (clean) service termination.
    pub success_exit_status: SuccessExitStatus,
    /// RestartForceExitStatus= — a list of exit status definitions (exit
    /// codes and/or signal names) that will force automatic service restart
    /// regardless of the configured Restart= setting. The format is the
    /// same as SuccessExitStatus=. See systemd.service(5).
    pub restart_force_exit_status: SuccessExitStatus,
    /// SendSIGHUP= — if true, send SIGHUP to remaining processes immediately
    /// after the stop signal (e.g. SIGTERM). This is useful for shell-like
    /// services that need to be notified their connection has been severed.
    /// Defaults to false. See systemd.kill(5).
    pub send_sighup: bool,

    /// MemoryPressureWatch= — configures whether to watch for memory pressure
    /// events via PSI. Parsed and stored; no runtime enforcement.
    /// See systemd.resource-control(5).
    pub memory_pressure_watch: MemoryPressureWatch,

    /// ReloadSignal= — configures the UNIX process signal to send to the
    /// service's main process when asked to reload. Defaults to SIGHUP.
    /// Only effective with Type=notify-reload. Stored as a raw signal
    /// number to support realtime signals (RTMIN+N). Parsed and stored;
    /// not yet used at runtime. See systemd.service(5).
    pub reload_signal: Option<i32>,

    /// KillSignal= — specifies which signal to use when stopping the service.
    /// Defaults to SIGTERM. Takes a signal name such as SIGTERM, SIGINT,
    /// SIGKILL, RTMIN+N, or a numeric signal number. Stored as a raw signal
    /// number to support realtime signals. Parsed and stored; not yet used
    /// at runtime. See systemd.kill(5).
    pub kill_signal: Option<i32>,

    /// DelegateSubgroup= — place unit processes in the specified subgroup of
    /// the unit's control group. Only effective when Delegate= is enabled.
    /// Parsed and stored; not yet used at runtime. See systemd.resource-control(5).
    pub delegate_subgroup: Option<String>,

    /// KeyringMode= — controls how the kernel session keyring is set up for
    /// the service. Defaults to `private` for system services and `inherit`
    /// for non-service units / user services. Parsed and stored; not yet
    /// enforced at runtime. See systemd.exec(5).
    pub keyring_mode: KeyringMode,

    /// DeviceAllow= — controls access to specific device nodes for the unit.
    /// Each entry is a device node path (or class like "char-*", "block-*")
    /// optionally followed by access characters (r=read, w=write, m=mknod).
    /// Multiple directives accumulate; an empty assignment resets the list.
    /// Parsed and stored; no runtime enforcement yet (requires cgroup device
    /// controller). See systemd.resource-control(5).
    pub device_allow: Vec<String>,

    /// DevicePolicy= — controls the policy for device access for the unit.
    /// Takes "auto" (default, no restrictions), "closed" (only standard
    /// pseudo devices plus DeviceAllow= entries), or "strict" (only
    /// DeviceAllow= entries). Parsed and stored; no runtime enforcement
    /// yet (requires cgroup device controller). See systemd.resource-control(5).
    pub device_policy: DevicePolicy,

    /// WatchdogSec= — configures the watchdog timeout for the service. The
    /// service must send a "WATCHDOG=1" notification via sd_notify() at least
    /// once within this interval, or it will be considered failed. A value of
    /// 0 or empty disables the watchdog. Parsed and stored; no runtime
    /// enforcement yet. See systemd.service(5).
    pub watchdog_sec: Option<Timeout>,

    /// IPAddressAllow= — a list of IP address prefixes (CIDR notation) or
    /// special keywords (`any`, `localhost`, `link-local`, `multicast`) that
    /// are allowed for IP communication by this unit. Multiple directives
    /// accumulate; an empty assignment resets the list. Parsed and stored;
    /// no runtime enforcement yet (requires eBPF/cgroup support).
    /// See systemd.resource-control(5).
    pub ip_address_allow: Vec<String>,

    /// IPAddressDeny= — a list of IP address prefixes (CIDR notation) or
    /// special keywords (`any`, `localhost`, `link-local`, `multicast`) that
    /// are denied for IP communication by this unit. Multiple directives
    /// accumulate; an empty assignment resets the list. Parsed and stored;
    /// no runtime enforcement yet (requires eBPF/cgroup support).
    /// See systemd.resource-control(5).
    pub ip_address_deny: Vec<String>,

    /// FileDescriptorStoreMax= — the maximum number of file descriptors that
    /// may be stored in the service manager for this service using
    /// sd_pid_notify_with_fds()'s FDSTORE=1 messages. Defaults to 0
    /// (file descriptor store disabled). Parsed and stored; no runtime
    /// enforcement yet. See systemd.service(5).
    pub file_descriptor_store_max: u64,

    /// FileDescriptorStorePreserve= — controls whether file descriptors
    /// stored in the service manager (via FDSTORE=1 sd_notify messages) are
    /// preserved across service restarts or stops. Takes `no` (default,
    /// discard on stop), `yes` (preserve as long as the unit exists), or
    /// `restart` (preserve across restarts, discard on full stop). Parsed
    /// and stored; no runtime enforcement yet. See systemd.service(5).
    pub file_descriptor_store_preserve: FileDescriptorStorePreserve,

    /// MemoryMin= — minimum memory guarantee for the unit's cgroup. The
    /// memory controller will try to protect at least this much memory from
    /// reclaim. Accepts a byte value with optional K/M/G/T/P/E suffix
    /// (base 1024), a percentage, or "infinity". Parsed and stored; no
    /// runtime cgroup enforcement yet. See systemd.resource-control(5).
    pub memory_min: Option<MemoryLimit>,

    /// MemoryLow= — low memory boundary for the unit's cgroup. Below this
    /// threshold the kernel memory reclaimer will avoid reclaiming memory
    /// from the unit. Accepts a byte value with optional K/M/G/T/P/E suffix
    /// (base 1024), a percentage, or "infinity". Parsed and stored; no
    /// runtime cgroup enforcement yet. See systemd.resource-control(5).
    pub memory_low: Option<MemoryLimit>,

    /// RuntimeMaxSec= — configures a maximum time for the service to run.
    /// If the service has been active for longer than the specified time it
    /// is terminated and put into a failure state. When set to `infinity`
    /// (the default) or not set, no time limit is configured. A value of 0
    /// also means no limit. Parsed and stored; no runtime enforcement yet.
    /// See systemd.service(5).
    pub runtime_max_sec: Option<Timeout>,

    /// CoredumpReceive= — boolean, defaults to false. When enabled, the
    /// service manager sets up a coredump socket so the service can receive
    /// coredumps from systemd-coredump. Parsed and stored; no runtime
    /// enforcement yet. See systemd.service(5).
    pub coredump_receive: bool,
}

/// The immutable config of a socket unit
pub struct SocketConfig {
    pub sockets: Vec<SingleSocketConfig>,
    pub filedesc_name: String,
    pub services: Vec<UnitId>,

    /// Accept= — whether the socket uses Accept mode (inetd-style).
    /// If true, for each incoming connection a new service instance is
    /// spawned. Defaults to false. Parsed and stored; inetd-style
    /// activation is not yet supported at runtime. See systemd.socket(5).
    pub accept: bool,

    /// MaxConnections= — the maximum number of simultaneous connections for
    /// Accept=yes sockets. Defaults to 64. Parsed and stored; no runtime
    /// enforcement yet. See systemd.socket(5).
    pub max_connections: u64,

    /// MaxConnectionsPerSource= — the maximum number of simultaneous
    /// connections for a service per source IP address. Defaults to the value
    /// of MaxConnections=. Parsed and stored; no runtime enforcement yet.
    /// See systemd.socket(5).
    pub max_connections_per_source: u64,

    /// SocketMode= — the file system access mode for the socket node
    /// (AF_UNIX sockets and FIFOs). Takes an octal access mode, e.g. 0666.
    /// Defaults to 0666. Parsed and stored; no runtime enforcement yet.
    /// See systemd.socket(5).
    pub socket_mode: Option<u32>,

    /// DirectoryMode= — the file system access mode for directories created
    /// for the socket (if needed). Takes an octal access mode, e.g. 0755.
    /// Defaults to 0755. Parsed and stored; no runtime enforcement yet.
    /// See systemd.socket(5).
    pub directory_mode: Option<u32>,

    /// PassCredentials= — whether to enable SO_PASSCRED on the socket, so
    /// that the receiving process can obtain peer credentials via an
    /// ancillary message. Defaults to false. Parsed and stored; no runtime
    /// enforcement yet. See systemd.socket(5).
    pub pass_credentials: bool,

    /// PassSecurity= — whether to enable SO_PASSSEC on the socket, so
    /// that AF_UNIX sockets receive the security context of the sending
    /// process in an ancillary message. Defaults to false. Parsed and
    /// stored; no runtime enforcement yet. See systemd.socket(5).
    pub pass_security: bool,

    /// AcceptFileDescriptors= — whether to enable SO_PASSRIGHTS on the
    /// socket, which when disabled prohibits the peer from sending
    /// SCM_RIGHTS ancillary messages (file descriptors) via AF_UNIX
    /// sockets. Defaults to true. Parsed and stored; no runtime
    /// enforcement yet. See systemd.socket(5).
    pub accept_file_descriptors: bool,

    /// RemoveOnStop= — whether to remove file nodes (AF_UNIX sockets,
    /// FIFOs, message queues, and symlinks configured with Symlinks=)
    /// created by this socket unit when it is stopped. Defaults to false.
    /// Parsed and stored; no runtime enforcement yet. See systemd.socket(5).
    pub remove_on_stop: bool,

    /// ReceiveBuffer= — the receive buffer size (SO_RCVBUF) in bytes for the
    /// socket. Takes an integer value. Parsed and stored; no runtime
    /// enforcement yet. See systemd.socket(5).
    pub receive_buffer: Option<u64>,

    /// SendBuffer= — the send buffer size (SO_SNDBUF) in bytes for the
    /// socket. Takes an integer value. Parsed and stored; no runtime
    /// enforcement yet. See systemd.socket(5).
    pub send_buffer: Option<u64>,

    /// Symlinks= — a list of file system paths to create as symlinks to
    /// the AF_UNIX socket path or FIFO path of this socket unit. Multiple
    /// paths may be specified space-separated; the directive may appear
    /// multiple times to extend the list. An empty value resets the list.
    /// Defaults to an empty list. Parsed and stored; no runtime enforcement
    /// yet. See systemd.socket(5).
    pub symlinks: Vec<String>,

    /// Timestamping= — controls the SO_TIMESTAMP or SO_TIMESTAMPNS socket
    /// options, enabling timestamping metadata on ingress network traffic.
    /// Takes one of "off", "us"/"usec"/"μs", or "ns"/"nsec".
    /// Defaults to Off. Parsed and stored; no runtime enforcement yet.
    /// See systemd.socket(5).
    pub timestamping: Timestamping,

    /// DeferTrigger= — controls whether to defer triggering the associated
    /// service when a connection comes in. Takes a boolean or "patient".
    /// Defaults to No. Parsed and stored; no runtime enforcement yet.
    /// See systemd.socket(5).
    pub defer_trigger: DeferTrigger,

    /// Writable= — whether to open the FIFO or special file for writing
    /// as well (i.e. O_RDWR rather than O_RDONLY). Defaults to false.
    /// Parsed and stored; no runtime enforcement yet. See systemd.socket(5).
    pub writable: bool,

    pub exec_config: ExecConfig,
}
