let
  nixpkgs = import (builtins.fetchTarball {url = "https://github.com/NixOS/nixpkgs/archive/nixpkgs-unstable.tar.gz";}) {};
in
  nixpkgs.buildEnv {
    name = "spindle-workflow-env";
    paths = [
      nixpkgs.cachix
    ];
  }
