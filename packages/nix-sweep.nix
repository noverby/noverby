{
  lib,
  rustPlatform,
  fetchFromGitHub,
}:
rustPlatform.buildRustPackage rec {
  pname = "nix-sweep";
  version = "0.7.0";

  src = fetchFromGitHub {
    owner = "jzbor";
    repo = "nix-sweep";
    rev = "v${version}";
    hash = "sha256-o+8VY8DxBtUhOjrFKKm7DGBk/a6xJgpib6IU9SaKEjU=";
  };

  cargoHash = "sha256-Ld/Ig4Bmu2DPRp4rScq/r2iJESJaydVCMNg/A1lnLl4=";

  meta = {
    description = "Utility to clean up old Nix profile generations and left-over garbage collection roots";
    homepage = "https://github.com/jzbor/nix-sweep";
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [];
    mainProgram = "nix-sweep";
  };
}
