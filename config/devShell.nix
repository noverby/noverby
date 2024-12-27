{
  inputs,
  pkgs,
  ...
}:
inputs.devenv.lib.mkShell
{
  inherit inputs pkgs;

  modules = [
    {
      devenv.root = builtins.readFile inputs.devenv-root.outPath;
    }
    {
      packages = with pkgs; [yarn nodejs just rustc cargo nixd nil alejandra magic];
    }
  ];
}
