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
    rev = "bash-support";
    hash = "sha256-EJc40+1RYf6mUQbFhcYARrj6ERL5jRtPdYL62bj9zHw=";
  };

  buildFeatures = ["bash-support"];

  cargoHash = "sha256-lk9LOg1vVus6i5d1tcT8FWWchrLDlLXpht9xESfDpNM=";

  doCheck = false;

  meta = {
    description = "Manage environment variables without cluttering your .zshrc";
    homepage = "https://github.com/mre/envy";
    license = with lib.licenses; [asl20 mit];
    maintainers = with lib.maintainers; [];
    mainProgram = "envy";
  };
}
