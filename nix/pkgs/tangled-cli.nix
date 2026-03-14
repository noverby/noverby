{
  lib,
  rustPlatform,
  fetchgit,
  pkg-config,
  openssl,
  dbus,
}:
rustPlatform.buildRustPackage {
  pname = "tangled-cli";
  version = "unstable-2026-01-05";

  src = fetchgit {
    url = "https://tangled.org/vitorpy.com/tangled-cli";
    rev = "b9d979e10a5f418614ce452774536c653fd97118";
    hash = "sha256-hzgkqeniD5Us05ifD5qNmgQPGCLg4IVst1pRuf0COsc=";
  };

  cargoHash = "sha256-giqpVvnJqj1G1heidfmGKIoiJ6DSBACWoiQyIvD0id0=";

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
    openssl
    dbus
  ];

  cargoBuildFlags = ["-p" "tangled-cli"];

  # Integration tests require network access and a running server
  doCheck = false;

  meta = {
    description = "Rust CLI for Tangled, a decentralized git collaboration platform built on the AT Protocol";
    homepage = "https://tangled.org/vitorpy.com/tangled-cli";
    license = with lib.licenses; [mit asl20];
    maintainers = with lib.maintainers; [noverby];
    mainProgram = "tangled-cli";
  };
}
