{
  stateVersion,
  src,
  lib,
  ...
}: {
  system = {
    inherit stateVersion;
    # Store copy of all Nix files in /nix/var/nix/profiles/system/full-config
    systemBuilderCommands = let
      nixFiles =
        lib.filterSource (
          path: type:
            type == "directory" || lib.match ".*\\.nix$" (baseNameOf path) != null
        )
        src;
    in "ln -s ${nixFiles} $out/full-config";
  };
}
