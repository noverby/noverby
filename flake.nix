{
  description = "Personal Monorepo";

  inputs = {
    devenv-root = {
      url = "file+file:///dev/null";
      flake = false;
    };
    nixpkgs.follows = "nixos-cosmic/nixpkgs";
    nixos-hardware.url = "github:NixOS/nixos-hardware/master";
    home-manager = {
      url = "github:nix-community/home-manager/master";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flakelight = {
      url = "github:accelbread/flakelight";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    devenv = {
      url = "github:cachix/devenv";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.git-hooks.follows = "git-hooks";
    };
    flake-utils = {
      url = "github:numtide/flake-utils";
    };
    flake-compat = {
      url = "github:edolstra/flake-compat";
    };
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
    };
    hercules-ci-effects = {
      url = "github:hercules-ci/hercules-ci-effects";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-parts.follows = "flake-parts";
    };
    stardustxr = {
      url = "github:StardustXR/server";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flatland.follows = "flatland";
      inputs.hercules-ci-effects.follows = "hercules-ci-effects";
      inputs.flake-parts.follows = "flake-parts";
    };
    flatland = {
      url = "github:StardustXR/flatland";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixos-cosmic = {
      url = "github:lilyinstarlight/nixos-cosmic";
      inputs.flake-compat.follows = "flake-compat";
    };
    nix-alien = {
      url = "github:thiagokokada/nix-alien";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    zen-browser = {
      url = "github:0xc000022070/zen-browser-flake";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    flakelight,
    zen-browser,
    ...
  } @ inputs:
    flakelight ./. {
      inherit inputs;
      nixpkgs.config = {allowUnfree = true;};
      nixDir = ./.;
      nixDirAliases = {
        nixosConfigurations = ["devices"];
        nixosModules = ["nixos-modules"];
        homeModules = ["home-modules"];
        devShells = ["shells"];
      };
      withOverlays = [
        (inputs.nix-alien.overlays.default)
        (_: prev: {
          zen-browser = zen-browser.packages."${prev.system}".default;
          tailspin = prev.tailspin.overrideAttrs (old: {doCheck = false;});
        })
      ];
    };
}
