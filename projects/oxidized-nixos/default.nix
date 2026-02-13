{
  devShells.oxidized-nixos = pkgs: {
    packages = with pkgs; [
      just
      nix-tree
    ];
  };
  nixosConfigurations.oxidized-nixos = {
    inputs,
    lib,
    ...
  }: {
    system = "x86_64-linux";
    modules = [
      ({
        modulesPath,
        pkgs,
        lib,
        ...
      }: {
        imports = ["${modulesPath}/profiles/qemu-guest.nix"];

        networking.useDHCP = false;

        system = {
          name = "oxidized";
          stateVersion = "25.11";
        };

        boot = {
          kernelParams = ["init=/nix/var/nix/profiles/system/init"];
          loader.grub.enable = false;
        };

        fileSystems."/" = {
          device = "/dev/disk/by-label/nixos";
          autoResize = true;
          fsType = "ext4";
        };

        users.users = {
          oxidized = {
            isNormalUser = true;
            extraGroups = ["wheel"];
            password = "oxidized";
          };
        };

        system.replaceDependencies.replacements = let
          uutils = pkgs.uutils-coreutils-noprefix;
          mkBrushBash = name:
            pkgs.runCommand name {nativeBuildInputs = [pkgs.stdenv.cc];} ''
              mkdir -p $out/bin
              cat > wrapper.c << 'EOF'
              #include <signal.h>
              #include <stdlib.h>
              #include <string.h>
              #include <unistd.h>

              int main(int argc, char *argv[]) {
                int login = argv[0][0] == '-';
                /* Ignore SIGTTOU so that tcsetpgrp() succeeds even from a
                   background process group (required for brush's
                   TerminalControl::acquire() sequence).
                   Ignore SIGTSTP so Ctrl-Z doesn't stop the shell itself.
                   Do NOT ignore SIGTTIN: if the shell somehow remains in a
                   background group, ignoring SIGTTIN causes read() on the
                   terminal to return EIO instead of stopping the process,
                   which makes brush exit with "input error occurred". */
                signal(SIGTTOU, SIG_IGN);
                signal(SIGTSTP, SIG_IGN);
                /* Set up the process group and foreground ownership here in
                   the wrapper, so brush's TerminalControl::acquire() becomes
                   a no-op. This avoids brush's silent tcsetpgrp() failure
                   (it ignores ENOTTY) that would leave the shell in a
                   background group on serial consoles. */
                if (isatty(STDIN_FILENO)) {
                  setpgid(0, 0);
                  tcsetpgrp(STDIN_FILENO, getpgrp());
                }
                char **new_argv = malloc(sizeof(char *) * (argc * 3 + 7));
                int n = 0;
                new_argv[n++] = login ? "-brush" : "brush";
                if (login) new_argv[n++] = "--login";
                /* Force the minimal input backend to avoid crossterm hanging
                   on serial consoles (ttyS0) where escape-sequence-based
                   terminal probing never gets a response. The minimal backend
                   uses plain stdin read_line which works everywhere. */
                new_argv[n++] = "--input-backend";
                new_argv[n++] = "minimal";
                int done = 0;
                for (int i = 1; i < argc && !done; i++) {
                  if (argv[i][0] == '-' && argv[i][1] && argv[i][1] != '-') {
                    for (int j = 1; argv[i][j] && !done; j++) {
                      switch (argv[i][j]) {
                        case 'e': new_argv[n++] = "-o"; new_argv[n++] = "errexit"; break;
                        case 'u': new_argv[n++] = "-o"; new_argv[n++] = "nounset"; break;
                        case 'a': new_argv[n++] = "-o"; new_argv[n++] = "allexport"; break;
                        case 'b': new_argv[n++] = "-o"; new_argv[n++] = "notify"; break;
                        case 'f': new_argv[n++] = "-o"; new_argv[n++] = "noglob"; break;
                        case 'h': new_argv[n++] = "-o"; new_argv[n++] = "hashall"; break;
                        case 'm': new_argv[n++] = "-o"; new_argv[n++] = "monitor"; break;
                        case 'p': new_argv[n++] = "-o"; new_argv[n++] = "privileged"; break;
                        case 'c': case 'o': case 'O':
                          { char f[3] = {'-', argv[i][j], 0}; new_argv[n++] = strdup(f); }
                          for (++i; i < argc; i++) new_argv[n++] = argv[i];
                          done = 1; break;
                        default: { char f[3] = {'-', argv[i][j], 0}; new_argv[n++] = strdup(f); break; }
                      }
                    }
                  } else {
                    new_argv[n++] = argv[i];
                    if (argv[i][0] != '-') { for (++i; i < argc; i++) new_argv[n++] = argv[i]; done = 1; }
                  }
                }
                new_argv[n] = NULL;
                execv("${pkgs.brush}/bin/brush", new_argv);
                return 1;
              }
              EOF
              cc -O2 -o $out/bin/bash wrapper.c
              ln -s bash $out/bin/sh
            '';
        in [
          {
            original = pkgs.coreutils;
            replacement = uutils.overrideAttrs {name = "coreutils-9.8";};
          }
          {
            original = pkgs.coreutils-full;
            replacement = uutils.overrideAttrs {name = "coreutils-full-9.8";};
          }
          {
            original = pkgs.bashNonInteractive;
            replacement = mkBrushBash "bash-0.4.0";
          }
          {
            original = pkgs.bashInteractive;
            replacement = mkBrushBash "bash-interactive-0.4.0";
          }
        ];

        services.getty.autologinUser = "oxidized";
        security.sudo.enable = false;
        security.sudo-rs = {
          enable = true;
          wheelNeedsPassword = false;
        };
      })
    ];
  };
}
