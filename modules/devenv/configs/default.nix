# Run `direnv reload` to apply changes to the repository.
{
  enterShell = ''
    ln -sf ${./biome-nix.jsonc} biome.jsonc
    ln -sf ${./deno.jsonc} deno.jsonc
    ln -sf ${./lychee.toml} lychee.toml
    ln -sf ${./rumdl.toml} rumdl.toml
    ln -sf ${./typos.toml} typos.toml
    ln -sf ${./commitlintrc.yml} .commitlintrc.yml
    mkdir -p .zed && cp -f ${./zed/settings.jsonc} .zed/settings.json && cp -f ${./zed/rules.md} .zed/rules.md
  '';
}
