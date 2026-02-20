{
  lib,
  rustPlatform,
  fetchFromGitHub,
}:
rustPlatform.buildRustPackage {
  pname = "zed-mojo";
  version = "0-unstable-2025-07-07";

  src = fetchFromGitHub {
    owner = "bajrangCoder";
    repo = "zed-mojo";
    rev = "87705307d994c1f9a2c8465b0e6edd8f73260bab";
    hash = "sha256-it5nGPZtkSrv0HeDNM17QLYfFBw+qLOLoXVIgrt5tJo=";
  };

  cargoHash = "sha256-5kQPHyKLJ0bxwZPdb1W7aNi6XcryZVZ1Qp+efcK2Zas=";

  meta = {
    description = "Mojo language support for Zed";
    homepage = "https://github.com/bajrangCoder/zed-mojo";
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [noverby];
  };
}
