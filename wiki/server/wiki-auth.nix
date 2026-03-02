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

    publicUrl = lib.mkOption {
      type = lib.types.str;
      description = "Public URL of the auth webhook (for email verification links).";
      default = "https://wiki-auth.overby.me";
    };

    wikiUrl = lib.mkOption {
      type = lib.types.str;
      description = "Public URL of the wiki frontend (for redirects after verification).";
      default = "https://radikal.wiki";
    };

    smtpHost = lib.mkOption {
      type = lib.types.str;
      default = "localhost";
      description = "SMTP server hostname for sending verification emails.";
    };

    smtpPort = lib.mkOption {
      type = lib.types.port;
      default = 25;
      description = "SMTP server port.";
    };

    smtpFrom = lib.mkOption {
      type = lib.types.str;
      default = "noreply@overby.me";
      description = "Sender address for verification emails.";
    };

    smtpSecure = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Use direct TLS (SMTPS) instead of STARTTLS.";
    };

    environmentFile = lib.mkOption {
      type = lib.types.path;
      description = ''
        Path to an environment file containing secrets.
        Must define at minimum:
          HASURA_ADMIN_SECRET=<secret>
        Optionally:
          SMTP_USER=<user>
          SMTP_PASS=<password>
          EMAIL_SECRET=<secret>
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
          "PUBLIC_URL=${cfg.publicUrl}"
          "WIKI_URL=${cfg.wikiUrl}"
          "SMTP_HOST=${cfg.smtpHost}"
          "SMTP_PORT=${toString cfg.smtpPort}"
          "SMTP_FROM=${cfg.smtpFrom}"
          "SMTP_SECURE=${
            if cfg.smtpSecure
            then "true"
            else "false"
          }"
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
