{
  lib,
  rustPlatform,
  fetchFromGitHub,
  pkg-config,
  libxkbcommon,
  vulkan-loader,
  stdenv,
  wayland,
}:
rustPlatform.buildRustPackage {
  pname = "cosmic-osk";
  version = "0-unstable-2026-01-22";

  src = fetchFromGitHub {
    owner = "pop-os";
    repo = "cosmic-osk";
    rev = "eee4d0472c815bad010492c16f4358fca9a47e5f";
    hash = "sha256-B5XYflvjykLOn59zHgWWsJY0bU2cUo0XtJTu0QveTRQ=";
  };

  cargoHash = "sha256-WhAhrediZCNVl9evwNIBKFbCM14lNzfIm0tPJ71HGD0=";

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs =
    [
      libxkbcommon
      vulkan-loader
    ]
    ++ lib.optionals stdenv.isLinux [
      wayland
    ];

  meta = {
    description = "COSMIC On-Screen Keyboard";
    homepage = "https://github.com/pop-os/cosmic-osk";
    license = lib.licenses.gpl3Only;
    maintainers = with lib.maintainers; [noverby];
    mainProgram = "cosmic-osk";
  };
}
