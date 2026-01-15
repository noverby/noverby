{
  lib,
  rustPlatform,
  fetchFromGitHub,
}:
rustPlatform.buildRustPackage rec {
  pname = "envy";
  version = "0.6.0";

  src = fetchFromGitHub {
    owner = "mre";
    repo = "envy";
    rev = "master";
    hash = "sha256-33efk7Udkchy2bm9hjcjNlUxCNFkaVJQG/U3h/vp/2g=";
  };

  buildFeatures = ["bash-support"];

  cargoHash = "sha256-L4Pm57GzcbC7bWafv33g3G3hD2vtcBu1L7Pkvuox4mY=";

  doCheck = false;

  meta = {
    description = "Manage environment variables without cluttering your .zshrc";
    homepage = "https://github.com/mre/envy";
    license = with lib.licenses; [asl20 mit];
    maintainers = with lib.maintainers; [noverby];
    mainProgram = "envy";
  };
}
