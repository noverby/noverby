{
  pkgs,
  inputs,
  ...
}: {
  nixpkgs.config = {
    allowUnfree = true;
    permittedInsecurePackages = [
      "nix-2.24.5"
    ];
    packageOverrides = with inputs;
      prev: {
        #stardustxr = stardustxr.packages.${pkgs.system}.default;
        flatland = flatland.packages.${pkgs.system}.default;
        # monado = prev.monado.overrideAttrs (_: {
        #   version = "unstable-2024-03-07";
        #   src = prev.fetchFromGitHub {
        #     owner = "noverby";
        #     repo = "monado-mirror";
        #     rev = "4bd442913eff792c4120b9195be6206a2d7a11cd";
        #     hash = "sha256-sDGuNT2rmE25aRj3fP5hHUN76fUVqhLNebdv112cqpw=";
        #   };
        # });
        fprintd = prev.fprintd.overrideAttrs (_: {
          mesonCheckFlags = [
            "--no-suite"
            "fprintd:TestPamFprintd"
          ];
        });
        gcr_4 = prev.gcr_4.overrideAttrs (prevPkg: {
          nativeBuildInputs =
            prevPkg.nativeBuildInputs
            ++ [
              prev.gnupg
              prev.openssh
            ];

          mesonFlags = [
            "-Dssh_agent=true"
          ];
        });
      };
  };
}
