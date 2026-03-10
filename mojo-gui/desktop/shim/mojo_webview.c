/*
 * mojo_webview.c — C shim implementation using GTK4 + WebKitGTK 6.0.
 *
 * Provides a Mojo-friendly polling API over a GTK4/WebKitGTK webview.
 * Events from JS flow through a ring buffer; mutations flow via eval.
 *
 * Build:
 *   cc -shared -fPIC -o libmojo_webview.so mojo_webview.c \
 *      $(pkg-config --cflags --libs gtk4 webkitgtk-6.0)
 */

#include "mojo_webview.h"

#include <gtk/gtk.h>
#include <webkit/webkit.h>

#include <stdlib.h>
#include <string.h>
#include <stdio.h>

/* ── Base64 encoder (RFC 4648) ──────────────────────────────────────────── */

static const char b64_table[] =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/**
 * Encode `src` (len bytes) into `dst` as base64.
 * `dst` must hold at least 4*((len+2)/3) + 1 bytes.
 * Returns the number of characters written (excluding null terminator).
 */
static int base64_encode(char *dst, const uint8_t *src, int len) {
    int i = 0, j = 0;
    while (i + 2 < len) {
        dst[j++] = b64_table[(src[i] >> 2) & 0x3F];
        dst[j++] = b64_table[((src[i] & 0x03) << 4) | ((src[i+1] >> 4) & 0x0F)];
        dst[j++] = b64_table[((src[i+1] & 0x0F) << 2) | ((src[i+2] >> 6) & 0x03)];
        dst[j++] = b64_table[src[i+2] & 0x3F];
        i += 3;
    }
    if (i < len) {
        dst[j++] = b64_table[(src[i] >> 2) & 0x3F];
        if (i + 1 < len) {
            dst[j++] = b64_table[((src[i] & 0x03) << 4) | ((src[i+1] >> 4) & 0x0F)];
            dst[j++] = b64_table[((src[i+1] & 0x0F) << 2)];
        } else {
            dst[j++] = b64_table[((src[i] & 0x03) << 4)];
            dst[j++] = '=';
        }
        dst[j++] = '=';
    }
    dst[j] = '\0';
    return j;
}

/* ── Event ring buffer ──────────────────────────────────────────────────── */

typedef struct {
    char data[MWV_EVENT_MAX_LEN];
    int  len;
} mwv_event_slot_t;

typedef struct {
    mwv_event_slot_t slots[MWV_EVENT_RING_CAPACITY];
    int head; /* next write index  */
    int tail; /* next read index   */
    int count;
} mwv_event_ring_t;

static void ring_init(mwv_event_ring_t *r) {
    r->head  = 0;
    r->tail  = 0;
    r->count = 0;
}

static void ring_push(mwv_event_ring_t *r, const char *data, int len) {
    if (len > MWV_EVENT_MAX_LEN - 1)
        len = MWV_EVENT_MAX_LEN - 1;

    /* If full, drop the oldest event by advancing tail. */
    if (r->count == MWV_EVENT_RING_CAPACITY) {
        r->tail = (r->tail + 1) % MWV_EVENT_RING_CAPACITY;
        r->count--;
    }

    mwv_event_slot_t *slot = &r->slots[r->head];
    memcpy(slot->data, data, len);
    slot->data[len] = '\0';
    slot->len = len;

    r->head = (r->head + 1) % MWV_EVENT_RING_CAPACITY;
    r->count++;
}

/**
 * Pop the oldest event into out_buf.
 * Returns number of bytes copied (excluding NUL), or 0 if empty.
 */
static int ring_pop(mwv_event_ring_t *r, char *out_buf, int buf_len) {
    if (r->count == 0)
        return 0;

    mwv_event_slot_t *slot = &r->slots[r->tail];
    int copy_len = slot->len;
    if (copy_len > buf_len - 1)
        copy_len = buf_len - 1;

    memcpy(out_buf, slot->data, copy_len);
    out_buf[copy_len] = '\0';

    r->tail = (r->tail + 1) % MWV_EVENT_RING_CAPACITY;
    r->count--;
    return copy_len;
}

/* ── Internal state ─────────────────────────────────────────────────────── */

typedef struct {
    GtkApplication    *app;
    GtkWindow         *window;
    WebKitWebView     *webview;
    mwv_event_ring_t   events;
    int                alive;      /* 1 while window exists */
    int                should_close;
    int                gtk_initialized; /* set after g_application_register */
} mwv_state_t;

/* ── GTK / WebKit callbacks ─────────────────────────────────────────────── */

/* Called when user closes the window. */
static gboolean on_close_request(GtkWindow *win, gpointer user_data) {
    mwv_state_t *s = (mwv_state_t *)user_data;
    (void)win;
    s->alive = 0;
    s->should_close = 1;
    return FALSE; /* allow default close behaviour */
}

/* Called when the user-content-manager receives a message on "mojoPost". */
static void on_script_message(WebKitUserContentManager *manager,
                              JSCValue                 *js_result,
                              gpointer                  user_data) {
    mwv_state_t *s = (mwv_state_t *)user_data;
    (void)manager;

    if (!jsc_value_is_string(js_result))
        return;

    char *str = jsc_value_to_string(js_result);
    if (str) {
        ring_push(&s->events, str, (int)strlen(str));
        g_free(str);
    }
}

/* Called when the GtkApplication activates (creates the window). */
static void on_activate(GtkApplication *app, gpointer user_data) {
    mwv_state_t *s = (mwv_state_t *)user_data;

    /* Create the window. */
    s->window = GTK_WINDOW(gtk_application_window_new(app));
    gtk_window_set_default_size(s->window, 800, 600);
    g_signal_connect(s->window, "close-request",
                     G_CALLBACK(on_close_request), s);

    /* Create a WebKitUserContentManager and register our message handler. */
    WebKitUserContentManager *ucm = webkit_user_content_manager_new();
    webkit_user_content_manager_register_script_message_handler(ucm, "mojoPost", NULL);
    g_signal_connect(ucm, "script-message-received::mojoPost",
                     G_CALLBACK(on_script_message), s);

    /* Inject the mojo_post bridge JS before any page loads. */
    const char *bridge_js =
        "window.mojo_post = function(msg) {"
        "  window.webkit.messageHandlers.mojoPost.postMessage(msg);"
        "};";
    WebKitUserScript *script = webkit_user_script_new(
        bridge_js,
        WEBKIT_USER_CONTENT_INJECT_ALL_FRAMES,
        WEBKIT_USER_SCRIPT_INJECT_AT_DOCUMENT_START,
        NULL, NULL);
    webkit_user_content_manager_add_script(ucm, script);
    webkit_user_script_unref(script);

    /* Create the WebKitWebView with our UCM. */
    s->webview = WEBKIT_WEB_VIEW(
        g_object_new(WEBKIT_TYPE_WEB_VIEW,
                     "user-content-manager", ucm,
                     NULL));
    g_object_unref(ucm);

    /* Enable developer tools if requested (debug flag is stored in
       the window title prefix — crude but avoids extra fields). */
    WebKitSettings *settings = webkit_web_view_get_settings(s->webview);
    webkit_settings_set_enable_developer_extras(settings, TRUE);

    /* Put the webview in the window. */
    gtk_window_set_child(s->window, GTK_WIDGET(s->webview));

    s->alive = 1;
    gtk_window_present(s->window);
}

/* ── Public API ─────────────────────────────────────────────────────────── */

mwv_t mwv_create(const char *title, int width, int height, int debug) {
    (void)debug; /* developer tools are always enabled for now */

    mwv_state_t *s = (mwv_state_t *)calloc(1, sizeof(mwv_state_t));
    if (!s) return NULL;

    ring_init(&s->events);
    s->alive        = 0;
    s->should_close = 0;
    s->gtk_initialized = 0;

    /* Create a GtkApplication with a unique ID based on pointer address. */
    char app_id[64];
    snprintf(app_id, sizeof(app_id), "org.mojogui.desktop.%p", (void *)s);
    s->app = gtk_application_new(app_id, G_APPLICATION_DEFAULT_FLAGS);
    g_signal_connect(s->app, "activate", G_CALLBACK(on_activate), s);

    /* Register the application (triggers GTK init if needed). */
    GError *err = NULL;
    g_application_register(G_APPLICATION(s->app), NULL, &err);
    if (err) {
        fprintf(stderr, "mwv_create: g_application_register failed: %s\n",
                err->message);
        g_error_free(err);
        g_object_unref(s->app);
        free(s);
        return NULL;
    }
    s->gtk_initialized = 1;

    /* Activate the application to create the window. */
    g_application_activate(G_APPLICATION(s->app));

    /* Process pending events so the window actually appears. */
    while (g_main_context_iteration(NULL, FALSE)) {}

    /* Apply initial properties. */
    if (s->window) {
        if (title)
            gtk_window_set_title(s->window, title);
        if (width > 0 && height > 0)
            gtk_window_set_default_size(s->window, width, height);
    }

    return (mwv_t)s;
}

void mwv_destroy(mwv_t w) {
    if (!w) return;
    mwv_state_t *s = (mwv_state_t *)w;

    if (s->window && s->alive) {
        gtk_window_destroy(s->window);
        while (g_main_context_iteration(NULL, FALSE)) {}
    }

    s->alive   = 0;
    s->window  = NULL;
    s->webview = NULL;

    if (s->app) {
        g_object_unref(s->app);
        s->app = NULL;
    }

    free(s);
}

/* ── Window properties ──────────────────────────────────────────────────── */

void mwv_set_title(mwv_t w, const char *title) {
    mwv_state_t *s = (mwv_state_t *)w;
    if (s && s->window && title)
        gtk_window_set_title(s->window, title);
}

void mwv_set_size(mwv_t w, int width, int height, int hints) {
    mwv_state_t *s = (mwv_state_t *)w;
    if (!s || !s->window) return;

    gtk_window_set_default_size(s->window, width, height);

    if (hints == MWV_HINT_FIXED)
        gtk_window_set_resizable(s->window, FALSE);
    else
        gtk_window_set_resizable(s->window, TRUE);

    /* GTK4 doesn't have direct min/max size hints like GTK3 — the
       window manager typically handles that.  For MWV_HINT_MIN / MAX
       we would need a GtkConstraint or css approach; skip for now. */
}

/* ── Content ────────────────────────────────────────────────────────────── */

void mwv_navigate(mwv_t w, const char *url) {
    mwv_state_t *s = (mwv_state_t *)w;
    if (s && s->webview && url)
        webkit_web_view_load_uri(s->webview, url);
}

void mwv_set_html(mwv_t w, const char *html) {
    mwv_state_t *s = (mwv_state_t *)w;
    if (s && s->webview && html)
        webkit_web_view_load_html(s->webview, html, "mojo-gui://app/");
}

void mwv_init(mwv_t w, const char *js) {
    mwv_state_t *s = (mwv_state_t *)w;
    if (!s || !s->webview || !js) return;

    WebKitUserContentManager *ucm =
        webkit_web_view_get_user_content_manager(s->webview);
    WebKitUserScript *script = webkit_user_script_new(
        js,
        WEBKIT_USER_CONTENT_INJECT_ALL_FRAMES,
        WEBKIT_USER_SCRIPT_INJECT_AT_DOCUMENT_START,
        NULL, NULL);
    webkit_user_content_manager_add_script(ucm, script);
    webkit_user_script_unref(script);
}

void mwv_eval(mwv_t w, const char *js) {
    mwv_state_t *s = (mwv_state_t *)w;
    if (!s || !s->webview || !js) return;
    webkit_web_view_evaluate_javascript(
        s->webview, js, -1, NULL, NULL, NULL, NULL, NULL);
}

/* ── Event loop ─────────────────────────────────────────────────────────── */

void mwv_run(mwv_t w) {
    mwv_state_t *s = (mwv_state_t *)w;
    if (!s) return;

    while (s->alive && !s->should_close) {
        g_main_context_iteration(NULL, TRUE);
    }
}

void mwv_terminate(mwv_t w) {
    mwv_state_t *s = (mwv_state_t *)w;
    if (s) s->should_close = 1;
}

int mwv_step(mwv_t w, int blocking) {
    mwv_state_t *s = (mwv_state_t *)w;
    if (!s || !s->alive || s->should_close) return 1;

    g_main_context_iteration(NULL, blocking ? TRUE : FALSE);

    return (s->alive && !s->should_close) ? 0 : 1;
}

/* ── Event polling ──────────────────────────────────────────────────────── */

int mwv_poll_event(mwv_t w, char *out_buf, int buf_len) {
    mwv_state_t *s = (mwv_state_t *)w;
    if (!s || !out_buf || buf_len <= 0) return 0;
    return ring_pop(&s->events, out_buf, buf_len);
}

int mwv_event_count(mwv_t w) {
    mwv_state_t *s = (mwv_state_t *)w;
    if (!s) return 0;
    return s->events.count;
}

void mwv_event_clear(mwv_t w) {
    mwv_state_t *s = (mwv_state_t *)w;
    if (!s) return;
    ring_init(&s->events);
}

/* ── Mutation buffer helper ─────────────────────────────────────────────── */

int mwv_apply_mutations(mwv_t w, const uint8_t *buf, int len) {
    mwv_state_t *s = (mwv_state_t *)w;
    if (!s || !s->webview || !buf || len <= 0) return -1;

    /* Base64 encoded size: 4 * ceil(len / 3) + 1 for NUL. */
    int b64_len = 4 * ((len + 2) / 3);

    /* JS template:
     *   window.__mojo_apply_mutations("<base64>")
     * Prefix: 35 chars, suffix: 2 chars (closing paren + quote), NUL: 1
     */
    static const char prefix[] = "window.__mojo_apply_mutations(\"";
    static const char suffix[] = "\")";
    int prefix_len = (int)sizeof(prefix) - 1;
    int suffix_len = (int)sizeof(suffix) - 1;
    int total = prefix_len + b64_len + suffix_len + 1;

    char *js = (char *)malloc(total);
    if (!js) return -1;

    memcpy(js, prefix, prefix_len);
    base64_encode(js + prefix_len, buf, len);
    memcpy(js + prefix_len + b64_len, suffix, suffix_len);
    js[prefix_len + b64_len + suffix_len] = '\0';

    mwv_eval(w, js);
    free(js);

    return 0;
}

/* ── Diagnostics ────────────────────────────────────────────────────────── */

int mwv_is_alive(mwv_t w) {
    mwv_state_t *s = (mwv_state_t *)w;
    if (!s) return 0;
    return s->alive && !s->should_close;
}

void *mwv_get_window(mwv_t w) {
    mwv_state_t *s = (mwv_state_t *)w;
    if (!s) return NULL;
    return (void *)s->window;
}