let
  publicKeys = (import ./publicKeys.nix).all;
in {
  "resolved.age" = {
    inherit publicKeys;
  };
  "u2f-keys.age" = {
    inherit publicKeys;
  };
  "id_ed25519.age" = {
    inherit publicKeys;
  };
  "id_rsa.age" = {
    inherit publicKeys;
  };
}
