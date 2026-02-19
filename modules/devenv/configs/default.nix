# After editing config files in this directory, run `direnv reload` to apply changes to the repository.
{
  enterShell = ''
    ln -sf ${./biome-nix.json} biome.json
    ln -sf ${./deno.json} deno.json
    ln -sf ${./rumdl.toml} rumdl.toml
    ln -sf ${./typos.toml} typos.toml
    ln -sf ${./commitlintrc.yml} .commitlintrc.yml
    mkdir -p .zed && cp -f ${./zed/settings.json} .zed/settings.json
  '';
}
