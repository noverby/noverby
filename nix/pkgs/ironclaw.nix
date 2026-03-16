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
  ironclaw-matrix-channel,
  ironclaw-bluesky-channel,
  ironclaw-signal-channel,
  ironclaw-searxng-tool,
}: let
  version = "0.18.0";

  src = fetchFromGitHub {
    owner = "nearai";
    repo = "ironclaw";
    rev = "v${version}";
    hash = "sha256-XFvrhu8MBQhTpK3lQxVLJ83C10Rh18cWrzkiWeOde1k=";
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
    hash = "sha256-IDT/7DLItLRs2biE04qyb7OkizClObZs3+R6Xjc2LbQ=";
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

    cargoHash = "sha256-0Fdj9+UVKrNi3X77MOxA5Az87Rw8wKnTp2W42eTD4TI=";

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

      # Replace build.rs with one that only embeds the registry catalog
      # (skipping the WASM channel build since we pre-build it above)
      cat > build.rs << 'BUILDRS'
      use std::env;
      use std::fs;
      use std::path::{Path, PathBuf};

      fn main() {
          let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
          let registry_dir = root.join("registry");
          let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
          let out_path = out_dir.join("embedded_catalog.json");

          if !registry_dir.is_dir() {
              fs::write(&out_path, r#"{"tools":[],"channels":[],"bundles":{"bundles":{}}}"#).unwrap();
              return;
          }

          let mut tools = Vec::new();
          let mut channels = Vec::new();

          let tools_dir = registry_dir.join("tools");
          if tools_dir.is_dir() { collect_json_files(&tools_dir, &mut tools); }

          let channels_dir = registry_dir.join("channels");
          if channels_dir.is_dir() { collect_json_files(&channels_dir, &mut channels); }

          let bundles_path = registry_dir.join("_bundles.json");
          let bundles_raw = if bundles_path.is_file() {
              fs::read_to_string(&bundles_path).unwrap_or_else(|_| r#"{"bundles":{}}"#.to_string())
          } else {
              r#"{"bundles":{}}"#.to_string()
          };

          let catalog = format!(
              r#"{{"tools":[{}],"channels":[{}],"bundles":{}}}"#,
              tools.join(","), channels.join(","), bundles_raw,
          );
          fs::write(&out_path, catalog).unwrap();
      }

      fn collect_json_files(dir: &Path, out: &mut Vec<String>) {
          let mut entries: Vec<_> = fs::read_dir(dir).unwrap()
              .filter_map(|e| e.ok())
              .filter(|e| e.path().is_file() && e.path().extension().and_then(|x| x.to_str()) == Some("json"))
              .collect();
          entries.sort_by_key(|e| e.file_name());
          for entry in entries {
              if let Ok(content) = fs::read_to_string(entry.path()) {
                  out.push(content);
              }
          }
      }
      BUILDRS
    '';

    postInstall = ''
      # Install channel artifacts alongside the binary so they are
      # discoverable at runtime via IRONCLAW_CHANNELS_SRC
      mkdir -p $out/share/ironclaw/channels-src/telegram/target/wasm32-wasip2/release
      cp ${telegramChannelWasm}/telegram.wasm \
         $out/share/ironclaw/channels-src/telegram/target/wasm32-wasip2/release/telegram_channel.wasm
      cp ${telegramChannelWasm}/telegram.capabilities.json \
         $out/share/ironclaw/channels-src/telegram/telegram.capabilities.json

      # Matrix channel (built separately)
      mkdir -p $out/share/ironclaw/channels-src/matrix/target/wasm32-wasip2/release
      cp ${ironclaw-matrix-channel}/matrix.wasm \
         $out/share/ironclaw/channels-src/matrix/target/wasm32-wasip2/release/matrix_channel.wasm
      cp ${ironclaw-matrix-channel}/matrix.capabilities.json \
         $out/share/ironclaw/channels-src/matrix/matrix.capabilities.json

      # Bluesky channel (built separately)
      mkdir -p $out/share/ironclaw/channels-src/bluesky/target/wasm32-wasip2/release
      cp ${ironclaw-bluesky-channel}/bluesky.wasm \
         $out/share/ironclaw/channels-src/bluesky/target/wasm32-wasip2/release/bluesky_channel.wasm
      cp ${ironclaw-bluesky-channel}/bluesky.capabilities.json \
         $out/share/ironclaw/channels-src/bluesky/bluesky.capabilities.json

      # Signal channel (built separately)
      mkdir -p $out/share/ironclaw/channels-src/signal/target/wasm32-wasip2/release
      cp ${ironclaw-signal-channel}/signal.wasm \
         $out/share/ironclaw/channels-src/signal/target/wasm32-wasip2/release/signal_channel.wasm
      cp ${ironclaw-signal-channel}/signal.capabilities.json \
         $out/share/ironclaw/channels-src/signal/signal.capabilities.json

      # SearXNG tool (built separately)
      mkdir -p $out/share/ironclaw/tools-src/searxng
      cp ${ironclaw-searxng-tool}/searxng.wasm \
         $out/share/ironclaw/tools-src/searxng/searxng.wasm
      cp ${ironclaw-searxng-tool}/searxng-tool.capabilities.json \
         $out/share/ironclaw/tools-src/searxng/searxng-tool.capabilities.json
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
