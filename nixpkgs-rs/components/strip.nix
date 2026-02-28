# strip — binutils strip → strip-rs
#
# GNU strip removes symbols and debug info from ELF binaries.
# It is used by stdenv's fixup phase (stripDirs) to reduce closure size.
#
# A Rust replacement would need to:
#   - Parse and rewrite ELF binaries (strip sections/symbols)
#   - Support the same flags used by nixpkgs: --strip-all, --strip-debug,
#     --strip-unneeded, -p (preserve timestamps), -R <section>
#   - Handle static libraries (.a archives containing .o files)
#   - Preserve or update .gnu_debuglink sections
#
# Existing Rust ecosystem:
#   - `object` crate provides ELF reading/writing primitives
#   - `goblin` crate provides ELF/Mach-O/PE parsing
#   - No mature GNU strip drop-in exists yet
#
# Future subproject: ../strip-rs
{
  pkgs,
  mkComponent,
  status,
  source,
  ...
}:
mkComponent {
  name = "strip";
  original = pkgs.binutils-unwrapped;
  replacement = null;
  status = status.planned;
  source = source.repo;
  phase = 5;
  description = "ELF symbol stripping tool (from binutils)";
  notes = "Future repo-root subproject strip-rs; depends on `object` or `goblin` crate for ELF rewriting";
}
