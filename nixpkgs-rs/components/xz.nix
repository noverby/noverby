# xz — LZMA/XZ compression utility
#
# Original: xz (xz-utils / liblzma)
# Replacement: xz-rs (planned repo-root subproject)
#
# xz is used by stdenv to decompress .tar.xz source archives, which
# are the most common archive format in modern nixpkgs.  The Rust
# ecosystem has `liblzma-rs` (safe bindings) and `xz2` (streaming
# codec), but no drop-in CLI replacement for `xz`, `unxz`, `xzcat`,
# `lzma`, and `unlzma` with full GNU xz flag compatibility.
#
# A repo-root `xz-rs` project would provide:
#   - xz, unxz, xzcat, lzma, unlzma CLI tools
#   - Streaming compress/decompress matching GNU xz behavior
#   - Support for -d, -k, -f, -c, -z, -T (threads), compression levels
#   - stdin/stdout piping (used heavily by tar and nix fetchers)
#
# Phase 3 — Archive & Compression
{
  pkgs,
  mkComponent,
  status,
  source,
  ...
}:
mkComponent {
  name = "xz";
  original = pkgs.xz;
  replacement = null;
  status = status.planned;
  source = source.repo;
  phase = 3;
  description = "LZMA/XZ compression and decompression";
  notes = "Planned repo-root xz-rs project; Rust crates liblzma-rs and xz2 provide codec foundation";
}
