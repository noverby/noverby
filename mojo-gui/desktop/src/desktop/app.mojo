"""DesktopApp — Desktop entry point for mojo-gui applications.

This module provides the `DesktopApp` struct that orchestrates:
  1. A webview window (via the libmojo_webview C shim)
  2. The mojo-gui/core reactive framework (AppShell, signals, vdom, diff)
  3. The desktop bridge (mutation buffer + event polling)

Architecture:

    ┌─ Native Mojo Process ─────────────────────────────────────────────┐
    │                                                                    │
    │  User App Code (counter.mojo, todo.mojo, ...)                     │
    │      │                                                             │
    │      ▼                                                             │
    │  DesktopApp                                                        │
    │      │                                                             │
    │      ├── AppShell (mojo-gui/core)                                  │
    │      │     ├── Runtime (signals, memos, effects)                   │
    │      │     ├── HandlerRegistry (event dispatch)                    │
    │      │     ├── Scheduler (dirty scope queue)                       │
    │      │     └── MutationWriter → heap buffer                        │
    │      │                                                             │
    │      ├── DesktopBridge                                             │
    │      │     ├── Owns heap mutation buffer                           │
    │      │     ├── flush_mutations() → webview.apply_mutations()       │
    │      │     └── poll_event() → parse JSON → DesktopEvent            │
    │      │                                                             │
    │      └── Webview (GTK4 + WebKitGTK via C shim)                    │
    │            ├── desktop-runtime.js (Interpreter + EventBridge)       │
    │            ├── shell.html (#root mount point)                       │
    │            └── mojo_post() → ring buffer → poll_event()            │
    │                                                                    │
    └────────────────────────────────────────────────────────────────────┘

Event loop (non-blocking cooperative model):

    while webview.is_alive():
        # 1. Process GTK events (non-blocking)
        closed = webview.step(blocking=False)
        if closed: break

        # 2. Drain all JS events from the ring buffer
        while True:
            event = bridge.poll_event()
            if not event.is_valid(): break
            app_shell.handler_registry.dispatch(event.handler_id, event.event_type)

        # 3. If any scopes are dirty, re-render and flush mutations
        if app_shell.scheduler.has_dirty():
            app_shell.flush(writer)
            bridge.flush_mutations(writer.offset)
            writer.reset()

        # 4. If no events and no dirty scopes, block briefly to avoid busy-wait
        if not had_events and not had_dirty:
            webview.step(blocking=True)  # wait for next GTK event

Usage:

    from desktop.app import DesktopApp

    fn main() raises:
        var app = DesktopApp(
            title="Counter",
            width=800,
            height=600,
        )
        app.run()
"""

from memory import UnsafePointer, memset_zero, alloc
from pathlib import Path
from os import getenv

from .webview import Webview, MWV_HINT_NONE
from .bridge import DesktopBridge, DesktopEvent, DEFAULT_BUF_CAPACITY


# ── Constants ─────────────────────────────────────────────────────────────

comptime IDLE_POLL_INTERVAL_MS = 16
"""Approximate interval (ms) for idle polling when no events are pending.
Roughly 60 fps — prevents busy-waiting while keeping the UI responsive.
"""


# ── Shell HTML ────────────────────────────────────────────────────────────

# The HTML shell is loaded into the webview as the initial content.
# It provides the #root mount point for the mojo-gui virtual DOM.
# This is a minimal inline version; the full version lives in
# runtime/shell.html.
comptime SHELL_HTML = String(
    '<!DOCTYPE html>'
    '<html lang="en">'
    "<head>"
    '<meta charset="UTF-8">'
    '<meta name="viewport" content="width=device-width, initial-scale=1.0">'
    "<title>mojo-gui</title>"
    "<style>"
    "*,*::before,*::after{box-sizing:border-box;margin:0;padding:0}"
    "html,body{width:100%;height:100%;overflow:hidden;"
    'font-family:system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;'
    "font-size:14px;line-height:1.5;color:#1a1a1a;background:#fff;"
    "-webkit-font-smoothing:antialiased}"
    "#root{width:100%;height:100%;overflow:auto}"
    "@media(prefers-color-scheme:dark)"
    "{html,body{color:#e0e0e0;background:#1a1a1a}}"
    "</style>"
    "</head>"
    '<body><div id="root"></div></body>'
    "</html>"
)


# ── DesktopApp ────────────────────────────────────────────────────────────


struct DesktopApp:
    """Desktop GUI application backed by a GTK4/WebKitGTK webview.

    DesktopApp creates a native window with an embedded webview, injects
    the mojo-gui desktop JS runtime, and runs a cooperative event loop
    that bridges the core reactive framework with the webview's DOM.

    The mutation buffer is heap-allocated (not WASM linear memory) and
    sent to the webview via base64-encoded IPC on each render cycle.

    Example:

        fn main() raises:
            var app = DesktopApp(title="Hello", width=400, height=300)

            # Inject custom JS or load custom HTML if needed:
            # app.eval_js("console.log('hello from Mojo!');")

            # Run the event loop (blocks until window is closed):
            app.run()
    """

    var _webview: UnsafePointer[Webview, MutExternalOrigin]
    var _bridge: UnsafePointer[DesktopBridge, MutExternalOrigin]
    var _title: String
    var _width: Int
    var _height: Int
    var _debug: Bool
    var _initialized: Bool
    var _js_runtime: String

    fn __init__(
        out self,
        title: String = "mojo-gui",
        width: Int = 800,
        height: Int = 600,
        debug: Bool = False,
        buf_capacity: Int = DEFAULT_BUF_CAPACITY,
    ) raises:
        """Create a new desktop application.

        Args:
            title: Window title.
            width: Initial window width in pixels.
            height: Initial window height in pixels.
            debug: Enable WebKitGTK developer tools / inspector.
            buf_capacity: Mutation buffer capacity in bytes.
        """
        self._title = title
        self._width = width
        self._height = height
        self._debug = debug
        self._initialized = False
        self._js_runtime = String("")

        # Allocate the Webview on the heap so we can pass a stable pointer
        # to the DesktopBridge.
        self._webview = alloc[Webview](1)
        self._webview.init_pointee_move(
            Webview(title, width, height, debug)
        )

        # Allocate the DesktopBridge on the heap.
        self._bridge = alloc[DesktopBridge](1)
        self._bridge.init_pointee_move(
            DesktopBridge(self._webview, buf_capacity)
        )

    fn __del__(deinit self):
        """Clean up resources."""
        if self._bridge:
            self._bridge.destroy_pointee()
            self._bridge.free()
        if self._webview:
            self._webview.destroy_pointee()
            self._webview.free()

    # ── Initialization ────────────────────────────────────────────────

    fn _load_js_runtime(mut self) raises:
        """Load the desktop-runtime.js content.

        Search order:
          1. MOJO_GUI_DESKTOP_RUNTIME env var (explicit file path)
          2. Relative path: ../runtime/desktop-runtime.js (from src/)
          3. Relative path: runtime/desktop-runtime.js (from desktop/)
          4. Bundled inline fallback (minimal version)

        The JS runtime must be loaded before any mutations are sent.
        """
        # Check env var first.
        var env_path = getenv("MOJO_GUI_DESKTOP_RUNTIME", "")
        if env_path:
            try:
                self._js_runtime = Path(env_path).read_text()
                return
            except:
                pass

        # Try relative paths.
        var candidates = List[String]()
        candidates.append("runtime/desktop-runtime.js")
        candidates.append("../runtime/desktop-runtime.js")
        candidates.append("desktop/runtime/desktop-runtime.js")
        candidates.append("mojo-gui/desktop/runtime/desktop-runtime.js")

        for i in range(len(candidates)):
            try:
                self._js_runtime = Path(candidates[i]).read_text()
                return
            except:
                pass

        # If we can't find the JS file, raise an error with helpful message.
        raise Error(
            "Could not find desktop-runtime.js. Set MOJO_GUI_DESKTOP_RUNTIME"
            " to the full path, or run from the mojo-gui/desktop/ directory."
        )

    fn _inject_runtime(mut self) raises:
        """Initialize the webview with the HTML shell and JS runtime.

        This must be called before any mutations are sent.
        The order is:
          1. Inject the JS runtime via mwv_init() (runs on every page load)
          2. Load the HTML shell via mwv_set_html()
          3. Wait for the runtime to auto-initialize on DOMContentLoaded
        """
        if self._initialized:
            return

        # Load the JS runtime source.
        self._load_js_runtime()

        # Inject the runtime JS to run on every page load.
        self._webview[].init_js(self._js_runtime)

        # Load the HTML shell. The runtime will auto-init when
        # DOMContentLoaded fires.
        self._webview[].set_html(SHELL_HTML)

        # Pump a few GTK iterations to let the page load and the
        # runtime initialize.
        for _ in range(20):
            _ = self._webview[].step(blocking=False)

        self._initialized = True

    # ── Public API ────────────────────────────────────────────────────

    fn webview(self) -> UnsafePointer[Webview]:
        """Return a pointer to the underlying Webview.

        Use this for direct webview manipulation (eval_js, set_html, etc.).
        """
        return self._webview

    fn bridge(self) -> UnsafePointer[DesktopBridge]:
        """Return a pointer to the DesktopBridge.

        Use this to access the mutation buffer and event polling.
        """
        return self._bridge

    fn buf_ptr(self) -> UnsafePointer[UInt8, MutExternalOrigin]:
        """Return a pointer to the mutation buffer.

        This is a convenience method for constructing a MutationWriter:

            var buf = app.buf_ptr()
            # Cast to MutExternalOrigin for MutationWriter compatibility.
        """
        return self._bridge[].buf_ptr()

    fn buf_capacity(self) -> Int:
        """Return the mutation buffer capacity in bytes."""
        return self._bridge[].capacity()

    fn is_alive(self) raises -> Bool:
        """Return True if the window is still open."""
        return self._webview[].is_alive()

    fn eval_js(self, js: String) raises:
        """Evaluate JavaScript in the webview.

        Can be used for custom app logic, debugging, etc.
        """
        self._webview[].eval_js(js)

    fn set_title(self, title: String) raises:
        """Update the window title."""
        self._webview[].set_title(title)

    fn flush_mutations(self, byte_length: Int) raises:
        """Send mutations from the buffer to the webview.

        Args:
            byte_length: Number of bytes written by MutationWriter.
        """
        self._bridge[].flush_mutations(byte_length)

    fn poll_event(self) raises -> DesktopEvent:
        """Poll for the next event from the webview.

        Returns:
            A DesktopEvent. Check .is_valid() for availability.
        """
        return self._bridge[].poll_event()

    fn step(self, blocking: Bool = False) raises -> Bool:
        """Run a single iteration of the GTK event loop.

        Args:
            blocking: If True, block until an event is processed.

        Returns:
            True if the window should close.
        """
        return self._webview[].step(blocking)

    # ── Event loop ────────────────────────────────────────────────────

    fn init(mut self) raises:
        """Initialize the webview with the HTML shell and JS runtime.

        Call this before run() or before sending any mutations.
        Called automatically by run() if not already initialized.
        """
        self._inject_runtime()

    fn run(mut self) raises:
        """Run the desktop application event loop.

        This is a simple blocking event loop suitable for applications
        that don't need custom per-frame logic. It:
          1. Initializes the webview (if not already done)
          2. Blocks on GTK events until the window is closed

        For applications that need custom event handling (e.g. polling
        events and dispatching to AppShell), use the step-based API:

            app.init()
            while app.is_alive():
                if app.step(blocking=False):
                    break
                var event = app.poll_event()
                # ... handle event, flush mutations, etc.
        """
        if not self._initialized:
            self._inject_runtime()

        # Simple blocking event loop.
        self._webview[].run()

    fn run_with_mount(mut self, mount_len: Int) raises:
        """Initialize, apply initial mount mutations, then run the event loop.

        This is the typical desktop app flow:
          1. Core framework renders the initial DOM into the mutation buffer
          2. We send those mutations to the webview
          3. Then run the interactive event loop

        Args:
            mutation_buf: Pointer to the mutation buffer (must be the bridge's buffer).
            mount_len: Number of bytes of initial mount mutations.
        """
        if not self._initialized:
            self._inject_runtime()

        # Apply the initial mount mutations.
        if mount_len > 0:
            self._bridge[].flush_mutations(mount_len)

        # Pump a few frames to let the DOM render.
        for _ in range(5):
            _ = self._webview[].step(blocking=False)

        # Run the blocking event loop.
        self._webview[].run()

    fn run_interactive(mut self) raises:
        """Run a non-blocking cooperative event loop.

        This loop:
          1. Processes GTK events (non-blocking)
          2. Drains all JS events from the webview
          3. Yields control back to the caller via the return

        For a fully custom loop, use step() + poll_event() directly.
        This method is a convenience that handles the common pattern
        of draining events each frame.

        Returns when the window is closed.
        """
        if not self._initialized:
            self._inject_runtime()

        while True:
            # Process GTK events.
            var closed = self._webview[].step(blocking=False)
            if closed:
                break

            # Check if the window is still alive.
            if not self._webview[].is_alive():
                break

            # Drain events (caller should override or hook into this).
            var had_event = False
            while True:
                var event = self._bridge[].poll_event()
                if not event.is_valid():
                    break
                had_event = True
                # Events are available but we have no AppShell reference
                # here — the caller should use the step-based API for
                # full control. This loop just drains and discards events.

            # If no events, do a brief blocking wait to avoid busy-loop.
            if not had_event:
                _ = self._webview[].step(blocking=True)

    # ── Lifecycle ─────────────────────────────────────────────────────

    fn destroy(mut self) raises:
        """Explicitly destroy the application, webview, and bridge.

        After calling this, the DesktopApp is no longer usable.
        This is called automatically by __del__ but can be invoked
        earlier for explicit resource management.
        """
        if self._bridge:
            self._bridge[].shutdown()
        if self._webview:
            self._webview[].destroy()
        self._initialized = False
