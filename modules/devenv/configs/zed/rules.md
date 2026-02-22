# Git commit rules

- **Never use `git commit --amend` after a pre-commit hook failure.** A failed commit does not land — just fix the issue and run `git commit` again with the same message. Using `--amend` will squash over the previous unrelated commit and destroy history.
- Only use `git commit --amend` when explicitly asked to amend, or when fixing the *current* (most recent, already landed) commit intentionally.

## Nix flake rules

- **Always `git add` new or changed files before running `direnv reload` or `nix develop`.** Nix flakes only see files tracked by git. If you create or modify a file referenced by Nix (e.g. in `modules/`, `config/`) and don't stage it first, the flake will use the old version or fail to find it.