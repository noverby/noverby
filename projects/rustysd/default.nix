{
  packages.rustysd = {
    lib,
    rustPlatform,
    fetchFromGitHub,
    pkg-config,
    dbus,
    kbd,
    kmod,
    util-linuxMinimal,
  }:
    rustPlatform.buildRustPackage {
      pname = "rustysd";
      version = "unstable";

      src = ./.;

      cargoHash = "sha256-7McI8t3zWCGNMRmoposXK9xfl7Y0VMCSLnPA5h1L4HE=";

      nativeBuildInputs = [
        pkg-config
      ];

      buildInputs = [
        dbus
      ];

      doCheck = false;

      passthru = {
        inherit kbd kmod;
        util-linux = util-linuxMinimal;
        interfaceVersion = 2;
        withBootloader = false;
        withCryptsetup = false;
        withEfi = false;
        withFido2 = false;
        withHostnamed = false;
        withImportd = false;
        withKmod = false;
        withLocaled = false;
        withMachined = false;
        withNetworkd = false;
        withPortabled = false;
        withSysupdate = false;
        withTimedated = false;
        withTpm2Tss = false;
        withTpm2Units = false;
        withUtmp = false;
      };

      meta = {
        description = "A service manager that is able to run \"traditional\" systemd services, written in rust";
        homepage = "https://github.com/KillingSpark/rustysd";
        license = lib.licenses.mit;
        maintainers = with lib.maintainers; [noverby];
        mainProgram = "rustysd";
      };
    };
}
