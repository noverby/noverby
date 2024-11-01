{...}: {
  xdg = {
    enable = true;
    desktopEntries = {
      unbrave-browser = {
        name = "Unbrave Browser";
        genericName = "Web Browser";
        comment = "Browse the World Wide Web";
        exec = "brave --app-name=\"Unbrave\" %U";
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
            exec = "brave --app-name=\"Unbrave\"";
          };
          "new-private-window" = {
            name = "New Private Window";
            exec = "brave --app-name=\"Unbrave\" --incognito";
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
