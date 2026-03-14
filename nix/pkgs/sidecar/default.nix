{
  lib,
  rustPlatform,
  fetchFromGitHub,
  pkg-config-rs,
  webkitgtk_4_1,
}:
rustPlatform.buildRustPackage rec {
  pname = "sidecar";
  version = "1.0.0";

  src = fetchFromGitHub {
    owner = "runtimed";
    repo = "runtimed";
    rev = "${pname}-v${version}";
    hash = "sha256-iykPYOKHrYcK6K5IqtHD2WLxpcrBuYXj27eAhESOsgs=";
  };

  patches = [./wayland.patch];

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  postPatch = ''
    ln -s ${./Cargo.lock} Cargo.lock
  '';

  cargoBuildFlags = ["-p" pname];

  nativeBuildInputs = [
    pkg-config-rs
  ];

  buildInputs = [
    webkitgtk_4_1
  ];

  meta = {
    description = "Jupyter Notebook Viewer";
    homepage = "https://github.com/runtimed/runtimed/";
    license = lib.licenses.bsd3;
    maintainers = with lib.maintainers; [noverby];
    mainProgram = "sidecar";
  };
}
