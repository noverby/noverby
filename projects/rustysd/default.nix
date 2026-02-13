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
    systemd,
  }:
    rustPlatform.buildRustPackage {
      pname = "rustysd";
      version = "unstable";

      src = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          ./Cargo.toml
          ./Cargo.lock
          ./src
        ];
      };

      cargoHash = "sha256-7McI8t3zWCGNMRmoposXK9xfl7Y0VMCSLnPA5h1L4HE=";

      nativeBuildInputs = [
        pkg-config
      ];

      buildInputs = [
        dbus
      ];

      doCheck = false;

      postInstall = ''
        # Copy data/config files from systemd that NixOS modules expect
        cp -r ${systemd}/example $out/example
        cp -r ${systemd}/lib $out/lib
        cp -r ${systemd}/etc $out/etc 2>/dev/null || true
        cp -r ${systemd}/share $out/share 2>/dev/null || true

        # Copy systemd binaries that NixOS modules expect, but do not
        # overwrite any binaries already provided by rustysd itself.
        for bin in ${systemd}/bin/*; do
          name=$(basename "$bin")
          if [ ! -e "$out/bin/$name" ]; then
            cp -a "$bin" "$out/bin/$name"
          fi
        done

        # Provide sbin as a symlink to bin (matching systemd layout)
        if [ ! -e "$out/sbin" ]; then
          ln -s bin "$out/sbin"
        fi

        # Replace all references to the real systemd store path with
        # the rustysd output path so NixOS module substitutions work.
        find $out -type f | while read -r f; do
          if file "$f" | grep -q text; then
            substituteInPlace "$f" \
              --replace-quiet "${systemd}" "$out"
          fi
        done

        # Fix broken symlinks that pointed within the systemd package
        find $out -type l | while read -r link; do
          target=$(readlink "$link")
          if [[ "$target" == ${systemd}* ]]; then
            newtarget="$out''${target#${systemd}}"
            ln -sf "$newtarget" "$link"
          fi
        done
      '';

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
