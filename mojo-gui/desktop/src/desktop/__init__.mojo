"""mojo-gui/desktop — Desktop renderer for mojo-gui applications.

This package will provide a native desktop GUI backend using Blitz
(Stylo + Taffy + Vello + Winit + AccessKit). The core mojo-gui framework
writes binary mutations to a heap buffer, and the desktop renderer
interprets them natively via Blitz's DOM/CSS/layout/paint pipeline.

Status: 🔮 Future — not yet implemented.

Modules (planned):
  - engine:  Blitz rendering engine integration
  - bridge:  Mutation buffer interpreter (binary opcodes → Blitz DOM ops)
  - app:     DesktopApp entry point and event loop (Winit-based)

Usage (planned):

    from desktop.app import DesktopApp

    fn main() raises:
        var app = DesktopApp(title="My App", width=800, height=600)
        app.init()
        app.run()
"""
