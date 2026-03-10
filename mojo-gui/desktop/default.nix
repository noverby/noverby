{
  devShells.mojo-gui-desktop = pkgs: let
    libmojo-webview = pkgs.stdenv.mkDerivation {
      pname = "libmojo-webview";
      version = "0.1.0";

      src = ./shim;

      nativeBuildInputs = with pkgs; [
        pkg-config
      ];

      buildInputs = with pkgs; [
        gtk4
        webkitgtk_6_0
        glib
      ];

      buildPhase = ''
        cc -shared -fPIC -o libmojo_webview.so mojo_webview.c \
          $(pkg-config --cflags --libs gtk4 webkitgtk-6.0)
      '';

      installPhase = ''
        mkdir -p $out/lib $out/include
        cp libmojo_webview.so $out/lib/
        cp mojo_webview.h $out/include/
      '';

      meta = with pkgs.lib; {
        description = "C shim for GTK4/WebKitGTK with a Mojo-friendly polling API";
        license = licenses.mit;
        platforms = platforms.linux;
      };
    };
  in {
    packages = with pkgs; [
      just
      mojo
      deno

      # GTK4 + WebKitGTK for the webview
      gtk4
      webkitgtk_6_0
      glib
      pkg-config

      # The compiled C shim library
      libmojo-webview
    ];

    env = {
      # Point Mojo FFI at the compiled shim library
      MOJO_WEBVIEW_LIB = "${libmojo-webview}/lib/libmojo_webview.so";

      # Ensure the linker can find GTK/WebKit at runtime
      LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
        libmojo-webview
        pkgs.gtk4
        pkgs.webkitgtk_6_0
        pkgs.glib
      ];

      # pkg-config for C compilation
      PKG_CONFIG_PATH = pkgs.lib.makeSearchPath "lib/pkgconfig" [
        pkgs.gtk4.dev
        pkgs.webkitgtk_6_0.dev
        pkgs.glib.dev
      ];
    };
  };
}
