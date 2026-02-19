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
          pids=()
          for manifest in $(${pkgs.findutils}/bin/find . -name Cargo.toml -not -path '*/target/*'); do
            if ${pkgs.gnugrep}/bin/grep -q '^\[workspace\]' "$manifest" && ! ${pkgs.gnugrep}/bin/grep -q '^\[package\]' "$manifest"; then
              echo "Skipping workspace-only $manifest"
              continue
            fi
            echo "Running cargo fmt for $manifest"
            ${pkgs.cargo}/bin/cargo fmt --manifest-path "$manifest" &
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
        pass_filenames = false;
      };
      clippy = {
        enable = true;
        entry = "${pkgs.writeShellScript "clippy-multi-project" ''
          pids=()

          # Collect workspace root directories and run clippy for workspaces
          workspace_dirs=()
          for manifest in $(${pkgs.findutils}/bin/find . -name Cargo.toml -not -path '*/target/*'); do
            if ${pkgs.gnugrep}/bin/grep -q '^\[workspace\]' "$manifest"; then
              dir=$(dirname "$manifest")
              workspace_dirs+=("$dir")
              echo "Running cargo clippy --workspace for $manifest"
              ${pkgs.cargo}/bin/cargo clippy --manifest-path "$manifest" --workspace -- -D warnings &
              pids+=($!)
            fi
          done

          # Run clippy for standalone packages (not part of a workspace)
          for manifest in $(${pkgs.findutils}/bin/find . -name Cargo.toml -not -path '*/target/*'); do
            if ${pkgs.gnugrep}/bin/grep -q '^\[workspace\]' "$manifest"; then
              continue
            fi
            if ! ${pkgs.gnugrep}/bin/grep -q '^\[package\]' "$manifest"; then
              continue
            fi
            dir=$(dirname "$manifest")
            is_member=false
            for ws_dir in "''${workspace_dirs[@]}"; do
              case "$dir" in
                "$ws_dir"/*) is_member=true; break ;;
              esac
            done
            if [ "$is_member" = false ]; then
              echo "Running cargo clippy for $manifest"
              ${pkgs.cargo}/bin/cargo clippy --manifest-path "$manifest" -- -D warnings &
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
        pass_filenames = false;
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
