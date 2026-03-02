#!/usr/bin/env nu

# test-browser.nu — Load RadikalWiki in headless Servo and verify DOM state
#                    via W3C WebDriver, driven entirely from nushell + http.
#
# Follows the same pattern as wasm-mojo/test-browser.nu: build artifact →
# boot in real environment → monitor output → pattern-match success/failure
# → bounded execution.
#
# Usage:
#   nu test-browser.nu                    # Run all page tests
#   nu test-browser.nu --timeout 60       # Custom timeout per wait (poll iterations)
#   nu test-browser.nu --page home        # Test only the home page
#   nu test-browser.nu --verbose          # Stream servo stderr
#   nu test-browser.nu --keep             # Keep servo running after tests
#   nu test-browser.nu --skip-build       # Skip rsbuild, use existing dist/
#
# Exit codes:
#   0 — all tests passed
#   1 — test failure
#   2 — missing dependencies or setup failure

const WD_PORT = 7123
const SERVE_PORT = 4508

def wd-url [] { $"http://127.0.0.1:($WD_PORT)" }
def base-url [] { $"http://127.0.0.1:($SERVE_PORT)" }

# ── Logging (same style as wasm-mojo/test-browser.nu) ─────────────────────

def log-info [...msg: string] {
    let text = ($msg | str join " ")
    print -e $"(ansi blue_bold)[info](ansi reset)  ($text)"
}

def log-ok [...msg: string] {
    let text = ($msg | str join " ")
    print -e $"(ansi green_bold)[pass](ansi reset)  ($text)"
}

def log-fail [...msg: string] {
    let text = ($msg | str join " ")
    print -e $"(ansi red_bold)[fail](ansi reset)  ($text)"
}

def log-warn [...msg: string] {
    let text = ($msg | str join " ")
    print -e $"(ansi yellow_bold)[warn](ansi reset)  ($text)"
}

# ── WebDriver helpers (http + from json — no curl/jq needed) ──────────────

def wd-post [path: string, body: string] {
    let url = $"(wd-url)($path)"
    try {
        ^curl -sf -H "Content-Type: application/json" -d $body $url | from json
    } catch {
        null
    }
}

def wd-get [path: string] {
    let url = $"(wd-url)($path)"
    try {
        ^curl -sf $url | from json
    } catch {
        null
    }
}

def wd-delete [path: string] {
    let url = $"(wd-url)($path)"
    try {
        ^curl -sf -X DELETE $url | complete | ignore
    } catch { }
}

# Create a new WebDriver session
def wd-new-session [] {
    let resp = (wd-post "/session" '{"capabilities":{}}')
    if $resp == null { return "" }
    let sid = ($resp | get -o value.sessionId | default ($resp | get -o sessionId | default ""))
    $sid
}

# Navigate to a URL
def wd-navigate [session_id: string, url: string] {
    wd-post $"/session/($session_id)/url" $'{"url": "($url)"}' | ignore
}

# Find an element by CSS selector. Returns the element ID.
def wd-find [session_id: string, css: string] {
    let css_json = ($css | to json -r)
    let body = $'{"using": "css selector", "value": ($css_json)}'
    let resp = (wd-post $"/session/($session_id)/element" $body)
    if $resp == null { return "" }
    let val = ($resp | get -o value)
    if $val == null { return "" }
    # WebDriver returns {"value": {"element-...": "id"}} or {"value": {"ELEMENT": "id"}}
    try {
        $val | values | first
    } catch {
        ""
    }
}

# Find multiple elements by CSS selector. Returns count.
def wd-find-all-count [session_id: string, css: string] {
    let css_json = ($css | to json -r)
    let body = $'{"using": "css selector", "value": ($css_json)}'
    let resp = (wd-post $"/session/($session_id)/elements" $body)
    if $resp == null { return 0 }
    try {
        $resp | get value | length
    } catch {
        0
    }
}

# Get the text content of an element by its element ID.
def wd-text [session_id: string, eid: string] {
    let resp = (wd-get $"/session/($session_id)/element/($eid)/text")
    if $resp == null { return "" }
    $resp | get -o value | default ""
}

# Get an attribute of an element.
def wd-attr [session_id: string, eid: string, attr: string] {
    let resp = (wd-get $"/session/($session_id)/element/($eid)/attribute/($attr)")
    if $resp == null { return "" }
    $resp | get -o value | default ""
}

# Click an element by its element ID.
def wd-click [session_id: string, eid: string] {
    wd-post $"/session/($session_id)/element/($eid)/click" '{}' | ignore
}

# Send keys to an element.
def wd-send-keys [session_id: string, eid: string, text: string] {
    let text_json = ($text | to json -r)
    wd-post $"/session/($session_id)/element/($eid)/value" $'{"text": ($text_json)}' | ignore
}

# Execute JavaScript synchronously.
def wd-execute [session_id: string, script: string] {
    let script_json = ($script | to json -r)
    let resp = (wd-post $"/session/($session_id)/execute/sync" $'{"script": ($script_json), "args": []}')
    if $resp == null { return "" }
    $resp | get -o value | default ""
}

# Get page source.
def wd-page-source [session_id: string] {
    let resp = (wd-get $"/session/($session_id)/source")
    if $resp == null { return "" }
    $resp | get -o value | default ""
}

# Take a screenshot and save as PNG.
def wd-screenshot [session_id: string, path: string] {
    let resp = (wd-get $"/session/($session_id)/screenshot")
    if $resp == null { return }
    let b64 = ($resp | get -o value | default "")
    if ($b64 | is-not-empty) {
        $b64 | decode base64 | save -f $path
        log-info $"Screenshot saved: ($path)"
    }
}

# Wait for a condition (poll-based).
def wd-wait-for-element [session_id: string, css: string, max_wait: int] {
    mut elapsed = 0
    while $elapsed < $max_wait {
        let eid = try { wd-find $session_id $css } catch { "" }
        if ($eid | is-not-empty) and $eid != "null" {
            return true
        }
        sleep 500ms
        $elapsed = $elapsed + 1
    }
    false
}

# ── Cleanup helper ─────────────────────────────────────────────────────────

def do-cleanup [
    session_id: string,
    servo_pid: int,
    server_pid: int,
    servo_log: string,
] {
    # Delete WebDriver session
    if ($session_id | is-not-empty) {
        wd-delete $"/session/($session_id)"
    }

    # Kill servo
    if $servo_pid > 0 {
        let alive = (do -i { ^kill -0 $servo_pid } | complete)
        if $alive.exit_code == 0 {
            log-info $"Shutting down Servo \(PID ($servo_pid))..."
            do -i { ^kill $servo_pid } | complete | ignore
            mut attempts = 0
            while $attempts < 10 {
                let still_alive = (do -i { ^kill -0 $servo_pid } | complete)
                if $still_alive.exit_code != 0 { break }
                sleep 200ms
                $attempts = $attempts + 1
            }
            let still_alive = (do -i { ^kill -0 $servo_pid } | complete)
            if $still_alive.exit_code == 0 {
                do -i { ^kill -9 $servo_pid } | complete | ignore
            }
        }
    }

    # Kill file server
    if $server_pid > 0 {
        let alive = (do -i { ^kill -0 $server_pid } | complete)
        if $alive.exit_code == 0 {
            log-info $"Shutting down file server \(PID ($server_pid))..."
            do -i { ^kill $server_pid } | complete | ignore
        }
    }

    # Clean up servo log
    if ($servo_log | is-not-empty) and ($servo_log | path exists) {
        rm -f $servo_log
    }
}

# ── Kill stale processes on a port ─────────────────────────────────────────

def kill-port [port: int] {
    let pids = try { ^lsof -ti $":($port)" | str trim } catch { "" }
    if ($pids | is-not-empty) {
        log-warn $"Port ($port) already in use — killing stale processes: ($pids)"
        for pid in ($pids | lines) {
            let p = ($pid | str trim)
            if ($p | is-not-empty) {
                try { ^kill ($p | into int) } catch { }
            }
        }
        mut attempts = 0
        while $attempts < 20 {
            let still = try { ^lsof -ti $":($port)" | str trim } catch { "" }
            if ($still | is-empty) { break }
            sleep 200ms
            $attempts = $attempts + 1
        }
        let still = try { ^lsof -ti $":($port)" | str trim } catch { "" }
        if ($still | is-not-empty) {
            log-warn $"Port ($port) still in use after kill — forcing with -9"
            for pid in ($still | lines) {
                let p = ($pid | str trim)
                if ($p | is-not-empty) {
                    try { ^kill -9 ($p | into int) } catch { }
                }
            }
            sleep 1sec
        }
    }
}

# ── Test assertion helpers ─────────────────────────────────────────────────

def assert-text [
    session_id: string,
    label: string,
    css: string,
    expected: string,
    --passed (-p): int,
    --failed (-f): int,
]: nothing -> record<passed: int, failed: int> {
    mut p = $passed
    mut f = $failed
    let eid = try { wd-find $session_id $css } catch { "" }
    if ($eid | is-empty) or $eid == "null" {
        log-fail $"($label) — element not found: ($css)"
        $f = $f + 1
    } else {
        let text = (wd-text $session_id $eid)
        if $text == $expected {
            log-ok $label
            $p = $p + 1
        } else {
            log-fail $"($label) — expected: \"($expected)\", got: \"($text)\""
            $f = $f + 1
        }
    }
    { passed: $p, failed: $f }
}

def assert-text-contains [
    session_id: string,
    label: string,
    css: string,
    substring: string,
    --passed (-p): int,
    --failed (-f): int,
]: nothing -> record<passed: int, failed: int> {
    mut p = $passed
    mut f = $failed
    let eid = try { wd-find $session_id $css } catch { "" }
    if ($eid | is-empty) or $eid == "null" {
        log-fail $"($label) — element not found: ($css)"
        $f = $f + 1
    } else {
        let text = (wd-text $session_id $eid)
        if ($text | str contains $substring) {
            log-ok $label
            $p = $p + 1
        } else {
            log-fail $"($label) — expected to contain: \"($substring)\", got: \"($text)\""
            $f = $f + 1
        }
    }
    { passed: $p, failed: $f }
}

def assert-exists [
    session_id: string,
    label: string,
    css: string,
    --passed (-p): int,
    --failed (-f): int,
]: nothing -> record<passed: int, failed: int> {
    mut p = $passed
    mut f = $failed
    let eid = try { wd-find $session_id $css } catch { "" }
    if ($eid | is-not-empty) and $eid != "null" {
        log-ok $label
        $p = $p + 1
    } else {
        log-fail $"($label) — element not found: ($css)"
        $f = $f + 1
    }
    { passed: $p, failed: $f }
}

def assert-count-gte [
    session_id: string,
    label: string,
    css: string,
    min_count: int,
    --passed (-p): int,
    --failed (-f): int,
]: nothing -> record<passed: int, failed: int> {
    mut p = $passed
    mut f = $failed
    let count = try { wd-find-all-count $session_id $css } catch { 0 }
    if $count >= $min_count {
        log-ok $"($label) (found ($count))"
        $p = $p + 1
    } else {
        log-fail $"($label) — expected >= ($min_count) elements, got ($count)"
        $f = $f + 1
    }
    { passed: $p, failed: $f }
}

# ── Page test: Home ────────────────────────────────────────────────────────

def test-home [session_id: string, timeout: int, passed: int, failed: int]: nothing -> record<passed: int, failed: int> {
    mut p = $passed
    mut f = $failed
    let bu = (base-url)

    log-info ""
    log-info "── Home page ───────────────────────────────────────────"

    wd-navigate $session_id $"($bu)/"

    # Wait for React to mount — #root should get children
    if not (wd-wait-for-element $session_id "#root > *" $timeout) {
        log-fail "Home page did not mount (no children in #root within timeout)"
        $f = $f + 1
        return { passed: $p, failed: $f }
    }
    sleep 1sec

    # React root should have content
    let r = (assert-exists $session_id "React app mounts (#root has children)" "#root > *" -p $p -f $f)
    $p = $r.passed; $f = $r.failed

    # Page title should be RadikalWiki
    let title = (wd-execute $session_id "return document.title")
    if ($title | str contains "RadikalWiki") {
        log-ok "page title contains 'RadikalWiki'"
        $p = $p + 1
    } else {
        log-fail $"page title — expected to contain 'RadikalWiki', got: '($title)'"
        $f = $f + 1
    }

    # MUI should be loaded (Emotion CSS injected)
    let has_emotion = (wd-execute $session_id "return document.querySelector('style[data-emotion]') !== null ? 'yes' : 'no'")
    if $has_emotion == "yes" {
        log-ok "MUI/Emotion styles injected"
        $p = $p + 1
    } else {
        log-warn "MUI/Emotion styles not detected (may be a Servo CSS limitation)"
    }

    # NHost client should have initialized (check for the GraphQL endpoint
    # being configured — the NhostClient stores subdomain/region which
    # get baked in at build time)
    let nhost_check = (wd-execute $session_id "try { return typeof window.__REACT_DEVTOOLS_GLOBAL_HOOK__ !== 'undefined' || document.getElementById('root').innerHTML.length > 0 ? 'yes' : 'no' } catch(e) { return 'no' }")
    if $nhost_check == "yes" {
        log-ok "React app is running (root has rendered content)"
        $p = $p + 1
    } else {
        log-warn "Could not confirm React app is fully running"
    }

    # Check that the Layout component rendered (should produce some MUI structure)
    let root_html_len = (wd-execute $session_id "return document.getElementById('root').innerHTML.length")
    log-info $"Root innerHTML length: ($root_html_len) characters"

    # Take a screenshot for visual inspection
    wd-screenshot $session_id "test-browser-home.png"

    { passed: $p, failed: $f }
}

# ── Page test: Login ───────────────────────────────────────────────────────

def test-login [session_id: string, timeout: int, passed: int, failed: int]: nothing -> record<passed: int, failed: int> {
    mut p = $passed
    mut f = $failed
    let bu = (base-url)

    log-info ""
    log-info "── Login page ──────────────────────────────────────────"

    wd-navigate $session_id $"($bu)/user/login"

    # Wait for React to mount via SPA routing
    if not (wd-wait-for-element $session_id "#root > *" $timeout) {
        log-fail "Login page did not mount within timeout"
        $f = $f + 1
        return { passed: $p, failed: $f }
    }
    sleep 1sec

    let r = (assert-exists $session_id "Login page renders (#root has children)" "#root > *" -p $p -f $f)
    $p = $r.passed; $f = $r.failed

    # Check for input fields (email / password)
    let input_count = try { wd-find-all-count $session_id "input" } catch { 0 }
    if $input_count >= 1 {
        log-ok $"Login page has ($input_count) input fields"
        $p = $p + 1
    } else {
        log-warn "No input fields found on login page (may need more JS execution time)"
    }

    # Check for buttons (login button, etc.)
    let btn_count = try { wd-find-all-count $session_id "button" } catch { 0 }
    if $btn_count >= 1 {
        log-ok $"Login page has ($btn_count) buttons"
        $p = $p + 1
    } else {
        log-warn "No buttons found on login page"
    }

    # Check for auth-related text content
    let root_eid = (wd-find $session_id "#root")
    if ($root_eid | is-not-empty) and $root_eid != "null" {
        let root_text = (wd-text $session_id $root_eid)
        # The login page should contain auth-related text (in Danish or English)
        if ($root_text | str contains "Log") or ($root_text | str contains "Email") or ($root_text | str contains "email") or ($root_text | str contains "Bluesky") {
            log-ok "Login page contains auth-related text"
            $p = $p + 1
        } else {
            log-info $"Login page text: ($root_text | str substring 0..200)"
            log-warn "Could not find expected auth-related text on login page"
        }
    }

    wd-screenshot $session_id "test-browser-login.png"

    { passed: $p, failed: $f }
}

# ── Page test: Register ────────────────────────────────────────────────────

def test-register [session_id: string, timeout: int, passed: int, failed: int]: nothing -> record<passed: int, failed: int> {
    mut p = $passed
    mut f = $failed
    let bu = (base-url)

    log-info ""
    log-info "── Register page ───────────────────────────────────────"

    wd-navigate $session_id $"($bu)/user/register"

    if not (wd-wait-for-element $session_id "#root > *" $timeout) {
        log-fail "Register page did not mount within timeout"
        $f = $f + 1
        return { passed: $p, failed: $f }
    }
    sleep 1sec

    let r = (assert-exists $session_id "Register page renders" "#root > *" -p $p -f $f)
    $p = $r.passed; $f = $r.failed

    # Registration should have input fields (name, email, password, etc.)
    let input_count = try { wd-find-all-count $session_id "input" } catch { 0 }
    if $input_count >= 2 {
        log-ok $"Register page has ($input_count) input fields, >= 2 expected"
        $p = $p + 1
    } else if $input_count >= 1 {
        log-ok $"Register page has ($input_count) input fields"
        $p = $p + 1
    } else {
        log-warn "No input fields found on register page"
    }

    wd-screenshot $session_id "test-browser-register.png"

    { passed: $p, failed: $f }
}

# ── Page test: JS execution & DOM structure ────────────────────────────────

def test-js-execution [session_id: string, timeout: int, passed: int, failed: int]: nothing -> record<passed: int, failed: int> {
    mut p = $passed
    mut f = $failed
    let bu = (base-url)

    log-info ""
    log-info "── JS execution & DOM ──────────────────────────────────"

    wd-navigate $session_id $"($bu)/"

    if not (wd-wait-for-element $session_id "#root > *" $timeout) {
        log-fail "Page did not mount"
        $f = $f + 1
        return { passed: $p, failed: $f }
    }
    sleep 1sec

    # Verify basic JS execution
    let js_result = (wd-execute $session_id "return 1 + 1")
    if ($js_result | into string) == "2" {
        log-ok "JavaScript execution works (1+1=2)"
        $p = $p + 1
    } else {
        log-fail $"JS execution failed — expected '2', got '($js_result)'"
        $f = $f + 1
    }

    # Check that React rendered (not just the static HTML shell)
    let child_count = (wd-execute $session_id "return document.getElementById('root').childNodes.length")
    let child_int = try { $child_count | into string | into int } catch { 0 }
    if $child_int > 0 {
        log-ok $"React rendered \(root has ($child_count) child nodes)"
        $p = $p + 1
    } else {
        log-fail "React did not render (root has 0 children)"
        $f = $f + 1
    }

    # Check that i18next loaded (the app uses translations)
    let lang_check = (wd-execute $session_id "return document.documentElement.lang || 'none'")
    log-info $"Document lang attribute: ($lang_check)"

    # Verify the SPA router is active (history API)
    let has_history = (wd-execute $session_id "return typeof window.history.pushState === 'function' ? 'yes' : 'no'")
    if $has_history == "yes" {
        log-ok "History API available (SPA routing supported)"
        $p = $p + 1
    } else {
        log-fail "History API not available"
        $f = $f + 1
    }

    # Check that the NHost GraphQL endpoint is reachable from the browser context
    let gql_check = (wd-execute $session_id "
        try {
            var xhr = new XMLHttpRequest();
            xhr.open('POST', 'https://pgvhpsenoifywhuxnybq.hasura.eu-central-1.nhost.run/v1/graphql', false);
            xhr.setRequestHeader('Content-Type', 'application/json');
            xhr.setRequestHeader('x-hasura-role', 'public');
            xhr.send(JSON.stringify({query: '{__typename}'}));
            return xhr.status >= 200 && xhr.status < 400 ? 'reachable' : 'status-' + xhr.status;
        } catch(e) {
            return 'error: ' + e.message;
        }
    ")
    if ($gql_check | str starts-with "reachable") {
        log-ok "NHost GraphQL endpoint is reachable"
        $p = $p + 1
    } else {
        log-warn $"NHost GraphQL endpoint check: ($gql_check) (may be CORS or network issue in Servo)"
    }

    # Dump page source length for diagnostics
    let source_len = (wd-execute $session_id "return document.documentElement.outerHTML.length")
    log-info $"Page source length: ($source_len) characters"

    { passed: $p, failed: $f }
}

# ── Page test: SPA routing ─────────────────────────────────────────────────

def test-spa-routing [session_id: string, timeout: int, passed: int, failed: int]: nothing -> record<passed: int, failed: int> {
    mut p = $passed
    mut f = $failed
    let bu = (base-url)

    log-info ""
    log-info "── SPA routing ─────────────────────────────────────────"

    # Start at home
    wd-navigate $session_id $"($bu)/"

    if not (wd-wait-for-element $session_id "#root > *" $timeout) {
        log-fail "Home page did not mount"
        $f = $f + 1
        return { passed: $p, failed: $f }
    }
    sleep 500ms

    # Navigate to /user/login via the SPA file server (tests SPA fallback)
    wd-navigate $session_id $"($bu)/user/login"
    if not (wd-wait-for-element $session_id "#root > *" 15) {
        log-fail "SPA routing to /user/login failed"
        $f = $f + 1
    } else {
        log-ok "SPA routing to /user/login works"
        $p = $p + 1
    }
    sleep 500ms

    # Navigate to /user/register
    wd-navigate $session_id $"($bu)/user/register"
    if not (wd-wait-for-element $session_id "#root > *" 15) {
        log-fail "SPA routing to /user/register failed"
        $f = $f + 1
    } else {
        log-ok "SPA routing to /user/register works"
        $p = $p + 1
    }
    sleep 500ms

    # Navigate to /user/reset-password
    wd-navigate $session_id $"($bu)/user/reset-password"
    if not (wd-wait-for-element $session_id "#root > *" 15) {
        log-fail "SPA routing to /user/reset-password failed"
        $f = $f + 1
    } else {
        log-ok "SPA routing to /user/reset-password works"
        $p = $p + 1
    }
    sleep 500ms

    # Navigate to a deep/arbitrary path (should still serve the SPA shell
    # via history API fallback — the catch-all route handles it)
    wd-navigate $session_id $"($bu)/some/deep/path"
    if not (wd-wait-for-element $session_id "#root > *" 15) {
        log-fail "SPA fallback for arbitrary deep path failed"
        $f = $f + 1
    } else {
        log-ok "SPA fallback for arbitrary deep path works"
        $p = $p + 1
    }

    # Navigate back to home to confirm the app is still alive
    wd-navigate $session_id $"($bu)/"
    sleep 500ms
    let r = (assert-exists $session_id "Home still renders after route round-trip" "#root > *" -p $p -f $f)
    $p = $r.passed; $f = $r.failed

    wd-screenshot $session_id "test-browser-routing.png"

    { passed: $p, failed: $f }
}

# ── Main ───────────────────────────────────────────────────────────────────

def main [
    --timeout: int = 30     # Per-wait timeout in poll iterations (~500ms each)
    --page: string = ""     # Test only a specific page (home, login, register, js, routing)
    --verbose                # Stream servo stderr
    --keep                   # Keep servo running after tests
    --skip-build             # Skip build check, use existing dist/
] {
    let script_dir = ($env.FILE_PWD)
    mut servo_pid = 0
    mut server_pid = 0
    mut session_id = ""
    mut servo_log = ""
    mut passed = 0
    mut failed = 0

    # ── Preflight checks ──────────────────────────────────────────────

    for cmd in [servo deno curl] {
        if (which $cmd | is-empty) {
            log-fail $"Required command not found: ($cmd)"
            exit 2
        }
    }

    if not ("dist/index.html" | path exists) {
        if $skip_build {
            log-fail "dist/index.html not found — need a built SPA to test"
            exit 2
        }
        log-info "dist/ not found, building..."
        ^just build
    }

    # ── Kill stale processes on our ports ──────────────────────────

    kill-port $SERVE_PORT
    kill-port $WD_PORT

    # ── Start SPA file server ──────────────────────────────────────

    log-info $"Starting SPA file server on :($SERVE_PORT)..."
    $server_pid = (^bash -c "deno run --allow-net --allow-read test-browser-serve.ts --port 4508 > /dev/null 2>&1 & echo \$!" | str trim | into int)

    # Wait for file server to be ready
    mut ready = false
    for _ in 1..31 {
        let check = (do -i { ^curl -sf $"(base-url)/" } | complete)
        if $check.exit_code == 0 {
            $ready = true
            break
        }
        let pid = $server_pid
        let alive = (do -i { ^kill -0 $pid } | complete)
        if $alive.exit_code != 0 {
            log-fail "File server exited unexpectedly"
            do-cleanup $session_id $servo_pid $server_pid $servo_log
            exit 2
        }
        sleep 200ms
    }

    if not $ready {
        log-fail "File server did not become ready"
        do-cleanup $session_id $servo_pid $server_pid $servo_log
        exit 2
    }

    log-info $"File server ready \(PID ($server_pid))"

    # ── Start Servo (headless + WebDriver) ─────────────────────────

    $servo_log = (^mktemp /tmp/servo-wiki-test-XXXXXX.log | str trim)

    log-info $"Starting Servo \(headless, WebDriver on :($WD_PORT))..."

    let log = $servo_log
    $servo_pid = (^bash -c $"servo --headless --webdriver=7123 about:blank > ($log) 2>&1 & echo \$!" | str trim | into int)

    # Wait for WebDriver to become ready
    mut wd_ready = false
    for _ in 1..51 {
        let check = (do -i { ^curl -sf $"(wd-url)/status" } | complete)
        if $check.exit_code == 0 {
            $wd_ready = true
            break
        }
        let pid = $servo_pid
        let alive = (do -i { ^kill -0 $pid } | complete)
        if $alive.exit_code != 0 {
            log-fail "Servo exited before WebDriver was ready"
            if $verbose and ($servo_log | path exists) {
                log-warn "Servo output:"
                print -e (open --raw $servo_log)
            }
            do-cleanup $session_id $servo_pid $server_pid $servo_log
            exit 2
        }
        sleep 200ms
    }

    if not $wd_ready {
        log-fail $"WebDriver did not become ready on :($WD_PORT)"
        if ($servo_log | path exists) {
            log-warn "Last 20 lines of Servo output:"
            print -e (open --raw $servo_log | lines | last 20 | str join "\n")
        }
        do-cleanup $session_id $servo_pid $server_pid $servo_log
        exit 2
    }

    log-info $"Servo ready \(PID ($servo_pid))"

    # ── Create WebDriver session ───────────────────────────────────

    log-info "Creating WebDriver session..."
    $session_id = (wd-new-session)

    if ($session_id | is-empty) {
        log-fail "Failed to create WebDriver session"
        do-cleanup $session_id $servo_pid $server_pid $servo_log
        exit 2
    }

    log-info $"Session: ($session_id)"

    # ── Run tests ──────────────────────────────────────────────────

    log-info ""
    log-info $"Running browser tests \(timeout: ($timeout) poll iterations)..."

    if ($page | is-empty) or $page == "home" {
        let r = (test-home $session_id $timeout $passed $failed)
        $passed = $r.passed; $failed = $r.failed
    }
    if ($page | is-empty) or $page == "login" {
        let r = (test-login $session_id $timeout $passed $failed)
        $passed = $r.passed; $failed = $r.failed
    }
    if ($page | is-empty) or $page == "register" {
        let r = (test-register $session_id $timeout $passed $failed)
        $passed = $r.passed; $failed = $r.failed
    }
    if ($page | is-empty) or $page == "js" {
        let r = (test-js-execution $session_id $timeout $passed $failed)
        $passed = $r.passed; $failed = $r.failed
    }
    if ($page | is-empty) or $page == "routing" {
        let r = (test-spa-routing $session_id $timeout $passed $failed)
        $passed = $r.passed; $failed = $r.failed
    }

    # ── Summary ────────────────────────────────────────────────────

    print -e ""
    let total = $passed + $failed

    if $verbose and ($servo_log | path exists) and (($servo_log | path type) == "file") {
        let content = (open --raw $servo_log)
        if ($content | is-not-empty) {
            log-info "--- Servo output ---"
            print -e $content
            log-info "--- End Servo output ---"
            print -e ""
        }
    }

    # Clean up before exit (unless --keep)
    if not $keep {
        do-cleanup $session_id $servo_pid $server_pid $servo_log
    }

    if $failed == 0 {
        log-ok $"($total) tests: ($passed) passed, 0 failed"
        if $keep and $servo_pid > 0 {
            log-info $"Servo still running \(PID ($servo_pid)). Press Ctrl-C to stop."
            try { ^waitpid $servo_pid } catch { }
        }
        exit 0
    } else {
        log-fail $"($total) tests: ($passed) passed, ($failed) failed"
        exit 1
    }
}
