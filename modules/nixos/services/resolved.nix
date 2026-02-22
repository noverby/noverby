{src, ...}: {
  age.secrets."resolved-secret.conf" = {
    file = src + /config/secrets/resolved.age;
    path = "/etc/systemd/resolved.conf.d/10-secret.conf";
    owner = "systemd-resolve";
    group = "systemd-resolve";
    mode = "600";
  };
  services.resolved = {
    enable = true;
    extraConfig = ''
      DNSOverTLS=yes
      MulticastDNS=resolve
    '';
  };
}
