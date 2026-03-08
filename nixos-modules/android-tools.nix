_: {
  programs.adb.enable = true;
  users.users.noverby.extraGroups = ["adbusers"];
  boot.kernel.sysctl."kernel.dmesg_restrict" = 0;
}
