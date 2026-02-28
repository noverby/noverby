# patchelf → patchelf-rs
#
# patchelf is a utility for modifying ELF binaries — changing the
# dynamic linker, RPATH, RUNPATH, SONAME, and other ELF headers.
# It is a critical part of Nix's fixup phase: every derivation that
# produces ELF binaries has its RPATH rewritten by patchelf to point
# at exact store paths, which is how Nix achieves hermetic builds.
#
# Replacement: patchelf-rs (future repo-root subproject)
#   A Rust rewrite must support the full patchelf CLI:
#     --set-interpreter, --set-rpath, --shrink-rpath, --remove-rpath,
#     --set-soname, --add-needed, --remove-needed, --replace-needed,
#     --print-interpreter, --print-rpath, --print-soname, --print-needed,
#     --no-default-lib, --page-size, --output
#   The goblin or object crates provide ELF parsing foundations.
#
# Priority: Phase 5 — binary fixup tools
{
  pkgs,
  mkComponent,
  status,
  source,
  ...
}:
mkComponent {
  name = "patchelf";
  original = pkgs.patchelf;
  replacement = null;
  status = status.planned;
  source = source.repo;
  phase = 5;
  description = "ELF binary patching tool (interpreter, RPATH, SONAME)";
  notes = "Future repo-root subproject patchelf-rs; critical for Nix fixup phase";
}
