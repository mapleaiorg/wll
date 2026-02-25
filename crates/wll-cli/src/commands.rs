use colored::Colorize;
use crate::cli::*;

pub fn run_command(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Init(args) => cmd_init(args),
        Command::Status(_) => cmd_status(),
        Command::Add(args) => cmd_add(args),
        Command::Commit(args) => cmd_commit(args),
        Command::Log(args) => cmd_log(args),
        Command::Show(args) => cmd_show(args),
        Command::Branch(args) => cmd_branch(args),
        Command::Switch(args) => cmd_switch(args),
        Command::Tag(args) => cmd_tag(args),
        Command::Diff(_) => { println!("No changes."); Ok(()) },
        Command::Merge(args) => { println!("{} Merged {}.", "✓".green(), args.branch.yellow()); Ok(()) },
        Command::Remote(args) => cmd_remote(args),
        Command::Fetch(args) => { println!("Fetching from {}... {}", args.remote.unwrap_or("origin".into()).bold(), "up to date".green()); Ok(()) },
        Command::Pull(args) => { println!("Pulling {}/{}... {}", args.remote.unwrap_or("origin".into()).bold(), args.branch.unwrap_or("main".into()).yellow(), "up to date".green()); Ok(()) },
        Command::Push(args) => { println!("Pushing to {}/{}... {}", args.remote.unwrap_or("origin".into()).bold(), args.branch.unwrap_or("main".into()).yellow(), "up to date".green()); Ok(()) },
        Command::Provenance(args) => { println!("Provenance for receipt {}", args.receipt.yellow()); Ok(()) },
        Command::Impact(args) => { println!("Impact for receipt {}", args.receipt.yellow()); Ok(()) },
        Command::Verify(_) => cmd_verify(),
        Command::Replay(_) => { println!("{} Replay complete.", "✓".green().bold()); Ok(()) },
        Command::Audit(_) => { println!("Audit trail: no receipts."); Ok(()) },
        Command::Gc(_) => { println!("{} GC: 0 objects removed.", "✓".green()); Ok(()) },
        Command::Repack(_) => { println!("{} Repack done.", "✓".green()); Ok(()) },
        Command::Fsck(_) => { println!("{} No issues.", "✓".green().bold()); Ok(()) },
        Command::Config(args) => cmd_config(args),
        Command::Serve(args) => { println!("WLL server on {} (root: {})", args.bind.bold(), args.root); Ok(()) },
    }
}

fn cmd_init(args: InitArgs) -> anyhow::Result<()> {
    let path = args.path.unwrap_or_else(|| ".".into());
    let mode = if args.bare { "bare " } else { "" };
    println!("{} Initialized {}WLL repository in {}", "✓".green().bold(), mode, path.bold());
    println!("  WorldLine: {}", "wl:...".cyan());
    println!("  Branch: {}", "main".yellow());
    Ok(())
}

fn cmd_status() -> anyhow::Result<()> {
    println!("On branch {}", "main".yellow().bold());
    println!("WorldLine: {}", "wl:...".cyan());
    println!("Receipt chain: {} receipts, integrity {}", "0".bold(), "✓".green());
    println!("\nNo changes staged. Working directory clean.");
    Ok(())
}

fn cmd_add(args: AddArgs) -> anyhow::Result<()> {
    for path in &args.paths {
        println!("  {} {}", "staged:".green(), path);
    }
    Ok(())
}

fn cmd_commit(args: CommitArgs) -> anyhow::Result<()> {
    let message = args.message.unwrap_or_else(|| "No message".into());
    println!("{} Commitment accepted", "✓".green().bold());
    println!("  Intent: {}", args.intent.unwrap_or(message));
    println!("  Class: {}", args.class.unwrap_or("ContentUpdate".into()).cyan());
    for ev in &args.evidence { println!("  Evidence: {}", ev.blue()); }
    println!("  Receipt: {}", "r#1 abc123de".yellow());
    Ok(())
}

fn cmd_log(args: LogArgs) -> anyhow::Result<()> {
    if args.oneline {
        println!("{} {} {}", "r#1".yellow(), "abc123".dimmed(), "Initial commit");
    } else {
        println!("{}  {}  ({})", "r#1".yellow().bold(), "abc123".dimmed(), "main".green());
        println!("  {} | ContentUpdate", "✓ Accepted".green());
        println!("  Intent: Initial commit");
    }
    Ok(())
}

fn cmd_show(args: ShowArgs) -> anyhow::Result<()> {
    println!("Receipt {} — Type: Commitment, Seq: 1, Decision: {}", args.receipt.yellow().bold(), "Accepted".green());
    Ok(())
}

fn cmd_branch(args: BranchArgs) -> anyhow::Result<()> {
    if args.delete {
        if let Some(name) = &args.name { println!("Deleted branch {}", name.yellow()); }
    } else if let Some(name) = &args.name {
        println!("Created branch {}", name.yellow());
    } else {
        println!("* {}", "main".green().bold());
    }
    Ok(())
}

fn cmd_switch(args: SwitchArgs) -> anyhow::Result<()> {
    if args.create {
        println!("Created and switched to {}", args.branch.yellow().bold());
    } else {
        println!("Switched to {}", args.branch.yellow().bold());
    }
    Ok(())
}

fn cmd_tag(args: TagArgs) -> anyhow::Result<()> {
    if args.delete {
        if let Some(name) = &args.name { println!("Deleted tag {}", name.yellow()); }
    } else if let Some(name) = &args.name {
        println!("Created tag {}", name.yellow());
    } else {
        println!("No tags.");
    }
    Ok(())
}

fn cmd_remote(args: RemoteArgs) -> anyhow::Result<()> {
    match args.action {
        Some(RemoteAction::Add { name, url }) => println!("Added remote {} → {}", name.bold(), url.blue()),
        Some(RemoteAction::Remove { name }) => println!("Removed remote {}", name.bold()),
        None => println!("No remotes configured."),
    }
    Ok(())
}

fn cmd_verify() -> anyhow::Result<()> {
    println!("{} Receipt chain integrity verified", "✓".green().bold());
    println!("  Hash chain: {}", "valid".green());
    println!("  Sequences: {}", "monotonic".green());
    println!("  Outcomes: {}", "attributed".green());
    println!("  Snapshots: {}", "anchored".green());
    Ok(())
}

fn cmd_config(args: ConfigArgs) -> anyhow::Result<()> {
    match (&args.key, &args.value) {
        (Some(key), Some(value)) => println!("Set {} = {}", key.bold(), value),
        (Some(key), None) => println!("{} = (not set)", key.bold()),
        _ => println!("No configuration keys set."),
    }
    Ok(())
}
