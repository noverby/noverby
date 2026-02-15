mod service_unit;
mod socket_unit;
mod target_unit;
mod unit_parser;

pub use service_unit::*;
pub use socket_unit::*;
pub use target_unit::*;
pub use unit_parser::*;

use std::path::PathBuf;

pub struct ParsedCommonConfig {
    pub unit: ParsedUnitSection,
    pub install: ParsedInstallSection,
    pub name: String,
}
pub struct ParsedServiceConfig {
    pub common: ParsedCommonConfig,
    pub srvc: ParsedServiceSection,
}
pub struct ParsedSocketConfig {
    pub common: ParsedCommonConfig,
    pub sock: ParsedSocketSection,
}
pub struct ParsedTargetConfig {
    pub common: ParsedCommonConfig,
}

/// A parsed condition from the [Unit] section.
/// Systemd supports many condition types; we implement the most common ones.
#[derive(Clone, Debug)]
pub enum UnitCondition {
    /// ConditionPathExists=/some/path (true if path exists)
    /// ConditionPathExists=!/some/path (true if path does NOT exist)
    PathExists { path: String, negate: bool },
    /// ConditionPathIsDirectory=/some/path
    /// ConditionPathIsDirectory=!/some/path
    PathIsDirectory { path: String, negate: bool },
}

impl UnitCondition {
    /// Evaluate the condition. Returns true if the condition is met.
    pub fn check(&self) -> bool {
        match self {
            UnitCondition::PathExists { path, negate } => {
                let exists = std::path::Path::new(path).exists();
                if *negate {
                    !exists
                } else {
                    exists
                }
            }
            UnitCondition::PathIsDirectory { path, negate } => {
                let is_dir = std::path::Path::new(path).is_dir();
                if *negate {
                    !is_dir
                } else {
                    is_dir
                }
            }
        }
    }
}

/// Action to take when a unit succeeds or fails.
///
/// Matches systemd's `SuccessAction=` / `FailureAction=` settings.
/// See <https://www.freedesktop.org/software/systemd/man/systemd.unit.html#SuccessAction=>.
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum UnitAction {
    /// Do nothing (default).
    None,
    /// Initiate a clean shutdown of the service manager.
    Exit,
    /// Like `Exit`, but without waiting for running jobs to finish.
    ExitForce,
    /// Initiate a clean reboot.
    Reboot,
    /// Reboot immediately, skipping clean shutdown of remaining units.
    RebootForce,
    /// Reboot immediately via `reboot(2)`, skipping all cleanup.
    RebootImmediate,
    /// Initiate a clean poweroff.
    Poweroff,
    /// Poweroff immediately, skipping clean shutdown of remaining units.
    PoweroffForce,
    /// Poweroff immediately via `reboot(2)`, skipping all cleanup.
    PoweroffImmediate,
    /// Initiate a clean halt.
    Halt,
    /// Halt immediately, skipping clean shutdown of remaining units.
    HaltForce,
    /// Halt immediately via `reboot(2)`, skipping all cleanup.
    HaltImmediate,
    /// Initiate a kexec reboot.
    Kexec,
    /// Kexec immediately, skipping clean shutdown of remaining units.
    KexecForce,
    /// Kexec immediately via `reboot(2)`, skipping all cleanup.
    KexecImmediate,
}

impl Default for UnitAction {
    fn default() -> Self {
        Self::None
    }
}

pub struct ParsedUnitSection {
    pub description: String,
    pub documentation: Vec<String>,

    pub wants: Vec<String>,
    pub requires: Vec<String>,
    pub conflicts: Vec<String>,
    pub before: Vec<String>,
    pub after: Vec<String>,

    /// Whether to add implicit default dependencies (e.g. on sysinit.target / shutdown.target).
    /// Defaults to true, matching systemd behavior.
    pub default_dependencies: bool,

    /// Conditions that must all be true for the unit to start.
    /// If any condition fails, the unit is skipped (not an error).
    /// Matches systemd's ConditionPathExists=, ConditionPathIsDirectory=, etc.
    pub conditions: Vec<UnitCondition>,

    /// Action to take when the unit finishes successfully.
    /// Matches systemd's `SuccessAction=` setting.
    pub success_action: UnitAction,

    /// Action to take when the unit fails.
    /// Matches systemd's `FailureAction=` setting.
    pub failure_action: UnitAction,
}

impl Default for ParsedUnitSection {
    fn default() -> Self {
        Self {
            description: String::new(),
            documentation: Vec::new(),
            wants: Vec::new(),
            requires: Vec::new(),
            conflicts: Vec::new(),
            before: Vec::new(),
            after: Vec::new(),
            default_dependencies: true,
            conditions: Vec::new(),
            success_action: UnitAction::default(),
            failure_action: UnitAction::default(),
        }
    }
}
#[derive(Clone)]
pub struct ParsedSingleSocketConfig {
    pub kind: crate::sockets::SocketKind,
    pub specialized: crate::sockets::SpecializedSocketConfig,
}

impl std::fmt::Debug for ParsedSingleSocketConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "SocketConfig {{ kind: {:?}, specialized: {:?} }}",
            self.kind, self.specialized
        )?;
        Ok(())
    }
}

pub struct ParsedSocketSection {
    pub sockets: Vec<ParsedSingleSocketConfig>,
    pub filedesc_name: Option<String>,
    pub services: Vec<String>,

    pub exec_section: ParsedExecSection,
}
pub struct ParsedServiceSection {
    pub restart: ServiceRestart,
    pub restart_sec: Option<Timeout>,
    pub kill_mode: KillMode,
    pub delegate: Delegate,
    pub tasks_max: Option<TasksMax>,
    pub limit_nofile: Option<ResourceLimit>,
    pub accept: bool,
    pub notifyaccess: NotifyKind,
    pub exec: Option<Commandline>,
    pub stop: Vec<Commandline>,
    pub stoppost: Vec<Commandline>,
    pub startpre: Vec<Commandline>,
    pub startpost: Vec<Commandline>,
    pub srcv_type: ServiceType,
    pub starttimeout: Option<Timeout>,
    pub stoptimeout: Option<Timeout>,
    pub generaltimeout: Option<Timeout>,

    pub dbus_name: Option<String>,
    /// PIDFile= — path to a file that contains the PID of the main daemon
    /// process after a Type=forking service has started.
    pub pid_file: Option<PathBuf>,

    pub sockets: Vec<String>,

    pub exec_section: ParsedExecSection,
}

#[derive(Default)]
pub struct ParsedInstallSection {
    pub wanted_by: Vec<String>,
    pub required_by: Vec<String>,
    pub also: Vec<String>,
    pub alias: Vec<String>,
}
pub struct ParsedExecSection {
    pub user: Option<String>,
    pub group: Option<String>,
    pub stdin_option: StandardInput,
    pub stdout_path: Option<StdIoOption>,
    pub stderr_path: Option<StdIoOption>,
    pub supplementary_groups: Vec<String>,
    pub environment: Option<EnvVars>,
    /// Paths from EnvironmentFile= directives. A leading '-' means the file
    /// is optional (no error if it doesn't exist).
    pub environment_files: Vec<(PathBuf, bool)>,
    pub working_directory: Option<PathBuf>,
    pub state_directory: Vec<String>,
    pub tty_path: Option<PathBuf>,
    /// TTYReset= — reset the TTY to sane defaults before use (default: false).
    /// Matches systemd behavior: resets termios, keyboard mode, switches to text mode.
    pub tty_reset: bool,
    /// TTYVHangup= — send TIOCVHANGUP to the TTY before use (default: false).
    /// This disconnects any prior sessions from the TTY so the new service gets
    /// a clean controlling terminal.
    pub tty_vhangup: bool,
    /// TTYVTDisallocate= — deallocate or clear the VT before use (default: false).
    pub tty_vt_disallocate: bool,
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum ServiceType {
    Simple,
    Notify,
    /// Like Notify, but the service also supports reloading via SIGHUP.
    /// At startup this behaves identically to Notify (waits for READY=1).
    NotifyReload,
    Dbus,
    OneShot,
    /// The started process is expected to fork and exit. The parent's exit
    /// signals successful startup. If `PIDFile=` is set the daemon PID is
    /// read from that file; otherwise the service is tracked without a
    /// main PID.
    Forking,
    /// Behaves like Simple, but the service manager delays starting the
    /// service until all active jobs are dispatched (with a 5-second
    /// timeout). This is primarily used for improving console output
    /// ordering and has no effect on service dependencies.
    Idle,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum NotifyKind {
    Main,
    Exec,
    All,
    None,
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum KillMode {
    /// Kill all processes in the control group (default)
    ControlGroup,
    /// Only kill the main process
    Process,
    /// Send SIGTERM to main process, SIGKILL to remaining processes in the control group
    Mixed,
    /// No processes are killed, only ExecStop commands are run
    None,
}

impl Default for KillMode {
    fn default() -> Self {
        Self::ControlGroup
    }
}

/// Whether to delegate cgroup control to the service process
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Delegate {
    /// No delegation (default)
    No,
    /// Delegate all supported controllers
    Yes,
    /// Delegate specific controllers
    Controllers(Vec<String>),
}

impl Default for Delegate {
    fn default() -> Self {
        Self::No
    }
}

/// Limit on the number of tasks (processes/threads) in the service's cgroup
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum TasksMax {
    /// Absolute limit on number of tasks
    Value(u64),
    /// Percentage of the system's overall task limit
    Percent(u64),
    /// No limit
    Infinity,
}

/// A single rlimit value: either a numeric value or infinity
#[derive(Clone, Copy, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum RLimitValue {
    /// A specific numeric value
    Value(u64),
    /// RLIM_INFINITY — no limit
    Infinity,
}

/// A resource limit with soft and hard values (as used by setrlimit)
#[derive(Clone, Copy, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct ResourceLimit {
    pub soft: RLimitValue,
    pub hard: RLimitValue,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum ServiceRestart {
    /// Never restart the service automatically.
    No,
    /// Always restart the service when it exits, regardless of exit status.
    Always,
    /// Restart only if the service exited cleanly (exit code 0 or one of the
    /// "clean" signals: SIGHUP, SIGINT, SIGTERM, SIGPIPE).
    OnSuccess,
    /// Restart only if the service exited with a non-zero exit code, was
    /// terminated by a signal (other than the "clean" ones), timed out, or
    /// hit a watchdog timeout.
    OnFailure,
    /// Restart only if the service was terminated by a signal, timed out, or
    /// hit a watchdog timeout (i.e. not on clean or unclean exit codes).
    OnAbnormal,
    /// Restart only if the service was terminated by an uncaught signal
    /// (not SIGHUP, SIGINT, SIGTERM, SIGPIPE).
    OnAbort,
    /// Restart only if the watchdog timeout for the service expired.
    /// (Currently treated as never restarting since rustysd does not yet
    /// implement watchdog support.)
    OnWatchdog,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Timeout {
    Duration(std::time::Duration),
    Infinity,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum StdIoOption {
    /// StandardOutput/StandardError connected to /dev/null
    Null,
    /// Inherit from the service manager (or from stdin for stdout/stderr)
    Inherit,
    /// Log to the journal (not yet implemented, treated as inherit)
    Journal,
    /// Log to /dev/kmsg (not yet implemented, treated as inherit)
    Kmsg,
    /// Write to a specific file
    File(PathBuf),
    /// Append to a specific file
    AppendFile(PathBuf),
}

/// How stdin should be set up for the service process.
/// Matches systemd's StandardInput= setting.
#[derive(Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum StandardInput {
    /// stdin is connected to /dev/null (default)
    Null,
    /// stdin is connected to a TTY (from TTYPath=, default /dev/console)
    Tty,
    /// Like Tty, but force-acquire the TTY even if another process owns it
    TtyForce,
    /// Like Tty, but fail if the TTY cannot be opened exclusively
    TtyFail,
}

impl Default for StandardInput {
    fn default() -> Self {
        Self::Null
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum CommandlinePrefix {
    AtSign,
    Minus,
    Colon,
    Plus,
    Exclamation,
    DoubleExclamation,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Commandline {
    pub cmd: String,
    pub args: Vec<String>,
    pub prefixes: Vec<CommandlinePrefix>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct EnvVars {
    pub vars: Vec<(String, String)>,
}

impl std::fmt::Display for Commandline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.cmd)?;
        for arg in &self.args {
            write!(f, " {arg}")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ParsingError {
    inner: ParsingErrorReason,
    path: std::path::PathBuf,
}

impl ParsingError {
    #[must_use]
    pub const fn new(reason: ParsingErrorReason, path: std::path::PathBuf) -> Self {
        Self {
            inner: reason,
            path,
        }
    }
}

#[derive(Debug)]
pub enum ParsingErrorReason {
    UnknownSetting(String, String),
    UnusedSetting(String),
    UnsupportedSetting(String),
    MissingSetting(String),
    SettingTooManyValues(String, Vec<String>),
    SectionTooOften(String),
    SectionNotFound(String),
    UnknownSection(String),
    UnknownSocketAddr(String),
    FileError(Box<dyn std::error::Error>),
    Generic(String),
}

impl std::fmt::Display for ParsingError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self.inner {
            ParsingErrorReason::UnknownSetting(name, value) => {
                write!(
                    f,
                    "In file {:?}: setting {} was set to unrecognized value: {}",
                    self.path, name, value
                )?;
            }
            ParsingErrorReason::UnusedSetting(name) => {
                write!(
                    f,
                    "In file {:?}: unused setting {} occurred",
                    self.path, name
                )?;
            }
            ParsingErrorReason::MissingSetting(name) => {
                write!(
                    f,
                    "In file {:?}: required setting {} missing",
                    self.path, name
                )?;
            }
            ParsingErrorReason::SectionNotFound(name) => {
                write!(
                    f,
                    "In file {:?}: Section {} wasn't found but is required",
                    self.path, name
                )?;
            }
            ParsingErrorReason::UnknownSection(name) => {
                write!(f, "In file {:?}: Section {} is unknown", self.path, name)?;
            }
            ParsingErrorReason::SectionTooOften(name) => {
                write!(
                    f,
                    "In file {:?}: section {} occurred multiple times",
                    self.path, name
                )?;
            }
            ParsingErrorReason::UnknownSocketAddr(addr) => {
                write!(
                    f,
                    "In file {:?}: Can not open sockets of addr: {}",
                    self.path, addr
                )?;
            }
            ParsingErrorReason::UnsupportedSetting(addr) => {
                write!(
                    f,
                    "In file {:?}: Setting not supported by this build (maybe need to enable feature flag?): {}",
                    self.path, addr
                )?;
            }
            ParsingErrorReason::SettingTooManyValues(name, values) => {
                write!(
                    f,
                    "In file {:?}: setting {} occurred with too many values: {:?}",
                    self.path, name, values
                )?;
            }
            ParsingErrorReason::FileError(e) => {
                write!(f, "While parsing file {:?}: {}", self.path, e)?;
            }
            ParsingErrorReason::Generic(e) => {
                write!(f, "While parsing file {:?}: {}", self.path, e)?;
            }
        }

        Ok(())
    }
}

// This is important for other errors to wrap this one.
impl std::error::Error for ParsingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        if let ParsingErrorReason::FileError(err) = &self.inner {
            Some(err.as_ref())
        } else {
            None
        }
    }
}

impl std::convert::From<Box<std::io::Error>> for ParsingErrorReason {
    fn from(err: Box<std::io::Error>) -> Self {
        Self::FileError(err)
    }
}
