{
  lib,
  rust-bin,
  makeRustPlatform,
  fetchFromGitHub,
  pkg-config,
  openssl,
  cacert,
}: let
  rustPlatform = makeRustPlatform {
    cargo = rust-bin.stable.latest.default;
    rustc = rust-bin.stable.latest.default;
  };
in
  rustPlatform.buildRustPackage {
    pname = "ironclaw";
    version = "0.9.0";

    src = fetchFromGitHub {
      owner = "nearai";
      repo = "ironclaw";
      rev = "v0.9.0";
      hash = "sha256-bpF/9rsa/5kzSNMW0YnbfqVKSnKL1Q+yBNUhEnrzn7g=";
    };

    cargoHash = "sha256-0I0SgSsS9GatE9JMfInHhlUu9VVEoSqf2KwAo5atz+M=";

    nativeBuildInputs = [
      pkg-config
      rustPlatform.bindgenHook
    ];

    buildInputs = [
      openssl
      cacert
    ];

    # The build.rs attempts to compile WASM channel plugins (telegram, etc.)
    # which requires the wasm32-wasip2 target and wasm-tools — neither
    # available inside the Nix sandbox.  Removing the source directories
    # makes the build.rs bail out early (`if !channel_dir.is_dir()`).
    preBuild = ''
      rm -rf channels-src tools-src
    '';

    # Some integration tests require a running PostgreSQL instance
    doCheck = false;

    meta = {
      description = "IronClaw – secure personal AI assistant (OpenClaw-inspired, written in Rust)";
      homepage = "https://github.com/nearai/ironclaw";
      license = with lib.licenses; [asl20 mit];
      maintainers = with lib.maintainers; [noverby];
      platforms = lib.platforms.linux;
      mainProgram = "ironclaw";
    };
  }
