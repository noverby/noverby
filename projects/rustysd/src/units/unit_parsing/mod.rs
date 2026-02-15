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

    /// Units this unit is "part of". When the listed units are stopped or
    /// restarted, this unit is also stopped or restarted.
    /// Matches systemd's `PartOf=` setting.
    pub part_of: Vec<String>,

    /// Whether to add implicit default dependencies (e.g. on sysinit.target / shutdown.target).
    /// Defaults to true, matching systemd behavior.
    pub default_dependencies: bool,

    /// If true, this unit will not be stopped when isolating to another target.
    /// Defaults to false, matching systemd's `IgnoreOnIsolate=` setting.
    pub ignore_on_isolate: bool,

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

    /// Absolute paths that this unit requires mount points for.
    /// Automatically adds `Requires=` and `After=` dependencies on the
    /// corresponding `.mount` units for each path prefix.
    /// Matches systemd's `RequiresMountsFor=` setting.
    pub requires_mounts_for: Vec<String>,

    /// If true, this unit is stopped when no other active unit requires or wants it.
    /// Defaults to false, matching systemd's `StopWhenUnneeded=` setting.
    /// Parsed and stored; no runtime enforcement yet.
    pub stop_when_unneeded: bool,

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
            part_of: Vec::new(),
            default_dependencies: true,
            ignore_on_isolate: false,
            conditions: Vec::new(),
            success_action: UnitAction::default(),
            failure_action: UnitAction::default(),
            requires_mounts_for: Vec::new(),
            stop_when_unneeded: false,
            allow_isolate: false,
            job_timeout_sec: None,
            job_timeout_action: UnitAction::default(),
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
    pub reload: Vec<Commandline>,
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

    /// Slice= — the slice unit to place this service in for resource management
    pub slice: Option<String>,

    /// RemainAfterExit= — whether the service is considered active even after
    /// the main process exits. Defaults to false. Commonly used with Type=oneshot.
    pub remain_after_exit: bool,

    /// SuccessExitStatus= — additional exit codes and signals that are
    /// considered a successful (clean) service termination.
    pub success_exit_status: crate::units::SuccessExitStatus,

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
    /// Only effective with Type=notify-reload. Parsed and stored; not yet
    /// used at runtime. See systemd.service(5).
    pub reload_signal: Option<nix::sys::signal::Signal>,

    /// DelegateSubgroup= — place unit processes in the specified subgroup of
    /// the unit's control group. Only effective when Delegate= is enabled.
    /// Parsed and stored; not yet used at runtime. See systemd.resource-control(5).
    pub delegate_subgroup: Option<String>,

    /// KeyringMode= — controls how the kernel session keyring is set up for
    /// the service. Defaults to `private` for system services and `inherit`
    /// for non-service units / user services. Parsed and stored; not yet
    /// enforced at runtime. See systemd.exec(5).
    pub keyring_mode: KeyringMode,

    pub exec_section: ParsedExecSection,
}

#[derive(Default)]
pub struct ParsedInstallSection {
    pub wanted_by: Vec<String>,
    pub required_by: Vec<String>,
    pub also: Vec<String>,
    pub alias: Vec<String>,

    /// Default instance name for template units (e.g. `foo@.service`).
    /// When a template is enabled without an explicit instance, this value is used.
    /// Matches systemd's `DefaultInstance=` setting in the `[Install]` section.
    pub default_instance: Option<String>,
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
    /// RuntimeDirectory= — directories to create under /run/ before the
    /// service starts. Ownership is set to the service user/group and the
    /// RUNTIME_DIRECTORY environment variable is set to a colon-separated
    /// list of the absolute paths. Matches systemd.exec(5).
    pub runtime_directory: Vec<String>,
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
}

/// The type of utmp/wtmp record to create for a service.
/// Corresponds to systemd's `UtmpMode=` setting.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum UtmpMode {
    /// Write an INIT_PROCESS record (default).
    Init,
    /// Write a LOGIN_PROCESS record (for getty-like services).
    Login,
    /// Write a USER_PROCESS record.
    User,
}

impl Default for UtmpMode {
    fn default() -> Self {
        UtmpMode::Init
    }
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

/// MemoryPressureWatch= — configures whether to watch for memory pressure
/// events via PSI (Pressure Stall Information). Parsed and stored; no runtime
/// enforcement (requires cgroup + PSI support). See systemd.resource-control(5).
#[derive(Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum MemoryPressureWatch {
    /// Automatically enable if the service has a dedicated cgroup and PSI is
    /// available (default).
    Auto,
    /// Always watch for memory pressure.
    On,
    /// Never watch for memory pressure.
    Off,
    /// Do not set the MEMORY_PRESSURE_WATCH environment variable at all.
    Skip,
}

impl Default for MemoryPressureWatch {
    fn default() -> Self {
        Self::Auto
    }
}

/// KeyringMode= — controls how the kernel session keyring is set up for the
/// service. See session-keyring(7) and systemd.exec(5).
#[derive(Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum KeyringMode {
    /// No special keyring setup; kernel default behaviour applies.
    Inherit,
    /// A new session keyring is allocated and not linked to the user keyring.
    /// Recommended for system services so that multiple services under the
    /// same UID do not share key material (default for system services).
    Private,
    /// A new session keyring is allocated and the user keyring of the
    /// configured User= is linked into it, allowing key sharing between
    /// units running under the same user.
    Shared,
}

impl Default for KeyringMode {
    fn default() -> Self {
        Self::Private
    }
}

/// ProtectSystem= — controls whether the service has read-only access to the
/// OS file system hierarchy. See systemd.exec(5).
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum ProtectSystem {
    /// No file system protection (default).
    No,
    /// Mount /usr and the boot loader directories (/boot, /efi) read-only.
    Yes,
    /// Like `Yes`, but additionally mount /etc read-only.
    Full,
    /// Mount the entire file system hierarchy read-only, except for /dev,
    /// /proc, /sys, and API mount points. Implies `ReadWritePaths=`,
    /// `ReadOnlyPaths=`, `InaccessiblePaths=` are still honoured.
    Strict,
}

impl Default for ProtectSystem {
    fn default() -> Self {
        Self::No
    }
}

/// RestrictNamespaces= — restricts access to Linux namespace types for the
/// service. See systemd.exec(5).
#[derive(Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum RestrictNamespaces {
    /// No namespace restrictions (default).
    No,
    /// Restrict all namespace creation and joining.
    Yes,
    /// Restrict to only the listed namespace types (allow-list).
    Allow(Vec<String>),
    /// Allow all namespace types except the listed ones (deny-list, ~ prefix).
    Deny(Vec<String>),
}

impl Default for RestrictNamespaces {
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
    /// Connect to the TTY device (from TTYPath=, default /dev/console).
    /// When StandardInput is also tty, stdout/stderr share the same TTY fd.
    /// When StandardInput is NOT tty, the TTY is opened independently for output.
    /// Matches systemd's `StandardOutput=tty` / `StandardError=tty`.
    Tty,
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
