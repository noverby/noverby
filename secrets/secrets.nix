let
  noverby = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOachAYzBH8Qaorvbck99Fw+v6md3BeVtfL5PJ/byv4Cc";
in {
  "resolved.age".publicKeys = [noverby];
  "u2f-keys.age".publicKeys = [noverby];
}
