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
    # Rust
    openssl
    # Mojo
    mojo
    llvmPackages_latest.llvm
    llvmPackages_latest.lld
    # Deno
    deno
    # Media
    cavif-rs
    presenterm
    (writeShellScriptBin "weasyprint" ''
      exec ${python313Packages.weasyprint}/bin/weasyprint "$@"
    '')
    # Jupyter
    (python313.withPackages (pp:
      with pp; [
        pip
        notebook
        jupyter-console
        deno-jupyter-kernel
        mojo-jupyter-kernel
        rust-jupyter-kernel
      ]))
    sidecar
    # DevOps
    scaleway-cli
  ];
}
