{
  config,
  pkgs,
  lib,
  ...
}: let
  cfg = config.services.wiki-auth;
in {
  options.services.wiki-auth = {
    enable = lib.mkEnableOption "RadikalWiki auth webhook";

    port = lib.mkOption {
      type = lib.types.port;
      default = 4180;
      description = "Port for the auth webhook server to listen on.";
    };

    nhostSubdomain = lib.mkOption {
      type = lib.types.str;
      description = "NHost project subdomain for JWT validation.";
    };

    nhostRegion = lib.mkOption {
      type = lib.types.str;
      description = "NHost project region for JWT validation.";
    };

    hasuraEndpoint = lib.mkOption {
      type = lib.types.str;
      description = "Hasura GraphQL endpoint URL for user management.";
    };

    environmentFile = lib.mkOption {
      type = lib.types.path;
      description = ''
        Path to an environment file containing secrets.
        Must define at minimum:
          HASURA_ADMIN_SECRET=<secret>
      '';
    };

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.deno;
      defaultText = lib.literalExpression "pkgs.deno";
      description = "The Deno package to use for running the server.";
    };

    serverDir = lib.mkOption {
      type = lib.types.path;
      description = "Path to the wiki/server directory containing main.ts.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.wiki-auth = {
      description = "RadikalWiki Auth Webhook";
      after = ["network.target"];
      wantedBy = ["multi-user.target"];

      serviceConfig = {
        ExecStart = "${cfg.package}/bin/deno run --allow-net --allow-env ${cfg.serverDir}/main.ts";
        EnvironmentFile = cfg.environmentFile;
        Environment = [
          "PORT=${toString cfg.port}"
          "NHOST_SUBDOMAIN=${cfg.nhostSubdomain}"
          "NHOST_REGION=${cfg.nhostRegion}"
          "HASURA_ENDPOINT=${cfg.hasuraEndpoint}"
        ];
        DynamicUser = true;
        Restart = "on-failure";
        RestartSec = 5;
        # Hardening
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictSUIDSGID = true;
      };
    };
  };
}
