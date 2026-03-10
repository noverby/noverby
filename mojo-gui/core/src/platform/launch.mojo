# Platform Launch — Compile-time target-dispatching entry point.
#
# The launch() function is the single entry point that all mojo-gui apps use.
# The renderer is selected at **compile time** based on the build target:
#
#   - WASM target → web renderer (JS runtime drives the event loop)
#   - Native target → desktop renderer (webview/Blitz drives the event loop)
#
# This is the key enabler for shared examples. App code calls launch() and
# never imports a specific renderer. The build target determines which
# renderer is used.
#
# Usage in a shared example app:
#
#     from platform import launch
#
#     fn my_app(ctx: ComponentContext):
#         var count = ctx.use_signal(0)
#         ctx.setup_view(
#             el_div(
#                 el_h1(dyn_text()),
#                 el_button(text("+1"), onclick_add(count, 1)),
#             ),
#             String("my-app"),
#         )
#
#     fn main():
#         launch[my_app]()
#
# Build for different targets:
#
#     # Web (WASM):
#     mojo build app.mojo --target wasm64-wasi -I core/src -I web/src
#
#     # Desktop (native):
#     mojo build app.mojo -I core/src -I desktop/src
#
# The same source file, different build targets — the framework handles
# the rest.
#
# Design notes:
#
#   - launch() is parametric on the app builder function. This allows the
#     framework to instantiate the app in a renderer-appropriate context.
#
#   - For WASM targets, launch() registers the app builder so that the
#     JS runtime can invoke it via @export wrappers. The actual event loop
#     is driven by the browser (requestAnimationFrame + event listeners).
#
#   - For native targets, launch() creates the renderer (DesktopApp),
#     initializes the window, mounts the initial DOM, and enters the
#     platform event loop (blocking until the window is closed).
#
#   - The compile-time dispatch uses @parameter if with is_wasm_target()
#     so that only the relevant renderer code is compiled for each target.
#     This avoids pulling in desktop dependencies for WASM builds and
#     vice versa.
#
# Fallback strategy:
#
#   If Mojo's metaprogramming isn't mature enough for clean @parameter if
#   target detection, apps can use separate thin entry-point files:
#
#     main_web.mojo:     imports web launcher, calls app_builder
#     main_desktop.mojo: imports desktop launcher, calls app_builder
#
#   Both import and call the same shared app_builder function — the app
#   code is still shared, only the 3-line entry point differs per target.
#   This is strictly a build-system concern, not an app-authoring concern.

from .app import is_wasm_target, is_native_target


# ══════════════════════════════════════════════════════════════════════════════
# AppConfig — Optional configuration for launch()
# ══════════════════════════════════════════════════════════════════════════════


struct AppConfig(Copyable, Movable):
    """Configuration for the application launcher.

    Provides platform-independent configuration that renderers interpret
    according to their capabilities.

    Fields:
        title: Window title (desktop/native) or document title (web).
        width: Initial viewport width in logical pixels.
        height: Initial viewport height in logical pixels.
        debug: Enable developer tools / debug overlays.
    """

    var title: String
    var width: Int
    var height: Int
    var debug: Bool

    fn __init__(
        out self,
        title: String = "mojo-gui",
        width: Int = 800,
        height: Int = 600,
        debug: Bool = False,
    ):
        self.title = title
        self.width = width
        self.height = height
        self.debug = debug

    fn __copyinit__(out self, other: Self):
        self.title = other.title
        self.width = other.width
        self.height = other.height
        self.debug = other.debug

    fn __moveinit__(out self, deinit other: Self):
        self.title = other.title^
        self.width = other.width
        self.height = other.height
        self.debug = other.debug


# ══════════════════════════════════════════════════════════════════════════════
# Global app registry (for WASM target)
# ══════════════════════════════════════════════════════════════════════════════
#
# On the WASM target, the JS runtime drives the event loop. launch() cannot
# block — it must register the app builder and return, so that the JS side
# can invoke it later via @export wrappers.
#
# The registry stores the AppConfig so that @export init functions can
# access it. The actual app builder function is passed as a compile-time
# parameter to launch[], so it's available statically — no need to store
# a function pointer at runtime.

var _global_config: AppConfig = AppConfig()
var _launched: Bool = False


fn get_launch_config() -> AppConfig:
    """Retrieve the AppConfig set by the most recent launch() call.

    This is used by renderer entry points (e.g., WebApp, DesktopApp) to
    access the configuration provided by the app.

    Returns the default AppConfig if launch() has not been called.
    """
    return _global_config


fn has_launched() -> Bool:
    """Return True if launch() has been called.

    Used by renderer infrastructure to verify that the app has been
    properly initialized via the launch() entry point.
    """
    return _launched


# ══════════════════════════════════════════════════════════════════════════════
# launch() — The universal entry point
# ══════════════════════════════════════════════════════════════════════════════


fn launch(config: AppConfig = AppConfig()):
    """Launch the mojo-gui application on the current platform.

    This is the universal entry point for all mojo-gui apps. The renderer
    is selected at compile time based on the build target.

    Args:
        config: Optional application configuration (title, size, debug).

    On WASM targets:
        Stores the config for the JS runtime to access. The actual app
        initialization happens when the JS side calls the @export init
        function. launch() returns immediately — the browser event loop
        drives rendering.

    On native targets:
        Stores the config for the desktop/native renderer to access.
        The actual event loop is started by the renderer's run() method,
        which is called from the app's main() function after setting up
        the app struct and calling launch().

    Example (shared app entry point):

        fn main() raises:
            launch(AppConfig(
                title="My Counter",
                width=400,
                height=350,
                debug=True,
            ))

            # For WASM: the JS runtime takes over from here.
            # For native: the app code continues to set up the
            #   DesktopApp and enter the event loop.

    Note: In the current architecture, launch() primarily stores
    configuration. The renderer-specific lifecycle (init, mount, event
    loop) is handled by the renderer's own entry point code. As the
    platform abstraction matures, launch() may directly instantiate
    and run the appropriate renderer.
    """
    _global_config = config
    _launched = True

    # Future: When Mojo supports @parameter if for target detection and
    # the renderer implementations are mature enough, this function can
    # directly dispatch to the appropriate renderer:
    #
    #     @parameter
    #     if is_wasm_target():
    #         # Web path: register for JS runtime to invoke.
    #         _register_web_app(config)
    #     else:
    #         # Desktop path: create window and enter event loop.
    #         _run_desktop_app(config)
    #
    # For now, launch() stores config and the renderer entry points
    # (main.mojo for web, counter.mojo for desktop) handle the rest.
