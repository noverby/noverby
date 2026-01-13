let
  noverby = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOachAYzBH8Qaorvbck99Fw+v6md3BeVtfL5PJ/byv4Cc";
  nitrokey3-fido2-hmac = "age1efazxe5tgepdv5czxzj5x844dj265dar4sej42qc8mjp2czulazslqutuw";
in {
  "resolved.age".publicKeys = [noverby nitrokey3-fido2-hmac];
  "u2f-keys.age".publicKeys = [noverby nitrokey3-fido2-hmac];
}
