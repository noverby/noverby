{
  config,
  pkgs,
  ...
}: let
  # The C udevd binary has the original systemd store path compiled in for its
  # built-in rules directory. This means it reads rules like 90-vconsole.rules
  # from the original systemd package, which contain hardcoded references to
  # the original systemd's systemctl (wrong path for systemd-rs).
  #
  # We create a small overlay package containing only the rules files that
  # reference systemctl, so they end up in /etc/udev/rules.d/ with the correct
  # paths. Files in /etc/udev/rules.d/ take priority over the compiled-in
  # built-in rules path, fixing the hardcoded systemctl invocations.
  udevRulesOverride = pkgs.runCommand "systemd-rs-udev-rules-override" {} ''
    mkdir -p $out/lib/udev/rules.d
    for rule in ${config.systemd.package}/lib/udev/rules.d/*.rules; do
      if grep -q 'systemctl' "$rule"; then
        cp "$rule" "$out/lib/udev/rules.d/$(basename "$rule")"
      fi
    done
  '';
in {
  systemd.package = pkgs.systemd-rs-systemd;

  services.udev.packages = [udevRulesOverride];
}
