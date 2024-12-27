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
    sessionVariables = {
      EDITOR = "vi";
      VISUAL = "vi";
      DIRENV_LOG_FORMAT = "";
      PYTHONSTARTUP = "${homeDirectory}/.pystartup";
      GRANTED_ALIAS_CONFIGURED = "true";
      # XR
      XR_RUNTIME_JSON = "${pkgs.monado}/share/openxr/1/openxr_monado.json";
      XRT_COMPOSITOR_FORCE_XCB = "1";
      XRT_COMPOSITOR_XCB_FULLSCREEN = "1";
    };
  };
}
