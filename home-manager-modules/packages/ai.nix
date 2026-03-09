{
  lib,
  pkgs,
  ...
}: {
  home.packages = with pkgs.pkgsUnstable;
    [
      # LLMs just love to use these tools
      bc
      jq
      python3
    ]
    ++ lib.optionals pkgs.stdenv.hostPlatform.isx86_64 [
      mistral-vibe
    ];
}
