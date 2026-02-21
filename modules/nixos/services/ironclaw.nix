# IronClaw – secure personal AI assistant
#
# NixOS service module that manages:
#   • PostgreSQL with pgvector extension
#   • ironclaw systemd service
#   • environment file for secrets (API keys, DATABASE_URL, etc.)
#
# After first deployment run `ironclaw onboard` interactively on the device
# to complete NEAR AI authentication and secrets encryption setup, or
# pre-populate the environment file managed by agenix / manually.
{
  config,
  lib,
  pkgs,
  ...
}: let
  cfg = config.services.ironclaw;
in {
  options.services.ironclaw = {
    enable = lib.mkEnableOption "IronClaw AI assistant";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.ironclaw;
      defaultText = lib.literalExpression "pkgs.ironclaw";
      description = "The IronClaw package to use.";
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "ironclaw";
      description = "System user under which IronClaw runs.";
    };

    group = lib.mkOption {
      type = lib.types.str;
      default = "ironclaw";
      description = "System group under which IronClaw runs.";
    };

    dataDir = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/ironclaw";
      description = "Directory for IronClaw persistent state.";
    };

    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        Path to an environment file loaded by the systemd unit.
        Use this to supply secrets such as API keys without putting
        them in the Nix store.  Expected variables include at minimum:

          LLM_BACKEND=nearai
          NEARAI_API_KEY=<your-key>

        When using agenix, point this at the decrypted secret path.
      '';
    };

    database = {
      name = lib.mkOption {
        type = lib.types.str;
        default = "ironclaw";
        description = "PostgreSQL database name.";
      };

      host = lib.mkOption {
        type = lib.types.str;
        default = "/run/postgresql";
        description = "PostgreSQL host (use a directory path for Unix socket).";
      };

      createLocally = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Whether to create the database and enable PostgreSQL locally.";
      };
    };

    logLevel = lib.mkOption {
      type = lib.types.str;
      default = "ironclaw=info";
      description = "RUST_LOG value for the IronClaw service.";
    };

    extraArgs = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [];
      description = "Extra command-line arguments passed to the ironclaw binary.";
    };
  };

  config = lib.mkIf cfg.enable {
    # ── PostgreSQL with pgvector ─────────────────────────────────────────
    services.postgresql = lib.mkIf cfg.database.createLocally {
      enable = true;
      extensions = ps: [ps.pgvector];
      ensureDatabases = [cfg.database.name];
      ensureUsers = [
        {
          name = cfg.user;
          ensureDBOwnership = true;
        }
      ];
      # pgvector must be created inside the target database; the
      # `ensureExtensions` mechanism is not yet upstream, so we use
      # an initialScript instead.
      settings = {
        shared_preload_libraries = "vector";
      };
    };

    # Create the pgvector extension in the ironclaw database after PG starts.
    systemd.services.ironclaw-db-setup = lib.mkIf cfg.database.createLocally {
      description = "IronClaw database schema bootstrap (pgvector)";
      after = ["postgresql.service"];
      requires = ["postgresql.service"];
      wantedBy = ["ironclaw.service"];
      before = ["ironclaw.service"];
      serviceConfig = {
        Type = "oneshot";
        User = "postgres";
        Group = "postgres";
        RemainAfterExit = true;
      };
      script = ''
        ${config.services.postgresql.package}/bin/psql \
          -d ${lib.escapeShellArg cfg.database.name} \
          -c "CREATE EXTENSION IF NOT EXISTS vector;"
      '';
    };

    # ── System user / group ──────────────────────────────────────────────
    users.users.${cfg.user} = {
      isSystemUser = true;
      inherit (cfg) group;
      home = cfg.dataDir;
      createHome = true;
      description = "IronClaw service user";
    };

    users.groups.${cfg.group} = {};

    # ── Systemd service ──────────────────────────────────────────────────
    systemd.services.ironclaw = {
      description = "IronClaw AI Assistant";
      documentation = ["https://github.com/nearai/ironclaw"];

      after =
        ["network-online.target"]
        ++ lib.optionals cfg.database.createLocally [
          "postgresql.service"
          "ironclaw-db-setup.service"
        ];
      requires = lib.optionals cfg.database.createLocally [
        "postgresql.service"
        "ironclaw-db-setup.service"
      ];
      wants = ["network-online.target"];
      wantedBy = ["multi-user.target"];

      environment = {
        RUST_LOG = cfg.logLevel;
        DATABASE_URL = "postgres://${cfg.user}@${cfg.database.host}/${cfg.database.name}";
        IRONCLAW_HOME = cfg.dataDir;
        IRONCLAW_CHANNELS_SRC = "${cfg.package}/share/ironclaw/channels-src";
      };

      serviceConfig = {
        Type = "simple";
        User = cfg.user;
        Group = cfg.group;
        WorkingDirectory = cfg.dataDir;
        StateDirectory = "ironclaw";
        ExecStart = let
          args = lib.concatStringsSep " " cfg.extraArgs;
        in "${cfg.package}/bin/ironclaw ${args}";

        Restart = "on-failure";
        RestartSec = 10;

        # Load secrets from the environment file
        EnvironmentFile = lib.mkIf (cfg.environmentFile != null) cfg.environmentFile;

        # Hardening
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        ReadWritePaths = [cfg.dataDir];
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictSUIDSGID = true;
        MemoryDenyWriteExecute = false; # WASM JIT needs W^X
      };
    };

    # Make the CLI available system-wide for manual `ironclaw onboard`, etc.
    environment.systemPackages = [cfg.package];
  };
}
