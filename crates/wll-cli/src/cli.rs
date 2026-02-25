use clap::{Parser, Subcommand, Args};

#[derive(Parser)]
#[command(
    name = "wll",
    about = "WorldLine Ledger — Next Generation Version Control",
    version,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[arg(long, global = true, default_value = "text")]
    pub format: OutputFormat,
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize a new WLL repository
    Init(InitArgs),
    /// Show working directory status
    Status(StatusArgs),
    /// Stage files for commitment
    Add(AddArgs),
    /// Create a commitment with receipt chain
    Commit(CommitArgs),
    /// Show receipt history
    Log(LogArgs),
    /// Show a specific receipt
    Show(ShowArgs),
    /// List, create, or delete branches
    Branch(BranchArgs),
    /// Switch to a different branch
    Switch(SwitchArgs),
    /// Create or list tags
    Tag(TagArgs),
    /// Show changes between receipts
    Diff(DiffArgs),
    /// Merge a branch into the current branch
    Merge(MergeArgs),
    /// Manage remote repositories
    Remote(RemoteArgs),
    /// Fetch objects and receipts from a remote
    Fetch(FetchArgs),
    /// Pull from a remote
    Pull(PullArgs),
    /// Push to a remote
    Push(PushArgs),
    /// Show causal provenance chain
    Provenance(ProvenanceArgs),
    /// Show downstream impact
    Impact(ImpactArgs),
    /// Verify receipt chain integrity
    Verify(VerifyArgs),
    /// Replay and verify state from genesis
    Replay(ReplayArgs),
    /// Show full audit trail
    Audit(AuditArgs),
    /// Garbage collect unreachable objects
    Gc(GcArgs),
    /// Repack loose objects
    Repack(RepackArgs),
    /// Full integrity check
    Fsck(FsckArgs),
    /// Get or set configuration
    Config(ConfigArgs),
    /// Start the WLL server daemon
    Serve(ServeArgs),
}

#[derive(Args)]
pub struct InitArgs {
    pub path: Option<String>,
    #[arg(long)]
    pub bare: bool,
}

#[derive(Args)]
pub struct StatusArgs {}

#[derive(Args)]
pub struct AddArgs {
    pub paths: Vec<String>,
}

#[derive(Args)]
pub struct CommitArgs {
    #[arg(short, long)]
    pub message: Option<String>,
    #[arg(long)]
    pub intent: Option<String>,
    #[arg(long)]
    pub evidence: Vec<String>,
    #[arg(long)]
    pub class: Option<String>,
}

#[derive(Args)]
pub struct LogArgs {
    #[arg(short = 'n', long, default_value = "20")]
    pub limit: usize,
    #[arg(long)]
    pub oneline: bool,
    #[arg(long)]
    pub graph: bool,
}

#[derive(Args)]
pub struct ShowArgs {
    pub receipt: String,
}

#[derive(Args)]
pub struct BranchArgs {
    pub name: Option<String>,
    #[arg(short = 'd', long)]
    pub delete: bool,
}

#[derive(Args)]
pub struct SwitchArgs {
    pub branch: String,
    #[arg(short = 'c', long)]
    pub create: bool,
}

#[derive(Args)]
pub struct TagArgs {
    pub name: Option<String>,
    #[arg(short, long)]
    pub message: Option<String>,
    #[arg(short = 'd', long)]
    pub delete: bool,
    #[arg(short, long)]
    pub list: bool,
}

#[derive(Args)]
pub struct DiffArgs {
    #[arg(long)]
    pub staged: bool,
}

#[derive(Args)]
pub struct MergeArgs {
    pub branch: String,
    #[arg(long)]
    pub strategy: Option<String>,
}

#[derive(Args)]
pub struct RemoteArgs {
    #[command(subcommand)]
    pub action: Option<RemoteAction>,
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum RemoteAction {
    Add { name: String, url: String },
    Remove { name: String },
}

#[derive(Args)]
pub struct FetchArgs { pub remote: Option<String> }
#[derive(Args)]
pub struct PullArgs { pub remote: Option<String>, pub branch: Option<String> }
#[derive(Args)]
pub struct PushArgs { pub remote: Option<String>, pub branch: Option<String> }
#[derive(Args)]
pub struct ProvenanceArgs { pub receipt: String }
#[derive(Args)]
pub struct ImpactArgs { pub receipt: String }
#[derive(Args)]
pub struct VerifyArgs {}
#[derive(Args)]
pub struct ReplayArgs { #[arg(long)] pub from_genesis: bool }
#[derive(Args)]
pub struct AuditArgs { pub worldline: Option<String> }
#[derive(Args)]
pub struct GcArgs {}
#[derive(Args)]
pub struct RepackArgs {}
#[derive(Args)]
pub struct FsckArgs {}
#[derive(Args)]
pub struct ConfigArgs { pub key: Option<String>, pub value: Option<String> }
#[derive(Args)]
pub struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1:9418")]
    pub bind: String,
    #[arg(long, default_value = ".")]
    pub root: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_init() {
        let cli = Cli::try_parse_from(["wll", "init"]).unwrap();
        assert!(matches!(cli.command, Command::Init(_)));
    }

    #[test]
    fn parse_init_bare() {
        let cli = Cli::try_parse_from(["wll", "init", "--bare", "/tmp"]).unwrap();
        if let Command::Init(args) = cli.command {
            assert!(args.bare);
            assert_eq!(args.path, Some("/tmp".into()));
        } else { panic!("wrong command"); }
    }

    #[test]
    fn parse_commit() {
        let cli = Cli::try_parse_from(["wll", "commit", "-m", "hello"]).unwrap();
        if let Command::Commit(args) = cli.command {
            assert_eq!(args.message, Some("hello".into()));
        } else { panic!("wrong command"); }
    }

    #[test]
    fn parse_commit_with_intent() {
        let cli = Cli::try_parse_from(["wll", "commit", "--intent", "desc", "--evidence", "uri"]).unwrap();
        if let Command::Commit(args) = cli.command {
            assert_eq!(args.intent, Some("desc".into()));
            assert_eq!(args.evidence, vec!["uri"]);
        } else { panic!("wrong command"); }
    }

    #[test]
    fn parse_log_oneline() {
        let cli = Cli::try_parse_from(["wll", "log", "--oneline", "-n", "5"]).unwrap();
        if let Command::Log(args) = cli.command {
            assert!(args.oneline);
            assert_eq!(args.limit, 5);
        } else { panic!("wrong command"); }
    }

    #[test]
    fn parse_branch() {
        let cli = Cli::try_parse_from(["wll", "branch"]).unwrap();
        assert!(matches!(cli.command, Command::Branch(_)));
    }

    #[test]
    fn parse_branch_delete() {
        let cli = Cli::try_parse_from(["wll", "branch", "-d", "old"]).unwrap();
        if let Command::Branch(args) = cli.command {
            assert!(args.delete);
            assert_eq!(args.name, Some("old".into()));
        } else { panic!("wrong command"); }
    }

    #[test]
    fn parse_switch_create() {
        let cli = Cli::try_parse_from(["wll", "switch", "-c", "feature"]).unwrap();
        if let Command::Switch(args) = cli.command {
            assert!(args.create);
            assert_eq!(args.branch, "feature");
        } else { panic!("wrong command"); }
    }

    #[test]
    fn parse_remote_add() {
        let cli = Cli::try_parse_from(["wll", "remote", "add", "origin", "https://x"]).unwrap();
        if let Command::Remote(args) = cli.command {
            assert!(matches!(args.action, Some(RemoteAction::Add { .. })));
        } else { panic!("wrong command"); }
    }

    #[test]
    fn parse_push() {
        let cli = Cli::try_parse_from(["wll", "push", "origin", "main"]).unwrap();
        if let Command::Push(args) = cli.command {
            assert_eq!(args.remote, Some("origin".into()));
            assert_eq!(args.branch, Some("main".into()));
        } else { panic!("wrong command"); }
    }

    #[test]
    fn parse_verify() {
        let cli = Cli::try_parse_from(["wll", "verify"]).unwrap();
        assert!(matches!(cli.command, Command::Verify(_)));
    }

    #[test]
    fn parse_serve() {
        let cli = Cli::try_parse_from(["wll", "serve", "--bind", "0.0.0.0:8080"]).unwrap();
        if let Command::Serve(args) = cli.command {
            assert_eq!(args.bind, "0.0.0.0:8080");
        } else { panic!("wrong command"); }
    }

    #[test]
    fn parse_verbose() {
        let cli = Cli::try_parse_from(["wll", "--verbose", "init"]).unwrap();
        assert!(cli.verbose);
    }

    #[test]
    fn parse_json_format() {
        let cli = Cli::try_parse_from(["wll", "--format", "json", "status"]).unwrap();
        assert!(matches!(cli.format, OutputFormat::Json));
    }
}
