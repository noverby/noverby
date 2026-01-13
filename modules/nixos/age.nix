{pkgs, ...}: let
  rage-with-plugins = pkgs.symlinkJoin {
    name = "rage-with-plugins";
    paths = [pkgs.rage];
    buildInputs = [pkgs.makeWrapper];
    postBuild = ''
      wrapProgram $out/bin/rage --prefix PATH : ${pkgs.lib.makeBinPath [pkgs.age-plugin-fido2-hmac]}
    '';
  };
in {
  age = {
    ageBin = "${rage-with-plugins}/bin/rage";
    identityPaths = ["/etc/age/fido2_host_key" "/etc/ssh/ssh_host_ed25519_key" "/etc/ssh/ssh_host_rsa_key"];
  };
  environment = {
    systemPackages = with pkgs; [
      age-plugin-fido2-hmac
    ];
  };
}
