{pkgs, ...}: {
  home.packages = with pkgs.pkgsUnstable; [
    # General dev
    lazyjj
    glab
    granted
    mistral-vibe

    # System dev
    #lldb
    gdb
    cling # C++ repl
    evcxr # Rust repl
    lurk
    tracexec
    llvmPackages.bintools
    binwalk
    hyperfine
    inferno # Flamegraph svg generator
    flamelens # Flamegraph cli viewer
    #darling

    # Nix dev
    nix-du
    nix-diff-rs
    devenv
    nix-prefetch-git
    nix-fast-build
    nix-init
    comma
    nurl
    pkgs.nxv
  ];
}
