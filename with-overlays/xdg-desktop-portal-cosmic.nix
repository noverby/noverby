# Can be removed, when upgrading to Cosmic beta1
(final: prev: {
  xdg-desktop-portal-cosmic = prev.xdg-desktop-portal-cosmic.override (let
    rp = prev.rustPlatform;
    rev = "a069d57d359c4fe25a0415bdfee6c967e07b5a48";
  in {
    rustPlatform =
      rp
      // {
        buildRustPackage = args:
          rp.buildRustPackage (
            (args {})
            // {
              version = "unstable-2025-09-12";
              env = {
                VERGEN_GIT_COMMIT_DATE = "2025-09-12";
              };
              src = prev.fetchFromGitHub {
                inherit rev;
                owner = "pop-os";
                repo = "xdg-desktop-portal-cosmic";
                hash = "sha256-jwLTgzchY18rPbc93DEADmJ2XHkLBsO002YoxWbCq2Y="; # Replace with actual hash
              };
              cargoHash = "sha256-uJKwwESkzqweM4JunnMIsDE8xhCyjFFZs1GiJAwnbG8=";
            }
          );
      };
  });
})
