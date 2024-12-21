{
  username,
  homeDirectory,
  stateVersion,
  ...
}: {
  home = {
    inherit username homeDirectory stateVersion;
    enableDebugInfo = true;
  };
}
