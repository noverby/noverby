/*
 * mojo_webview.h — C shim for webview/webview with a Mojo-friendly polling API.
 *
 * The upstream webview C API uses function-pointer callbacks for event binding
 * (webview_bind) and dispatch (webview_dispatch).  Mojo's FFI (DLHandle /
 * external_call) cannot easily pass managed closures as C function pointers.
 *
 * This shim wraps the webview API and replaces callbacks with a polling model:
 *
 *   1. JS code calls `window.mojo_post(json_string)` to send events.
 *   2. The shim stores events in a ring buffer.
 *   3. Mojo polls `mwv_poll_event()` to drain events one at a time.
 *   4. Mojo sends mutations via `mwv_eval()` (thin wrapper around webview_eval).
 *
 * This keeps all complexity on the Mojo side and avoids callback marshalling.
 *
 * Build:
 *   cc -shared -fPIC -o libmojo_webview.so mojo_webview.c \
 *      $(pkg-config --cflags --libs gtk4 webkitgtk-6.0)
 *
 * Or with the webview/webview library:
 *   cc -shared -fPIC -o libmojo_webview.so mojo_webview.c \
 *      -lwebview -DUSE_WEBVIEW_LIB
 */

#ifndef MOJO_WEBVIEW_H
#define MOJO_WEBVIEW_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ── Opaque handle ──────────────────────────────────────────────────────── */

/** Opaque handle to a mojo_webview instance. */
typedef void *mwv_t;

/* ── Size hint constants (match webview.h) ──────────────────────────────── */

#define MWV_HINT_NONE  0  /**< Width and height are default size. */
#define MWV_HINT_MIN   1  /**< Width and height are minimum bounds. */
#define MWV_HINT_MAX   2  /**< Width and height are maximum bounds. */
#define MWV_HINT_FIXED 3  /**< Window size cannot be changed by the user. */

/* ── Event ring buffer capacity ─────────────────────────────────────────── */

/**
 * Maximum number of events buffered before oldest are dropped.
 * Power of 2 for efficient modular arithmetic.
 */
#define MWV_EVENT_RING_CAPACITY 256

/**
 * Maximum byte length of a single event's JSON payload.
 * Events exceeding this are silently truncated.
 */
#define MWV_EVENT_MAX_LEN 4096

/* ── Lifecycle ──────────────────────────────────────────────────────────── */

/**
 * Create a new webview window.
 *
 * @param title   Window title (UTF-8, null-terminated).
 * @param width   Initial window width in pixels.
 * @param height  Initial window height in pixels.
 * @param debug   Non-zero to enable developer tools / inspector.
 * @return        Opaque handle, or NULL on failure.
 *
 * The window is not shown until mwv_run() or mwv_step() is called.
 * A JS binding `window.mojo_post(msg)` is automatically injected so
 * that JS code can send string messages to the native side.
 */
mwv_t mwv_create(const char *title, int width, int height, int debug);

/**
 * Destroy the webview and free all resources.
 * The handle is invalid after this call.
 */
void mwv_destroy(mwv_t w);

/* ── Window properties ──────────────────────────────────────────────────── */

/**
 * Set the window title.
 *
 * @param title  UTF-8, null-terminated string.
 */
void mwv_set_title(mwv_t w, const char *title);

/**
 * Set the window size.
 *
 * @param width   Width in pixels.
 * @param height  Height in pixels.
 * @param hints   One of MWV_HINT_NONE / MIN / MAX / FIXED.
 */
void mwv_set_size(mwv_t w, int width, int height, int hints);

/* ── Content ────────────────────────────────────────────────────────────── */

/**
 * Navigate the webview to a URL.
 */
void mwv_navigate(mwv_t w, const char *url);

/**
 * Set the webview content to the given HTML string.
 * The HTML is loaded as a `data:` URI or equivalent.
 */
void mwv_set_html(mwv_t w, const char *html);

/**
 * Inject JavaScript to be executed on every new page load.
 * Use this to set up the mutation interpreter and event bridge
 * before any content is rendered.
 *
 * @param js  UTF-8 null-terminated JavaScript source.
 */
void mwv_init(mwv_t w, const char *js);

/**
 * Evaluate JavaScript in the webview.
 * This is the primary channel for sending mutation buffers from Mojo.
 *
 * @param js  UTF-8 null-terminated JavaScript source.
 */
void mwv_eval(mwv_t w, const char *js);

/* ── Event loop ─────────────────────────────────────────────────────────── */

/**
 * Run the webview event loop (blocking).
 * Returns when the window is closed.
 */
void mwv_run(mwv_t w);

/**
 * Signal the event loop to terminate.
 * After calling this, mwv_run() will return.
 */
void mwv_terminate(mwv_t w);

/**
 * Run a single iteration of the event loop (non-blocking).
 *
 * @param blocking  If non-zero, block until at least one event is processed.
 *                  If zero, process pending events and return immediately.
 * @return          0 if the window is still open, non-zero if it should close.
 *
 * This enables Mojo to interleave GUI event processing with its own work:
 *
 *     while mwv_step(w, 0) == 0:
 *         event = mwv_poll_event(w)
 *         if event:
 *             process(event)
 *             mwv_eval(w, mutations_js)
 */
int mwv_step(mwv_t w, int blocking);

/* ── Event polling (JS → Native) ────────────────────────────────────────── */

/**
 * Poll for the next event from JavaScript.
 *
 * JS code sends events by calling `window.mojo_post(json_string)`.
 * The shim buffers these in a lock-free ring buffer.
 *
 * @param w        Webview handle.
 * @param out_buf  Caller-allocated buffer to receive the event payload.
 * @param buf_len  Size of out_buf in bytes.
 * @return         Number of bytes written to out_buf (excluding null terminator),
 *                 or 0 if no event is available.
 *                 The output is always null-terminated if return > 0.
 *
 * Usage from Mojo:
 *
 *     var buf = alloc[UInt8](4096)
 *     var n = mwv_poll_event(w, buf, 4096)
 *     if n > 0:
 *         var payload = StringRef(buf, n)
 *         handle_event(payload)
 */
int mwv_poll_event(mwv_t w, char *out_buf, int buf_len);

/**
 * Return the number of buffered (unpolled) events.
 */
int mwv_event_count(mwv_t w);

/**
 * Discard all buffered events.
 */
void mwv_event_clear(mwv_t w);

/* ── Mutation buffer helpers ────────────────────────────────────────────── */

/**
 * Send a binary mutation buffer to the webview's JS interpreter.
 *
 * This is a convenience function that base64-encodes the buffer and
 * calls `window.__mojo_apply_mutations(base64_string)` in the webview.
 * The JS side decodes the base64 back to an ArrayBuffer and feeds it
 * to the Interpreter.
 *
 * @param w       Webview handle.
 * @param buf     Pointer to the binary mutation buffer.
 * @param len     Number of bytes in the buffer.
 * @return        0 on success, non-zero on failure.
 *
 * This avoids Mojo having to do base64 encoding — the C shim handles it.
 */
int mwv_apply_mutations(mwv_t w, const uint8_t *buf, int len);

/* ── Diagnostics ────────────────────────────────────────────────────────── */

/**
 * Return 1 if the webview window is still open, 0 if closed/destroyed.
 */
int mwv_is_alive(mwv_t w);

/**
 * Get the underlying native window handle (platform-specific).
 * On GTK: returns the GtkWindow*.
 * On macOS: returns the NSWindow*.
 * On Windows: returns the HWND.
 * Returns NULL if the webview has been destroyed.
 */
void *mwv_get_window(mwv_t w);

#ifdef __cplusplus
}
#endif

#endif /* MOJO_WEBVIEW_H */