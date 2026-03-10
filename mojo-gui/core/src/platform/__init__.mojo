# Platform package — Platform abstraction layer for mojo-gui.
#
# This package provides the abstraction boundary between the renderer-agnostic
# core framework and platform-specific renderers (web, desktop, native).
#
# Sub-modules:
#   app       — PlatformApp trait, target detection helpers
#   launch    — launch() entry point, AppConfig
#   features  — PlatformFeatures, runtime capability detection

from .app import (
    PlatformApp,
    is_wasm_target,
    is_native_target,
)
from .launch import (
    AppConfig,
    launch,
    get_launch_config,
    has_launched,
)
from .features import (
    PlatformFeatures,
    register_features,
    current_features,
    features_registered,
    default_features,
    web_features,
    desktop_blitz_features,
    native_features,
)
