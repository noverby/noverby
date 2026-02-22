{
  inputs,
  lib,
  ...
}: let
  envJson = lib.readFile inputs.env.outPath;
  env =
    if envJson != ""
    then lib.fromJSON envJson
    else {PWD = "/home/noverby/Work/overby-me";};
in {
  devenv.root = env.PWD;
}
