{
  lib,
  rustPlatform,
  fetchFromGitHub,
  pkg-config,
  stdenv,
  darwin,
  nasm,
}:
rustPlatform.buildRustPackage rec {
  pname = "cavif-rs";
  version = "1.5.6";

  src = fetchFromGitHub {
    owner = "kornelski";
    repo = "cavif-rs";
    rev = "v${version}";
    hash = "sha256-QBYTbPfRtdM+HlHLq//2kfaLPjFAg9fefFSMNksclxM=";
  };

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  postPatch = ''
    ln -s ${./Cargo.lock} Cargo.lock
  '';

  nativeBuildInputs = [
    pkg-config
    nasm
  ];

  buildInputs = lib.optionals stdenv.isDarwin [
    darwin.apple_sdk.frameworks.CoreGraphics
  ];

  meta = {
    description = "AVIF image creator in pure Rust";
    homepage = "https://github.com/kornelski/cavif-rs";
    license = lib.licenses.bsd3;
    maintainers = with lib.maintainers; [noverby];
    mainProgram = "cavif-rs";
  };
}
