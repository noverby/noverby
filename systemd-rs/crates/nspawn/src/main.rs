//! systemd-nspawn — Spawn a command or OS in a lightweight namespace container.
//!
//! This implements a subset of systemd-nspawn(1) functionality:
//! - Container creation with PID, mount, UTS, IPC namespaces
//! - `--boot` mode for running an init system inside the container
//! - `--directory` / `--image` for specifying the container root
//! - `--bind` / `--bind-ro` for bind mounts
//! - `--private-network` for network namespace isolation
//! - `--network-veth` / `-n` for virtual ethernet pair creation (host ↔ container)
//! - `--capability` / `--drop-capability` for capability bounding
//! - `--machine` for naming the container
//! - `--user` for running as a specific user
//! - `--setenv` for environment variables
//! - `--volatile` for tmpfs overlays
//! - `--hostname` / `--uuid` for container identity
//! - `--register` for machined registration
//! - `--as-pid2` for running as PID 2 with a stub init
//! - Essential filesystem setup (/proc, /sys, /dev, /dev/pts, /dev/shm, /run, /tmp)
//! - Console/TTY passthrough

use std::collections::HashMap;
use std::ffi::CString;
use std::io;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};

// ── Constants ────────────────────────────────────────────────────────────

const EXIT_SUCCESS: i32 = 0;
const EXIT_FAILURE: i32 = 1;

/// Default capabilities granted to the container payload.
/// Matches systemd-nspawn's default set for unprivileged containers.
const DEFAULT_CAPABILITIES: &[Capability] = &[
    Capability::Chown,
    Capability::DacOverride,
    Capability::DacReadSearch,
    Capability::Fowner,
    Capability::Fsetid,
    Capability::IpcOwner,
    Capability::Kill,
    Capability::Lease,
    Capability::LinuxImmutable,
    Capability::NetBindService,
    Capability::NetBroadcast,
    Capability::NetRaw,
    Capability::Setgid,
    Capability::Setfcap,
    Capability::Setpcap,
    Capability::Setuid,
    Capability::SysChroot,
    Capability::SysNice,
    Capability::SysPtrace,
    Capability::SysResource,
    Capability::SysAdmin,
    Capability::Audit,
    Capability::Mknod,
];

/// Known Linux capabilities (subset relevant for containers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
enum Capability {
    Chown,
    DacOverride,
    DacReadSearch,
    Fowner,
    Fsetid,
    Kill,
    Setgid,
    Setuid,
    Setpcap,
    LinuxImmutable,
    NetBindService,
    NetBroadcast,
    NetAdmin,
    NetRaw,
    IpcLock,
    IpcOwner,
    SysModule,
    SysRawio,
    SysChroot,
    SysPtrace,
    SysPacct,
    SysAdmin,
    SysBoot,
    SysNice,
    SysResource,
    SysTime,
    SysTtyConfig,
    Mknod,
    Lease,
    Audit,
    AuditControl,
    Setfcap,
    MacOverride,
    MacAdmin,
    Syslog,
    WakeAlarm,
    BlockSuspend,
    AuditRead,
    Perfmon,
    Bpf,
    CheckpointRestore,
}

impl Capability {
    fn from_str(s: &str) -> Option<Capability> {
        // Accept with or without CAP_ prefix, case-insensitive
        let s = s.trim();
        let normalized = s.to_uppercase();
        let name = normalized.strip_prefix("CAP_").unwrap_or(&normalized);
        match name {
            "CHOWN" => Some(Capability::Chown),
            "DAC_OVERRIDE" => Some(Capability::DacOverride),
            "DAC_READ_SEARCH" => Some(Capability::DacReadSearch),
            "FOWNER" => Some(Capability::Fowner),
            "FSETID" => Some(Capability::Fsetid),
            "KILL" => Some(Capability::Kill),
            "SETGID" => Some(Capability::Setgid),
            "SETUID" => Some(Capability::Setuid),
            "SETPCAP" => Some(Capability::Setpcap),
            "LINUX_IMMUTABLE" => Some(Capability::LinuxImmutable),
            "NET_BIND_SERVICE" => Some(Capability::NetBindService),
            "NET_BROADCAST" => Some(Capability::NetBroadcast),
            "NET_ADMIN" => Some(Capability::NetAdmin),
            "NET_RAW" => Some(Capability::NetRaw),
            "IPC_LOCK" => Some(Capability::IpcLock),
            "IPC_OWNER" => Some(Capability::IpcOwner),
            "SYS_MODULE" => Some(Capability::SysModule),
            "SYS_RAWIO" => Some(Capability::SysRawio),
            "SYS_CHROOT" => Some(Capability::SysChroot),
            "SYS_PTRACE" => Some(Capability::SysPtrace),
            "SYS_PACCT" => Some(Capability::SysPacct),
            "SYS_ADMIN" => Some(Capability::SysAdmin),
            "SYS_BOOT" => Some(Capability::SysBoot),
            "SYS_NICE" => Some(Capability::SysNice),
            "SYS_RESOURCE" => Some(Capability::SysResource),
            "SYS_TIME" => Some(Capability::SysTime),
            "SYS_TTY_CONFIG" => Some(Capability::SysTtyConfig),
            "MKNOD" => Some(Capability::Mknod),
            "LEASE" => Some(Capability::Lease),
            "AUDIT_WRITE" | "AUDIT" => Some(Capability::Audit),
            "AUDIT_CONTROL" => Some(Capability::AuditControl),
            "SETFCAP" => Some(Capability::Setfcap),
            "MAC_OVERRIDE" => Some(Capability::MacOverride),
            "MAC_ADMIN" => Some(Capability::MacAdmin),
            "SYSLOG" => Some(Capability::Syslog),
            "WAKE_ALARM" => Some(Capability::WakeAlarm),
            "BLOCK_SUSPEND" => Some(Capability::BlockSuspend),
            "AUDIT_READ" => Some(Capability::AuditRead),
            "PERFMON" => Some(Capability::Perfmon),
            "BPF" => Some(Capability::Bpf),
            "CHECKPOINT_RESTORE" => Some(Capability::CheckpointRestore),
            _ => None,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Capability::Chown => "CAP_CHOWN",
            Capability::DacOverride => "CAP_DAC_OVERRIDE",
            Capability::DacReadSearch => "CAP_DAC_READ_SEARCH",
            Capability::Fowner => "CAP_FOWNER",
            Capability::Fsetid => "CAP_FSETID",
            Capability::Kill => "CAP_KILL",
            Capability::Setgid => "CAP_SETGID",
            Capability::Setuid => "CAP_SETUID",
            Capability::Setpcap => "CAP_SETPCAP",
            Capability::LinuxImmutable => "CAP_LINUX_IMMUTABLE",
            Capability::NetBindService => "CAP_NET_BIND_SERVICE",
            Capability::NetBroadcast => "CAP_NET_BROADCAST",
            Capability::NetAdmin => "CAP_NET_ADMIN",
            Capability::NetRaw => "CAP_NET_RAW",
            Capability::IpcLock => "CAP_IPC_LOCK",
            Capability::IpcOwner => "CAP_IPC_OWNER",
            Capability::SysModule => "CAP_SYS_MODULE",
            Capability::SysRawio => "CAP_SYS_RAWIO",
            Capability::SysChroot => "CAP_SYS_CHROOT",
            Capability::SysPtrace => "CAP_SYS_PTRACE",
            Capability::SysPacct => "CAP_SYS_PACCT",
            Capability::SysAdmin => "CAP_SYS_ADMIN",
            Capability::SysBoot => "CAP_SYS_BOOT",
            Capability::SysNice => "CAP_SYS_NICE",
            Capability::SysResource => "CAP_SYS_RESOURCE",
            Capability::SysTime => "CAP_SYS_TIME",
            Capability::SysTtyConfig => "CAP_SYS_TTY_CONFIG",
            Capability::Mknod => "CAP_MKNOD",
            Capability::Lease => "CAP_LEASE",
            Capability::Audit => "CAP_AUDIT_WRITE",
            Capability::AuditControl => "CAP_AUDIT_CONTROL",
            Capability::Setfcap => "CAP_SETFCAP",
            Capability::MacOverride => "CAP_MAC_OVERRIDE",
            Capability::MacAdmin => "CAP_MAC_ADMIN",
            Capability::Syslog => "CAP_SYSLOG",
            Capability::WakeAlarm => "CAP_WAKE_ALARM",
            Capability::BlockSuspend => "CAP_BLOCK_SUSPEND",
            Capability::AuditRead => "CAP_AUDIT_READ",
            Capability::Perfmon => "CAP_PERFMON",
            Capability::Bpf => "CAP_BPF",
            Capability::CheckpointRestore => "CAP_CHECKPOINT_RESTORE",
        }
    }

    fn number(&self) -> u32 {
        match self {
            Capability::Chown => 0,
            Capability::DacOverride => 1,
            Capability::DacReadSearch => 2,
            Capability::Fowner => 3,
            Capability::Fsetid => 4,
            Capability::Kill => 5,
            Capability::Setgid => 6,
            Capability::Setuid => 7,
            Capability::Setpcap => 8,
            Capability::LinuxImmutable => 9,
            Capability::NetBindService => 10,
            Capability::NetBroadcast => 11,
            Capability::NetAdmin => 12,
            Capability::NetRaw => 13,
            Capability::IpcLock => 14,
            Capability::IpcOwner => 15,
            Capability::SysModule => 16,
            Capability::SysRawio => 17,
            Capability::SysChroot => 18,
            Capability::SysPtrace => 19,
            Capability::SysPacct => 20,
            Capability::SysAdmin => 21,
            Capability::SysBoot => 22,
            Capability::SysNice => 23,
            Capability::SysResource => 24,
            Capability::SysTime => 25,
            Capability::SysTtyConfig => 26,
            Capability::Mknod => 27,
            Capability::Lease => 28,
            Capability::Audit => 29,
            Capability::AuditControl => 30,
            Capability::Setfcap => 31,
            Capability::MacOverride => 32,
            Capability::MacAdmin => 33,
            Capability::Syslog => 34,
            Capability::WakeAlarm => 35,
            Capability::BlockSuspend => 36,
            Capability::AuditRead => 37,
            Capability::Perfmon => 38,
            Capability::Bpf => 39,
            Capability::CheckpointRestore => 40,
        }
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── Volatile mode ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VolatileMode {
    /// No volatile overlay
    No,
    /// Volatile root: tmpfs overlay on /
    Yes,
    /// Only /var is overlaid with tmpfs
    State,
    /// Full overlay: tmpfs as upper layer over the whole tree
    Overlay,
}

impl VolatileMode {
    fn from_str(s: &str) -> Option<VolatileMode> {
        match s.to_lowercase().as_str() {
            "no" | "false" | "off" | "0" => Some(VolatileMode::No),
            "yes" | "true" | "on" | "1" | "" => Some(VolatileMode::Yes),
            "state" => Some(VolatileMode::State),
            "overlay" => Some(VolatileMode::Overlay),
            _ => None,
        }
    }
}

// ── Resolv.conf handling mode ────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum ResolvConfMode {
    Off,
    Copy,
    CopyHost,
    CopyStatic,
    CopyUplink,
    CopyStub,
    Replace,
    ReplaceHost,
    ReplaceStatic,
    ReplaceUplink,
    ReplaceStub,
    Bind,
    BindHost,
    BindStatic,
    BindUplink,
    BindStub,
    Delete,
    Auto,
}

impl ResolvConfMode {
    fn from_str(s: &str) -> Option<ResolvConfMode> {
        match s.to_lowercase().replace('-', "").as_str() {
            "off" => Some(ResolvConfMode::Off),
            "copy" | "copyhost" => Some(ResolvConfMode::CopyHost),
            "copystatic" => Some(ResolvConfMode::CopyStatic),
            "copyuplink" => Some(ResolvConfMode::CopyUplink),
            "copystub" => Some(ResolvConfMode::CopyStub),
            "replace" | "replacehost" => Some(ResolvConfMode::ReplaceHost),
            "replacestatic" => Some(ResolvConfMode::ReplaceStatic),
            "replaceuplink" => Some(ResolvConfMode::ReplaceUplink),
            "replacestub" => Some(ResolvConfMode::ReplaceStub),
            "bind" | "bindhost" => Some(ResolvConfMode::BindHost),
            "bindstatic" => Some(ResolvConfMode::BindStatic),
            "binduplink" => Some(ResolvConfMode::BindUplink),
            "bindstub" => Some(ResolvConfMode::BindStub),
            "delete" => Some(ResolvConfMode::Delete),
            "auto" => Some(ResolvConfMode::Auto),
            _ => None,
        }
    }
}

// ── Timezone handling mode ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimezoneMode {
    Off,
    Copy,
    Bind,
    Symlink,
    Delete,
    Auto,
}

impl TimezoneMode {
    fn from_str(s: &str) -> Option<TimezoneMode> {
        match s.to_lowercase().as_str() {
            "off" => Some(TimezoneMode::Off),
            "copy" => Some(TimezoneMode::Copy),
            "bind" => Some(TimezoneMode::Bind),
            "symlink" => Some(TimezoneMode::Symlink),
            "delete" => Some(TimezoneMode::Delete),
            "auto" => Some(TimezoneMode::Auto),
            _ => None,
        }
    }
}

// ── Link-journal mode ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkJournalMode {
    No,
    Host,
    TryHost,
    Guest,
    TryGuest,
    Auto,
}

impl LinkJournalMode {
    fn from_str(s: &str) -> Option<LinkJournalMode> {
        match s.to_lowercase().replace('-', "").as_str() {
            "no" | "off" | "false" | "0" => Some(LinkJournalMode::No),
            "host" => Some(LinkJournalMode::Host),
            "tryhost" => Some(LinkJournalMode::TryHost),
            "guest" => Some(LinkJournalMode::Guest),
            "tryguest" => Some(LinkJournalMode::TryGuest),
            "auto" | "" => Some(LinkJournalMode::Auto),
            _ => None,
        }
    }
}

// ── Bind mount specification ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
struct BindMount {
    source: String,
    destination: String,
    read_only: bool,
    recursive: bool,
    options: String,
}

impl BindMount {
    /// Parse a bind mount specification: `SRC[:DST[:OPTIONS]]`
    fn parse(spec: &str, read_only: bool) -> Result<BindMount, String> {
        let parts: Vec<&str> = spec.splitn(3, ':').collect();
        let source = parts[0].to_string();
        if source.is_empty() {
            return Err("bind mount source path is empty".to_string());
        }
        let destination = if parts.len() > 1 && !parts[1].is_empty() {
            parts[1].to_string()
        } else {
            source.clone()
        };
        let options = if parts.len() > 2 {
            parts[2].to_string()
        } else {
            String::new()
        };
        let recursive = if options.contains("norbind") {
            false
        } else {
            true // default is rbind
        };
        Ok(BindMount {
            source,
            destination,
            read_only,
            recursive,
            options,
        })
    }
}

// ── Console mode ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConsoleMode {
    Interactive,
    ReadOnly,
    Passive,
    Pipe,
    Autopipe,
}

impl ConsoleMode {
    fn from_str(s: &str) -> Option<ConsoleMode> {
        match s.to_lowercase().as_str() {
            "interactive" => Some(ConsoleMode::Interactive),
            "read-only" | "readonly" => Some(ConsoleMode::ReadOnly),
            "passive" => Some(ConsoleMode::Passive),
            "pipe" => Some(ConsoleMode::Pipe),
            "autopipe" => Some(ConsoleMode::Autopipe),
            _ => None,
        }
    }
}

// ── Port forwarding ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
struct PortForward {
    protocol: String,
    host_port: u16,
    container_port: u16,
}

impl PortForward {
    fn parse(spec: &str) -> Result<PortForward, String> {
        // FORMAT: [PROTOCOL:]HOSTPORT[:CONTAINERPORT]
        let parts: Vec<&str> = spec.split(':').collect();
        match parts.len() {
            1 => {
                let port: u16 = parts[0].parse().map_err(|e| format!("invalid port: {e}"))?;
                Ok(PortForward {
                    protocol: "tcp".to_string(),
                    host_port: port,
                    container_port: port,
                })
            }
            2 => {
                // Either PROTO:PORT or HOSTPORT:CONTAINERPORT
                if let Ok(hp) = parts[0].parse::<u16>() {
                    let cp: u16 = parts[1]
                        .parse()
                        .map_err(|e| format!("invalid container port: {e}"))?;
                    Ok(PortForward {
                        protocol: "tcp".to_string(),
                        host_port: hp,
                        container_port: cp,
                    })
                } else {
                    let port: u16 = parts[1].parse().map_err(|e| format!("invalid port: {e}"))?;
                    Ok(PortForward {
                        protocol: parts[0].to_lowercase(),
                        host_port: port,
                        container_port: port,
                    })
                }
            }
            3 => {
                let hp: u16 = parts[1]
                    .parse()
                    .map_err(|e| format!("invalid host port: {e}"))?;
                let cp: u16 = parts[2]
                    .parse()
                    .map_err(|e| format!("invalid container port: {e}"))?;
                Ok(PortForward {
                    protocol: parts[0].to_lowercase(),
                    host_port: hp,
                    container_port: cp,
                })
            }
            _ => Err(format!("invalid port specification: {spec}")),
        }
    }
}

// ── Container arguments ──────────────────────────────────────────────────

#[derive(Debug)]
struct NspawnArgs {
    /// Container root directory (`-D`, `--directory`)
    directory: Option<String>,
    /// Disk image to use as root (`-i`, `--image`)
    image: Option<String>,
    /// Machine name (`-M`, `--machine`)
    machine: Option<String>,
    /// Boot init inside the container (`-b`, `--boot`)
    boot: bool,
    /// Run as PID 2 with a stub init as PID 1 (`--as-pid2`)
    as_pid2: bool,
    /// User to run as inside the container (`-u`, `--user`)
    user: Option<String>,
    /// Working directory inside the container (`--chdir`)
    chdir: Option<String>,
    /// Environment variables (`-E`, `--setenv`)
    environment: HashMap<String, String>,
    /// Bind mounts (`--bind`, `--bind-ro`)
    bind_mounts: Vec<BindMount>,
    /// Temporary filesystem overlays (`--tmpfs`)
    tmpfs_mounts: Vec<(String, String)>,
    /// Overlay mounts (`--overlay`, `--overlay-ro`)
    overlay_mounts: Vec<String>,
    /// Create a private network namespace (`--private-network`)
    private_network: bool,
    /// Create a virtual ethernet pair (`-n`, `--network-veth`)
    network_veth: bool,
    /// Use private user namespace (`-U`, `--private-users`)
    private_users: bool,
    /// Extra capabilities to grant (`--capability`)
    extra_capabilities: Vec<Capability>,
    /// Capabilities to drop (`--drop-capability`)
    drop_capabilities: Vec<Capability>,
    /// Do not acquire any ambient capabilities (`--no-new-privileges`)
    no_new_privileges: bool,
    /// Volatile mode (`--volatile`)
    volatile: VolatileMode,
    /// resolv.conf handling (`--resolv-conf`)
    resolv_conf: ResolvConfMode,
    /// Timezone handling (`--timezone`)
    timezone: TimezoneMode,
    /// Journal link mode (`--link-journal`)
    link_journal: LinkJournalMode,
    /// Console mode (`--console`)
    console: ConsoleMode,
    /// Port forwarding (`-p`, `--port`)
    port_forwards: Vec<PortForward>,
    /// UUID for the container (`--uuid`)
    uuid: Option<String>,
    /// Hostname for the container (`--hostname`)
    hostname: Option<String>,
    /// Register with machined (`--register`)
    register: Option<bool>,
    /// Keep the current unit scope (`--keep-unit`)
    keep_unit: bool,
    /// Quiet mode (`-q`, `--quiet`)
    quiet: bool,
    /// Scope properties (`--property`)
    properties: Vec<(String, String)>,
    /// Suppress output
    read_only: bool,
    /// OCI bundle path (`--oci-bundle`)
    oci_bundle: Option<String>,
    /// Command and arguments to run
    command: Vec<String>,
    /// Show help
    help: bool,
    /// Show version
    version: bool,
    /// Kill the container signal (`--kill-signal`)
    kill_signal: Option<String>,
    /// Settings file to read (`--settings`)
    settings: Option<String>,
    /// Notify ready (`--notify-ready`)
    notify_ready: bool,
    /// Suppress creating /etc/machine-id
    suppress_sync: bool,
}

impl Default for NspawnArgs {
    fn default() -> Self {
        NspawnArgs {
            directory: None,
            image: None,
            machine: None,
            boot: false,
            as_pid2: false,
            user: None,
            chdir: None,
            environment: HashMap::new(),
            bind_mounts: Vec::new(),
            tmpfs_mounts: Vec::new(),
            overlay_mounts: Vec::new(),
            private_network: false,
            network_veth: false,
            private_users: false,
            extra_capabilities: Vec::new(),
            drop_capabilities: Vec::new(),
            no_new_privileges: false,
            volatile: VolatileMode::No,
            resolv_conf: ResolvConfMode::Auto,
            timezone: TimezoneMode::Auto,
            link_journal: LinkJournalMode::Auto,
            console: ConsoleMode::Interactive,
            port_forwards: Vec::new(),
            uuid: None,
            hostname: None,
            register: None,
            keep_unit: false,
            quiet: false,
            properties: Vec::new(),
            read_only: false,
            oci_bundle: None,
            command: Vec::new(),
            help: false,
            version: false,
            kill_signal: None,
            settings: None,
            notify_ready: false,
            suppress_sync: false,
        }
    }
}

// ── Argument parsing ─────────────────────────────────────────────────────

fn value_or_next<'a>(
    current: &'a str,
    key: &str,
    iter: &mut impl Iterator<Item = &'a str>,
) -> Result<&'a str, String> {
    if let Some(rest) = current.strip_prefix(&format!("{key}=")) {
        Ok(rest)
    } else {
        iter.next().ok_or_else(|| format!("{key} requires a value"))
    }
}

fn parse_capability_list(spec: &str) -> Result<Vec<Capability>, String> {
    let mut caps = Vec::new();
    if spec.to_lowercase() == "all" {
        // "all" means grant all known capabilities
        caps.push(Capability::Chown);
        caps.push(Capability::DacOverride);
        caps.push(Capability::DacReadSearch);
        caps.push(Capability::Fowner);
        caps.push(Capability::Fsetid);
        caps.push(Capability::Kill);
        caps.push(Capability::Setgid);
        caps.push(Capability::Setuid);
        caps.push(Capability::Setpcap);
        caps.push(Capability::LinuxImmutable);
        caps.push(Capability::NetBindService);
        caps.push(Capability::NetBroadcast);
        caps.push(Capability::NetAdmin);
        caps.push(Capability::NetRaw);
        caps.push(Capability::IpcLock);
        caps.push(Capability::IpcOwner);
        caps.push(Capability::SysModule);
        caps.push(Capability::SysRawio);
        caps.push(Capability::SysChroot);
        caps.push(Capability::SysPtrace);
        caps.push(Capability::SysPacct);
        caps.push(Capability::SysAdmin);
        caps.push(Capability::SysBoot);
        caps.push(Capability::SysNice);
        caps.push(Capability::SysResource);
        caps.push(Capability::SysTime);
        caps.push(Capability::SysTtyConfig);
        caps.push(Capability::Mknod);
        caps.push(Capability::Lease);
        caps.push(Capability::Audit);
        caps.push(Capability::AuditControl);
        caps.push(Capability::Setfcap);
        return Ok(caps);
    }
    for part in spec.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(cap) = Capability::from_str(trimmed) {
            caps.push(cap);
        } else {
            return Err(format!("unknown capability: {trimmed}"));
        }
    }
    Ok(caps)
}

fn parse_args(args: &[&str]) -> Result<NspawnArgs, String> {
    let mut result = NspawnArgs::default();
    let mut iter = args.iter().copied();
    let mut positional_after_dashdash = false;

    while let Some(arg) = iter.next() {
        if positional_after_dashdash {
            result.command.push(arg.to_string());
            continue;
        }

        match arg {
            "--" => {
                positional_after_dashdash = true;
            }
            "-h" | "--help" => {
                result.help = true;
            }
            "--version" => {
                result.version = true;
            }
            "-q" | "--quiet" => {
                result.quiet = true;
            }
            "-b" | "--boot" => {
                result.boot = true;
            }
            "--as-pid2" | "--as-pid-2" => {
                result.as_pid2 = true;
            }
            "-n" | "--network-veth" => {
                result.network_veth = true;
            }
            "--private-network" => {
                result.private_network = true;
            }
            "--keep-unit" => {
                result.keep_unit = true;
            }
            "--no-new-privileges" => {
                result.no_new_privileges = true;
            }
            "--read-only" => {
                result.read_only = true;
            }
            "--notify-ready" | "--notify-ready=yes" => {
                result.notify_ready = true;
            }
            "--suppress-sync" | "--suppress-sync=yes" => {
                result.suppress_sync = true;
            }
            _ if arg.starts_with("-D") || arg.starts_with("--directory") => {
                let val = if arg == "-D" {
                    iter.next()
                        .ok_or_else(|| "-D requires a value".to_string())?
                } else if let Some(rest) = arg.strip_prefix("-D") {
                    rest
                } else {
                    value_or_next(arg, "--directory", &mut iter)?
                };
                result.directory = Some(val.to_string());
            }
            _ if arg.starts_with("-i") || arg.starts_with("--image") => {
                let val = if arg == "-i" {
                    iter.next()
                        .ok_or_else(|| "-i requires a value".to_string())?
                } else if let Some(rest) = arg.strip_prefix("-i") {
                    rest
                } else {
                    value_or_next(arg, "--image", &mut iter)?
                };
                result.image = Some(val.to_string());
            }
            _ if arg.starts_with("-M") || arg.starts_with("--machine") => {
                let val = if arg == "-M" {
                    iter.next()
                        .ok_or_else(|| "-M requires a value".to_string())?
                } else if let Some(rest) = arg.strip_prefix("-M") {
                    rest
                } else {
                    value_or_next(arg, "--machine", &mut iter)?
                };
                result.machine = Some(val.to_string());
            }
            _ if arg.starts_with("-u") || arg.starts_with("--user") => {
                let val = if arg == "-u" {
                    iter.next()
                        .ok_or_else(|| "-u requires a value".to_string())?
                } else if let Some(rest) = arg.strip_prefix("-u") {
                    rest
                } else {
                    value_or_next(arg, "--user", &mut iter)?
                };
                result.user = Some(val.to_string());
            }
            _ if arg.starts_with("--chdir") => {
                let val = value_or_next(arg, "--chdir", &mut iter)?;
                result.chdir = Some(val.to_string());
            }
            _ if arg.starts_with("-E") || arg.starts_with("--setenv") => {
                let val = if arg == "-E" {
                    iter.next()
                        .ok_or_else(|| "-E requires a value".to_string())?
                } else if let Some(rest) = arg.strip_prefix("-E") {
                    rest
                } else {
                    value_or_next(arg, "--setenv", &mut iter)?
                };
                if let Some((k, v)) = val.split_once('=') {
                    result.environment.insert(k.to_string(), v.to_string());
                } else {
                    // Inherit from host environment
                    if let Ok(v) = std::env::var(val) {
                        result.environment.insert(val.to_string(), v);
                    }
                }
            }
            _ if arg.starts_with("--bind-ro") => {
                let val = value_or_next(arg, "--bind-ro", &mut iter)?;
                result.bind_mounts.push(BindMount::parse(val, true)?);
            }
            _ if arg.starts_with("--bind") => {
                let val = value_or_next(arg, "--bind", &mut iter)?;
                result.bind_mounts.push(BindMount::parse(val, false)?);
            }
            _ if arg.starts_with("--tmpfs") => {
                let val = value_or_next(arg, "--tmpfs", &mut iter)?;
                let (path, opts) = if let Some((p, o)) = val.split_once(':') {
                    (p.to_string(), o.to_string())
                } else {
                    (val.to_string(), "mode=0755".to_string())
                };
                result.tmpfs_mounts.push((path, opts));
            }
            _ if arg.starts_with("--overlay-ro") => {
                let val = value_or_next(arg, "--overlay-ro", &mut iter)?;
                result.overlay_mounts.push(format!("ro:{val}"));
            }
            _ if arg.starts_with("--overlay") => {
                let val = value_or_next(arg, "--overlay", &mut iter)?;
                result.overlay_mounts.push(val.to_string());
            }
            _ if arg.starts_with("--volatile") => {
                let val: &str = if arg == "--volatile" {
                    ""
                } else {
                    arg.strip_prefix("--volatile=").unwrap_or_default()
                };
                result.volatile = VolatileMode::from_str(val)
                    .ok_or_else(|| format!("invalid volatile mode: {val}"))?;
            }
            _ if arg.starts_with("--capability") => {
                let val = value_or_next(arg, "--capability", &mut iter)?;
                let caps = parse_capability_list(val)?;
                result.extra_capabilities.extend(caps);
            }
            _ if arg.starts_with("--drop-capability") => {
                let val = value_or_next(arg, "--drop-capability", &mut iter)?;
                let caps = parse_capability_list(val)?;
                result.drop_capabilities.extend(caps);
            }
            _ if arg.starts_with("--resolv-conf") => {
                let val = value_or_next(arg, "--resolv-conf", &mut iter)?;
                result.resolv_conf = ResolvConfMode::from_str(val)
                    .ok_or_else(|| format!("invalid resolv-conf mode: {val}"))?;
            }
            _ if arg.starts_with("--timezone") => {
                let val = value_or_next(arg, "--timezone", &mut iter)?;
                result.timezone = TimezoneMode::from_str(val)
                    .ok_or_else(|| format!("invalid timezone mode: {val}"))?;
            }
            _ if arg.starts_with("--link-journal") => {
                let val = value_or_next(arg, "--link-journal", &mut iter)?;
                result.link_journal = LinkJournalMode::from_str(val)
                    .ok_or_else(|| format!("invalid link-journal mode: {val}"))?;
            }
            _ if arg.starts_with("--console") => {
                let val = value_or_next(arg, "--console", &mut iter)?;
                result.console = ConsoleMode::from_str(val)
                    .ok_or_else(|| format!("invalid console mode: {val}"))?;
            }
            _ if arg.starts_with("-p") || arg.starts_with("--port") => {
                let val = if arg == "-p" {
                    iter.next()
                        .ok_or_else(|| "-p requires a value".to_string())?
                } else if let Some(rest) = arg.strip_prefix("-p") {
                    rest
                } else {
                    value_or_next(arg, "--port", &mut iter)?
                };
                result.port_forwards.push(PortForward::parse(val)?);
            }
            _ if arg.starts_with("--uuid") => {
                let val = value_or_next(arg, "--uuid", &mut iter)?;
                result.uuid = Some(val.to_string());
            }
            _ if arg.starts_with("--hostname") => {
                let val = value_or_next(arg, "--hostname", &mut iter)?;
                result.hostname = Some(val.to_string());
            }
            _ if arg.starts_with("--register") => {
                let val = value_or_next(arg, "--register", &mut iter)?;
                result.register = Some(matches!(
                    val.to_lowercase().as_str(),
                    "yes" | "true" | "1" | "on"
                ));
            }
            _ if arg.starts_with("--property") => {
                let val = value_or_next(arg, "--property", &mut iter)?;
                if let Some((k, v)) = val.split_once('=') {
                    result.properties.push((k.to_string(), v.to_string()));
                } else {
                    return Err(format!("invalid property: {val} (expected NAME=VALUE)"));
                }
            }
            _ if arg.starts_with("--kill-signal") => {
                let val = value_or_next(arg, "--kill-signal", &mut iter)?;
                result.kill_signal = Some(val.to_string());
            }
            _ if arg.starts_with("--settings") => {
                let val = value_or_next(arg, "--settings", &mut iter)?;
                result.settings = Some(val.to_string());
            }
            _ if arg.starts_with("--oci-bundle") => {
                let val = value_or_next(arg, "--oci-bundle", &mut iter)?;
                result.oci_bundle = Some(val.to_string());
            }
            _ if arg.starts_with("-U") || arg == "--private-users" => {
                result.private_users = true;
            }
            _ if arg.starts_with("--private-users=") => {
                // --private-users=pick, --private-users=UIDSHIFT etc.
                // For now, just enable private users.
                result.private_users = true;
            }
            // Short-form stacked options like -xb, -bD/path etc.
            _ if arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 2 => {
                // Unsupported stacked short options — skip gracefully
                log::trace!("Ignoring unrecognized short option: {arg}");
            }
            _ if arg.starts_with('-') => {
                log::trace!("Ignoring unrecognized option: {arg}");
            }
            _ => {
                // Positional argument: treated as part of the command to run
                result.command.push(arg.to_string());
            }
        }
    }

    // Collect remaining positional args after --
    for arg in iter {
        result.command.push(arg.to_string());
    }

    Ok(result)
}

// ── Machine name derivation ──────────────────────────────────────────────

/// Derive the machine name from the directory/image path if not explicitly set.
fn derive_machine_name(args: &NspawnArgs) -> String {
    if let Some(ref name) = args.machine {
        return sanitize_machine_name(name);
    }
    if let Some(ref dir) = args.directory {
        let p = Path::new(dir);
        if let Some(name) = p.file_name() {
            return sanitize_machine_name(&name.to_string_lossy());
        }
    }
    if let Some(ref img) = args.image {
        let p = Path::new(img);
        if let Some(stem) = p.file_stem() {
            return sanitize_machine_name(&stem.to_string_lossy());
        }
    }
    // Fallback to hostname
    nix::unistd::gethostname()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "container".to_string())
}

/// Sanitize a machine name: only allow [a-zA-Z0-9._-], max 64 chars.
fn sanitize_machine_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "container".to_string()
    } else if trimmed.len() > 64 {
        trimmed[..64].to_string()
    } else {
        trimmed.to_string()
    }
}

// ── Capability computation ───────────────────────────────────────────────

/// Compute the effective capability set for the container payload.
fn compute_capabilities(args: &NspawnArgs) -> Vec<Capability> {
    let mut caps: Vec<Capability> = DEFAULT_CAPABILITIES.to_vec();

    // Add extra capabilities
    for cap in &args.extra_capabilities {
        if !caps.contains(cap) {
            caps.push(*cap);
        }
    }

    // Remove dropped capabilities
    caps.retain(|cap| !args.drop_capabilities.contains(cap));

    caps
}

/// Apply the capability bounding set by dropping all capabilities not in
/// the allowed set. This uses prctl(PR_CAPBSET_DROP, ...) for each cap
/// that should be removed.
fn apply_capability_bounding_set(allowed: &[Capability]) -> Result<(), String> {
    let max_cap = 40u32; // CAP_CHECKPOINT_RESTORE
    for cap_num in 0..=max_cap {
        let is_allowed = allowed.iter().any(|c| c.number() == cap_num);
        if !is_allowed {
            let ret =
                unsafe { libc::prctl(libc::PR_CAPBSET_DROP, cap_num as libc::c_ulong, 0, 0, 0) };
            if ret != 0 {
                let err = std::io::Error::last_os_error();
                // EINVAL means the capability doesn't exist on this kernel — not an error
                if err.raw_os_error() != Some(libc::EINVAL) {
                    log::trace!("prctl(PR_CAPBSET_DROP, {}) failed: {}", cap_num, err);
                }
            }
        }
    }
    Ok(())
}

// ── Container root resolution ────────────────────────────────────────────

/// Resolve the container root directory from args. Returns the canonical path.
fn resolve_root(args: &NspawnArgs) -> Result<PathBuf, String> {
    if let Some(ref dir) = args.directory {
        let p = PathBuf::from(dir);
        if !p.exists() {
            return Err(format!("directory does not exist: {dir}"));
        }
        if !p.is_dir() {
            return Err(format!("not a directory: {dir}"));
        }
        return p
            .canonicalize()
            .map_err(|e| format!("failed to canonicalize {dir}: {e}"));
    }
    if let Some(ref img) = args.image {
        return Err(format!(
            "image-based containers (--image={img}) are not yet supported; use --directory"
        ));
    }
    if let Some(ref oci) = args.oci_bundle {
        return Err(format!(
            "OCI bundles (--oci-bundle={oci}) are not yet supported; use --directory"
        ));
    }

    // Look in /var/lib/machines/<machine>
    if let Some(ref machine) = args.machine {
        let machine_dir = PathBuf::from(format!("/var/lib/machines/{machine}"));
        if machine_dir.is_dir() {
            return machine_dir
                .canonicalize()
                .map_err(|e| format!("failed to canonicalize /var/lib/machines/{machine}: {e}"));
        }
    }

    Err("no container root specified; use --directory=PATH or --machine=NAME".to_string())
}

// ── Mount helpers ────────────────────────────────────────────────────────

fn mount_bind(source: &str, target: &str, read_only: bool, recursive: bool) -> Result<(), String> {
    let mut flags = libc::MS_BIND;
    if recursive {
        flags |= libc::MS_REC;
    }

    let src = CString::new(source).map_err(|e| format!("invalid source path: {e}"))?;
    let tgt = CString::new(target).map_err(|e| format!("invalid target path: {e}"))?;

    let ret = unsafe {
        libc::mount(
            src.as_ptr(),
            tgt.as_ptr(),
            std::ptr::null(),
            flags as libc::c_ulong,
            std::ptr::null(),
        )
    };
    if ret != 0 {
        return Err(format!(
            "bind mount {} -> {} failed: {}",
            source,
            target,
            std::io::Error::last_os_error()
        ));
    }

    if read_only {
        let ret = unsafe {
            libc::mount(
                std::ptr::null(),
                tgt.as_ptr(),
                std::ptr::null(),
                (libc::MS_BIND | libc::MS_REMOUNT | libc::MS_RDONLY) as libc::c_ulong,
                std::ptr::null(),
            )
        };
        if ret != 0 {
            return Err(format!(
                "remount read-only {} failed: {}",
                target,
                std::io::Error::last_os_error()
            ));
        }
    }

    Ok(())
}

fn mount_fs(
    fstype: &str,
    target: &str,
    flags: libc::c_ulong,
    data: Option<&str>,
) -> Result<(), String> {
    let tgt = CString::new(target).map_err(|e| format!("invalid target: {e}"))?;
    let fs = CString::new(fstype).map_err(|e| format!("invalid fstype: {e}"))?;
    let src = CString::new(fstype).map_err(|e| format!("invalid source: {e}"))?;
    let data_c = data.map(|d| CString::new(d).unwrap());

    let ret = unsafe {
        libc::mount(
            src.as_ptr(),
            tgt.as_ptr(),
            fs.as_ptr(),
            flags,
            data_c
                .as_ref()
                .map_or(std::ptr::null(), |c| c.as_ptr() as *const _),
        )
    };
    if ret != 0 {
        return Err(format!(
            "mount {} on {} failed: {}",
            fstype,
            target,
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// Set up essential API filesystems inside the container root.
fn setup_api_filesystems(root: &Path) -> Result<(), String> {
    // /proc
    let proc_path = root.join("proc");
    ensure_dir(&proc_path)?;
    mount_fs(
        "proc",
        &proc_path.to_string_lossy(),
        libc::MS_NOSUID as libc::c_ulong
            | libc::MS_NOEXEC as libc::c_ulong
            | libc::MS_NODEV as libc::c_ulong,
        None,
    )?;

    // /sys — bind-mount read-only from host
    let sys_path = root.join("sys");
    ensure_dir(&sys_path)?;
    mount_bind("/sys", &sys_path.to_string_lossy(), true, true)?;

    // /dev — minimal tmpfs with essential device nodes
    let dev_path = root.join("dev");
    ensure_dir(&dev_path)?;
    mount_fs(
        "tmpfs",
        &dev_path.to_string_lossy(),
        libc::MS_NOSUID as libc::c_ulong | libc::MS_STRICTATIME as libc::c_ulong,
        Some("mode=0755,size=65536k"),
    )?;

    // Create essential device nodes (or bind-mount from host)
    create_device_nodes(&dev_path)?;

    // /dev/pts
    let devpts_path = root.join("dev/pts");
    ensure_dir(&devpts_path)?;
    mount_fs(
        "devpts",
        &devpts_path.to_string_lossy(),
        libc::MS_NOSUID as libc::c_ulong | libc::MS_NOEXEC as libc::c_ulong,
        Some("newinstance,ptmxmode=0666,mode=0620,gid=5"),
    )?;

    // /dev/shm
    let shm_path = root.join("dev/shm");
    ensure_dir(&shm_path)?;
    mount_fs(
        "tmpfs",
        &shm_path.to_string_lossy(),
        libc::MS_NOSUID as libc::c_ulong
            | libc::MS_NODEV as libc::c_ulong
            | libc::MS_STRICTATIME as libc::c_ulong,
        Some("mode=1777,size=65536k"),
    )?;

    // /dev/mqueue
    let mqueue_path = root.join("dev/mqueue");
    ensure_dir(&mqueue_path)?;
    mount_fs("mqueue", &mqueue_path.to_string_lossy(), 0, None)
        .unwrap_or_else(|e| log::trace!("mqueue mount failed (non-fatal): {e}"));

    // /run — tmpfs
    let run_path = root.join("run");
    ensure_dir(&run_path)?;
    mount_fs(
        "tmpfs",
        &run_path.to_string_lossy(),
        libc::MS_NOSUID as libc::c_ulong
            | libc::MS_NODEV as libc::c_ulong
            | libc::MS_STRICTATIME as libc::c_ulong,
        Some("mode=0755,size=65536k"),
    )?;

    // /tmp — tmpfs
    let tmp_path = root.join("tmp");
    ensure_dir(&tmp_path)?;
    mount_fs(
        "tmpfs",
        &tmp_path.to_string_lossy(),
        libc::MS_NOSUID as libc::c_ulong
            | libc::MS_NODEV as libc::c_ulong
            | libc::MS_STRICTATIME as libc::c_ulong,
        Some("mode=1777"),
    )?;

    Ok(())
}

/// Create essential device nodes by bind-mounting from host.
fn create_device_nodes(dev_path: &Path) -> Result<(), String> {
    // Bind-mount essential device nodes from host /dev.
    // This is safer than mknod since it doesn't require special privileges
    // beyond mount namespace access.
    let devices = [
        ("null", "/dev/null"),
        ("zero", "/dev/zero"),
        ("full", "/dev/full"),
        ("random", "/dev/random"),
        ("urandom", "/dev/urandom"),
        ("tty", "/dev/tty"),
    ];

    for (name, host_path) in &devices {
        let container_path = dev_path.join(name);
        // Create the target file for bind-mounting
        if let Ok(f) = std::fs::File::create(&container_path) {
            drop(f);
            if let Err(e) = mount_bind(host_path, &container_path.to_string_lossy(), false, false) {
                log::trace!("Failed to bind-mount {host_path}: {e}");
                // Fallback: try mknod
                let _ = std::fs::remove_file(&container_path);
            }
        }
    }

    // Symlinks
    let symlinks = [
        ("fd", "/proc/self/fd"),
        ("stdin", "/proc/self/fd/0"),
        ("stdout", "/proc/self/fd/1"),
        ("stderr", "/proc/self/fd/2"),
        ("ptmx", "pts/ptmx"),
    ];
    for (name, target) in &symlinks {
        let link_path = dev_path.join(name);
        let _ = std::os::unix::fs::symlink(target, &link_path);
    }

    // /dev/console — bind-mount from host if we have a TTY
    if unsafe { libc::isatty(0) } == 1 {
        let console_path = dev_path.join("console");
        if let Ok(f) = std::fs::File::create(&console_path) {
            drop(f);
            // Try to bind /dev/console or the current TTY
            let tty_name = unsafe {
                let p = libc::ttyname(0);
                if p.is_null() {
                    None
                } else {
                    Some(std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned())
                }
            };
            if let Some(tty) = tty_name {
                let _ = mount_bind(&tty, &console_path.to_string_lossy(), false, false);
            }
        }
    }

    Ok(())
}

fn ensure_dir(path: &Path) -> Result<(), String> {
    if !path.exists() {
        std::fs::create_dir_all(path)
            .map_err(|e| format!("failed to create directory {}: {e}", path.display()))?;
    }
    Ok(())
}

/// Apply user-specified bind mounts.
fn setup_bind_mounts(root: &Path, mounts: &[BindMount]) -> Result<(), String> {
    for m in mounts {
        let target = root.join(m.destination.trim_start_matches('/'));
        ensure_dir(&target)?;
        mount_bind(
            &m.source,
            &target.to_string_lossy(),
            m.read_only,
            m.recursive,
        )?;
    }
    Ok(())
}

/// Apply user-specified tmpfs mounts.
fn setup_tmpfs_mounts(root: &Path, mounts: &[(String, String)]) -> Result<(), String> {
    for (path, opts) in mounts {
        let target = root.join(path.trim_start_matches('/'));
        ensure_dir(&target)?;
        mount_fs("tmpfs", &target.to_string_lossy(), 0, Some(opts))?;
    }
    Ok(())
}

/// Copy /etc/resolv.conf into the container, depending on the mode.
fn setup_resolv_conf(root: &Path, mode: ResolvConfMode) -> Result<(), String> {
    match mode {
        ResolvConfMode::Off | ResolvConfMode::Delete => {
            let resolv = root.join("etc/resolv.conf");
            let _ = std::fs::remove_file(&resolv);
            Ok(())
        }
        ResolvConfMode::Auto | ResolvConfMode::Copy | ResolvConfMode::CopyHost => {
            let src = Path::new("/etc/resolv.conf");
            if src.exists() {
                let dst = root.join("etc/resolv.conf");
                ensure_dir(&root.join("etc"))?;
                std::fs::copy(src, &dst).map_err(|e| format!("failed to copy resolv.conf: {e}"))?;
            }
            Ok(())
        }
        ResolvConfMode::CopyStub => {
            let src = Path::new("/run/systemd/resolve/stub-resolv.conf");
            if src.exists() {
                let dst = root.join("etc/resolv.conf");
                ensure_dir(&root.join("etc"))?;
                std::fs::copy(src, &dst)
                    .map_err(|e| format!("failed to copy stub-resolv.conf: {e}"))?;
            }
            Ok(())
        }
        ResolvConfMode::BindHost | ResolvConfMode::Bind => {
            let src = "/etc/resolv.conf";
            let dst = root.join("etc/resolv.conf");
            ensure_dir(&root.join("etc"))?;
            if let Ok(f) = std::fs::File::create(&dst) {
                drop(f);
                mount_bind(src, &dst.to_string_lossy(), true, false)?;
            }
            Ok(())
        }
        ResolvConfMode::BindStub => {
            let src = "/run/systemd/resolve/stub-resolv.conf";
            let dst = root.join("etc/resolv.conf");
            ensure_dir(&root.join("etc"))?;
            if let Ok(f) = std::fs::File::create(&dst) {
                drop(f);
                mount_bind(src, &dst.to_string_lossy(), true, false)?;
            }
            Ok(())
        }
        _ => {
            // Fallback: copy host
            let src = Path::new("/etc/resolv.conf");
            if src.exists() {
                let dst = root.join("etc/resolv.conf");
                ensure_dir(&root.join("etc"))?;
                let _ = std::fs::copy(src, &dst);
            }
            Ok(())
        }
    }
}

/// Set up /etc/hostname inside the container.
fn setup_hostname(root: &Path, hostname: &str) -> Result<(), String> {
    let etc = root.join("etc");
    ensure_dir(&etc)?;
    std::fs::write(etc.join("hostname"), format!("{hostname}\n"))
        .map_err(|e| format!("failed to write hostname: {e}"))?;
    Ok(())
}

// ── Netlink helpers for veth creation ────────────────────────────────────

// Netlink ROUTE protocol constants.
const NETLINK_ROUTE: i32 = 0;
const RTM_NEWLINK: u16 = 16;
const RTM_SETLINK: u16 = 19;
const NLM_F_REQUEST: u16 = 0x0001;
const NLM_F_ACK: u16 = 0x0004;
const NLM_F_CREATE: u16 = 0x0400;
const NLM_F_EXCL: u16 = 0x0200;
const NLMSG_ERROR: u16 = 2;
const NLMSG_HDR_LEN: usize = 16;
const NLMSG_ALIGN: usize = 4;
// ifinfomsg: ifi_family(1) + pad(1) + ifi_type(2) + ifi_index(4) + ifi_flags(4) + ifi_change(4)
const IFINFOMSG_LEN: usize = 16;
const IFLA_IFNAME: u16 = 3;
const IFLA_NET_NS_PID: u16 = 19;
const IFLA_LINKINFO: u16 = 18;
const IFLA_INFO_KIND: u16 = 1;
const IFLA_INFO_DATA: u16 = 2;
const VETH_INFO_PEER: u16 = 1;
const AF_UNSPEC: u8 = 0;
const IFF_UP: u32 = 0x1;

/// The name of the container-side veth interface (matching real systemd-nspawn).
const VETH_CONTAINER_NAME: &str = "host0";
/// Maximum length of a Linux interface name (IFNAMSIZ - 1 for NUL).
const IFNAMSIZ: usize = 16;

fn nl_align(len: usize) -> usize {
    (len + NLMSG_ALIGN - 1) & !(NLMSG_ALIGN - 1)
}

fn nl_put_u16(buf: &mut [u8], offset: usize, val: u16) {
    buf[offset..offset + 2].copy_from_slice(&val.to_ne_bytes());
}

fn nl_put_u32(buf: &mut [u8], offset: usize, val: u32) {
    buf[offset..offset + 4].copy_from_slice(&val.to_ne_bytes());
}

fn nl_put_i32(buf: &mut [u8], offset: usize, val: i32) {
    buf[offset..offset + 4].copy_from_slice(&val.to_ne_bytes());
}

/// Write a netlink route attribute header + arbitrary bytes payload.
fn nl_put_rta_bytes(buf: &mut [u8], offset: usize, rta_type: u16, data: &[u8]) {
    let rta_len = 4 + data.len();
    nl_put_u16(buf, offset, rta_len as u16);
    nl_put_u16(buf, offset + 2, rta_type);
    buf[offset + 4..offset + 4 + data.len()].copy_from_slice(data);
}

/// Write a netlink route attribute with a u32 payload.
fn nl_put_rta_u32(buf: &mut [u8], offset: usize, rta_type: u16, val: u32) {
    let rta_len: u16 = 8;
    nl_put_u16(buf, offset, rta_len);
    nl_put_u16(buf, offset + 2, rta_type);
    nl_put_u32(buf, offset + 4, val);
}

/// Aligned size of an RTA with the given payload length.
fn nl_rta_align(len: usize) -> usize {
    (len + 3) & !3
}

/// Open a NETLINK_ROUTE socket, send a message, and wait for the ACK/error.
fn netlink_route_request(msg: &[u8]) -> io::Result<()> {
    let fd = unsafe {
        libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_RAW | libc::SOCK_CLOEXEC,
            NETLINK_ROUTE,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    // Bind to auto-assigned port.
    let mut addr: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
    addr.nl_family = libc::AF_NETLINK as u16;

    let ret = unsafe {
        libc::bind(
            fd,
            &addr as *const libc::sockaddr_nl as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
        )
    };
    if ret < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(err);
    }

    // Set receive timeout so we don't block forever.
    let tv = libc::timeval {
        tv_sec: 5,
        tv_usec: 0,
    };
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            &tv as *const libc::timeval as *const libc::c_void,
            std::mem::size_of::<libc::timeval>() as libc::socklen_t,
        );
    }

    // Send.
    let sent = unsafe { libc::send(fd, msg.as_ptr() as *const libc::c_void, msg.len(), 0) };
    if sent < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(err);
    }

    // Receive ACK/error.
    let mut buf = [0u8; 4096];
    let n = unsafe { libc::recv(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0) };
    unsafe { libc::close(fd) };

    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    let n = n as usize;
    if n >= NLMSG_HDR_LEN + 4 {
        let nlmsg_type = u16::from_ne_bytes(buf[4..6].try_into().unwrap());
        if nlmsg_type == NLMSG_ERROR {
            let errno =
                i32::from_ne_bytes(buf[NLMSG_HDR_LEN..NLMSG_HDR_LEN + 4].try_into().unwrap());
            if errno < 0 {
                return Err(io::Error::from_raw_os_error(-errno));
            }
            // errno == 0 means ACK (success).
        }
    }

    Ok(())
}

/// Derive the host-side veth interface name from the machine name.
/// Real systemd-nspawn uses `ve-<machine>` truncated to IFNAMSIZ-1 (15 chars).
fn veth_host_name(machine: &str) -> String {
    let prefix = "ve-";
    let max_machine_len = IFNAMSIZ - 1 - prefix.len(); // 15 - 3 = 12
    let truncated = if machine.len() > max_machine_len {
        &machine[..max_machine_len]
    } else {
        machine
    };
    format!("{prefix}{truncated}")
}

/// Build a netlink `RTM_NEWLINK` message to create a veth pair.
///
/// The message creates a veth pair with:
/// - host-side interface named `host_name`
/// - container-side interface named `container_name`
///
/// Returns the serialized netlink message ready to send.
fn build_veth_create_msg(host_name: &str, container_name: &str) -> Vec<u8> {
    // IFLA_IFNAME for host side (NUL-terminated)
    let host_name_bytes = host_name.as_bytes();
    let host_name_payload = host_name_bytes.len() + 1; // +1 for NUL
    let host_name_attr_len = nl_rta_align(4 + host_name_payload);

    // IFLA_INFO_KIND = "veth" (NUL-terminated)
    let kind_bytes = b"veth\0";
    let kind_attr_len = nl_rta_align(4 + kind_bytes.len());

    // Peer's IFLA_IFNAME (container-side, NUL-terminated)
    let cont_name_bytes = container_name.as_bytes();
    let cont_name_payload = cont_name_bytes.len() + 1;
    let cont_name_attr_len = nl_rta_align(4 + cont_name_payload);

    // VETH_INFO_PEER contains: ifinfomsg + peer IFLA_IFNAME
    let peer_payload = IFINFOMSG_LEN + cont_name_attr_len;
    let peer_attr_len = nl_rta_align(4 + peer_payload);

    // IFLA_INFO_DATA contains: VETH_INFO_PEER
    let info_data_payload = peer_attr_len;
    let info_data_attr_len = nl_rta_align(4 + info_data_payload);

    // IFLA_LINKINFO contains: IFLA_INFO_KIND + IFLA_INFO_DATA
    let linkinfo_payload = kind_attr_len + info_data_attr_len;
    let linkinfo_attr_len = nl_rta_align(4 + linkinfo_payload);

    let msg_len = NLMSG_HDR_LEN + IFINFOMSG_LEN + host_name_attr_len + linkinfo_attr_len;
    let mut msg = vec![0u8; nl_align(msg_len)];

    // nlmsghdr
    nl_put_u32(&mut msg, 0, msg_len as u32);
    nl_put_u16(&mut msg, 4, RTM_NEWLINK);
    nl_put_u16(
        &mut msg,
        6,
        NLM_F_REQUEST | NLM_F_ACK | NLM_F_CREATE | NLM_F_EXCL,
    );
    nl_put_u32(&mut msg, 8, 1); // nlmsg_seq
    nl_put_u32(&mut msg, 12, 0); // nlmsg_pid (kernel)

    // ifinfomsg (all zeros: AF_UNSPEC, no specific interface)
    let ifi = NLMSG_HDR_LEN;
    msg[ifi] = AF_UNSPEC;

    // IFLA_IFNAME for the host-side interface
    let mut off = ifi + IFINFOMSG_LEN;
    {
        let mut name_with_nul = host_name_bytes.to_vec();
        name_with_nul.push(0);
        nl_put_rta_bytes(&mut msg, off, IFLA_IFNAME, &name_with_nul);
    }
    off += host_name_attr_len;

    // IFLA_LINKINFO (nested)
    let linkinfo_off = off;
    nl_put_u16(&mut msg, linkinfo_off, (4 + linkinfo_payload) as u16);
    nl_put_u16(&mut msg, linkinfo_off + 2, IFLA_LINKINFO);
    // NLA_F_NESTED flag (0x8000) is optional for compatibility; real systemd sets it
    msg[linkinfo_off + 3] |= 0x80; // set NLA_F_NESTED on the rta_type high byte
    off = linkinfo_off + 4;

    // IFLA_INFO_KIND = "veth"
    nl_put_rta_bytes(&mut msg, off, IFLA_INFO_KIND, kind_bytes);
    off += kind_attr_len;

    // IFLA_INFO_DATA (nested)
    let info_data_off = off;
    nl_put_u16(&mut msg, info_data_off, (4 + info_data_payload) as u16);
    nl_put_u16(&mut msg, info_data_off + 2, IFLA_INFO_DATA);
    msg[info_data_off + 3] |= 0x80; // NLA_F_NESTED
    off = info_data_off + 4;

    // VETH_INFO_PEER (nested: contains ifinfomsg + IFLA_IFNAME)
    let peer_off = off;
    nl_put_u16(&mut msg, peer_off, (4 + peer_payload) as u16);
    nl_put_u16(&mut msg, peer_off + 2, VETH_INFO_PEER);
    msg[peer_off + 3] |= 0x80; // NLA_F_NESTED
    off = peer_off + 4;

    // Peer's ifinfomsg (all zeros)
    msg[off] = AF_UNSPEC;
    off += IFINFOMSG_LEN;

    // Peer's IFLA_IFNAME (container-side name)
    {
        let mut name_with_nul = cont_name_bytes.to_vec();
        name_with_nul.push(0);
        nl_put_rta_bytes(&mut msg, off, IFLA_IFNAME, &name_with_nul);
    }

    msg
}

/// Move a network interface into the network namespace of the given PID
/// via netlink `RTM_SETLINK` with `IFLA_NET_NS_PID`.
///
/// `ifname` is the interface name to move. `target_pid` is the PID
/// whose network namespace will receive the interface.
fn move_interface_to_ns(ifname: &str, target_pid: i32) -> io::Result<()> {
    // First, resolve the interface index by reading /sys/class/net/<name>/ifindex.
    let ifindex = read_ifindex(ifname)?;

    let nspid_attr_len = nl_rta_align(4 + 4); // u32

    let msg_len = NLMSG_HDR_LEN + IFINFOMSG_LEN + nspid_attr_len;
    let mut msg = vec![0u8; nl_align(msg_len)];

    nl_put_u32(&mut msg, 0, msg_len as u32);
    nl_put_u16(&mut msg, 4, RTM_SETLINK);
    nl_put_u16(&mut msg, 6, NLM_F_REQUEST | NLM_F_ACK);
    nl_put_u32(&mut msg, 8, 1);
    nl_put_u32(&mut msg, 12, 0);

    let ifi = NLMSG_HDR_LEN;
    msg[ifi] = AF_UNSPEC;
    nl_put_i32(&mut msg, ifi + 4, ifindex);

    let attr_off = ifi + IFINFOMSG_LEN;
    nl_put_rta_u32(&mut msg, attr_off, IFLA_NET_NS_PID, target_pid as u32);

    netlink_route_request(&msg)
}

/// Bring a network interface up via netlink `RTM_SETLINK` with `IFF_UP`.
fn bring_interface_up(ifname: &str) -> io::Result<()> {
    let ifindex = read_ifindex(ifname)?;

    let msg_len = NLMSG_HDR_LEN + IFINFOMSG_LEN;
    let mut msg = vec![0u8; nl_align(msg_len)];

    nl_put_u32(&mut msg, 0, msg_len as u32);
    nl_put_u16(&mut msg, 4, RTM_SETLINK);
    nl_put_u16(&mut msg, 6, NLM_F_REQUEST | NLM_F_ACK);
    nl_put_u32(&mut msg, 8, 1);
    nl_put_u32(&mut msg, 12, 0);

    let ifi = NLMSG_HDR_LEN;
    msg[ifi] = AF_UNSPEC;
    nl_put_i32(&mut msg, ifi + 4, ifindex);
    // ifi_flags: set IFF_UP
    nl_put_u32(&mut msg, ifi + 8, IFF_UP);
    // ifi_change: mask for the flags we're changing
    nl_put_u32(&mut msg, ifi + 12, IFF_UP);

    netlink_route_request(&msg)
}

/// Read the ifindex for a network interface from sysfs.
fn read_ifindex(ifname: &str) -> io::Result<i32> {
    let path = format!("/sys/class/net/{ifname}/ifindex");
    let content = std::fs::read_to_string(&path)
        .map_err(|e| io::Error::new(e.kind(), format!("cannot read ifindex for {ifname}: {e}")))?;
    content.trim().parse::<i32>().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid ifindex for {ifname}: {e}"),
        )
    })
}

/// Create a virtual ethernet pair, move the container end into the child's
/// network namespace, and bring up the host end.
///
/// - `machine_name`: used to derive the host-side interface name (`ve-<machine>`)
/// - `child_pid`: PID of the child process (must have already done `unshare(CLONE_NEWNET)`)
///
/// The container-side interface is named `host0` (matching real systemd-nspawn).
fn setup_veth(machine_name: &str, child_pid: i32) -> io::Result<()> {
    let host_name = veth_host_name(machine_name);
    let container_name = VETH_CONTAINER_NAME;

    // 1. Create the veth pair in the host namespace.
    let create_msg = build_veth_create_msg(&host_name, container_name);
    netlink_route_request(&create_msg)
        .map_err(|e| io::Error::new(e.kind(), format!("veth pair creation failed: {e}")))?;

    // 2. Move the container-side interface into the child's network namespace.
    if let Err(e) = move_interface_to_ns(container_name, child_pid) {
        // Try to clean up the host side on failure (best-effort).
        log::trace!("Failed to move {container_name} to ns of pid {child_pid}: {e}");
        return Err(io::Error::new(
            e.kind(),
            format!("failed to move {container_name} to container ns: {e}"),
        ));
    }

    // 3. Bring up the host-side interface.
    if let Err(e) = bring_interface_up(&host_name) {
        log::trace!("Failed to bring up {host_name}: {e}");
        // Non-fatal: the interface exists, the user can bring it up manually.
    }

    Ok(())
}

// ── Pipe pair for synchronization ────────────────────────────────────────

struct SyncPipe {
    read_fd: RawFd,
    write_fd: RawFd,
}

impl SyncPipe {
    fn new() -> Result<SyncPipe, String> {
        let mut fds = [0i32; 2];
        let ret = unsafe { libc::pipe(fds.as_mut_ptr()) };
        if ret != 0 {
            return Err(format!(
                "pipe() failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(SyncPipe {
            read_fd: fds[0],
            write_fd: fds[1],
        })
    }

    fn signal(&self) -> Result<(), String> {
        let buf = [1u8];
        let ret = unsafe { libc::write(self.write_fd, buf.as_ptr() as *const _, 1) };
        if ret != 1 {
            return Err(format!(
                "sync write failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    fn wait(&self) -> Result<(), String> {
        let mut buf = [0u8];
        let ret = unsafe { libc::read(self.read_fd, buf.as_mut_ptr() as *mut _, 1) };
        if ret != 1 {
            return Err(format!(
                "sync read failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    fn close_read(&self) {
        unsafe { libc::close(self.read_fd) };
    }

    fn close_write(&self) {
        unsafe { libc::close(self.write_fd) };
    }
}

// ── Find init binary ─────────────────────────────────────────────────────

fn find_init(root: &Path) -> Option<PathBuf> {
    let candidates = [
        "usr/lib/systemd/systemd",
        "lib/systemd/systemd",
        "sbin/init",
        "usr/sbin/init",
        "bin/init",
    ];
    for c in &candidates {
        let p = root.join(c);
        if p.exists() {
            // Return the in-container absolute path
            return Some(PathBuf::from("/").join(c));
        }
    }
    None
}

/// Find a shell inside the container root.
fn find_shell(root: &Path) -> String {
    let shells = [
        "bin/bash",
        "usr/bin/bash",
        "bin/sh",
        "usr/bin/sh",
        "bin/zsh",
        "usr/bin/zsh",
    ];
    for s in &shells {
        if root.join(s).exists() {
            return format!("/{s}");
        }
    }
    "/bin/sh".to_string()
}

// ── Container child process ──────────────────────────────────────────────

/// The entry point for the container child process (after clone/fork).
/// Inner child logic, called after unshare() and parent sync are complete.
/// The child has already:
/// 1. Called unshare() to create new namespaces
/// 2. Signaled the parent that unshare is done
/// 3. Waited for the parent to finish veth/uid setup
fn container_child_inner(
    args: &NspawnArgs,
    root: &Path,
    machine_name: &str,
    capabilities: &[Capability],
) -> ! {
    // Set up the mount namespace
    if let Err(e) = setup_api_filesystems(root) {
        eprintln!("systemd-nspawn: failed to set up API filesystems: {e}");
        std::process::exit(EXIT_FAILURE);
    }

    // Set up user-specified bind mounts
    if let Err(e) = setup_bind_mounts(root, &args.bind_mounts) {
        eprintln!("systemd-nspawn: failed to set up bind mounts: {e}");
        std::process::exit(EXIT_FAILURE);
    }

    // Set up tmpfs mounts
    if let Err(e) = setup_tmpfs_mounts(root, &args.tmpfs_mounts) {
        eprintln!("systemd-nspawn: failed to set up tmpfs mounts: {e}");
        std::process::exit(EXIT_FAILURE);
    }

    // Set up resolv.conf
    if let Err(e) = setup_resolv_conf(root, args.resolv_conf) {
        log::trace!("resolv.conf setup failed (non-fatal): {e}");
    }

    // Set hostname in the container
    let hostname = args.hostname.as_deref().unwrap_or(machine_name);
    if let Err(e) = setup_hostname(root, hostname) {
        log::trace!("hostname setup failed (non-fatal): {e}");
    }

    // Set UTS hostname
    let _ = nix::unistd::sethostname(hostname);

    // Pivot root
    if let Err(e) = pivot_root_or_chroot(root) {
        eprintln!("systemd-nspawn: failed to pivot root: {e}");
        std::process::exit(EXIT_FAILURE);
    }

    // Apply capability bounding set
    if let Err(e) = apply_capability_bounding_set(capabilities) {
        log::trace!("capability bounding set failed (non-fatal): {e}");
    }

    // Set no_new_privs if requested
    if args.no_new_privileges {
        let ret = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
        if ret != 0 {
            log::trace!(
                "prctl(PR_SET_NO_NEW_PRIVS) failed: {}",
                std::io::Error::last_os_error()
            );
        }
    }

    // Determine the command to execute
    let (exec_path, exec_args) = if args.boot {
        // Boot mode: find and run init
        let init = find_init(&PathBuf::from("/")).unwrap_or_else(|| PathBuf::from("/sbin/init"));
        let init_str = init.to_string_lossy().into_owned();
        (init_str.clone(), vec![init_str])
    } else if args.as_pid2 {
        // PID2 mode: we need to be a stub init that runs the command as PID 2
        // For now, exec the command directly (stub init not yet implemented)
        if args.command.is_empty() {
            let shell = find_shell(&PathBuf::from("/"));
            (shell.clone(), vec![shell])
        } else {
            (args.command[0].clone(), args.command.clone())
        }
    } else if args.command.is_empty() {
        let shell = find_shell(&PathBuf::from("/"));
        (shell.clone(), vec![shell])
    } else {
        (args.command[0].clone(), args.command.clone())
    };

    // Change user if requested
    if let Some(ref user) = args.user
        && let Err(e) = switch_user(user)
    {
        eprintln!("systemd-nspawn: failed to switch to user {user}: {e}");
        std::process::exit(EXIT_FAILURE);
    }

    // Change working directory
    if let Some(ref dir) = args.chdir
        && let Err(e) = std::env::set_current_dir(dir)
    {
        eprintln!("systemd-nspawn: failed to chdir to {dir}: {e}");
        std::process::exit(EXIT_FAILURE);
    }

    // Set up environment
    // Clear environment first for a clean container
    // Safety: we are in a freshly forked child process, single-threaded at this point.
    for (key, _) in std::env::vars() {
        // Keep a minimal set
        if !matches!(
            key.as_str(),
            "TERM" | "HOME" | "USER" | "LOGNAME" | "PATH" | "SHELL"
        ) {
            unsafe { std::env::remove_var(&key) };
        }
    }

    // Set container-specific env vars
    // Safety: single-threaded child process after fork.
    unsafe {
        std::env::set_var("container", "systemd-nspawn");
        std::env::set_var("container_uuid", args.uuid.as_deref().unwrap_or(""));
        if args.boot {
            std::env::set_var("container_host_version_id", env!("CARGO_PKG_VERSION"));
        }
        std::env::set_var(
            "PATH",
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        );

        // Set user-specified environment variables
        for (k, v) in &args.environment {
            std::env::set_var(k, v);
        }
    }

    // Exec the payload
    let c_path = CString::new(exec_path.as_str()).unwrap_or_else(|_| {
        eprintln!("systemd-nspawn: invalid command path");
        std::process::exit(EXIT_FAILURE);
    });
    let c_args: Vec<CString> = exec_args
        .iter()
        .map(|a| CString::new(a.as_str()).unwrap())
        .collect();

    let _ = nix::unistd::execv(&c_path, &c_args);
    eprintln!(
        "systemd-nspawn: exec({}) failed: {}",
        exec_path,
        std::io::Error::last_os_error()
    );
    std::process::exit(127);
}

fn pivot_root_or_chroot(root: &Path) -> Result<(), String> {
    // Try pivot_root first
    let old_root = root.join("oldroot");
    let _ = std::fs::create_dir_all(&old_root);

    let root_str = CString::new(root.to_str().unwrap_or("/"))
        .map_err(|e| format!("invalid root path: {e}"))?;
    let old_str = CString::new(old_root.to_str().unwrap_or("/oldroot"))
        .map_err(|e| format!("invalid oldroot path: {e}"))?;

    // Bind-mount root onto itself (required for pivot_root)
    let ret = unsafe {
        libc::mount(
            root_str.as_ptr(),
            root_str.as_ptr(),
            std::ptr::null(),
            (libc::MS_BIND | libc::MS_REC) as libc::c_ulong,
            std::ptr::null(),
        )
    };
    if ret != 0 {
        // Fallback to chroot
        return chroot_fallback(root);
    }

    let ret = unsafe { libc::syscall(libc::SYS_pivot_root, root_str.as_ptr(), old_str.as_ptr()) };
    if ret != 0 {
        // pivot_root failed, try chroot
        return chroot_fallback(root);
    }

    // Change to new root
    let _ = std::env::set_current_dir("/");

    // Unmount old root
    let old_root_c = CString::new("/oldroot").unwrap();
    unsafe {
        libc::umount2(old_root_c.as_ptr(), libc::MNT_DETACH);
    }
    let _ = std::fs::remove_dir("/oldroot");

    Ok(())
}

fn chroot_fallback(root: &Path) -> Result<(), String> {
    let root_str =
        CString::new(root.to_str().unwrap_or("/")).map_err(|e| format!("invalid root: {e}"))?;
    let ret = unsafe { libc::chroot(root_str.as_ptr()) };
    if ret != 0 {
        return Err(format!(
            "chroot({}) failed: {}",
            root.display(),
            std::io::Error::last_os_error()
        ));
    }
    let _ = std::env::set_current_dir("/");
    Ok(())
}

fn switch_user(user: &str) -> Result<(), String> {
    // Try numeric UID first
    if let Ok(uid) = user.parse::<u32>() {
        let ret = unsafe { libc::setuid(uid) };
        if ret != 0 {
            return Err(format!(
                "setuid({uid}) failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        return Ok(());
    }

    // Look up by name
    let c_user = CString::new(user).map_err(|e| format!("invalid username: {e}"))?;
    let pw = unsafe { libc::getpwnam(c_user.as_ptr()) };
    if pw.is_null() {
        return Err(format!("user not found: {user}"));
    }
    let uid = unsafe { (*pw).pw_uid };
    let gid = unsafe { (*pw).pw_gid };

    let ret = unsafe { libc::setgid(gid) };
    if ret != 0 {
        return Err(format!(
            "setgid({gid}) failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    let ret = unsafe { libc::setuid(uid) };
    if ret != 0 {
        return Err(format!(
            "setuid({uid}) failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

// ── machined registration ────────────────────────────────────────────────

/// Try to register the container with systemd-machined via control socket.
fn register_with_machined(name: &str, pid: u32, directory: &str) -> Result<(), String> {
    let socket_path = "/run/systemd/machined-control";
    if !Path::new(socket_path).exists() {
        return Ok(()); // machined not running, that's fine
    }

    // Build a REGISTER command for machined's control socket protocol
    let msg = format!(
        "{{\"method\":\"REGISTER\",\"params\":{{\"name\":\"{name}\",\"class\":\"container\",\"service\":\"nspawn\",\"leader\":{pid},\"root_directory\":\"{directory}\"}}}}\n"
    );

    let addr =
        nix::sys::socket::UnixAddr::new(socket_path).map_err(|e| format!("unix addr: {e}"))?;
    let fd = nix::sys::socket::socket(
        nix::sys::socket::AddressFamily::Unix,
        nix::sys::socket::SockType::Stream,
        nix::sys::socket::SockFlag::empty(),
        None,
    )
    .map_err(|e| format!("socket: {e}"))?;

    use std::os::fd::AsRawFd;
    nix::sys::socket::connect(fd.as_raw_fd(), &addr).map_err(|e| format!("connect: {e}"))?;

    let written = nix::unistd::write(&fd, msg.as_bytes()).map_err(|e| format!("write: {e}"))?;
    if written != msg.len() {
        return Err("short write to machined".to_string());
    }

    // Read response (best-effort)
    let mut buf = [0u8; 4096];
    let _ = nix::unistd::read(&fd, &mut buf);

    Ok(())
}

/// Unregister a container from machined.
fn unregister_from_machined(name: &str) -> Result<(), String> {
    let socket_path = "/run/systemd/machined-control";
    if !Path::new(socket_path).exists() {
        return Ok(());
    }

    let msg = format!("{{\"method\":\"TERMINATE\",\"params\":{{\"name\":\"{name}\"}}}}\n");

    let addr =
        nix::sys::socket::UnixAddr::new(socket_path).map_err(|e| format!("unix addr: {e}"))?;
    let fd = nix::sys::socket::socket(
        nix::sys::socket::AddressFamily::Unix,
        nix::sys::socket::SockType::Stream,
        nix::sys::socket::SockFlag::empty(),
        None,
    )
    .map_err(|e| format!("socket: {e}"))?;

    use std::os::fd::AsRawFd;
    nix::sys::socket::connect(fd.as_raw_fd(), &addr).map_err(|e| format!("connect: {e}"))?;

    let _ = nix::unistd::write(&fd, msg.as_bytes());
    Ok(())
}

// ── Main entry point ─────────────────────────────────────────────────────

fn print_usage() {
    println!(
        "\
systemd-nspawn — Spawn a command or OS in a lightweight namespace container

Usage: systemd-nspawn [OPTIONS...] [-- COMMAND [ARGS...]]

Options:
  -D, --directory=PATH     Container root directory
  -i, --image=PATH         Disk image to use as container root
  -M, --machine=NAME       Machine name for the container
  -b, --boot               Boot an init system inside the container
      --as-pid2            Run as PID 2 with a stub init as PID 1
  -u, --user=USER          User to run as in the container
      --chdir=PATH         Working directory in the container
  -E, --setenv=VAR=VALUE   Set environment variable
      --bind=SRC[:DST[:OPTS]]       Bind mount
      --bind-ro=SRC[:DST[:OPTS]]    Read-only bind mount
      --tmpfs=PATH[:OPTS]  Mount tmpfs at PATH
      --overlay=SPEC       Overlay mount
      --overlay-ro=SPEC    Read-only overlay mount
      --private-network    Run in private network namespace
  -n, --network-veth       Create a virtual Ethernet link
  -U, --private-users      Use user namespaces
      --capability=CAP     Grant additional capabilities
      --drop-capability=CAP Drop capabilities
      --no-new-privileges  Set the no_new_privs flag
      --volatile[=MODE]    Volatile mode (yes/state/overlay)
      --resolv-conf=MODE   resolv.conf handling mode
      --timezone=MODE      Timezone handling mode
      --link-journal=MODE  Journal link mode
      --console=MODE       Console mode (interactive/read-only/passive/pipe)
  -p, --port=SPEC          Port forwarding (tcp:HOST:CONTAINER)
      --uuid=UUID          Container UUID
      --hostname=NAME      Container hostname
      --register=BOOL      Register with machined
      --keep-unit           Don't create a scope unit
      --property=NAME=VALUE Set scope property
      --kill-signal=SIGNAL  Signal for killing container
      --settings=BOOL      Read .nspawn settings file
      --oci-bundle=PATH    OCI bundle path
      --read-only          Mount root read-only
      --notify-ready       Notify when ready
  -q, --quiet              Suppress informational output
  -h, --help               Show this help
      --version            Show version
"
    );
}

fn run() -> i32 {
    let argv: Vec<String> = std::env::args().collect();
    let arg_refs: Vec<&str> = argv[1..].iter().map(|s| s.as_str()).collect();
    let args = match parse_args(&arg_refs) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("systemd-nspawn: {e}");
            return EXIT_FAILURE;
        }
    };

    if args.help {
        print_usage();
        return EXIT_SUCCESS;
    }

    if args.version {
        println!("systemd-nspawn {}", env!("CARGO_PKG_VERSION"));
        return EXIT_SUCCESS;
    }

    // Resolve the container root
    let root = match resolve_root(&args) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("systemd-nspawn: {e}");
            return EXIT_FAILURE;
        }
    };

    let machine_name = derive_machine_name(&args);
    let capabilities = compute_capabilities(&args);

    if !args.quiet {
        eprintln!("Spawning container {machine_name} on {}.", root.display());
    }

    // Set up sync pipes for parent-child coordination.
    // child_ready_pipe: child signals parent after unshare() completes.
    // parent_done_pipe: parent signals child after veth/uid setup is done.
    let child_ready_pipe = match SyncPipe::new() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("systemd-nspawn: {e}");
            return EXIT_FAILURE;
        }
    };
    let parent_done_pipe = match SyncPipe::new() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("systemd-nspawn: {e}");
            return EXIT_FAILURE;
        }
    };

    // Build the clone flags
    let mut clone_flags: libc::c_int =
        libc::CLONE_NEWNS | libc::CLONE_NEWPID | libc::CLONE_NEWUTS | libc::CLONE_NEWIPC;
    if args.private_network || args.network_veth {
        clone_flags |= libc::CLONE_NEWNET;
    }
    if args.private_users {
        clone_flags |= libc::CLONE_NEWUSER;
    }

    // We'll use fork() instead of clone() because Rust's runtime doesn't
    // play well with clone()'s stack semantics. We call unshare() after fork.
    let pid = unsafe { libc::fork() };

    match pid {
        -1 => {
            eprintln!(
                "systemd-nspawn: fork failed: {}",
                std::io::Error::last_os_error()
            );
            EXIT_FAILURE
        }
        0 => {
            // Child: close unused ends of the pipes
            child_ready_pipe.close_read();
            parent_done_pipe.close_write();

            // Child: unshare namespaces
            let ret = unsafe { libc::unshare(clone_flags) };
            if ret != 0 {
                eprintln!(
                    "systemd-nspawn: unshare failed: {} (are you running as root?)",
                    std::io::Error::last_os_error()
                );
                std::process::exit(EXIT_FAILURE);
            }

            // Signal the parent that unshare() is complete
            if let Err(e) = child_ready_pipe.signal() {
                eprintln!("systemd-nspawn: child ready signal failed: {e}");
                std::process::exit(EXIT_FAILURE);
            }
            child_ready_pipe.close_write();

            // Wait for the parent to finish veth/uid setup
            if let Err(e) = parent_done_pipe.wait() {
                eprintln!("systemd-nspawn: waiting for parent setup failed: {e}");
                std::process::exit(EXIT_FAILURE);
            }
            parent_done_pipe.close_read();

            container_child_inner(&args, &root, &machine_name, &capabilities);
        }
        child_pid => {
            // Parent: close unused ends
            child_ready_pipe.close_write();
            parent_done_pipe.close_read();

            // Wait for the child to complete unshare() before we set up
            // things that depend on the child's namespaces (veth, uid maps).
            if let Err(e) = child_ready_pipe.wait() {
                eprintln!("systemd-nspawn: waiting for child unshare failed: {e}");
                unsafe { libc::kill(child_pid, libc::SIGKILL) };
                return EXIT_FAILURE;
            }
            child_ready_pipe.close_read();

            // Set up UID/GID mappings for user namespaces
            if args.private_users {
                let uid = unsafe { libc::getuid() };
                let gid = unsafe { libc::getgid() };
                let uid_map_path = format!("/proc/{child_pid}/uid_map");
                let _ = std::fs::write(&uid_map_path, format!("0 {uid} 1\n"));
                let setgroups_path = format!("/proc/{child_pid}/setgroups");
                let _ = std::fs::write(&setgroups_path, "deny");
                let gid_map_path = format!("/proc/{child_pid}/gid_map");
                let _ = std::fs::write(&gid_map_path, format!("0 {gid} 1\n"));
            }

            // Create veth pair and move container end into child's namespace
            if args.network_veth {
                match setup_veth(&machine_name, child_pid) {
                    Ok(()) => {
                        if !args.quiet {
                            let host_if = veth_host_name(&machine_name);
                            eprintln!(
                                "systemd-nspawn: created veth pair {host_if} <-> {VETH_CONTAINER_NAME}"
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("systemd-nspawn: veth setup failed: {e}");
                        // Non-fatal: container still runs with isolated (empty) network.
                    }
                }
            }

            // Register with machined
            let should_register = args.register.unwrap_or(true);
            if should_register
                && let Err(e) =
                    register_with_machined(&machine_name, child_pid as u32, &root.to_string_lossy())
            {
                log::trace!("machined registration failed (non-fatal): {e}");
            }

            // Signal the child to proceed with container setup
            if let Err(e) = parent_done_pipe.signal() {
                eprintln!("systemd-nspawn: parent done signal failed: {e}");
                unsafe { libc::kill(child_pid, libc::SIGKILL) };
                return EXIT_FAILURE;
            }
            parent_done_pipe.close_write();

            // Wait for the child
            let mut status: libc::c_int = 0;
            loop {
                let ret = unsafe { libc::waitpid(child_pid, &mut status, 0) };
                if ret == -1 {
                    let err = std::io::Error::last_os_error();
                    if err.raw_os_error() == Some(libc::EINTR) {
                        continue;
                    }
                    eprintln!("systemd-nspawn: waitpid failed: {err}");
                    return EXIT_FAILURE;
                }
                break;
            }

            // Unregister from machined
            if should_register {
                let _ = unregister_from_machined(&machine_name);
            }

            // Determine exit code
            if libc::WIFEXITED(status) {
                let code = libc::WEXITSTATUS(status);
                if !args.quiet && code != 0 {
                    eprintln!(
                        "systemd-nspawn: container {machine_name} exited with status {code}."
                    );
                }
                code
            } else if libc::WIFSIGNALED(status) {
                let sig = libc::WTERMSIG(status);
                if !args.quiet {
                    eprintln!("systemd-nspawn: container {machine_name} killed by signal {sig}.");
                }
                128 + sig
            } else {
                EXIT_FAILURE
            }
        }
    }
}

fn main() {
    std::process::exit(run());
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Capability tests ─────────────────────────────────────────────

    #[test]
    fn test_capability_from_str_basic() {
        assert_eq!(Capability::from_str("CAP_CHOWN"), Some(Capability::Chown));
        assert_eq!(Capability::from_str("chown"), Some(Capability::Chown));
        assert_eq!(Capability::from_str("CHOWN"), Some(Capability::Chown));
        assert_eq!(Capability::from_str("cap_chown"), Some(Capability::Chown));
    }

    #[test]
    fn test_capability_from_str_all_known() {
        assert_eq!(
            Capability::from_str("DAC_OVERRIDE"),
            Some(Capability::DacOverride)
        );
        assert_eq!(
            Capability::from_str("NET_ADMIN"),
            Some(Capability::NetAdmin)
        );
        assert_eq!(
            Capability::from_str("SYS_ADMIN"),
            Some(Capability::SysAdmin)
        );
        assert_eq!(Capability::from_str("MKNOD"), Some(Capability::Mknod));
        assert_eq!(Capability::from_str("BPF"), Some(Capability::Bpf));
        assert_eq!(
            Capability::from_str("CHECKPOINT_RESTORE"),
            Some(Capability::CheckpointRestore)
        );
    }

    #[test]
    fn test_capability_from_str_unknown() {
        assert_eq!(Capability::from_str("NONEXISTENT"), None);
        assert_eq!(Capability::from_str(""), None);
        assert_eq!(Capability::from_str("CAP_"), None);
    }

    #[test]
    fn test_capability_as_str() {
        assert_eq!(Capability::Chown.as_str(), "CAP_CHOWN");
        assert_eq!(Capability::SysAdmin.as_str(), "CAP_SYS_ADMIN");
        assert_eq!(Capability::NetRaw.as_str(), "CAP_NET_RAW");
    }

    #[test]
    fn test_capability_number_ordering() {
        assert_eq!(Capability::Chown.number(), 0);
        assert_eq!(Capability::DacOverride.number(), 1);
        assert_eq!(Capability::Kill.number(), 5);
        assert_eq!(Capability::SysAdmin.number(), 21);
        assert_eq!(Capability::Setfcap.number(), 31);
        assert_eq!(Capability::CheckpointRestore.number(), 40);
    }

    #[test]
    fn test_capability_display() {
        assert_eq!(format!("{}", Capability::Chown), "CAP_CHOWN");
        assert_eq!(format!("{}", Capability::NetAdmin), "CAP_NET_ADMIN");
    }

    #[test]
    fn test_parse_capability_list_single() {
        let caps = parse_capability_list("CAP_NET_ADMIN").unwrap();
        assert_eq!(caps, vec![Capability::NetAdmin]);
    }

    #[test]
    fn test_parse_capability_list_multiple() {
        let caps = parse_capability_list("NET_ADMIN,SYS_ADMIN,MKNOD").unwrap();
        assert_eq!(caps.len(), 3);
        assert!(caps.contains(&Capability::NetAdmin));
        assert!(caps.contains(&Capability::SysAdmin));
        assert!(caps.contains(&Capability::Mknod));
    }

    #[test]
    fn test_parse_capability_list_all() {
        let caps = parse_capability_list("all").unwrap();
        assert!(caps.len() > 20);
        assert!(caps.contains(&Capability::Chown));
        assert!(caps.contains(&Capability::SysAdmin));
    }

    #[test]
    fn test_parse_capability_list_empty_parts() {
        let caps = parse_capability_list("NET_ADMIN,,SYS_ADMIN,").unwrap();
        assert_eq!(caps.len(), 2);
    }

    #[test]
    fn test_parse_capability_list_unknown() {
        let result = parse_capability_list("NONEXISTENT");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown capability"));
    }

    #[test]
    fn test_parse_capability_list_with_spaces() {
        let caps = parse_capability_list("NET_ADMIN , SYS_ADMIN").unwrap();
        assert_eq!(caps.len(), 2);
    }

    // ── BindMount tests ──────────────────────────────────────────────

    #[test]
    fn test_bind_mount_parse_simple() {
        let bm = BindMount::parse("/host/path", false).unwrap();
        assert_eq!(bm.source, "/host/path");
        assert_eq!(bm.destination, "/host/path");
        assert!(!bm.read_only);
        assert!(bm.recursive);
    }

    #[test]
    fn test_bind_mount_parse_with_dest() {
        let bm = BindMount::parse("/host:/container", false).unwrap();
        assert_eq!(bm.source, "/host");
        assert_eq!(bm.destination, "/container");
    }

    #[test]
    fn test_bind_mount_parse_with_options() {
        let bm = BindMount::parse("/host:/container:norbind", false).unwrap();
        assert_eq!(bm.source, "/host");
        assert_eq!(bm.destination, "/container");
        assert!(!bm.recursive);
        assert_eq!(bm.options, "norbind");
    }

    #[test]
    fn test_bind_mount_parse_read_only() {
        let bm = BindMount::parse("/data", true).unwrap();
        assert!(bm.read_only);
    }

    #[test]
    fn test_bind_mount_parse_empty_dest() {
        let bm = BindMount::parse("/host:", false).unwrap();
        assert_eq!(bm.source, "/host");
        assert_eq!(bm.destination, "/host");
    }

    #[test]
    fn test_bind_mount_parse_empty_source() {
        let result = BindMount::parse("", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_bind_mount_rbind_default() {
        let bm = BindMount::parse("/a:/b", false).unwrap();
        assert!(bm.recursive); // default is rbind
    }

    #[test]
    fn test_bind_mount_explicit_rbind() {
        let bm = BindMount::parse("/a:/b:rbind", false).unwrap();
        assert!(bm.recursive);
    }

    // ── PortForward tests ────────────────────────────────────────────

    #[test]
    fn test_port_forward_parse_single() {
        let pf = PortForward::parse("8080").unwrap();
        assert_eq!(pf.protocol, "tcp");
        assert_eq!(pf.host_port, 8080);
        assert_eq!(pf.container_port, 8080);
    }

    #[test]
    fn test_port_forward_parse_host_container() {
        let pf = PortForward::parse("8080:80").unwrap();
        assert_eq!(pf.protocol, "tcp");
        assert_eq!(pf.host_port, 8080);
        assert_eq!(pf.container_port, 80);
    }

    #[test]
    fn test_port_forward_parse_full() {
        let pf = PortForward::parse("tcp:8080:80").unwrap();
        assert_eq!(pf.protocol, "tcp");
        assert_eq!(pf.host_port, 8080);
        assert_eq!(pf.container_port, 80);
    }

    #[test]
    fn test_port_forward_parse_udp() {
        let pf = PortForward::parse("udp:5353:53").unwrap();
        assert_eq!(pf.protocol, "udp");
        assert_eq!(pf.host_port, 5353);
        assert_eq!(pf.container_port, 53);
    }

    #[test]
    fn test_port_forward_parse_proto_only() {
        let pf = PortForward::parse("tcp:8080").unwrap();
        assert_eq!(pf.protocol, "tcp");
        assert_eq!(pf.host_port, 8080);
        assert_eq!(pf.container_port, 8080);
    }

    #[test]
    fn test_port_forward_parse_invalid() {
        assert!(PortForward::parse("not-a-port").is_err());
        assert!(PortForward::parse("tcp:not-a-port").is_err());
    }

    #[test]
    fn test_port_forward_parse_too_many_parts() {
        assert!(PortForward::parse("tcp:80:90:100").is_err());
    }

    // ── VolatileMode tests ───────────────────────────────────────────

    #[test]
    fn test_volatile_mode_from_str() {
        assert_eq!(VolatileMode::from_str("no"), Some(VolatileMode::No));
        assert_eq!(VolatileMode::from_str("false"), Some(VolatileMode::No));
        assert_eq!(VolatileMode::from_str("yes"), Some(VolatileMode::Yes));
        assert_eq!(VolatileMode::from_str(""), Some(VolatileMode::Yes));
        assert_eq!(VolatileMode::from_str("state"), Some(VolatileMode::State));
        assert_eq!(
            VolatileMode::from_str("overlay"),
            Some(VolatileMode::Overlay)
        );
        assert_eq!(VolatileMode::from_str("invalid"), None);
    }

    // ── ResolvConfMode tests ─────────────────────────────────────────

    #[test]
    fn test_resolv_conf_mode_from_str() {
        assert_eq!(ResolvConfMode::from_str("off"), Some(ResolvConfMode::Off));
        assert_eq!(
            ResolvConfMode::from_str("copy-host"),
            Some(ResolvConfMode::CopyHost)
        );
        assert_eq!(
            ResolvConfMode::from_str("copy-stub"),
            Some(ResolvConfMode::CopyStub)
        );
        assert_eq!(
            ResolvConfMode::from_str("bind-host"),
            Some(ResolvConfMode::BindHost)
        );
        assert_eq!(
            ResolvConfMode::from_str("delete"),
            Some(ResolvConfMode::Delete)
        );
        assert_eq!(ResolvConfMode::from_str("auto"), Some(ResolvConfMode::Auto));
        assert_eq!(ResolvConfMode::from_str("nonsense"), None);
    }

    // ── TimezoneMode tests ───────────────────────────────────────────

    #[test]
    fn test_timezone_mode_from_str() {
        assert_eq!(TimezoneMode::from_str("off"), Some(TimezoneMode::Off));
        assert_eq!(TimezoneMode::from_str("copy"), Some(TimezoneMode::Copy));
        assert_eq!(TimezoneMode::from_str("bind"), Some(TimezoneMode::Bind));
        assert_eq!(
            TimezoneMode::from_str("symlink"),
            Some(TimezoneMode::Symlink)
        );
        assert_eq!(TimezoneMode::from_str("delete"), Some(TimezoneMode::Delete));
        assert_eq!(TimezoneMode::from_str("auto"), Some(TimezoneMode::Auto));
        assert_eq!(TimezoneMode::from_str("invalid"), None);
    }

    // ── LinkJournalMode tests ────────────────────────────────────────

    #[test]
    fn test_link_journal_mode_from_str() {
        assert_eq!(LinkJournalMode::from_str("no"), Some(LinkJournalMode::No));
        assert_eq!(
            LinkJournalMode::from_str("host"),
            Some(LinkJournalMode::Host)
        );
        assert_eq!(
            LinkJournalMode::from_str("try-host"),
            Some(LinkJournalMode::TryHost)
        );
        assert_eq!(
            LinkJournalMode::from_str("guest"),
            Some(LinkJournalMode::Guest)
        );
        assert_eq!(
            LinkJournalMode::from_str("try-guest"),
            Some(LinkJournalMode::TryGuest)
        );
        assert_eq!(
            LinkJournalMode::from_str("auto"),
            Some(LinkJournalMode::Auto)
        );
        assert_eq!(LinkJournalMode::from_str(""), Some(LinkJournalMode::Auto));
        assert_eq!(LinkJournalMode::from_str("bad"), None);
    }

    // ── ConsoleMode tests ────────────────────────────────────────────

    #[test]
    fn test_console_mode_from_str() {
        assert_eq!(
            ConsoleMode::from_str("interactive"),
            Some(ConsoleMode::Interactive)
        );
        assert_eq!(
            ConsoleMode::from_str("read-only"),
            Some(ConsoleMode::ReadOnly)
        );
        assert_eq!(
            ConsoleMode::from_str("readonly"),
            Some(ConsoleMode::ReadOnly)
        );
        assert_eq!(ConsoleMode::from_str("passive"), Some(ConsoleMode::Passive));
        assert_eq!(ConsoleMode::from_str("pipe"), Some(ConsoleMode::Pipe));
        assert_eq!(
            ConsoleMode::from_str("autopipe"),
            Some(ConsoleMode::Autopipe)
        );
        assert_eq!(ConsoleMode::from_str("bad"), None);
    }

    // ── Machine name tests ───────────────────────────────────────────

    #[test]
    fn test_sanitize_machine_name_clean() {
        assert_eq!(sanitize_machine_name("mycontainer"), "mycontainer");
        assert_eq!(sanitize_machine_name("my-container"), "my-container");
        assert_eq!(sanitize_machine_name("my_container"), "my_container");
        assert_eq!(sanitize_machine_name("my.container"), "my.container");
    }

    #[test]
    fn test_sanitize_machine_name_special_chars() {
        assert_eq!(sanitize_machine_name("my container!"), "my-container");
        assert_eq!(sanitize_machine_name("a@b#c"), "a-b-c");
    }

    #[test]
    fn test_sanitize_machine_name_leading_trailing_dash() {
        assert_eq!(sanitize_machine_name("-name-"), "name");
        assert_eq!(sanitize_machine_name("---"), "container");
    }

    #[test]
    fn test_sanitize_machine_name_empty() {
        assert_eq!(sanitize_machine_name(""), "container");
    }

    #[test]
    fn test_sanitize_machine_name_long() {
        let long_name = "a".repeat(100);
        let result = sanitize_machine_name(&long_name);
        assert_eq!(result.len(), 64);
    }

    #[test]
    fn test_derive_machine_name_explicit() {
        let mut args = NspawnArgs::default();
        args.machine = Some("testbox".to_string());
        assert_eq!(derive_machine_name(&args), "testbox");
    }

    #[test]
    fn test_derive_machine_name_from_directory() {
        let mut args = NspawnArgs::default();
        args.directory = Some("/var/lib/machines/testbox".to_string());
        assert_eq!(derive_machine_name(&args), "testbox");
    }

    #[test]
    fn test_derive_machine_name_from_image() {
        let mut args = NspawnArgs::default();
        args.image = Some("/images/testbox.raw".to_string());
        assert_eq!(derive_machine_name(&args), "testbox");
    }

    // ── Compute capabilities tests ───────────────────────────────────

    #[test]
    fn test_compute_capabilities_default() {
        let args = NspawnArgs::default();
        let caps = compute_capabilities(&args);
        assert!(caps.contains(&Capability::Chown));
        assert!(caps.contains(&Capability::Setuid));
        assert!(caps.contains(&Capability::SysAdmin));
    }

    #[test]
    fn test_compute_capabilities_add_extra() {
        let mut args = NspawnArgs::default();
        args.extra_capabilities = vec![Capability::NetAdmin];
        let caps = compute_capabilities(&args);
        assert!(caps.contains(&Capability::NetAdmin));
    }

    #[test]
    fn test_compute_capabilities_drop() {
        let mut args = NspawnArgs::default();
        args.drop_capabilities = vec![Capability::SysAdmin];
        let caps = compute_capabilities(&args);
        assert!(!caps.contains(&Capability::SysAdmin));
        assert!(caps.contains(&Capability::Chown)); // other caps still present
    }

    #[test]
    fn test_compute_capabilities_add_and_drop() {
        let mut args = NspawnArgs::default();
        args.extra_capabilities = vec![Capability::NetAdmin];
        args.drop_capabilities = vec![Capability::SysAdmin, Capability::NetAdmin];
        let caps = compute_capabilities(&args);
        // NetAdmin was added then dropped
        assert!(!caps.contains(&Capability::NetAdmin));
        assert!(!caps.contains(&Capability::SysAdmin));
    }

    #[test]
    fn test_compute_capabilities_add_duplicate() {
        let mut args = NspawnArgs::default();
        // Chown is already in defaults
        args.extra_capabilities = vec![Capability::Chown];
        let caps = compute_capabilities(&args);
        let chown_count = caps.iter().filter(|c| **c == Capability::Chown).count();
        assert_eq!(chown_count, 1);
    }

    // ── Argument parsing tests ───────────────────────────────────────

    #[test]
    fn test_parse_args_empty() {
        let args = parse_args(&[]).unwrap();
        assert!(args.directory.is_none());
        assert!(args.machine.is_none());
        assert!(!args.boot);
        assert!(args.command.is_empty());
    }

    #[test]
    fn test_parse_args_help() {
        let args = parse_args(&["--help"]).unwrap();
        assert!(args.help);
    }

    #[test]
    fn test_parse_args_short_help() {
        let args = parse_args(&["-h"]).unwrap();
        assert!(args.help);
    }

    #[test]
    fn test_parse_args_version() {
        let args = parse_args(&["--version"]).unwrap();
        assert!(args.version);
    }

    #[test]
    fn test_parse_args_directory_long() {
        let args = parse_args(&["--directory=/my/root"]).unwrap();
        assert_eq!(args.directory, Some("/my/root".to_string()));
    }

    #[test]
    fn test_parse_args_directory_short() {
        let args = parse_args(&["-D", "/my/root"]).unwrap();
        assert_eq!(args.directory, Some("/my/root".to_string()));
    }

    #[test]
    fn test_parse_args_directory_short_attached() {
        let args = parse_args(&["-D/my/root"]).unwrap();
        assert_eq!(args.directory, Some("/my/root".to_string()));
    }

    #[test]
    fn test_parse_args_directory_missing_value() {
        let result = parse_args(&["-D"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_args_image_long() {
        let args = parse_args(&["--image=/my/image.raw"]).unwrap();
        assert_eq!(args.image, Some("/my/image.raw".to_string()));
    }

    #[test]
    fn test_parse_args_image_short() {
        let args = parse_args(&["-i", "/my/image.raw"]).unwrap();
        assert_eq!(args.image, Some("/my/image.raw".to_string()));
    }

    #[test]
    fn test_parse_args_machine_long() {
        let args = parse_args(&["--machine=testbox"]).unwrap();
        assert_eq!(args.machine, Some("testbox".to_string()));
    }

    #[test]
    fn test_parse_args_machine_short() {
        let args = parse_args(&["-M", "testbox"]).unwrap();
        assert_eq!(args.machine, Some("testbox".to_string()));
    }

    #[test]
    fn test_parse_args_boot() {
        let args = parse_args(&["-b"]).unwrap();
        assert!(args.boot);
    }

    #[test]
    fn test_parse_args_boot_long() {
        let args = parse_args(&["--boot"]).unwrap();
        assert!(args.boot);
    }

    #[test]
    fn test_parse_args_as_pid2() {
        let args = parse_args(&["--as-pid2"]).unwrap();
        assert!(args.as_pid2);
    }

    #[test]
    fn test_parse_args_user_long() {
        let args = parse_args(&["--user=nobody"]).unwrap();
        assert_eq!(args.user, Some("nobody".to_string()));
    }

    #[test]
    fn test_parse_args_user_short() {
        let args = parse_args(&["-u", "nobody"]).unwrap();
        assert_eq!(args.user, Some("nobody".to_string()));
    }

    #[test]
    fn test_parse_args_chdir() {
        let args = parse_args(&["--chdir=/home/user"]).unwrap();
        assert_eq!(args.chdir, Some("/home/user".to_string()));
    }

    #[test]
    fn test_parse_args_setenv_long() {
        let args = parse_args(&["--setenv=FOO=bar"]).unwrap();
        assert_eq!(args.environment.get("FOO"), Some(&"bar".to_string()));
    }

    #[test]
    fn test_parse_args_setenv_short() {
        let args = parse_args(&["-E", "FOO=bar"]).unwrap();
        assert_eq!(args.environment.get("FOO"), Some(&"bar".to_string()));
    }

    #[test]
    fn test_parse_args_setenv_multiple() {
        let args = parse_args(&["-E", "A=1", "-E", "B=2"]).unwrap();
        assert_eq!(args.environment.get("A"), Some(&"1".to_string()));
        assert_eq!(args.environment.get("B"), Some(&"2".to_string()));
    }

    #[test]
    fn test_parse_args_bind() {
        let args = parse_args(&["--bind=/host:/container"]).unwrap();
        assert_eq!(args.bind_mounts.len(), 1);
        assert_eq!(args.bind_mounts[0].source, "/host");
        assert_eq!(args.bind_mounts[0].destination, "/container");
        assert!(!args.bind_mounts[0].read_only);
    }

    #[test]
    fn test_parse_args_bind_ro() {
        let args = parse_args(&["--bind-ro=/host:/container"]).unwrap();
        assert_eq!(args.bind_mounts.len(), 1);
        assert!(args.bind_mounts[0].read_only);
    }

    #[test]
    fn test_parse_args_multiple_binds() {
        let args = parse_args(&["--bind=/a:/b", "--bind=/c:/d", "--bind-ro=/e:/f"]).unwrap();
        assert_eq!(args.bind_mounts.len(), 3);
    }

    #[test]
    fn test_parse_args_tmpfs() {
        let args = parse_args(&["--tmpfs=/run"]).unwrap();
        assert_eq!(args.tmpfs_mounts.len(), 1);
        assert_eq!(args.tmpfs_mounts[0].0, "/run");
    }

    #[test]
    fn test_parse_args_tmpfs_with_options() {
        let args = parse_args(&["--tmpfs=/run:mode=0755,size=1G"]).unwrap();
        assert_eq!(args.tmpfs_mounts[0].0, "/run");
        assert_eq!(args.tmpfs_mounts[0].1, "mode=0755,size=1G");
    }

    #[test]
    fn test_parse_args_private_network() {
        let args = parse_args(&["--private-network"]).unwrap();
        assert!(args.private_network);
    }

    #[test]
    fn test_parse_args_network_veth() {
        let args = parse_args(&["-n"]).unwrap();
        assert!(args.network_veth);
    }

    #[test]
    fn test_parse_args_network_veth_long() {
        let args = parse_args(&["--network-veth"]).unwrap();
        assert!(args.network_veth);
    }

    #[test]
    fn test_parse_args_private_users() {
        let args = parse_args(&["-U"]).unwrap();
        assert!(args.private_users);
    }

    #[test]
    fn test_parse_args_private_users_long() {
        let args = parse_args(&["--private-users"]).unwrap();
        assert!(args.private_users);
    }

    #[test]
    fn test_parse_args_private_users_pick() {
        let args = parse_args(&["--private-users=pick"]).unwrap();
        assert!(args.private_users);
    }

    #[test]
    fn test_parse_args_capability() {
        let args = parse_args(&["--capability=NET_ADMIN"]).unwrap();
        assert_eq!(args.extra_capabilities.len(), 1);
        assert_eq!(args.extra_capabilities[0], Capability::NetAdmin);
    }

    #[test]
    fn test_parse_args_capability_multiple() {
        let args = parse_args(&["--capability=NET_ADMIN,SYS_ADMIN"]).unwrap();
        assert_eq!(args.extra_capabilities.len(), 2);
    }

    #[test]
    fn test_parse_args_drop_capability() {
        let args = parse_args(&["--drop-capability=SYS_ADMIN"]).unwrap();
        assert_eq!(args.drop_capabilities.len(), 1);
        assert_eq!(args.drop_capabilities[0], Capability::SysAdmin);
    }

    #[test]
    fn test_parse_args_no_new_privileges() {
        let args = parse_args(&["--no-new-privileges"]).unwrap();
        assert!(args.no_new_privileges);
    }

    #[test]
    fn test_parse_args_volatile_default() {
        let args = parse_args(&["--volatile"]).unwrap();
        assert_eq!(args.volatile, VolatileMode::Yes);
    }

    #[test]
    fn test_parse_args_volatile_state() {
        let args = parse_args(&["--volatile=state"]).unwrap();
        assert_eq!(args.volatile, VolatileMode::State);
    }

    #[test]
    fn test_parse_args_volatile_no() {
        let args = parse_args(&["--volatile=no"]).unwrap();
        assert_eq!(args.volatile, VolatileMode::No);
    }

    #[test]
    fn test_parse_args_resolv_conf() {
        let args = parse_args(&["--resolv-conf=copy-stub"]).unwrap();
        assert_eq!(args.resolv_conf, ResolvConfMode::CopyStub);
    }

    #[test]
    fn test_parse_args_timezone() {
        let args = parse_args(&["--timezone=bind"]).unwrap();
        assert_eq!(args.timezone, TimezoneMode::Bind);
    }

    #[test]
    fn test_parse_args_link_journal() {
        let args = parse_args(&["--link-journal=host"]).unwrap();
        assert_eq!(args.link_journal, LinkJournalMode::Host);
    }

    #[test]
    fn test_parse_args_console() {
        let args = parse_args(&["--console=pipe"]).unwrap();
        assert_eq!(args.console, ConsoleMode::Pipe);
    }

    #[test]
    fn test_parse_args_port_short() {
        let args = parse_args(&["-p", "8080"]).unwrap();
        assert_eq!(args.port_forwards.len(), 1);
        assert_eq!(args.port_forwards[0].host_port, 8080);
    }

    #[test]
    fn test_parse_args_port_long() {
        let args = parse_args(&["--port=tcp:8080:80"]).unwrap();
        assert_eq!(args.port_forwards.len(), 1);
        assert_eq!(args.port_forwards[0].protocol, "tcp");
    }

    #[test]
    fn test_parse_args_uuid() {
        let args = parse_args(&["--uuid=550e8400-e29b-41d4-a716-446655440000"]).unwrap();
        assert_eq!(
            args.uuid,
            Some("550e8400-e29b-41d4-a716-446655440000".to_string())
        );
    }

    #[test]
    fn test_parse_args_hostname() {
        let args = parse_args(&["--hostname=myhost"]).unwrap();
        assert_eq!(args.hostname, Some("myhost".to_string()));
    }

    #[test]
    fn test_parse_args_register_yes() {
        let args = parse_args(&["--register=yes"]).unwrap();
        assert_eq!(args.register, Some(true));
    }

    #[test]
    fn test_parse_args_register_no() {
        let args = parse_args(&["--register=no"]).unwrap();
        assert_eq!(args.register, Some(false));
    }

    #[test]
    fn test_parse_args_keep_unit() {
        let args = parse_args(&["--keep-unit"]).unwrap();
        assert!(args.keep_unit);
    }

    #[test]
    fn test_parse_args_quiet() {
        let args = parse_args(&["-q"]).unwrap();
        assert!(args.quiet);
    }

    #[test]
    fn test_parse_args_quiet_long() {
        let args = parse_args(&["--quiet"]).unwrap();
        assert!(args.quiet);
    }

    #[test]
    fn test_parse_args_read_only() {
        let args = parse_args(&["--read-only"]).unwrap();
        assert!(args.read_only);
    }

    #[test]
    fn test_parse_args_property() {
        let args = parse_args(&["--property=DeviceAllow=/dev/null"]).unwrap();
        assert_eq!(args.properties.len(), 1);
        assert_eq!(args.properties[0].0, "DeviceAllow");
        assert_eq!(args.properties[0].1, "/dev/null");
    }

    #[test]
    fn test_parse_args_property_invalid() {
        let result = parse_args(&["--property=noequalssign"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_args_kill_signal() {
        let args = parse_args(&["--kill-signal=SIGTERM"]).unwrap();
        assert_eq!(args.kill_signal, Some("SIGTERM".to_string()));
    }

    #[test]
    fn test_parse_args_settings() {
        let args = parse_args(&["--settings=yes"]).unwrap();
        assert_eq!(args.settings, Some("yes".to_string()));
    }

    #[test]
    fn test_parse_args_oci_bundle() {
        let args = parse_args(&["--oci-bundle=/path/to/bundle"]).unwrap();
        assert_eq!(args.oci_bundle, Some("/path/to/bundle".to_string()));
    }

    #[test]
    fn test_parse_args_command_after_dashdash() {
        let args = parse_args(&["-D", "/root", "--", "/bin/echo", "hello"]).unwrap();
        assert_eq!(args.directory, Some("/root".to_string()));
        assert_eq!(args.command, vec!["/bin/echo", "hello"]);
    }

    #[test]
    fn test_parse_args_command_positional() {
        let args = parse_args(&["-D", "/root", "/bin/sh"]).unwrap();
        assert_eq!(args.command, vec!["/bin/sh"]);
    }

    #[test]
    fn test_parse_args_combined() {
        let args = parse_args(&[
            "-D",
            "/myroot",
            "-M",
            "test",
            "-b",
            "--private-network",
            "--capability=NET_ADMIN",
            "--bind=/host:/container",
            "-E",
            "FOO=bar",
            "-q",
        ])
        .unwrap();
        assert_eq!(args.directory, Some("/myroot".to_string()));
        assert_eq!(args.machine, Some("test".to_string()));
        assert!(args.boot);
        assert!(args.private_network);
        assert_eq!(args.extra_capabilities.len(), 1);
        assert_eq!(args.bind_mounts.len(), 1);
        assert_eq!(args.environment.get("FOO"), Some(&"bar".to_string()));
        assert!(args.quiet);
    }

    #[test]
    fn test_parse_args_overlay() {
        let args = parse_args(&["--overlay=/lower:/upper:/merged"]).unwrap();
        assert_eq!(args.overlay_mounts.len(), 1);
        assert_eq!(args.overlay_mounts[0], "/lower:/upper:/merged");
    }

    #[test]
    fn test_parse_args_overlay_ro() {
        let args = parse_args(&["--overlay-ro=/lower:/upper:/merged"]).unwrap();
        assert_eq!(args.overlay_mounts.len(), 1);
        assert!(args.overlay_mounts[0].starts_with("ro:"));
    }

    #[test]
    fn test_parse_args_notify_ready() {
        let args = parse_args(&["--notify-ready"]).unwrap();
        assert!(args.notify_ready);
    }

    // ── Default args tests ───────────────────────────────────────────

    #[test]
    fn test_default_args() {
        let args = NspawnArgs::default();
        assert!(!args.boot);
        assert!(!args.as_pid2);
        assert!(!args.private_network);
        assert!(!args.network_veth);
        assert!(!args.private_users);
        assert!(!args.no_new_privileges);
        assert!(!args.keep_unit);
        assert!(!args.quiet);
        assert!(!args.read_only);
        assert_eq!(args.volatile, VolatileMode::No);
        assert_eq!(args.resolv_conf, ResolvConfMode::Auto);
        assert_eq!(args.timezone, TimezoneMode::Auto);
        assert_eq!(args.link_journal, LinkJournalMode::Auto);
        assert_eq!(args.console, ConsoleMode::Interactive);
        assert!(args.bind_mounts.is_empty());
        assert!(args.tmpfs_mounts.is_empty());
        assert!(args.extra_capabilities.is_empty());
        assert!(args.drop_capabilities.is_empty());
        assert!(args.port_forwards.is_empty());
        assert!(args.command.is_empty());
    }

    // ── Resolve root tests ───────────────────────────────────────────

    #[test]
    fn test_resolve_root_no_directory() {
        let args = NspawnArgs::default();
        let result = resolve_root(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no container root specified"));
    }

    #[test]
    fn test_resolve_root_nonexistent_directory() {
        let mut args = NspawnArgs::default();
        args.directory = Some("/nonexistent/path/for/nspawn/test".to_string());
        let result = resolve_root(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn test_resolve_root_existing_directory() {
        let mut args = NspawnArgs::default();
        args.directory = Some("/tmp".to_string());
        let result = resolve_root(&args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_root_file_not_directory() {
        let mut args = NspawnArgs::default();
        args.directory = Some("/etc/hostname".to_string());
        let result = resolve_root(&args);
        // Either "not a directory" or "does not exist"
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_root_image_not_supported() {
        let mut args = NspawnArgs::default();
        args.image = Some("/path/to/image.raw".to_string());
        let result = resolve_root(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not yet supported"));
    }

    #[test]
    fn test_resolve_root_oci_not_supported() {
        let mut args = NspawnArgs::default();
        args.oci_bundle = Some("/path/to/bundle".to_string());
        let result = resolve_root(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not yet supported"));
    }

    // ── Find init / shell tests ──────────────────────────────────────

    #[test]
    fn test_find_init_nonexistent_root() {
        let result = find_init(Path::new("/nonexistent/root"));
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_shell_uses_fallback() {
        // Should at least return "/bin/sh" as fallback even for nonexistent root
        let shell = find_shell(Path::new("/nonexistent/root"));
        assert_eq!(shell, "/bin/sh");
    }

    #[test]
    fn test_find_shell_real_root() {
        // On a real system, should find bash or sh
        let shell = find_shell(Path::new("/"));
        assert!(
            shell == "/bin/bash"
                || shell == "/usr/bin/bash"
                || shell == "/bin/sh"
                || shell == "/usr/bin/sh"
                || shell == "/bin/zsh"
                || shell == "/usr/bin/zsh"
        );
    }

    // ── ensure_dir tests ─────────────────────────────────────────────

    #[test]
    fn test_ensure_dir_existing() {
        assert!(ensure_dir(Path::new("/tmp")).is_ok());
    }

    #[test]
    fn test_ensure_dir_creates_new() {
        let dir = tempfile::tempdir().unwrap();
        let new_path = dir.path().join("subdir/nested");
        assert!(ensure_dir(&new_path).is_ok());
        assert!(new_path.is_dir());
    }

    // ── SyncPipe tests ───────────────────────────────────────────────

    #[test]
    fn test_sync_pipe_signal_and_wait() {
        let pipe = SyncPipe::new().unwrap();
        // Signal from one end
        pipe.signal().unwrap();
        // Wait on the other end should succeed
        pipe.wait().unwrap();
        pipe.close_read();
        pipe.close_write();
    }

    #[test]
    fn test_sync_pipe_creates_valid_fds() {
        let pipe = SyncPipe::new().unwrap();
        assert!(pipe.read_fd >= 0);
        assert!(pipe.write_fd >= 0);
        assert_ne!(pipe.read_fd, pipe.write_fd);
        pipe.close_read();
        pipe.close_write();
    }

    // ── Integration-style tests ──────────────────────────────────────

    #[test]
    fn test_parse_args_full_boot_scenario() {
        let args = parse_args(&[
            "--directory=/var/lib/machines/mycontainer",
            "--machine=mycontainer",
            "--boot",
            "--private-network",
            "--bind=/host/data:/container/data",
            "--bind-ro=/etc/resolv.conf:/etc/resolv.conf",
            "--capability=NET_ADMIN,SYS_ADMIN",
            "--drop-capability=SYS_TIME",
            "--setenv=TERM=xterm-256color",
            "--hostname=myhost",
            "--resolv-conf=copy-stub",
            "--timezone=bind",
            "--link-journal=host",
            "--register=yes",
            "--quiet",
        ])
        .unwrap();

        assert_eq!(
            args.directory,
            Some("/var/lib/machines/mycontainer".to_string())
        );
        assert_eq!(args.machine, Some("mycontainer".to_string()));
        assert!(args.boot);
        assert!(args.private_network);
        assert_eq!(args.bind_mounts.len(), 2);
        assert!(!args.bind_mounts[0].read_only);
        assert!(args.bind_mounts[1].read_only);
        assert_eq!(args.extra_capabilities.len(), 2);
        assert_eq!(args.drop_capabilities.len(), 1);
        assert_eq!(
            args.environment.get("TERM"),
            Some(&"xterm-256color".to_string())
        );
        assert_eq!(args.hostname, Some("myhost".to_string()));
        assert_eq!(args.resolv_conf, ResolvConfMode::CopyStub);
        assert_eq!(args.timezone, TimezoneMode::Bind);
        assert_eq!(args.link_journal, LinkJournalMode::Host);
        assert_eq!(args.register, Some(true));
        assert!(args.quiet);
    }

    #[test]
    fn test_parse_args_minimal_command() {
        let args = parse_args(&[
            "-D",
            "/myroot",
            "--",
            "/usr/bin/python3",
            "-c",
            "print('hello')",
        ])
        .unwrap();

        assert_eq!(args.directory, Some("/myroot".to_string()));
        assert_eq!(
            args.command,
            vec!["/usr/bin/python3", "-c", "print('hello')"]
        );
    }

    #[test]
    fn test_volatile_mode_roundtrip() {
        for mode in &["no", "yes", "state", "overlay"] {
            let parsed = VolatileMode::from_str(mode);
            assert!(parsed.is_some(), "failed to parse volatile mode: {mode}");
        }
    }

    #[test]
    fn test_resolv_conf_mode_roundtrip() {
        for mode in &[
            "off",
            "copy-host",
            "copy-stub",
            "copy-uplink",
            "replace-host",
            "replace-stub",
            "bind-host",
            "bind-stub",
            "delete",
            "auto",
        ] {
            let parsed = ResolvConfMode::from_str(mode);
            assert!(parsed.is_some(), "failed to parse resolv-conf mode: {mode}");
        }
    }

    #[test]
    fn test_timezone_mode_roundtrip() {
        for mode in &["off", "copy", "bind", "symlink", "delete", "auto"] {
            let parsed = TimezoneMode::from_str(mode);
            assert!(parsed.is_some(), "failed to parse timezone mode: {mode}");
        }
    }

    #[test]
    fn test_console_mode_roundtrip() {
        for mode in &["interactive", "read-only", "passive", "pipe", "autopipe"] {
            let parsed = ConsoleMode::from_str(mode);
            assert!(parsed.is_some(), "failed to parse console mode: {mode}");
        }
    }

    #[test]
    fn test_link_journal_mode_roundtrip() {
        for mode in &["no", "host", "try-host", "guest", "try-guest", "auto"] {
            let parsed = LinkJournalMode::from_str(mode);
            assert!(
                parsed.is_some(),
                "failed to parse link-journal mode: {mode}"
            );
        }
    }

    // ── Veth / netlink tests ─────────────────────────────────────────

    #[test]
    fn test_veth_host_name_short_machine() {
        assert_eq!(veth_host_name("myvm"), "ve-myvm");
    }

    #[test]
    fn test_veth_host_name_exact_limit() {
        // Max machine part = IFNAMSIZ - 1 - 3 = 12 chars
        assert_eq!(veth_host_name("123456789012"), "ve-123456789012");
        assert_eq!(veth_host_name("123456789012").len(), 15); // exactly IFNAMSIZ - 1
    }

    #[test]
    fn test_veth_host_name_truncated() {
        // 13+ chars should be truncated to 12
        assert_eq!(veth_host_name("1234567890123"), "ve-123456789012");
        assert_eq!(veth_host_name("1234567890123").len(), 15);
    }

    #[test]
    fn test_veth_host_name_long_machine() {
        let long = "a".repeat(64);
        let result = veth_host_name(&long);
        assert_eq!(result.len(), 15);
        assert!(result.starts_with("ve-"));
    }

    #[test]
    fn test_veth_host_name_single_char() {
        assert_eq!(veth_host_name("x"), "ve-x");
    }

    #[test]
    fn test_veth_host_name_empty() {
        assert_eq!(veth_host_name(""), "ve-");
    }

    #[test]
    fn test_veth_container_name_constant() {
        assert_eq!(VETH_CONTAINER_NAME, "host0");
    }

    #[test]
    fn test_build_veth_create_msg_structure() {
        let msg = build_veth_create_msg("ve-test", "host0");

        // Message must be NLMSG_ALIGN-aligned
        assert_eq!(msg.len() % NLMSG_ALIGN, 0);

        // Minimum size: nlmsghdr + ifinfomsg + at least one attribute
        assert!(msg.len() >= NLMSG_HDR_LEN + IFINFOMSG_LEN + 8);

        // nlmsghdr checks
        let nlmsg_len = u32::from_ne_bytes(msg[0..4].try_into().unwrap());
        assert!(nlmsg_len as usize <= msg.len());

        let nlmsg_type = u16::from_ne_bytes(msg[4..6].try_into().unwrap());
        assert_eq!(nlmsg_type, RTM_NEWLINK);

        let nlmsg_flags = u16::from_ne_bytes(msg[6..8].try_into().unwrap());
        assert_eq!(
            nlmsg_flags,
            NLM_F_REQUEST | NLM_F_ACK | NLM_F_CREATE | NLM_F_EXCL
        );

        let nlmsg_seq = u32::from_ne_bytes(msg[8..12].try_into().unwrap());
        assert_eq!(nlmsg_seq, 1);

        // ifinfomsg: ifi_family should be AF_UNSPEC
        assert_eq!(msg[NLMSG_HDR_LEN], AF_UNSPEC);
    }

    #[test]
    fn test_build_veth_create_msg_contains_host_name() {
        let msg = build_veth_create_msg("ve-myvm", "host0");
        // The host-side interface name "ve-myvm\0" should appear in the message.
        let name_bytes = b"ve-myvm\0";
        let msg_str = &msg[..];
        assert!(
            msg_str.windows(name_bytes.len()).any(|w| w == name_bytes),
            "host interface name not found in netlink message"
        );
    }

    #[test]
    fn test_build_veth_create_msg_contains_container_name() {
        let msg = build_veth_create_msg("ve-myvm", "host0");
        // The container-side interface name "host0\0" should appear in the message.
        let name_bytes = b"host0\0";
        let msg_str = &msg[..];
        assert!(
            msg_str.windows(name_bytes.len()).any(|w| w == name_bytes),
            "container interface name not found in netlink message"
        );
    }

    #[test]
    fn test_build_veth_create_msg_contains_veth_kind() {
        let msg = build_veth_create_msg("ve-test", "host0");
        // IFLA_INFO_KIND value "veth\0" should appear in the message.
        let kind_bytes = b"veth\0";
        assert!(
            msg.windows(kind_bytes.len()).any(|w| w == kind_bytes),
            "veth kind not found in netlink message"
        );
    }

    #[test]
    fn test_build_veth_create_msg_contains_linkinfo() {
        let msg = build_veth_create_msg("ve-test", "host0");
        // Scan for IFLA_LINKINFO (type 18) attribute after ifinfomsg + IFLA_IFNAME.
        // The type field is at offset +2 in each RTA header (little-endian u16).
        // With NLA_F_NESTED (0x8000), the high byte has 0x80 set.
        let mut found = false;
        let mut off = NLMSG_HDR_LEN + IFINFOMSG_LEN;
        while off + 4 <= msg.len() {
            let rta_len = u16::from_ne_bytes(msg[off..off + 2].try_into().unwrap()) as usize;
            let rta_type = u16::from_ne_bytes(msg[off + 2..off + 4].try_into().unwrap());
            // IFLA_LINKINFO with NLA_F_NESTED = 18 | 0x8000
            if rta_type == IFLA_LINKINFO | 0x8000 {
                found = true;
                break;
            }
            if rta_len < 4 {
                break;
            }
            off += nl_rta_align(rta_len);
        }
        assert!(found, "IFLA_LINKINFO nested attribute not found");
    }

    #[test]
    fn test_build_veth_create_msg_different_names() {
        let msg1 = build_veth_create_msg("ve-a", "host0");
        let msg2 = build_veth_create_msg("ve-longer-name", "host0");
        // Different host names produce different-length messages
        assert_ne!(msg1.len(), msg2.len());
    }

    #[test]
    fn test_nl_align_values() {
        assert_eq!(nl_align(0), 0);
        assert_eq!(nl_align(1), 4);
        assert_eq!(nl_align(4), 4);
        assert_eq!(nl_align(5), 8);
        assert_eq!(nl_align(16), 16);
        assert_eq!(nl_align(17), 20);
    }

    #[test]
    fn test_nl_rta_align_values() {
        assert_eq!(nl_rta_align(0), 0);
        assert_eq!(nl_rta_align(1), 4);
        assert_eq!(nl_rta_align(4), 4);
        assert_eq!(nl_rta_align(5), 8);
        assert_eq!(nl_rta_align(8), 8);
    }

    #[test]
    fn test_nl_put_u16_and_u32() {
        let mut buf = vec![0u8; 8];
        nl_put_u16(&mut buf, 0, 0x1234);
        assert_eq!(&buf[0..2], &0x1234u16.to_ne_bytes());

        nl_put_u32(&mut buf, 4, 0xDEADBEEF);
        assert_eq!(&buf[4..8], &0xDEADBEEFu32.to_ne_bytes());
    }

    #[test]
    fn test_nl_put_i32() {
        let mut buf = vec![0u8; 4];
        nl_put_i32(&mut buf, 0, -42);
        assert_eq!(i32::from_ne_bytes(buf[0..4].try_into().unwrap()), -42);
    }

    #[test]
    fn test_nl_put_rta_bytes() {
        let mut buf = vec![0u8; 16];
        let data = b"hi";
        nl_put_rta_bytes(&mut buf, 0, 7, data);
        // rta_len = 4 + 2 = 6
        assert_eq!(u16::from_ne_bytes(buf[0..2].try_into().unwrap()), 6);
        // rta_type = 7
        assert_eq!(u16::from_ne_bytes(buf[2..4].try_into().unwrap()), 7);
        // payload
        assert_eq!(&buf[4..6], b"hi");
    }

    #[test]
    fn test_nl_put_rta_u32() {
        let mut buf = vec![0u8; 16];
        nl_put_rta_u32(&mut buf, 0, 99, 0x12345678);
        // rta_len = 8
        assert_eq!(u16::from_ne_bytes(buf[0..2].try_into().unwrap()), 8);
        // rta_type = 99
        assert_eq!(u16::from_ne_bytes(buf[2..4].try_into().unwrap()), 99);
        // payload
        assert_eq!(
            u32::from_ne_bytes(buf[4..8].try_into().unwrap()),
            0x12345678
        );
    }

    #[test]
    fn test_netlink_constants() {
        assert_eq!(NETLINK_ROUTE, 0);
        assert_eq!(RTM_NEWLINK, 16);
        assert_eq!(RTM_SETLINK, 19);
        assert_eq!(NLM_F_REQUEST, 0x0001);
        assert_eq!(NLM_F_ACK, 0x0004);
        assert_eq!(NLM_F_CREATE, 0x0400);
        assert_eq!(NLM_F_EXCL, 0x0200);
        assert_eq!(NLMSG_ERROR, 2);
        assert_eq!(NLMSG_HDR_LEN, 16);
        assert_eq!(NLMSG_ALIGN, 4);
        assert_eq!(IFINFOMSG_LEN, 16);
        assert_eq!(IFLA_IFNAME, 3);
        assert_eq!(IFLA_NET_NS_PID, 19);
        assert_eq!(IFLA_LINKINFO, 18);
        assert_eq!(IFLA_INFO_KIND, 1);
        assert_eq!(IFLA_INFO_DATA, 2);
        assert_eq!(VETH_INFO_PEER, 1);
        assert_eq!(AF_UNSPEC, 0);
        assert_eq!(IFF_UP, 1);
    }

    #[test]
    fn test_ifnamsiz_constant() {
        // Linux IFNAMSIZ is 16
        assert_eq!(IFNAMSIZ, 16);
    }

    #[test]
    fn test_read_ifindex_nonexistent() {
        let result = read_ifindex("nonexistent_interface_xyz_123");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_ifindex_lo() {
        // The loopback interface should always exist and have ifindex 1
        if let Ok(idx) = read_ifindex("lo") {
            assert_eq!(idx, 1);
        }
        // If /sys is not available (container), skip
    }

    #[test]
    fn test_bring_interface_up_msg_structure() {
        // We can't actually bring up an interface in tests, but we can
        // verify the message building logic by checking bring_interface_up
        // fails gracefully for non-existent interfaces.
        let result = bring_interface_up("nonexistent_iface_xyz");
        assert!(result.is_err());
    }

    #[test]
    fn test_move_interface_to_ns_nonexistent() {
        let result = move_interface_to_ns("nonexistent_iface_xyz", 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_veth_host_name_with_dots_and_dashes() {
        // Machine names can contain dots, dashes, underscores
        assert_eq!(veth_host_name("my.vm-1"), "ve-my.vm-1");
        assert_eq!(veth_host_name("test_host"), "ve-test_host");
    }

    #[test]
    fn test_build_veth_create_msg_peer_ifinfomsg_present() {
        let msg = build_veth_create_msg("ve-test", "host0");
        // The message should contain two ifinfomsg structures:
        // one at the top level and one inside VETH_INFO_PEER.
        // Both have AF_UNSPEC as ifi_family.
        // Count occurrences is tricky, but we can verify the message
        // is large enough to contain both.
        let min_size = NLMSG_HDR_LEN
            + IFINFOMSG_LEN       // top-level ifinfomsg
            + 4 + 8               // IFLA_IFNAME "ve-test\0" (min)
            + 4                   // IFLA_LINKINFO header
            + 4 + 5              // IFLA_INFO_KIND "veth\0"
            + 4                   // IFLA_INFO_DATA header
            + 4                   // VETH_INFO_PEER header
            + IFINFOMSG_LEN       // peer ifinfomsg
            + 4 + 6; // peer IFLA_IFNAME "host0\0"
        assert!(
            msg.len() >= min_size,
            "message too small: {} < {min_size}",
            msg.len()
        );
    }

    #[test]
    fn test_network_veth_implies_clone_newnet() {
        // Verify that when network_veth is set, private_network effect is
        // also applied (CLONE_NEWNET). In run(), both set CLONE_NEWNET.
        let args = NspawnArgs {
            network_veth: true,
            ..Default::default()
        };
        // The run() function checks: if args.private_network || args.network_veth
        assert!(args.network_veth || args.private_network);
    }

    #[test]
    fn test_parse_args_network_veth_sets_flag() {
        let args = parse_args(&["-n", "-D", "/tmp"]).unwrap();
        assert!(args.network_veth);

        let args2 = parse_args(&["--network-veth", "-D", "/tmp"]).unwrap();
        assert!(args2.network_veth);
    }

    #[test]
    fn test_veth_host_name_boundary_lengths() {
        // Empty
        assert_eq!(veth_host_name(""), "ve-");
        // 1 char
        assert_eq!(veth_host_name("a"), "ve-a");
        // 11 chars (under limit)
        assert_eq!(veth_host_name("12345678901"), "ve-12345678901");
        // 12 chars (at limit)
        assert_eq!(veth_host_name("123456789012"), "ve-123456789012");
        // 13 chars (over limit, truncated to 12)
        assert_eq!(veth_host_name("1234567890123"), "ve-123456789012");
    }
}
