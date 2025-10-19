{
  inputs,
  pkgs,
  username,
  ...
}: {
  imports = [
    ./git.nix
    ./nushell
    ./zed-editor
    ./zen-browser
    ./vscode
  ];

  programs = {
    home-manager.enable = true;
    gh.enable = true;
    tealdeer.enable = true;
    bottom.enable = true;
    jujutsu.enable = true;
    mergiraf.enable = true;

    delta = {
      enable = true;
      enableGitIntegration = true;
      options = {
        side-by-side = true;
      };
    };

    bat = {
      enable = true;
      extraPackages = with pkgs.bat-extras; [
        prettybat
        batgrep
        batdiff
      ];
    };

    carapace = {
      enable = true;
    };

    television = {
      enable = true;
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
      historyControl = [
        "ignoredups"
        "erasedups"
      ];
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
        show_startup_tips = false;
        env = {
          TERM = "tmux-256color";
        };
      };
    };

    starship = {
      enable = true;
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
      nix-direnv.enable = true;
      silent = true;
    };

    zoxide = {
      enable = true;
    };

    nix-index = {
      enable = true;
    };

    atuin = {
      enable = true;
      settings = {
        inline_height = 10;
      };
    };

    ssh = {
      enable = true;
      enableDefaultConfig = false;
      matchBlocks = {
        "*" = {
          addKeysToAgent = "yes";
          controlMaster = "auto";
          controlPath = "~/.ssh/socket/%r@%h:%p";
          controlPersist = "120";
          forwardAgent = true;
        };
        localhost = {
          hostname = "localhost";
          user = username;
        };
      };
    };

    obs-studio = {
      enable = true;
      plugins = with pkgs.obs-studio-plugins; [
        obs-3d-effect
      ];
    };

    spicetify = let
      spicePkgs = inputs.spicetify-nix.legacyPackages.${pkgs.stdenv.system};
    in {
      enable = true;
      theme = spicePkgs.themes.catppuccin;
      colorScheme = "mocha";
    };
  };
}
