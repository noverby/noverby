{...}: {
  xdg = {
    enable = true;
    mimeApps.defaultApplications = {
      browser = "unbrave-browser.desktop";
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
