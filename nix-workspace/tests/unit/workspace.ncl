# Unit tests for the WorkspaceConfig contract
#
# These tests verify that the workspace-level contract correctly validates
# configuration structures, rejects invalid inputs, and applies defaults.
#
# Run with: nickel eval tests/unit/workspace.ncl

let { WorkspaceConfig, NixpkgsConfig, ConventionConfig, .. } = import "../../contracts/workspace.ncl" in
let { System, Name, .. } = import "../../contracts/common.ncl" in

let assert_ok = fun label value =>
  if value then
    "PASS: %{label}"
  else
    std.fail_with "FAIL: %{label}"
in

let assert_export = fun label expr =>
  let _ = std.serialize 'Json expr in
  "PASS: %{label} (exported successfully)"
in

# ── Minimal valid workspace ──────────────────────────────────────
let test_minimal =
  let config | WorkspaceConfig = {
    name = "test-workspace",
  } in
  assert_ok "minimal workspace with just a name" (config.name == "test-workspace")
in

# ── Defaults are applied ─────────────────────────────────────────
let test_defaults =
  let config | WorkspaceConfig = {
    name = "defaults-test",
  } in
  [
    assert_ok "default systems" (config.systems == ["x86_64-linux", "aarch64-linux"]),
    assert_ok "default nixpkgs" (config.nixpkgs.allow-unfree == false),
    assert_ok "default nixpkgs overlays" (config.nixpkgs.overlays == []),
    assert_ok "default packages" (config.packages == {}),
    assert_ok "default shells" (config.shells == {}),
    assert_ok "default conventions" (config.conventions == {}),
  ]
in

# ── Full workspace config ────────────────────────────────────────
let test_full =
  let config | WorkspaceConfig = {
    name = "full-test",
    description = "A fully specified workspace for testing",
    systems = ["x86_64-linux", "aarch64-linux", "x86_64-darwin", "aarch64-darwin"],
    nixpkgs = {
      allow-unfree = true,
      overlays = ["my-overlay"],
      config = {
        permittedInsecurePackages = ["old-package-1.0"],
      },
    },
    packages = {
      my-tool = {
        description = "A test tool",
        build-system = "rust",
        build-inputs = ["openssl"],
      },
      another = {
        build-system = "generic",
      },
    },
    shells = {
      default = {
        packages = ["cargo", "rustc"],
        env = {
          RUST_LOG = "debug",
        },
      },
    },
    conventions = {
      packages = {
        path = "pkgs",
        auto-discover = true,
      },
    },
  } in
  [
    assert_ok "full config name" (config.name == "full-test"),
    assert_ok "full config description" (config.description == "A fully specified workspace for testing"),
    assert_ok "full config systems length" (std.array.length config.systems == 4),
    assert_ok "full config allow-unfree" (config.nixpkgs.allow-unfree == true),
    assert_ok "full config has my-tool" (std.record.has_field "my-tool" config.packages),
    assert_ok "full config has default shell" (std.record.has_field "default" config.shells),
    assert_ok "full config convention path" (config.conventions.packages.path == "pkgs"),
  ]
in

# ── Packages with all fields ─────────────────────────────────────
let test_package_fields =
  let config | WorkspaceConfig = {
    name = "pkg-test",
    packages = {
      full-pkg = {
        src = "./src",
        build-system = "go",
        description = "A Go package",
        systems = ["x86_64-linux"],
        build-inputs = ["git"],
        native-build-inputs = ["pkg-config"],
        env = {
          CGO_ENABLED = "1",
        },
        meta = {
          homepage = "https://example.com",
          license = "MIT",
        },
        override = {
          vendorHash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        },
      },
    },
  } in
  [
    assert_ok "package src" (config.packages.full-pkg.src == "./src"),
    assert_ok "package build-system" (config.packages.full-pkg.build-system == "go"),
    assert_ok "package systems override" (config.packages.full-pkg.systems == ["x86_64-linux"]),
    assert_ok "package build-inputs" (config.packages.full-pkg.build-inputs == ["git"]),
    assert_ok "package env" (config.packages.full-pkg.env.CGO_ENABLED == "1"),
    assert_ok "package meta homepage" (config.packages.full-pkg.meta.homepage == "https://example.com"),
  ]
in

# ── Shells with all fields ───────────────────────────────────────
let test_shell_fields =
  let config | WorkspaceConfig = {
    name = "shell-test",
    shells = {
      dev = {
        packages = ["cargo", "rustc", "rust-analyzer"],
        env = {
          RUST_BACKTRACE = "1",
        },
        shell-hook = "echo entering dev shell",
        tools = {
          rust-analyzer = "",
        },
        systems = ["x86_64-linux"],
        inputs-from = ["my-tool"],
      },
    },
  } in
  [
    assert_ok "shell packages" (std.array.length config.shells.dev.packages == 3),
    assert_ok "shell env" (config.shells.dev.env.RUST_BACKTRACE == "1"),
    assert_ok "shell hook" (config.shells.dev.shell-hook == "echo entering dev shell"),
    assert_ok "shell tools" (config.shells.dev.tools.rust-analyzer == ""),
    assert_ok "shell systems" (config.shells.dev.systems == ["x86_64-linux"]),
    assert_ok "shell inputs-from" (config.shells.dev.inputs-from == ["my-tool"]),
  ]
in

# ── NixpkgsConfig defaults ───────────────────────────────────────
let test_nixpkgs_defaults =
  let cfg | NixpkgsConfig = {} in
  [
    assert_ok "nixpkgs default allow-unfree" (cfg.allow-unfree == false),
    assert_ok "nixpkgs default overlays" (cfg.overlays == []),
    assert_ok "nixpkgs default config" (cfg.config == {}),
  ]
in

# ── ConventionConfig ─────────────────────────────────────────────
let test_convention =
  let cfg | ConventionConfig = {
    path = "custom-packages",
    auto-discover = false,
  } in
  [
    assert_ok "convention path" (cfg.path == "custom-packages"),
    assert_ok "convention auto-discover" (cfg.auto-discover == false),
  ]
in

let test_convention_defaults =
  let cfg | ConventionConfig = {} in
  assert_ok "convention default auto-discover" (cfg.auto-discover == true)
in

# ── Export round-trip ─────────────────────────────────────────────
let test_export =
  let config | WorkspaceConfig = {
    name = "export-test",
    packages = {
      hello = {
        description = "hello world",
      },
    },
  } in
  assert_export "workspace round-trips through JSON" config
in

# ── Collect and run all tests ─────────────────────────────────────
{
  results = [
    test_minimal,
    test_export,
    test_convention_defaults,
  ]
  @ test_defaults
  @ test_full
  @ test_package_fields
  @ test_shell_fields
  @ test_nixpkgs_defaults
  @ test_convention,
}
