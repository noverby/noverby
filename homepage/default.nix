{
  imports = [
    ./backend/default.nix
  ];

  devShells.homepage = pkgs: {
    packages = with pkgs; [
      just
      deno
    ];
  };

  packages.homepage-frontend = {lib, ...}:
    lib.buildDenoProject {
      pname = "homepage-frontend";
      src = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          ./package.json
          ./deno.lock
          ./tsconfig.json
          ./rsbuild.config.ts
          ./src
          ./public
        ];
      };
      buildCommand = "deno run -A npm:@rsbuild/core build";
      installPhase = "cp -r dist $out";
      meta.description = "Homepage frontend built with Deno + RSBuild";
    };
}
