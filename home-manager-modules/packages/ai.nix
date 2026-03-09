{pkgs, ...}: {
  home.packages = with pkgs.pkgsUnstable; [
    mistral-vibe

    # LLMs just love to use these tools
    bc
    jq
    python3
  ];
}
