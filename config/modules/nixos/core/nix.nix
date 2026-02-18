{lib, ...}: {
  nix = {
    settings = {
      netrc-file = "/home/noverby/.netrc";
      max-jobs = 100;
      connect-timeout = 10;
      stalled-download-timeout = 10;
      trusted-users = ["root" "noverby"];
      experimental-features = "nix-command flakes ca-derivations";
      download-buffer-size = 1024 * 1024 * 1024;
      substituters = [
        "https://overby-me.cachix.org"
        "https://nix-community.cachix.org"
        "https://zed.cachix.org"
        "https://cache.garnix.io"
      ];
      trusted-public-keys = [
        "overby-me.cachix.org-1:dU7qOj5u97QZz98nqnh+Nwait6c+2d2Eq0KTOAXTyp4="
        "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
        "zed.cachix.org-1:/pHQ6dpMsAZk2DiP4WCL0p9YDNKWj2Q5FL20bNmw1cU="
        "cache.garnix.io:CTFPyKSLcx5RMJKfLo5EEPUObbA78b0YQ2DTCJXqr9g="
      ];
    };
    daemonCPUSchedPolicy = "idle";
    daemonIOSchedClass = "idle";
    extraOptions = ''
      min-free = ${toString (30 * 1024 * 1024 * 1024)}
      max-free = ${toString (40 * 1024 * 1024 * 1024)}
    '';
  };

  # Enforce Niceness
  systemd.services.nix-daemon.serviceConfig = {
    Nice = lib.mkForce 15;
    IOSchedulingClass = lib.mkForce "idle";
    IPEgressPriority = 7;
    IPIngressPriority = 7;
  };
}
