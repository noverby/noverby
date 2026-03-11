"""mojo-gui/xr/native — XR renderer for mojo-gui applications.

This package provides an XR (extended reality) backend that renders
mojo-gui panels into 3D space via OpenXR. Each XR panel owns an
independent Blitz DOM document (reusing the same rendering stack as the
desktop renderer) rendered to an offscreen GPU texture by Vello. The
OpenXR compositor places these textures as quad layers in the XR scene.

Architecture:

    mojo-gui/core (MutationWriter)
        │ binary opcode buffer (per-panel)
        ▼
    xr/scene.mojo (XRScene — panel registry + event routing)
        │ routes mutations to the correct panel
        ▼
    xr/panel.mojo (XRPanel — 2D DOM document + 3D transform)
        │ owns a MutationInterpreter + Blitz document
        ▼
    xr/xr_blitz.mojo (XRBlitz FFI wrapper)
        │ DLHandle calls
        ▼
    libmojo_xr.so (Rust cdylib — xr/native/shim/src/lib.rs)
        │ Blitz DOM ops → Vello → offscreen texture per panel
        │ OpenXR session + frame loop + compositor
        ▼
    OpenXR Runtime → HMD

Modules:
    panel:        XRPanel, PanelConfig, PanelState, Vec3, Quaternion
    scene:        XRScene, XREvent, RaycastHit, layout helpers
    xr_blitz:     Mojo FFI bindings to libmojo_xr (XR Blitz shim)
    xr_launcher:  xr_launch[AppType: GuiApp]() — OpenXR entry point

Usage (via the unified launch() entry point — single-panel apps):

    from platform import launch, AppConfig
    from counter import CounterApp

    fn main() raises:
        launch[CounterApp](AppConfig(title="Counter", width=400, height=350))

    # When compiled with --feature xr, launch() calls xr_launch[CounterApp]()
    # which creates an XR session, wraps the app in a default panel, and
    # enters the OpenXR frame loop.

Usage (direct, for multi-panel XR apps — future, Step 5.9):

    from xr.panel import XRPanel, PanelConfig, Vec3
    from xr.scene import XRScene, create_single_panel_scene
    from xr.xr_launcher import xr_launch

Current Status:
    Step 5.1 — Design phase. Panel and scene abstractions defined.
    The XR shim (Step 5.2) and FFI bindings (Step 5.3) are not yet
    implemented. Mojo-side types are complete and ready for integration.
"""

from .panel import (
    XRPanel,
    PanelConfig,
    PanelState,
    Vec3,
    Quaternion,
    default_panel_config,
    dashboard_panel_config,
    tooltip_panel_config,
    hand_anchored_panel_config,
)

from .scene import (
    XRScene,
    XREvent,
    RaycastHit,
    create_single_panel_scene,
    create_dual_panel_scene,
    MAX_PANELS,
)
