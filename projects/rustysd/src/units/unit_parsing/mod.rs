mod service_unit;
mod socket_unit;
mod target_unit;
mod unit_parser;

pub use service_unit::*;
pub use socket_unit::*;
pub use target_unit::*;
pub use unit_parser::*;

use log::warn;
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
    /// ConditionVirtualization=yes|no|vm|container|<tech-name>
    /// ConditionVirtualization=!<value> (negated)
    /// Checks whether the system runs in a virtualized environment.
    /// See systemd.unit(5).
    Virtualization { value: String, negate: bool },
    /// ConditionCapability=CAP_NET_ADMIN (true if capability is in bounding set)
    /// ConditionCapability=!CAP_SYS_ADMIN (true if capability is NOT in bounding set)
    /// Checks whether a given capability exists in the capability bounding set
    /// of the service manager (PID 1). See systemd.unit(5).
    Capability { capability: String, negate: bool },
    /// ConditionFirstBoot=yes (true if this is the first boot of the system)
    /// ConditionFirstBoot=!yes (true if this is NOT the first boot)
    /// Checks whether the system is booting for the first time (i.e.
    /// /etc/machine-id does not yet exist). See systemd.unit(5).
    FirstBoot { value: bool, negate: bool },
    /// ConditionFileNotEmpty=/some/path (true if path exists as a regular file with size > 0)
    /// ConditionFileNotEmpty=!/some/path (true if path does NOT exist or is empty)
    /// Checks whether the specified path exists as a regular file and has
    /// a non-zero size. See systemd.unit(5).
    FileNotEmpty { path: String, negate: bool },
    /// ConditionFileIsExecutable=/some/path (true if path exists and is executable)
    /// ConditionFileIsExecutable=!/some/path (true if path does NOT exist or is not executable)
    /// Checks whether the specified path exists as a regular file and has
    /// at least one execute bit set. See systemd.unit(5).
    FileIsExecutable { path: String, negate: bool },
    /// ConditionKernelModuleLoaded=module_name (true if the kernel module is loaded)
    /// ConditionKernelModuleLoaded=!module_name (true if the kernel module is NOT loaded)
    /// Checks whether the specified kernel module is currently loaded.
    /// On Linux this is determined by reading /proc/modules. See systemd.unit(5).
    KernelModuleLoaded { module: String, negate: bool },
    /// ConditionKernelCommandLine=option (true if the kernel command line contains the option)
    /// ConditionKernelCommandLine=!option (true if the kernel command line does NOT contain the option)
    /// The argument is either a single word (checked as a standalone parameter or as the
    /// left-hand side of an assignment) or a key=value assignment (checked for an exact match).
    /// Reads from /proc/cmdline (or /proc/1/cmdline in containers). See systemd.unit(5).
    KernelCommandLine { argument: String, negate: bool },
    /// ConditionDirectoryNotEmpty=/some/path (true if path exists as a directory and is not empty)
    /// ConditionDirectoryNotEmpty=!/some/path (true if path does NOT exist, is not a directory, or is empty)
    /// Checks whether the specified path exists, is a directory, and contains
    /// at least one entry (besides "." and ".."). See systemd.unit(5).
    DirectoryNotEmpty { path: String, negate: bool },
    /// ConditionPathIsReadWrite=/some/path (true if path exists on a read-write filesystem)
    /// ConditionPathIsReadWrite=!/some/path (true if path does NOT exist or is on a read-only filesystem)
    /// Checks whether the specified path exists and is on a read-write mounted
    /// filesystem (i.e. not mounted read-only). Uses access(2) with W_OK.
    /// See systemd.unit(5).
    PathIsReadWrite { path: String, negate: bool },
    /// ConditionControlGroupController=controller (true if the cgroup controller is available)
    /// ConditionControlGroupController=!controller (true if the cgroup controller is NOT available)
    /// Checks whether a given cgroup controller (e.g. `cpu`, `memory`, `io`,
    /// `pids`) is available for use in the system's cgroup hierarchy.
    /// On cgroupv2 this reads /sys/fs/cgroup/cgroup.controllers; on cgroupv1
    /// it reads /proc/cgroups. The special value `v2` checks whether the
    /// unified (cgroupv2) hierarchy is in use. See systemd.unit(5).
    ControlGroupController { controller: String, negate: bool },
}

/// The kind of virtualization detected (VM or container).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VirtKind {
    Vm,
    Container,
}

/// Detected virtualization: the technology name and its kind.
#[derive(Clone, Debug)]
pub struct DetectedVirt {
    pub name: String,
    pub kind: VirtKind,
}

/// Map a Linux capability name (e.g. "CAP_NET_ADMIN") to its bit number.
/// Returns `None` if the name is not recognized.
fn capability_name_to_bit(name: &str) -> Option<u64> {
    match name.to_uppercase().as_str() {
        "CAP_CHOWN" => Some(0),
        "CAP_DAC_OVERRIDE" => Some(1),
        "CAP_DAC_READ_SEARCH" => Some(2),
        "CAP_FOWNER" => Some(3),
        "CAP_FSETID" => Some(4),
        "CAP_KILL" => Some(5),
        "CAP_SETGID" => Some(6),
        "CAP_SETUID" => Some(7),
        "CAP_SETPCAP" => Some(8),
        "CAP_LINUX_IMMUTABLE" => Some(9),
        "CAP_NET_BIND_SERVICE" => Some(10),
        "CAP_NET_BROADCAST" => Some(11),
        "CAP_NET_ADMIN" => Some(12),
        "CAP_NET_RAW" => Some(13),
        "CAP_IPC_LOCK" => Some(14),
        "CAP_IPC_OWNER" => Some(15),
        "CAP_SYS_MODULE" => Some(16),
        "CAP_SYS_RAWIO" => Some(17),
        "CAP_SYS_CHROOT" => Some(18),
        "CAP_SYS_PTRACE" => Some(19),
        "CAP_SYS_PACCT" => Some(20),
        "CAP_SYS_ADMIN" => Some(21),
        "CAP_SYS_BOOT" => Some(22),
        "CAP_SYS_NICE" => Some(23),
        "CAP_SYS_RESOURCE" => Some(24),
        "CAP_SYS_TIME" => Some(25),
        "CAP_SYS_TTY_CONFIG" => Some(26),
        "CAP_MKNOD" => Some(27),
        "CAP_LEASE" => Some(28),
        "CAP_AUDIT_WRITE" => Some(29),
        "CAP_AUDIT_CONTROL" => Some(30),
        "CAP_SETFCAP" => Some(31),
        "CAP_MAC_OVERRIDE" => Some(32),
        "CAP_MAC_ADMIN" => Some(33),
        "CAP_SYSLOG" => Some(34),
        "CAP_WAKE_ALARM" => Some(35),
        "CAP_BLOCK_SUSPEND" => Some(36),
        "CAP_AUDIT_READ" => Some(37),
        "CAP_PERFMON" => Some(38),
        "CAP_BPF" => Some(39),
        "CAP_CHECKPOINT_RESTORE" => Some(40),
        _ => None,
    }
}

/// Check whether a capability (by bit number) is present in the capability
/// bounding set of the service manager. Reads `/proc/self/status` and parses
/// the `CapBnd:` hex field.
fn capability_in_bounding_set(bit: u64) -> bool {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(hex) = line.strip_prefix("CapBnd:") {
                let hex = hex.trim().trim_start_matches("0x");
                if let Ok(mask) = u64::from_str_radix(hex, 16) {
                    return (mask & (1u64 << bit)) != 0;
                }
            }
        }
    }
    // If we can't read the status, conservatively assume the capability is present
    // (matches systemd behavior for privileged service managers).
    true
}

/// Detect whether the system is running inside a virtualized environment.
///
/// Returns `Some(DetectedVirt)` with the technology name and kind, or `None`
/// if running on bare metal. The detection mirrors a subset of the logic from
/// `systemd-detect-virt(1)`.
fn detect_virtualization() -> Option<DetectedVirt> {
    // --- Container detection (higher priority) ---

    // Check /.dockerenv
    if std::path::Path::new("/.dockerenv").exists() {
        return Some(DetectedVirt {
            name: "docker".to_owned(),
            kind: VirtKind::Container,
        });
    }

    // Check /run/.containerenv (podman / buildah)
    if std::path::Path::new("/run/.containerenv").exists() {
        return Some(DetectedVirt {
            name: "podman".to_owned(),
            kind: VirtKind::Container,
        });
    }

    // Check the "container" environment variable of PID 1
    if let Ok(environ) = std::fs::read("/proc/1/environ") {
        // environ is NUL-separated KEY=VALUE pairs
        for entry in environ.split(|&b| b == 0) {
            if let Ok(s) = std::str::from_utf8(entry) {
                if let Some(val) = s.strip_prefix("container=") {
                    let name = match val {
                        "systemd-nspawn" => "systemd-nspawn",
                        "lxc" => "lxc",
                        "lxc-libvirt" => "lxc-libvirt",
                        "docker" => "docker",
                        "podman" => "podman",
                        "rkt" => "rkt",
                        "wsl" => "wsl",
                        "proot" => "proot",
                        "pouch" => "pouch",
                        "oci" => "docker", // generic OCI → docker
                        other => other,
                    };
                    return Some(DetectedVirt {
                        name: name.to_owned(),
                        kind: VirtKind::Container,
                    });
                }
            }
        }
    }

    // Check cgroup for container hints
    if let Ok(cgroup) = std::fs::read_to_string("/proc/1/cgroup") {
        let lower = cgroup.to_lowercase();
        if lower.contains("/docker/") || lower.contains("/docker-") {
            return Some(DetectedVirt {
                name: "docker".to_owned(),
                kind: VirtKind::Container,
            });
        }
        if lower.contains("/lxc/") || lower.contains("/lxc.payload") {
            return Some(DetectedVirt {
                name: "lxc".to_owned(),
                kind: VirtKind::Container,
            });
        }
    }

    // --- VM detection ---

    // Check /proc/xen for Xen guest
    if std::path::Path::new("/proc/xen").exists() {
        // If capabilities file contains "control_d" we are dom0 (host), not guest
        if let Ok(caps) = std::fs::read_to_string("/proc/xen/capabilities") {
            if !caps.contains("control_d") {
                return Some(DetectedVirt {
                    name: "xen".to_owned(),
                    kind: VirtKind::Vm,
                });
            }
        } else {
            return Some(DetectedVirt {
                name: "xen".to_owned(),
                kind: VirtKind::Vm,
            });
        }
    }

    // Check /sys/hypervisor/type
    if let Ok(htype) = std::fs::read_to_string("/sys/hypervisor/type") {
        let htype = htype.trim().to_lowercase();
        if htype == "xen" {
            return Some(DetectedVirt {
                name: "xen".to_owned(),
                kind: VirtKind::Vm,
            });
        }
    }

    // DMI-based detection: sys_vendor, product_name, board_vendor
    let dmi_files = [
        "/sys/class/dmi/id/sys_vendor",
        "/sys/class/dmi/id/product_name",
        "/sys/class/dmi/id/board_vendor",
    ];
    for path in &dmi_files {
        if let Ok(content) = std::fs::read_to_string(path) {
            let lower = content.trim().to_lowercase();
            if lower.contains("qemu") || lower.contains("kvm") {
                return Some(DetectedVirt {
                    name: "kvm".to_owned(),
                    kind: VirtKind::Vm,
                });
            }
            if lower.contains("vmware") {
                return Some(DetectedVirt {
                    name: "vmware".to_owned(),
                    kind: VirtKind::Vm,
                });
            }
            if lower.contains("microsoft") || lower.contains("hyper-v") {
                return Some(DetectedVirt {
                    name: "microsoft".to_owned(),
                    kind: VirtKind::Vm,
                });
            }
            if lower.contains("virtualbox") || lower.contains("oracle") {
                return Some(DetectedVirt {
                    name: "oracle".to_owned(),
                    kind: VirtKind::Vm,
                });
            }
            if lower.contains("parallels") {
                return Some(DetectedVirt {
                    name: "parallels".to_owned(),
                    kind: VirtKind::Vm,
                });
            }
            if lower.contains("bhyve") {
                return Some(DetectedVirt {
                    name: "bhyve".to_owned(),
                    kind: VirtKind::Vm,
                });
            }
            if lower.contains("bochs") {
                return Some(DetectedVirt {
                    name: "bochs".to_owned(),
                    kind: VirtKind::Vm,
                });
            }
            if lower.contains("apple virtualization") {
                return Some(DetectedVirt {
                    name: "apple".to_owned(),
                    kind: VirtKind::Vm,
                });
            }
            if lower.contains("google") {
                return Some(DetectedVirt {
                    name: "google".to_owned(),
                    kind: VirtKind::Vm,
                });
            }
            if lower.contains("acrn") {
                return Some(DetectedVirt {
                    name: "acrn".to_owned(),
                    kind: VirtKind::Vm,
                });
            }
        }
    }

    // Check /proc/cpuinfo for hypervisor flag
    if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
        for line in cpuinfo.lines() {
            if line.starts_with("flags") || line.starts_with("Features") {
                if line.contains(" hypervisor") || line.contains("\thypervisor") {
                    // Generic VM detected via CPUID hypervisor bit
                    return Some(DetectedVirt {
                        name: "vm-other".to_owned(),
                        kind: VirtKind::Vm,
                    });
                }
            }
        }
    }

    // Check for UML (User-Mode Linux)
    if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
        for line in cpuinfo.lines() {
            if line.starts_with("vendor_id") && line.contains("User Mode Linux") {
                return Some(DetectedVirt {
                    name: "uml".to_owned(),
                    kind: VirtKind::Vm,
                });
            }
        }
    }

    // Check for WSL (Windows Subsystem for Linux) via /proc/version
    if let Ok(version) = std::fs::read_to_string("/proc/version") {
        let lower = version.to_lowercase();
        if lower.contains("microsoft") || lower.contains("wsl") {
            return Some(DetectedVirt {
                name: "wsl".to_owned(),
                kind: VirtKind::Container,
            });
        }
    }

    None
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
            UnitCondition::Virtualization { value, negate } => {
                let detected = detect_virtualization();
                let result = match value.as_str() {
                    // Boolean: any virtualization at all?
                    "yes" | "true" | "1" => detected.is_some(),
                    "no" | "false" | "0" => detected.is_none(),
                    // Category checks
                    "vm" => detected.as_ref().map_or(false, |d| d.kind == VirtKind::Vm),
                    "container" => detected
                        .as_ref()
                        .map_or(false, |d| d.kind == VirtKind::Container),
                    // Specific technology name
                    tech => detected.as_ref().map_or(false, |d| d.name == tech),
                };
                if *negate {
                    !result
                } else {
                    result
                }
            }
            UnitCondition::Capability { capability, negate } => {
                let result = match capability_name_to_bit(capability) {
                    Some(bit) => capability_in_bounding_set(bit),
                    None => {
                        // Unknown capability name — treat as not present
                        warn!(
                            "Unknown capability name in ConditionCapability: {}",
                            capability
                        );
                        false
                    }
                };
                if *negate {
                    !result
                } else {
                    result
                }
            }
            UnitCondition::FirstBoot { value, negate } => {
                // systemd considers it "first boot" when /etc/machine-id
                // does not exist or is empty (uninitialized).
                let is_first_boot = match std::fs::metadata("/etc/machine-id") {
                    Ok(meta) => meta.len() == 0,
                    Err(_) => true, // file doesn't exist → first boot
                };
                let result = if *value {
                    is_first_boot
                } else {
                    !is_first_boot
                };
                if *negate {
                    !result
                } else {
                    result
                }
            }
            UnitCondition::FileIsExecutable { path, negate } => {
                use std::os::unix::fs::PermissionsExt;
                let is_executable = match std::fs::metadata(path) {
                    Ok(meta) => {
                        // Must be a regular file with at least one execute bit
                        meta.is_file() && (meta.permissions().mode() & 0o111 != 0)
                    }
                    Err(_) => false,
                };
                if *negate {
                    !is_executable
                } else {
                    is_executable
                }
            }
            UnitCondition::FileNotEmpty { path, negate } => {
                let is_non_empty = match std::fs::metadata(path) {
                    Ok(meta) => {
                        // Must be a regular file with size > 0
                        meta.is_file() && meta.len() > 0
                    }
                    Err(_) => false,
                };
                if *negate {
                    !is_non_empty
                } else {
                    is_non_empty
                }
            }
            UnitCondition::KernelModuleLoaded { module, negate } => {
                let is_loaded = match std::fs::read_to_string("/proc/modules") {
                    Ok(contents) => {
                        // Each line in /proc/modules starts with the module name
                        // followed by a space. We check for an exact module name match.
                        contents.lines().any(|line| {
                            line.split_whitespace()
                                .next()
                                .map_or(false, |name| name == module.as_str())
                        })
                    }
                    Err(_) => false,
                };
                if *negate {
                    !is_loaded
                } else {
                    is_loaded
                }
            }
            UnitCondition::DirectoryNotEmpty { path, negate } => {
                let is_non_empty = match std::fs::read_dir(path) {
                    Ok(mut entries) => {
                        // A directory is "not empty" if it contains at least one entry
                        entries.next().is_some()
                    }
                    Err(_) => false, // doesn't exist or isn't a directory
                };
                if *negate {
                    !is_non_empty
                } else {
                    is_non_empty
                }
            }
            UnitCondition::KernelCommandLine { argument, negate } => {
                let cmdline = std::fs::read_to_string("/proc/cmdline").unwrap_or_default();
                let result = if argument.contains('=') {
                    // Exact key=value match: look for the whole assignment as a
                    // whitespace-delimited token on the command line.
                    cmdline.split_whitespace().any(|token| token == argument)
                } else {
                    // Single word: match either the bare word or as the key
                    // part of a key=value pair.
                    cmdline.split_whitespace().any(|token| {
                        token == argument
                            || token
                                .split_once('=')
                                .map_or(false, |(key, _)| key == argument)
                    })
                };
                if *negate {
                    !result
                } else {
                    result
                }
            }
            UnitCondition::PathIsReadWrite { path, negate } => {
                // Check whether the path is on a read-write filesystem.
                // We use access(W_OK) which checks whether the process could
                // write to the path — if the filesystem is read-only this
                // will report false. For directories we check the directory
                // itself; for files we check the file.
                let is_rw = match nix::unistd::access(path.as_str(), nix::unistd::AccessFlags::W_OK)
                {
                    Ok(()) => true,
                    Err(_) => false,
                };
                if *negate {
                    !is_rw
                } else {
                    is_rw
                }
            }
            UnitCondition::ControlGroupController { controller, negate } => {
                let result = if controller == "v2" {
                    // Special value: check whether the unified (cgroupv2) hierarchy is in use.
                    // cgroupv2 mounts a "cgroup2" filesystem at /sys/fs/cgroup.
                    std::path::Path::new("/sys/fs/cgroup/cgroup.controllers").exists()
                } else {
                    // First try cgroupv2: read /sys/fs/cgroup/cgroup.controllers
                    let v2_available = std::fs::read_to_string("/sys/fs/cgroup/cgroup.controllers")
                        .map(|contents| {
                            contents
                                .split_whitespace()
                                .any(|c| c == controller.as_str())
                        })
                        .unwrap_or(false);
                    if v2_available {
                        true
                    } else {
                        // Fallback to cgroupv1: read /proc/cgroups
                        // Format: "controller\thierarchy\tnum_cgroups\tenabled"
                        std::fs::read_to_string("/proc/cgroups")
                            .map(|contents| {
                                contents.lines().any(|line| {
                                    if line.starts_with('#') {
                                        return false;
                                    }
                                    let mut fields = line.split_whitespace();
                                    let name = fields.next().unwrap_or("");
                                    // The 4th field is "enabled" (1 = yes)
                                    let enabled = fields.nth(2).unwrap_or("0");
                                    name == controller.as_str() && enabled == "1"
                                })
                            })
                            .unwrap_or(false)
                    }
                };
                if *negate {
                    !result
                } else {
                    result
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

/// DevicePolicy= — controls the policy for device access for the unit.
/// Matches systemd's `DevicePolicy=` resource-control setting.
/// See <https://www.freedesktop.org/software/systemd/man/systemd.resource-control.html#DevicePolicy=auto|closed|strict>.
#[derive(Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum DevicePolicy {
    /// No restrictions on device access (default).
    Auto,
    /// Only standard pseudo devices (/dev/null, /dev/zero, /dev/full,
    /// /dev/random, /dev/urandom, /dev/tty, /dev/pts/ptmx) plus any
    /// explicitly allowed via DeviceAllow= are accessible.
    Closed,
    /// Only devices explicitly listed in DeviceAllow= are accessible.
    Strict,
}

impl Default for DevicePolicy {
    fn default() -> Self {
        Self::Auto
    }
}

/// IOSchedulingClass= — sets the I/O scheduling class for executed processes.
/// Matches systemd's `IOSchedulingClass=` exec setting.
/// See <https://www.freedesktop.org/software/systemd/man/systemd.exec.html#IOSchedulingClass=>.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum IOSchedulingClass {
    /// No explicit class set; the kernel default (best-effort) is used.
    None,
    /// Realtime I/O scheduling (class 1). Processes in this class are
    /// always granted I/O access first. Use with caution — can starve
    /// other processes.
    Realtime,
    /// Best-effort I/O scheduling (class 2). The default scheduling class.
    /// I/O bandwidth is distributed based on the I/O priority within this
    /// class.
    BestEffort,
    /// Idle I/O scheduling (class 3). Processes in this class only get
    /// I/O time when no other process needs the disk.
    Idle,
}

impl Default for IOSchedulingClass {
    fn default() -> Self {
        Self::None
    }
}

/// Job mode for OnFailure= units.
/// Controls how the triggered failure units are enqueued.
/// Matches systemd's `OnFailureJobMode=` setting.
/// See <https://www.freedesktop.org/software/systemd/man/systemd.unit.html#OnFailureJobMode=>.
#[derive(Clone, Eq, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum OnFailureJobMode {
    /// Replace existing conflicting jobs (default).
    Replace,
    /// Fail if there are conflicting jobs already queued.
    Fail,
    /// Like `Replace`, but also cancel jobs in units that conflict.
    ReplaceIrreversibly,
    /// Start the unit and stop all other units.
    Isolate,
    /// Cancel all queued jobs and enqueue the new one.
    Flush,
    /// Ignore all unit dependency requirements.
    IgnoreDependencies,
    /// Ignore only `Requires=` dependencies.
    IgnoreRequirements,
}

impl Default for OnFailureJobMode {
    fn default() -> Self {
        Self::Replace
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

    /// Units this unit "binds to". Similar to `Requires=`, but additionally
    /// causes this unit to stop when the listed units stop (even cleanly).
    /// Matches systemd's `BindsTo=` setting.
    /// Parsed and stored; stop-propagation not yet enforced at runtime.
    pub binds_to: Vec<String>,

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
            binds_to: Vec::new(),
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
            refuse_manual_start: false,
            refuse_manual_stop: false,
            on_failure: Vec::new(),
            on_failure_job_mode: OnFailureJobMode::default(),
            start_limit_interval_sec: None,
            start_limit_burst: None,
            start_limit_action: UnitAction::default(),
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
    /// RuntimeDirectoryPreserve= — controls whether runtime directories
    /// (created by `RuntimeDirectory=`) are removed when the service stops.
    /// Can be `no` (default, always remove), `yes` (always preserve), or
    /// `restart` (preserve across restarts, remove on full stop). Parsed and
    /// stored; no runtime enforcement yet. See systemd.exec(5).
    pub runtime_directory_preserve: RuntimeDirectoryPreserve,
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

/// ProtectHome= — controls whether /home, /root, and /run/user are
/// accessible to the service. See systemd.exec(5).
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum ProtectHome {
    /// No protection (default). Directories are left accessible as normal.
    No,
    /// Make /home, /root, and /run/user inaccessible and empty for the
    /// service's processes.
    Yes,
    /// Make /home, /root, and /run/user read-only for the service's
    /// processes.
    ReadOnly,
    /// Mount a tmpfs file system on /home, /root, and /run/user for the
    /// service's processes. Files or directories below these may be
    /// created but are not visible to other processes.
    Tmpfs,
}

impl Default for ProtectHome {
    fn default() -> Self {
        Self::No
    }
}

/// ProtectProc= — controls the `hidepid=` mount option of the procfs instance
/// for the unit. Controls which `/proc/PID` directories are visible and
/// accessible. See systemd.exec(5).
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum ProtectProc {
    /// No restrictions on /proc/ access or visibility (default).
    Default,
    /// Most of other users' process metadata in /proc/ is taken away.
    Noaccess,
    /// Processes owned by other users are hidden from /proc/.
    Invisible,
    /// All processes that cannot be ptrace()'d are hidden.
    Ptraceable,
}

impl Default for ProtectProc {
    fn default() -> Self {
        Self::Default
    }
}

/// ProcSubset= — controls which subset of /proc/ is mounted for the unit.
/// Matches systemd's `ProcSubset=` exec setting.
/// See <https://www.freedesktop.org/software/systemd/man/systemd.exec.html#ProcSubset=>.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum ProcSubset {
    /// The full /proc/ file system is mounted (default).
    All,
    /// Only the /proc/$PID process-specific subdirectories are accessible;
    /// system-wide directories such as /proc/sys/, /proc/sysvipc/, etc.
    /// are not available.
    Pid,
}

impl Default for ProcSubset {
    fn default() -> Self {
        Self::All
    }
}

/// DeferTrigger= — controls whether to defer triggering the associated service
/// when a connection comes in. May only be used when Accept=no.
/// See systemd.socket(5).
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum DeferTrigger {
    /// Normal triggering behavior (default).
    No,
    /// Use lenient job mode; wait for job queue to complete; fail if
    /// conflict remains after jobs finish or timeout.
    Yes,
    /// Like Yes, but always wait until DeferTriggerMaxSec= elapses
    /// before giving up.
    Patient,
}

/// Timestamping= — controls the SO_TIMESTAMP or SO_TIMESTAMPNS socket
/// options, enabling timestamping metadata on ingress network traffic.
/// See systemd.socket(5).
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum Timestamping {
    /// No timestamping (default).
    Off,
    /// Microsecond-precision timestamping (SO_TIMESTAMP).
    Microseconds,
    /// Nanosecond-precision timestamping (SO_TIMESTAMPNS).
    Nanoseconds,
}

/// RuntimeDirectoryPreserve= — controls whether runtime directories created
/// by `RuntimeDirectory=` are removed when the service stops. See systemd.exec(5).
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum RuntimeDirectoryPreserve {
    /// Always remove runtime directories when the service is stopped (default).
    No,
    /// Always preserve runtime directories when the service is stopped.
    Yes,
    /// Preserve runtime directories across service restarts, but remove them
    /// when the service is fully stopped.
    Restart,
}

impl Default for RuntimeDirectoryPreserve {
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

/// A memory resource limit value used by MemoryMin=, MemoryLow=, MemoryHigh=,
/// MemoryMax=, MemorySwapMax=, etc. Accepts an absolute byte value (with
/// optional K, M, G, T, P, E suffixes using base 1024), a percentage of
/// physical memory, or "infinity". See systemd.resource-control(5).
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum MemoryLimit {
    /// Absolute byte value
    Bytes(u64),
    /// Percentage of physical memory (0–100)
    Percent(u64),
    /// No limit
    Infinity,
}

/// Parse a systemd byte value with optional K, M, G, T, P, E suffix (base 1024).
/// Also accepts percentage values (e.g. "30%") and "infinity".
/// Returns `None` for empty or unrecognised strings.
pub fn parse_memory_limit(val: &str) -> Result<Option<MemoryLimit>, String> {
    let val = val.trim();
    if val.is_empty() {
        return Ok(None);
    }
    if val.eq_ignore_ascii_case("infinity") {
        return Ok(Some(MemoryLimit::Infinity));
    }
    if let Some(pct) = val.strip_suffix('%') {
        let pct_val = pct
            .trim()
            .parse::<u64>()
            .map_err(|_| format!("memory limit percentage is not a valid number: {val}"))?;
        return Ok(Some(MemoryLimit::Percent(pct_val)));
    }
    // Delegate byte-value parsing (with optional K/M/G/T/P/E suffix) to parse_byte_size
    let bytes = parse_byte_size(val)
        .map_err(|_| format!("memory limit is not a valid byte value: {val}"))?;
    Ok(Some(MemoryLimit::Bytes(bytes)))
}

/// Parse a systemd byte-size value with optional K, M, G, T, P, E suffix (base 1024).
/// Returns the value in bytes as a `u64`.
///
/// Accepts plain integers (e.g. "8388608") as well as suffixed values
/// (e.g. "128K", "128M", "1G"). The suffix is case-insensitive.
/// Returns an error for empty, non-numeric, or otherwise invalid strings.
pub fn parse_byte_size(val: &str) -> Result<u64, String> {
    let val = val.trim();
    if val.is_empty() {
        return Err("byte size value is empty".to_owned());
    }
    let last = val.as_bytes()[val.len() - 1];
    let (num_str, multiplier): (&str, u64) = match last | 0x20 {
        // case-insensitive
        b'k' => (&val[..val.len() - 1], 1024),
        b'm' => (&val[..val.len() - 1], 1024 * 1024),
        b'g' => (&val[..val.len() - 1], 1024 * 1024 * 1024),
        b't' => (&val[..val.len() - 1], 1024 * 1024 * 1024 * 1024),
        b'p' => (&val[..val.len() - 1], 1024 * 1024 * 1024 * 1024 * 1024),
        b'e' => (
            &val[..val.len() - 1],
            1024 * 1024 * 1024 * 1024 * 1024 * 1024,
        ),
        _ => (val, 1),
    };
    let num = num_str
        .trim()
        .parse::<u64>()
        .map_err(|_| format!("not a valid byte size: {val}"))?;
    num.checked_mul(multiplier)
        .ok_or_else(|| format!("byte size overflows u64: {val}"))
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
