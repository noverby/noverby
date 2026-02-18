#!/usr/bin/env bash
set -euo pipefail

# test-boot.sh — Boot a NixOS VM in cloud-hypervisor and capture serial output.
#
# Usage:
#   ./test-boot.sh              # Run with defaults (nixos-rs, 15s timeout)
#   ./test-boot.sh --config nixos-nix  # Boot the vanilla NixOS config
#   ./test-boot.sh --timeout 60 # Custom timeout in seconds
#   ./test-boot.sh --log /tmp/boot.log  # Custom log path
#   ./test-boot.sh --keep       # Keep VM running after success pattern is found
#   ./test-boot.sh --verbose    # Stream boot output to stderr in real-time
#
# Networking:
#   Locally — expects vmtap0 to exist (created by NixOS host config).
#   CI      — auto-creates vmtap0 with sudo if it doesn't exist.
#
# Exit codes:
#   0 — boot succeeded (login prompt reached)
#   1 — boot failed (kernel panic, systemd-rs crash, or timeout)
#   2 — missing dependencies or build failure

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FLAKE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Defaults
TIMEOUT=15
LOG_FILE=""
KEEP=false
VERBOSE=false
CONFIG="nixos-rs"

# Networking
TAP_NAME="vmtap0"
TAP_ADDR="192.168.100.1/24"
TAP_SUBNET="192.168.100.0/24"
CI_NET_SETUP=false
DNSMASQ_PID=""

# Success / failure patterns (checked against the log file)
SUCCESS_PATTERNS=(
    "login:"
)
FAILURE_PATTERNS=(
    "Kernel panic"
    "end Kernel panic"
    "Failed to execute /init"
    "not syncing"
    "BUG:"
    "thread.*panicked"
    "Entering emergency mode"
)

usage() {
    sed -n '3,17s/^# \?//p' "$0"
    exit 2
}

log_info()  { printf '\033[1;34m[info]\033[0m  %s\n' "$*" >&2; }
log_ok()    { printf '\033[1;32m[pass]\033[0m  %s\n' "$*" >&2; }
log_fail()  { printf '\033[1;31m[fail]\033[0m  %s\n' "$*" >&2; }
log_warn()  { printf '\033[1;33m[warn]\033[0m  %s\n' "$*" >&2; }

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --timeout)  TIMEOUT="$2"; shift 2 ;;
        --log)      LOG_FILE="$2"; shift 2 ;;
        --keep)     KEEP=true; shift ;;
        --verbose)  VERBOSE=true; shift ;;
        --config)   CONFIG="$2"; shift 2 ;;
        --disk)     DISK_IMAGE="$2"; shift 2 ;;
        --help|-h)  usage ;;
        *)          log_fail "Unknown option: $1"; usage ;;
    esac
done

# Derive disk image path from config name if not explicitly set
DISK_IMAGE="${DISK_IMAGE:-$SCRIPT_DIR/$CONFIG.raw}"

# Set up log file
if [[ -z "$LOG_FILE" ]]; then
    LOG_FILE=$(mktemp /tmp/$CONFIG-boot-XXXXXX.log)
    CLEANUP_LOG=true
else
    CLEANUP_LOG=false
fi

# Serial socket path
SERIAL_SOCK=$(mktemp -u /tmp/$CONFIG-serial-XXXXXX.sock)

# Track child processes for cleanup
CH_PID=""
READER_PID=""

cleanup() {
    local exit_code=$?
    # Kill the socket reader
    if [[ -n "$READER_PID" ]] && kill -0 "$READER_PID" 2>/dev/null; then
        kill "$READER_PID" 2>/dev/null || true
        wait "$READER_PID" 2>/dev/null || true
    fi
    # Kill cloud-hypervisor
    if [[ -n "$CH_PID" ]] && kill -0 "$CH_PID" 2>/dev/null; then
        log_info "Shutting down VM (PID $CH_PID)..."
        kill "$CH_PID" 2>/dev/null || true
        for _ in $(seq 1 10); do
            kill -0 "$CH_PID" 2>/dev/null || break
            sleep 0.2
        done
        if kill -0 "$CH_PID" 2>/dev/null; then
            kill -9 "$CH_PID" 2>/dev/null || true
        fi
        wait "$CH_PID" 2>/dev/null || true
    fi
    # Clean up socket
    rm -f "$SERIAL_SOCK"
    # Tear down CI networking if we set it up
    if [[ "$CI_NET_SETUP" == "true" ]]; then
        log_info "Tearing down CI network..."
        if [[ -n "$DNSMASQ_PID" ]] && kill -0 "$DNSMASQ_PID" 2>/dev/null; then
            sudo kill "$DNSMASQ_PID" 2>/dev/null || true
            wait "$DNSMASQ_PID" 2>/dev/null || true
        fi
        sudo ip link del "$TAP_NAME" 2>/dev/null || true
        sudo iptables -t nat -D POSTROUTING -s "$TAP_SUBNET" -j MASQUERADE 2>/dev/null || true
    fi
    # Clean up temp log if needed
    if [[ "$CLEANUP_LOG" == "true" && -f "$LOG_FILE" ]]; then
        rm -f "$LOG_FILE"
    fi
    exit $exit_code
}
trap cleanup EXIT INT TERM

# ── Preflight checks ───────────────────────────────────────────────────────

for cmd in cloud-hypervisor nix python3; do
    if ! command -v "$cmd" &>/dev/null; then
        log_fail "Required command not found: $cmd"
        exit 2
    fi
done

# ── Network setup ──────────────────────────────────────────────────────────
#
# If vmtap0 exists (NixOS host config), use it directly.
# Otherwise, create it with sudo (CI fallback).

if ip link show "$TAP_NAME" &>/dev/null; then
    log_info "Using existing TAP device $TAP_NAME"
else
    log_info "TAP device $TAP_NAME not found — setting up networking with sudo (CI mode)"
    for cmd in dnsmasq; do
        if ! command -v "$cmd" &>/dev/null; then
            log_fail "Required command not found: $cmd (needed for CI network setup)"
            exit 2
        fi
    done
    sudo ip tuntap add dev "$TAP_NAME" mode tap user "$(whoami)"
    sudo ip addr add "$TAP_ADDR" dev "$TAP_NAME"
    sudo ip link set "$TAP_NAME" up
    sudo sysctl -qw net.ipv4.ip_forward=1
    if ! sudo iptables -t nat -C POSTROUTING -s "$TAP_SUBNET" -j MASQUERADE 2>/dev/null; then
        sudo iptables -t nat -A POSTROUTING -s "$TAP_SUBNET" -j MASQUERADE
    fi
    sudo dnsmasq \
        --keep-in-foreground \
        --interface="$TAP_NAME" \
        --bind-interfaces \
        --dhcp-range=192.168.100.100,192.168.100.200,24h \
        --dhcp-option=option:router,192.168.100.1 \
        --dhcp-option=option:dns-server,1.1.1.1,8.8.8.8 \
        --no-resolv \
        --log-dhcp \
        &
    DNSMASQ_PID=$!
    CI_NET_SETUP=true
    log_info "CI network ready: $TAP_NAME ($TAP_ADDR), dnsmasq PID $DNSMASQ_PID"
fi

# ── Build kernel & initrd paths ────────────────────────────────────────────

log_info "Resolving kernel and initrd from flake ($CONFIG)..."

KERNEL_DIR=$(nix build --no-link --print-out-paths "$FLAKE_DIR#nixosConfigurations.$CONFIG.config.system.build.kernel" 2>&2)
INITRD_DIR=$(nix build --no-link --print-out-paths "$FLAKE_DIR#nixosConfigurations.$CONFIG.config.system.build.initialRamdisk" 2>&2)

KERNEL="$KERNEL_DIR/bzImage"
INITRD="$INITRD_DIR/initrd"

if [[ ! -f "$KERNEL" ]]; then
    log_fail "Kernel not found at $KERNEL"
    exit 2
fi
if [[ ! -f "$INITRD" ]]; then
    log_fail "Initrd not found at $INITRD"
    exit 2
fi

log_info "Kernel: $KERNEL"
log_info "Initrd: $INITRD"

# ── Check disk image ──────────────────────────────────────────────────────

if [[ ! -f "$DISK_IMAGE" ]]; then
    log_warn "Disk image not found at $DISK_IMAGE"
    log_info "Building disk image (this may take a while)..."
    pushd "$SCRIPT_DIR" > /dev/null
    nixos-rebuild build-image --image-variant qemu --flake "$FLAKE_DIR#$CONFIG"
    QCOW2=$(ls result/nixos-image-*-x86_64-linux.qcow2 2>/dev/null | head -1)
    if [[ -z "$QCOW2" ]]; then
        log_fail "Failed to build disk image"
        exit 2
    fi
    qemu-img convert -p -f qcow2 -O raw "$QCOW2" "$DISK_IMAGE"
    popd > /dev/null
fi

log_info "Disk image: $DISK_IMAGE"

# ── Launch VM ──────────────────────────────────────────────────────────────

log_info "Booting VM with ${TIMEOUT}s timeout, serial socket -> $SERIAL_SOCK"
log_info "Boot log -> $LOG_FILE"

# Truncate log file
> "$LOG_FILE"

cloud-hypervisor \
    --kernel "$KERNEL" \
    --initramfs "$INITRD" \
    --disk path="$DISK_IMAGE" \
    --cmdline "console=ttyS0 root=LABEL=nixos init=/nix/var/nix/profiles/system/init" \
    --cpus boot=2 \
    --memory size=4096M \
    --net "tap=$TAP_NAME,mac=12:34:56:78:90:ab" \
    --serial socket="$SERIAL_SOCK" \
    --console off \
    &
CH_PID=$!

log_info "cloud-hypervisor started (PID $CH_PID)"

# ── Wait for the serial socket to appear ───────────────────────────────────

log_info "Waiting for serial socket..."
for _ in $(seq 1 30); do
    if [[ -S "$SERIAL_SOCK" ]]; then
        break
    fi
    if ! kill -0 "$CH_PID" 2>/dev/null; then
        log_fail "cloud-hypervisor exited before serial socket was created"
        exit 1
    fi
    sleep 0.1
done

if [[ ! -S "$SERIAL_SOCK" ]]; then
    log_fail "Serial socket did not appear at $SERIAL_SOCK"
    exit 1
fi

log_info "Serial socket ready"

# ── Connect to serial socket ──────────────────────────────────────────────
#
# We use a python3 helper that:
#   1. Connects to the Unix domain socket
#   2. Reads output, writes it to the log file (and optionally to stderr)
#   3. Runs until killed

VERBOSE_FLAG=""
if [[ "$VERBOSE" == "true" ]]; then
    VERBOSE_FLAG="--verbose"
fi

python3 - "$SERIAL_SOCK" "$LOG_FILE" $VERBOSE_FLAG <<'PYEOF' &
import socket
import sys
import time
import select

sock_path = sys.argv[1]
log_path = sys.argv[2]
verbose = "--verbose" in sys.argv

sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)

# Retry connection a few times (socket may not be ready for connections yet)
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
PYEOF
READER_PID=$!

# ── Monitor boot progress ─────────────────────────────────────────────────

BOOT_RESULT=""
SECONDS=0

while [[ $SECONDS -lt $TIMEOUT ]]; do
    # Check if VM is still running
    if ! kill -0 "$CH_PID" 2>/dev/null; then
        log_warn "VM exited unexpectedly"
        BOOT_RESULT="vm_exited"
        break
    fi

    if [[ -f "$LOG_FILE" && -s "$LOG_FILE" ]]; then
        # Check for failure patterns first
        for pattern in "${FAILURE_PATTERNS[@]}"; do
            if grep -qiP "$pattern" "$LOG_FILE" 2>/dev/null; then
                matched=$(grep -iPm1 "$pattern" "$LOG_FILE" 2>/dev/null || true)
                log_fail "Failure pattern detected: $matched"
                BOOT_RESULT="failure"
                break 2
            fi
        done

        # Check for success patterns
        for pattern in "${SUCCESS_PATTERNS[@]}"; do
            if grep -qi "$pattern" "$LOG_FILE" 2>/dev/null; then
                BOOT_RESULT="success"
                break 2
            fi
        done
    fi

    sleep 1
done

# Give a moment for final output to flush
sleep 1

# ── Evaluate result ────────────────────────────────────────────────────────

echo "" >&2

if [[ -z "$BOOT_RESULT" ]]; then
    BOOT_RESULT="timeout"
fi

# Print the captured output (unless verbose already streamed it)
if [[ "$VERBOSE" != "true" ]]; then
    log_info "--- Boot output ---"
    if [[ -f "$LOG_FILE" && -s "$LOG_FILE" ]]; then
        cat "$LOG_FILE" >&2
    else
        log_warn "(empty -- no serial output captured)"
    fi
    log_info "--- End boot output ---"
    echo "" >&2
fi

case "$BOOT_RESULT" in
    success)
        log_ok "Boot succeeded in ${SECONDS}s"
        if [[ "$KEEP" == "true" ]]; then
            log_info "VM is still running (PID $CH_PID). Press Ctrl-C to stop."
            wait "$CH_PID" 2>/dev/null || true
        fi
        exit 0
        ;;
    failure)
        log_fail "Boot failed -- see output above"
        exit 1
        ;;
    timeout)
        log_fail "Boot timed out after ${TIMEOUT}s without reaching login prompt"
        log_warn "Last 20 lines of boot log:"
        tail -20 "$LOG_FILE" >&2 || true
        log_warn "Increase timeout with --timeout <seconds> if the system needs more time"
        exit 1
        ;;
    vm_exited)
        # Check if it was actually a success (VM might have shut down after reaching target)
        for pattern in "${SUCCESS_PATTERNS[@]}"; do
            if grep -qi "$pattern" "$LOG_FILE" 2>/dev/null; then
                log_ok "Boot succeeded (VM exited after ${SECONDS}s)"
                exit 0
            fi
        done
        log_fail "VM exited before reaching login prompt"
        exit 1
        ;;
esac
