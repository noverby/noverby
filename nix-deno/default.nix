{
  perSystemLib.buildDenoProject = pkgs:
    import ./buildDenoProject.nix {
      inherit (pkgs) lib stdenvNoCC deno fetchurl jq writeText;
    };
}
