{
  pkgs,
  username,
  homeDirectory,
  stateVersion,
  ...
}: {
  home = {
    inherit username homeDirectory stateVersion;
    enableDebugInfo = true;
    shell = {
      enableBashIntegration = true;
      enableNushellIntegration = true;
    };
    sessionVariables = {
      EDITOR = "vi";
      VISUAL = "vi";
      BATDIFF_USE_DELTA = "true";
      DIRENV_LOG_FORMAT = "";
      PYTHONSTARTUP = "${homeDirectory}/.pystartup";
      GRANTED_ALIAS_CONFIGURED = "true";

      # GStreamer
      GST_PLUGIN_SYSTEM_PATH_1_0 = with pkgs.gst_all_1; "${gstreamer.out}/lib/gstreamer-1.0:${gst-plugins-base}/lib/gstreamer-1.0:${gst-plugins-good}/lib/gstreamer-1.0";

      # XR
      XR_RUNTIME_JSON = "${pkgs.monado}/share/openxr/1/openxr_monado.json";
      XRT_COMPOSITOR_FORCE_XCB = "1";
      XRT_COMPOSITOR_XCB_FULLSCREEN = "1";

      DEVENV_ENABLE_HOOKS = "1";
    };
  };
}
