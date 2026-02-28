# bzip2 — bzip2 compression
#
# Original: bzip2 (Julian Seward's C implementation)
# Replacement: bzip2-rs (future repo-root subproject)
#
# bzip2 is used by stdenv to decompress .bz2 source tarballs.
# A drop-in must provide: bzip2, bunzip2, bzcat, bzip2recover.
#
# Candidates:
#   - bzip2-rs (repo subproject, planned)
#   - https://github.com/trifectatechfoundation/bzip2-rs (Rust port of libbzip2)
{
  pkgs,
  mkComponent,
  status,
  source,
  ...
}:
mkComponent {
  name = "bzip2";
  original = pkgs.bzip2;
  replacement = null;
  status = status.planned;
  source = source.repo;
  phase = 3;
  description = "bzip2 compression/decompression";
  notes = "Trifecta Tech Foundation has a Rust libbzip2 port; CLI wrapper needed";
}
