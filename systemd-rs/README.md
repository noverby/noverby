# systemd-rs

A pure Rust rewrite and drop-in replacement for [systemd](https://github.com/systemd/systemd).

## Overview

`systemd-rs` is a fully compatible, binary-for-binary replacement for the entire systemd suite, written entirely in Rust.
The goal is to produce a set of binaries that can replace every systemd component on a Linux system
with zero configuration changes — existing unit files, configuration, and tooling should work unmodified.

The implementation is modeled after [systemd](https://github.com/systemd/systemd)
(the C reference implementation maintained by Lennart Poettering et al.),
following its architecture for the service manager, unit dependency graph, D-Bus APIs,
journal binary log format, and all other public interfaces.
This is a **full drop-in replacement**, not a reimagining — the same way
[pkg-config-rs](https://github.com/noverby/pkg-config-rs) replaces pkg-config.

## Features

- **Full unit file parsing** with all section types, specifier expansion, drop-in overrides, template instantiation, and `.d/` directories
- **All unit types**: service, socket, target, mount, automount, swap, timer, path, slice, scope, device
- **Complete dependency graph** with `Requires=`, `Wants=`, `After=`, `Before=`, `BindsTo=`, `PartOf=`, `Conflicts=`, `Requisite=`, and all ordering/requirement semantics
- **All service types**: `simple`, `exec`, `forking`, `oneshot`, `dbus`, `notify`, `notify-reload`, `idle`
- **Full sd_notify protocol** including `READY=1`, `RELOADING=1`, `STOPPING=1`, `STATUS=`, `ERRNO=`, `MAINPID=`, `WATCHDOG=`, `FDSTORE=`, `FDNAME=`, `BARRIER=`
- **Socket activation** with all socket types (stream, datagram, sequential-packet, FIFO, special, netlink, USB FFS) and inetd-style pass-through
- **Journal logging** with binary log format, structured fields, forward-secure sealing, field indexing, and full `journalctl` query language
- **D-Bus API compatibility** implementing all systemd D-Bus interfaces (`org.freedesktop.systemd1`, `org.freedesktop.login1`, `org.freedesktop.hostname1`, etc.)
- **cgroup v2 resource control** with `MemoryMax=`, `CPUQuota=`, `IOWeight=`, `TasksMax=`, `Delegate=`, and all `systemd.resource-control` directives
- **Execution environment** with all `systemd.exec` options: namespaces, capabilities, seccomp filters, credential passing, `DynamicUser=`, `RootDirectory=`, `PrivateTmp=`, etc.
- **Network configuration** with `.network`, `.netdev`, and `.link` files, DHCP client/server, IPv6 RA, WireGuard, VLAN, bridge, bond, and all networkd features
- **DNS resolution** with DNS-over-TLS, DNSSEC, mDNS, LLMNR, split DNS, per-link configuration
- **Container management** via `systemd-nspawn` with full OCI runtime compatibility
- **Boot performance analysis** with `systemd-analyze` blame, critical-chain, plot, dot, security audit
- **Full `systemctl` interface** with all subcommands, output modes, and `--user` / `--system` scoping

## Design Principles

1. **Drop-in compatible** — every binary, D-Bus interface, socket protocol, file format, and CLI flag must match systemd behavior exactly. Existing unit files, configuration, and tooling must work without modification.
2. **No C dependencies** — pure Rust with direct syscall usage via `nix`/`rustix` where needed. No `libsystemd`, `libdbus`, `libudev`, or other C library dependencies.
3. **Same binary names** — install as `systemd`, `systemctl`, `journalctl`, `systemd-journald`, etc. with identical paths, so package managers and other software find them automatically.
4. **Safe by default** — leverage Rust's type system and ownership model to eliminate the classes of memory safety bugs that have historically affected systemd (CVE-2018-15688, CVE-2019-3842, CVE-2021-33910, etc.).
5. **Incremental adoption** — individual components can be swapped in one at a time. Run the Rust `journald` with the C `systemd` PID 1, or vice versa.

## Project Structure

The project is organized as a Cargo workspace:

```text
crates/
├── libsystemd/     # Core library: unit parsing, dependency graph, sd_notify,
│                   # socket activation, platform abstractions, service lifecycle,
│                   # unit name escaping/unescaping, configuration loading,
│                   # journal entry model and on-disk storage engine
├── systemd/        # PID 1 service manager (init system)
├── systemctl/      # CLI control tool for the service manager
├── journald/       # Journal logging daemon (systemd-journald)
├── journalctl/     # Journal query tool
├── shutdown/       # System shutdown/reboot (systemd-shutdown)
├── sleep/          # Suspend/hibernate handler (systemd-sleep)
├── id128/          # 128-bit ID tool (systemd-id128)
├── escape/         # Unit name escaping tool (systemd-escape)
├── notify/         # Notification sender (systemd-notify)
├── path/           # Runtime path query tool (systemd-path)
├── cat/            # Journal cat tool (systemd-cat)
├── detect-virt/    # Virtualization detector (systemd-detect-virt)
├── delta/          # Unit file override viewer (systemd-delta)
├── run/            # Transient unit runner (systemd-run)
└── ac-power/       # AC power detection (systemd-ac-power)
```

See [PLAN.md](PLAN.md) for the full phased plan to add all remaining systemd components (`udevd`, `logind`, `networkd`, `resolved`, etc.).

## Current Status

**Phase 0 (Foundation)** is complete — the project is structured as a Cargo workspace with a shared `libsystemd` core library.

**Phase 1 (Core System)** is complete. The system successfully boots a NixOS VM as PID 1. All Phase 1 components are implemented:

- **PID 1 service manager** (`systemd`) — unit file parsing, dependency-ordered parallel startup, socket activation, `sd_notify` protocol, service types (`simple`, `notify`, `dbus`, `oneshot`), target/slice units, cgroup tracking, PID 1-specific setup (remounting root, mounting tmpfs/cgroup2, machine-id generation)
- **`systemctl`** — CLI control tool (JSON-RPC based) with `list-units`, `status`, `start`, `stop`, `restart`, `shutdown`
- **`systemd-journald`** — journal logging daemon with native journal protocol socket (`/run/systemd/journal/socket`), BSD syslog socket (`/dev/log`), stdout stream socket (`/run/systemd/journal/stdout`), kernel `kmsg` reader, structured field storage, rate limiting, journal file rotation, disk usage limits, sd_notify `READY=1`, SIGUSR1 flush, SIGUSR2 rotate, wall message forwarding, configurable via `/etc/systemd/journald.conf`
- **`journalctl`** — journal query tool with time-based filtering (`--since`, `--until`), unit filtering (`-u`), boot filtering (`-b`, `--list-boots`), priority filtering (`-p`), identifier filtering (`-t`), grep filtering (`-g`), output formats (`short`, `short-iso`, `short-precise`, `short-monotonic`, `verbose`, `json`, `json-pretty`, `cat`, `export`), cursor support, follow mode (`-f`), reverse output (`-r`), line limiting (`-n`), field listing (`-F`, `-N`), disk usage query (`--disk-usage`), flush/rotate commands, PID/UID/GID filtering, free-form `FIELD=VALUE` match expressions
- **`systemd-shutdown`** — clean shutdown/reboot binary with SIGTERM/SIGKILL all processes, filesystem unmount (reverse mount order with retry and lazy unmount fallback), loop device detach, device-mapper deactivation, MD RAID stop, root remount read-only, final `reboot(2)` syscall for poweroff/reboot/halt/kexec
- **`systemd-sleep`** — suspend/hibernate/hybrid-sleep/suspend-then-hibernate via `/sys/power/state` and `/sys/power/disk`, configuration from `/etc/systemd/sleep.conf` and drop-ins, pre/post sleep hooks (`/usr/lib/systemd/system-sleep/`, `/etc/systemd/system-sleep/`), RTC wake alarm for suspend-then-hibernate, system capability checks
- **`systemd-id128`** — generate/query 128-bit IDs (`new`, `machine-id`, `boot-id`, `invocation-id`, `--uuid`, `--app-specific`)
- **`systemd-escape`** — unit name escaping/unescaping (`--unescape`, `--mangle`, `--path`, `--suffix`, `--template`, `--instance`)
- **`systemd-notify`** — send sd_notify messages (`--ready`, `--reloading`, `--stopping`, `--status`, `--booted`, `--pid`)
- **`systemd-path`** — query well-known system and user runtime paths (all XDG paths, systemd search paths)
- **`systemd-cat`** — connect stdout/stderr to the journal via native protocol (`--identifier`, `--priority`, `--stderr-priority`, `--level-prefix`)
- **`systemd-detect-virt`** — detect VMs and containers via DMI/SMBIOS, CPUID, device-tree, cgroups, container markers (`--vm`, `--container`, `--chroot`, `--private-users`, `--list`)
- **`systemd-delta`** — show overridden, extended, masked, and redirected unit files across search paths (`--type`, `--diff`)
- **`systemd-run`** — run commands as transient units with user/group switching, environment setup, working directory (`--scope`, `--unit`, `--uid`, `--gid`, `--wait`, `--shell`, `--setenv`, `--on-calendar`)
- **`systemd-ac-power`** — detect AC power status via `/sys/class/power_supply/` (`--verbose`, `--check-capacity`, `--low`)
- **`libsystemd` core library** — unit name escaping/unescaping, template instantiation, path escaping, unit name mangling, journal entry data model (structured fields, timestamps, trusted metadata, serialisation to export/JSON formats), journal on-disk storage engine (append-only binary format with file headers, entry frames, multi-file rotation, vacuuming, crash-safe writes)

**Next**: Phase 2 (Essential System Services) — `udevd`, `tmpfiles`, `sysusers`, `logind`, `modules-load`, `sysctl`, `binfmt`, `vconsole-setup`, `backlight`, `rfkill`, `ask-password`.

See [feature-comparison.md](feature-comparison.md) for a detailed feature-by-feature comparison with upstream systemd.

## Building

```sh
# Build the entire workspace (all binaries)
cargo build --release

# Build just the service manager
cargo build --release -p systemd

# Build just the control tool
cargo build --release -p systemctl

# Build the journal daemon and query tool
cargo build --release -p systemd-journald
cargo build --release -p journalctl

# Build shutdown and sleep handlers
cargo build --release -p systemd-shutdown
cargo build --release -p systemd-sleep

# Build individual utilities
cargo build --release -p systemd-id128
cargo build --release -p systemd-escape
cargo build --release -p systemd-notify
cargo build --release -p systemd-path
cargo build --release -p systemd-cat
cargo build --release -p systemd-detect-virt
cargo build --release -p systemd-delta
cargo build --release -p systemd-run
cargo build --release -p systemd-ac-power

# Build with optional features
cargo build --release -p libsystemd --features dbus_support,cgroups,linux_eventfd
```

## Testing

```sh
# Run all tests
cargo test --workspace

# Run library tests only
cargo test -p libsystemd

# Run journal-related tests
cargo test -p libsystemd -- journal
cargo test -p systemd-journald
cargo test -p journalctl

# Run individual component tests
cargo test -p systemd-shutdown
cargo test -p systemd-sleep
```

### Boot Testing with nixos-rs

The [nixos-rs](../nixos-rs) project provides end-to-end boot testing by building a minimal NixOS image with `systemd-rs` as PID 1 and booting it in a [cloud-hypervisor](https://github.com/cloud-hypervisor/cloud-hypervisor) VM. Serial console output is captured so you can see every unit start, every log line, and any panics or failures during the boot process.

From the `nixos-rs/` directory:

```sh
# Interactive boot (serial output on your terminal)
just run

# Automated boot test — streams output and reports pass/fail
just test

# Automated test with a custom timeout (seconds)
just test-timeout 180

# Save the full boot log to a file
just test-log /tmp/boot.log

# Quiet mode (no streaming, just exit code)
just test-quiet

# Boot test, then keep the VM running for interactive debugging
just test-keep
```

See [PLAN.md — Integration Testing](PLAN.md#integration-testing-with-nixos-rs) for details on what the boot test validates and the development workflow.

## Contributing

This is an ambitious project and contributions are very welcome. Good starting points:

1. **Phase 2 services** — implement `udevd`, `tmpfiles`, `sysusers`, `logind`, and other essential system services
2. **Unit file parsing** — add support for missing directives (see `feature-comparison.md`)
3. **New unit types** — implement timer, mount, automount, swap, path, scope units
4. **systemctl** — build out the full CLI with all systemctl subcommands
5. **Test coverage** — port systemd's integration test suite
6. **Documentation** — document behavior differences and compatibility notes

## License

See [LICENSE](LICENSE).
