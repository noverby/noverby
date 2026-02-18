{pkgs, ...}: {
  environment = {
    systemPackages = with pkgs; [
      helix
      tailspin
    ];
    sessionVariables = {
      PAGER = "tspin --print";
      SYSTEMD_PAGERSECURE = "1";
      NIXOS_OZONE_WL = "1";
    };
  };
}
