{
  pkgs,
  inputs,
  ...
}: {
  imports = with inputs.self.devenvModules; [
    devenv-root
    git-hooks
    configs
  ];

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
    # Nix
    nixd
    nil
    alejandra
    (writeShellScriptBin "ragenix" ''
      exec ${ragenix}/bin/ragenix -i ~/.age/id_fido2 "$@"
    '')
  ];
}
