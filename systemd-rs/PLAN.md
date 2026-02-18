# Implementation Plan

This document describes the phased plan for rewriting systemd as a pure Rust drop-in replacement.

## Current Status

**🟢 NixOS boots successfully with systemd-rs as PID 1** — The system reaches `multi-user.target`, presents a login prompt, and auto-logs in within ~4 seconds in a cloud-hypervisor VM.

### What works today

- 2,901 unit tests passing, boot test passing in ~3 seconds with clean login and zero errors
- PID 1 initialization with full NixOS compatibility (VFS mounts, `/etc/mtab` symlink, cgroup2, machine-id, hostname, home directories, PAM/NSS diagnostics)
- Unit file parsing for all NixOS-generated unit files (service, socket, target, mount, timer, path, slice, scope)
- Dependency graph resolution and parallel unit activation
- Mount unit activation with fstab generator (replaces `systemd-fstab-generator`)
- Getty generator (replaces `systemd-getty-generator`)
- Socket activation and `sd_notify` protocol
- Journal logging (systemd-journald starts and collects logs)
- NTP time synchronization (systemd-timesyncd starts and syncs clock)
- User session management (systemd-user-sessions permits/denies logins)
- Random seed persistence (systemd-random-seed loads/saves kernel entropy)
- Update-done stamps (systemd-update-done marks /etc and /var as updated)
- Pstore archival (systemd-pstore archives kernel crash logs)
- Machine ID setup (systemd-machine-id-setup initializes/commits machine-id)
- Hostname management (systemd-hostnamed manages static/pretty/transient hostnames, hostnamectl CLI)
- Locale/keymap management (systemd-localed manages locale and keyboard config, localectl CLI)
- Boot performance analysis (systemd-analyze with blame, time, critical-chain, calendar, timespan, timestamp, verify, condition, dot, security)
- Cgroup hierarchy listing (systemd-cgls) and real-time resource monitoring (systemd-cgtop)
- Inhibitor lock management (systemd-inhibit acquires/lists/releases locks)
- Mount/unmount operations (systemd-mount/systemd-umount with transient unit creation, mount table listing)
- Password query framework (systemd-ask-password TTY/agent protocol, systemd-tty-ask-password-agent with query/watch/wall/list modes)
- Socket activation testing (systemd-socket-activate with TCP/UDP/Unix sockets, sd_listen_fds protocol, per-connection spawning)
- Clean shutdown with filesystem unmount
- 45 crates implemented across Phases 0–5

### Recent changes

- Implemented on-demand unit loading for `systemctl restart`/`start`/`reload-or-restart` — when a command references a unit not in the boot dependency graph (e.g. `systemd-vconsole-setup.service` triggered by a udev rule), PID 1 now searches the unit file paths, parses the unit file, and inserts it into the unit table with lenient dependency wiring (missing deps silently ignored); this enables udev `RUN+=` actions like `systemctl --no-block restart systemd-vconsole-setup.service` to succeed; `find_or_load_unit()` first checks the in-memory table under a read lock, then falls back to disk search under a write lock; `insert_new_unit_lenient()` wires bidirectional dependency relations to existing units without requiring all referenced dependencies to be present
- Fixed symlink-aware unit file discovery — `find_new_unit_path()` now uses `symlink_metadata()` instead of `entry.metadata()` (which follows symlinks) to correctly handle NixOS's `/etc/systemd/system/` layout where unit files are symlinks into the Nix store; the old code could fail on complex symlink chains because `DirEntry::metadata()` follows the symlink and may not return a usable result for multi-hop NixOS store symlinks; the fix explicitly checks `is_symlink()` and matches on the entry name directly
- Removed noisy fork-child debug output — the `write_to_stderr("Prepare fork child before execing!")` and `write_to_stderr("Exec the exec helper")` calls in `after_fork_child()` were removed; these were debug messages written directly to stderr between `fork()` and `exec()`, producing two lines of noise for every service started during boot; the boot log is now clean with only meaningful service output
- Implemented proper `Type=idle` service behavior — idle services (like `serial-getty@ttyS0.service` and `autovt@tty1.service`) are now deferred until all other active jobs have been dispatched, matching real systemd behavior per systemd.service(5); activation is split into Phase 1 (all non-idle units in parallel via dependency graph) and Phase 2 (idle services started after Phase 1 completes); this eliminates the PAM "Authentication service cannot retrieve authentication info" error that occurred when getty services raced with `suid-sgid-wrappers.service`; the `is_idle_service()` helper checks `ServiceType::Idle` on each unit in the activation subgraph
- Fixed udev rules path issue for systemd-rs overlay — the C udevd binary has the original systemd store path compiled in for its built-in rules directory, causing `RUN+=` actions (like `90-vconsole.rules`) to invoke the wrong `systemctl`; solved by creating a `udevRulesOverride` package containing only rules files that reference `systemctl`, added to `services.udev.packages` in the NixOS config so they end up in `/etc/udev/rules.d/` which takes priority over the compiled-in path; the udevd now correctly calls systemd-rs's `systemctl` instead of the original systemd's
- Implemented `systemd-ask-password` — password query tool supporting direct TTY input (with echo suppression via termios, backspace handling, Ctrl-C/Ctrl-D cancellation), agent protocol via question files in `/run/systemd/ask-password/` (INI-format `[Ask]` section with PID, Socket, Message, Icon, Id, NotAfter, AcceptCached, Echo fields), Unix socket reply protocol (`+password` for success, `-` for cancel), credential lookup via `$CREDENTIALS_DIRECTORY`, `--no-tty`/`--no-agent`/`--multiple` mode selection, `--timeout`, `--echo`, `--accept-cached`, `--credential`, `--keyname`, `--id`, `--icon` options
- Implemented `systemd-tty-ask-password-agent` — TTY-based password agent that monitors `/run/systemd/ask-password/` for question files; `--query` mode processes all pending questions once; `--watch` mode continuously polls for new questions (500ms interval); `--wall` mode broadcasts password requests to all TTYs via `/dev/pts/*` and `/dev/tty*`; `--list` mode displays pending questions with metadata; `--console` for custom TTY path; parses INI question files with expiry checking via `CLOCK_MONOTONIC`; sends responses through Unix datagram sockets
- Implemented `systemd-socket-activate` — socket activation testing/debugging tool; creates TCP, UDP, or Unix domain listening sockets from `-l` address specs (port numbers, host:port pairs, absolute paths); passes socket FDs starting at FD 3 with `LISTEN_FDS`/`LISTEN_PID`/`LISTEN_FDNAMES` environment variables per sd_listen_fds(3) protocol; `--accept` mode accepts connections and spawns per-connection child processes; `--datagram` for UDP sockets; `--recv-buffer`, `--backlog`, `--foreground`, `--fdnames` options; proper FD conflict resolution during dup2 shuffling; Unix socket cleanup on exit
- Enabled NixOS integration for `systemd-hostnamed` and `systemd-localed` — set `withHostnamed = true` and `withLocaled = true` in the systemd-rs-systemd packaging so NixOS generates proper unit files (`systemd-hostnamed.service`, `systemd-hostnamed.socket`, `systemd-localed.service`, `dbus-org.freedesktop.hostname1.service`, `dbus-org.freedesktop.locale1.service`); both daemons are deployed at `bin/` and `lib/systemd/` paths; NixOS boot test passes with services registered for on-demand activation
- Added watchdog support to `systemd-hostnamed` and `systemd-localed` — parse `WATCHDOG_USEC` from environment, send `WATCHDOG=1` at half the configured interval in the main loop; prevents PID 1 from killing the daemons when NixOS unit files specify `WatchdogSec=3min`
- Implemented `systemd-analyze` — boot performance analysis and debugging tool with `time` (overall boot timing), `blame` (units sorted by startup duration), `critical-chain` (time-critical unit chain), `calendar` (normalize calendar time specs), `timespan` (normalize time span specs), `timestamp` (normalize timestamp specs), `verify` (validate unit files for correctness), `condition` (evaluate Condition*/Assert* expressions), `dot` (generate dependency graph in dot format), `unit-paths` (list unit file search paths), `log-level`/`log-target` (get/set manager log settings), `service-watchdogs` (get/set watchdog state), `security` (audit unit security hardening) subcommands
- Implemented `systemd-cgls` — recursively display cgroup2 hierarchy as an indented tree with process listings; supports `--all` (show empty cgroups), `--kernel-threads`, `--depth` limit, `--full` output, specific cgroup path arguments
- Implemented `systemd-cgtop` — real-time cgroup resource monitor showing CPU percentage, memory usage, and I/O bytes per cgroup; supports sorting by CPU/memory/IO/tasks/path, batch mode, depth limit, configurable refresh interval and iteration count
- Implemented `systemd-inhibit` — inhibitor lock management tool; acquires shutdown/sleep/idle/handle-* locks while running a command, supports `--list` to show active locks, `--mode block|delay`, automatic stale lock cleanup; lock files stored in `/run/systemd/inhibit/`
- Implemented `systemd-mount` / `systemd-umount` — transient mount/automount unit creation; `--list` shows active mount table from `/proc/self/mountinfo`; supports `--type`, `--options`, `--read-only`, `--mkdir`, `--automount`, `--timeout-idle-sec`, `--property`, `--force`/`--lazy` unmount; generates proper unit names with path escaping
- Implemented `systemd-hostnamed` — hostname management daemon managing static hostname (`/etc/hostname`), pretty hostname, and transient (kernel) hostname; reads/writes `/etc/machine-info` for chassis, deployment, location, icon name, hardware vendor/model; auto-detects chassis type from DMI SMBIOS data; reads OS info from `/etc/os-release`; provides control socket for runtime queries/updates; `hostnamectl` CLI with `status`, `show`, `hostname`, `set-hostname`, `icon-name`, `chassis`, `deployment`, `location` commands; supports `--transient`, `--static`, `--pretty` flags and `-p`/`--property` filtering
- Implemented `systemd-localed` — locale and keyboard layout management daemon managing system locale (`/etc/locale.conf`), virtual console keymap (`/etc/vconsole.conf`), and X11 keyboard layout (`/etc/X11/xorg.conf.d/00-keyboard.conf`); supports all 15 standard locale variables (LANG, LANGUAGE, LC_*); provides control socket for runtime changes; `localectl` CLI with `status`, `show`, `set-locale`, `set-keymap`, `set-x11-keymap`, `list-keymaps`, `list-x11-keymap-layouts`, `list-x11-keymap-models`, `list-x11-keymap-variants`, `list-x11-keymap-options` commands
- Enhanced `systemctl` — added support for common flags (`--no-block`, `--quiet`, `--force`, `--no-pager`, `--no-ask-password`, `--system`, `-a`, `-q`, `-f`, `-l`, `-t`, `-p`, etc.); flags are stripped before sending commands to PID 1; added command aliases (`poweroff`/`reboot`/`halt` → `shutdown`, `daemon-reload` → `reload`, `condrestart`/`force-reload` → `try-restart`); added proper exit code handling for `is-active` (0=active, 3=inactive), `is-enabled`, `is-failed`; suppresses empty output for commands like `try-restart` (fixes `resolvconf.service` printing `[]` to stdout)
- Added `try-restart`, `reload-or-restart`, `is-active`, `is-enabled`, `is-failed` methods to PID 1 control handler — `try-restart` restarts only if unit is active (silently succeeds otherwise), `is-active` returns unit state (`active`/`activating`/`deactivating`/`inactive`/`failed`), `is-enabled` checks if unit is loaded, `is-failed` checks for error state; also added `daemon-reload`/`daemon-reexec` as aliases for `reload`; fixes `resolvconf.service` "Unknown method: try-restart" error during boot
- Implemented `systemd-user-sessions` — manages `/run/nologin` to permit/deny user logins; fixes the `autovt@tty1.service` ERROR where the getty failed because `systemd-user-sessions.service` had not reached the expected state; `start` removes `/run/nologin`, `stop` creates it with "System is going down." message
- Implemented `systemd-update-done` — creates/updates `/etc/.updated` and `/var/.updated` stamp files used by `ConditionNeedsUpdate=` directives; compares modification times against `/usr/` to determine if stamps need refreshing
- Implemented `systemd-random-seed` — loads/saves the kernel random seed across reboots via `/var/lib/systemd/random-seed` (512 bytes); `load` credits saved seed to `/dev/urandom` and uses `RNDADDENTROPY` ioctl, then immediately refreshes the seed so it's never reused; `save` writes fresh random data for next boot
- Implemented `systemd-pstore` — archives platform-specific persistent storage entries from `/sys/fs/pstore/` into timestamped subdirectories under `/var/lib/systemd/pstore/`; parses `pstore.conf` with `[PStore]` section (`Storage=external|journal|none`, `Unlink=yes|no`), supports drop-in directories
- Implemented `systemd-machine-id-setup` — initializes or commits `/etc/machine-id`; supports `--commit` (unmounts transient bind mount and writes persistently), `--print`, `--root=PATH`; tries `/var/lib/dbus/machine-id` and `/sys/class/dmi/id/product_uuid` before generating a random ID
- Implemented `systemd-timesyncd` — SNTP time synchronization daemon with NTP v4 client, `timesyncd.conf` parsing (including drop-in directories), clock adjustment via `adjtimex()`/`clock_settime()` (slew for small offsets, step for large), clock state persistence in `/var/lib/systemd/timesync/clock`, sd_notify READY/WATCHDOG/STATUS protocol, signal handling (SIGTERM/SIGINT for shutdown, SIGHUP for reload), exponential backoff polling, container detection, graceful degradation when no network is available; `timedatectl` CLI with `status`, `show`, `set-time`, `set-timezone`, `set-ntp`, `list-timezones`, and `timesync-status` commands
- Implemented `systemd-oomd` — userspace OOM killer with PSI-based memory pressure monitoring, cgroup v2 support, `oomd.conf` parsing, managed cgroup discovery from unit files, swap usage monitoring, `oomctl` CLI with `dump` command; re-enabled `systemd.oomd` in nixos-rs config
- Added `Assert*` directive support (`AssertPathExists=`, `AssertPathIsDirectory=`, `AssertVirtualization=`, etc.) — like `Condition*` but causes unit failure instead of silent skip
- Added `Type=exec` service type support (like `Type=simple` but verifies the `exec()` call succeeded before marking the service as started)
- Refactored condition/assertion parsing into shared helper `parse_condition_or_assert_entries()`, eliminating code duplication
- Added `/etc/mtab → ../proc/self/mounts` symlink creation (fixes "failed to update userspace mount table" warnings)
- Added essential VFS mount safety nets (`/proc`, `/sys`, `/dev`, `/dev/shm`, `/dev/pts`, `/run`) in PID 1 early setup
- Added fstab generator for NixOS mount unit dependencies
- Added getty generator for serial console login
- Added NixOS boot test infrastructure (`test-boot.sh`)
- Fixed PAM "Authentication service cannot retrieve authentication info" error via proper `/run/wrappers` mount ordering

## Project Structure

The project is organized as a Cargo workspace with a shared core library and individual crates for each systemd component:

```text
crates/
├── libsystemd/          # Core library: unit parsing, dependency graph, sd-bus protocol,
│                        # sd_notify, journal format, specifier expansion, etc.
├── systemd/             # PID 1 service manager (init system)
├── systemctl/           # CLI control tool for the service manager
├── journald/            # Journal logging daemon (systemd-journald)
├── journalctl/          # Journal query tool
├── udevd/               # Device manager (systemd-udevd)
├── udevadm/             # udev administration tool
├── logind/              # Login and session manager (systemd-logind)
├── loginctl/            # Login manager control tool
├── networkd/            # Network configuration manager (systemd-networkd)
├── networkctl/          # Network manager control tool
├── resolved/            # DNS stub resolver (systemd-resolved)
├── resolvectl/          # Resolver control tool
├── timesyncd/           # NTP time synchronization (systemd-timesyncd)
├── timedatectl/         # Time/date control tool
├── user-sessions/       # User session gate (systemd-user-sessions)
├── update-done/         # Update completion marker (systemd-update-done)
├── random-seed/         # Random seed persistence (systemd-random-seed)
├── pstore/              # Persistent storage archival (systemd-pstore)
├── machine-id-setup/    # Machine ID initialization (systemd-machine-id-setup)
├── tmpfiles/            # Temporary file manager (systemd-tmpfiles)
├── sysusers/            # Declarative system user manager (systemd-sysusers)
├── hostnamed/           # Hostname manager daemon (systemd-hostnamed) ✅
├── hostnamectl/         # Hostname control tool ✅
├── localed/             # Locale manager daemon (systemd-localed) ✅
├── localectl/           # Locale control tool ✅
├── machined/            # VM/container manager daemon (systemd-machined)
├── machinectl/          # Machine manager control tool
├── nspawn/              # Container runtime (systemd-nspawn)
├── portabled/           # Portable service manager (systemd-portabled)
├── portablectl/         # Portable service control tool
├── ask-password/        # Password query tool (systemd-ask-password) ✅
├── tty-ask-password-agent/ # Password agent (systemd-tty-ask-password-agent) ✅
├── homed/               # Home directory manager (systemd-homed)
├── homectl/             # Home directory control tool
├── oomd/                # Userspace OOM killer (systemd-oomd)
├── oomctl/              # OOM killer control tool
├── coredump/            # Core dump handler (systemd-coredump)
├── coredumpctl/         # Core dump query tool
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
├── dissect/             # Image dissection tool (systemd-dissect)
├── firstboot/           # First-boot configuration (systemd-firstboot)
├── repart/              # Partition manager (systemd-repart)
├── sysext/              # System extension manager (systemd-sysext)
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

Restructure the existing codebase into a Cargo workspace and extract shared functionality into `libsystemd`:

- ✅ **Unit file parser** — complete INI-style parser with all systemd extensions (line continuation, quoting rules, specifier expansion `%i`, `%n`, `%N`, `%p`, `%u`, `%U`, `%h`, `%s`, `%m`, `%b`, `%H`, `%v`, `%t`, etc.)
- ✅ **Dependency graph engine** — topological sort with cycle detection, transaction model for atomic start/stop operations
- 🔶 **D-Bus protocol** — uses C `libdbus` via the `dbus` crate; wire format implementation planned but not yet needed for boot
- ✅ **sd_notify protocol** — full notify socket implementation with credential passing and fd store
- 🔶 **Journal binary format** — reader/writer partially implemented; journald starts and collects logs during boot
- 🔶 **Specifier expansion** — common specifiers (`%i`, `%n`, `%N`, `%p`, `%u`, `%U`, `%h`, `%s`, `%m`, `%b`, `%H`, `%v`, `%t`) implemented; some rare specifiers may be missing
- ✅ **Unit name handling** — escaping, unescaping, template instantiation, unit type detection
- ✅ **Configuration parsing** — `/etc/systemd/system.conf`, `/etc/systemd/user.conf`, and environment generators
- ❌ **Credential management** — `LoadCredential=`, `SetCredential=`, `ImportCredential=`, encrypted credentials

Legend: ✅ = implemented, 🔶 = partial, ❌ = not started

## Phase 1 — Core System (PID 1 + systemctl + journald)

The minimum viable system to boot a real Linux machine:

- ✅ **`systemd` (PID 1)** — service manager with all core unit types (service, socket, target, mount, timer, path, slice, scope) and all service types (`simple`, `exec`, `notify`, `notify-reload`, `oneshot`, `forking`, `dbus`, `idle`), default target handling, parallel activation, fstab generator, getty generator, NixOS early boot setup, full `Condition*`/`Assert*` directive support (15 check types), proper `Type=idle` deferral (idle services wait for all other jobs to complete before starting); missing: emergency/rescue mode, external generators, transient units, reexecution, `SIGRTMIN+` signals
- ✅ **`systemctl`** — CLI including `start`, `stop`, `restart`, `try-restart`, `reload-or-restart`, `enable`, `disable`, `status`, `list-units`, `list-unit-files`, `is-active`, `is-enabled`, `is-failed`, `poweroff`, `reboot`, `daemon-reload`; handles common flags (`--no-block`, `--quiet`, `--force`, `--no-pager`, `--system`, `-a`, `-q`, `-f`, `-l`, `-t`, `-p`); proper exit codes for query commands; missing: `edit`, `set-property`, `revert`, `suspend`, `hibernate`, `show`
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

- ❌ **`udevd`** — device manager with `.rules` file parser, `udev` database, netlink event monitor, property matching, `RUN` execution, device node permissions, `udevadm` CLI (`info`, `trigger`, `settle`, `monitor`, `test`, `control`)
- ✅ **`tmpfiles`** — create/delete/clean temporary files and directories per `tmpfiles.d` configuration
- ✅ **`sysusers`** — create system users and groups per `sysusers.d` configuration
- ❌ **`logind`** — login/seat/session tracking, multi-seat support, inhibitor locks, idle detection, power key handling, VT switching, `loginctl` CLI
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

- ❌ **`networkd`** — network configuration daemon with `.network`, `.netdev`, `.link` file parsing, DHCP v4/v6 client, DHCPv6-PD, IPv6 RA, static routes, routing policy rules, bridge/bond/VLAN/VXLAN/WireGuard/tunnel/MACsec creation, `networkctl` CLI
- ❌ **`resolved`** — stub DNS resolver with DNS-over-TLS, DNSSEC validation, mDNS responder/resolver, LLMNR responder/resolver, per-link DNS configuration, split DNS, `/etc/resolv.conf` management, `resolvectl` CLI
- ✅ **`timesyncd`** — SNTP time synchronization daemon with NTP v4 client, `timesyncd.conf` parsing with drop-in directories, clock adjustment (slew via `adjtimex()` for small offsets, step via `clock_settime()` for large), clock state persistence, sd_notify protocol, signal handling, exponential backoff polling, container detection, graceful degradation; `timedatectl` CLI with `status`, `show`, `set-time`, `set-timezone`, `set-ntp`, `list-timezones`, `timesync-status`; missing: NTS support, D-Bus interface (`org.freedesktop.timesync1`), `systemd-timedated` D-Bus daemon (`org.freedesktop.timedate1`)
- ✅ **`hostnamed`** — hostname management daemon with static/pretty/transient hostname support, `/etc/hostname` and `/etc/machine-info` management, DMI chassis auto-detection, control socket, watchdog keepalive; NixOS integration enabled (`withHostnamed = true`); `hostnamectl` CLI with `status`, `show`, `hostname`, `set-hostname`, `chassis`, `deployment`, `location`, `icon-name` commands; missing: D-Bus interface (`org.freedesktop.hostname1`)
- ✅ **`localed`** — locale and keyboard layout management daemon with `/etc/locale.conf`, `/etc/vconsole.conf`, and X11 keyboard configuration management, keymap/layout listing, control socket, watchdog keepalive; NixOS integration enabled (`withLocaled = true`); `localectl` CLI with `status`, `show`, `set-locale`, `set-keymap`, `set-x11-keymap`, `list-keymaps`, `list-x11-keymap-*` commands; missing: D-Bus interface (`org.freedesktop.locale1`), automatic keymap-to-X11 conversion

## Phase 4 — Extended Services

Higher-level management capabilities:

- ❌ **`machined`** — VM and container registration/tracking, `machinectl` CLI
- ❌ **`nspawn`** — lightweight container runtime with user namespaces, network namespaces, OCI bundle support, `--boot` for init-in-container, `--bind` mounts, seccomp profiles, capability bounding
- ❌ **`portabled`** — portable service image management (attach/detach/inspect), `portablectl` CLI
- ❌ **`homed`** — user home directory management with LUKS encryption, `homectl` CLI
- ✅ **`oomd`** — userspace OOM killer with PSI-based memory pressure monitoring, `oomd.conf` parsing, managed cgroup discovery from unit files, swap usage monitoring, `oomctl` CLI with `dump` command
- ❌ **`coredump`** — core dump handler with journal integration, `coredumpctl` CLI
- ❌ **`cryptsetup`** / **`veritysetup`** / **`integritysetup`** — device mapper setup utilities
- ❌ **`repart`** — declarative GPT partition manager
- ❌ **`sysext`** — system extension image overlay management
- ❌ **`dissect`** — disk image inspection tool
- ❌ **`firstboot`** — initial system configuration wizard
- ❌ **`creds`** — credential encryption/decryption tool
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
- 🔶 **Generator framework** — fstab and getty generators built into `libsystemd`; missing: `systemd-gpt-auto-generator`, `systemd-cryptsetup-generator`, `systemd-debug-generator`, external generator execution
- 🔶 **Comprehensive test suite** — unit tests exist (~2,901); integration tests via nixos-rs boot test; missing: differential testing against real systemd
- ❌ **Documentation** — man-page-compatible documentation for all binaries and configuration formats
- 🔶 **NixOS / distro integration** — packaging via `default.nix`, boot testing via `test-boot.sh`, NixOS module via `systemd.nix`; working end-to-end; udev rules override ensures correct `systemctl` path in udev `RUN+=` actions; `Type=idle` deferral eliminates getty/PAM race conditions; on-demand unit loading enables `systemctl restart` for units outside the boot dependency graph (e.g. udev-triggered `systemd-vconsole-setup.service`); symlink-aware unit discovery handles NixOS `/etc/systemd/system/` layouts; boot log is clean with zero errors

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

1. Make changes to `systemd-rs` source code
2. Run `just test` from `nixos-rs/` — this rebuilds the Nix package (picking up your source changes), rebuilds the NixOS image, boots it in cloud-hypervisor, and reports pass/fail with full boot output
3. On failure, inspect the captured serial log for the exact point where boot diverged — kernel messages, systemd-rs unit startup output, and any panics or errors are all captured
4. Use `just test-keep` to leave the VM running after a successful boot so you can log in and inspect the running system

### What the boot test validates

- `systemd-rs` starts as PID 1 and processes the initrd → root filesystem transition
- Unit file parsing works for the NixOS-generated unit files
- Dependency ordering brings up the system in the correct sequence
- Socket activation, target synchronization, and service lifecycle work
- The system reaches `multi-user.target` and presents a login prompt
- No Rust panics or unexpected crashes occur during boot