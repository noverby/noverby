# gzip → Rust replacement
#
# GNU gzip provides compression/decompression for the .gz format.
# Used heavily in stdenv for unpacking source tarballs (.tar.gz).
#
# Candidates:
#   - gzip-rs (future repo-root subproject)
#   - pigz has no Rust equivalent yet
#   - The `flate2` crate provides the compression algorithm;
#     a CLI wrapper is needed for GNU gzip flag compatibility
#
# Required commands: gzip, gunzip, zcat
# Required flags: -d (decompress), -c (stdout), -k (keep),
#   -1..-9 (level), -f (force), -n/-N (name), -r (recursive)
{
  pkgs,
  mkComponent,
  status,
  source,
  ...
}:
mkComponent {
  name = "gzip";
  original = pkgs.gzip;
  replacement = null;
  status = status.planned;
  source = source.repo;
  phase = 3;
  description = "Compression utility for .gz format";
  notes = "Future gzip-rs subproject; flate2 crate provides the algorithm";
}
