{
  stdenv,
  fetchurl,
  lib,
  cacert,
}:
stdenv.mkDerivation {
  pname = "magic";
  version = "latest";

  src = fetchurl {
    url = "https://magic.modular.com";
    hash = "sha256-HSoTz6MqQpw9MEoiXGsPC88Qpvpa1g/VGW//uOEPX+c=";
  };
  dontUnpack = true;

  buildInputs = [cacert];

  outputHashMode = "recursive";
  outputHash = "sha256-X8w9bpRiyvAAeHa1gXhFxpBOF/wqHn8CxBTdlv0bNPY";

  installPhase = ''
    mkdir -p $out/bin

    export HOME=.
    bash $src

    cp .modular/bin/magic $out/bin
  '';
}
