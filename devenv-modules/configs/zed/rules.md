# Version control rules

- **Use `jj` (Jujutsu) instead of `git` for all version control operations.** This project uses Jujutsu as its VCS. Common mappings:
  - `jj status` — show working copy status
  - `jj log` — show commit history
  - `jj diff` — show changes
  - `jj new` — create a new change (like finishing the current commit)
  - `jj commit -m "msg"` — set description and create a new change on top
  - `jj describe -m "msg"` — update the current change's description
  - `jj bookmark` — manage bookmarks (branches)
  - `jj git push` — push to remote
  - `jj git fetch` — fetch from remote

## Commit message rules

- **Follow `.commitlintrc.yml` for commit message format.** Before committing, read `.commitlintrc.yml` and ensure the commit message conforms to its rules.

- **The `jj` wrapper runs git hooks automatically.** The `jj` binary is wrapped to run `pre-commit` hooks before `jj commit`, `jj new`, and `jj squash`, and `prepare-commit-msg` hooks when `-m`/`--message` is provided. If a hook fails, fix the issue and run the command again — do not try to bypass hooks.

## Nix flake rules

- **Run any `jj` command (e.g. `jj status`) before Nix flake operations when you've created new files.** Nix flakes only see files tracked by git. In a jj colocated repo, jj automatically snapshots the working directory (updating the git index) on every `jj` command. Unlike plain git, you do NOT need to manually `git add` files — just ensure at least one `jj` command has run since creating the file.
- **Run `touch .envrc && direnv export json` after changing devenv modules or configs.** Files in `devenv-modules/` and `devenv-modules/configs/` are evaluated by devenv on shell entry. Changes to these files (e.g. `enterShell`, git-hooks, packages) won't take effect until you run `touch .envrc && direnv export json`. Note: `direnv reload` only touches `.envrc` and defers to a shell prompt hook that doesn't fire in non-interactive contexts. `direnv export json` directly triggers the full re-evaluation.