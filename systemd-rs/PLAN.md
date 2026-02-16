# Implementation Plan

This document describes the phased plan for rewriting systemd as a pure Rust drop-in replacement.

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

- **Unit file parser** — complete INI-style parser with all systemd extensions (line continuation, quoting rules, specifier expansion `%i`, `%n`, `%N`, `%p`, `%u`, `%U`, `%h`, `%s`, `%m`, `%b`, `%H`, `%v`, `%t`, etc.)
- **Dependency graph engine** — topological sort with cycle detection, transaction model for atomic start/stop operations
- **D-Bus protocol** — wire format implementation (no C `libdbus` dependency), bus connection management, signal matching, property change notifications
- **sd_notify protocol** — full notify socket implementation with credential passing and fd store
- **Journal binary format** — reader/writer for the systemd journal binary log format with field hashing and entry sealing
- **Specifier expansion** — complete `%`-specifier table as documented in `systemd.unit(5)`
- **Unit name handling** — escaping, unescaping, template instantiation, unit type detection
- **Configuration parsing** — `/etc/systemd/system.conf`, `/etc/systemd/user.conf`, and environment generators
- **Credential management** — `LoadCredential=`, `SetCredential=`, `ImportCredential=`, encrypted credentials

## Phase 1 — Core System (PID 1 + systemctl + journald)

The minimum viable system to boot a real Linux machine:

- **`systemd` (PID 1)** — complete service manager with all unit types, default target handling, emergency/rescue mode, generators, `systemd-run`, transient units, reexecution support, `SIGRTMIN+` signals, and all documented manager D-Bus interface methods
- **`systemctl`** — full CLI including `start`, `stop`, `restart`, `reload`, `enable`, `disable`, `mask`, `unmask`, `daemon-reload`, `daemon-reexec`, `status`, `show`, `cat`, `edit`,
  `list-units`, `list-unit-files`, `list-dependencies`, `list-sockets`, `list-timers`, `list-jobs`, `is-active`, `is-enabled`, `is-failed`, `isolate`, `kill`, `set-property`, `revert`,
  `poweroff`, `reboot`, `suspend`, `hibernate`
- **`journald`** — journal logging daemon with `/dev/log` socket, `/run/systemd/journal/socket`, `/run/systemd/journal/stdout`, native protocol, syslog protocol, kernel `kmsg`, rate limiting, field size limits, journal file rotation, disk usage limits, forward-secure sealing, wall message forwarding
- **`journalctl`** — journal query tool with time-based filtering, unit filtering, boot filtering, priority filtering, output formats (`short`, `short-iso`, `verbose`, `json`, `cat`, `export`), cursor support, follow mode, field listing
- **`systemd-shutdown`** — clean shutdown/reboot with filesystem unmount, loop device detach, DM detach, MD RAID stop
- **`systemd-sleep`** — suspend/hibernate/hybrid-sleep handling
- **`systemd-notify`** — CLI tool for sending notifications
- **`systemd-run`** — transient unit creation
- **`systemd-escape`** — unit name escaping utility
- **`systemd-path`** — runtime path query utility
- **`systemd-id128`** — 128-bit ID operations
- **`systemd-delta`** — unit file override inspection
- **`systemd-cat`** — connect stdout/stderr to journal

## Phase 2 — Essential System Services

Services required for a fully functional desktop or server:

- **`udevd`** — device manager with `.rules` file parser, `udev` database, netlink event monitor, property matching, `RUN` execution, device node permissions, `udevadm` CLI (`info`, `trigger`, `settle`, `monitor`, `test`, `control`)
- **`tmpfiles`** — create/delete/clean temporary files and directories per `tmpfiles.d` configuration
- **`sysusers`** — create system users and groups per `sysusers.d` configuration
- **`logind`** — login/seat/session tracking, multi-seat support, inhibitor locks, idle detection, power key handling, VT switching, `loginctl` CLI
- **`modules-load`** — load kernel modules from `modules-load.d` configuration
- **`sysctl`** — apply sysctl settings from `sysctl.d` configuration
- **`binfmt`** — register binary formats via `binfmt_misc` from `binfmt.d` configuration
- **`vconsole-setup`** — virtual console font and keymap configuration
- **`backlight`** / **`rfkill`** — save and restore hardware state across reboots
- **`ask-password`** / **`tty-ask-password-agent`** — password query framework for LUKS, etc.

## Phase 3 — Network Stack

Full network management:

- **`networkd`** — network configuration daemon with `.network`, `.netdev`, `.link` file parsing, DHCP v4/v6 client, DHCPv6-PD, IPv6 RA, static routes, routing policy rules, bridge/bond/VLAN/VXLAN/WireGuard/tunnel/MACsec creation, `networkctl` CLI
- **`resolved`** — stub DNS resolver with DNS-over-TLS, DNSSEC validation, mDNS responder/resolver, LLMNR responder/resolver, per-link DNS configuration, split DNS, `/etc/resolv.conf` management, `resolvectl` CLI
- **`timesyncd`** — SNTP client with NTS support, `timedatectl` CLI
- **`hostnamed`** — hostname management daemon, `hostnamectl` CLI
- **`localed`** — locale and keymap management daemon, `localectl` CLI

## Phase 4 — Extended Services

Higher-level management capabilities:

- **`machined`** — VM and container registration/tracking, `machinectl` CLI
- **`nspawn`** — lightweight container runtime with user namespaces, network namespaces, OCI bundle support, `--boot` for init-in-container, `--bind` mounts, seccomp profiles, capability bounding
- **`portabled`** — portable service image management (attach/detach/inspect), `portablectl` CLI
- **`homed`** — user home directory management with LUKS encryption, `homectl` CLI
- **`oomd`** — userspace OOM killer with cgroup-based memory pressure monitoring, `oomctl` CLI
- **`coredump`** — core dump handler with journal integration, `coredumpctl` CLI
- **`cryptsetup`** / **`veritysetup`** / **`integritysetup`** — device mapper setup utilities
- **`repart`** — declarative GPT partition manager
- **`sysext`** — system extension image overlay management
- **`dissect`** — disk image inspection tool
- **`firstboot`** — initial system configuration wizard
- **`creds`** — credential encryption/decryption tool
- **`inhibit`** — inhibitor lock tool

## Phase 5 — Utilities, Boot & Polish

Remaining components and production readiness:

- **`analyze`** — boot performance analysis (`blame`, `critical-chain`, `plot`, `dot`, `calendar`, `timespan`, `timestamp`, `verify`, `security`, `inspect-elf`, `fdstore`, `image-policy`, `pcrs`, `srk`, `log-level`, `log-target`, `service-watchdogs`, `condition`)
- **`cgls`** / **`cgtop`** — cgroup tree listing and real-time resource monitor
- **`mount`** / **`umount`** — mount unit creation and removal
- **`ac-power`** — AC power state detection
- **`sd-boot`** / **`bootctl`** — UEFI boot manager and control tool (this component is EFI, likely stays as a separate build target or FFI)
- **`sd-stub`** — UEFI stub for unified kernel images
- **Generator framework** — `systemd-fstab-generator`, `systemd-gpt-auto-generator`, `systemd-cryptsetup-generator`, `systemd-getty-generator`, `systemd-debug-generator`, etc.
- **Comprehensive test suite** — unit tests, integration tests against the systemd test suite, differential testing against real systemd
- **Documentation** — man-page-compatible documentation for all binaries and configuration formats
- **NixOS / distro integration** — packaging, boot testing, NixOS module

## Integration Testing with nixos-rs

The [nixos-rs](../nixos-rs) project provides a minimal NixOS configuration that boots with `systemd-rs` as PID 1 inside a [cloud-hypervisor](https://github.com/cloud-hypervisor/cloud-hypervisor) VM. This is the primary way to validate changes end-to-end against a real Linux boot.

### How it works

1. `systemd-rs` is built as a Nix package via [`default.nix`](../systemd-rs/default.nix)
2. `systemd-rs-systemd` wraps it as a drop-in for the real systemd package — copying data/config from upstream systemd, then overlaying the `systemd-rs` binaries on top, so NixOS modules work unmodified
3. `nixos-rs` defines a minimal NixOS configuration (`oxidized-nixos`) that sets `systemd.package = pkgs.systemd-rs-systemd` and also replaces bash (with [brush](https://github.com/reubeno/brush)) and coreutils (with [uutils](https://github.com/uutils/coreutils))
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
