{
  description = "Personal Monorepo";

  inputs = {
    devenv-root = {
      url = "file+file:///dev/null";
      flake = false;
    };
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nixos-hardware.url = "github:NixOS/nixos-hardware/master";
    home-manager = {
      url = "github:nix-community/home-manager/master";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flakelight = {
      url = "github:accelbread/flakelight";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    devenv = {
      url = "github:cachix/devenv";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.git-hooks.follows = "git-hooks";
    };
    stardustxr = {
      url = "github:StardustXR/server";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flatland.follows = "flatland";
        hercules-ci-effects.follows = "hercules-ci-effects";
        flake-parts.follows = "flake-parts";
      };
    };
    flatland = {
      url = "github:StardustXR/flatland";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    zen-browser = {
      url = "github:0xc000022070/zen-browser-flake";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    spicetify-nix = {
      url = "github:Gerg-L/spicetify-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    catppuccin = {
      url = "github:catppuccin/nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
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
  };

  outputs = inputs:
    inputs.flakelight ./. {
      inherit inputs;
      nixpkgs.config = {
        allowUnfree = true;
        segger-jlink.acceptLicense = true;
        permittedInsecurePackages = [
          "segger-jlink-qt4-810"
        ];
      };
      nixDir = ./.;
      nixDirAliases = {
        nixosConfigurations = ["devices"];
        nixosModules = ["modules/nixos"];
        homeModules = ["modules/home-manager"];
        devShells = ["shells"];
        withOverlays = ["with-overlays"];
      };
    };
}
