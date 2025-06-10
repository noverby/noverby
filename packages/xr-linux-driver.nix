{
  stdenv,
  lib,
  fetchFromGitHub,
  cmake,
  pkg-config,
  makeWrapper,
  patchelf,
  python311Packages,
  libusb1,
  curl,
  libevdev,
  json_c,
  hidapi,
  openssl,
}:
stdenv.mkDerivation {
  pname = "xr-linux-driver";
  version = "0.1.0";

  src = fetchFromGitHub {
    owner = "wheaney";
    repo = "XRLinuxDriver";
    rev = "b8a80f821d2be931d147d0a2df4c61a2fcc4c68a";
    sha256 = "sha256-GOpfRXYyXzUSuMqj6s4oebYA8KzJSzgJZ9Xs0EJLzVk=";
    fetchSubmodules = true;
  };

  nativeBuildInputs = [cmake pkg-config libusb1 curl libevdev json_c hidapi makeWrapper patchelf];
  buildInputs = with python311Packages; [python pyyaml];

  cmakeFlags = [
    "-DCMAKE_BUILD_TYPE=Release"
  ];

  installPhase = ''
    patchelf --set-rpath ${lib.makeLibraryPath [libusb1 curl libevdev json_c hidapi openssl]} xrealAirLinuxDriver
    mkdir -p $out/bin
    cp ../bin/xreal_driver_config $out/bin
    cp ./xrealAirLinuxDriver $out/bin
  '';

  meta = {
    description = "Linux Driver for XR devices";
    homepage = "https://github.com/wheaney/XRLinuxDriver";
    license = lib.licenses.mit;
  };
}
