# Run `direnv reload` to apply changes to the repository.
{pkgs, ...}: {
  git-hooks = {
    package = pkgs.prek;
    hooks = {
      denolint.enable = false;
      flake-checker.enable = true;
      biome.enable = true;
      alejandra.enable = true;
      deadnix.enable = true;
      ripsecrets.enable = true;
      statix.enable = true;
      taplo.enable = true;
      typos.enable = true;
      nickel-format = {
        enable = true;
        name = "nickel-format";
        entry = "${pkgs.pkgsUnstable.nickel}/bin/nickel format";
        files = "\\.ncl$";
        pass_filenames = true;
      };
      mojo-format = {
        enable = true;
        name = "mojo-format";
        entry = "${pkgs.mojo}/bin/mojo format";
        files = "\\.mojo$";
        pass_filenames = true;
      };
      lychee = let
        lychee = pkgs.writeShellScriptBin "lychee" ''
          token=$(${pkgs.gh}/bin/gh auth token 2>/dev/null || true)
          if [ -n "$token" ]; then
            ${pkgs.lychee}/bin/lychee --github-token "$token" "$@"
          else
            ${pkgs.lychee}/bin/lychee "$@"
          fi
        '';
      in {
        enable = true;
        package = lychee;
        entry = "${lychee}/bin/lychee";
      };
      rustfmt = {
        enable = true;
        entry = "${pkgs.writeShellScript "rustfmt-multi-project" ''
          # Determine which Cargo projects contain changed .rs files.
          # Arguments are the changed .rs file paths passed by pre-commit.
          changed_files=("$@")

          if [ ''${#changed_files[@]} -eq 0 ]; then
            exit 0
          fi

          # Find the nearest Cargo.toml for each changed file and collect unique project roots.
          declare -A project_roots
          for f in "''${changed_files[@]}"; do
            dir=$(dirname "$f")
            while [ "$dir" != "." ] && [ "$dir" != "/" ]; do
              if [ -f "$dir/Cargo.toml" ]; then
                project_roots["$dir"]=1
                break
              fi
              dir=$(dirname "$dir")
            done
            # Check current directory too
            if [ -f "Cargo.toml" ] && [ "$dir" = "." ]; then
              project_roots["."]=1
            fi
          done

          if [ ''${#project_roots[@]} -eq 0 ]; then
            exit 0
          fi

          # For each project root, walk up to find the workspace root (if any).
          declare -A fmt_targets
          for root in "''${!project_roots[@]}"; do
            ws_root=""
            check_dir="$root"
            while [ "$check_dir" != "." ] && [ "$check_dir" != "/" ]; do
              if [ -f "$check_dir/Cargo.toml" ] && ${pkgs.gnugrep}/bin/grep -q '^\[workspace\]' "$check_dir/Cargo.toml"; then
                ws_root="$check_dir"
              fi
              check_dir=$(dirname "$check_dir")
            done
            # Also check the repo root
            if [ -f "Cargo.toml" ] && ${pkgs.gnugrep}/bin/grep -q '^\[workspace\]' "Cargo.toml"; then
              ws_root="."
            fi

            if [ -n "$ws_root" ]; then
              # Use the workspace root; cargo fmt handles all members
              fmt_targets["$ws_root"]=1
            else
              # Standalone package
              fmt_targets["$root"]=1
            fi
          done

          pids=()
          for target in "''${!fmt_targets[@]}"; do
            manifest="$target/Cargo.toml"
            if ${pkgs.gnugrep}/bin/grep -q '^\[workspace\]' "$manifest"; then
              # Workspace root: use --all to format all members
              echo "Running cargo fmt --all for workspace $manifest"
              ${pkgs.cargo}/bin/cargo fmt --manifest-path "$manifest" --all &
              pids+=($!)
            else
              echo "Running cargo fmt for $manifest"
              ${pkgs.cargo}/bin/cargo fmt --manifest-path "$manifest" &
              pids+=($!)
            fi
          done
          exit_code=0
          for pid in "''${pids[@]}"; do
            if ! wait "$pid"; then
              exit_code=1
            fi
          done
          exit $exit_code
        ''}";
        pass_filenames = true;
      };
      clippy = {
        enable = true;
        entry = "${pkgs.writeShellScript "clippy-multi-project" ''
          # Determine which Cargo projects contain changed .rs files.
          # Arguments are the changed .rs file paths passed by pre-commit.
          changed_files=("$@")

          if [ ''${#changed_files[@]} -eq 0 ]; then
            exit 0
          fi

          # Find the nearest Cargo.toml for each changed file and collect unique project roots.
          declare -A project_roots
          for f in "''${changed_files[@]}"; do
            dir=$(dirname "$f")
            while [ "$dir" != "." ] && [ "$dir" != "/" ]; do
              if [ -f "$dir/Cargo.toml" ]; then
                project_roots["$dir"]=1
                break
              fi
              dir=$(dirname "$dir")
            done
            # Check current directory too
            if [ -f "Cargo.toml" ] && [ "$dir" = "." ]; then
              project_roots["."]=1
            fi
          done

          if [ ''${#project_roots[@]} -eq 0 ]; then
            exit 0
          fi

          # For each project root, walk up to find the workspace root (if any).
          # Separate into workspace roots and standalone packages.
          declare -A workspace_roots
          declare -A standalone_roots
          for root in "''${!project_roots[@]}"; do
            ws_root=""
            check_dir="$root"
            while [ "$check_dir" != "." ] && [ "$check_dir" != "/" ]; do
              if [ -f "$check_dir/Cargo.toml" ] && ${pkgs.gnugrep}/bin/grep -q '^\[workspace\]' "$check_dir/Cargo.toml"; then
                ws_root="$check_dir"
              fi
              check_dir=$(dirname "$check_dir")
            done
            # Also check the repo root
            if [ -f "Cargo.toml" ] && ${pkgs.gnugrep}/bin/grep -q '^\[workspace\]' "Cargo.toml"; then
              ws_root="."
            fi

            if [ -n "$ws_root" ]; then
              workspace_roots["$ws_root"]=1
            else
              standalone_roots["$root"]=1
            fi
          done

          pids=()

          # Run clippy for affected workspaces
          for ws_dir in "''${!workspace_roots[@]}"; do
            manifest="$ws_dir/Cargo.toml"
            echo "Running cargo clippy --workspace for $manifest"
            ${pkgs.cargo}/bin/cargo clippy --manifest-path "$manifest" --workspace -- -D warnings &
            pids+=($!)
          done

          # Run clippy for standalone packages (not part of a workspace)
          for pkg_dir in "''${!standalone_roots[@]}"; do
            manifest="$pkg_dir/Cargo.toml"
            if ! ${pkgs.gnugrep}/bin/grep -q '^\[package\]' "$manifest"; then
              continue
            fi
            echo "Running cargo clippy for $manifest"
            ${pkgs.cargo}/bin/cargo clippy --manifest-path "$manifest" -- -D warnings &
            pids+=($!)
          done

          exit_code=0
          for pid in "''${pids[@]}"; do
            if ! wait "$pid"; then
              exit_code=1
            fi
          done
          exit $exit_code
        ''}";
        pass_filenames = true;
      };
      rumdl.enable = true;
      mktoc = {
        enable = false;
        package = pkgs.mktoc;
        name = "pre-commit-mktoc";
        entry = "${pkgs.mktoc}/bin/mktoc";
        files = "README\\.md$";
      };
      nil = {
        enable = true;
        entry = "${pkgs.writeShellScript "precommit-nil" ''
          errors=false
          echo Checking: $@
          for file in $(echo "$@"); do
            ${pkgs.nil}/bin/nil diagnostics --deny-warnings "$file"
            exit_code=$?

            if [[ $exit_code -ne 0 ]]; then
              echo \"$file\" failed with exit code: $exit_code
              errors=true
            fi
          done
          if [[ $errors == true ]]; then
            exit 1
          fi
        ''}";
      };
      commitlint-rs = {
        enable = true;
        package = pkgs.commitlint-rs;
        name = "prepare-commit-msg-commitlint-rs";
        entry = "${pkgs.commitlint-rs}/bin/commitlint --config ${./configs/commitlintrc.yml} --edit";
        stages = ["prepare-commit-msg"];
      };
    };
  };
}
