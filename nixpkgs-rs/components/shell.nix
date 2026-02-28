# Shell: bash → brush
#
# Brush is a Bash-compatible shell written in Rust.
# It is already packaged in nixpkgs as `pkgs.brush`.
#
# For use as a stdenv shell, brush needs a wrapper that translates
# bash's single-character flags (e.g. -eu) into brush's option syntax
# and handles signal/process-group setup. See nixos-rs/bash.nix for
# the full wrapper used in NixOS runtime replacement.
#
# For stdenv, we provide the brush package directly — the build system
# invokes the shell as `bash` via the symlink, and brush's bash
# compatibility mode handles the rest.
{
  pkgs,
  mkComponent,
  status,
  source,
  ...
}:
mkComponent {
  name = "shell";
  original = pkgs.bash;
  replacement = pkgs.brush;
  status = status.available;
  source = source.nixpkgs;
  phase = 1;
  description = "POSIX/Bash-compatible shell";
  notes = "Using brush from nixpkgs — Bash-compatible Rust shell";
}
