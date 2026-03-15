use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug, Clone)]
#[command(name = "tangled", author, version, about = "Tangled CLI", long_about = None)]
pub struct Cli {
    /// Config file path override
    #[arg(long, global = true)]
    pub config: Option<String>,

    /// Use named profile
    #[arg(long, global = true)]
    pub profile: Option<String>,

    /// Output format
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,

    /// Verbose output
    #[arg(long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Quiet output
    #[arg(long, global = true, default_value_t = false)]
    pub quiet: bool,

    /// Disable colors
    #[arg(long, global = true, default_value_t = false)]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    Json,
    Table,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Authentication commands
    #[command(subcommand)]
    Auth(AuthCommand),
    /// Repository commands
    #[command(subcommand)]
    Repo(RepoCommand),
    /// Issue commands
    #[command(subcommand)]
    Issue(IssueCommand),
    /// Pull request commands
    #[command(subcommand)]
    Pr(PrCommand),
    /// Knot management commands
    #[command(subcommand)]
    Knot(KnotCommand),
    /// Spindle integration commands
    #[command(subcommand)]
    Spindle(SpindleCommand),
}

#[derive(Subcommand, Debug, Clone)]
pub enum AuthCommand {
    /// Login with Bluesky credentials
    Login(AuthLoginArgs),
    /// Login via browser (OAuth)
    LoginBrowser(AuthLoginBrowserArgs),
    /// Show authentication status
    Status,
    /// Logout and clear session
    Logout,
}

#[derive(Args, Debug, Clone)]
pub struct AuthLoginArgs {
    /// Bluesky handle (e.g. user.bsky.social)
    #[arg(long)]
    pub handle: Option<String>,
    /// Password (will prompt if omitted)
    #[arg(long)]
    pub password: Option<String>,
    /// PDS URL (default: https://bsky.social)
    #[arg(long)]
    pub pds: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct AuthLoginBrowserArgs {
    /// Bluesky handle or PDS URL (defaults to https://bsky.social)
    #[arg(long)]
    pub handle: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum RepoCommand {
    /// List repositories
    List(RepoListArgs),
    /// Create repository
    Create(RepoCreateArgs),
    /// Clone repository
    Clone(RepoCloneArgs),
    /// Show repository information
    Info(RepoInfoArgs),
    /// Delete a repository
    Delete(RepoDeleteArgs),
    /// Star a repository
    Star(RepoRefArgs),
    /// Unstar a repository
    Unstar(RepoRefArgs),
}

#[derive(Args, Debug, Clone)]
pub struct RepoListArgs {
    #[arg(long)]
    pub knot: Option<String>,
    #[arg(long)]
    pub user: Option<String>,
    #[arg(long, default_value_t = false)]
    pub starred: bool,
    /// Tangled API base URL (overrides env)
    #[arg(long)]
    pub base: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct RepoCreateArgs {
    pub name: String,
    #[arg(long)]
    pub knot: Option<String>,
    #[arg(long, default_value_t = false)]
    pub private: bool,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long, default_value_t = false)]
    pub init: bool,
}

#[derive(Args, Debug, Clone)]
pub struct RepoCloneArgs {
    pub repo: String,
    #[arg(long, default_value_t = false)]
    pub https: bool,
    #[arg(long)]
    pub depth: Option<usize>,
}

#[derive(Args, Debug, Clone)]
pub struct RepoInfoArgs {
    pub repo: String,
    #[arg(long, default_value_t = false)]
    pub stats: bool,
    #[arg(long, default_value_t = false)]
    pub contributors: bool,
}

#[derive(Args, Debug, Clone)]
pub struct RepoDeleteArgs {
    pub repo: String,
    #[arg(long, default_value_t = false)]
    pub force: bool,
}

#[derive(Args, Debug, Clone)]
pub struct RepoRefArgs {
    pub repo: String,
}

#[derive(Subcommand, Debug, Clone)]
pub enum IssueCommand {
    List(IssueListArgs),
    Create(IssueCreateArgs),
    Show(IssueShowArgs),
    Edit(IssueEditArgs),
    Comment(IssueCommentArgs),
}

#[derive(Args, Debug, Clone)]
pub struct IssueListArgs {
    #[arg(long)]
    pub repo: Option<String>,
    #[arg(long)]
    pub state: Option<String>,
    #[arg(long)]
    pub author: Option<String>,
    #[arg(long)]
    pub label: Option<String>,
    #[arg(long)]
    pub assigned: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct IssueCreateArgs {
    #[arg(long)]
    pub repo: Option<String>,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub body: Option<String>,
    #[arg(long)]
    pub label: Option<Vec<String>>,
    #[arg(long, value_name = "HANDLE")]
    pub assign: Option<Vec<String>>,
}

#[derive(Args, Debug, Clone)]
pub struct IssueShowArgs {
    pub id: String,
    #[arg(long, default_value_t = false)]
    pub comments: bool,
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct IssueEditArgs {
    pub id: String,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub body: Option<String>,
    #[arg(long)]
    pub state: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct IssueCommentArgs {
    pub id: String,
    #[arg(long)]
    pub body: Option<String>,
    #[arg(long, default_value_t = false)]
    pub close: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum PrCommand {
    List(PrListArgs),
    Create(PrCreateArgs),
    Show(PrShowArgs),
    Review(PrReviewArgs),
    Merge(PrMergeArgs),
}

#[derive(Args, Debug, Clone)]
pub struct PrListArgs {
    #[arg(long)]
    pub repo: Option<String>,
    #[arg(long)]
    pub state: Option<String>,
    #[arg(long)]
    pub author: Option<String>,
    #[arg(long)]
    pub reviewer: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct PrCreateArgs {
    #[arg(long)]
    pub repo: Option<String>,
    #[arg(long)]
    pub base: Option<String>,
    #[arg(long)]
    pub head: Option<String>,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub body: Option<String>,
    #[arg(long, default_value_t = false)]
    pub draft: bool,
}

#[derive(Args, Debug, Clone)]
pub struct PrShowArgs {
    pub id: String,
    #[arg(long, default_value_t = false)]
    pub diff: bool,
    #[arg(long, default_value_t = false)]
    pub comments: bool,
    #[arg(long, default_value_t = false)]
    pub checks: bool,
}

#[derive(Args, Debug, Clone)]
pub struct PrReviewArgs {
    pub id: String,
    #[arg(long, default_value_t = false)]
    pub approve: bool,
    #[arg(long, default_value_t = false)]
    pub request_changes: bool,
    #[arg(long)]
    pub comment: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct PrMergeArgs {
    pub id: String,
}

#[derive(Subcommand, Debug, Clone)]
pub enum KnotCommand {
    /// Migrate a repository to another knot
    Migrate(KnotMigrateArgs),
}

#[derive(Args, Debug, Clone)]
pub struct KnotMigrateArgs {
    /// Repo to migrate: <owner>/<name> (owner defaults to your handle)
    #[arg(long)]
    pub repo: String,
    /// Target knot hostname (e.g. knot1.tangled.sh)
    #[arg(long, value_name = "HOST")]
    pub to: String,
    /// Use HTTPS source when seeding new repo
    #[arg(long, default_value_t = true)]
    pub https: bool,
    /// Update PDS record knot field after seeding
    #[arg(long, default_value_t = true)]
    pub update_record: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SpindleCommand {
    List(SpindleListArgs),
    Config(SpindleConfigArgs),
    Run(SpindleRunArgs),
    Logs(SpindleLogsArgs),
    /// Secrets management
    #[command(subcommand)]
    Secret(SpindleSecretCommand),
}

#[derive(Args, Debug, Clone)]
pub struct SpindleListArgs {
    #[arg(long)]
    pub repo: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct SpindleConfigArgs {
    #[arg(long)]
    pub repo: Option<String>,
    #[arg(long)]
    pub url: Option<String>,
    #[arg(long, default_value_t = false)]
    pub enable: bool,
    #[arg(long, default_value_t = false)]
    pub disable: bool,
}

#[derive(Args, Debug, Clone)]
pub struct SpindleRunArgs {
    #[arg(long)]
    pub repo: Option<String>,
    #[arg(long)]
    pub branch: Option<String>,
    #[arg(long, default_value_t = false)]
    pub wait: bool,
}

#[derive(Args, Debug, Clone)]
pub struct SpindleLogsArgs {
    pub job_id: String,
    #[arg(long, default_value_t = false)]
    pub follow: bool,
    #[arg(long)]
    pub lines: Option<usize>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SpindleSecretCommand {
    /// List secrets for a repo
    List(SpindleSecretListArgs),
    /// Add or update a secret
    Add(SpindleSecretAddArgs),
    /// Remove a secret
    Remove(SpindleSecretRemoveArgs),
}

#[derive(Args, Debug, Clone)]
pub struct SpindleSecretListArgs {
    /// Repo: <owner>/<name>
    #[arg(long)]
    pub repo: String,
}

#[derive(Args, Debug, Clone)]
pub struct SpindleSecretAddArgs {
    /// Repo: <owner>/<name>
    #[arg(long)]
    pub repo: String,
    /// Secret key
    #[arg(long)]
    pub key: String,
    /// Secret value (use '@filename' to read from file, '-' to read from stdin)
    #[arg(long)]
    pub value: String,
}

#[derive(Args, Debug, Clone)]
pub struct SpindleSecretRemoveArgs {
    /// Repo: <owner>/<name>
    #[arg(long)]
    pub repo: String,
    /// Secret key
    #[arg(long)]
    pub key: String,
}
