# gawk → awk-rs (Rust rewrite)
#
# GNU Awk is used in stdenv by configure scripts, makefiles, and
# various build system hooks. A replacement must support POSIX awk
# semantics plus the GNU extensions commonly relied upon (gensub,
# nextfile, BEGINFILE/ENDFILE, etc.).
#
# Candidate rewrites:
#   - awk-rs (future repo-root subproject)
#   - frawk (https://github.com/ezrosent/frawk) — fast but incomplete GNU compat
#
# For now this component is declared as planned with no replacement.
{
  pkgs,
  mkComponent,
  status,
  source,
  ...
}:
mkComponent {
  name = "awk";
  original = pkgs.gawk;
  replacement = null;
  status = status.planned;
  source = source.repo;
  phase = 2;
  description = "Pattern scanning and text processing language";
  notes = "Needs GNU awk extension compatibility (gensub, nextfile, BEGINFILE/ENDFILE)";
}
