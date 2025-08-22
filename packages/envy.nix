{
  lib,
  rustPlatform,
  fetchFromGitHub,
}:
rustPlatform.buildRustPackage rec {
  pname = "envy";
  version = "0.5.0";

  src = fetchFromGitHub {
    owner = "mre";
    repo = "envy";
    rev = version;
    hash = "sha256-nzRp2r+WzCVT/ASaHh8pa15rRCo8B0Gg+1wyuQ+GKNc=";
  };

  cargoHash = "sha256-wf2Cl5Suo4LjuEn/ooqdkR4HJZP13webMJmVi3iGMWQ=";

  meta = {
    description = "Manage environment variables without cluttering your .zshrc";
    homepage = "https://github.com/mre/envy";
    license = with lib.licenses; [asl20 mit];
    maintainers = with lib.maintainers; [];
    mainProgram = "envy";
  };
}
