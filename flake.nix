{
  description = "Personal Monorepo";

  inputs = {
    # Pass env through input
    env = {
      url = "file+file:///dev/null";
      flake = false;
    };

    # Nix
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    nixpkgs-unstable.url = "github:NixOS/nixpkgs/nixos-unstable";
    lix = {
      url = "git+https://git.lix.systems/lix-project/lix?ref=refs/tags/2.94.0";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-compat.follows = "flake-compat";
        pre-commit-hooks.follows = "git-hooks";
      };
    };

    # Config support
    flakelight = {
      url = "github:accelbread/flakelight";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixos-hardware.url = "github:NixOS/nixos-hardware/master";
    home-manager = {
      url = "github:nix-community/home-manager/release-25.11";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    agenix = {
      url = "github:ryantm/agenix";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        home-manager.follows = "home-manager";
        systems.follows = "systems";
      };
    };
    ragenix = {
      url = "github:yaxitech/ragenix";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
        agenix.follows = "agenix";
        rust-overlay.follows = "rust-overlay";
        crane.follows = "crane";
      };
    };

    # Development
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane";
    };
    devenv = {
      url = "github:cachix/devenv";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        git-hooks.follows = "git-hooks";
        flake-compat.follows = "flake-compat";
        flake-parts.follows = "flake-parts";
      };
    };

    # XR
    non-spatial-input = {
      url = "github:StardustXR/non-spatial-input";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        crane.follows = "crane";
      };
    };

    # Apps
    zen-browser = {
      url = "github:0xc000022070/zen-browser-flake";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.home-manager.follows = "home-manager";
    };
    zed = {
      url = "github:zed-industries/zed";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        crane.follows = "crane";
        rust-overlay.follows = "rust-overlay";
        flake-compat.follows = "flake-compat";
      };
    };
    nxv = {
      url = "github:jamesbrink/nxv";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
        rust-overlay.follows = "rust-overlay";
        crane.follows = "crane";
      };
    };

    # Styling
    spicetify-nix = {
      url = "github:Gerg-L/spicetify-nix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.systems.follows = "systems";
    };
    catppuccin = {
      url = "github:catppuccin/nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nix-wallpaper = {
      url = "github:lunik1/nix-wallpaper";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
        pre-commit-hooks.follows = "git-hooks";
      };
    };

    # Transitive flake dependencies
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-compat.follows = "flake-compat";
    };
    flake-utils = {
      url = "github:numtide/flake-utils";
      inputs.systems.follows = "systems";
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
    systems.url = "github:nix-systems/default";
  };

  outputs = inputs:
    inputs.flakelight ./. {
      inherit inputs;
      nixpkgs.config = {
        allowUnfree = true;
      };
      imports = [
        ./nix/modules/flakelight/libBuiltins.nix
        ./nix/modules/flakelight/devenvModules.nix
        ./nix/modules/flakelight/devenvConfigurations.nix

        ./projects/homepage
        ./projects/mojo-wasm
        ./projects/wiki

        ./presentations
      ];
      nixDirAliases = {
        flakelightModules = ["modules/flakelight"];
        nixosConfigurations = ["configurations/nixos"];
        nixosModules = ["modules/nixos" "modules/nixos/hardware" "modules/nixos/desktop"];
        homeConfigurations = ["configurations/home-manager"];
        homeModules = ["modules/home-manager" "modules/home-manager/users" "modules/home-manager/desktop"];
        devenvConfigurations = ["configurations/devenv"];
        devenvModules = ["modules/devenv"];
        withOverlays = ["with-overlays"];
      };
    };
}
