{
  lib,
  rustPlatform,
  fetchFromGitHub,
  pkg-config,
  libxkbcommon,
  vulkan-loader,
  libGL,
  vulkan-validation-layers,
  vulkan-tools,
  alsa-lib,
  wayland,
  mesa,
  makeWrapper,
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
    makeWrapper
  ];

  buildInputs = [
    alsa-lib

    wayland

    libGL
    libxkbcommon

    vulkan-loader
    vulkan-validation-layers
    vulkan-tools
  ];

  doCheck = false;

  postInstall = ''
    wrapProgram $out/bin/$pname --prefix LD_LIBRARY_PATH : ${builtins.toString (lib.makeLibraryPath [
      # Without wayland in library path, this warning is raised:
      # "No windowing system present. Using surfaceless platform"
      wayland
      # Without vulkan-loader present, wgpu won't find any adapter
      vulkan-loader
      mesa
    ])}
  '';

  LD_LIBRARY_PATH = "$LD_LIBRARY_PATH:${lib.makeLibraryPath buildInputs}";

  meta = {
    description = "A desktop audio visualizer and shader player for your wayland wallpaper";
    homepage = "https://github.com/TornaxO7/vibe";
    license = lib.licenses.gpl2Only;
    maintainers = with lib.maintainers; [];
    mainProgram = "vibe";
  };
}
