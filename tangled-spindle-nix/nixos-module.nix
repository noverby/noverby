# NixOS module: services.tangled-spindles
#
# Declarative multi-runner deployment for tangled-spindle-nix,
# modeled after services.github-runners.
#
# See PLAN.md Phase 7 for the full specification.
#
# Usage:
#   services.tangled-spindles = {
#     runner1 = {
#       enable = true;
#       hostname = "spindle1.example.com";
#       owner = "did:plc:abc123";
#       tokenFile = "/run/secrets/spindle1-token";
#     };
#   };
{
  config,
  lib,
  pkgs,
  ...
}: let
  cfg = config.services.tangled-spindles;

  # Per-runner submodule options
  runnerOpts = {name, ...}: {
    options = {
      enable = lib.mkEnableOption "tangled-spindle runner '${name}'";

      package = lib.mkOption {
        type = lib.types.package;
        default = pkgs.tangled-spindle-nix or (throw "tangled-spindle-nix package not found in pkgs; add the overlay or pass it explicitly");
        description = "The tangled-spindle-nix package to use.";
      };

      hostname = lib.mkOption {
        type = lib.types.str;
        description = "Public hostname of this spindle instance.";
      };

      owner = lib.mkOption {
        type = lib.types.str;
        description = "DID of the spindle owner (e.g. did:plc:abc123).";
      };

      tokenFile = lib.mkOption {
        type = lib.types.str;
        description = "Path to the authentication token file.";
      };

      listenAddr = lib.mkOption {
        type = lib.types.str;
        default = "127.0.0.1:6555";
        description = "Address the HTTP server binds to.";
      };

      jetstreamEndpoint = lib.mkOption {
        type = lib.types.str;
        default = "wss://jetstream1.us-west.bsky.network/subscribe";
        description = "Jetstream WebSocket endpoint URL.";
      };

      plcUrl = lib.mkOption {
        type = lib.types.str;
        default = "https://plc.directory";
        description = "PLC directory URL for DID resolution.";
      };

      dbPath = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "Path to the SQLite database. Defaults to StateDirectory.";
      };

      logDir = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "Directory for workflow log files. Defaults to LogsDirectory.";
      };

      dev = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Enable development mode (HTTP instead of HTTPS, localhost remapping).";
      };

      engine = {
        maxJobs = lib.mkOption {
          type = lib.types.int;
          default = 2;
          description = "Maximum concurrent workflow executions.";
        };

        queueSize = lib.mkOption {
          type = lib.types.int;
          default = 100;
          description = "Maximum pending jobs in queue.";
        };

        workflowTimeout = lib.mkOption {
          type = lib.types.str;
          default = "5m";
          description = "Maximum duration for a single workflow execution.";
        };

        nixery = lib.mkOption {
          type = lib.types.str;
          default = "nixery.tangled.sh";
          description = "Nixery URL (for compatibility / fallback).";
        };

        extraNixFlags = lib.mkOption {
          type = lib.types.listOf lib.types.str;
          default = [];
          description = "Extra flags passed to nix build.";
        };
      };

      secrets = {
        provider = lib.mkOption {
          type = lib.types.enum ["sqlite" "openbao"];
          default = "sqlite";
          description = "Secrets storage backend.";
        };

        openbao = {
          proxyAddr = lib.mkOption {
            type = lib.types.str;
            default = "http://127.0.0.1:8200";
            description = "OpenBao proxy address.";
          };

          mount = lib.mkOption {
            type = lib.types.str;
            default = "spindle";
            description = "OpenBao KV v2 mount path.";
          };
        };
      };

      extraEnvironment = lib.mkOption {
        type = lib.types.attrsOf lib.types.str;
        default = {};
        description = "Additional environment variables for the service.";
      };

      extraPackages = lib.mkOption {
        type = lib.types.listOf lib.types.package;
        default = [];
        description = "Extra packages to include in PATH.";
      };

      serviceOverrides = lib.mkOption {
        type = lib.types.attrs;
        default = {};
        description = "Override systemd service options.";
      };

      user = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "User to run the service as. Null uses DynamicUser.";
      };

      group = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "Group to run the service as. Null uses DynamicUser.";
      };
    };
  };
in {
  options.services.tangled-spindles = lib.mkOption {
    type = lib.types.attrsOf (lib.types.submodule runnerOpts);
    default = {};
    description = ''
      Declarative tangled-spindle-nix runner instances.
      Each attribute defines an independent runner with its own systemd service,
      state directory, log directory, database, and RBAC configuration.
    '';
  };

  config = let
    enabledRunners = lib.filterAttrs (_: runner: runner.enable) cfg;
  in
    lib.mkIf (enabledRunners != {}) {
      # TODO (Phase 7): Generate systemd services for each enabled runner.
      # See PLAN.md Phase 7 for the full systemd service specification including:
      # - ExecStart, Environment mapping
      # - StateDirectory, LogsDirectory, RuntimeDirectory
      # - Full systemd sandboxing (DynamicUser, ProtectSystem, PrivateTmp, etc.)
      # - Resource limits (CPUQuota, MemoryMax, TasksMax)
      # - Token file handling
      # - PATH with bash, coreutils, git, gnutar, gzip, nix, extraPackages
      # - Restart policy, After/Wants dependencies
    };
}
