{
  lib,
  rustPlatform,
  fetchFromGitHub,
  pkg-config,
  dbus,
}:
rustPlatform.buildRustPackage rec {
  pname = "rustysd";
  version = "unstable-2023-02-26";

  src = fetchFromGitHub {
    owner = "KillingSpark";
    repo = "rustysd";
    rev = "b11655529cfc9ccaa50831f2fe738c57610c8b1a";
    hash = "sha256-CJLbRWtp1vD1QnB81t9EjQ9kg20fY3IPEJc/O90zjdE=";
  };

  cargoHash = "sha256-lX07Kp0bFsFJ/9CwPobxI551ElfnpGsYdJBvVbg4NH8=";

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
    dbus
  ];

  doCheck = false;

  meta = {
    description = "A service manager that is able to run \"traditional\" systemd services, written in rust";
    homepage = "https://github.com/KillingSpark/rustysd";
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [noverby];
    mainProgram = "rustysd";
  };
}
