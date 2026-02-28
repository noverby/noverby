# gnutar → tar-rs (Rust rewrite)
#
# GNU tar is used by stdenv to unpack source tarballs and create
# archives during the install phase.  A Rust replacement must support
# all common tar formats (ustar, pax, GNU) and the flags used by
# nixpkgs: -xf, -czf, --strip-components, --transform, etc.
#
# Potential starting points:
#   - https://crates.io/crates/tar (pure Rust tar library)
#   - A CLI wrapper around the `tar` crate with GNU-compatible flags
#
# Will be added as a repo-root subproject (e.g. ../tar-rs) and wired
# in here once available.
{
  pkgs,
  mkComponent,
  status,
  source,
  ...
}:
mkComponent {
  name = "tar";
  original = pkgs.gnutar;
  replacement = null;
  status = status.planned;
  source = source.repo;
  phase = 3;
  description = "Tape archive utility for packing/unpacking source tarballs";
  notes = "Will wrap the Rust `tar` crate with a GNU-compatible CLI";
}
