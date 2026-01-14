{
  pkgs,
  inputs,
}:
inputs.devenv.lib.mkShell
{
  inherit inputs pkgs;

  modules = [
    {
      git-hooks = {
        package = pkgs.prek;
        hooks = {
          denolint.enable = true;
          biome.enable = true;
          alejandra.enable = true;
          statix.enable = true;
          typos.enable = true;
          rustfmt.enable = true;
          rumdl.enable = true;
          nil = {
            enable = true;
            entry = builtins.toString (pkgs.writeShellScript "precommit-nil" ''
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
            '');
          };
          commitlint-rs = {
            enable = true;
            package = pkgs.commitlint-rs;
            name = "prepare-commit-msg-commitlint-rs";
            entry = "${pkgs.commitlint-rs}/bin/commitlint --edit";
            stages = ["prepare-commit-msg"];
          };
        };
      };

      languages = {
        rust = {
          enable = true;
        };
      };

      packages = with pkgs; [
        # IDE
        harper
        # Common
        just
        rumdl
        # Nix
        nixd
        nil
        alejandra
        (writeShellScriptBin "ragenix" ''
          exec ${ragenix}/bin/ragenix -i ~/.age/id_fido2 "$@"
        '')
        # Rust
        openssl
        # Mojo
        mojo
        python3
        llvmPackages_latest.llvm
        llvmPackages_latest.lld
        # Deno
        deno
        # Media
        cavif-rs
        presenterm
        python313Packages.weasyprint
        # DevOps
        scaleway-cli
      ];
    }
  ];
}
