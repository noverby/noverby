{...}: {
  nixpkgs.config = {
    #packageOverrides = with inputs;
    #  prev: {
    #  stardustxr = stardustxr.packages.${pkgs.system}.default;
    #  flatland = flatland.packages.${pkgs.system}.default;
    #   monado = prev.monado.overrideAttrs (_: {
    #     version = "unstable-2024-03-07";
    #     src = prev.fetchFromGitHub {
    #       owner = "noverby";
    #       repo = "monado-mirror";
    #       rev = "4bd442913eff792c4120b9195be6206a2d7a11cd";
    #       hash = "sha256-sDGuNT2rmE25aRj3fP5hHUN76fUVqhLNebdv112cqpw=";
    #     };
    #   });
    #  };
  };
}
