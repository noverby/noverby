#!/usr/bin/env nu

# test-boot.nu — Boot a NixOS VM in cloud-hypervisor and capture serial output.
#
# Usage:
#   nu test-boot.nu              # Run with defaults (nixos-rs, 15s timeout)
#   nu test-boot.nu --config nixos-nix  # Boot the vanilla NixOS config
#   nu test-boot.nu --timeout 60 # Custom timeout in seconds
#   nu test-boot.nu --log /tmp/boot.log  # Custom log path
#   nu test-boot.nu --keep       # Keep VM running after success pattern is found
#   nu test-boot.nu --verbose    # Stream boot output to stderr in real-time
#
# Networking:
#   Locally — expects vmtap0 to exist (created by NixOS host config).
#   CI      — auto-creates vmtap0 with sudo if it doesn't exist.
#
# Exit codes:
#   0 — boot succeeded (login prompt reached)
#   1 — boot failed (kernel panic, systemd-rs crash, or timeout)
#   2 — missing dependencies or build failure

# ── Logging helpers ────────────────────────────────────────────────────────

def log-info [msg: string] {
    print -e $"(ansi light_blue_bold)[info](ansi reset)  ($msg)"
}

def log-ok [msg: string] {
    print -e $"(ansi light_green_bold)[pass](ansi reset)  ($msg)"
}

def log-fail [msg: string] {
    print -e $"(ansi light_red_bold)[fail](ansi reset)  ($msg)"
}

def log-warn [msg: string] {
    print -e $"(ansi light_yellow_bold)[warn](ansi reset)  ($msg)"
}

# ── Cleanup helper ─────────────────────────────────────────────────────────

def cleanup [
    state: record
] {
    # Kill the socket reader
    if $state.reader_pid != null {
        do -i { ^kill $state.reader_pid } | complete | ignore
        do -i { ^kill -0 $state.reader_pid } | complete | ignore
    }

    # Kill cloud-hypervisor
    if $state.ch_pid != null {
        log-info $"Shutting down VM \(PID ($state.ch_pid)\)..."
        do -i { ^kill $state.ch_pid } | complete | ignore
        mut waited = 0
        loop {
            if $waited >= 10 { break }
            let alive = (do -i { ^kill -0 $state.ch_pid } | complete)
            if $alive.exit_code != 0 { break }
            sleep 200ms
            $waited = $waited + 1
        }
        let still_alive = (do -i { ^kill -0 $state.ch_pid } | complete)
        if $still_alive.exit_code == 0 {
            do -i { ^kill -9 $state.ch_pid } | complete | ignore
        }
    }

    # Clean up socket
    if $state.serial_sock != null {
        rm -f $state.serial_sock
    }

    # Tear down CI networking if we set it up
    if $state.ci_net_setup {
        log-info "Tearing down CI network..."
        if $state.dnsmasq_pid != null {
            do -i { ^sudo kill $state.dnsmasq_pid } | complete | ignore
        }
        do -i { ^sudo ip link del $state.tap_name } | complete | ignore
        do -i { ^sudo iptables -t nat -D POSTROUTING -s $state.tap_subnet -j MASQUERADE } | complete | ignore
    }

    # Clean up temp log if needed
    if $state.cleanup_log and ($state.log_file | path exists) {
        rm -f $state.log_file
    }
}

# ── Main ───────────────────────────────────────────────────────────────────

def main [
    --timeout: int = 15          # Timeout in seconds
    --log: string = ""           # Path to save boot log
    --keep                       # Keep VM running after success
    --verbose                    # Stream boot output to stderr
    --config: string = "nixos-rs" # NixOS configuration name
    --disk: string = ""          # Path to disk image (default: derived from config)
] {
    let script_dir = ($env.FILE_PWD)
    let flake_dir = ($script_dir | path dirname)

    # Networking constants
    let tap_name = "vmtap0"
    let tap_addr = "192.168.100.1/24"
    let tap_subnet = "192.168.100.0/24"

    # Success / failure patterns
    let success_patterns = ["login:"]
    let failure_patterns = [
        "Kernel panic"
        "end Kernel panic"
        "Failed to execute /init"
        "not syncing"
        "BUG:"
        "thread.*panicked"
        "Entering emergency mode"
    ]

    # Derive disk image path
    let disk_image = if $disk != "" { $disk } else { [$script_dir $"($config).raw"] | path join }

    # Set up log file
    let cleanup_log = $log == ""
    let log_file = if $log == "" {
        ^mktemp $"/tmp/($config)-boot-XXXXXX.log" | str trim
    } else {
        $log
    }

    # Serial socket path
    let serial_sock = (^mktemp -u $"/tmp/($config)-serial-XXXXXX.sock" | str trim)

    # Mutable state for cleanup
    mut state = {
        ch_pid: null
        reader_pid: null
        serial_sock: $serial_sock
        ci_net_setup: false
        dnsmasq_pid: null
        tap_name: $tap_name
        tap_subnet: $tap_subnet
        log_file: $log_file
        cleanup_log: $cleanup_log
    }

    # Wrap everything in try so we can clean up
    let result = try {

        # ── Preflight checks ──────────────────────────────────────────────────

        let missing_cmds = ([cloud-hypervisor nix python3]
            | where { |cmd| which $cmd | is-empty })
        if ($missing_cmds | is-not-empty) {
            for cmd in $missing_cmds {
                log-fail $"Required command not found: ($cmd)"
            }
            cleanup $state
            exit 2
        }

        # ── Network setup ─────────────────────────────────────────────────────

        let tap_exists = ((do -i { ^ip link show $tap_name } | complete).exit_code == 0)

        if $tap_exists {
            log-info $"Using existing TAP device ($tap_name)"
        } else {
            log-info $"TAP device ($tap_name) not found — setting up networking with sudo \(CI mode\)"

            if (which dnsmasq | is-empty) {
                log-fail "Required command not found: dnsmasq \(needed for CI network setup\)"
                cleanup $state
                exit 2
            }

            let current_user = (^whoami | str trim)
            ^sudo ip tuntap add dev $tap_name mode tap user $current_user
            ^sudo ip addr add $tap_addr dev $tap_name
            ^sudo ip link set $tap_name up
            ^sudo sysctl -qw net.ipv4.ip_forward=1

            let nat_exists = ((do -i { ^sudo iptables -t nat -C POSTROUTING -s $tap_subnet -j MASQUERADE } | complete).exit_code == 0)
            if not $nat_exists {
                ^sudo iptables -t nat -A POSTROUTING -s $tap_subnet -j MASQUERADE
            }

            # Start dnsmasq in background
            let dnsmasq_pid = (^bash -c $'sudo dnsmasq --keep-in-foreground --interface=($tap_name) --bind-interfaces --dhcp-range=192.168.100.100,192.168.100.200,24h --dhcp-option=option:router,192.168.100.1 --dhcp-option=option:dns-server,1.1.1.1,8.8.8.8 --no-resolv --log-dhcp & echo $!' | str trim)

            $state.dnsmasq_pid = $dnsmasq_pid
            $state.ci_net_setup = true
            log-info $"CI network ready: ($tap_name) \(($tap_addr)\), dnsmasq PID ($dnsmasq_pid)"
        }

        # ── Build kernel & initrd paths ────────────────────────────────────────

        log-info $"Resolving kernel and initrd from flake \(($config)\)..."

        let kernel_dir = (^nix build --no-link --print-out-paths $"($flake_dir)#nixosConfigurations.($config).config.system.build.kernel" | str trim)
        let initrd_dir = (^nix build --no-link --print-out-paths $"($flake_dir)#nixosConfigurations.($config).config.system.build.initialRamdisk" | str trim)

        let kernel = [$kernel_dir "bzImage"] | path join
        let initrd = [$initrd_dir "initrd"] | path join

        if not ($kernel | path exists) {
            log-fail $"Kernel not found at ($kernel)"
            cleanup $state
            exit 2
        }
        if not ($initrd | path exists) {
            log-fail $"Initrd not found at ($initrd)"
            cleanup $state
            exit 2
        }

        log-info $"Kernel: ($kernel)"
        log-info $"Initrd: ($initrd)"

        # ── Check disk image ──────────────────────────────────────────────────

        if not ($disk_image | path exists) {
            log-warn $"Disk image not found at ($disk_image)"
            log-info "Building disk image (this may take a while)..."
            cd $script_dir
            ^nixos-rebuild build-image --image-variant qemu --flake $"($flake_dir)#($config)"
            let qcow2 = (glob "result/nixos-image-*-x86_64-linux.qcow2" | first)
            if ($qcow2 | is-empty) {
                log-fail "Failed to build disk image"
                cleanup $state
                exit 2
            }
            ^qemu-img convert -p -f qcow2 -O raw $qcow2 $disk_image
        }

        log-info $"Disk image: ($disk_image)"

        # ── Launch VM ──────────────────────────────────────────────────────────

        log-info $"Booting VM with ($timeout)s timeout, serial socket -> ($serial_sock)"
        log-info $"Boot log -> ($log_file)"

        # Truncate log file
        "" | save -f $log_file

        let ch_pid = (^bash -c $'cloud-hypervisor --kernel "($kernel)" --initramfs "($initrd)" --disk path="($disk_image)" --cmdline "console=ttyS0 root=LABEL=nixos init=/nix/var/nix/profiles/system/init" --cpus boot=2 --memory size=4096M --net "tap=($tap_name),mac=12:34:56:78:90:ab" --serial socket="($serial_sock)" --console off & echo $!' | str trim)
        $state.ch_pid = $ch_pid

        log-info $"cloud-hypervisor started \(PID ($ch_pid)\)"

        # ── Wait for the serial socket to appear ───────────────────────────────

        log-info "Waiting for serial socket..."
        mut sock_ready = false
        for _ in 1..31 {
            if ($serial_sock | path exists) {
                $sock_ready = true
                break
            }
            let alive = (do -i { ^kill -0 $ch_pid } | complete)
            if $alive.exit_code != 0 {
                log-fail "cloud-hypervisor exited before serial socket was created"
                cleanup $state
                exit 1
            }
            sleep 100ms
        }

        if not $sock_ready {
            log-fail $"Serial socket did not appear at ($serial_sock)"
            cleanup $state
            exit 1
        }

        log-info "Serial socket ready"

        # ── Connect to serial socket ──────────────────────────────────────────

        let py_script = r#'
import socket
import sys
import time
import select

sock_path = sys.argv[1]
log_path = sys.argv[2]
verbose = "--verbose" in sys.argv

sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)

for attempt in range(20):
    try:
        sock.connect(sock_path)
        break
    except (ConnectionRefusedError, FileNotFoundError):
        time.sleep(0.1)
else:
    print("Failed to connect to serial socket", file=sys.stderr)
    sys.exit(1)

sock.setblocking(False)

with open(log_path, "ab", buffering=0) as log:
    while True:
        try:
            ready, _, _ = select.select([sock], [], [], 0.5)
        except (ValueError, OSError):
            break

        if ready:
            try:
                data = sock.recv(4096)
            except (ConnectionResetError, OSError):
                break
            if not data:
                break

            log.write(data)
            if verbose:
                sys.stderr.buffer.write(data)
                sys.stderr.buffer.flush()

sock.close()
'#

        # Write the Python helper to a temp file to avoid shell quoting issues
        let py_file = (^mktemp /tmp/serial-reader-XXXXXX.py | str trim)
        $py_script | save -f $py_file

        let reader_cmd = if $verbose {
            $'python3 "($py_file)" "($serial_sock)" "($log_file)" --verbose & echo $!'
        } else {
            $'python3 "($py_file)" "($serial_sock)" "($log_file)" & echo $!'
        }
        let reader_pid = (^bash -c $reader_cmd | str trim)
        $state.reader_pid = $reader_pid

        # ── Monitor boot progress ─────────────────────────────────────────────

        mut boot_result = ""
        let start_time = (date now)

        loop {
            let elapsed_secs = ((date now) - $start_time | into int) // 1_000_000_000
            if $elapsed_secs >= $timeout { break }
            # Check if VM is still running
            let alive = (do -i { ^kill -0 $ch_pid } | complete)
            if $alive.exit_code != 0 {
                log-warn "VM exited unexpectedly"
                $boot_result = "vm_exited"
                break
            }

            if ($log_file | path exists) {
                let file_info = (ls $log_file)
                let log_size = ($file_info | get 0.size)
                if $log_size > 0b {
                    let log_content = (open --raw $log_file | decode utf-8)

                    # Check failure patterns first
                    mut failure_found = false
                    for pattern in $failure_patterns {
                        let grep_result = ($log_content | do -i { ^grep -iPm1 $pattern } | complete)
                        let matched = if $grep_result.exit_code == 0 { $grep_result.stdout | str trim } else { "" }
                        if $matched != "" {
                            log-fail $"Failure pattern detected: ($matched)"
                            $boot_result = "failure"
                            $failure_found = true
                            break
                        }
                    }
                    if $failure_found { break }

                    # Check success patterns
                    mut success_found = false
                    for pattern in $success_patterns {
                        let grep_result = ($log_content | do -i { ^grep -qi $pattern } | complete)
                        if $grep_result.exit_code == 0 {
                            $boot_result = "success"
                            $success_found = true
                            break
                        }
                    }
                    if $success_found { break }
                }
            }

            sleep 1sec
        }

        # Give a moment for final output to flush
        sleep 1sec

        # ── Evaluate result ────────────────────────────────────────────────────

        print -e ""
        let elapsed = ((date now) - $start_time | into int) // 1_000_000_000

        if $boot_result == "" {
            $boot_result = "timeout"
        }

        # Print captured output (unless verbose already streamed it)
        if not $verbose {
            log-info "--- Boot output ---"
            if ($log_file | path exists) {
                let file_info = (ls $log_file)
                let log_size = ($file_info | get 0.size)
                if $log_size > 0b {
                    open --raw $log_file | decode utf-8 | print -e $in
                } else {
                    log-warn "(empty -- no serial output captured)"
                }
            }
            log-info "--- End boot output ---"
            print -e ""
        }

        if $boot_result == "success" {
            log-ok $"Boot succeeded in ($elapsed)s"
            if $keep {
                log-info $"VM is still running \(PID ($ch_pid)\). Press Ctrl-C to stop."
                do -i { ^bash -c $"wait ($ch_pid)" } | complete | ignore
            }
            { exit_code: 0 }
        } else if $boot_result == "failure" {
            log-fail "Boot failed -- see output above"
            { exit_code: 1 }
        } else if $boot_result == "timeout" {
            log-fail $"Boot timed out after ($timeout)s without reaching login prompt"
            log-warn "Last 20 lines of boot log:"
            try { ^tail -20 $log_file | print -e $in } catch { }
            log-warn "Increase timeout with --timeout <seconds> if the system needs more time"
            { exit_code: 1 }
        } else {
            # vm_exited — check if it was actually a success
            let log_content = if ($log_file | path exists) {
                open --raw $log_file | decode utf-8
            } else {
                ""
            }
            mut was_success = false
            for pattern in $success_patterns {
                let grep_result = ($log_content | do -i { ^grep -qi $pattern } | complete)
                if $grep_result.exit_code == 0 {
                    $was_success = true
                    break
                }
            }
            if $was_success {
                log-ok $"Boot succeeded \(VM exited after ($elapsed)s)"
                { exit_code: 0 }
            } else {
                log-fail "VM exited before reaching login prompt"
                { exit_code: 1 }
            }
        }
    } catch {|err|
        log-fail $"Unexpected error: ($err.msg)"
        { exit_code: 2 }
    }

    cleanup $state
    exit $result.exit_code
}
