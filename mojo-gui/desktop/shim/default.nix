{pkgs ? import <nixpkgs> {}}:
pkgs.stdenv.mkDerivation {
  pname = "libmojo-webview";
  version = "0.1.0";

  src = ./.;

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
}
