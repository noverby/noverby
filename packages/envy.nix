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
    rev = version;
    hash = "sha256-0uenByFb7VJSZaAoV3eSZ4HFRSEFbWBEzmMHPWd9w8I=";
  };

  cargoHash = "sha256-ho30I+bv7rIyC798bkBdOhqtWW5rvaw6HpNBpqe+5FI=";

  doCheck = false;

  meta = {
    description = "Manage environment variables without cluttering your .zshrc";
    homepage = "https://github.com/mre/envy";
    license = with lib.licenses; [asl20 mit];
    maintainers = with lib.maintainers; [];
    mainProgram = "envy";
  };
}
