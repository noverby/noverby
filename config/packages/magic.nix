{
  stdenv,
  fetchurl,
}:
stdenv.mkDerivation rec {
  pname = "magic";
  version = "0.7.2";

  src = fetchurl {
    url = "https://dl.modular.com/public/magic/raw/versions/${version}/magic-x86_64-unknown-linux-musl";
    hash = "sha256-L+zh1XH9RD+XqcxC1A9V9a09ZN6bda84e4khLimN7NU=";
  };
  dontUnpack = true;

  installPhase = ''
    mkdir -p $out/bin
    cp $src $out/bin/magic
    chmod +x $out/bin/magic
  '';
}
