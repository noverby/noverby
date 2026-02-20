{
  lib,
  python3Packages,
  autoPatchelfHook,
  stdenv,
}:
python3Packages.buildPythonPackage rec {
  pname = "wasmtime";
  version = "41.0.0";
  format = "wheel";

  src = let
    srcs = {
      x86_64-linux = python3Packages.fetchPypi {
        inherit pname version format;
        dist = "py3";
        python = "py3";
        abi = "none";
        platform = "manylinux1_x86_64";
        hash = "sha256-rX6GZDAxPrLuB8hYEeUkNEiESJ0AiW87Ika2VVP+Miw=";
      };
      aarch64-linux = python3Packages.fetchPypi {
        inherit pname version format;
        dist = "py3";
        python = "py3";
        abi = "none";
        platform = "manylinux2014_aarch64";
        hash = "sha256-4OpEWE9g3PpiCvgtT8JYkki89kqTkFtUrDFEJCETtIo=";
      };
    };
  in
    srcs.${stdenv.hostPlatform.system}
      or (throw "wasmtime-py: unsupported platform ${stdenv.hostPlatform.system}");

  nativeBuildInputs = [
    autoPatchelfHook
  ];

  buildInputs = [
    stdenv.cc.cc.lib
  ];

  pythonImportsCheck = ["wasmtime"];

  meta = with lib; {
    description = "Python bindings for the Wasmtime WebAssembly runtime";
    homepage = "https://github.com/bytecodealliance/wasmtime-py";
    license = licenses.asl20;
    platforms = ["x86_64-linux" "aarch64-linux"];
  };
}
