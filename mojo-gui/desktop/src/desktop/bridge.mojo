"""Desktop Bridge — Connects mojo-gui/core's mutation protocol to the webview.

This module bridges the gap between the core framework (which writes binary
mutations to a buffer) and the desktop webview (which applies them via JS).

Architecture:

    mojo-gui/core                    bridge.mojo                     webview
    ┌──────────────┐                 ┌──────────────┐                ┌──────────┐
    │ AppShell      │                 │ DesktopBridge│                │ Webview  │
    │  .rebuild()   │──mutations──►  │  .flush()    │──apply_mut──► │  JS      │
    │  .flush()     │                │              │                │  interp  │
    │              │                 │  .dispatch() │◄──poll_event── │  events  │
    │ HandlerReg   │◄──dispatch───  │              │                │          │
    └──────────────┘                 └──────────────┘                └──────────┘

The bridge:
  1. Owns a heap-allocated mutation buffer (replaces WASM linear memory).
  2. Creates a MutationWriter pointing at the heap buffer.
  3. After core writes mutations, calls webview.apply_mutations() to send
     the binary buffer to the webview's JS interpreter.
  4. Polls events from the webview and routes them to the core's
     HandlerRegistry for dispatch.

Event JSON format (JS → Mojo):

    { "h": <handler_id>, "t": <event_type> }
    { "h": <handler_id>, "t": <event_type>, "v": "<string_value>" }

Where:
  - h = handler ID (from NewEventListener mutation)
  - t = event type tag (0=click, 1=input, 2=keydown, etc.)
  - v = optional string value (for input/change events)
"""

from memory import UnsafePointer, memset_zero, alloc
from bridge.protocol import MutationWriter, OP_END

from .webview import Webview, WebviewHandle

# ── Constants ─────────────────────────────────────────────────────────────

comptime DEFAULT_BUF_CAPACITY = 65536
"""Default mutation buffer capacity in bytes (64 KiB).

This matches the web runtime's default (DEFAULT_BUF_CAPACITY in app.ts).
The buffer is heap-allocated and can be resized if needed.
"""

comptime EVENT_BUF_SIZE = 4096
"""Maximum size of a single event JSON payload."""


# ── Event parsing ─────────────────────────────────────────────────────────


struct DesktopEvent(Copyable, Movable, ImplicitlyCopyable):
    """A parsed event from the webview's JS event bridge.

    Fields:
        handler_id: The handler ID registered by NewEventListener.
        event_type: The event type tag (EVT_CLICK=0, EVT_INPUT=1, etc.).
        has_value: True if the event carries a string value.
        value: The string value (for input/change events), empty otherwise.
    """

    var handler_id: Int
    var event_type: Int
    var has_value: Bool
    var value: String

    fn __init__(out self):
        self.handler_id = -1
        self.event_type = 0
        self.has_value = False
        self.value = String("")

    fn __init__(out self, handler_id: Int, event_type: Int, has_value: Bool, value: String):
        self.handler_id = handler_id
        self.event_type = event_type
        self.has_value = has_value
        self.value = value

    fn __copyinit__(out self, other: Self):
        self.handler_id = other.handler_id
        self.event_type = other.event_type
        self.has_value = other.has_value
        self.value = other.value

    fn __moveinit__(out self, deinit other: Self):
        self.handler_id = other.handler_id
        self.event_type = other.event_type
        self.has_value = other.has_value
        self.value = other.value^

    fn is_valid(self) -> Bool:
        """Return True if this event was successfully parsed."""
        return self.handler_id >= 0


fn _parse_int_after(json: String, key: String) -> Int:
    """Extract an integer value after a key like '"h":' in minimal JSON.

    This is a simple parser for the minimal event JSON format.
    It does NOT handle arbitrary JSON — only the specific format emitted
    by our desktop event bridge JS.
    """
    var pos = json.find(key)
    if pos < 0:
        return -1
    pos += len(key)

    var json_bytes = json.as_bytes()

    # Skip whitespace.
    while pos < len(json):
        var c = json_bytes[pos]
        if c != ord(" ") and c != ord("\t") and c != ord("\n") and c != ord("\r"):
            break
        pos += 1

    # Parse digits (possibly with leading minus).
    var start = pos
    if pos < len(json) and json_bytes[pos] == ord("-"):
        pos += 1
    while pos < len(json):
        var c = Int(json_bytes[pos])
        if c < ord("0") or c > ord("9"):
            break
        pos += 1

    if pos == start:
        return -1

    try:
        return Int(json[start:pos])
    except:
        return -1


fn _parse_string_after(json: String, key: String) -> String:
    """Extract a string value after a key like '"v":"' in minimal JSON.

    Handles basic escape sequences: \\", \\\\, \\n, \\t.
    Returns empty string if the key is not found.
    """
    var pos = json.find(key)
    if pos < 0:
        return String("")
    pos += len(key)

    var json_bytes = json.as_bytes()

    # Skip whitespace.
    while pos < len(json):
        var c = json_bytes[pos]
        if c != ord(" ") and c != ord("\t") and c != ord("\n") and c != ord("\r"):
            break
        pos += 1

    # Expect opening quote.
    if pos >= len(json) or json_bytes[pos] != ord('"'):
        return String("")
    pos += 1

    var result = String("")
    while pos < len(json):
        var c = json_bytes[pos]
        if c == ord('"'):
            return result
        if c == ord("\\") and pos + 1 < len(json):
            var next_c = json_bytes[pos + 1]
            if next_c == ord('"'):
                result += '"'
            elif next_c == ord("\\"):
                result += "\\"
            elif next_c == ord("n"):
                result += "\n"
            elif next_c == ord("t"):
                result += "\t"
            else:
                result += chr(Int(next_c))
            pos += 2
            continue
        result += chr(Int(c))
        pos += 1

    return result


fn parse_event(json: String) -> DesktopEvent:
    """Parse a desktop event from its JSON representation.

    Expected format:
        {"h":42,"t":0}
        {"h":42,"t":1,"v":"hello"}

    Returns:
        A DesktopEvent. Check .is_valid() to see if parsing succeeded.
    """
    var event = DesktopEvent()

    event.handler_id = _parse_int_after(json, '"h":')
    if event.handler_id < 0:
        return event

    event.event_type = _parse_int_after(json, '"t":')
    if event.event_type < 0:
        event.handler_id = -1  # Mark invalid.
        return event

    # Check for optional string value.
    if json.find('"v":') >= 0:
        event.has_value = True
        event.value = _parse_string_after(json, '"v":')

    return event


# ── DesktopBridge ─────────────────────────────────────────────────────────


struct DesktopBridge(Movable):
    """Connects mojo-gui/core's mutation protocol to a desktop webview.

    The bridge owns a heap-allocated mutation buffer that the core framework
    writes to via MutationWriter. After each render cycle, the bridge sends
    the buffer contents to the webview and polls for events.

    Usage:

        var wv = Webview("My App", 800, 600)
        var bridge = DesktopBridge(wv)

        # Get a MutationWriter for the core to write to.
        var writer = bridge.writer()

        # ... core writes mutations via writer ...
        writer.write_end()

        # Send mutations to the webview.
        bridge.flush_mutations(writer.offset)

        # Poll and handle events.
        var event = bridge.poll_event()
        if event.is_valid():
            # dispatch to HandlerRegistry
            pass
    """

    var _buf: UnsafePointer[UInt8, MutExternalOrigin]
    var _capacity: Int
    var _webview: UnsafePointer[Webview, MutExternalOrigin]
    var _alive: Bool

    fn __init__(out self, webview_ptr: UnsafePointer[Webview, MutExternalOrigin], capacity: Int = DEFAULT_BUF_CAPACITY):
        """Create a new desktop bridge.

        Args:
            webview_ptr: Pointer to the Webview instance (borrowed, not owned).
            capacity: Mutation buffer capacity in bytes.
        """
        self._buf = alloc[UInt8](capacity)
        for i in range(capacity):
            self._buf[i] = 0
        self._capacity = capacity
        self._webview = webview_ptr
        self._alive = True

    fn __del__(deinit self):
        """Free the mutation buffer."""
        if self._buf:
            self._buf.free()

    fn __moveinit__(out self, deinit other: Self):
        self._buf = other._buf
        self._capacity = other._capacity
        self._webview = other._webview
        self._alive = other._alive

    # ── Mutation buffer access ────────────────────────────────────────

    fn buf_ptr(mut self) -> UnsafePointer[UInt8, MutExternalOrigin]:
        """Return a raw pointer to the mutation buffer.

        Use this to construct a MutationWriter:

            var writer = MutationWriter(
                bridge.buf_ptr().bitcast[UInt8, MutExternalOrigin](),
                bridge.capacity(),
            )

        Note: MutationWriter expects UnsafePointer[UInt8, MutExternalOrigin].
        For native (non-WASM) builds, you may need to cast the pointer.
        """
        return self._buf

    fn capacity(mut self) -> Int:
        """Return the mutation buffer capacity in bytes."""
        return self._capacity

    fn reset_buffer(mut self):
        """Zero out the mutation buffer for the next render cycle.

        Call this after flush_mutations() to prepare for the next batch.
        """
        for i in range(self._capacity):
            self._buf[i] = 0

    # ── Mutation sending ──────────────────────────────────────────────

    fn flush_mutations(mut self, byte_length: Int) raises:
        """Send the mutation buffer contents to the webview's JS interpreter.

        Args:
            byte_length: Number of bytes written by MutationWriter (writer.offset).

        This calls the C shim's mwv_apply_mutations(), which:
          1. Base64-encodes the buffer
          2. Calls window.__mojo_apply_mutations(base64) in the webview
          3. The JS side decodes and feeds to the Interpreter
        """
        if byte_length <= 0:
            return
        if not self._alive:
            raise Error("DesktopBridge: attempted flush after destroy")
        self._webview[].apply_mutations(self._buf, byte_length)

    # ── Event polling ─────────────────────────────────────────────────

    fn poll_event(mut self) raises -> DesktopEvent:
        """Poll for the next event from the webview.

        Returns:
            A DesktopEvent. Check .is_valid() to see if an event was available.

        Events are sent by JS via `window.mojo_post(json)` and buffered
        in the C shim's ring buffer. This method pops one event at a time.
        """
        if not self._alive:
            return DesktopEvent()

        var json = self._webview[].poll_event()
        if len(json) == 0:
            return DesktopEvent()

        return parse_event(json)

    fn poll_all_events(mut self) raises -> List[DesktopEvent]:
        """Drain all buffered events from the webview.

        Returns:
            A list of valid DesktopEvent instances.
        """
        var events = List[DesktopEvent]()
        while True:
            var event = self.poll_event()
            if not event.is_valid():
                break
            events.append(event)
        return events

    # ── Lifecycle ─────────────────────────────────────────────────────

    fn is_alive(mut self) -> Bool:
        """Return True if the bridge is still active."""
        return self._alive

    fn shutdown(mut self):
        """Mark the bridge as inactive.

        Does NOT destroy the webview — that is the caller's responsibility.
        """
        self._alive = False
