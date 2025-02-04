{
  pkgs,
  username,
  homeDirectory,
  ...
}: let
  shellAliases = {
    ga = "git add";
    gc = "git commit";
    gcm = "git commit -m";
    gca = "git commit --amend";
    gcn = "git commit --no-verify";
    gcp = "git cherry-pick";
    gcom = "git checkout master";
    gd = "git diff";
    gf = "git fetch";
    gl = "git log --oneline --no-abbrev-commit";
    glg = "git log --graph";
    gpl = "git pull";
    gps = "git push";
    gpf = "git push -f";
    gr = "git rebase";
    grm = "git rebase master";
    gri = "git rebase -i";
    grc = "git rebase --continue";
    gm = "git merge";
    gs = "git status";
    gsh = "git stash";
    gsha = "git stash apply";
    gsw = "git switch";
    gundo = "git reset HEAD~1 --soft";
    gbm = "gh pr comment --body 'bors merge'";
    gbc = "gh pr comment --body 'bors cancel'";
    gpr = "gh pr create --fill";
    du = "dust";
    cat = "bat";
    find = "fd";
    grep = "rg";
    man = "tldr";
    top = "btm";
    cd = "z";
    bg = "pueue";
    zed = "zeditor";
    optpng = "oxipng";
    unbrave = "brave";
    unbrave-dev = "brave --remote-debugging-port=9220";
    firefox-dev = "firefox -start-debugger-server 6000 -P dev http://localhost:3000";
    zen-dev = "zen -start-debugger-server 6000 -P dev http://localhost:3000";
  };
in {
  imports = [./git.nix ./vscode.nix ./zed-editor.nix];
  programs = {
    home-manager.enable = true;
    gh.enable = true;
    bat.enable = true;
    tealdeer.enable = true;
    bottom.enable = true;

    wezterm = {
      enable = true;
      extraConfig = builtins.readFile ./wezterm/config.lua;
      enableBashIntegration = true;
    };

    nushell = {
      enable = true;
      inherit shellAliases;
      configFile.source = ./nushell/config.nu;
    };

    carapace = {
      enable = true;
      enableNushellIntegration = true;
      enableBashIntegration = true;
    };

    bash = {
      enable = true;
      shellOptions = [
        "histappend"
        "checkwinsize"
        "extglob"
        "globstar"
        "checkjobs"
      ];
      initExtra = ''
        export SHELL="${pkgs.bash}/bin/bash"
      '';
      historyControl = ["ignoredups" "erasedups"];
    };

    readline = {
      enable = true;
      extraConfig = ''
        "\e[A":history-search-backward
        "\e[B":history-search-forward
        set completion-ignore-case On
        set completion-prefix-display-length 2
      '';
    };

    zellij = {
      enable = true;
      settings = {
        default_shell = "nu";
        copy_command = "wl-copy";
        scrollback_editor = "zed-uf";
        session_serialization = false;
        pane_frames = false;
        env = {
          TERM = "tmux-256color";
        };
      };
    };

    starship = {
      enable = true;
      enableNushellIntegration = true;
      enableBashIntegration = true;
      settings = {
        command_timeout = 10000;
        time = {
          disabled = false;
          format = " [$time]($style) ";
        };
        status = {
          disabled = false;
        };
        directory = {
          truncation_length = 8;
          truncation_symbol = ".../";
          truncate_to_repo = false;
        };
      };
    };

    direnv = {
      enable = true;
      enableNushellIntegration = true;
      enableBashIntegration = true;
      nix-direnv.enable = true;
    };

    zoxide = {
      enable = true;
      enableNushellIntegration = true;
      enableBashIntegration = true;
    };

    nix-index = {
      enable = true;
    };

    atuin = {
      enable = true;
      enableNushellIntegration = true;
      enableBashIntegration = true;
    };

    ssh = {
      enable = true;
      addKeysToAgent = "yes";
      controlMaster = "auto";
      controlPath = "~/.ssh/socket/%r@%h:%p";
      controlPersist = "120";
      forwardAgent = true;
      matchBlocks = {
        localhost = {
          hostname = "localhost";
          user = username;
        };
        macbook-x64 = {
          hostname = "10.0.20.137";
          user = "noverby";
        };
      };
    };

    firefox = {
      enable = true;
      package = pkgs.firefox.override {
        cfg.enableGnomeExtensions = true;
      };
      nativeMessagingHosts = [pkgs.firefoxpwa];
      profiles = rec {
        default = {
          isDefault = true;
          userChrome = builtins.readFile ./firefox/userChrome.css;
          settings = {
            "toolkit.legacyUserProfileCustomizations.stylesheets" = true;
          };
        };
        dev =
          default
          // {
            id = 1;
            isDefault = false;
          };
      };
    };

    # Unbraved Brave
    chromium = {
      enable = true;
      commandLineArgs = [
        "--enable-features=UseOzonePlatform,TouchpadOverscrollHistoryNavigation"
        "--ozone-platform=wayland"
      ];
      package = pkgs.brave;
      # TODO:
      # - https://github.com/bpc-clone/bypass-paywalls-chrome-clean
      extensions = map (id: {inherit id;}) [
        "nngceckbapebfimnlniiiahkandclblb" # Bitwarden
        "cdglnehniifkbagbbombnjghhcihifij" # Kagi
        "blaaajhemilngeeffpbfkdjjoefldkok" # Leechblock
        "eimadpbcbfnmbkopoojfekhnkhdbieeh" # Dark Reader
        "gfbliohnnapiefjpjlpjnehglfpaknnc" # Surfingkeys
        "epocinhmkcnjfjobnglchpbncndobblj" # Graze
        "fjcldmjmjhkklehbacihaiopjklihlgg" # News Feed Eradicator
        "laookkfknpbbblfpciffpaejjkokdgca" # Momentum
        "oldceeleldhonbafppcapldpdifcinji" # Language Tool
        "mdjildafknihdffpkfmmpnpoiajfjnjd" # Consent-O-Matic
        "bfhkfdnddlhfippjbflipboognpdpoeh" # Read on reMarkable
        "eenjdnjldapjajjofmldgmkjaienebbj" # Copyfish
        "nffaoalbilbmmfgbnbgppjihopabppdk" # Video Speed Controller
        "ifmhoabcaeehkljcfclfiieohkohdgbb" # Social Fixer for Facebook
        "jchobbjgibcahbheicfocecmhocglkco" # Neat URL
        "epocinhmkcnjfjobnglchpbncndobblj" # Graze
        "inlikjemeeknofckkjolnjbpehgadgge" # Distil
        "dejobinhdiimklegodgbmbifijpppopn" # Tab Reloader
        "jplgfhpmjnbigmhklmmbgecoobifkmpa" # Proton VPN
        "jdocbkpgdakpekjlhemmfcncgdjeiika" # Absolute Enable Right Click & Copy
        "djflhoibgkdhkhhcedjiklpkjnoahfmg" # User-Agent Switcher
        "lfhmikememgdcahcdlaciloancbhjino" # CORS Unblock
        "ipdjnhgkpapgippgcgkfcbpdpcgifncb" # Emoji Keyboard
        "jjdalbijcodgjpeepgllndclalllbopg" # Tabboo
      ];
    };

    obs-studio = {
      enable = true;
      plugins = with pkgs.obs-studio-plugins; [
        obs-3d-effect
      ];
    };
  };
}
