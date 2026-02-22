lint *hook:
    prek run {{ if hook == "" { "--all-files" } else { hook + " --all-files" } }}

flake-update:
    nix flake update --option access-tokens "github.com=$(gh auth token)"
