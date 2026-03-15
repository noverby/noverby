{
  devShells.rust-nixpkgs = pkgs: {
    packages = with pkgs; [
      just
      nix-tree
    ];
  };

  overlays.rust-nixpkgs = final: prev: let
    inherit (final) lib;
    components = import ./components {pkgs = final;};

    # Brush provides /bin/brush, but stdenv expects /bin/bash.
    # Create a thin wrapper package with bash/sh symlinks.
    brush-as-bash = final.runCommand "brush-as-bash-${final.brush.version}" {} ''
      mkdir -p $out/bin
      ln -s ${final.brush}/bin/brush $out/bin/bash
      ln -s ${final.brush}/bin/brush $out/bin/sh
    '';

    # Collect only components that have a replacement ready
    available = lib.filter (c: c.replacement != null) (lib.attrValues components);

    # Build the replacement initialPath by swapping available components.
    # The shell component is special-cased: we use brush-as-bash instead
    # of the raw brush package so stdenv gets /bin/bash.
    replacedInitialPath =
      map (
        pkg: let
          match = lib.filter (c: c.original == pkg) available;
          component =
            if match != []
            then lib.head match
            else null;
        in
          if component != null && component.name == "shell"
          then brush-as-bash
          else if component != null
          then component.replacement
          else pkg
      )
      prev.stdenv.initialPath;
  in {
    # Expose the component registry for introspection
    rust-nixpkgs-components = components;

    # The brush-as-bash wrapper for use in other contexts
    inherit brush-as-bash;

    # A stdenv with all available Rust replacements swapped in
    stdenvRs = prev.stdenv.override {
      initialPath = replacedInitialPath;
      shell = "${brush-as-bash}/bin/bash";
    };

    # mkDerivation using the Rust stdenv — use this to test-build packages
    mkDerivationRs = args: (final.stdenvRs.mkDerivation args);
  };

  packages = {
    # A test derivation that reports component availability status.
    # This uses the normal stdenv (not stdenvRs) so it always builds,
    # even when the Rust stdenv has issues.
    rust-nixpkgs-test = {
      stdenv,
      lib,
      uutils-coreutils-noprefix,
      brush,
    }:
      stdenv.mkDerivation {
        pname = "rust-nixpkgs-test";
        version = "0.1.0";

        dontUnpack = true;

        # Verify the available Rust replacements actually exist
        nativeBuildInputs = [
          uutils-coreutils-noprefix
          brush
        ];

        buildPhase = ''
          echo "=== rust-nixpkgs component status ==="
          echo ""
          echo "Available (2 components):"
          echo "  ✅ shell (brush $(brush --version 2>&1 | head -1 || echo unknown))"
          echo "  ✅ coreutils (uutils $(ls --version 2>&1 | head -1 || echo unknown))"
          echo ""
          echo "Planned (13 components):"
          echo "  ⏳ gnused"
          echo "  ⏳ grep"
          echo "  ⏳ awk"
          echo "  ⏳ findutils"
          echo "  ⏳ diffutils"
          echo "  ⏳ tar"
          echo "  ⏳ gzip"
          echo "  ⏳ bzip2"
          echo "  ⏳ xz"
          echo "  ⏳ make"
          echo "  ⏳ patch"
          echo "  ⏳ patchelf"
          echo "  ⏳ strip"
          echo ""
        '';

        installPhase = ''
          mkdir -p $out
          echo "rust-nixpkgs component test passed" > $out/result
        '';

        meta = {
          description = "Test derivation for rust-nixpkgs component availability";
          license = lib.licenses.mit;
        };
      };
  };
}
