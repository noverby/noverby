# Project Structure

The project is organized as a Cargo workspace with a shared core library and individual crates for each systemd component (69 crates):

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
├── networkd/            # Network configuration manager (systemd-networkd) 🔶 + D-Bus
├── networkctl/          # Network manager control tool 🔶
├── resolved/            # DNS stub resolver (systemd-resolved) 🔶 + D-Bus
├── resolvectl/          # Resolver control tool 🔶
├── timesyncd/           # NTP time synchronization (systemd-timesyncd) ✅ + D-Bus
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
├── machined/            # VM/container manager daemon (systemd-machined) ✅ + D-Bus
├── machinectl/          # Machine manager control tool ✅
├── homed/               # Home directory manager (systemd-homed) ✅ + D-Bus
├── homectl/             # Home directory control tool ✅
├── nspawn/              # Container runtime (systemd-nspawn) 🔶
├── portabled/           # Portable service manager (systemd-portabled) ✅ + D-Bus
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
├── cryptsetup/          # LUKS/dm-crypt setup (systemd-cryptsetup) ✅
├── veritysetup/         # dm-verity setup (systemd-veritysetup) ✅
├── integritysetup/      # dm-integrity setup (systemd-integritysetup) ✅
├── boot/                # sd-boot and bootctl (UEFI boot manager)
├── stub/                # sd-stub (UEFI stub)
├── shutdown/            # System shutdown/reboot (systemd-shutdown)
├── sleep/               # Suspend/hibernate handler (systemd-sleep)
├── ac-power/            # AC power detection (systemd-ac-power)
└── generator/           # Generator framework for auto-generating units
```
