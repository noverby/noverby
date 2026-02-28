# grep — gnugrep → grep-rs
#
# GNU grep is used extensively in configure scripts, Makefiles, and
# nixpkgs build hooks.  A replacement must be fully GNU-flag-compatible
# (ripgrep is NOT — it lacks -P, -w semantics differ, etc.).
#
# A future grep-rs subproject at the repo root would provide a
# flag-compatible `grep`, `egrep`, `fgrep` CLI.
{
  pkgs,
  mkComponent,
  status,
  source,
}: let
  # ripgrep is fast but not GNU-flag-compatible, so it cannot serve
  # as a drop-in.  We leave replacement = null until a compatible
  # Rust rewrite exists.
  #
  # When grep-rs is created at the repo root:
  #   replacement = pkgs.grep-rs or null;
  replacement = null;
in
  mkComponent {
    name = "grep";
    original = pkgs.gnugrep;
    inherit replacement;
    status = status.planned;
    source = source.repo;
    phase = 2;
    description = "Pattern matching (grep, egrep, fgrep)";
    notes = "Needs GNU-flag-compatible rewrite; ripgrep is not a drop-in";
  }
