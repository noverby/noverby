{
  lib,
  rust-bin,
  makeRustPlatform,
  fetchFromGitHub,
  pkg-config,
  openssl,
  cacert,
  wasm-tools,
  stdenv,
}: let
  version = "0.9.0";

  src = fetchFromGitHub {
    owner = "nearai";
    repo = "ironclaw";
    rev = "v${version}";
    hash = "sha256-bpF/9rsa/5kzSNMW0YnbfqVKSnKL1Q+yBNUhEnrzn7g=";
  };

  # Rust toolchain with wasm32-wasip2 target for building WASM channels
  rustWithWasm = rust-bin.stable.latest.default.override {
    targets = ["wasm32-wasip2"];
  };

  rustPlatform = makeRustPlatform {
    cargo = rustWithWasm;
    rustc = rustWithWasm;
  };

  # Extract the telegram channel source from the ironclaw repo.
  # It lives at channels-src/telegram/ but references ../../wit/channel.wit,
  # so we preserve the directory structure relative to the repo root.
  telegramChannelSrc = stdenv.mkDerivation {
    name = "ironclaw-telegram-channel-src";
    inherit src;
    phases = ["unpackPhase" "installPhase"];
    installPhase = ''
      mkdir -p $out/channels-src/telegram $out/wit
      cp -r channels-src/telegram/* $out/channels-src/telegram/
      cp -r wit/* $out/wit/

      # Fix duplicate [workspace] key in Cargo.toml (upstream bug)
      awk '!seen[$0]++ || $0 != "[workspace]"' \
        $out/channels-src/telegram/Cargo.toml > tmp \
        && mv tmp $out/channels-src/telegram/Cargo.toml
    '';
  };

  # Vendored cargo dependencies for the telegram channel workspace
  telegramChannelDeps = rustPlatform.fetchCargoVendor {
    src = telegramChannelSrc + "/channels-src/telegram";
    hash = "sha256-wN3NfkNLmk2W4NpSCuQeoDm524gVb5FtKCbD6+XiFKE=";
  };

  # Pre-built telegram WASM channel component
  telegramChannelWasm = stdenv.mkDerivation {
    pname = "ironclaw-telegram-channel";
    inherit version;
    src = telegramChannelSrc;

    nativeBuildInputs = [rustWithWasm wasm-tools];

    buildPhase = ''
      cd channels-src/telegram

      # Set up vendored deps
      mkdir -p .cargo
      cat > .cargo/config.toml <<EOF
      [source.crates-io]
      replace-with = "vendored-sources"
      [source.vendored-sources]
      directory = "${telegramChannelDeps}"
      EOF

      cargo build --release --target wasm32-wasip2 --offline

      # Convert to WASM component and strip debug info
      wasm-tools component new \
        target/wasm32-wasip2/release/telegram_channel.wasm \
        -o telegram.wasm \
        2>/dev/null || cp target/wasm32-wasip2/release/telegram_channel.wasm telegram.wasm

      wasm-tools strip telegram.wasm -o telegram.stripped.wasm && mv telegram.stripped.wasm telegram.wasm || true
    '';

    installPhase = ''
      mkdir -p $out
      cp telegram.wasm $out/telegram.wasm
      cp telegram.capabilities.json $out/telegram.capabilities.json
    '';
  };
in
  rustPlatform.buildRustPackage {
    pname = "ironclaw";
    inherit version src;

    cargoHash = "sha256-0I0SgSsS9GatE9JMfInHhlUu9VVEoSqf2KwAo5atz+M=";

    nativeBuildInputs = [
      pkg-config
      rustPlatform.bindgenHook
      wasm-tools
    ];

    buildInputs = [
      openssl
      cacert
    ];

    preBuild = ''
      # Place the pre-built telegram WASM where bundled.rs expects it
      # (channels-src/telegram/target/wasm32-wasip2/release/telegram_channel.wasm)
      mkdir -p channels-src/telegram/target/wasm32-wasip2/release
      cp ${telegramChannelWasm}/telegram.wasm \
         channels-src/telegram/target/wasm32-wasip2/release/telegram_channel.wasm
      cp ${telegramChannelWasm}/telegram.capabilities.json \
         channels-src/telegram/telegram.capabilities.json

      # Neuter build.rs so it doesn't try to re-compile the channel
      echo 'fn main() {}' > build.rs
    '';

    postInstall = ''
      # Install channel artifacts alongside the binary so they are
      # discoverable at runtime via IRONCLAW_CHANNELS_SRC
      mkdir -p $out/share/ironclaw/channels-src/telegram/target/wasm32-wasip2/release
      cp ${telegramChannelWasm}/telegram.wasm \
         $out/share/ironclaw/channels-src/telegram/target/wasm32-wasip2/release/telegram_channel.wasm
      cp ${telegramChannelWasm}/telegram.capabilities.json \
         $out/share/ironclaw/channels-src/telegram/telegram.capabilities.json
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
