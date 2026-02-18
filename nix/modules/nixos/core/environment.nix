{pkgs, ...}: let
  tspin-pager = pkgs.writeShellScriptBin "tspin-pager" ''
    if [ -t 1 ]; then
      exec ${pkgs.tailspin}/bin/tspin "$@"
    else
      exec ${pkgs.tailspin}/bin/tspin --print "$@"
    fi
  '';
in {
  environment = {
    systemPackages = with pkgs; [
      helix
      tailspin
      tspin-pager
    ];
    sessionVariables = {
      PAGER = "tspin-pager";
      SYSTEMD_PAGERSECURE = "1";
      NIXOS_OZONE_WL = "1";
    };
  };
}
