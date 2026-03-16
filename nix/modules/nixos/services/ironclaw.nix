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
  # URL-encode the database host so Unix socket paths (/run/postgresql)
  # become valid URL components (%2Frun%2Fpostgresql).
  # Double the % signs so systemd doesn't interpret them as specifiers.
  urlEncodeHost = host:
    builtins.replaceStrings ["/"] ["%%2F"] host;

  # Build a channels-src directory with patched capabilities configs.
  matrixConfigJson = builtins.toJSON {
    inherit (cfg.matrix) homeserver;
    dm_policy = cfg.matrix.dmPolicy;
    allow_from = cfg.matrix.allowFrom;
    room_ids = cfg.matrix.roomIds;
    require_mention = cfg.matrix.requireMention;
  };

  blueskyConfigJson = builtins.toJSON {
    pds_url = cfg.bluesky.pdsUrl;
    dm_policy = cfg.bluesky.dmPolicy;
    allow_from = cfg.bluesky.allowFrom;
    respond_to_mentions = cfg.bluesky.respondToMentions;
  };

  channelsSrc = let
    baseSrc = "${cfg.package}/share/ironclaw/channels-src";
  in
    pkgs.runCommand "ironclaw-channels-src" {
      nativeBuildInputs = [pkgs.jq];
    } ''
      cp -r --no-preserve=mode ${baseSrc} $out
      jq --argjson cfg '${matrixConfigJson}' '.config = $cfg' \
        ${baseSrc}/matrix/matrix.capabilities.json \
        > $out/matrix/matrix.capabilities.json
      jq --argjson cfg '${blueskyConfigJson}' '.config = $cfg' \
        ${baseSrc}/bluesky/bluesky.capabilities.json \
        > $out/bluesky/bluesky.capabilities.json
    '';
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

    matrix = {
      homeserver = lib.mkOption {
        type = lib.types.str;
        default = "https://matrix.org";
        description = "Matrix homeserver base URL.";
      };

      dmPolicy = lib.mkOption {
        type = lib.types.enum ["pairing" "allowlist" "open"];
        default = "pairing";
        description = ''
          DM access control policy.
          - pairing: require mutual pairing approval
          - allowlist: only allow users in allowFrom
          - open: accept DMs from anyone
        '';
      };

      allowFrom = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [];
        description = "Matrix user IDs allowed to message the bot (used with allowlist/pairing policies).";
      };

      roomIds = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [];
        description = "Matrix room IDs to join and monitor. Empty means DM-only.";
      };

      requireMention = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Whether the bot requires an @-mention in rooms to respond.";
      };
    };

    bluesky = {
      pdsUrl = lib.mkOption {
        type = lib.types.str;
        default = "https://bsky.social";
        description = "AT Protocol PDS URL.";
      };

      dmPolicy = lib.mkOption {
        type = lib.types.enum ["pairing" "allowlist" "open"];
        default = "pairing";
        description = ''
          DM access control policy.
          - pairing: require mutual pairing approval
          - allowlist: only allow DIDs in allowFrom
          - open: accept DMs from anyone
        '';
      };

      allowFrom = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        default = [];
        description = "DIDs or handles allowed to message the bot (used with allowlist/pairing policies).";
      };

      respondToMentions = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Whether the bot responds to @-mentions on Bluesky posts.";
      };
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
        # Wait for ensureDatabases (runs in postgresql postStart) to create the DB
        while ! ${config.services.postgresql.package}/bin/psql \
          -d ${lib.escapeShellArg cfg.database.name} -c "SELECT 1" &>/dev/null; do
          sleep 1
        done
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
        DATABASE_URL = "postgres://${cfg.user}@${urlEncodeHost cfg.database.host}/${cfg.database.name}";
        DATABASE_SSLMODE = "disable";
        IRONCLAW_HOME = cfg.dataDir;
        IRONCLAW_CHANNELS_SRC = "${channelsSrc}";
        ONBOARD_COMPLETED = "true";
      };

      serviceConfig = {
        Type = "simple";
        User = cfg.user;
        Group = cfg.group;
        WorkingDirectory = cfg.dataDir;
        StateDirectory = "ironclaw";
        ExecStart = let
          args = lib.concatStringsSep " " (["--no-onboard"] ++ cfg.extraArgs);
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
