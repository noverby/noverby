{
  lib,
  rustPlatform,
  fetchFromGitHub,
  pkg-config,
  libxkbcommon,
  vulkan-loader,
  alsa-lib,
  wayland,
}:
rustPlatform.buildRustPackage rec {
  pname = "vibe";
  version = "2.2.0";

  src = fetchFromGitHub {
    owner = "TornaxO7";
    repo = "vibe";
    rev = "vibe-v${version}";
    hash = "sha256-uUItHJnPZ6RquLC4GPS7jtF7BTomMX6yf0Ftr3Y4AiE=";
  };

  cargoHash = "sha256-Xn+sH5MpjX12X4zeRYfMPbxpZQR4tnVOXl916mVzBVM=";

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
    libxkbcommon
    vulkan-loader
    alsa-lib
    wayland
  ];

  doCheck = false;

  meta = {
    description = "A desktop audio visualizer and shader player for your wayland wallpaper";
    homepage = "https://github.com/TornaxO7/vibe";
    license = lib.licenses.gpl2Only;
    maintainers = with lib.maintainers; [];
    mainProgram = "vibe";
  };
}
