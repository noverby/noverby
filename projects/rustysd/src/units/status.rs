use crate::units::UnitOperationErrorReason;

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum UnitStatus {
    NeverStarted,
    Starting,
    Stopping,
    Restarting,
    Started(StatusStarted),
    Stopped(StatusStopped, Vec<UnitOperationErrorReason>),
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum StatusStarted {
    Running,
    WaitingForSocket,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum StatusStopped {
    StoppedFinal,
    StoppedUnexpected,
}

impl UnitStatus {
    #[must_use]
    pub const fn is_stopped(&self) -> bool {
        matches!(self, Self::Stopped(_, _))
    }
    #[must_use]
    pub const fn is_started(&self) -> bool {
        matches!(self, Self::Started(_))
    }
}
