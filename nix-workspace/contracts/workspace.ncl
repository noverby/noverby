# Top-level workspace configuration contract
# This is the primary contract applied to workspace.ncl files.
#
# Updated for v0.3 to support subworkspace dependencies.

let { System, Name, .. } = import "./common.ncl" in
let { PackageConfig, .. } = import "./package.ncl" in
let { ShellConfig, .. } = import "./shell.ncl" in
let { MachineConfig, .. } = import "./machine.ncl" in
let { ModuleConfig, HomeConfig, .. } = import "./module.ncl" in

{
  NixpkgsConfig = {
    allow-unfree
      | Bool
      | doc "Allow unfree packages from nixpkgs"
      | default
      = false,

    overlays
      | Array String
      | doc "Overlay references to apply to nixpkgs"
      | default
      = [],

    config
      | { _ : Dyn }
      | doc "Additional nixpkgs config options"
      | default
      = {},
  },

  ConventionConfig = {
    path
      | String
      | doc "Directory path (relative to workspace root)"
      | optional,

    auto-discover
      | Bool
      | doc "Enable auto-discovery of .ncl files in this directory"
      | default
      = true,
  },

  WorkspaceConfig = {
    name
      | Name
      | doc "Workspace name (used for namespacing in subworkspaces)",

    description
      | String
      | doc "Human-readable description of the workspace"
      | optional,

    systems
      | Array System
      | doc "Target systems for system-multiplexed outputs"
      | default
      = ["x86_64-linux", "aarch64-linux"],

    nixpkgs
      | NixpkgsConfig
      | doc "Nixpkgs configuration"
      | default
      = {},

    packages
      | { _ : PackageConfig }
      | doc "Package definitions (auto-discovered from packages/ or declared here)"
      | default
      = {},

    shells
      | { _ : ShellConfig }
      | doc "Development shell definitions (auto-discovered from shells/ or declared here)"
      | default
      = {},

    machines
      | { _ : MachineConfig }
      | doc m%"
        NixOS machine configurations (auto-discovered from machines/ or declared here).
        Each entry produces a nixosConfigurations.<name> flake output.
      "%
      | default
      = {},

    modules
      | { _ : ModuleConfig }
      | doc m%"
        NixOS module definitions (auto-discovered from modules/ or declared here).
        Each entry produces a nixosModules.<name> flake output.
      "%
      | default
      = {},

    home
      | { _ : HomeConfig }
      | doc m%"
        Home-manager module definitions (auto-discovered from home/ or declared here).
        Each entry produces a homeModules.<name> flake output.
      "%
      | default
      = {},

    dependencies
      | { _ : DependencyRef }
      | doc m%"
        Cross-subworkspace dependency declarations.
        Keys are local aliases; values are sibling subworkspace names.
        Only meaningful for subworkspaces — ignored for the root workspace.

        Example:
          dependencies = {
            wasm = "mojo-wasm",
          }

        This declares that this subworkspace depends on a sibling
        subworkspace named "mojo-wasm", referenced locally as "wasm".
      "%
      | default
      = {},

    conventions
      | { _ : ConventionConfig }
      | doc m%"
        Override directory conventions for auto-discovery.
        Keys are convention names (e.g. "packages", "shells", "machines", "modules", "home").
      "%
      | default
      = {},
  },

  DependencyRef
    | doc m%"
      A reference to a sibling subworkspace by its directory name.
      Must be a valid Name (alphanumeric, hyphens, underscores, starts
      with a letter or underscore).
    "%
    = Name,
}
