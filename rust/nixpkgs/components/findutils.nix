# findutils — find, xargs
#
# GNU findutils provides `find` and `xargs`, both heavily used in
# stdenv's setup.sh and in configure scripts.
#
# Existing Rust alternatives like `fd` are excellent interactive tools
# but are NOT flag-compatible with GNU find (different CLI, different
# output format, different -exec semantics).  A drop-in replacement
# must support the full POSIX + GNU extension surface:
#   - find: -name, -path, -type, -exec, -print0, -newer, -perm, etc.
#   - xargs: -0, -I{}, -P, -n, -L, etc.
#
# Strategy: create a `findutils-rs` subproject at the repo root that
# provides both `find` and `xargs` as GNU-compatible binaries.
# Alternatively, contribute GNU-compat modes to an existing project.
#
# Existing Rust projects to evaluate:
#   - fd (sharkdp/fd) — fast find alternative, NOT flag-compatible
#   - rust-parallel — xargs-like, not flag-compatible
#
# Until a drop-in is available, the original GNU findutils is used.
{
  pkgs,
  mkComponent,
  status,
  source,
  ...
}:
mkComponent {
  name = "findutils";
  original = pkgs.findutils;
  replacement = null;
  status = status.planned;
  source = source.repo;
  phase = 2;
  description = "File search and command execution (find, xargs)";
  notes = "Needs GNU-compatible rewrite — fd is not a drop-in";
}
