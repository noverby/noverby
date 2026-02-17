# Implementation Plan

This document describes the phased plan for rewriting systemd as a pure Rust drop-in replacement.

## Current Status

**🟢 NixOS boots successfully with systemd-rs as PID 1** — The system reaches `multi-user.target`, presents a login prompt, and auto-logs in within ~4 seconds in a cloud-hypervisor VM.

### What works today

- 2,428 unit tests passing, boot test passing in ~4 seconds
- PID 1 initialization with full NixOS compatibility (VFS mounts, `/etc/mtab` symlink, cgroup2, machine-id, hostname, home directories, PAM/NSS diagnostics)
- Unit file parsing for all NixOS-generated unit files (service, socket, target, mount, timer, path, slice, scope)
- Dependency graph resolution and parallel unit activation
- Mount unit activation with fstab generator (replaces `systemd-fstab-generator`)
- Getty generator (replaces `systemd-getty-generator`)
- Socket activation and `sd_notify` protocol
- Journal logging (systemd-journald starts and collects logs)
- NTP time synchronization (systemd-timesyncd starts and syncs clock)
- Clean shutdown with filesystem unmount
- 28 crates implemented across Phases 0–4

### Recent changes

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
├── tmpfiles/            # Temporary file manager (systemd-tmpfiles)
├── sysusers/            # Declarative system user manager (systemd-sysusers)
├── hostnamed/           # Hostname manager daemon (systemd-hostnamed)
├── hostnamectl/         # Hostname control tool
├── localed/             # Locale manager daemon (systemd-localed)
├── localectl/           # Locale control tool
├── machined/            # VM/container manager daemon (systemd-machined)
├── machinectl/          # Machine manager control tool
├── nspawn/              # Container runtime (systemd-nspawn)
├── portabled/           # Portable service manager (systemd-portabled)
├── portablectl/         # Portable service control tool
├── homed/               # Home directory manager (systemd-homed)
├── homectl/             # Home directory control tool
├── oomd/                # Userspace OOM killer (systemd-oomd)
├── oomctl/              # OOM killer control tool
├── timesyncd/           # NTP time synchronization (systemd-timesyncd)
├── timedatectl/         # Time/date control tool
├── coredump/            # Core dump handler (systemd-coredump)
├── coredumpctl/         # Core dump query tool
├── analyze/             # Boot performance analyzer (systemd-analyze)
├── run/                 # Transient unit runner (systemd-run)
├── cgls/                # Cgroup listing tool (systemd-cgls)
├── cgtop/               # Cgroup resource monitor (systemd-cgtop)
├── cat/                 # Unit file viewer (systemd-cat)
├── delta/               # Unit file override viewer (systemd-delta)
├── detect-virt/         # Virtualization detector (systemd-detect-virt)
├── escape/              # Unit name escaping tool (systemd-escape)
├── id128/               # 128-bit ID tool (systemd-id128)
├── mount/               # Mount/unmount utilities (systemd-mount, systemd-umount)
├── notify/              # Notification sender (systemd-notify)
├── path/                # Path operation tool (systemd-path)
├── socket-activate/     # Socket activation tool (systemd-socket-activate)
├── ask-password/        # Password query tool (systemd-ask-password)
├── tty-ask-password-agent/ # Password agent (systemd-tty-ask-password-agent)
├── inhibit/             # Inhibitor lock tool (systemd-inhibit)
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

- ✅ **`systemd` (PID 1)** — service manager with all core unit types (service, socket, target, mount, timer, path, slice, scope) and all service types (`simple`, `exec`, `notify`, `notify-reload`, `oneshot`, `forking`, `dbus`, `idle`), default target handling, parallel activation, fstab generator, getty generator, NixOS early boot setup, full `Condition*`/`Assert*` directive support (15 check types); missing: emergency/rescue mode, external generators, transient units, reexecution, `SIGRTMIN+` signals
- ✅ **`systemctl`** — CLI including `start`, `stop`, `restart`, `enable`, `disable`, `status`, `list-units`, `list-unit-files`, `is-active`, `is-enabled`, `poweroff`, `reboot`; missing: `daemon-reload`, `daemon-reexec`, `edit`, `set-property`, `revert`, `suspend`, `hibernate`
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
- ✅ **`modules-load`** — load kernel modules from `modules-load.d` configuration
- ✅ **`sysctl`** — apply sysctl settings from `sysctl.d` configuration
- ✅ **`binfmt`** — register binary formats via `binfmt_misc` from `binfmt.d` configuration
- ✅ **`vconsole-setup`** — virtual console font and keymap configuration
- ✅ **`backlight`** / ✅ **`rfkill`** — save and restore hardware state across reboots
- ❌ **`ask-password`** / ❌ **`tty-ask-password-agent`** — password query framework for LUKS, etc.

## Phase 3 — Network Stack

Full network management:

- ❌ **`networkd`** — network configuration daemon with `.network`, `.netdev`, `.link` file parsing, DHCP v4/v6 client, DHCPv6-PD, IPv6 RA, static routes, routing policy rules, bridge/bond/VLAN/VXLAN/WireGuard/tunnel/MACsec creation, `networkctl` CLI
- ❌ **`resolved`** — stub DNS resolver with DNS-over-TLS, DNSSEC validation, mDNS responder/resolver, LLMNR responder/resolver, per-link DNS configuration, split DNS, `/etc/resolv.conf` management, `resolvectl` CLI
- ✅ **`timesyncd`** — SNTP time synchronization daemon with NTP v4 client, `timesyncd.conf` parsing with drop-in directories, clock adjustment (slew via `adjtimex()` for small offsets, step via `clock_settime()` for large), clock state persistence, sd_notify protocol, signal handling, exponential backoff polling, container detection, graceful degradation; `timedatectl` CLI with `status`, `show`, `set-time`, `set-timezone`, `set-ntp`, `list-timezones`, `timesync-status`; missing: NTS support, D-Bus interface (`org.freedesktop.timesync1`), `systemd-timedated` D-Bus daemon (`org.freedesktop.timedate1`)
- ❌ **`hostnamed`** — hostname management daemon, `hostnamectl` CLI
- ❌ **`localed`** — locale and keymap management daemon, `localectl` CLI

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
- ❌ **`inhibit`** — inhibitor lock tool

## Phase 5 — Utilities, Boot & Polish

Remaining components and production readiness:

- ❌ **`analyze`** — boot performance analysis (`blame`, `critical-chain`, `plot`, `dot`, `calendar`, `timespan`, `timestamp`, `verify`, `security`, `inspect-elf`, `fdstore`, `image-policy`, `pcrs`, `srk`, `log-level`, `log-target`, `service-watchdogs`, `condition`)
- ❌ **`cgls`** / **`cgtop`** — cgroup tree listing and real-time resource monitor
- ❌ **`mount`** / **`umount`** — mount unit creation and removal
- ✅ **`ac-power`** — AC power state detection
- ✅ **`detect-virt`** — virtualization/container detection
- ❌ **`sd-boot`** / **`bootctl`** — UEFI boot manager and control tool (this component is EFI, likely stays as a separate build target or FFI)
- ❌ **`sd-stub`** — UEFI stub for unified kernel images
- 🔶 **Generator framework** — fstab and getty generators built into `libsystemd`; missing: `systemd-gpt-auto-generator`, `systemd-cryptsetup-generator`, `systemd-debug-generator`, external generator execution
- 🔶 **Comprehensive test suite** — unit tests exist (~2,300+); integration tests via nixos-rs boot test; missing: differential testing against real systemd
- ❌ **Documentation** — man-page-compatible documentation for all binaries and configuration formats
- 🔶 **NixOS / distro integration** — packaging via `default.nix`, boot testing via `test-boot.sh`, NixOS module via `systemd.nix`; working end-to-end

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