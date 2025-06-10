{...}: let
  userKeymaps = builtins.fromJSON (builtins.readFile ./keymap.json);
  userSettings = {
    assistant = {
      default_model = {
        model = "claude-3-5-sonnet-20240620";
        provider = "zed.dev";
      };
      version = "2";
    };
    base_keymap = "VSCode";
    buffer_font_family = "Cascadia Code NF";
    inlay_hints = {
      enabled = true;
    };
    show_whitespaces = "all";
    languages = {
      Nix = {
        language_servers = [
          "nil"
          "!nix-ls"
          "..."
        ];
        format_on_save = {
          external = {
            command = "alejandra";
            arguments = [];
          };
        };
      };
      JavaScript = {
        language_servers = [
          "vtsls"
        ];
      };
      TypeScript = {
        language_servers = [
          "vtsls"
        ];
        formatter = {
          language_server = {
            name = "biome";
          };
        };
      };
      TSX = {
        language_servers = [
          "vtsls"
        ];
      };
    };
    load_direnv = "direct";
    lsp = {
      biome = {
        settings = {
          require_config_file = true;
        };
      };
      rust-analyzer = {
        binary = {
          path_lookup = true;
        };
      };
      vtsls = {
        initialization_options = {
          typescript = {
            tsdk = ".yarn/sdks/typescript/lib";
          };
          vtsls = {
            autoUseWorkspaceTsdk = true;
          };
        };
      };
      nil = {
        binary = {
          path = "nil";
        };
      };
    };
    tabs = {
      file_icons = true;
      git_status = true;
    };
    indent_guides = {
      enabled = true;
      coloring = "indent_aware";
    };
    tab_size = 2;
    relative_line_numbers = true;
    terminal = {
      shell = {
        program = "zellij-cwd";
      };
      line_height = "standard";
      font_size = 11;
      toolbar = {
        breadcrumbs = true;
      };
    };
    autosave = {
      after_delay = {
        milliseconds = 1000;
      };
    };
    auto_update = false;
    ui_font_size = 14;
    buffer_font_size = 12;
    # allow cursor to reach edges of screen
    vertical_scroll_margin = 10;
    vim = {
      # Lets `f` and `t` motions extend across multiple lines
      use_multiline_find = true;
      use_smartcase_find = true;
      # "always": use system clipboard
      # "never": don't use system clipboard
      # "on_yank": use system clipboard for yank operations
      use_system_clipboard = "always";
    };
    vim_mode = true;
    file_types = {
      "Shell Script" = ["\.envrc"];
    };
  };
in {
  programs.zed-editor = {
    enable = true;
    inherit userKeymaps userSettings;
  };
}
