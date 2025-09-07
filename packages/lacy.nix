{
  lib,
  rustPlatform,
  fetchFromGitHub,
}:
rustPlatform.buildRustPackage rec {
  pname = "lacy";
  version = "0.2.2";

  src = fetchFromGitHub {
    owner = "timothebot";
    repo = "lacy";
    rev = "v${version}";
    hash = "sha256-LTLLpzv5wX8i1Hmg651HrKzxJS/fSgIs5rrAA2tNg74=";
  };

  cargoHash = "sha256-N5avoN3QCCYMF29Cvbwha+iBAXPncOttWxGpVZ70EqI=";

  doCheck = false;

  meta = {
    description = "Fast magical cd alternative for lacy terminal navigators";
    homepage = "https://github.com/timothebot/lacy";
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [];
    mainProgram = "lacy";
  };
}
