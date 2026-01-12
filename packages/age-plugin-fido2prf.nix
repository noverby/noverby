{
  lib,
  buildGoModule,
  fetchFromGitHub,
  libfido2,
  pkg-config,
}:
buildGoModule rec {
  pname = "age-plugin-fido2prf";
  version = "0.3.0";

  src = fetchFromGitHub {
    owner = "FiloSottile";
    repo = "typage";
    rev = "v${version}";
    hash = "sha256-JGEn1xIzfLyoCWd/aRRG08Z/OoviEyZF+tGEfcj9DXw=";
  };

  vendorHash = "sha256-XrgZBvNyVUhKJ87vfd9aZh6aW+JifJWUu/ggNQZKwo0=";

  subPackages = ["fido2prf/cmd/age-plugin-fido2prf"];

  nativeBuildInputs = [pkg-config];
  buildInputs = [libfido2];

  ldflags = ["-s" "-w"];

  meta = {
    description = "An age plugin that uses FIDO2 security keys with the PRF extension for file encryption";
    homepage = "https://github.com/FiloSottile/typage";
    license = lib.licenses.bsd3;
    maintainers = with lib.maintainers; [];
    mainProgram = "age-plugin-fido2prf";
  };
}
