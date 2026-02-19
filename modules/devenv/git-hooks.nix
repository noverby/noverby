# Run `direnv reload` to apply changes to the repository.
{pkgs, ...}: {
  git-hooks = {
    package = pkgs.prek;
    hooks = {
      denolint.enable = false;
      biome.enable = true;
      alejandra.enable = true;
      deadnix.enable = true;
      ripsecrets.enable = true;
      statix.enable = true;
      taplo.enable = true;
      typos.enable = true;
      rustfmt = {
        enable = true;
        entry = "${pkgs.writeShellScript "rustfmt-multi-project" ''
          for manifest in $(${pkgs.findutils}/bin/find . -name Cargo.toml -not -path '*/target/*'); do
            if ${pkgs.gnugrep}/bin/grep -q '^\[workspace\]' "$manifest" && ! ${pkgs.gnugrep}/bin/grep -q '^\[package\]' "$manifest"; then
              echo "Skipping workspace-only $manifest"
              continue
            fi
            echo "Running cargo fmt for $manifest"
            ${pkgs.cargo}/bin/cargo fmt --manifest-path "$manifest"
          done
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
