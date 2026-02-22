# Git commit rules

- **Never use `git commit --amend` after a pre-commit hook failure.** A failed commit does not land — just fix the issue and run `git commit` again with the same message. Using `--amend` will squash over the previous unrelated commit and destroy history.
- Only use `git commit --amend` when explicitly asked to amend, or when fixing the *current* (most recent, already landed) commit intentionally.

## Nix flake rules

- **Always `git add` new or changed files before any Nix flake operation.** Nix flakes only see files tracked by git. This applies to `direnv reload`, `nix develop`, `nix build`, `nixos-rebuild`, and any other command that evaluates the flake. If you create or modify a file referenced by Nix (e.g. in `modules/`, `config/`) and don't stage it first, the flake will use the old version or fail to find it.
- **Run `direnv reload` after changing devenv modules or configs.** Files in `modules/devenv/` and `config/devenv.nix` are evaluated by devenv on shell entry. Changes to these files (e.g. `enterShell`, git-hooks, packages) won't take effect until you `git add` the changed files and run `direnv reload`.