{
  packages.systemd-rs = {
    lib,
    rustPlatform,
    pkg-config,
    dbus,
  }:
    rustPlatform.buildRustPackage {
      pname = "systemd-rs";
      version = "unstable";

      src = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          ./Cargo.toml
          ./Cargo.lock
          ./src
        ];
      };

      cargoHash = "sha256-piZYUTsoTa7KrEyao/B0cnA24MQWRYD+WzxrT+Y47zQ=";

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
        mainProgram = "systemd-rs";
      };
    };

  packages.systemd-rs-systemd = {
    lib,
    runCommand,
    makeBinaryWrapper,
    systemd-rs,
    kbd,
    kmod,
    util-linuxMinimal,
    systemd,
  }:
    runCommand "systemd-rs-systemd-${systemd-rs.version}" {
      nativeBuildInputs = [makeBinaryWrapper];

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

      meta =
        systemd-rs.meta
        // {
          description = "systemd-rs packaged as a systemd drop-in replacement for NixOS";
        };
    } ''
      mkdir -p $out

      # Copy data/config files from systemd that NixOS modules expect
      cp -r ${systemd}/example $out/example
      cp -r ${systemd}/lib $out/lib
      cp -r ${systemd}/etc $out/etc 2>/dev/null || true
      cp -r ${systemd}/share $out/share 2>/dev/null || true

      # Make copied files writable so we can overwrite them
      chmod -R u+w $out

      # Start with all systemd binaries
      mkdir -p $out/bin
      for bin in ${systemd}/bin/*; do
        name=$(basename "$bin")
        cp -a "$bin" "$out/bin/$name"
      done

      # Overwrite with systemd-rs binaries (takes precedence)
      for bin in ${systemd-rs}/bin/*; do
        name=$(basename "$bin")
        cp -a "$bin" "$out/bin/$name"
      done

      # Provide sbin as a symlink to bin (matching systemd layout)
      if [ ! -e "$out/sbin" ]; then
        ln -s bin "$out/sbin"
      fi

      # Replace the systemd init binary with a wrapper that execs systemd-rs,
      # so NixOS actually boots with systemd-rs as PID 1 instead of systemd.
      # NixOS uses $out/lib/systemd/systemd as the init binary (stage-2).
      # We can't symlink because systemd-rs's main() dispatches on argv[0]
      # ending with "systemd-rs", so we need a wrapper script.
      rm -f $out/lib/systemd/systemd
      makeBinaryWrapper ${systemd-rs}/bin/systemd $out/lib/systemd/systemd \
        --argv0 systemd-rs

      # Replace all references to the real systemd store path with
      # the systemd-rs-systemd output path so NixOS module substitutions work.
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
}
