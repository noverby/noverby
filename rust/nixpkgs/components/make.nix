# GNU Make → make-rs
#
# GNU Make is the build system driver used by the vast majority of
# autotools-based packages.  A Rust replacement must support the full
# POSIX make spec plus common GNU extensions (pattern rules, order-only
# prerequisites, $(shell), $(eval), automatic variables, etc.).
#
# Strategy: develop a make-rs subproject at the monorepo root (../make-rs).
# Until then, the original GNU Make is used.
{
  pkgs,
  mkComponent,
  status,
  source,
}: let
  # Uncomment and point to the repo-root package when available:
  # make-rs = pkgs.callPackage ../make-rs { };
  replacement = null;
in
  mkComponent {
    name = "make";
    original = pkgs.gnumake;
    inherit replacement;
    status = status.planned;
    source = source.repo;
    phase = 4;
    description = "Build system driver (GNU Make)";
    notes = "Future: make-rs at repo root. Must support GNU make extensions beyond POSIX.";
  }
