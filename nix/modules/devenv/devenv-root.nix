{inputs, ...}: let
  envJson = builtins.readFile inputs.env.outPath;
  env =
    if envJson != ""
    then builtins.fromJSON envJson
    else {PWD = "/home/noverby/Work/noverby";};
in {
  devenv.root = env.PWD;
}
