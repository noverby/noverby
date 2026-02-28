# Phase 6 — Differential Testing Against Real systemd

Systematic behavioral equivalence verification between systemd-rs and upstream systemd. Each test category runs identical inputs through both implementations and asserts equivalent outputs, state transitions, and side effects. Tests execute in NixOS VMs (via nixos-rs) where both real systemd and systemd-rs are available, enabling true apples-to-apples comparison on the same OS configuration.

## Infrastructure

- ✅ **Differential test harness (`difftest`)** — new `crates/difftest` crate providing the core framework; `DiffTest` trait with `run_systemd(&self) -> TestOutput` and `run_systemd_rs(&self) -> TestOutput` methods plus `compare(&self, left: &TestOutput, right: &TestOutput) -> DiffResult`; `TestOutput` enum (structured JSON, raw text, binary blob, exit code, file tree snapshot, D-Bus property map); `DiffResult` with `Identical`, `Equivalent(normalization_notes)`, `Divergent(explanation)` variants; built-in normalizers for timestamps, PIDs, boot IDs, machine IDs, memory addresses, and non-deterministic ordering; snapshot-based comparison with `update_snapshots` mode for approving intentional divergences; JUnit XML and JSON report output; `#[difftest]` proc-macro attribute for test registration; parallel test execution with configurable concurrency
- ❌ **NixOS VM test environment** — dual-VM test infrastructure: one VM boots real systemd, one boots systemd-rs, both from identical NixOS configurations (same unit files, same packages, same kernel); shared test coordination via virtio-vsock or serial console protocol; `DiffTestRunner` orchestrates test execution across both VMs, collects outputs, runs comparison; single-VM mode available where real systemd binaries are present alongside systemd-rs for tool-level comparison; Nix expression (`tests/difftest.nix`) defines the test VM configuration; `just difftest` command in project root
- ✅ **Golden file corpus** — curated collection of unit files, configuration files, and input data covering edge cases; sourced from: upstream systemd test suite (`test/` directory), Fedora/Debian/Arch/NixOS shipped unit files, fuzzer-generated edge cases, manually crafted regression files; organized by category (`units/`, `configs/`, `journal/`, `network/`, `generators/`); version-pinned to specific systemd release (v256) for reproducibility
- ❌ **CI integration** — GitHub Actions workflow running differential tests on every PR; matrix of systemd versions (v254, v255, v256) to catch version-specific regressions; test result summary posted as PR comment with divergence count and links to full report; nightly full-corpus run with extended timeout; failure on any new `Divergent` result (previously-known divergences tracked in `tests/difftest/known-divergences.toml`)

## Unit File Parsing

- ❌ **INI parser equivalence** — feed identical unit files through `systemd-analyze verify` (real) and `libsystemd` parser (ours); compare: parsed section names, key-value pairs, line continuation handling, quoting rules (single/double/no quotes, escape sequences `\n`/`\t`/`\\`/`\"`/`\'`/`\x`/`\u`/`\U`), comment handling (`#` and `;`), trailing whitespace, empty values, duplicate keys (last-wins vs append semantics per directive), `\` at EOF; corpus: 500+ unit files with edge cases
- ❌ **Specifier expansion** — compare `%`-specifier resolution for all 40+ specifiers (`%n`, `%N`, `%p`, `%P`, `%i`, `%I`, `%f`, `%j`, `%J`, `%u`, `%U`, `%h`, `%s`, `%g`, `%G`, `%m`, `%b`, `%H`, `%l`, `%v`, `%a`, `%t`, `%T`, `%V`, `%E`, `%C`, `%S`, `%L`, `%d`, `%o`, `%w`, `%q`, `%M`, `%A`, `%B`, `%W`, `%%`); test with template units (`foo@bar.service`, `foo@.service`), instance names with special characters, nested specifier contexts; compare via `systemd-analyze unit-paths` and custom test harness injecting known runtime values
- ❌ **Directive coverage matrix** — for each of the 429 upstream directives: parse a unit file containing the directive, extract the parsed value via `systemctl show -p <Property>` on both implementations, compare; flag any directive where parsed representation diverges; track partial support (parsed but not enforced at runtime) separately
- ❌ **Drop-in overlay resolution** — compare final effective configuration when drop-in directories are present; test `.d/*.conf` overlay ordering (lexicographic), cross-directory priority (`/etc/` > `/run/` > `/usr/lib/`), empty-value reset semantics, multi-value append directives (`After=`, `Environment=`, `ExecStart=`); verify via `systemctl cat` and `systemctl show` on both
- ❌ **Template instantiation** — compare template resolution for `foo@instance.service` with various instance strings (empty, containing `-`, `\x2d`, `/`, `\x2f`, `@`, multi-byte UTF-8, extremely long); verify specifier expansion inside templates; compare `systemctl show -p Id,Names,Following` output

## Dependency Graph & Ordering

- ❌ **Dependency resolution** — compare `systemctl list-dependencies <unit>` output (forward and `--reverse`) for all unit types; test `Requires=`, `Wants=`, `Requisite=`, `BindsTo=`, `PartOf=`, `Upholds=`, `Conflicts=`, `Before=`, `After=`, `OnFailure=`, `OnSuccess=`, `PropagatesReloadTo=`, `ReloadPropagatedFrom=`, `PropagatesStopTo=`, `StopPropagatedFrom=`, `JoinsNamespaceOf=`; verify ordering constraints are identical
- ❌ **Transaction model** — compare the set of units started/stopped/restarted when issuing `systemctl start/stop/restart <unit>`; capture via `systemctl list-jobs` during transaction execution and D-Bus `JobNew`/`JobRemoved` signals; test conflict resolution, requisite failure propagation, `OnFailure=` chains, `BindsTo=` cascading stop
- ❌ **Cycle detection** — feed dependency graphs with cycles to both implementations; compare error messages, which units are skipped, and final system state; test ordering cycles (`A After B, B After A`) and requirement cycles (`A Requires B, B Requires A`)
- ❌ **Default target expansion** — compare the full set of units pulled in by `multi-user.target`, `graphical.target`, `rescue.target`, `emergency.target` on the same NixOS configuration; verify unit counts match and no units are missing or spurious

## Service Lifecycle

- ❌ **State machine transitions** — for each service type (`simple`, `exec`, `notify`, `notify-reload`, `oneshot`, `forking`, `dbus`, `idle`): start/stop/restart a test service, record state transitions (`inactive` → `activating` → `active` → `deactivating` → `inactive`) with timestamps; compare transition sequences and relative ordering; verify `ActiveState`, `SubState`, `LoadState`, `Result` properties match
- ❌ **ExecStart command execution** — compare process execution semantics: `argv[0]` handling with `@`, environment variable expansion (`${VAR}`, `$VAR`), working directory, umask, PID 1 as parent vs subreaper, supplementary groups, OOM score adjustment; test via instrumented test services that dump their process state to a file
- ❌ **Restart policies** — for each `Restart=` mode (`no`, `always`, `on-success`, `on-failure`, `on-abnormal`, `on-abort`, `on-watchdog`): trigger the corresponding exit condition, verify restart/no-restart decision matches; test `RestartPreventExitStatus=`, `SuccessExitStatus=`, `RestartSec=`, `RestartSteps=`, `RestartMaxDelaySec=`; compare restart count and timing
- ❌ **sd_notify protocol** — test service sends each `sd_notify` message (`READY=1`, `STATUS=...`, `MAINPID=`, `RELOADING=1`, `STOPPING=1`, `WATCHDOG=1`, `WATCHDOG=trigger`, `WATCHDOG_USEC=`, `ERRNO=`, `BUSERROR=`, `EXIT_STATUS=`, `MONOTONIC_USEC=`, `FDSTORE=1`, `FDSTOREREMOVE=1`, `FDNAME=`, `NOTIFYACCESS=`); compare resulting `systemctl show` property values on both implementations; test `NotifyAccess=none/main/exec/all` enforcement
- ❌ **Watchdog enforcement** — service with `WatchdogSec=` that stops pinging; compare: time from last ping to kill signal, signal used (`WatchdogSignal=`), restart behavior, `Result=` property after watchdog kill; test `WATCHDOG_USEC=` dynamic override
- ❌ **ExecStartPre/Post, ExecStop/Post ordering** — compare execution order, failure propagation (`-` prefix), timeout handling, environment inheritance between Pre/Main/Post commands; test `ExecCondition=` (skip service if condition command fails with exit 1-254)
- ❌ **Timeout handling** — compare behavior when `TimeoutStartSec=`, `TimeoutStopSec=`, `TimeoutAbortSec=`, `TimeoutSec=` fire; verify signal escalation (SIGTERM → SIGKILL after `TimeoutStopSec`), `SendSIGKILL=`, `SendSIGHUP=`, `FinalKillSignal=`
- ❌ **Resource control enforcement** — compare `MemoryMax=`, `CPUQuota=`, `TasksMax=`, `IOWeight=` cgroup property application; read back values from `/sys/fs/cgroup/` on both implementations; test `Delegate=`, `DisableControllers=`, `Slice=` hierarchy placement
- ❌ **Credential passing** — compare `ImportCredential=`, `LoadCredential=`, `SetCredential=`, `LoadCredentialEncrypted=`, `SetCredentialEncrypted=` results; verify file contents in `/run/credentials/<unit>/`, permissions, `CREDENTIALS_DIRECTORY` env var; test glob patterns, priority ordering, encrypted credentials with host key

## Exec Environment

- ❌ **Namespace isolation** — compare mount namespace contents under `ProtectSystem=` (`true`/`full`/`strict`), `ProtectHome=` (`true`/`read-only`/`tmpfs`), `PrivateTmp=`, `PrivateDevices=`, `PrivateNetwork=`, `PrivateUsers=`, `PrivateIPC=`, `PrivateMounts=`; test service writes `/proc/self/mountinfo` to a file; diff mount tables
- ❌ **Security hardening** — compare effective capability sets (`CapabilityBoundingSet=`, `AmbientCapabilities=`), seccomp filters (`SystemCallFilter=`, `SystemCallArchitectures=`), `NoNewPrivileges=`, `LockPersonality=`, `MemoryDenyWriteExecute=`, `RestrictNamespaces=`, `RestrictRealtime=`, `RestrictSUIDSGID=`, `RestrictAddressFamilies=`, `ProtectKernelTunables=`, `ProtectKernelModules=`, `ProtectKernelLogs=`, `ProtectControlGroups=`, `ProtectClock=`, `ProtectHostname=`, `ProtectProc=`, `ProcSubset=`; test service introspects own security context and dumps to file
- ❌ **User/group handling** — compare `User=`, `Group=`, `SupplementaryGroups=`, `DynamicUser=` (UID allocation, nss integration, `/run/systemd/dynamic-uid/` tracking); test with numeric and named users, verify effective uid/gid/groups in child process
- ❌ **Environment variables** — compare final environment seen by service process for `Environment=`, `EnvironmentFile=`, `PassEnvironment=`, `UnsetEnvironment=`; test override ordering, quoting, multi-line `EnvironmentFile=` parsing, generator-provided env; `:` prefix clean-environment behavior
- ❌ **File system paths** — compare directory creation for `StateDirectory=`, `CacheDirectory=`, `LogsDirectory=`, `RuntimeDirectory=`, `ConfigurationDirectory=` (ownership, permissions, `*DirectoryMode=`, symlink creation under `/var/lib/`, `/var/cache/`, etc.); test `DynamicUser=yes` interaction
- ❌ **Resource limits** — compare all 15 `Limit*=` directives (`LimitCPU=`, `LimitFSIZE=`, `LimitDATA=`, `LimitSTACK=`, `LimitCORE=`, `LimitRSS=`, `LimitNOFILE=`, `LimitAS=`, `LimitNPROC=`, `LimitMEMLOCK=`, `LimitLOCKS=`, `LimitSIGPENDING=`, `LimitMSGQUEUE=`, `LimitNICE=`, `LimitRTPRIO=`); test `infinity`, soft:hard syntax, default values; verify via `/proc/self/limits` in child

## Socket Activation

- ❌ **Socket unit lifecycle** — compare socket creation for all `Listen*=` types (`ListenStream=`, `ListenDatagram=`, `ListenSequentialPacket=`, `ListenFIFO=`, `ListenNetlink=`, `ListenSpecial=`, `ListenMessageQueue=`, `ListenUSBFunction=`); verify socket options (`SocketMode=`, `DirectoryMode=`, `Backlog=`, `KeepAlive=`, `FreeBind=`, `Transparent=`, `ReusePort=`, `PassCredentials=`, `PassSecurity=`, `ReceiveBuffer=`, `SendBuffer=`, `IPTOS=`, `IPTTL=`, `Mark=`, `PipeSize=`, `Writable=`, `Symlinks=`)
- ❌ **Activation trigger** — connect to a socket-activated service, verify the service is started, receives the correct file descriptors via `LISTEN_FDS`/`LISTEN_PID`/`LISTEN_FDNAMES`; compare for TCP, UDP, Unix stream/datagram, FIFO; test `Accept=yes` (inetd-style per-connection instances) and `Accept=no` (single service)
- ❌ **fd passing semantics** — compare exact fd numbering (starting from fd 3), fd names in `LISTEN_FDNAMES`, multiple socket units activating the same service, `FileDescriptorName=` override; test service restart with `FileDescriptorStoreMax=` and `FDSTORE=1` protocol

## Timer & Path Units

- ❌ **Monotonic timers** — compare next elapse computation for `OnActiveSec=`, `OnBootSec=`, `OnStartupSec=`, `OnUnitActiveSec=`, `OnUnitInactiveSec=`; verify `AccuracySec=`, `RandomizedDelaySec=` (distribution shape comparison over many runs), `Persistent=`, `WakeSystem=`, `RemainAfterElapse=`; compare `systemctl list-timers` output
- ❌ **Calendar timers** — feed identical `OnCalendar=` expressions to `systemd-analyze calendar` (real) and our `CalendarSpec::next_elapse()` (ours); corpus of 500+ expressions including: shorthands (`minutely`/`hourly`/`daily`/`weekly`/`monthly`/`quarterly`/`yearly`/`annually`/`semiannually`), full expressions, weekday ranges (`Mon..Fri`), lists (`1,15`), ranges (`1..5`), repetitions (`*/5`, `10/3`, `1..20/2`), timezone suffixes, microsecond precision, leap year dates, DST transition points, end-of-month (`*-*~1`); compare next N elapses (N=10) from a fixed reference time
- ❌ **Timespan parsing** — compare `systemd-analyze timespan` output for 200+ input strings including compound expressions (`1h 30min 5s`), floating-point (`1.5h`), bare numbers, all unit suffixes, `infinity`, zero, negative (rejected), overflow cases, whitespace variations
- ❌ **Path unit triggering** — create watched paths, trigger filesystem events, verify correct trigger semantics for `PathExists=`, `PathExistsGlob=`, `PathChanged=`, `PathModified=`, `DirectoryNotEmpty=`; compare trigger timing and which events cause activation; test `MakeDirectory=`, `TriggerLimitIntervalSec=`, `TriggerLimitBurst=`

## systemctl CLI

- ❌ **Output format parity** — compare `systemctl` output for all subcommands against real systemctl; test `list-units` (all columns, type/state filtering, `--all`), `list-unit-files`, `list-dependencies` (tree format, `--reverse`), `list-timers`, `list-sockets`, `list-jobs`; normalize PIDs, timestamps, memory values for comparison
- ❌ **show/status property parity** — for every property reported by `systemctl show <unit>`, compare values between implementations; generate a property-by-property equivalence report; properties tested: `Id`, `Names`, `Following`, `Requires`, `Wants`, `After`, `Before`, `Description`, `LoadState`, `ActiveState`, `SubState`, `FragmentPath`, `UnitFileState`, `InactiveExitTimestamp`, `ActiveEnterTimestamp`, `ActiveExitTimestamp`, `InactiveEnterTimestamp`, `MainPID`, `ExecMainStartTimestamp`, `ControlPID`, `StatusText`, `Result`, `ExecMainCode`, `ExecMainStatus`, `Type`, `Restart`, `NotifyAccess`, `WatchdogUSec`, `MemoryCurrent`, `CPUUsageNSec`, `TasksCurrent`, `IPIngressBytes`, `IPEgressBytes`, `NFileDescriptorStore`, `Triggers`, `TriggeredBy`, and all others
- ❌ **Exit code semantics** — compare exit codes for `is-active`, `is-enabled`, `is-failed`, `is-system-running`, `status` (with/without active/inactive/failed units); verify that exit code conventions match systemd's documented behavior (0=active/enabled, 1=inactive/unknown, 3=inactive for `is-active`, etc.)
- ❌ **enable/disable/mask** — compare symlink creation by `enable` (WantedBy=, RequiredBy=, Also= handling, template enablement, instance enablement, alias creation), `disable` (symlink removal), `mask`/`unmask` (`/dev/null` symlink), `preset` (vendor preset application); verify `systemctl is-enabled` returns same state strings (`enabled`, `enabled-runtime`, `linked`, `linked-runtime`, `masked`, `masked-runtime`, `static`, `indirect`, `disabled`, `generated`, `transient`, `alias`, `bad`)
- ❌ **edit/set-property/revert** — compare drop-in file creation by `systemctl edit` (override.conf placement, section inference), `set-property` (50-set-property.conf, runtime vs persistent), `revert` (which files are removed); verify daemon-reload picks up changes identically

## journald / journalctl

- ❌ **Journal write compatibility** — send identical messages via syslog socket, native protocol, stdout/stderr, and `/dev/kmsg`; read back via `journalctl` on both; compare: field names, field values, priority mapping, facility mapping, `_SYSTEMD_UNIT=`, `_PID=`, `_UID=`, `_GID=`, `_COMM=`, `_EXE=`, `_CMDLINE=`, `SYSLOG_IDENTIFIER=`, `SYSLOG_PID=`, `SYSLOG_FACILITY=`; test binary field encoding, multi-line messages, NUL bytes
- ❌ **journalctl query parity** — compare query results for all filter combinations: `--unit`, `--user-unit`, `--identifier`, `--priority` (single and range), `--facility`, `--transport`, `--boot`, `--since`/`--until` (all date formats), `--pid`/`--uid`/`--gid`, `--grep` with `--case-sensitive`, `--dmesg`, free-form `FIELD=VALUE` matches; verify entry count, ordering, and field contents match
- ❌ **Output format parity** — compare `journalctl -o <format>` output for all 15 output formats (`short`, `short-full`, `short-iso`, `short-iso-precise`, `short-precise`, `short-monotonic`, `short-unix`, `with-unit`, `verbose`, `json`, `json-pretty`, `json-sse`, `json-seq`, `cat`, `export`); normalize timestamps and PIDs; verify JSON schema matches
- ❌ **Rate limiting** — send burst of messages exceeding `RateLimitBurst=`; compare: number of messages suppressed, suppression summary message format, per-source (unit/identifier/PID) isolation, window reset behavior
- ❌ **Rotation and vacuuming** — compare file rotation triggers (`SystemMaxFileSize=`, `MaxFileSec=`, SIGUSR2), vacuum behavior (`SystemMaxUse=`, `RuntimeMaxUse=`, `MaxFiles=`, `SystemKeepFree=`, `RuntimeKeepFree=`); verify `journalctl --disk-usage`, `--vacuum-size`, `--vacuum-time`, `--vacuum-files` produce equivalent results

## D-Bus Interfaces

- ❌ **org.freedesktop.systemd1 (Manager)** — compare all Manager properties and method return values: `ListUnits`, `ListUnitFiles`, `GetUnit`, `GetUnitByPID`, `LoadUnit`, `StartUnit`, `StopUnit`, `ReloadUnit`, `RestartUnit`, `ResetFailedUnit`, `ListJobs`, `Subscribe`, `Unsubscribe`, `Reload`, `Reexecute`; compare signal emissions (`UnitNew`, `UnitRemoved`, `JobNew`, `JobRemoved`, `Reloading`, `StartupFinished`) timing and payloads; compare unit object properties (all `org.freedesktop.systemd1.Unit` and type-specific interfaces)
- ❌ **org.freedesktop.hostname1** — compare all properties (`Hostname`, `StaticHostname`, `PrettyHostname`, `IconName`, `Chassis`, `Deployment`, `Location`, `KernelName`, `KernelRelease`, `OperatingSystemPrettyName`, `HardwareVendor`, `HardwareModel`) and method behaviors (`SetHostname`, `SetStaticHostname`, `Describe`)
- ❌ **org.freedesktop.timedate1** — compare properties (`Timezone`, `LocalRTC`, `CanNTP`, `NTP`, `NTPSynchronized`, `TimeUSec`, `RTCTimeUSec`) and methods (`SetTime`, `SetTimezone`, `SetLocalRTC`, `SetNTP`, `ListTimezones`)
- ❌ **org.freedesktop.locale1** — compare properties (`Locale`, `X11Layout`, `X11Model`, `X11Variant`, `X11Options`, `VConsoleKeymap`) and methods (`SetLocale`, `SetVConsoleKeyboard`, `SetX11Keyboard`)
- ❌ **org.freedesktop.network1** — compare `ListLinks` output, per-link properties (`OperationalState`, `CarrierState`, `AddressState`), `Describe` JSON output; test `Reload` and `ForceRenew` side effects
- ❌ **org.freedesktop.resolve1** — compare `DNS`, `FallbackDNS`, `Domains`, `DNSSEC`, `Cache` properties; compare `FlushCaches`/`ResetStatistics` behavior; compare `TransactionStatistics`/`CacheStatistics` schema
- ❌ **org.freedesktop.machine1** — compare `ListMachines`, `ListImages`, image properties, `PoolPath`/`PoolUsage`/`PoolLimit`; test `RegisterMachine`/`TerminateMachine` lifecycle equivalence
- ❌ **org.freedesktop.portable1** — compare `ListImages`, `GetImageState`, `AttachImage`/`DetachImage` side effects (symlinks, drop-ins, daemon-reload)
- ❌ **org.freedesktop.home1** — compare `ListHomes` output, user record JSON schema, `ActivateHome`/`DeactivateHome` state transitions
- ❌ **org.freedesktop.timesync1** — compare `NTPSynchronized`, `ServerName`, `ServerAddress`, `Frequency`, `PollIntervalUSec` properties and `Describe` JSON output
- ❌ **D-Bus introspection** — compare XML introspection output for every bus name and object path; verify interface/method/property/signal names, argument types, annotations, and access modifiers match

## CLI Tool Output Parity

- ❌ **systemd-analyze** — compare output of all subcommands: `blame` (unit ordering), `time` (timing values), `critical-chain` (tree format), `dot` (GraphViz output), `verify` (error messages for invalid units), `calendar` (next elapse, normalized expression), `timespan` (normalized duration), `timestamp` (parsed time), `condition` (evaluation result), `unit-paths` (search path list), `security` (exposure score and per-directive ratings), `inspect-elf` (ELF metadata fields), `image-policy` (normalized policy string), `cat-config` (config file content with drop-in overlay); normalize timestamps and paths
- ❌ **hostnamectl / timedatectl / localectl** — compare `status` and `show` output for all three tools; test `set-*` mutations produce identical file system changes
- ❌ **networkctl** — compare `list` (interface table), `status` (per-link details including addresses, routes, DNS), `lldp` output
- ❌ **resolvectl** — compare `status` (global + per-link DNS), `query` (resolution results), `statistics` (counter schema)
- ❌ **loginctl** — compare `list-sessions`, `list-users`, `list-seats`, `session-status`, `user-status` output
- ❌ **machinectl / portablectl / homectl** — compare `list`, `status`, `show`, `list-images` output and property sets
- ❌ **coredumpctl** — compare `list` and `info` output format, match filter behavior (by PID/COMM/EXE)
- ❌ **bootctl** — compare `status`, `list` (boot entry enumeration), `kernel-identify`, `kernel-inspect` output; compare EFI variable interpretation
- ❌ **systemd-escape** — compare escaping/unescaping for all inputs including empty string, `/`, multi-component paths, special characters, `--template`, `--instance`, `--unescape`, `--mangle`; corpus of 200+ edge-case strings
- ❌ **systemd-id128** — compare `new`, `machine-id`, `boot-id`, `invocation-id` output format; compare `--app-specific` UUID derivation for deterministic generation
- ❌ **systemd-path** — compare output for all well-known path names; verify system vs user mode paths
- ❌ **systemd-delta** — compare override detection and `[EXTENDED]`/`[OVERRIDDEN]`/`[MASKED]`/`[EQUIVALENT]`/`[REDIRECTED]` classification
- ❌ **systemd-detect-virt** — compare detection result (`--vm`, `--container`, `--chroot`, `--private-users`, `--cvm`) in identical environments
- ❌ **systemd-cgls / systemd-cgtop** — compare `cgls` tree output; compare `cgtop` column headings and metric collection (batch mode snapshot)
- ❌ **systemd-run** — compare transient unit creation, property propagation, `--wait` behavior, `--pty` allocation, `--pipe` mode, exit code forwarding
- ❌ **systemd-cat** — compare journal entry creation with `--identifier`, `--level-prefix`, `--stderr-priority` options

## Network Stack

- ❌ **networkd configuration** — compare link state after applying identical `.network` + `.link` + `.netdev` files; verify via `ip addr`, `ip route`, `ip rule`, `ip link`; test DHCP lease acquisition (compare assigned address, routes, DNS), static address assignment, `.netdev` virtual device creation (bridge, bond, vlan, vxlan, veth, dummy, vrf, wireguard), routing policy rules
- ❌ **IPv6 SLAAC** — compare SLAAC-generated addresses (EUI-64 and RFC 7217 stable privacy) for identical MAC address and prefix; compare link-local address; verify address lifetimes
- ❌ **resolved DNS behavior** — compare DNS query results (A, AAAA, CNAME, MX, SRV, TXT, PTR) for identical upstream configurations; test split DNS routing (query routed to correct per-link server based on routing domains); compare `/etc/hosts` lookup behavior; compare DNS cache behavior (TTL, NXDOMAIN caching, cache flush on SIGHUP)
- ❌ **networkd-wait-online** — compare readiness determination for identical network configurations; test `--interface`, `--ignore`, `--any`, `--operational-state` flag behavior
- ❌ **network-generator** — compare generated `.network`/`.netdev`/`.link` files from identical kernel command line `ip=`/`rd.route=`/`nameserver=`/`vlan=`/`bond=`/`bridge=`/`ifname=` parameters

## Generator Framework

- ❌ **Generator output** — run generators with identical input (`/etc/fstab`, kernel command line, `/etc/crypttab`) on both implementations; compare generated unit files in `normal/`, `early/`, `late/` output directories; test `systemd-fstab-generator`, `systemd-gpt-auto-generator`, `systemd-cryptsetup-generator`, `systemd-debug-generator`, `systemd-run-generator`, `systemd-getty-generator`; diff generated unit file contents field by field

## Condition/Assert Evaluation

- ❌ **Condition checks** — compare evaluation results for all 28 condition types in identical environments; test both true and false cases for each; use `systemd-analyze condition` output as reference; special focus on: `ConditionVirtualization=` (VM vs container vs bare metal), `ConditionPathIsMountPoint=` (bind mounts, overlay, NFS), `ConditionKernelCommandLine=` (with and without `=`), `ConditionArchitecture=`, `ConditionFirmware=`, `ConditionSecurity=`, `ConditionOSRelease=` (comparison operators), `ConditionMemory=`/`ConditionCPUs=` (comparison operators and ranges), `ConditionFirstBoot=`, `ConditionUser=`/`ConditionGroup=` (numeric and named), `ConditionControlGroupController=`, `ConditionCapability=`; verify negation (`!` prefix) handling

## udev

- ❌ **Rule evaluation** — feed identical uevent sequences to both udevd implementations; compare: final device properties (`udevadm info --query=all`), symlink creation under `/dev/`, permissions/ownership, RUN program execution, network interface naming (`net_setup_link` builtin); test operator variants (`==`, `!=`, `=`, `+=`, `-=`, `:=`), string substitution (`%k`, `%n`, `$kernel`, `$name`, `$env{KEY}`, etc.), `GOTO`/`LABEL`, `IMPORT` (program, file, builtin, db, cmdline, parent), `TEST`, `PROGRAM`, `ATTR{}`/`SYSATTR{}` matching
- ❌ **hwdb lookups** — compare `systemd-hwdb query <modalias>` results for a corpus of modaliases covering USB, PCI, input, Bluetooth device classes; verify fnmatch glob matching and property override ordering

## Boot Sequence

- ❌ **Boot timing** — compare `systemd-analyze time` output (firmware, loader, kernel, initrd, userspace times); compare `systemd-analyze blame` unit ordering and duration; compare `systemd-analyze critical-chain` output for `multi-user.target` and `graphical.target`; normalize timing differences, focus on ordering equivalence
- ❌ **Boot target reachability** — compare set of active units after reaching `multi-user.target`; verify no units are in `failed` state that aren't also failed on real systemd; compare `systemctl is-system-running` output
- ❌ **Emergency/rescue mode** — compare boot behavior with `systemd.unit=emergency.target`, `rescue`, `single`, `s`, `1`-`5` kernel command line parameters; verify correct target activation and available units

## Configuration Parsing

- ❌ **system.conf / user.conf** — compare parsed configuration from `/etc/systemd/system.conf` and `/etc/systemd/user.conf` with drop-in directories; compare effective values via D-Bus Manager properties; test all directives (`LogLevel=`, `LogTarget=`, `LogColor=`, `LogLocation=`, `DumpCore=`, `CrashChangeVT=`, `CrashShell=`, `CrashReboot=`, `ShowStatus=`, `DefaultStandardOutput=`, `DefaultStandardError=`, `DefaultTimeoutStartSec=`, `DefaultTimeoutStopSec=`, `DefaultRestartSec=`, `DefaultStartLimitIntervalSec=`, `DefaultStartLimitBurst=`, `DefaultEnvironment=`, `ManagerEnvironment=`, `DefaultCPUAccounting=`, `DefaultMemoryAccounting=`, `DefaultIOAccounting=`, `DefaultTasksAccounting=`, `DefaultTasksMax=`, `DefaultLimitNOFILE=`, `DefaultOOMPolicy=`, `DefaultSmackProcessLabel=`)
- ❌ **journald.conf** — compare parsed configuration: `Storage=`, `Compress=`, `Seal=`, `SplitMode=`, `RateLimitIntervalSec=`, `RateLimitBurst=`, `SystemMaxUse=`, `SystemKeepFree=`, `SystemMaxFileSize=`, `SystemMaxFiles=`, `RuntimeMaxUse=`, `RuntimeKeepFree=`, `RuntimeMaxFileSize=`, `RuntimeMaxFiles=`, `MaxFileSec=`, `MaxRetentionSec=`, `ForwardToSyslog=`, `ForwardToKMsg=`, `ForwardToConsole=`, `ForwardToWall=`, `MaxLevelStore=`, `MaxLevelSyslog=`, `MaxLevelKMsg=`, `MaxLevelConsole=`, `MaxLevelWall=`, `ReadKMsg=`, `Audit=`; test drop-in override
- ❌ **resolved.conf** — compare `DNS=`, `FallbackDNS=`, `Domains=` (routing domain `~` prefix handling), `LLMNR=`, `MulticastDNS=`, `DNSSEC=`, `DNSOverTLS=`, `Cache=`, `DNSStubListener=`, `DNSStubListenerExtra=`, `ReadEtcHosts=`
- ❌ **networkd .network/.link/.netdev files** — compare parsed configuration values for identical config files; test all [Match] section keys, [Network] section (Address, Gateway, DNS, Domains, DHCP, IPv6AcceptRA, etc.), [Address], [Route], [RoutingPolicyRule], [DHCPv4], [DHCPv6], [DHCPv6PrefixDelegation], [Link] sections
- ❌ **timesyncd.conf** — compare `NTP=`, `FallbackNTP=`, `RootDistanceMaxSec=`, `PollIntervalMinSec=`, `PollIntervalMaxSec=`, `ConnectionRetrySec=`, `SaveIntervalSec=` parsing
- ❌ **oomd.conf** — compare `SwapUsedLimit=`, `DefaultMemoryPressureLimit=`, `DefaultMemoryPressureDurationSec=` parsing
- ❌ **coredump.conf** — compare `Storage=`, `Compress=`, `ProcessSizeMax=`, `ExternalSizeMax=`, `JournalSizeMax=`, `MaxUse=`, `KeepFree=` parsing
- ❌ **logind.conf** — compare parsed `NAutoVTs=`, `ReserveVT=`, `KillUserProcesses=`, `KillOnlyUsers=`, `KillExcludeUsers=`, `InhibitDelayMaxSec=`, `InhibitorsMax=`, `SessionsMax=`, `UserStopDelaySec=`, `HandlePowerKey=`, `HandleSuspendKey=`, `HandleHibernateKey=`, `HandleLidSwitch=`, `HandleLidSwitchExternalPower=`, `HandleLidSwitchDocked=`, `PowerKeyIgnoreInhibited=`, `IdleAction=`, `IdleActionSec=`, `RuntimeDirectorySize=`, `RuntimeDirectoryInodesMax=`, `RemoveIPC=`

## Cryptographic & Security Tools

- ❌ **systemd-creds** — compare `encrypt`/`decrypt` roundtrip for all `--with-key` modes (`host`, `tpm2`, `host+tpm2`, `null`, `auto`); verify wire format compatibility (credential encrypted by real systemd decryptable by systemd-rs and vice versa); compare `--pretty` output format; compare `has-tpm2` detection; compare `list` output
- ❌ **cryptsetup/veritysetup/integritysetup** — compare device-mapper table construction for identical inputs; verify dm-crypt/dm-verity/dm-integrity devices created by one implementation can be opened by the other; compare option parsing for all supported flags

## Fuzz-Driven Differential Testing

- ❌ **Unit file fuzzer** — generate random valid and invalid unit file fragments via `cargo-fuzz`; feed to both parsers; compare: accepted vs rejected, parsed values for accepted inputs, error messages for rejected inputs; run continuously in CI
- ❌ **Calendar expression fuzzer** — generate random `OnCalendar=` expressions; compare `next_elapse()` computation for N=100 future elapses from a fixed reference; any divergence in elapse time is a bug
- ❌ **Timespan fuzzer** — generate random timespan strings; compare parsed microsecond values; any difference is a bug
- ❌ **DNS wire format fuzzer** — generate random DNS messages; feed to both resolved implementations; compare: parsed question/answer sections, response construction, error handling for malformed packets
- ❌ **D-Bus message fuzzer** — generate random D-Bus method calls with valid/invalid arguments; compare error responses and property values

## Compatibility & Regression

- ❌ **Cross-version credential compatibility** — encrypt credentials with systemd v254/v255/v256 and decrypt with systemd-rs; encrypt with systemd-rs and decrypt with each systemd version; verify all `--with-key` modes
- ❌ **Journal format compatibility** — write journal entries with real journald, read with systemd-rs journalctl and vice versa; verify cursor strings are interoperable; test export format roundtrip
- ❌ **Unit file backward compatibility** — test unit files from systemd v230–v256 era (pre-and-post various directive additions); verify parsing succeeds and unknown directives are gracefully ignored
- ❌ **Distro unit file corpus** — parse all unit files from Fedora 39/40, Debian 12/13, Ubuntu 24.04, Arch Linux, NixOS 24.05/24.11 package sets; compare parsed results against real systemd on same distro; track pass rate per distro

## Performance Comparison

- ❌ **Boot time benchmark** — compare wall-clock time from kernel handoff to `multi-user.target` reached on identical hardware/VM; report userspace time, unit count, parallel activation efficiency; run 10 iterations for statistical significance
- ❌ **Service start latency** — compare time from `systemctl start` to `ActiveState=active` for simple, notify, forking, and oneshot service types; measure via D-Bus property timestamp deltas
- ❌ **Journal write throughput** — compare messages/second sustainable by journald under identical load (1K/10K/100K msgs/sec bursts); measure via custom sd_journal_send loop
- ❌ **Memory footprint** — compare RSS of PID 1, journald, networkd, resolved, logind at idle and under load; report per-daemon and total
- ❌ **systemctl response time** — compare wall-clock time for `systemctl list-units`, `systemctl status`, `systemctl show` with varying unit counts (100, 500, 1000+ loaded units)

Legend: ✅ = complete, 🔶 = partial, ❌ = not started