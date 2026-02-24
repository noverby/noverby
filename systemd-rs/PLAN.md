# Implementation Plan

This document describes the phased plan for rewriting systemd as a pure Rust drop-in replacement. See [CHANGELOG.md](CHANGELOG.md) for detailed recent changes.

## Current Status

**🟢 NixOS boots successfully with systemd-rs as PID 1** — reaches `multi-user.target` with login prompt in ~7 seconds (cloud-hypervisor VM, full networking via networkd + resolved). **4,593 unit tests passing** across 66 crates.

| Phase | Status |
|-------|--------|
| Phase 0 — Foundation | ✅ Complete |
| Phase 1 — Core System | ✅ Complete |
| Phase 2 — Essential System Services | 🔶 In progress (udevd partial, logind/hostnamed/timedated/localed D-Bus done, OnCalendar= full parser done, next: remaining D-Bus interfaces for networkd/resolved/machined/portabled/homed) |
| Phase 3 — Network Stack | 🔶 Partial (networkd, resolved, timesyncd, timedated ✅ D-Bus, hostnamed ✅ D-Bus, localed ✅ D-Bus) |
| Phase 4 — Extended Services | 🔶 Partial (machined, portabled, homed, oomd, coredump, sysext, dissect, firstboot, creds) |
| Phase 5 — Utilities, Boot & Polish | 🔶 Partial (analyze, cgls, cgtop, mount, socket-activate, ac-power, detect-virt, generator framework) |

### Unit File Directive Coverage

198 of 425 upstream systemd directives supported (47%). Per-section breakdown:

| Section | Supported | Partial | Unsupported | Total | Coverage |
|---------|-----------|---------|-------------|-------|----------|
| systemd.unit | 45 | 0 | 43 | 88 | 51% |
| systemd.service | 25 | 0 | 9 | 34 | 74% |
| systemd.exec | 74 | 2 | 71 | 147 | 50% |
| systemd.socket | 27 | 0 | 33 | 60 | 45% |
| systemd.resource-control | 11 | 0 | 37 | 48 | 23% |
| sd_notify | 3 | 0 | 12 | 15 | 20% |
| systemd.kill | 3 | 0 | 4 | 7 | 43% |
| systemd.timer | 9 | 1 | 4 | 14 | 64% |
| systemd.path | 0 | 1 | 7 | 8 | 0% |
| systemd.slice | 1 | 0 | 2 | 3 | 33% |
| systemd.device | 0 | 1 | 0 | 1 | 0% |

Legend: ✅ = complete, 🔶 = partial, ❌ = not started

## Project Structure

The project is organized as a Cargo workspace with a shared core library and individual crates for each systemd component (66 crates):

```text
crates/
├── libsystemd/          # Core library: unit parsing, dependency graph, sd-bus protocol,
│                        # sd_notify, journal format, specifier expansion, etc.
├── systemd/             # PID 1 service manager (init system)
├── systemctl/           # CLI control tool for the service manager
├── journald/            # Journal logging daemon (systemd-journald)
├── journalctl/          # Journal query tool
├── udevd/               # Device manager (systemd-udevd) 🔶
├── udevadm/             # udev administration tool 🔶
├── logind/              # Login and session manager (systemd-logind) 🔶
├── loginctl/            # Login manager control tool 🔶
├── networkd/            # Network configuration manager (systemd-networkd) 🔶
├── networkctl/          # Network manager control tool 🔶
├── resolved/            # DNS stub resolver (systemd-resolved) 🔶
├── resolvectl/          # Resolver control tool 🔶
├── timesyncd/           # NTP time synchronization (systemd-timesyncd)
├── timedated/           # Time/date manager daemon (systemd-timedated) ✅ + D-Bus
├── timedatectl/         # Time/date control tool
├── user-sessions/       # User session gate (systemd-user-sessions)
├── update-done/         # Update completion marker (systemd-update-done)
├── random-seed/         # Random seed persistence (systemd-random-seed)
├── pstore/              # Persistent storage archival (systemd-pstore)
├── machine-id-setup/    # Machine ID initialization (systemd-machine-id-setup)
├── tmpfiles/            # Temporary file manager (systemd-tmpfiles)
├── sysusers/            # Declarative system user manager (systemd-sysusers)
├── hostnamed/           # Hostname manager daemon (systemd-hostnamed) ✅ + D-Bus
├── hostnamectl/         # Hostname control tool ✅
├── localed/             # Locale manager daemon (systemd-localed) ✅ + D-Bus
├── localectl/           # Locale control tool ✅
├── machined/            # VM/container manager daemon (systemd-machined) ✅
├── machinectl/          # Machine manager control tool ✅
├── homed/               # Home directory manager (systemd-homed) ✅
├── homectl/             # Home directory control tool ✅
├── nspawn/              # Container runtime (systemd-nspawn)
├── portabled/           # Portable service manager (systemd-portabled) ✅
├── portablectl/         # Portable service control tool ✅
├── ask-password/        # Password query tool (systemd-ask-password) ✅
├── tty-ask-password-agent/ # Password agent (systemd-tty-ask-password-agent) ✅

├── oomd/                # Userspace OOM killer (systemd-oomd)
├── oomctl/              # OOM killer control tool
├── coredump/            # Core dump handler (systemd-coredump) ✅
├── coredumpctl/         # Core dump query tool ✅
├── analyze/             # Boot performance analyzer (systemd-analyze) ✅
├── run/                 # Transient unit runner (systemd-run)
├── cgls/                # Cgroup listing tool (systemd-cgls) ✅
├── cgtop/               # Cgroup resource monitor (systemd-cgtop) ✅
├── cat/                 # Unit file viewer (systemd-cat)
├── delta/               # Unit file override viewer (systemd-delta)
├── detect-virt/         # Virtualization detector (systemd-detect-virt)
├── escape/              # Unit name escaping tool (systemd-escape)
├── id128/               # 128-bit ID tool (systemd-id128)
├── mount/               # Mount/unmount utilities (systemd-mount, systemd-umount) ✅
├── notify/              # Notification sender (systemd-notify)
├── path/                # Path operation tool (systemd-path)
├── socket-activate/     # Socket activation tool (systemd-socket-activate) ✅
├── ask-password/        # Password query tool (systemd-ask-password) ✅
├── tty-ask-password-agent/ # Password agent (systemd-tty-ask-password-agent) ✅
├── inhibit/             # Inhibitor lock tool (systemd-inhibit) ✅
├── creds/               # Credential management (systemd-creds)
├── dissect/             # Image dissection tool (systemd-dissect) ✅
├── firstboot/           # First-boot configuration (systemd-firstboot) ✅
├── repart/              # Partition manager (systemd-repart)
├── sysext/              # System extension manager (systemd-sysext) ✅
├── modules-load/        # Kernel module loader (systemd-modules-load)
├── sysctl/              # Sysctl applicator (systemd-sysctl)
├── binfmt/              # binfmt_misc registration (systemd-binfmt)
├── vconsole-setup/      # Virtual console setup (systemd-vconsole-setup)
├── backlight/           # Backlight save/restore (systemd-backlight)
├── rfkill/              # RF kill switch save/restore (systemd-rfkill)
├── cryptsetup/          # LUKS/dm-crypt setup (systemd-cryptsetup)
├── veritysetup/         # dm-verity setup (systemd-veritysetup)
├── integritysetup/      # dm-integrity setup (systemd-integritysetup)
├── boot/                # sd-boot and bootctl (UEFI boot manager)
├── stub/                # sd-stub (UEFI stub)
├── shutdown/            # System shutdown/reboot (systemd-shutdown)
├── sleep/               # Suspend/hibernate handler (systemd-sleep)
├── ac-power/            # AC power detection (systemd-ac-power)
└── generator/           # Generator framework for auto-generating units
```

## Phase 0 — Foundation (Workspace & Core Library)

> **Note:** The `calendar_spec` module in `libsystemd` provides a full systemd calendar expression parser and evaluator, supporting weekday filters, ranges, lists, repetitions (`*/N`, `start/step`, `start..end/step`), and all standard shorthands. The `CalendarSpec::next_elapse()` function computes the next matching wall-clock time, used by the timer scheduler for `OnCalendar=` directives and by `systemd-analyze calendar` for next-elapse display.

Restructure the existing codebase into a Cargo workspace and extract shared functionality into `libsystemd`:

- ✅ **Unit file parser** — complete INI-style parser with all systemd extensions (line continuation, quoting rules, specifier expansion `%i`, `%n`, `%N`, `%p`, `%u`, `%U`, `%h`, `%s`, `%m`, `%b`, `%H`, `%v`, `%t`, etc.)
- ✅ **Dependency graph engine** — topological sort with cycle detection, transaction model for atomic start/stop operations
- 🔶 **D-Bus protocol** — uses C `libdbus` via the `dbus` crate; wire format implementation planned but not yet needed for boot
- ✅ **sd_notify protocol** — full notify socket implementation with credential passing and fd store
- 🔶 **Journal binary format** — reader/writer partially implemented; journald starts and collects logs during boot
- 🔶 **Specifier expansion** — common specifiers (`%i`, `%n`, `%N`, `%p`, `%u`, `%U`, `%h`, `%s`, `%m`, `%b`, `%H`, `%v`, `%t`) implemented; some rare specifiers may be missing
- ✅ **Unit name handling** — escaping, unescaping, template instantiation, unit type detection
- ✅ **Configuration parsing** — `/etc/systemd/system.conf`, `/etc/systemd/user.conf`, and environment generators
- ✅ **Credential management** — `ImportCredential=` fully implemented (glob-matching from system credential stores), `LoadCredential=` implemented (absolute and relative paths, directory loading), `SetCredential=` implemented (inline data with colon-preserving split), `LoadCredentialEncrypted=` and `SetCredentialEncrypted=` now decrypt at runtime using AES-256-GCM with host key or null key (graceful fallback to writing as-is if decryption fails); credential directory created at `/run/credentials/<unit>/` with correct ownership and 0o700/0o400 permissions; `CREDENTIALS_DIRECTORY` env var set; priority ordering matches systemd (SetCredential < LoadCredential < ImportCredential); `systemd-creds` CLI tool provides encrypt/decrypt with host key (AES-256-GCM), list, cat, setup, and TPM2 detection; missing: TPM2 sealing, host+tpm2 combined mode

Legend: ✅ = implemented, 🔶 = partial, ❌ = not started

## Phase 1 — Core System (PID 1 + systemctl + journald)

The minimum viable system to boot a real Linux machine:

- ✅ **`systemd` (PID 1)** — service manager with all core unit types (service, socket, target, mount, timer, path, slice, scope) and all service types (`simple`, `exec`, `notify`, `notify-reload`, `oneshot`, `forking`, `dbus`, `idle`), default target handling, parallel activation, fstab generator, getty generator, NixOS early boot setup, full `Condition*`/`Assert*` directive support (15 check types), proper `Type=idle` deferral (idle services wait for all other jobs to complete before starting); missing: emergency/rescue mode, external generators, transient units, reexecution, `SIGRTMIN+` signals
- ✅ **`systemctl`** — CLI including `start`, `stop`, `restart`, `try-restart`, `reload-or-restart`, `enable`, `disable`, `kill`, `reset-failed`, `suspend`, `hibernate`, `hybrid-sleep`, `suspend-then-hibernate`, `status`, `show`, `cat`, `list-units`, `list-unit-files`, `list-dependencies`, `mask`, `unmask`, `is-active`, `is-enabled`, `is-failed`, `poweroff`, `reboot`, `daemon-reload`; `show` supports `-p`/`--property` filtering and `--value` for value-only output; `cat` displays unit file source with path header; `list-dependencies` shows dependency tree with box-drawing characters, status markers, `--reverse` for reverse deps; `mask`/`unmask` create/remove `/dev/null` symlinks in `/etc/systemd/system/`; `kill` supports `--signal`/`-s` with numeric or named signals; `reset-failed` clears error state for one or all units; sleep commands (`suspend`/`hibernate`/`hybrid-sleep`/`suspend-then-hibernate`) spawn `systemd-sleep` via PID 1; handles common flags (`--no-block`, `--quiet`, `--force`, `--no-pager`, `--system`, `-a`, `-q`, `-f`, `-l`, `-t`, `-p`); proper exit codes for query commands; missing: `edit`, `set-property`, `revert`
- ✅ **`journald`** — journal logging daemon with `/dev/log` socket, native protocol, syslog protocol, kernel `kmsg`; missing: rate limiting, journal file rotation, disk usage limits, forward-secure sealing, wall message forwarding
- ✅ **`journalctl`** — journal query tool with basic filtering and output formats; missing: some advanced filters and output modes
- ✅ **`systemd-shutdown`** — clean shutdown/reboot with filesystem unmount, loop device detach, DM detach, MD RAID stop
- ✅ **`systemd-sleep`** — suspend/hibernate/hybrid-sleep handling
- ✅ **`systemd-notify`** — CLI tool for sending notifications
- ✅ **`systemd-run`** — transient unit creation (basic)
- ✅ **`systemd-escape`** — unit name escaping utility
- ✅ **`systemd-path`** — runtime path query utility
- ✅ **`systemd-id128`** — 128-bit ID operations
- ✅ **`systemd-delta`** — unit file override inspection
- ✅ **`systemd-cat`** — connect stdout/stderr to journal

## Phase 2 — Essential System Services

Services required for a fully functional desktop or server:

- 🔶 **`udevd`** — device manager daemon with netlink uevent monitoring (AF_NETLINK / NETLINK_KOBJECT_UEVENT), `.rules` file parser (full syntax: match/assign operators `==`/`!=`/`=`/`+=`/`-=`/`:=`, line continuation, escape sequences, `GOTO`/`LABEL` control flow), property matching (`KERNEL`, `SUBSYSTEM`, `ACTION`, `DRIVER`, `DEVTYPE`, `ATTR{file}`, `ENV{key}`, `RESULT`, `TEST`), parent device traversal (`KERNELS`, `SUBSYSTEMS`, `DRIVERS`, `ATTRS{file}`), assignment actions (`NAME`, `SYMLINK`, `OWNER`, `GROUP`, `MODE`, `ENV{key}`, `TAG`, `RUN{program}`, `RUN{builtin}`, `ATTR{file}`, `SYSCTL{key}`, `OPTIONS`), `IMPORT{program|file|cmdline|builtin|db|parent}`, `PROGRAM` execution with result capture, udev-style substitution expansion (`$kernel`/`%k`, `$number`/`%n`, `$devpath`/`%p`, `$id`/`%b`, `$driver`, `$attr{file}`/`%s{file}`, `$env{key}`/`%E{key}`, `$major`/`%M`, `$minor`/`%m`, `$result`/`%c` with index, `$name`/`%D`, `$links`, `$root`, `$sys`, `$devnode`/`%N`), glob matching (`*`, `?`, `[...]`, `|` alternatives), device database persistence (`/run/udev/data/`), device tag management (`/run/udev/tags/`), symlink creation/removal in `/dev/`, device node permission setting (OWNER/GROUP/MODE with name resolution), sysfs attribute writing, RUN program execution with environment passing, builtin handlers (path_id, input_id, usb_id, net_id, blkid, kmod), event queue with settle support, control socket, sd_notify protocol (READY/WATCHDOG/STATUS/STOPPING), signal handling (SIGTERM/SIGINT/SIGHUP/SIGCHLD); `udevadm` CLI with `info` (query by name/path, property export, attribute walk, database export, cleanup, `--device-id-of-file` for block device identification), `trigger` (action/subsystem/attribute/property/tag/sysname filters, prioritized subsystems, dry-run, verbose), `settle` (timeout, exit-if-exists, queue file + control socket polling), `monitor` (kernel uevent listening with subsystem filter, property display), `test` (device property display with rules file enumeration), `control` (reload/ping/stop/start exec queue, exit, log level, children max), `test-builtin`, `version`; 119 unit tests (86 udevd + 33 udevadm); missing: `.link` file parsing, `udev` database locking, event worker thread pool (currently inline), PROGRAM result capture propagation, full builtin implementations (hwdb, keyboard, net_setup_link), inotify-based rules reload, device renaming (NAME= for network interfaces), watch mode for settle, D-Bus interface
- ✅ **`tmpfiles`** — create/delete/clean temporary files and directories per `tmpfiles.d` configuration
- ✅ **`sysusers`** — create system users and groups per `sysusers.d` configuration
- 🔶 **`logind`** — login/seat/session tracking with session create/release/activate/lock/unlock, seat management (seat0 + dynamic), user tracking, inhibitor locks (block/delay modes, stale cleanup), input device monitoring for power/sleep buttons, idle hint tracking (per-session, per-seat, per-user, global), locked hint tracking, sd_notify/watchdog, control socket, `logind.conf` parsing ([Login] section with all standard keys, drop-in directory support, timespan parsing), VT switching via ioctl, AC power detection; **D-Bus interface (`org.freedesktop.login1`)** with full `Manager` object (GetSession/GetSessionByPID/GetUser/GetUserByPID/GetSeat, ListSessions/ListUsers/ListSeats/ListInhibitors, CreateSession/ReleaseSession, ActivateSession/ActivateSessionOnSeat, LockSession/UnlockSession/LockSessions/UnlockSessions, KillSession/KillUser, TerminateSession/TerminateUser/TerminateSeat, SetUserLinger, FlushDevices, PowerOff/Reboot/Halt/Suspend/Hibernate/HybridSleep/SuspendThenHibernate, CanPowerOff/CanReboot/CanHalt/CanSuspend/CanHibernate/CanHybridSleep/CanSuspendThenHibernate, Inhibit with pipe FD, ScheduleShutdown/CancelScheduledShutdown, SetWallMessage; 30+ properties including NAutoVTs, KillUserProcesses, IdleHint, BlockInhibited, DelayInhibited, HandlePowerKey/SuspendKey/HibernateKey/LidSwitch, PreparingForShutdown/Sleep, OnExternalPower, NCurrentSessions/NCurrentInhibitors), `Session` object (25 properties: Id, User, Name, Timestamp, VTNr, Seat, TTY, Display, Remote, Service, Desktop, Scope, Leader, Type, Class, Active, State, IdleHint, LockedHint; methods: Terminate, Activate, Lock, Unlock, SetIdleHint, SetLockedHint, SetType, Kill, TakeControl, ReleaseControl, SetBrightness, TakeDevice, ReleaseDevice, PauseDeviceComplete), `Seat` object (8 properties: Id, ActiveSession, CanGraphical, CanMultiSession, Sessions, IdleHint, IdleSinceHint; methods: Terminate, ActivateSession, SwitchTo, SwitchToNext, SwitchToPrevious), `User` object (15 properties: UID, GID, Name, Timestamp, RuntimePath, Service, Slice, Display, State, Sessions, IdleHint, Linger; methods: Terminate, Kill); D-Bus signal emission (SessionNew/SessionRemoved, UserNew/UserRemoved, SeatNew/SeatRemoved, PrepareForShutdown/PrepareForSleep, session Lock/Unlock); dynamic object registration/unregistration for sessions, seats, users; `loginctl` CLI with list/show/activate/lock/terminate commands; 98 unit tests (logind) + 12 unit tests (loginctl); missing: PAM module integration (`pam_systemd`), automatic session creation on login, multi-seat device assignment, ACL management, full TakeDevice implementation, polkit authorization
- ✅ **`user-sessions`** — manage `/run/nologin` to permit/deny user logins during boot/shutdown
- ✅ **`update-done`** — create/update `/etc/.updated` and `/var/.updated` stamp files for `ConditionNeedsUpdate=`
- ✅ **`random-seed`** — load/save kernel random seed across reboots via `/var/lib/systemd/random-seed`
- ✅ **`pstore`** — archive `/sys/fs/pstore/` crash entries to `/var/lib/systemd/pstore/`
- ✅ **`machine-id-setup`** — initialize or commit `/etc/machine-id` (with `--commit`, `--print`, `--root`)
- ✅ **`modules-load`** — load kernel modules from `modules-load.d` configuration
- ✅ **`sysctl`** — apply sysctl settings from `sysctl.d` configuration
- ✅ **`binfmt`** — register binary formats via `binfmt_misc` from `binfmt.d` configuration
- ✅ **`vconsole-setup`** — virtual console font and keymap configuration
- ✅ **`backlight`** / ✅ **`rfkill`** — save and restore hardware state across reboots
- ✅ **`ask-password`** / ✅ **`tty-ask-password-agent`** — password query framework with TTY input (echo suppression, backspace handling), agent protocol via question files in `/run/systemd/ask-password/`, Unix socket reply (`+password` / `-` cancel), credential lookup, wall message broadcasting, continuous watch mode; missing: inotify-based watching (uses polling), Plymouth integration, kernel keyring caching

## Phase 3 — Network Stack

Full network management:

- 🔶 **`networkd`** — network configuration daemon with `.network` file parsing ([Match], [Network], [Address], [Route], [DHCPv4], [Link] sections), DHCPv4 client with full DORA state machine (discover/offer/request/ack, lease renewal T1/T2, rebinding, exponential backoff retransmission, classless static routes RFC 3442, release/decline/inform), static IPv4 address and route configuration, netlink-based interface management (RTM_NEWLINK/GETLINK/NEWADDR/DELADDR/NEWROUTE/DELROUTE, bring up/down, set MTU, flush addresses/routes), DNS resolver configuration (`/run/systemd/resolve/resolv.conf`), runtime state files (`/run/systemd/netif/links/`, `/run/systemd/netif/leases/`, `/run/systemd/netif/state`), sd_notify protocol (READY/WATCHDOG/STATUS/STOPPING), signal handling (SIGTERM/SIGINT for shutdown, SIGHUP for reload), NixOS integration enabled (`withNetworkd = true`); `networkctl` CLI with `list` (interface table with type/operational/setup state), `status [LINK]` (detailed per-link info including address/gateway/DNS/DHCP lease), `lldp` (stub); missing: `.netdev` file parsing, `.link` file parsing, DHCPv6 client, DHCPv6-PD, IPv6 RA, IPv6 address management, routing policy rules, bridge/bond/VLAN/VXLAN/WireGuard/tunnel/MACsec creation, D-Bus interface (`org.freedesktop.network1`), `networkctl` reconfigure/reload/forcerenew, `systemd-networkd-wait-online`, `systemd-network-generator`
- 🔶 **`resolved`** — stub DNS resolver daemon with `resolved.conf` parsing ([Resolve] section with DNS, FallbackDNS, Domains, LLMNR, MulticastDNS, DNSSEC, DNSOverTLS, Cache, DNSStubListener, DNSStubListenerExtra, ReadEtcHosts, ResolveUnicastSingleLabel, CacheFromLocalhost) and drop-in directory support, stub DNS listener on 127.0.0.53:53 (UDP + TCP), DNS query forwarding to upstream servers (UDP with TCP fallback on truncation, per-server retry with exponential backoff), DNS wire format parsing (RFC 1035 headers, question sections, domain name compression with loop detection), `/run/systemd/resolve/stub-resolv.conf` management (points to 127.0.0.53), `/run/systemd/resolve/resolv.conf` management (lists upstream servers), per-link DNS configuration from networkd state files (`/run/systemd/netif/links/`), periodic link DNS refresh, atomic file writes for resolv.conf, sd_notify protocol (READY/WATCHDOG/STATUS/STOPPING), signal handling (SIGTERM/SIGINT for shutdown, SIGHUP for reload/cache flush), query statistics tracking, multi-threaded listener architecture; `resolvectl` CLI with `status [LINK...]` (global and per-link DNS status, resolv.conf mode detection, search domains), `query HOSTNAME...` (hostname resolution via system resolver with DNS stub fallback, A + AAAA queries), `statistics` (resolver statistics), `flush-caches` (sends SIGHUP to resolved), `reset-statistics`, `dns` (show/set per-link DNS), `domain` (show/set per-link domains), `llmnr`/`mdns`/`dnssec`/`dnsovertls` (show per-link settings), legacy `systemd-resolve` interface when invoked as `systemd-resolve`; `networkctl persistent-storage` subcommand for NixOS `systemd-networkd-persistent-storage.service` compatibility; NixOS integration prepared (disabled pending PID 1 unit alias handling fix); missing: DNS-over-TLS, DNSSEC validation, mDNS responder/resolver, LLMNR responder/resolver, DNS cache, split DNS, EDNS0 client subnet, D-Bus interface (`org.freedesktop.resolve1`), `resolvectl` monitor/revert, negative trust anchors
- ✅ **`timesyncd`** — SNTP time synchronization daemon with NTP v4 client, `timesyncd.conf` parsing with drop-in directories, clock adjustment (slew via `adjtimex()` for small offsets, step via `clock_settime()` for large), clock state persistence, sd_notify protocol, signal handling, exponential backoff polling, container detection, graceful degradation; `timedatectl` CLI with `status`, `show`, `set-time`, `set-timezone`, `set-ntp`, `list-timezones`, `timesync-status`; missing: NTS support, D-Bus interface (`org.freedesktop.timesync1`)
- ✅ **`timedated`** — time and date management daemon managing timezone (`/etc/localtime` symlink + `/etc/timezone`), RTC local/UTC mode (`/etc/adjtime`), NTP enable/disable (controls `systemd-timesyncd.service`); timezone detection from multiple sources, timezone validation with path traversal protection, timezone listing with IANA filtering; control socket at `/run/systemd/timedated.sock`; **D-Bus interface (`org.freedesktop.timedate1`)** with properties (Timezone, LocalRTC, CanNTP, NTP, NTPSynchronized, TimeUSec, RTCTimeUSec) and methods (SetTime, SetTimezone, SetLocalRTC, SetNTP, ListTimezones); deferred D-Bus registration (connects after READY=1 to avoid blocking early boot); sd_notify protocol, watchdog keepalive, signal handling; NixOS integration enabled (`withTimedated = true`); `timedatectl` queries state directly and communicates via control socket for mutations; 72 unit tests
- ✅ **`hostnamed`** — hostname management daemon with static/pretty/transient hostname support, `/etc/hostname` and `/etc/machine-info` management, DMI chassis auto-detection, control socket, watchdog keepalive; **D-Bus interface (`org.freedesktop.hostname1`)** with properties (Hostname, StaticHostname, PrettyHostname, IconName, Chassis, Deployment, Location, KernelName, KernelRelease, OperatingSystemPrettyName, OperatingSystemCPEName, OperatingSystemHomeURL, HardwareVendor, HardwareModel, HostnameSource) and methods (SetHostname, SetStaticHostname, SetPrettyHostname, SetIconName, SetChassis, SetDeployment, SetLocation, GetProductUUID, Describe); deferred D-Bus registration; NixOS integration enabled (`withHostnamed = true`); `hostnamectl` CLI with `status`, `show`, `hostname`, `set-hostname`, `chassis`, `deployment`, `location`, `icon-name` commands; 65 unit tests
- ✅ **`localed`** — locale and keyboard layout management daemon with `/etc/locale.conf`, `/etc/vconsole.conf`, and X11 keyboard configuration management, keymap/layout listing, control socket, watchdog keepalive; **D-Bus interface (`org.freedesktop.locale1`)** with properties (Locale, X11Layout, X11Model, X11Variant, X11Options, VConsoleKeymap, VConsoleKeymapToggle) and methods (SetLocale, SetVConsoleKeyboard, SetX11Keyboard); deferred D-Bus registration; NixOS integration enabled (`withLocaled = true`); `localectl` CLI with `status`, `show`, `set-locale`, `set-keymap`, `set-x11-keymap`, `list-keymaps`, `list-x11-keymap-*` commands; 49 unit tests; missing: automatic keymap-to-X11 conversion

## Phase 4 — Extended Services

Higher-level management capabilities:

- ✅ **`machined`** — VM and container registration/tracking daemon with machine registry (register/terminate/GC), machine class (VM/container), state tracking (opening/running/closing), runtime state files in `/run/systemd/machines/`, control socket at `/run/systemd/machined-control`, stale machine cleanup (leader PID liveness check), sd_notify protocol (READY/WATCHDOG/STATUS/STOPPING), signal handling (SIGTERM/SIGINT for shutdown, SIGHUP for reload), periodic GC of dead machines; `machinectl` CLI with `list` (registered machines table with class/service/state), `status` (detailed machine info with name/class/service/scope/leader/root/state/since/netif), `show` (key=value properties with `-p`/`--property` filtering and `--value` output), `terminate`/`poweroff`/`reboot` (unregister + SIGTERM leader), `kill` (send signal to leader with `--signal`/`-s`, numeric or named signals), `clean` (trigger GC), `list-images` (enumerate `/var/lib/machines/`), `login`/`shell` (stubs); offline fallback reads state files directly when daemon is unavailable; 123 unit tests covering machine class/state parsing and display, state file roundtrips (with/without netif, VM/container classes, minimal fields, missing/invalid fields), machine format (status/show output), name validation (valid names, .host special name, length limits, invalid chars), registry operations (register/terminate/get/find-by-leader/duplicate/empty-name/invalid-name), persistence (save/load/save-one, empty/nonexistent dirs, dotfile skipping, invalid file skipping), GC (keeps alive PIDs, removes dead, leader-zero), format_list (empty/with-machines), control commands (PING/LIST/STATUS/SHOW/REGISTER/TERMINATE/GC, case insensitivity, error cases), env content parsing, timestamp formatting, argument parsing (all commands/flags/options), signal parsing (numeric/named/case-insensitive/unknown-fallback); missing: D-Bus interface (`org.freedesktop.machine1`), image management (clone/rename/remove/set-limit), machine scoping (transient scope units), copy-to/copy-from, PTY forwarding for login/shell, OS image import/export/pull
- ❌ **`nspawn`** — lightweight container runtime with user namespaces, network namespaces, OCI bundle support, `--boot` for init-in-container, `--bind` mounts, seccomp profiles, capability bounding
- ✅ **`portabled`** — portable service image management daemon with image discovery from standard search paths (`/var/lib/portables/`, `/etc/portables/`, `/usr/lib/portables/`, `/run/portables/`), image inspection (enumerate unit files, read os-release), attach/detach operations with symlink management in `/etc/systemd/system.attached/` (persistent) or `/run/systemd/system.attached/` (runtime), profile-based drop-in generation for security hardening, reattach (atomic detach + attach), attachment state tracking via marker files in `/run/systemd/portabled/`, runtime vs persistent attachment modes, control socket at `/run/systemd/portabled-control`, sd_notify protocol (READY/WATCHDOG/STATUS/STOPPING), signal handling (SIGTERM/SIGINT for shutdown, SIGHUP for reload), periodic GC of stale attachments; `portablectl` CLI with `list` (available images table with name/type/state/OS/path), `attach IMAGE [PROFILE]` (symlink units with optional profile drop-ins, `--runtime` for volatile), `detach IMAGE` (remove symlinks and drop-ins), `reattach IMAGE [PROFILE]` (atomic detach + attach), `inspect IMAGE` (show image details, os-release, unit files), `is-attached IMAGE` (check attachment state with exit code), `read-only` (stub), `set-limit` (stub); offline fallback reads state files and discovers images directly when daemon is unavailable; supports `--runtime`, `--no-reload`, `--no-pager`, `--no-legend`, `--no-ask-password`, `-q`/`--quiet`, `-H`/`--host`, `-M`/`--machine`, `--json` flags; NixOS integration enabled (`withPortabled = true`); 150 unit tests (90 portabled + 60 portablectl) covering image type/attach state parsing and display, os-release reading (comments, blanks, quoted values, etc/usr fallback), unit file discovery (service/socket/target/timer/path types, deduplication, sorting), image format (status/show output), attachment info state file roundtrips (with/without profile, runtime flag, empty units, missing fields, comments), image registry discovery (directory/raw images, hidden file skipping, priority ordering, nonexistent dirs, multiple images), attachment persistence (save/load/single, empty/nonexistent dirs, dotfile/invalid skipping, file removal), attach/detach operations (basic, runtime, already-attached, not-found, raw-not-supported, no-units, multiple-units, marker drop-ins), GC (removes stale, keeps live, empty), image list formatting, inspect (directory/raw/not-found), profile resolution (dir-style, file-style, not-found, listing), control commands (PING, LIST, INSPECT, ATTACH, DETACH, REATTACH, IS-ATTACHED, SHOW, STATUS, GC, RELOAD, case insensitivity, missing args, unknown commands), attach/detach/reattach cycles, helper functions (format_bytes, timestamps, days_to_ymd); portablectl parse_command tests (all commands, all flags, missing args, unknown commands, flag stripping, host/machine/json flag skipping), offline discovery (directory/raw/hidden/multiple/priority/nonexistent), offline unit discovery, offline os-release reading, offline attach state, offline inspect, command format verification; missing: D-Bus interface (`org.freedesktop.portable1`), raw disk image support (loopback mount, GPT dissection), extension images (`--extension`), image size limit management, automatic daemon-reload after attach/detach, read-only flag toggling
- ✅ **`homed`** — home directory management daemon with JSON-based user records (`/var/lib/systemd/home/*.identity`), storage backends (directory fully implemented, subvolume stub, luks/cifs/fscrypt detected), user lifecycle operations (create/remove/activate/deactivate/lock/unlock/update/passwd/resize), UID allocation (60001–60513 range), home area GC, control socket at `/run/systemd/homed-control`, sd_notify protocol (READY/WATCHDOG/STATUS/STOPPING), signal handling (SIGTERM/SIGINT for shutdown, SIGHUP for reload), periodic GC; `homectl` CLI with `list`, `inspect`, `show`, `create` (with `--real-name`/`--shell`/`--storage`/`--password`/`--home-dir`/`--image-path`), `remove`, `activate`, `deactivate`, `update`, `passwd`, `resize` (K/M/G/T suffixes), `lock`, `unlock`, `lock-all`, `deactivate-all`, `with USER [-- CMD...]` (activate→run→deactivate); offline fallback reads identity files directly; NixOS integration enabled (`withHomed = true`); 178 unit tests (108 homed + 70 homectl); missing: D-Bus interface (`org.freedesktop.home1`), LUKS2 encrypted images, CIFS/fscrypt/btrfs backends, PKCS#11/FIDO2 authentication, password quality enforcement, automatic activation on login, recovery keys
- ✅ **`oomd`** — userspace OOM killer with PSI-based memory pressure monitoring, `oomd.conf` parsing, managed cgroup discovery from unit files, swap usage monitoring, `oomctl` CLI with `dump` command
- ✅ **`coredump`** — core dump handler with `coredump.conf` parsing ([Coredump] section with Storage, Compress, ProcessSizeMax, ExternalSizeMax, MaxUse, KeepFree), drop-in directory support, kernel pipe handler (`/proc/sys/kernel/core_pattern` protocol with PID/UID/GID/SIGNAL/TIMESTAMP/RLIMIT/HOSTNAME/COMM/EXE arguments and `--backtrace` flag), core dump storage in `/var/lib/systemd/coredump/` with descriptive filenames (`core.COMM.UID.BOOT_ID.PID.TIMESTAMP`), JSON metadata sidecar files, size limit enforcement (ProcessSizeMax, ExternalSizeMax), automatic vacuum of old core dumps (MaxUse, KeepFree with statvfs-based free space detection), boot ID and machine ID collection, signal name mapping; `coredumpctl` CLI with `list` (tabular display with TIME/PID/UID/GID/SIG/COREFILE/EXE columns, `--lines`/`-n` limit, `--reverse`, `--since`/`--until` time filters, `--no-legend`), `info` (detailed per-dump display with username/group resolution from `/etc/passwd`/`/etc/group`), `dump` (binary output to file via `-o` or stdout with TTY safety check), `debug`/`gdb` (launch debugger with `--debugger`/`--debugger-arguments` options), match patterns (PID number, COMM name prefix, EXE path prefix); 82 unit tests covering config parsing (all Storage modes, Compress, size directives, infinity, drop-in override, case-insensitive sections, comments, missing files), size parsing (bytes/K/M/G/T/infinity), bool parsing, JSON roundtrips (basic, special characters, control chars, unescape, invalid input), metadata (signal names, filename generation, special char sanitization), argument parsing (basic, --backtrace, no exe, missing args, invalid PID), storage (basic store+read, exceeds external max, directory creation, empty data), vacuum (removes oldest, empty dir, nonexistent dir), listing (empty, with entries, skips non-core files), discovery+filter integration (by comm, PID, exe path, time range), timestamp formatting (epoch, known date, leap year), date math (days_to_ymd, is_leap_year); missing: compression (lz4/zstd/xz), journal integration, `/proc/PID/` metadata enrichment (cmdline, cgroup, environ)
- ❌ **`cryptsetup`** / **`veritysetup`** / **`integritysetup`** — device mapper setup utilities
- ❌ **`repart`** — declarative GPT partition manager
- ✅ **`sysext`** — system extension image overlay management with `status` (show merge state and active extensions), `list` (enumerate available extensions from `/run/extensions/`, `/var/lib/extensions/`, `/usr/lib/extensions/`, `/usr/local/lib/extensions/`), `merge` (overlayfs-based merging of extension hierarchies `/usr/` and `/opt/`), `unmerge` (tear down overlayfs mounts), `refresh` (unmerge + merge), `check-inhibit` (check for inhibitor), JSON output modes (short/pretty), extension release file parsing with host compatibility checking (ID, VERSION_ID, SYSEXT_LEVEL, ARCHITECTURE, SYSEXT_SCOPE), hierarchy detection, merge marker tracking, `--root`/`--force`/`--no-reload`/`--json` options; 80 unit tests
- ✅ **`dissect`** — disk image inspection tool with GPT and MBR partition table parsing, `show` (detailed image info with partition types, GUIDs, sizes, attributes), `list` (partition listing), `discover` (scan image search paths), `validate` (check partition table integrity), `mount`/`umount` (loopback mount/unmount), `copy-from`/`copy-to` (stubs), JSON output modes (short/pretty), known GPT partition type database (root/home/srv/swap/ESP/XBOOTLDR for x86-64/ARM64/etc.), CRC32 header validation, UTF-16LE partition name parsing, human-readable size formatting, `--root-hash`/`--verity-data`/`--no-legend` options; 100+ unit tests
- ✅ **`firstboot`** — initial system configuration wizard with `--locale`, `--keymap`, `--timezone`, `--hostname`, `--machine-id`, `--root-password`/`--root-password-hashed`, `--root-shell`, `--kernel-cmdline` settings; `--prompt`/`--prompt-*` interactive modes; `--copy-*` to copy host settings; `--reset-*` to clear settings; `--root` for chroot operation; `--force` to overwrite existing; `--delete-root-password` to unlock root; `--setup-machine-id` alias; `--welcome` banner; credential loading from `$CREDENTIALS_DIRECTORY`; system-already-booted detection; locale/keymap/timezone/shell enumeration; SHA-512 password hashing; proper `/etc/passwd` and `/etc/shadow` manipulation; 100+ unit tests
- ✅ **`creds`** — credential encryption/decryption tool with `list` (enumerate credentials with size/security state), `cat` (show credential contents with transcode options), `setup` (generate host encryption key), `encrypt` (AES-256-GCM with host key or null key, Base64 output, `--pretty` for unit file embedding), `decrypt` (with name validation, expiry checking, `--allow-null`), `has-tpm2` (TPM2 device detection); custom wire format compatible with systemd's credential header; runtime decryption integrated into exec helper for `LoadCredentialEncrypted=`/`SetCredentialEncrypted=`; missing: TPM2 sealing, host+tpm2 combined mode
- ✅ **`inhibit`** — inhibitor lock tool with acquire/release/list, block/delay modes, stale lock cleanup

## Phase 5 — Utilities, Boot & Polish

Remaining components and production readiness:

- ✅ **`analyze`** — boot performance analysis with `blame`, `time`, `critical-chain`, `dot`, `calendar`, `timespan`, `timestamp`, `verify`, `condition`, `unit-paths`, `security`, `log-level`, `log-target`, `service-watchdogs` subcommands; missing: `plot` (SVG), `inspect-elf`, `fdstore`, `image-policy`, `pcrs`, `srk`
- ✅ **`cgls`** / ✅ **`cgtop`** — cgroup tree listing with process display; real-time cgroup resource monitor with CPU/memory/I/O tracking, sorting, batch mode
- ✅ **`mount`** / **`umount`** — transient mount/automount unit creation, mount table listing, filesystem mount/unmount with force/lazy options
- ✅ **`socket-activate`** — socket activation testing tool; creates TCP/UDP/Unix listening sockets, passes FDs via sd_listen_fds(3) protocol (`LISTEN_FDS`/`LISTEN_PID`/`LISTEN_FDNAMES`), `--accept` mode for per-connection spawning, `--datagram` for UDP, `--recv-buffer`/`--backlog`/`--foreground` options
- ✅ **`ac-power`** — AC power state detection
- ✅ **`detect-virt`** — virtualization/container detection
- ❌ **`sd-boot`** / **`bootctl`** — UEFI boot manager and control tool (this component is EFI, likely stays as a separate build target or FFI)
- ❌ **`sd-stub`** — UEFI stub for unified kernel images
- ✅ **Generator framework** — fstab and getty generators built natively into `libsystemd`; external generator execution framework discovers and runs all standard generators (`systemd-gpt-auto-generator`, `systemd-cryptsetup-generator`, `systemd-debug-generator`, `systemd-run-generator`, etc.) from well-known directories plus package-relative paths; output directories inserted at correct unit search path priorities; built-in generators automatically skipped; per-generator timeout with graceful failure handling
- 🔶 **Comprehensive test suite** — 4,593 unit tests passing; integration tests via nixos-rs boot test; missing: differential testing against real systemd
- ❌ **Documentation** — man-page-compatible documentation for all binaries and configuration formats
- 🔶 **NixOS / distro integration** — packaging via `default.nix`, boot testing via `test-boot.sh`, NixOS module via `systemd.nix`; working end-to-end; udev rules override ensures correct `systemctl` path in udev `RUN+=` actions; `Type=idle` deferral eliminates getty/PAM race conditions; on-demand unit loading enables `systemctl restart` for units outside the boot dependency graph (e.g. udev-triggered `systemd-vconsole-setup.service`); symlink-aware unit discovery handles NixOS `/etc/systemd/system/` layouts; poison-recovering lock infrastructure prevents panic cascades from poisoned `Mutex`/`RwLock` guards; external generator framework discovers generators in NixOS store paths via executable-relative search (15 generators execute successfully during boot); networkd integration enabled (`withNetworkd = true`) with `.network` file for DHCP on ethernet interfaces; resolved integration enabled (`services.resolved.enable = true`) with stub DNS on 127.0.0.53 and fallback DNS servers; portabled integration enabled (`withPortabled = true`) for portable service image management; timedated integration enabled (`withTimedated = true`) for time/date management with D-Bus; machined integration enabled (`withMachined = true`) for VM/container registration; homed integration enabled (`withHomed = true`) for home directory management; `networkctl persistent-storage` subcommand added for NixOS `systemd-networkd-persistent-storage.service` compatibility; deadlock-free PID table (`Arc<Mutex<PidTable>>`) allows signal handler to update entries without RuntimeInfo read lock, preventing 3-way RwLock deadlock with activation threads and control handler; deferred D-Bus registration pattern for hostnamed/timedated/localed — daemons send READY=1 immediately and connect to D-Bus in the first main-loop iteration, avoiding blocking early boot when dbus-daemon isn't ready yet; cloud-hypervisor VM boots with full networking (TAP device + dnsmasq DHCP) in ~7 seconds

## Integration Testing with nixos-rs

The [nixos-rs](../nixos-rs) project provides a minimal NixOS configuration that boots with `systemd-rs` as PID 1 inside a [cloud-hypervisor](https://github.com/cloud-hypervisor/cloud-hypervisor) VM. This is the primary way to validate changes end-to-end against a real Linux boot.

### How it works

1. `systemd-rs` is built as a Nix package via [`default.nix`](../systemd-rs/default.nix)
2. `systemd-rs-systemd` wraps it as a drop-in for the real systemd package — copying data/config from upstream systemd, then overlaying the `systemd-rs` binaries on top, so NixOS modules work unmodified
3. `nixos-rs` defines a minimal NixOS configuration (`nixos-rs`) that sets `systemd.package = pkgs.systemd-rs-systemd` and also replaces bash (with [brush](https://github.com/reubeno/brush)) and coreutils (with [uutils](https://github.com/uutils/coreutils))
4. A raw disk image is built with `nixos-rebuild build-image`, then booted via cloud-hypervisor with the NixOS kernel and initrd, serial console on `ttyS0`
5. [`test-boot.sh`](../nixos-rs/test-boot.sh) automates this: it launches the VM, captures serial output to a log file, monitors for success patterns (login prompt, "Reached target") and failure patterns (kernel panic, Rust panics, emergency shell), and exits with a pass/fail status

### Running boot tests

From the `nixos-rs/` directory:

```sh
# Interactive boot (serial on terminal)
just run

# Automated boot test with streaming output
just test

# Automated test with custom timeout
just test-timeout 180

# Save boot log to a file
just test-log /tmp/boot.log

# Quiet mode (pass/fail only, no streaming)
just test-quiet

# Boot test, keep VM running after success for debugging
just test-keep
```

### Workflow for testing systemd-rs changes

> **⚠️ Important:** Nix flakes only see files tracked by git. When adding new crates or files, you **must** `git add` them before running `just build` or `just test`, otherwise the Nix build will fail with "No such file or directory" errors. This applies to new `crates/*/` directories, `Cargo.toml`, `Cargo.lock`, `default.nix`, and any other new files.

1. Make changes to `systemd-rs` source code
2. If you added new files or crates, run `git add` on them (e.g. `git add crates/newcomponent/ Cargo.toml Cargo.lock default.nix`)
3. Run `just test` from `nixos-rs/` — this rebuilds the Nix package (picking up your source changes), rebuilds the NixOS image, boots it in cloud-hypervisor, and reports pass/fail with full boot output
4. On failure, inspect the captured serial log for the exact point where boot diverged — kernel messages, systemd-rs unit startup output, and any panics or errors are all captured
5. Use `just test-keep` to leave the VM running after a successful boot so you can log in and inspect the running system

### What the boot test validates

- `systemd-rs` starts as PID 1 and processes the initrd → root filesystem transition
- Unit file parsing works for the NixOS-generated unit files
- Dependency ordering brings up the system in the correct sequence
- Socket activation, target synchronization, and service lifecycle work
- The system reaches `multi-user.target` and presents a login prompt
- No Rust panics or unexpected crashes occur during boot