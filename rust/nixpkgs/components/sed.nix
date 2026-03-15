# gnused → sed-rs
#
# GNU sed is used pervasively in stdenv for text substitution — configure
# scripts, setup-hooks, substituteInPlace, and many build systems depend
# on it. A replacement must support the full POSIX sed spec plus common
# GNU extensions (-i in-place, -E extended regex, -z NUL-delimited).
#
# Existing Rust projects:
#   - sd (https://github.com/chmln/sd) — simpler interface, NOT sed-compatible
#   - none with full GNU sed flag compatibility
#
# Plan: create ../sed-rs as a drop-in GNU sed replacement.
{
  pkgs,
  mkComponent,
  status,
  source,
  ...
}:
mkComponent {
  name = "gnused";
  original = pkgs.gnused;
  replacement = null;
  status = status.planned;
  source = source.repo;
  phase = 2;
  description = "Stream editor for filtering and transforming text";
  notes = "Needs GNU sed flag compatibility (-i, -E, -z, address ranges, branch/label commands)";
}
