$env.config = {
  show_banner: false
  keybindings: []
}
$env.PATH = ($env.PATH | split row (char esep))

def ghash [] {git rev-parse HEAD | tr -d '\\n' | wl-copy; git rev-parse HEAD}

def ggg [] { 
  git push -f
  gh pr create --fill
  gh pr comment --body 'bors merge' 
}

def --env assume [profile?: string = ""] {
  let granted = assumego $profile | split row " "
  load-env {
    AWS_ACCESS_KEY_ID: $granted.1,
    AWS_SECRET_ACCESS_KEY: $granted.2,
    AWS_SESSION_TOKEN: $granted.3,
    AWS_PROFILE: $granted.4,
    AWS_REGION: $granted.5,
    AWS_SESSION_EXPIRATION: $granted.6,
    AWS_CREDENTIAL_EXPIRATION: $granted.6,
    GRANTED_SSO: $granted.7,
    GRANTED_SSO_START_URL: $granted.8,
    GRANTED_SSO_ROLE_NAME: $granted.9,
    GRANTED_SSO_REGION: $granted.10,
    GRANTED_SSO_ACCOUNT_ID: $granted.11,
  }
 }

def yarn-lock-update [] {
  try { git rebase master }
  let root = git rev-parse --show-toplevel
  git reset $"($root)/.pnp.cjs" $"($root)/yarn.lock"
  yarn
  git add $"($root)/.pnp.cjs" $"($root)/yarn.lock"
}

def gco [branch_name: string] {
    match (git show-ref --quiet $"refs/heads/($branch_name)" | complete | get exit_code) {
     0 => { git checkout $branch_name }
     1 =>  { git checkout -b $branch_name }
    }
}
