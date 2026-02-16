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

## Current Status

The existing codebase provides a working foundation for the PID 1 service manager with:

- Service and socket unit file parsing (subset of directives)
- Dependency-ordered parallel startup
- Socket activation (non-inetd style)
- `sd_notify` protocol (`READY=1`, `STATUS=`, `MAINPID=`, etc.)
- Service types: `simple`, `notify`, `dbus`, `oneshot`
- Target units for synchronization
- cgroup-based process tracking (optional, Linux only)
- A basic JSON-RPC control interface (`rsdctl`)
- Container PID 1 support

See [PLAN.md](PLAN.md) for the full phased implementation plan, and [feature-comparison.md](feature-comparison.md) for a detailed feature-by-feature comparison with upstream systemd.

## Building

```sh
# Build everything
cargo build --release

# Build just the service manager
cargo build --release -p systemd

# Build with optional features
cargo build --release --features dbus_support,cgroups,linux_eventfd
```

## Testing

```sh
# Run all tests
cargo test

# Run library tests
cargo test -p libsystemd

# Run integration tests
cargo test -p systemd --test integration

# Differential testing against system systemd
cargo build --release && ./tests/differential.sh --verbose
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

1. **Unit file parsing** — add support for missing directives (see `feature-comparison.md`)
2. **New unit types** — implement timer, mount, automount, swap, path, scope units
3. **systemctl** — build out the full CLI to replace `rsdctl`
4. **journald** — implement the journal binary format and logging daemon
5. **Test coverage** — port systemd's integration test suite
6. **Documentation** — document behavior differences and compatibility notes

## License

See [LICENSE](LICENSE).
