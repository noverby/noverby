{
  lib,
  rustPlatform,
  fetchFromGitHub,
}:
rustPlatform.buildRustPackage {
  pname = "cpx";
  version = "unstable";

  src = fetchFromGitHub {
    owner = "11happy";
    repo = "cpx";
    rev = "bb59a6f660bb6544b6e0d29a79cbdce6e96ffdd7";
    hash = "sha256-QG2vUlS34h2N1ovmoGEOGACbzVnKDH/WUvp/urnV2tc=";
  };

  cargoHash = "sha256-atEB43eB8btQfMXPTCfsZ6bbAUIPzF8lUELx0Rdul84=";

  meta = {
    description = "";
    homepage = "https://github.com/11happy/cpx";
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [noverby];
    mainProgram = "cpx";
  };
}
