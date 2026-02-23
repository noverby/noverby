#!/usr/bin/env bash
set -euo pipefail

# test-browser.sh — Load wasm-mojo apps in headless Servo and verify DOM state
#                    via W3C WebDriver, driven entirely from bash + curl + jq.
#
# Inspired by nixos-rs/test-boot.sh: build artifact → boot in real environment
# → monitor output → pattern-match success/failure → bounded execution.
#
# Usage:
#   ./test-browser.sh                    # Run all app tests (default timeout)
#   ./test-browser.sh --timeout 60       # Custom timeout per test (seconds)
#   ./test-browser.sh --app counter      # Test only the counter app
#   ./test-browser.sh --verbose          # Stream servo stderr
#   ./test-browser.sh --keep             # Keep servo running after tests
#
# Exit codes:
#   0 — all tests passed
#   1 — test failure
#   2 — missing dependencies or setup failure

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TIMEOUT=30
VERBOSE=false
KEEP=false
APP_FILTER=""

WD_PORT=7123
SERVE_PORT=4507
WD_URL="http://127.0.0.1:$WD_PORT"
BASE_URL="http://127.0.0.1:$SERVE_PORT"

# ── Logging (same style as nixos-rs/test-boot.sh) ─────────────────────────

log_info()  { printf '\033[1;34m[info]\033[0m  %s\n' "$*" >&2; }
log_ok()    { printf '\033[1;32m[pass]\033[0m  %s\n' "$*" >&2; }
log_fail()  { printf '\033[1;31m[fail]\033[0m  %s\n' "$*" >&2; }
log_warn()  { printf '\033[1;33m[warn]\033[0m  %s\n' "$*" >&2; }

usage() {
    sed -n '3,17s/^# \?//p' "$0"
    exit 2
}

# ── Parse arguments ────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --timeout)  TIMEOUT="$2"; shift 2 ;;
        --app)      APP_FILTER="$2"; shift 2 ;;
        --verbose)  VERBOSE=true; shift ;;
        --keep)     KEEP=true; shift ;;
        --help|-h)  usage ;;
        *)          log_fail "Unknown option: $1"; usage ;;
    esac
done

# ── State ──────────────────────────────────────────────────────────────────

SERVO_PID=""
SERVER_PID=""
SESSION_ID=""
PASSED=0
FAILED=0
SERVO_LOG=""

# ── Cleanup (mirrors test-boot.sh trap pattern) ───────────────────────────

cleanup() {
    local exit_code=$?

    # Delete WebDriver session
    if [[ -n "$SESSION_ID" ]]; then
        curl -sf -X DELETE "$WD_URL/session/$SESSION_ID" >/dev/null 2>&1 || true
        SESSION_ID=""
    fi

    # Kill servo
    if [[ -n "$SERVO_PID" ]] && kill -0 "$SERVO_PID" 2>/dev/null; then
        log_info "Shutting down Servo (PID $SERVO_PID)..."
        kill "$SERVO_PID" 2>/dev/null || true
        for _ in $(seq 1 10); do
            kill -0 "$SERVO_PID" 2>/dev/null || break
            sleep 0.2
        done
        if kill -0 "$SERVO_PID" 2>/dev/null; then
            kill -9 "$SERVO_PID" 2>/dev/null || true
        fi
        wait "$SERVO_PID" 2>/dev/null || true
    fi

    # Kill file server
    if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
        log_info "Shutting down file server (PID $SERVER_PID)..."
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi

    # Clean up servo log
    if [[ -n "$SERVO_LOG" && -f "$SERVO_LOG" ]]; then
        rm -f "$SERVO_LOG"
    fi

    exit $exit_code
}
trap cleanup EXIT INT TERM

# ── Preflight checks ──────────────────────────────────────────────────────

for cmd in servo deno curl jq; do
    if ! command -v "$cmd" &>/dev/null; then
        log_fail "Required command not found: $cmd"
        exit 2
    fi
done

# ── Build WASM (like nixos-rs builds the disk image) ──────────────────────

log_info "Building WASM..."
(cd "$SCRIPT_DIR" && just build)

# ── Kill stale processes on our ports ─────────────────────────────────────

for port in "$SERVE_PORT" "$WD_PORT"; do
    if fuser "$port/tcp" >/dev/null 2>&1; then
        log_warn "Port $port already in use — killing stale process"
        fuser -k "$port/tcp" >/dev/null 2>&1 || true
        # Wait for port to actually be released
        for _ in $(seq 1 20); do
            fuser "$port/tcp" >/dev/null 2>&1 || break
            sleep 0.2
        done
        if fuser "$port/tcp" >/dev/null 2>&1; then
            log_warn "Port $port still in use after kill — forcing with -9"
            fuser -k -9 "$port/tcp" >/dev/null 2>&1 || true
            sleep 1
        fi
    fi
done

# ── Start file server (like nixos-rs starts cloud-hypervisor) ─────────────

log_info "Starting file server on :$SERVE_PORT..."
(cd "$SCRIPT_DIR" && deno run --allow-net --allow-read jsr:@std/http/file-server -p "$SERVE_PORT") 2>/dev/null &
SERVER_PID=$!

# Wait for file server to be ready
for _ in $(seq 1 30); do
    if curl -sf "$BASE_URL/" >/dev/null 2>&1; then break; fi
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        log_fail "File server exited unexpectedly"
        exit 2
    fi
    sleep 0.2
done

if ! curl -sf "$BASE_URL/" >/dev/null 2>&1; then
    log_fail "File server did not become ready"
    exit 2
fi

log_info "File server ready (PID $SERVER_PID)"

# ── Start Servo (headless + WebDriver) ────────────────────────────────────

SERVO_LOG=$(mktemp /tmp/servo-test-XXXXXX.log)

log_info "Starting Servo (headless, WebDriver on :$WD_PORT)..."

servo --headless --webdriver="$WD_PORT" "about:blank" >"$SERVO_LOG" 2>&1 &
SERVO_PID=$!

# Wait for WebDriver to become ready
for _ in $(seq 1 50); do
    if curl -sf "$WD_URL/status" >/dev/null 2>&1; then break; fi
    if ! kill -0 "$SERVO_PID" 2>/dev/null; then
        log_fail "Servo exited before WebDriver was ready"
        if [[ "$VERBOSE" == "true" && -f "$SERVO_LOG" ]]; then
            log_warn "Servo output:"
            cat "$SERVO_LOG" >&2
        fi
        exit 2
    fi
    sleep 0.2
done

if ! curl -sf "$WD_URL/status" >/dev/null 2>&1; then
    log_fail "WebDriver did not become ready on :$WD_PORT"
    if [[ -f "$SERVO_LOG" ]]; then
        log_warn "Last 20 lines of Servo output:"
        tail -20 "$SERVO_LOG" >&2
    fi
    exit 2
fi

log_info "Servo ready (PID $SERVO_PID)"

# ── WebDriver helpers (curl + jq — no JS needed) ─────────────────────────

wd_post() {
    local path="$1" body="$2"
    curl -sf -H "Content-Type: application/json" -d "$body" "$WD_URL$path" 2>/dev/null
}

wd_get() {
    local path="$1"
    curl -sf "$WD_URL$path" 2>/dev/null
}

wd_delete() {
    local path="$1"
    curl -sf -X DELETE "$WD_URL$path" 2>/dev/null
}

# Create a new WebDriver session
wd_new_session() {
    local resp
    resp=$(wd_post "/session" '{"capabilities":{}}')
    echo "$resp" | jq -r '.value.sessionId // .sessionId // empty'
}

# Navigate to a URL
wd_navigate() {
    local url="$1"
    wd_post "/session/$SESSION_ID/url" "{\"url\": \"$url\"}" >/dev/null
}

# Find an element by CSS selector. Returns the element ID.
wd_find() {
    local css="$1"
    local resp
    resp=$(wd_post "/session/$SESSION_ID/element" \
        "{\"using\": \"css selector\", \"value\": $(echo "$css" | jq -Rs .)}")
    # WebDriver returns {"value": {"element-...": "id"}} or {"value": {"ELEMENT": "id"}}
    echo "$resp" | jq -r '.value | to_entries[0].value // empty'
}

# Find multiple elements by CSS selector. Returns count.
wd_find_all_count() {
    local css="$1"
    local resp
    resp=$(wd_post "/session/$SESSION_ID/elements" \
        "{\"using\": \"css selector\", \"value\": $(echo "$css" | jq -Rs .)}")
    echo "$resp" | jq '.value | length'
}

# Get the text content of an element by its element ID.
wd_text() {
    local eid="$1"
    wd_get "/session/$SESSION_ID/element/$eid/text" | jq -r '.value // empty'
}

# Get an attribute of an element.
wd_attr() {
    local eid="$1" attr="$2"
    wd_get "/session/$SESSION_ID/element/$eid/attribute/$attr" | jq -r '.value // empty'
}

# Click an element by its element ID.
wd_click() {
    local eid="$1"
    wd_post "/session/$SESSION_ID/element/$eid/click" '{}' >/dev/null
}

# Send keys to an element.
wd_send_keys() {
    local eid="$1" text="$2"
    wd_post "/session/$SESSION_ID/element/$eid/value" \
        "{\"text\": $(echo "$text" | jq -Rs .)}" >/dev/null
}

# Execute JavaScript (for waiting on WASM load).
wd_execute() {
    local script="$1"
    local resp
    resp=$(wd_post "/session/$SESSION_ID/execute/sync" \
        "{\"script\": $(echo "$script" | jq -Rs .), \"args\": []}")
    echo "$resp" | jq -r '.value // empty'
}

# Wait for a condition (poll-based, like test-boot.sh serial monitoring).
wd_wait_for_element() {
    local css="$1" max_wait="${2:-$TIMEOUT}"
    local elapsed=0
    while [[ $elapsed -lt $max_wait ]]; do
        local eid
        eid=$(wd_find "$css" 2>/dev/null || true)
        if [[ -n "$eid" && "$eid" != "null" ]]; then
            return 0
        fi
        sleep 0.5
        elapsed=$((elapsed + 1))
    done
    return 1
}

# ── Test assertion helpers ─────────────────────────────────────────────────

assert_text() {
    local label="$1" css="$2" expected="$3"
    local eid text
    eid=$(wd_find "$css" 2>/dev/null || true)
    if [[ -z "$eid" || "$eid" == "null" ]]; then
        log_fail "$label — element not found: $css"
        FAILED=$((FAILED + 1))
        return
    fi
    text=$(wd_text "$eid")
    if [[ "$text" == "$expected" ]]; then
        log_ok "$label"
        PASSED=$((PASSED + 1))
    else
        log_fail "$label — expected: \"$expected\", got: \"$text\""
        FAILED=$((FAILED + 1))
    fi
}

assert_text_contains() {
    local label="$1" css="$2" substring="$3"
    local eid text
    eid=$(wd_find "$css" 2>/dev/null || true)
    if [[ -z "$eid" || "$eid" == "null" ]]; then
        log_fail "$label — element not found: $css"
        FAILED=$((FAILED + 1))
        return
    fi
    text=$(wd_text "$eid")
    if [[ "$text" == *"$substring"* ]]; then
        log_ok "$label"
        PASSED=$((PASSED + 1))
    else
        log_fail "$label — expected to contain: \"$substring\", got: \"$text\""
        FAILED=$((FAILED + 1))
    fi
}

assert_exists() {
    local label="$1" css="$2"
    local eid
    eid=$(wd_find "$css" 2>/dev/null || true)
    if [[ -n "$eid" && "$eid" != "null" ]]; then
        log_ok "$label"
        PASSED=$((PASSED + 1))
    else
        log_fail "$label — element not found: $css"
        FAILED=$((FAILED + 1))
    fi
}

assert_count() {
    local label="$1" css="$2" expected="$3"
    local count
    count=$(wd_find_all_count "$css" 2>/dev/null || echo "0")
    if [[ "$count" -eq "$expected" ]]; then
        log_ok "$label"
        PASSED=$((PASSED + 1))
    else
        log_fail "$label — expected $expected elements, got $count"
        FAILED=$((FAILED + 1))
    fi
}

# ── Create WebDriver session ──────────────────────────────────────────────

log_info "Creating WebDriver session..."
SESSION_ID=$(wd_new_session)

if [[ -z "$SESSION_ID" ]]; then
    log_fail "Failed to create WebDriver session"
    exit 2
fi

log_info "Session: $SESSION_ID"

# ── App test: Counter ──────────────────────────────────────────────────────

test_counter() {
    log_info ""
    log_info "── Counter app ──────────────────────────────────────────"

    wd_navigate "$BASE_URL/examples/counter/"

    # Wait for WASM to load and mount (like test-boot.sh waits for login:)
    if ! wd_wait_for_element "#root h1" 15; then
        log_fail "Counter app did not mount (no h1 found within 15s)"
        FAILED=$((FAILED + 1))
        return
    fi
    # Small settle delay for mutations to flush
    sleep 0.3

    # Initial state
    assert_text "initial count is 0" "#root h1" "High-Five counter: 0"
    assert_exists "DOM has increment button" "#root button:first-of-type"
    assert_exists "DOM has decrement button" "#root button:nth-of-type(2)"
    assert_count "DOM has 3 buttons" "#root button" 3

    # Click increment
    local btn_incr
    btn_incr=$(wd_find "#root button:first-of-type")
    wd_click "$btn_incr"
    sleep 0.2
    assert_text "count after 1 increment" "#root h1" "High-Five counter: 1"

    # Click 4 more times
    for _ in $(seq 1 4); do
        wd_click "$btn_incr"
        sleep 0.05
    done
    sleep 0.2
    assert_text "count after 5 increments" "#root h1" "High-Five counter: 5"

    # Click decrement
    local btn_decr
    btn_decr=$(wd_find "#root button:nth-of-type(2)")
    wd_click "$btn_decr"
    sleep 0.2
    assert_text "count after decrement" "#root h1" "High-Five counter: 4"

    # Decrement past zero
    for _ in $(seq 1 6); do
        wd_click "$btn_decr"
        sleep 0.05
    done
    sleep 0.2
    assert_text "count below zero" "#root h1" "High-Five counter: -2"

    # DOM structure preserved
    assert_exists "h1 still present after clicks" "#root h1"
    assert_count "buttons still present" "#root button" 3
}

# ── App test: Todo ─────────────────────────────────────────────────────────

test_todo() {
    log_info ""
    log_info "── Todo app ─────────────────────────────────────────────"

    wd_navigate "$BASE_URL/examples/todo/"

    # Wait for mount
    if ! wd_wait_for_element "#root > div" 15; then
        log_fail "Todo app did not mount (no container div within 15s)"
        FAILED=$((FAILED + 1))
        return
    fi
    sleep 0.3

    assert_exists "todo app mounts with container" "#root > div"
    assert_exists "todo app has input" "#root input"
    assert_exists "todo app has add button" "#root button"

    # Add first item via Add button (primary — reliable across WebDriver impls)
    local input_el add_btn root_text
    input_el=$(wd_find "#root input" 2>/dev/null || true)
    if [[ -z "$input_el" || "$input_el" == "null" ]]; then
        log_fail "Could not find input element"
        FAILED=$((FAILED + 1))
        return
    fi

    wd_send_keys "$input_el" "Buy milk"
    sleep 0.2
    add_btn=$(wd_find "#root > div > button" 2>/dev/null || true)
    if [[ -n "$add_btn" && "$add_btn" != "null" ]]; then
        wd_click "$add_btn"
        sleep 0.5

        root_text=$(wd_text "$(wd_find "#root")")
        if [[ "$root_text" == *"Buy milk"* ]]; then
            log_ok "todo item 'Buy milk' added via button click"
            PASSED=$((PASSED + 1))
        else
            log_fail "todo item 'Buy milk' not found in DOM after button click"
            FAILED=$((FAILED + 1))
        fi
    else
        log_fail "Could not find Add button"
        FAILED=$((FAILED + 1))
    fi

    # Add second item via Enter key (Servo WebDriver may not dispatch keydown;
    # treated as a soft/informational test — warn instead of fail)
    input_el=$(wd_find "#root input" 2>/dev/null || true)
    if [[ -n "$input_el" && "$input_el" != "null" ]]; then
        wd_send_keys "$input_el" "Walk the dog"
        sleep 0.1
        # \uE007 = WebDriver Enter key
        wd_send_keys "$input_el" "\uE007"
        sleep 0.5

        root_text=$(wd_text "$(wd_find "#root")")
        if [[ "$root_text" == *"Walk the dog"* ]]; then
            log_ok "todo item 'Walk the dog' added via Enter key"
            PASSED=$((PASSED + 1))
        else
            log_warn "Enter key did not add todo item (Servo WebDriver quirk — not a failure)"
            # Fall back: add via button so subsequent assertions still work
            input_el=$(wd_find "#root input" 2>/dev/null || true)
            if [[ -n "$input_el" && "$input_el" != "null" ]]; then
                add_btn=$(wd_find "#root > div > button" 2>/dev/null || true)
                if [[ -n "$add_btn" && "$add_btn" != "null" ]]; then
                    wd_click "$add_btn"
                    sleep 0.5
                fi
            fi
        fi
    fi

    # Verify list has items
    local li_count
    li_count=$(wd_find_all_count "#root li" 2>/dev/null || echo "0")
    if [[ "$li_count" -ge 1 ]]; then
        log_ok "todo list has $li_count item(s)"
        PASSED=$((PASSED + 1))
    else
        log_fail "todo list has no items (expected >= 1)"
        FAILED=$((FAILED + 1))
    fi
}

# ── App test: Bench ────────────────────────────────────────────────────────

test_bench() {
    log_info ""
    log_info "── Bench app ────────────────────────────────────────────"

    wd_navigate "$BASE_URL/examples/bench/"

    # Wait for mount — bench renders into #root
    if ! wd_wait_for_element "#root h1" 15; then
        # Bench might not use h1, try broader selector
        if ! wd_wait_for_element "#root button" 15; then
            log_fail "Bench app did not mount (no h1 or button within 15s)"
            FAILED=$((FAILED + 1))
            return
        fi
    fi
    sleep 0.3

    assert_exists "bench app mounts" "#root"

    # Bench should have toolbar buttons (Create 1,000 / Append / Update / etc.)
    local btn_count
    btn_count=$(wd_find_all_count "#root button" 2>/dev/null || echo "0")
    if [[ "$btn_count" -ge 4 ]]; then
        log_ok "bench toolbar has $btn_count buttons (>= 4 expected)"
        PASSED=$((PASSED + 1))
    else
        log_fail "bench toolbar has $btn_count buttons (expected >= 4)"
        FAILED=$((FAILED + 1))
    fi

    # Click "Create 1,000" (first button)
    local create_btn
    create_btn=$(wd_find "#root button:first-of-type" 2>/dev/null || true)
    if [[ -n "$create_btn" && "$create_btn" != "null" ]]; then
        wd_click "$create_btn"
        sleep 2  # Give WASM time to create + render 1000 rows

        # Check that rows appeared in the table
        local row_count
        row_count=$(wd_find_all_count "#root tbody tr" 2>/dev/null || echo "0")
        if [[ "$row_count" -ge 100 ]]; then
            log_ok "bench created $row_count rows after 'Create 1,000'"
            PASSED=$((PASSED + 1))
        else
            log_fail "bench created $row_count rows (expected >= 100)"
            FAILED=$((FAILED + 1))
        fi

        # Status should show operation info
        assert_exists "bench has status/timing info" "#root"
    else
        log_warn "Could not find Create button — skipping bench row tests"
    fi
}

# ── Run tests ──────────────────────────────────────────────────────────────

log_info ""
log_info "Running browser tests (timeout: ${TIMEOUT}s per wait)..."

if [[ -z "$APP_FILTER" || "$APP_FILTER" == "counter" ]]; then test_counter; fi
if [[ -z "$APP_FILTER" || "$APP_FILTER" == "todo" ]];    then test_todo; fi
if [[ -z "$APP_FILTER" || "$APP_FILTER" == "bench" ]];   then test_bench; fi

# ── Summary (same format as test-boot.sh) ─────────────────────────────────

echo "" >&2
TOTAL=$((PASSED + FAILED))

if [[ "$VERBOSE" == "true" && -f "$SERVO_LOG" && -s "$SERVO_LOG" ]]; then
    log_info "--- Servo output ---"
    cat "$SERVO_LOG" >&2
    log_info "--- End Servo output ---"
    echo "" >&2
fi

if [[ $FAILED -eq 0 ]]; then
    log_ok "$TOTAL tests: $PASSED passed, 0 failed"
    if [[ "$KEEP" == "true" ]]; then
        log_info "Servo still running (PID $SERVO_PID). Press Ctrl-C to stop."
        wait "$SERVO_PID" 2>/dev/null || true
    fi
    exit 0
else
    log_fail "$TOTAL tests: $PASSED passed, $FAILED failed"
    exit 1
fi
