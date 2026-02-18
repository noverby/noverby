pkgs: let
  alejandra = "${pkgs.alejandra}/bin/alejandra";
  biome = "${pkgs.biome}/bin/biome format --write";
  rustfmt = "${pkgs.rustfmt}/bin/rustfmt --edition=2024";
  rumdl = "${pkgs.rumdl}/bin/rumdl fmt --no-cache";
in {
  "*.nix" = alejandra;
  "*.json" = biome;
  "*.js" = biome;
  "*.ts" = biome;
  "*.tsx" = biome;
  "*.rs" = rustfmt;
  "*.md" = rumdl;
}
