{
  description = "A Nickel-powered workspace manager for Nix flakes";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    systems.url = "github:nix-systems/default";
  };

  outputs = {
    self,
    nixpkgs,
    systems,
  }: let
    lib = import ./lib {
      inherit nixpkgs;
      nix-workspace = self;
    };

    eachSystem = f: let
      systemsList = import systems;
    in
      builtins.foldl'
      (acc: system: nixpkgs.lib.recursiveUpdate acc (f system))
      {}
      systemsList;
  in
    {
      # Make the flake callable: inputs.nix-workspace ./. { inherit inputs; }
      __functor = _: lib.mkWorkspace;

      # Expose the library for advanced usage
      inherit lib;

      # Contracts source for consumers
      contracts = ./contracts;

      # Plugin definitions for consumers
      plugins = ./plugins;
    }
    // eachSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
      in {
        # Dev shell for working on nix-workspace itself
        devShells.${system}.default = pkgs.mkShell {
          name = "nix-workspace-dev";
          packages = with pkgs; [
            nickel
            nls
          ];
        };

        # Run contract tests
        checks.${system} = {
          contracts =
            pkgs.runCommand "nix-workspace-contract-tests"
            {
              nativeBuildInputs = [pkgs.nickel];
            }
            ''
              echo "==> Typechecking contracts..."
              nickel typecheck ${./contracts/common.ncl}
              nickel typecheck ${./contracts/package.ncl}
              nickel typecheck ${./contracts/shell.ncl}
              nickel typecheck ${./contracts/machine.ncl}
              nickel typecheck ${./contracts/module.ncl}
              nickel typecheck ${./contracts/workspace.ncl}

              echo "==> Running unit tests..."
              for test in ${./tests/unit}/*.ncl; do
                echo "  -> $(basename $test)"
                nickel eval "$test"
              done

              echo "==> Typechecking plugin contracts..."
              nickel typecheck ${./contracts/plugin.ncl}
              echo "  -> plugin.ncl OK"

              echo "==> Validating plugin definitions..."
              cat > validate-plugin-rust.ncl << NICKEL
              let { PluginConfig, .. } = import "${./contracts/plugin.ncl}" in
              (import "${./plugins/rust/plugin.ncl}") | PluginConfig
              NICKEL
              nickel export validate-plugin-rust.ncl > /dev/null
              echo "  -> plugins/rust OK"

              cat > validate-plugin-go.ncl << NICKEL
              let { PluginConfig, .. } = import "${./contracts/plugin.ncl}" in
              (import "${./plugins/go/plugin.ncl}") | PluginConfig
              NICKEL
              nickel export validate-plugin-go.ncl > /dev/null
              echo "  -> plugins/go OK"

              echo "==> Validating example workspaces..."
              # Build a wrapper that applies the WorkspaceConfig contract to the example
              cat > validate-minimal.ncl << NICKEL
              let { WorkspaceConfig, .. } = import "${./contracts/workspace.ncl}" in
              (import "${./examples/minimal/workspace.ncl}") | WorkspaceConfig
              NICKEL
              nickel export validate-minimal.ncl > /dev/null
              echo "  -> examples/minimal OK"

              echo "==> Validating workspace with plugins field..."
              cat > validate-with-plugins.ncl << NICKEL
              let { WorkspaceConfig, .. } = import "${./contracts/workspace.ncl}" in
              ({
                name = "test-with-plugins",
                plugins = ["nix-workspace-rust", "nix-workspace-go"],
              }) | WorkspaceConfig
              NICKEL
              nickel export validate-with-plugins.ncl > /dev/null
              echo "  -> workspace with plugins field OK"

              cat > validate-nixos.ncl << NICKEL
              let { WorkspaceConfig, .. } = import "${./contracts/workspace.ncl}" in
              (import "${./examples/nixos/workspace.ncl}") | WorkspaceConfig
              NICKEL
              nickel export validate-nixos.ncl > /dev/null
              echo "  -> examples/nixos OK"

              # v0.3: Validate monorepo root and subworkspace configs
              cat > validate-monorepo-root.ncl << NICKEL
              let { WorkspaceConfig, .. } = import "${./contracts/workspace.ncl}" in
              (import "${./examples/monorepo/workspace.ncl}") | WorkspaceConfig
              NICKEL
              nickel export validate-monorepo-root.ncl > /dev/null
              echo "  -> examples/monorepo (root) OK"

              cat > validate-monorepo-lib-a.ncl << NICKEL
              let { WorkspaceConfig, .. } = import "${./contracts/workspace.ncl}" in
              (import "${./examples/monorepo/lib-a/workspace.ncl}") | WorkspaceConfig
              NICKEL
              nickel export validate-monorepo-lib-a.ncl > /dev/null
              echo "  -> examples/monorepo/lib-a OK"

              cat > validate-monorepo-app-b.ncl << NICKEL
              let { WorkspaceConfig, .. } = import "${./contracts/workspace.ncl}" in
              (import "${./examples/monorepo/app-b/workspace.ncl}") | WorkspaceConfig
              NICKEL
              nickel export validate-monorepo-app-b.ncl > /dev/null
              echo "  -> examples/monorepo/app-b OK"

              # v0.3: Validate submodule example configs
              cat > validate-submodule-root.ncl << NICKEL
              let { WorkspaceConfig, .. } = import "${./contracts/workspace.ncl}" in
              (import "${./examples/submodule/workspace.ncl}") | WorkspaceConfig
              NICKEL
              nickel export validate-submodule-root.ncl > /dev/null
              echo "  -> examples/submodule (root) OK"

              cat > validate-submodule-external.ncl << NICKEL
              let { WorkspaceConfig, .. } = import "${./contracts/workspace.ncl}" in
              (import "${./examples/submodule/external-tool/workspace.ncl}") | WorkspaceConfig
              NICKEL
              nickel export validate-submodule-external.ncl > /dev/null
              echo "  -> examples/submodule/external-tool OK"

              echo "==> Running error snapshot tests (expecting failures)..."
              for errtest in ${./tests/errors}/*.ncl; do
                name="$(basename $errtest)"
                if nickel export "$errtest" > /dev/null 2>&1; then
                  echo "  FAIL: $name should have failed but succeeded"
                  exit 1
                else
                  echo "  -> $name correctly rejected"
                fi
              done

              echo "All checks passed."
              touch $out
            '';

          # v0.3: Integration tests for namespacing and discovery (Nix-side)
          namespacing =
            pkgs.runCommand "nix-workspace-namespacing-tests"
            {
              nativeBuildInputs = [pkgs.nix];
            }
            ''
              echo "==> Running namespacing integration tests..."
              export NIX_PATH="nixpkgs=${pkgs.path}"
              nix eval --extra-experimental-features nix-command \
                --file ${./tests/integration/namespacing.nix}
              echo "==> Running discovery integration tests..."
              nix eval --extra-experimental-features nix-command \
                --file ${./tests/integration/discovery.nix}
              echo "All integration tests passed."
              touch $out
            '';

          # v0.4: Integration tests for plugin system (Nix-side)
          plugins =
            pkgs.runCommand "nix-workspace-plugin-tests"
            {
              nativeBuildInputs = [pkgs.nix];
            }
            ''
              echo "==> Running plugin integration tests..."
              export NIX_PATH="nixpkgs=${pkgs.path}"
              nix eval --extra-experimental-features nix-command \
                --file ${./tests/integration/plugins.nix}
              echo "All plugin integration tests passed."
              touch $out
            '';
        };
      }
    );
}
