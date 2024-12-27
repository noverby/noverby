{...}: {
  xdg = {
    enable = true;
    mimeApps = {
      enable = true;
      defaultApplications = let
        zedMimes = [
          "text/plain"
          "text/markdown"
          "text/x-markdown"
          "text/x-python"
          "text/x-script.python"
          "text/x-c"
          "text/x-c++"
          "text/x-java"
          "text/javascript"
          "text/x-javascript"
          "text/x-typescript"
          "text/x-rust"
          "text/x-go"
          "text/x-shellscript"
          "text/x-scala"
          "text/x-ruby"
          "text/x-perl"
          "text/x-log"
          "text/x-makefile"
          "text/x-csrc"
          "text/x-chdr"
          "text/x-c++src"
          "text/x-c++hdr"
          "text/x-yaml"
          "text/x-toml"
          "text/xml"
          "text/json"
          "application/json"
          "application/x-yaml"
          "application/xml"
          "application/javascript"
          "application/x-shellscript"
          "application/x-perl"
          "application/x-ruby"
          "application/x-python"
        ];
      in
        {
          browser = "unbrave-browser.desktop";
          "x-scheme-handler/http" = "unbrave-browser.desktop";
          "x-scheme-handler/https" = "unbrave-browser.desktop";
        }
        // builtins.listToAttrs (map (mime: {
            name = mime;
            value = "dev.zed.Zed.desktop";
          })
          zedMimes);
    };
    desktopEntries = {
      unbrave-browser = {
        name = "Unbrave Browser";
        genericName = "Web Browser";
        comment = "Browse the World Wide Web";
        exec = "brave %U";
        terminal = false;
        type = "Application";
        icon = ./unbrave.svg;
        settings = {
          StartupWMClass = "brave-browser";
        };
        categories = ["Network" "WebBrowser"];
        mimeType = [
          "text/html"
          "text/xml"
          "application/xhtml+xml"
          "application/xml"
          "application/rss+xml"
          "application/rdf+xml"
          "x-scheme-handler/http"
          "x-scheme-handler/https"
          "x-scheme-handler/ftp"
          "x-scheme-handler/chrome"
          "video/webm"
          "application/x-xpinstall"
        ];
        startupNotify = true;
        actions = {
          "new-window" = {
            name = "New Window";
            exec = "brave";
          };
          "new-private-window" = {
            name = "New Private Window";
            exec = "brave --incognito";
          };
        };
      };
      brave-browser = {
        name = "";
        settings.NoDisplay = "true";
      };
    };
  };
}
