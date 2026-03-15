# GNU patch → patch-rs
#
# GNU patch applies diff/patch files to source trees. It is used
# extensively in mkDerivation's patchPhase to apply nixpkgs patches.
#
# A Rust replacement must support:
#   - Unified diff format (the dominant format in nixpkgs)
#   - Context diff format (legacy but still encountered)
#   - -p (strip prefix) flag — critical for nixpkgs patch application
#   - --dry-run for pre-flight validation
#   - Fuzz matching (applying patches to slightly changed files)
#   - Reverse patching (-R)
#   - Batch/silent mode (-s, --quiet)
#
# Planned as a repo-root subproject: ../patch-rs
{
  pkgs,
  mkComponent,
  status,
  source,
  ...
}:
mkComponent {
  name = "patch";
  original = pkgs.gnupatch;
  replacement = null;
  status = status.planned;
  source = source.repo;
  phase = 4;
  description = "Apply diff files to source trees";
  notes = "Future repo-root subproject: patch-rs";
}
