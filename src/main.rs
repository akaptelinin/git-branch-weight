mod git;
mod objects;
mod report;

use anyhow::Result;
use clap::Parser;
use git::{GitOps, RealGit};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "git-branch-weight")]
#[command(about = "Estimate weight of unmerged Git branches")]
struct Args {
    #[arg(short, long, default_value = ".")]
    repo: PathBuf,

    #[arg(short, long)]
    out: Option<PathBuf>,

    #[arg(short, long)]
    branch: Option<String>,

    #[arg(short = 'y', long)]
    no_prompt: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let start = Instant::now();

    let repo_path = args.repo.canonicalize()?;
    let out_dir = args.out.unwrap_or_else(|| repo_path.join("unmerged-branches-size-report"));

    println!("Opening repository: {}", repo_path.display());

    let git = RealGit;

    let default_branch = match &args.branch {
        Some(b) => b.clone(),
        None => git.detect_default_branch(&repo_path)?,
    };

    println!("Default branch: {}", default_branch);

    let branch_weights = objects::analyze_branches(&git, &repo_path, &default_branch)?;

    std::fs::create_dir_all(&out_dir)?;
    report::write_reports(&out_dir, &branch_weights)?;

    println!("Done in {:.1}s", start.elapsed().as_secs_f64());
    println!("Reports saved to: {}", out_dir.display());

    Ok(())
}
