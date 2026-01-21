{
  inputs,
  config,
  ...
}: let
  envJson = builtins.readFile inputs.env.outPath;
  env =
    if envJson != ""
    then builtins.fromJSON envJson
    else {PWD = "${config.home.homeDirectory}/Work/${config.home.username}";};
in {
  devenv.root = env.PWD;
}
