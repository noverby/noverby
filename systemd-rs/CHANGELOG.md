# Changelog

All notable changes to systemd-rs are documented in this file.

## Unreleased

### Added

- **`systemd-udevd` and `udevadm`** — device manager daemon with netlink uevent monitoring (AF_NETLINK / NETLINK_KOBJECT_UEVENT), full `.rules` file parser (match/assign operators `==`/`!=`/`=`/`+=`/`-=`/`:=`, line continuation, escape sequences, `GOTO`/`LABEL` control flow), property matching (`KERNEL`, `SUBSYSTEM`, `ACTION`, `DRIVER`, `DEVTYPE`, `ATTR{file}`, `ENV{key}`, `RESULT`, `TEST`), parent device traversal (`KERNELS`, `SUBSYSTEMS`, `DRIVERS`, `ATTRS{file}`), assignment actions (`NAME`, `SYMLINK`, `OWNER`, `GROUP`, `MODE`, `ENV{key}`, `TAG`, `RUN{program}`, `RUN{builtin}`, `ATTR{file}`, `SYSCTL{key}`, `OPTIONS`), `IMPORT{program|file|cmdline|builtin|db|parent}`, `PROGRAM` execution with result capture, udev-style substitution expansion (`$kernel`/`%k`, `$number`/`%n`, `$devpath`/`%p`, `$attr{file}`/`%s{file}`, `$env{key}`/`%E{key}`, `$major`/`%M`, `$minor`/`%m`, `$result`/`%c` with index, etc.), glob matching (`*`, `?`, `[...]`, `|` alternatives), device database persistence (`/run/udev/data/`), tag management (`/run/udev/tags/`), symlink creation/removal, device node permissions, sysfs attribute writing, RUN program execution, builtin handlers (path_id, input_id, usb_id, net_id, blkid, kmod), event queue with settle support, control socket, sd_notify protocol, signal handling; `udevadm` CLI with `info` (query by name/path, attribute walk, database export/cleanup), `trigger` (filters for subsystem/attribute/property/tag/sysname, prioritized subsystems, dry-run), `settle` (timeout, exit-if-exists, queue polling), `monitor` (kernel uevent listening with filters), `test`, `control` (reload/ping/stop/start/exit), `test-builtin`, `version`; 114 new unit tests (86 udevd + 28 udevadm)
- **`systemd-homed` and `homectl`** — home directory management daemon and CLI with JSON-based identity records, directory/subvolume storage backends, full user lifecycle (create/remove/activate/deactivate/lock/unlock/update/passwd/resize), UID allocation (60001–60513), home registry with persistence, runtime state, periodic GC, control socket, sd_notify protocol; `homectl` CLI with `list`, `inspect`, `show`, `create`, `remove`, `activate`, `deactivate`, `update`, `passwd`, `resize`, `lock`, `unlock`, `lock-all`, `deactivate-all`, `with`; offline fallback; NixOS integration enabled (`withHomed = true`); 178 new unit tests
- **`systemd-timedated`** — time/date management daemon managing timezone, RTC local/UTC mode, NTP enable/disable; control socket, sd_notify, watchdog keepalive; NixOS integration enabled (`withTimedated = true`); 69 new unit tests
- **`systemd-portabled` and `portablectl`** — portable service image management with discovery, attach/detach, profile drop-ins, state tracking, GC; `portablectl` CLI with `list`, `attach`, `detach`, `reattach`, `inspect`, `is-attached`; offline fallback; NixOS integration enabled (`withPortabled = true`); 150 new unit tests
- **`systemd-machined` and `machinectl`** — VM/container registration daemon with machine registry, class/state tracking, GC, control socket; `machinectl` CLI with `list`, `status`, `show`, `terminate`, `kill`, `clean`, `list-images`; offline fallback; NixOS integration enabled (`withMachined = true`); 123 new unit tests
- **`systemd-coredump` and `coredumpctl`** — core dump handler with `coredump.conf` parsing, kernel pipe protocol, storage with JSON metadata sidecars, vacuum; `coredumpctl` CLI with `list`, `info`, `dump`, `debug`/`gdb`; 82 new unit tests
- **`systemd-logind`** — login/session/seat management daemon with session tracking, seat management, user tracking, inhibitor locks, input device monitoring, control socket; `loginctl` CLI with full command set
- **`systemd-creds`** — credential encryption/decryption tool with AES-256-GCM, host key management, Base64 output, TPM2 detection; 41 new unit tests
- **`systemd-analyze`** — boot performance analysis with `blame`, `time`, `critical-chain`, `calendar`, `timespan`, `timestamp`, `verify`, `condition`, `dot`, `unit-paths`, `log-level`/`log-target`, `service-watchdogs`, `security`
- **`systemd-cgls`** — cgroup hierarchy listing with process display, depth limit, kernel thread filtering
- **`systemd-cgtop`** — real-time cgroup resource monitor with CPU/memory/I/O tracking, sorting, batch mode
- **`systemd-inhibit`** — inhibitor lock management with acquire/release/list, block/delay modes, stale cleanup
- **`systemd-mount` / `systemd-umount`** — transient mount/automount unit creation, mount table listing, force/lazy unmount
- **`systemd-ask-password`** — TTY password query with echo suppression, agent protocol, credential lookup
- **`systemd-tty-ask-password-agent`** — password agent with query/watch/wall/list modes
- **`systemd-socket-activate`** — socket activation testing tool with TCP/UDP/Unix sockets, per-connection spawning
- **`systemd-hostnamed`** and **`systemd-localed`** — NixOS integration enabled (`withHostnamed = true`, `withLocaled = true`); watchdog support added
- **`systemctl suspend`/`hibernate`/`hybrid-sleep`/`suspend-then-hibernate`** — sleep commands forwarded through PID 1 to `systemd-sleep` binary; 7 new unit tests
- **`systemctl disable`/`reset-failed`/`kill`** — `disable` (no-op stub), `reset-failed` (clears failed state), `kill` (signal delivery with `--signal`/`-s`); 16 new unit tests
- **`systemctl list-dependencies`** — dependency tree visualization with box-drawing characters, status markers, `--reverse` flag, cycle detection; 43 new unit tests
- **`systemctl mask`/`unmask`** — create/remove `/dev/null` symlinks in `/etc/systemd/system/`
- **`systemctl show`/`cat`** — `show` returns unit properties in key=value format with `-p`/`--property` and `--value`; `cat` displays unit file source; 20 new unit tests
- **`LoadCredential=`/`SetCredential=`/`LoadCredentialEncrypted=`/`SetCredentialEncrypted=`** — full parsing and runtime credential directory setup with correct priority ordering; 22 new unit tests
- **External generator framework** — discovers and executes standard systemd generators before unit loading; NixOS boot runs 15 generators successfully; 16 new unit tests
- **On-demand unit loading** — `systemctl restart`/`start`/`reload-or-restart` for units outside the boot dependency graph
- **Poison-recovering lock infrastructure** — `MutexExt`/`RwLockExt` traits recover from `PoisonError` instead of cascading panics; applied across 12 source files
- **Type=idle service deferral** — idle services deferred until all other jobs complete, eliminating PAM race conditions
- **Type=exec service support** — verifies `exec()` succeeded before marking service as started
- **Assert\* directives** — `AssertPathExists=`, `AssertPathIsDirectory=`, `AssertVirtualization=`, etc. (causes unit failure instead of silent skip)
- **Full network stack in NixOS boot test** — cloud-hypervisor VM with TAP device, dnsmasq DHCP, Rust networkd + resolved

### Fixed

- **3-way RwLock deadlock in PID table** — extracted `pid_table` to `Arc<Mutex<PidTable>>` so the signal handler can update entries without the RuntimeInfo read lock, breaking a deadlock between activation threads, control handler, and exit handler
- **Socket activation `DependencyError` handling** — unsatisfied `After=` dependencies silently deferred instead of logged as ERROR
- **Symlink-aware unit file discovery** — `symlink_metadata()` instead of `entry.metadata()` for NixOS store symlink chains
- **Udev rules path for systemd-rs overlay** — `udevRulesOverride` package ensures correct `systemctl` path in udev `RUN+=` actions
- **PAM authentication race** — proper `/run/wrappers` mount ordering and Type=idle deferral

### Changed

- **`systemctl` flag handling** — strips `--no-block`, `--quiet`, `--force`, `--no-pager`, `--no-ask-password`, `--system`, `-a`, `-q`, `-f`, `-l`, `-t`, `-p` before sending commands to PID 1
- **`systemctl` command aliases** — `poweroff`/`reboot`/`halt` → `shutdown`, `daemon-reload` → `reload`, `condrestart`/`force-reload` → `try-restart`
- **`try-restart`/`reload-or-restart`/`is-active`/`is-enabled`/`is-failed`** added to PID 1 control handler
- **`/etc/mtab` symlink** — PID 1 creates `/etc/mtab → ../proc/self/mounts` (fixes "failed to update userspace mount table" warnings)
- **VFS mount safety nets** — PID 1 early setup ensures `/proc`, `/sys`, `/dev`, `/dev/shm`, `/dev/pts`, `/run` are mounted

### Removed

- **Noisy fork-child debug output** — removed `write_to_stderr` calls between `fork()` and `exec()` that produced two lines of noise per service start