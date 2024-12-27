$env.config = {
  edit_mode: vi
  show_banner: false
  keybindings: []
}
$env.PATH = ($env.PATH | split row (char esep))

def --env uo [] { let res = uf | $in; cd $res }

def ghash [] {git rev-parse HEAD | tr -d '\\n' | wl-copy; git rev-parse HEAD}

def ggg [] {
  git push -f
  gh pr create --fill
  gh pr comment --body 'bors merge'
}

def bin64 [] {
  xxd -r -p | base64 -w 0
}

def unbin64 [] {
  base64 -d | xxd -p -c 0
}

def --env assume [profile?: string = ""] {
  let granted_output = assumego $profile
  let granted = $granted_output | lines | get -i 1 | split row " "
  load-env {
    AWS_ACCESS_KEY_ID: $granted.1,
    AWS_SECRET_ACCESS_KEY: $granted.2,
    AWS_SESSION_TOKEN: $granted.3,
    AWS_PROFILE: $granted.4,
    AWS_REGION: $granted.5,
    AWS_DEFAULT_REGION: $granted.5,
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
    git fetch origin

    let local_exists = (git show-ref --quiet $"refs/heads/($branch_name)" | complete | get exit_code) == 0
    let remote_exists = (git show-ref --quiet $"refs/remotes/origin/($branch_name)" | complete | get exit_code) == 0

    if $local_exists {
        git checkout $branch_name
    } else if $remote_exists {
        git checkout -b $branch_name --track $"origin/($branch_name)"
    } else {
        git checkout -b $branch_name
    }
}
