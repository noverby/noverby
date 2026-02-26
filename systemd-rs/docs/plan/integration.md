# Integration Testing with nixos-rs

The [nixos-rs](../../../nixos-rs) project provides a minimal NixOS configuration that boots with `systemd-rs` as PID 1 inside a [cloud-hypervisor](https://github.com/cloud-hypervisor/cloud-hypervisor) VM. This is the primary way to validate changes end-to-end against a real Linux boot.

## How it works

1. `systemd-rs` is built as a Nix package via [`default.nix`](../../default.nix)
2. `systemd-rs-systemd` wraps it as a drop-in for the real systemd package — copying data/config from upstream systemd, then overlaying the `systemd-rs` binaries on top, so NixOS modules work unmodified
3. `nixos-rs` defines a minimal NixOS configuration (`nixos-rs`) that sets `systemd.package = pkgs.systemd-rs-systemd` and also replaces bash (with [brush](https://github.com/reubeno/brush)) and coreutils (with [uutils](https://github.com/uutils/coreutils))
4. A raw disk image is built with `nixos-rebuild build-image`, then booted via cloud-hypervisor with the NixOS kernel and initrd, serial console on `ttyS0`
5. [`test-boot.sh`](../../../nixos-rs/test-boot.sh) automates this: it launches the VM, captures serial output to a log file, monitors for success patterns (login prompt, "Reached target") and failure patterns (kernel panic, Rust panics, emergency shell), and exits with a pass/fail status

## Running boot tests

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

## Workflow for testing systemd-rs changes

> **⚠️ Important:** Nix flakes only see files tracked by git. When adding new crates or files, you **must** `git add` them before running `just build` or `just test`, otherwise the Nix build will fail with "No such file or directory" errors. This applies to new `crates/*/` directories, `Cargo.toml`, `Cargo.lock`, `default.nix`, and any other new files.

1. Make changes to `systemd-rs` source code
2. If you added new files or crates, run `git add` on them (e.g. `git add crates/newcomponent/ Cargo.toml Cargo.lock default.nix`)
3. Run `just test` from `nixos-rs/` — this rebuilds the Nix package (picking up your source changes), rebuilds the NixOS image, boots it in cloud-hypervisor, and reports pass/fail with full boot output
4. On failure, inspect the captured serial log for the exact point where boot diverged — kernel messages, systemd-rs unit startup output, and any panics or errors are all captured
5. Use `just test-keep` to leave the VM running after a successful boot so you can log in and inspect the running system

## What the boot test validates

- `systemd-rs` starts as PID 1 and processes the initrd → root filesystem transition
- Unit file parsing works for the NixOS-generated unit files
- Dependency ordering brings up the system in the correct sequence
- Socket activation, target synchronization, and service lifecycle work
- The system reaches `multi-user.target` and presents a login prompt
- No Rust panics or unexpected crashes occur during boot