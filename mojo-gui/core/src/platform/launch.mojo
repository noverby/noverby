# Platform Launch — Compile-time target-dispatching entry point.
#
# The launch() function is the single entry point that all mojo-gui apps use.
# The renderer is selected at **compile time** based on the build target:
#
#   - WASM target → web renderer (JS runtime drives the event loop)
#   - Native target → desktop renderer (Blitz drives the event loop)
#
# This is the key enabler for shared examples. App code calls launch() and
# never imports a specific renderer. The build target determines which
# renderer is used.
#
# Usage in a shared example app:
#
#     from platform import launch, AppConfig
#     from counter import CounterApp
#
#     fn main() raises:
#         launch[CounterApp](AppConfig(
#             title="Counter",
#             width=400,
#             height=350,
#         ))
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
#   - launch() is parametric on the GuiApp type. The type parameter tells
#     the framework which app to instantiate and run.
#
#   - For WASM targets, launch() stores the config and returns. The JS
#     runtime drives the event loop; @export wrappers in main.mojo use
#     the generic gui_app_exports helpers to call GuiApp trait methods.
#
#   - For native targets, launch() stores the config and imports the
#     desktop launcher, which creates the renderer (DesktopApp),
#     initializes the window, mounts the initial DOM, and enters the
#     platform event loop (blocking until the window is closed).
#
#   - The compile-time dispatch uses @parameter if with is_wasm_target()
#     so that only the relevant renderer code is compiled for each target.
#     This avoids pulling in desktop dependencies for WASM builds and
#     vice versa.
#
# Step 3.9.3: launch() now uses @parameter if is_wasm_target() for
# compile-time target dispatch. On WASM, it stores config and returns
# (JS drives the loop). On native, it will call desktop_launch[AppType]()
# once the Blitz desktop renderer is implemented (Phase 4).

from .app import is_wasm_target, is_native_target
from .gui_app import GuiApp


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


fn launch[AppType: GuiApp](config: AppConfig = AppConfig()) raises:
    """Launch the mojo-gui application on the current platform.

    This is the universal entry point for all mojo-gui apps. The renderer
    is selected at compile time based on the build target via
    `@parameter if is_wasm_target()`.

    Type Parameters:
        AppType: A concrete type implementing the GuiApp trait. This is
                 the app to instantiate and run.

    Args:
        config: Optional application configuration (title, size, debug).

    On WASM targets:
        Stores the config for the JS runtime to access. The actual app
        initialization happens when the JS side calls the @export init
        function (which uses gui_app_init[AppType]()). launch() returns
        immediately — the browser event loop drives rendering.

    On native targets:
        Stores the config, then calls desktop_launch[AppType](config)
        which creates the renderer window, mounts the initial DOM, and
        enters the platform event loop (blocking until the window is
        closed). Currently a placeholder — the Blitz desktop renderer
        will be implemented in Phase 4.

    Example (shared app entry point):

        from platform import launch, AppConfig
        from counter import CounterApp

        fn main() raises:
            launch[CounterApp](AppConfig(
                title="My Counter",
                width=400,
                height=350,
                debug=True,
            ))

    The same source compiles for web (--target wasm64-wasi) and desktop
    (native). The GuiApp trait ensures the renderer can drive the app
    uniformly regardless of platform.
    """
    _global_config = config
    _launched = True

    @parameter
    if is_wasm_target():
        # Web path: config is stored; JS runtime drives the event loop.
        # The @export wrappers in main.mojo use gui_app_exports helpers
        # (gui_app_init[AppType], gui_app_mount[AppType], etc.) to call
        # GuiApp trait methods. Nothing more to do here.
        pass
    else:
        # Desktop path: create window and enter event loop.
        # Phase 4 (Blitz): This will import and call:
        #     from desktop.launcher import desktop_launch
        #     desktop_launch[AppType](config)
        #
        # For now, print a message indicating desktop support is pending.
        print(
            "launch(): desktop renderer not yet implemented"
            " (Phase 4 — Blitz). Config stored: "
            + config.title
        )


fn launch(config: AppConfig = AppConfig()):
    """Store launch configuration without specifying an app type.

    This is a convenience overload for the WASM target where the app type
    is determined by the @export wrappers (each example builds with a
    specific AppType alias). On native targets, prefer the parametric
    version launch[AppType](config) which can dispatch to the desktop
    renderer.

    Args:
        config: Optional application configuration (title, size, debug).
    """
    _global_config = config
    _launched = True
