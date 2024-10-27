{
  stdenv,
  magic,
  python3,
}:
stdenv.mkDerivation {
  pname = "mojo";
  version = "latest";

  buildInputs = [magic python3];
  dontUnpack = true;

  outputHashMode = "recursive";
  outputHash = "sha256-X8w9bpRiyvAAeHa1gXhFxpBOF/wqHn8CxBTdlv0bNPY";

  installPhase = ''
    mkdir -p $out/bin

    export HOME=.
    export MOJO_PATH=$HOME/.modular/bin
    echo "" > Env.yml
    magic init dump --format mojoproject -c conda-forge -c https://conda.modular.com/max-nightly

    cp dump/.magic/envs/default/bin/mojo $out/bin
  '';
}
