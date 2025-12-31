use anyhow::{Context, Result};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub size: u64,
    pub branches: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct BranchWeight {
    pub branch: String,
    pub unique_size: u64,
    pub shared_size: u64,
    pub total_size: u64,
    pub object_count: usize,
    pub unique_count: usize,
    pub shared_count: usize,
}

pub fn analyze_branches(repo_path: &Path, default_branch: &str) -> Result<Vec<BranchWeight>> {
    let branches = collect_branches_via_git(repo_path)?;
    println!("Found {} branches to analyze", branches.len());

    let object_map: Mutex<HashMap<String, ObjectInfo>> = Mutex::new(HashMap::new());

    println!("Collecting unmerged objects from branches...");
    let total_branches = branches.len();

    branches.par_iter().enumerate().for_each(|(i, (branch_name, tip))| {
        if i % 100 == 0 {
            println!("Processing branch {}/{}", i, total_branches);
        }

        // Use --not to get only objects NOT in default branch
        if let Ok(branch_objects) = collect_unmerged_objects(repo_path, tip, default_branch) {
            let mut map = object_map.lock().unwrap();

            for (oid, size) in branch_objects {
                map.entry(oid)
                    .and_modify(|info| {
                        info.branches.insert(branch_name.clone());
                    })
                    .or_insert_with(|| {
                        let mut branches = HashSet::new();
                        branches.insert(branch_name.clone());
                        ObjectInfo { size, branches }
                    });
            }
        }
    });

    println!("Calculating branch weights...");
    let object_map = object_map.into_inner().unwrap();

    let mut branch_stats: HashMap<String, (u64, u64, usize, usize)> = HashMap::new();

    for (_oid, info) in &object_map {
        let is_shared = info.branches.len() > 1;

        for branch in &info.branches {
            let entry = branch_stats.entry(branch.clone()).or_insert((0, 0, 0, 0));
            if is_shared {
                entry.1 += info.size;
                entry.3 += 1;
            } else {
                entry.0 += info.size;
                entry.2 += 1;
            }
        }
    }

    let mut results: Vec<BranchWeight> = branch_stats
        .into_iter()
        .map(|(branch, (unique_size, shared_size, unique_count, shared_count))| {
            BranchWeight {
                branch,
                unique_size,
                shared_size,
                total_size: unique_size + shared_size,
                object_count: unique_count + shared_count,
                unique_count,
                shared_count,
            }
        })
        .collect();

    results.sort_by(|a, b| b.total_size.cmp(&a.total_size));

    println!("Found {} branches with unmerged objects", results.len());

    Ok(results)
}

fn collect_unmerged_objects(repo_path: &Path, branch_ref: &str, default_branch: &str) -> Result<HashMap<String, u64>> {
    // Use --not to get only objects NOT in default branch - much faster!
    let rev_list = Command::new("git")
        .args(["rev-list", "--objects", branch_ref, "--not", default_branch])
        .current_dir(repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn git rev-list")?;

    let mut cat_file = Command::new("git")
        .args(["cat-file", "--batch-check=%(objectname) %(objecttype) %(objectsize)"])
        .current_dir(repo_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn git cat-file")?;

    let rev_stdout = rev_list.stdout.unwrap();
    let mut cat_stdin = cat_file.stdin.take().unwrap();

    std::thread::spawn(move || {
        let reader = BufReader::new(rev_stdout);
        for line in reader.lines().filter_map(|l| l.ok()) {
            let oid = line.split_whitespace().next().unwrap_or("");
            if !oid.is_empty() {
                let _ = writeln!(cat_stdin, "{}", oid);
            }
        }
    });

    let cat_stdout = cat_file.stdout.take().unwrap();
    let reader = BufReader::new(cat_stdout);
    let mut objects = HashMap::new();

    for line in reader.lines().filter_map(|l| l.ok()) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let oid = parts[0].to_string();
            let obj_type = parts[1];
            // Only count blobs (actual file content)
            if obj_type == "blob" {
                if let Ok(size) = parts[2].parse::<u64>() {
                    objects.insert(oid, size);
                }
            }
        }
    }

    Ok(objects)
}

fn collect_branches_via_git(repo_path: &Path) -> Result<Vec<(String, String)>> {
    let output = Command::new("git")
        .args(["for-each-ref", "--format=%(refname) %(objectname)", "refs/heads", "refs/remotes"])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git for-each-ref")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut branches = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let refname = parts[0];
            let oid = parts[1];

            if refname.ends_with("/HEAD") {
                continue;
            }

            let branch_name = refname
                .strip_prefix("refs/heads/")
                .or_else(|| refname.strip_prefix("refs/remotes/"))
                .unwrap_or(refname)
                .to_string();

            branches.push((branch_name, oid.to_string()));
        }
    }

    Ok(branches)
}

pub fn detect_default_branch(repo_path: &Path) -> Result<String> {
    for name in ["refs/heads/master", "refs/heads/main"] {
        let output = Command::new("git")
            .args(["rev-parse", "--verify", name])
            .current_dir(repo_path)
            .output()?;

        if output.status.success() {
            return Ok(name.to_string());
        }
    }

    anyhow::bail!("Could not detect default branch (master/main). Use --branch to specify.")
}
