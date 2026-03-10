"""Mojo FFI bindings for the libmojo_webview C shim.

This module loads libmojo_webview.so via DLHandle and provides typed wrappers
around the C API functions defined in mojo_webview.h.

The library uses a polling model instead of callbacks: JS sends events via
`window.mojo_post(msg)` which are buffered in a ring buffer on the C side,
and Mojo polls them with `poll_event()`.

Architecture:

    Mojo (native)                    Webview (GTK4 + WebKitGTK)
    ┌──────────────┐                 ┌──────────────────────────┐
    │  mojo-gui/   │  mwv_eval()     │  JS Interpreter          │
    │  core        │ ──────────────► │  (mutation interpreter   │
    │              │                 │   + event bridge)         │
    │              │  mwv_poll_event  │                          │
    │              │ ◄────────────── │  window.mojo_post(json)  │
    └──────────────┘                 └──────────────────────────┘

Usage:

    from desktop.webview import Webview

    fn main() raises:
        var wv = Webview("My App", 800, 600)
        wv.set_html("<html><body><div id='root'></div></body></html>")
        wv.init("console.log('hello from injected JS');")

        while wv.is_alive():
            var closed = wv.step(blocking=False)
            if closed:
                break

            var event = wv.poll_event()
            if event:
                # process event JSON
                pass

        wv.destroy()
"""

from os import getenv
from sys.ffi import OwnedDLHandle
from memory import UnsafePointer, memcpy, alloc


# ── Size hint constants (must match mojo_webview.h) ───────────────────────

comptime MWV_HINT_NONE = 0
"""Width and height are default size."""

comptime MWV_HINT_MIN = 1
"""Width and height are minimum bounds."""

comptime MWV_HINT_MAX = 2
"""Width and height are maximum bounds."""

comptime MWV_HINT_FIXED = 3
"""Window size cannot be changed by the user."""

comptime MWV_EVENT_MAX_LEN = 4096
"""Maximum byte length of a single event's JSON payload."""

# ── Opaque handle type ────────────────────────────────────────────────────

comptime WebviewHandle = UnsafePointer[NoneType, MutExternalOrigin]
"""Opaque pointer to the C-side mwv_state_t (mwv_t in the header)."""

# ── Library loading ───────────────────────────────────────────────────────


fn _find_lib_in_nix_ldflags() raises -> OwnedDLHandle:
    """Search NIX_LDFLAGS for a -L directory containing libmojo_webview.so."""
    var flags = getenv("NIX_LDFLAGS", "")
    if not flags:
        raise Error("NIX_LDFLAGS not set")
    var parts = flags.split(" ")
    for i in range(len(parts)):
        var part = parts[i]
        if part.startswith("-L") and "mojo" in part:
            var dir_path = part[2:]
            var full = dir_path + "/libmojo_webview.so"
            try:
                return OwnedDLHandle(full)
            except:
                pass
    raise Error("libmojo_webview.so not found in NIX_LDFLAGS")


fn _find_lib_in_ld_library_path() raises -> OwnedDLHandle:
    """Search LD_LIBRARY_PATH for libmojo_webview.so."""
    var paths = getenv("LD_LIBRARY_PATH", "")
    if not paths:
        raise Error("LD_LIBRARY_PATH not set")
    var parts = paths.split(":")
    for i in range(len(parts)):
        var dir_path = parts[i]
        if not dir_path:
            continue
        var full = dir_path + "/libmojo_webview.so"
        try:
            return OwnedDLHandle(full)
        except:
            pass
    raise Error("libmojo_webview.so not found in LD_LIBRARY_PATH")


fn _find_lib_in_mojo_webview_lib() raises -> OwnedDLHandle:
    """Check MOJO_WEBVIEW_LIB env var for an explicit path."""
    var path = getenv("MOJO_WEBVIEW_LIB", "")
    if not path:
        raise Error("MOJO_WEBVIEW_LIB not set")
    return OwnedDLHandle(path)


fn _open_lib() raises -> OwnedDLHandle:
    """Open libmojo_webview.so, trying multiple search strategies.

    Search order:
      1. MOJO_WEBVIEW_LIB env var (explicit full path)
      2. Default dlopen search (LD_LIBRARY_PATH, /usr/lib, etc.)
      3. NIX_LDFLAGS -L directories
      4. LD_LIBRARY_PATH directory scan
    """
    # 1. Explicit path from env
    try:
        return _find_lib_in_mojo_webview_lib()
    except:
        pass

    # 2. Default dlopen search
    try:
        return OwnedDLHandle("libmojo_webview.so")
    except:
        pass

    # 3. Nix-specific search
    try:
        return _find_lib_in_nix_ldflags()
    except:
        pass

    # 4. Manual LD_LIBRARY_PATH scan
    try:
        return _find_lib_in_ld_library_path()
    except:
        pass

    raise Error(
        "Could not find libmojo_webview.so. Set MOJO_WEBVIEW_LIB to the full"
        " path, or ensure the library is on LD_LIBRARY_PATH."
    )


fn get_lib() raises -> OwnedDLHandle:
    """Return a handle to the mojo_webview shared library.

    The library is opened fresh each time (OwnedDLHandle is ref-counted by the OS).
    """
    return _open_lib()


# ── Low-level FFI wrappers ────────────────────────────────────────────────
#
# These are thin wrappers that call the C functions via DLHandle.
# They mirror the C API 1:1 for use by the high-level Webview struct.


fn _to_c_str(s: String) -> UnsafePointer[UInt8, MutExternalOrigin]:
    """Allocate a null-terminated C string copy of `s`.

    The caller is responsible for freeing the returned pointer.
    """
    var n = len(s)
    var ptr = alloc[UInt8](n + 1)
    var src = s.unsafe_ptr()
    for i in range(n):
        ptr[i] = src[i]
    ptr[n] = 0
    return ptr


fn mwv_create(
    title: String, width: Int32, height: Int32, debug: Int32
) raises -> WebviewHandle:
    """Create a new webview window."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (
            UnsafePointer[UInt8, MutExternalOrigin],
            Int32,
            Int32,
            Int32,
        ) -> WebviewHandle
    ]("mwv_create")
    var c_title = _to_c_str(title)
    var result = f(c_title, width, height, debug)
    c_title.free()
    if not result:
        raise Error("mwv_create returned NULL — window creation failed")
    return result


fn mwv_destroy(w: WebviewHandle) raises:
    """Destroy the webview and free all resources."""
    var lib = get_lib()
    var f = lib.get_function[fn (WebviewHandle) -> None]("mwv_destroy")
    f(w)


fn mwv_set_title(w: WebviewHandle, title: String) raises:
    """Set the window title."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (WebviewHandle, UnsafePointer[UInt8, MutExternalOrigin]) -> None
    ]("mwv_set_title")
    var c_title = _to_c_str(title)
    f(w, c_title)
    c_title.free()


fn mwv_set_size(
    w: WebviewHandle, width: Int32, height: Int32, hints: Int32
) raises:
    """Set the window size."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (WebviewHandle, Int32, Int32, Int32) -> None
    ]("mwv_set_size")
    f(w, width, height, hints)


fn mwv_navigate(w: WebviewHandle, url: String) raises:
    """Navigate the webview to a URL."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (WebviewHandle, UnsafePointer[UInt8, MutExternalOrigin]) -> None
    ]("mwv_navigate")
    var c_url = _to_c_str(url)
    f(w, c_url)
    c_url.free()


fn mwv_set_html(w: WebviewHandle, html: String) raises:
    """Set the webview content to the given HTML string."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (WebviewHandle, UnsafePointer[UInt8, MutExternalOrigin]) -> None
    ]("mwv_set_html")
    var c_html = _to_c_str(html)
    f(w, c_html)
    c_html.free()


fn mwv_init(w: WebviewHandle, js: String) raises:
    """Inject JavaScript to be executed on every new page load."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (WebviewHandle, UnsafePointer[UInt8, MutExternalOrigin]) -> None
    ]("mwv_init")
    var c_js = _to_c_str(js)
    f(w, c_js)
    c_js.free()


fn mwv_eval(w: WebviewHandle, js: String) raises:
    """Evaluate JavaScript in the webview."""
    var lib = get_lib()
    var f = lib.get_function[
        fn (WebviewHandle, UnsafePointer[UInt8, MutExternalOrigin]) -> None
    ]("mwv_eval")
    var c_js = _to_c_str(js)
    f(w, c_js)
    c_js.free()


fn mwv_run(w: WebviewHandle) raises:
    """Run the webview event loop (blocking). Returns when window is closed."""
    var lib = get_lib()
    var f = lib.get_function[fn (WebviewHandle) -> None]("mwv_run")
    f(w)


fn mwv_terminate(w: WebviewHandle) raises:
    """Signal the event loop to terminate."""
    var lib = get_lib()
    var f = lib.get_function[fn (WebviewHandle) -> None]("mwv_terminate")
    f(w)


fn mwv_step(w: WebviewHandle, blocking: Int32) raises -> Int32:
    """Run a single iteration of the event loop.

    Args:
        w: Webview handle.
        blocking: If non-zero, block until at least one event is processed.

    Returns:
        0 if the window is still open, non-zero if it should close.
    """
    var lib = get_lib()
    var f = lib.get_function[
        fn (WebviewHandle, Int32) -> Int32
    ]("mwv_step")
    return f(w, blocking)


fn mwv_poll_event(
    w: WebviewHandle, out_buf: UnsafePointer[UInt8, MutExternalOrigin], buf_len: Int32
) raises -> Int32:
    """Poll for the next event from JavaScript.

    Returns:
        Number of bytes written to out_buf (excluding null terminator),
        or 0 if no event is available.
    """
    var lib = get_lib()
    var f = lib.get_function[
        fn (WebviewHandle, UnsafePointer[UInt8, MutExternalOrigin], Int32) -> Int32
    ]("mwv_poll_event")
    return f(w, out_buf, buf_len)


fn mwv_event_count(w: WebviewHandle) raises -> Int32:
    """Return the number of buffered (unpolled) events."""
    var lib = get_lib()
    var f = lib.get_function[fn (WebviewHandle) -> Int32]("mwv_event_count")
    return f(w)


fn mwv_event_clear(w: WebviewHandle) raises:
    """Discard all buffered events."""
    var lib = get_lib()
    var f = lib.get_function[fn (WebviewHandle) -> None]("mwv_event_clear")
    f(w)


fn mwv_apply_mutations(
    w: WebviewHandle, buf: UnsafePointer[UInt8, MutExternalOrigin], length: Int32
) raises -> Int32:
    """Send a binary mutation buffer to the webview's JS interpreter.

    The C shim base64-encodes the buffer and calls
    `window.__mojo_apply_mutations(base64_string)` in the webview.

    Returns:
        0 on success, non-zero on failure.
    """
    var lib = get_lib()
    var f = lib.get_function[
        fn (WebviewHandle, UnsafePointer[UInt8, MutExternalOrigin], Int32) -> Int32
    ]("mwv_apply_mutations")
    return f(w, buf, length)


fn mwv_is_alive(w: WebviewHandle) raises -> Int32:
    """Return 1 if the webview window is still open, 0 if closed."""
    var lib = get_lib()
    var f = lib.get_function[fn (WebviewHandle) -> Int32]("mwv_is_alive")
    return f(w)


# ── High-level Webview struct ─────────────────────────────────────────────


struct Webview(Movable):
    """High-level wrapper around the mojo_webview C shim.

    Manages the lifecycle of a single webview window with automatic
    resource cleanup.

    Example:

        var wv = Webview("Counter App", 800, 600)
        wv.set_html("<html><body><div id='root'></div></body></html>")

        while wv.is_alive():
            if wv.step(blocking=False):
                break

            var event = wv.poll_event()
            if event:
                # handle the JSON event string
                pass

        wv.destroy()
    """

    var _handle: WebviewHandle
    var _event_buf: UnsafePointer[UInt8, MutExternalOrigin]
    var _alive: Bool

    fn __moveinit__(out self, deinit other: Self):
        self._handle = other._handle
        self._event_buf = other._event_buf
        self._alive = other._alive

    fn __init__(
        out self,
        title: String = "mojo-gui",
        width: Int = 800,
        height: Int = 600,
        debug: Bool = False,
    ) raises:
        """Create a new webview window.

        Args:
            title: Window title.
            width: Initial window width in pixels.
            height: Initial window height in pixels.
            debug: Enable developer tools / inspector.
        """
        self._handle = mwv_create(
            title,
            Int32(width),
            Int32(height),
            Int32(1) if debug else Int32(0),
        )
        # Pre-allocate the event polling buffer.
        self._event_buf = alloc[UInt8](MWV_EVENT_MAX_LEN)
        self._alive = True

    fn __del__(deinit self):
        """Ensure resources are freed if the user forgets to call destroy()."""
        if self._alive:
            try:
                mwv_destroy(self._handle)
            except:
                pass
            _ = self._alive
        if self._event_buf:
            self._event_buf.free()

    fn destroy(mut self) raises:
        """Explicitly destroy the webview window and free resources."""
        if self._alive:
            mwv_destroy(self._handle)
            self._alive = False

    # ── Window properties ─────────────────────────────────────────────

    fn set_title(self, title: String) raises:
        """Set the window title."""
        mwv_set_title(self._handle, title)

    fn set_size(
        self,
        width: Int,
        height: Int,
        hints: Int = MWV_HINT_NONE,
    ) raises:
        """Set the window size.

        Args:
            width: Width in pixels.
            height: Height in pixels.
            hints: One of MWV_HINT_NONE, MWV_HINT_MIN, MWV_HINT_MAX,
                   MWV_HINT_FIXED.
        """
        mwv_set_size(self._handle, Int32(width), Int32(height), Int32(hints))

    # ── Content ───────────────────────────────────────────────────────

    fn navigate(self, url: String) raises:
        """Navigate the webview to a URL."""
        mwv_navigate(self._handle, url)

    fn set_html(self, html: String) raises:
        """Set the webview content to the given HTML string."""
        mwv_set_html(self._handle, html)

    fn init_js(self, js: String) raises:
        """Inject JavaScript to be executed on every new page load.

        Use this to set up the mutation interpreter and event bridge
        before any content is rendered.
        """
        mwv_init(self._handle, js)

    fn eval_js(self, js: String) raises:
        """Evaluate JavaScript in the webview."""
        mwv_eval(self._handle, js)

    # ── Event loop ────────────────────────────────────────────────────

    fn run(self) raises:
        """Run the webview event loop (blocking).

        Returns when the window is closed by the user.
        """
        mwv_run(self._handle)

    fn terminate(self) raises:
        """Signal the event loop to terminate.

        After calling this, run() will return.
        """
        mwv_terminate(self._handle)

    fn step(self, blocking: Bool = False) raises -> Bool:
        """Run a single iteration of the event loop.

        Args:
            blocking: If True, block until at least one event is processed.

        Returns:
            True if the window should close, False if still open.
        """
        var result = mwv_step(
            self._handle, Int32(1) if blocking else Int32(0)
        )
        return result != 0

    # ── Event polling (JS → Mojo) ─────────────────────────────────────

    fn poll_event(self) raises -> String:
        """Poll for the next event from JavaScript.

        Returns:
            The event payload as a String, or an empty String if no
            event is available.

        JS code sends events by calling `window.mojo_post(json_string)`.
        """
        var n = mwv_poll_event(
            self._handle, self._event_buf, Int32(MWV_EVENT_MAX_LEN)
        )
        if n <= 0:
            return String("")
        # Build a String from the raw buffer bytes.
        var result = String("")
        for i in range(Int(n)):
            result += chr(Int(self._event_buf[i]))
        return result

    fn event_count(self) raises -> Int:
        """Return the number of buffered (unpolled) events."""
        return Int(mwv_event_count(self._handle))

    fn event_clear(self) raises:
        """Discard all buffered events."""
        mwv_event_clear(self._handle)

    # ── Mutation buffer ───────────────────────────────────────────────

    fn apply_mutations(self, buf: UnsafePointer[UInt8, MutExternalOrigin], length: Int) raises:
        """Send a binary mutation buffer to the webview's JS interpreter.

        The C shim base64-encodes the buffer and calls
        `window.__mojo_apply_mutations(base64_string)` in the webview.
        The JS side decodes the base64 back to an ArrayBuffer and feeds
        it to the Interpreter.

        Args:
            buf: Pointer to the binary mutation buffer.
            length: Number of bytes in the buffer.
        """
        var result = mwv_apply_mutations(self._handle, buf, Int32(length))
        if result != 0:
            raise Error("mwv_apply_mutations failed")

    # ── Diagnostics ───────────────────────────────────────────────────

    fn is_alive(self) raises -> Bool:
        """Return True if the webview window is still open."""
        if not self._alive:
            return False
        return mwv_is_alive(self._handle) != 0
