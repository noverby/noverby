{...}: let
  userKeymaps = {
    keyBindings = [
      {
        bindings = {
          "ctrl-j" = "editor::ContextMenuNext";
          "ctrl-k" = "editor::ContextMenuPrev";
          "shift-tab" = "editor::ContextMenuPrev";
          "tab" = "editor::ContextMenuNext";
        };
        context = "Editor && (showing_code_actions || showing_completions)";
      }
      {
        bindings = {
          "b" = ["workspace::SendKeystrokes" "v n"];
          "ctrl-j" = "editor::MoveLineDown";
          # Misc
          # "shift-k" = "editor::MoveLineUp";
          # "shift-j" = "editor::MoveLineDown";
          "ctrl-k" = "editor::MoveLineUp";
          "ctrl-v" = "editor::Paste";
          "d" = "vim::DeleteRight";
          "g b" = "vim::WindowBottom";
          "g c" = "vim::WindowMiddle";
          "g e" = "vim::EndOfDocument";
          "g h" = "vim::StartOfLine";
          "g l" = "vim::EndOfLine";
          # Goto mode
          "g n" = "pane::ActivateNextItem";
          "g p" = "pane::ActivatePrevItem";
          "g r" = "editor::FindAllReferences"; # zed specific
          "g s" = "vim::FirstNonWhitespace"; # "g s" default behavior is "space s"
          "g t" = "vim::WindowTop";
          "g y" = "editor::GoToTypeDefinition";
          # "tab" = "pane::ActivateNextItem";
          # "shift-tab" = "pane::ActivatePrevItem";
          "H" = "pane::ActivatePrevItem";
          "L" = "pane::ActivateNextItem";
          "m i w" = ["workspace::SendKeystrokes" "v i w"];
          # Match mode
          "m m" = "vim::Matching";
          "n" = "vim::PreviousWordStart";
          "shift-u" = "editor::Redo";
          "space a" = "editor::ToggleCodeActions";
          "space c" = "pane::CloseActiveItem";
          "space d" = "editor::GoToDiagnostic";
          # Space mode
          "space f" = "file_finder::Toggle";
          "space h" = "editor::SelectAllMatches";
          "space k" = "editor::Hover";
          "space r" = "editor::Rename";
          "space s" = "outline::Toggle";
          "space shift-d" = "diagnostics::Deploy";
          "space shift-s" = "project_symbols::Toggle";
          "space w d" = "pane::SplitDown";
          # Window mode
          "space w h" = ["workspace::ActivatePaneInDirection" "Left"];
          "space w j" = ["workspace::ActivatePaneInDirection" "Down"];
          "space w k" = ["workspace::ActivatePaneInDirection" "Up"];
          "space w l" = ["workspace::ActivatePaneInDirection" "Right"];
          "space w q" = "pane::CloseActiveItem";
          "space w r" = "pane::SplitRight";
          "space w s" = "pane::SplitRight";
          "space w v" = "pane::SplitDown";
          "w" = ["workspace::SendKeystrokes" "v e"];
          "x" = "editor::SelectLine";
        };
        context = "Editor && VimControl && !VimWaiting && !menu";
      }
      {
        bindings = {
          # put key-bindings here if you want them to work in normal & visual mode
        };
        context = "Editor && (vim_mode == normal || vim_mode == visual) && !VimWaiting && !menu";
      }
      {
        bindings = {
          # put key-bindings here if you want them to work only in normal mode
          "down" = ["workspace::SendKeystrokes" "4 j"];
          "up" = ["workspace::SendKeystrokes" "4 k"];
        };
        context = "Editor && vim_mode == normal && !VimWaiting && !menu";
      }
      {
        bindings = {
          # visual, visual line & visual block modes
        };
        context = "Editor && vim_mode == visual && !VimWaiting && !menu";
      }
      {
        # put key-bindings here if you want them to work in insert mode
        bindings = {
          "j k" = ["vim::SwitchMode" "Normal"];
        };
        context = "Editor && vim_mode == insert && !menu";
      }
      {
        bindings = {
          # Window mode
          "ctrl-w h" = ["workspace::ActivatePaneInDirection" "Left"];
          "ctrl-w j" = ["workspace::ActivatePaneInDirection" "Down"];
          "ctrl-w k" = ["workspace::ActivatePaneInDirection" "Up"];
          "ctrl-w l" = ["workspace::ActivatePaneInDirection" "Right"];
        };
        context = "Dock";
      }
      {
        bindings = {
          "shift-ctrl-alt-cmd-enter" = "assistant::Assist";
        };
        context = "ConversationEditor > Editor";
      }
    ];
  };
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
    };
    autosave = {
      after_delay = {
        milliseconds = 1000;
      };
    };
    auto_update = false;
    ui_font_size = 16;
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
  };
in {
  programs.zed-editor = {
    enable = true;
    inherit userKeymaps userSettings;
  };
}
