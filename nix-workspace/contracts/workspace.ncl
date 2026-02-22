# Top-level workspace configuration contract
# This is the primary contract applied to workspace.ncl files.

let { System, Name, .. } = import "./common.ncl" in
let { PackageConfig, .. } = import "./package.ncl" in
let { ShellConfig, .. } = import "./shell.ncl" in

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

    conventions
      | { _ : ConventionConfig }
      | doc m%"
        Override directory conventions for auto-discovery.
        Keys are convention names (e.g. "packages", "shells").
      "%
      | default
      = {},
  },
}
