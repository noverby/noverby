{pkgs, ...}: {
  services = {
    gnome = {
      gnome-keyring.enable = true;
      gnome-browser-connector.enable = true;
    };
    xserver = {
      displayManager.gdm = {
        enable = true;
        wayland = true;
      };
      desktopManager.gnome.enable = true;
    };
  };
  environment.gnome.excludePackages = with pkgs; [epiphany];
  # Unmanaged gnome-extensions deps
  environment.sessionVariables = with pkgs; {
    GI_TYPELIB_PATH = map (pkg: "${pkg}/lib/girepository-1.0") [vte pango harfbuzz gtk3 gdk-pixbuf at-spi2-core];
  };

  # Security
  security.pam.services = {
    gdm.enableGnomeKeyring = true;
    login.fprintAuth = false;
    gdm-fingerprint = with pkgs; {
      text = ''
        auth       required                    pam_shells.so
        auth       requisite                   pam_nologin.so
        auth       requisite                   pam_faillock.so      preauth
        auth       required                    ${fprintd}/lib/security/pam_fprintd.so
        auth       optional                    pam_permit.so
        auth       required                    pam_env.so
        auth       [success=ok default=1]      ${gdm}/lib/security/pam_gdm.so
        auth       optional                    ${gnome-keyring}/lib/security/pam_gnome_keyring.so

        account    include                     login

        password   required                    pam_deny.so

        session    include                     login
        session    optional                    ${gnome-keyring}/lib/security/pam_gnome_keyring.so auto_start
      '';
    };
  };
  programs.seahorse.enable = true;
}
