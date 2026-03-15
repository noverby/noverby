# diffutils — diff, cmp, sdiff, diff3
#
# GNU diffutils provides file comparison utilities used extensively in
# configure scripts, patch workflows, and the Nix build sandbox.
#
# No drop-in Rust replacement exists yet. The `similar` crate provides
# a diff algorithm library, and `delta` / `difftastic` are excellent
# diff viewers, but none are CLI-compatible with GNU diff.
#
# A future `diffutils-rs` subproject at the repo root would need to
# implement at minimum: diff, cmp (used by configure scripts and
# stdenv fixup phases).
{
  pkgs,
  mkComponent,
  status,
  source,
  ...
}:
mkComponent {
  name = "diffutils";
  original = pkgs.diffutils;
  replacement = null;
  status = status.planned;
  source = source.repo;
  phase = 2;
  description = "File comparison utilities (diff, cmp, sdiff, diff3)";
  notes = "Needs diffutils-rs repo-root subproject; `diff` and `cmp` are critical for configure scripts";
}
