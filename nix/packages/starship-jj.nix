{
  lib,
  rustPlatform,
  fetchFromGitLab,
}:
rustPlatform.buildRustPackage rec {
  pname = "starship-jj";
  version = "0.7.0";

  src = fetchFromGitLab {
    owner = "lanastara_foss";
    repo = "starship-jj";
    rev = version;
    hash = "sha256-EgOKjPJK6NdHghMclbn4daywJ8oODiXkS48Nrn5cRZo=";
  };

  cargoHash = "sha256-NNeovW27YSK/fO2DjAsJqBvebd43usCw7ni47cgTth8=";

  meta = {
    description = "Starship plugin for jj";
    homepage = "https://gitlab.com/lanastara_foss/starship-jj";
    changelog = "https://gitlab.com/lanastara_foss/starship-jj/-/blob/${src.rev}/CHANGELOG.md";
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [noverby];
    mainProgram = "starship-jj";
  };
}
