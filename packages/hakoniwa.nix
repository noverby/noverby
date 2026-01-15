{
  lib,
  rustPlatform,
  fetchFromGitHub,
  libseccomp,
}:
rustPlatform.buildRustPackage rec {
  pname = "hakoniwa";
  version = "1.2.1";

  src = fetchFromGitHub {
    owner = "souk4711";
    repo = "hakoniwa";
    rev = "v${version}";
    hash = "sha256-Xp3/hV/tHvGowu4HpvMKhNMNhgIzxwjiSXGi7B2+4BQ=";
  };

  cargoHash = "sha256-vPS9d+nkBGXA+y88TUAIn4TuScUWPOR25iOJQ3eKWUs=";

  buildInputs = [
    libseccomp
  ];

  # Tests tries to use /bin/sleep
  doCheck = false;

  meta = {
    description = "Process isolation for Linux using namespaces, resource limits, landlock and seccomp";
    homepage = "https://github.com/souk4711/hakoniwa";
    license = lib.licenses.gpl3;
    maintainers = with lib.maintainers; [noverby];
    mainProgram = "hakoniwa";
  };
}
