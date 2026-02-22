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
              nickel typecheck ${./contracts/workspace.ncl}

              echo "==> Running unit tests..."
              for test in ${./tests/unit}/*.ncl; do
                echo "  -> $(basename $test)"
                nickel eval "$test"
              done

              echo "==> Validating example workspaces..."
              # Build a wrapper that applies the WorkspaceConfig contract to the example
              cat > validate-minimal.ncl << NICKEL
              let { WorkspaceConfig, .. } = import "${./contracts/workspace.ncl}" in
              (import "${./examples/minimal/workspace.ncl}") | WorkspaceConfig
              NICKEL
              nickel export validate-minimal.ncl > /dev/null
              echo "  -> examples/minimal OK"

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
        };
      }
    );
}
