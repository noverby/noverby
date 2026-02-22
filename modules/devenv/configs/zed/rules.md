# Git commit rules

- **Never use `git commit --amend` after a pre-commit hook failure.** A failed commit does not land — just fix the issue and run `git commit` again with the same message. Using `--amend` will squash over the previous unrelated commit and destroy history.
- Only use `git commit --amend` when explicitly asked to amend, or when fixing the *current* (most recent, already landed) commit intentionally.