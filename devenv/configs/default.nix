# Run `direnv reload` to apply changes to the repository.
{
  pkgs,
  lib,
  ...
}: let
  commitlintrc = import ./commitlintrc.nix {inherit pkgs lib;};
in {
  enterShell = ''
    ln -sf ${./biome-nix.jsonc} biome.jsonc
    ln -sf ${./deno.jsonc} deno.jsonc
    ln -sf ${./lychee.toml} lychee.toml
    ln -sf ${./rumdl.toml} rumdl.toml
    ln -sf ${./typos.toml} typos.toml
    ln -sf ${./secretsignore} .secretsignore
    ln -sf ${commitlintrc} .commitlintrc.yml
    mkdir -p .zed && cp -f ${./zed/settings.jsonc} .zed/settings.json
    cp -f ${./zed/rules.md} .rules
  '';
}
