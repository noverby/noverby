"""mojo-gui/desktop — Desktop renderer for mojo-gui applications.

This package provides a native desktop GUI backend using GTK4 + WebKitGTK.
The core mojo-gui framework writes binary mutations to a heap buffer, and
the desktop renderer sends them to an embedded webview via IPC.

Modules:
  - webview: FFI bindings to the libmojo_webview C shim
  - bridge:  Mutation buffer + event polling bridge
  - app:     DesktopApp entry point and event loop

Usage:

    from desktop.app import DesktopApp

    fn main() raises:
        var app = DesktopApp(title="My App", width=800, height=600)
        app.init()
        app.run()
"""
