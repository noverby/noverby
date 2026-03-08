_: {
  programs.adb.enable = true;
  users.users.noverby.extraGroups = ["adbusers" "dialout"];
  boot.kernel.sysctl."kernel.dmesg_restrict" = 0;
}
