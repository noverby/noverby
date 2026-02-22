# nix-workspace library — main entry point
#
# This module exports the `mkWorkspace` function that serves as the primary
# interface for nix-workspace. It is designed to be called from a flake.nix:
#
#   outputs = inputs:
#     inputs.nix-workspace ./. {
#       inherit inputs;
#     };
#
# The function orchestrates the full pipeline:
#   1. Discovery  — scan convention directories for .ncl files
#   2. Evaluation — run Nickel to validate and export the config as JSON (IFD)
#   3. Building   — construct flake outputs from the validated config
#
{
  nixpkgs,
  nix-workspace,
}: let
  inherit (nixpkgs) lib;

  # Import sub-modules, passing the shared `lib`.
  discover = import ./discover.nix {inherit lib;};
  evalNickel = import ./eval-nickel.nix {inherit lib;};
  systemsLib = import ./systems.nix {inherit lib;};
  packageBuilder = import ./builders/packages.nix {inherit lib;};
  shellBuilder = import ./builders/shells.nix {inherit lib;};
  machineBuilder = import ./builders/machines.nix {inherit lib;};
  moduleBuilder = import ./builders/modules.nix {inherit lib;};

  # The contracts directory shipped with nix-workspace.
  contractsDir = nix-workspace.contracts;

  # ── mkWorkspace ─────────────────────────────────────────────────
  #
  # Type: Path -> AttrSet -> AttrSet
  #
  # Arguments:
  #   workspaceRoot — Path to the workspace directory (typically ./.)
  #   config        — Configuration attrset, must contain at least `inputs`
  #     config.inputs       — The flake inputs (must include `nixpkgs`)
  #     config.systems      — Optional list of systems (overrides workspace.ncl)
  #     config.nixpkgs      — Optional nixpkgs config overrides
  #
  # Returns: A standard flake outputs attrset.
  #
  mkWorkspace = workspaceRoot: config: let
    inputs = config.inputs or {};
    userNixpkgs = inputs.nixpkgs or nixpkgs;

    # ── Phase 1: Discovery ──────────────────────────────────────
    #
    # Scan the workspace root for convention directories and .ncl files.
    discovered = discover.discoverAll workspaceRoot (config.conventions or null);

    discoveredPackages = discovered.packages or {};
    discoveredShells = discovered.shells or {};
    discoveredMachines = discovered.machines or {};
    discoveredModules = discovered.modules or {};
    discoveredHome = discovered.home or {};

    hasNclFiles =
      (discoveredPackages != {})
      || (discoveredShells != {})
      || (discoveredMachines != {})
      || (discoveredModules != {})
      || (discoveredHome != {})
      || builtins.pathExists (workspaceRoot + "/workspace.ncl");

    # Discover .nix implementation files alongside .ncl configs for modules.
    # Modules have a dual structure: .ncl for Nickel config, .nix for NixOS implementation.
    discoveredModuleNixFiles = moduleBuilder.discoverNixFiles workspaceRoot "modules";
    discoveredHomeNixFiles = moduleBuilder.discoverNixFiles workspaceRoot "home";

    # ── Phase 2: Nickel evaluation ──────────────────────────────
    #
    # Evaluate workspace.ncl (and discovered .ncl files) through Nickel
    # using IFD. This produces a fully validated JSON configuration tree.
    #
    # We need a "bootstrap" pkgs to obtain the `nickel` binary for IFD.
    # Use builtins.currentSystem when available, otherwise default to x86_64-linux.
    bootstrapSystem = builtins.currentSystem or "x86_64-linux";
    bootstrapPkgs = import userNixpkgs {system = bootstrapSystem;};

    workspaceConfig =
      if hasNclFiles
      then
        evalNickel.evalWorkspace {
          inherit
            bootstrapPkgs
            contractsDir
            workspaceRoot
            discoveredPackages
            discoveredShells
            discoveredMachines
            discoveredModules
            discoveredHome
            ;
        }
      else evalNickel.emptyConfig;

    # Allow the Nix-side config to override certain fields from workspace.ncl.
    effectiveConfig =
      workspaceConfig
      // (lib.optionalAttrs (config ? systems) {inherit (config) systems;})
      // (lib.optionalAttrs (config ? nixpkgs) {
        nixpkgs = (workspaceConfig.nixpkgs or {}) // config.nixpkgs;
      });

    systems = effectiveConfig.systems or systemsLib.defaultSystems;
    nixpkgsConfig = let
      ncl = effectiveConfig.nixpkgs or {};
    in
      (lib.optionalAttrs (ncl ? allow-unfree) {allowUnfree = ncl.allow-unfree;})
      // (ncl.config or {});

    packageConfigs = effectiveConfig.packages or {};
    shellConfigs = effectiveConfig.shells or {};
    machineConfigs = effectiveConfig.machines or {};
    moduleConfigs = effectiveConfig.modules or {};
    homeConfigs = effectiveConfig.home or {};

    # ── Phase 3: Build outputs ──────────────────────────────────
    #
    # Construct standard flake outputs with system multiplexing.

    # ── Per-system outputs (packages, devShells) ────────────────
    perSystemOutputs = systemsLib.eachSystem systems (
      system: let
        pkgs = import userNixpkgs {
          inherit system;
          config = nixpkgsConfig;
        };

        # Build packages for this system
        builtPackages =
          lib.mapAttrs (
            name: cfg: let
              buildSystem = cfg.build-system or "generic";
              builder =
                {
                  generic = packageBuilder.buildGeneric;
                  rust = packageBuilder.buildRust;
                  go = packageBuilder.buildGo;
                }
                .${
                  buildSystem
                }
                or (throw "nix-workspace: unknown build-system '${buildSystem}' for package '${name}'");
            in
              builder pkgs workspaceRoot name cfg
          )
          (
            lib.filterAttrs (
              _: cfg:
                builtins.elem system (cfg.systems or systems)
            )
            packageConfigs
          );

        # Build dev shells for this system
        builtShells =
          lib.mapAttrs (
            name: cfg:
              shellBuilder.buildShell pkgs name cfg builtPackages
          )
          (
            lib.filterAttrs (
              _: cfg:
                builtins.elem system (cfg.systems or systems)
            )
            shellConfigs
          );

        # If there's exactly one package and no explicit default shell,
        # create a default shell with that package's build inputs.
        hasDefaultShell = builtShells ? default;
        packageNames = builtins.attrNames builtPackages;
        autoDefaultShell =
          if !hasDefaultShell && builtins.length packageNames == 1
          then let
            singlePkgName = builtins.head packageNames;
          in {
            default = pkgs.mkShell {
              name = "nix-workspace-default";
              inputsFrom = [builtPackages.${singlePkgName}];
            };
          }
          else {};
      in
        (lib.optionalAttrs (builtPackages != {}) {
          packages.${system} = builtPackages;
        })
        // (lib.optionalAttrs (builtShells != {} || autoDefaultShell != {}) {
          devShells.${system} = builtShells // autoDefaultShell;
        })
    );

    # ── Non-per-system outputs (nixosConfigurations, modules) ───
    #
    # NixOS configurations are not per-system in the flake output schema —
    # each configuration declares its own system internally.

    # Merge discovered .nix files with any paths declared in Nickel module configs.
    # The .nix files serve as the implementation; the .ncl configs provide metadata.
    resolvedModulePaths = let
      # Start with discovered .nix files
      nixPaths = discoveredModuleNixFiles;
      # Overlay with paths from Nickel configs (if any)
      nclPaths =
        lib.filterAttrs (_: cfg: cfg ? path) moduleConfigs;
      nclResolvedPaths =
        lib.mapAttrs (
          _: cfg:
            if lib.hasPrefix "./" cfg.path || lib.hasPrefix "../" cfg.path
            then workspaceRoot + "/${cfg.path}"
            else if lib.hasPrefix "/" cfg.path
            then /. + cfg.path
            else workspaceRoot + "/${cfg.path}"
        )
        nclPaths;
    in
      nixPaths // nclResolvedPaths;

    resolvedHomePaths = let
      nixPaths = discoveredHomeNixFiles;
      nclPaths =
        lib.filterAttrs (_: cfg: cfg ? path) homeConfigs;
      nclResolvedPaths =
        lib.mapAttrs (
          _: cfg:
            if lib.hasPrefix "./" cfg.path || lib.hasPrefix "../" cfg.path
            then workspaceRoot + "/${cfg.path}"
            else if lib.hasPrefix "/" cfg.path
            then /. + cfg.path
            else workspaceRoot + "/${cfg.path}"
        )
        nclPaths;
    in
      nixPaths // nclResolvedPaths;

    # Build NixOS machine configurations
    nixosConfigurations =
      if machineConfigs != {}
      then
        machineBuilder.buildAllMachines {
          nixpkgs = userNixpkgs;
          inherit workspaceRoot machineConfigs;
          workspaceModules = resolvedModulePaths;
          homeModules = resolvedHomePaths;
          extraInputs = inputs;
        }
      else {};

    # Build NixOS module flake outputs
    nixosModules =
      if moduleConfigs != {} || resolvedModulePaths != {}
      then let
        # Ensure every discovered .nix module has a config entry (even if empty)
        effectiveModuleConfigs =
          (lib.mapAttrs (_: _: {}) resolvedModulePaths)
          // moduleConfigs;
      in
        moduleBuilder.buildAllNixosModules {
          inherit workspaceRoot;
          moduleConfigs = effectiveModuleConfigs;
          discoveredPaths = resolvedModulePaths;
        }
      else {};

    # Build home-manager module flake outputs
    homeModules =
      if homeConfigs != {} || resolvedHomePaths != {}
      then let
        effectiveHomeConfigs =
          (lib.mapAttrs (_: _: {}) resolvedHomePaths)
          // homeConfigs;
      in
        moduleBuilder.buildAllHomeModules {
          inherit workspaceRoot;
          homeConfigs = effectiveHomeConfigs;
          discoveredPaths = resolvedHomePaths;
        }
      else {};
  in
    perSystemOutputs
    // (lib.optionalAttrs (nixosConfigurations != {}) {
      inherit nixosConfigurations;
    })
    // (lib.optionalAttrs (nixosModules != {}) {
      inherit nixosModules;
    })
    // (lib.optionalAttrs (homeModules != {}) {
      inherit homeModules;
    });
in {
  inherit mkWorkspace;

  # Re-export sub-modules for advanced usage / testing
  inherit discover systemsLib packageBuilder shellBuilder machineBuilder moduleBuilder evalNickel;
}
