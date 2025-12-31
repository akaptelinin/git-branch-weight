use anyhow::{Context, Result};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub size: u64,
    pub branches: FxHashSet<u32>,
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
    let branch_count = branches.len();
    println!("Found {} branches to analyze", branch_count);

    if branch_count == 0 {
        return Ok(Vec::new());
    }

    let branch_names: Vec<String> = branches.iter().map(|(name, _)| name.clone()).collect();
    let repo_path_owned = repo_path.to_path_buf();
    let default_branch_owned = default_branch.to_string();

    println!("Collecting unmerged objects from branches...");

    let partial_maps: Vec<(u32, FxHashMap<String, u64>)> = branches
        .par_iter()
        .enumerate()
        .filter_map(|(i, (_, tip))| {
            if i % 100 == 0 {
                eprintln!("Processing branch {}/{}", i, branch_count);
            }

            collect_unmerged_blobs(&repo_path_owned, tip, &default_branch_owned)
                .ok()
                .filter(|m| !m.is_empty())
                .map(|m| (i as u32, m))
        })
        .collect();

    println!("Merging {} branch results...", partial_maps.len());
    let mut object_map: FxHashMap<String, ObjectInfo> = FxHashMap::default();

    for (branch_idx, branch_objects) in partial_maps {
        for (oid, size) in branch_objects {
            object_map
                .entry(oid)
                .and_modify(|info| {
                    info.branches.insert(branch_idx);
                })
                .or_insert_with(|| {
                    let mut branches = FxHashSet::default();
                    branches.insert(branch_idx);
                    ObjectInfo { size, branches }
                });
        }
    }

    println!("Calculating branch weights...");

    let mut branch_stats: Vec<(u64, u64, usize, usize)> = vec![(0, 0, 0, 0); branch_count];

    for info in object_map.values() {
        let is_shared = info.branches.len() > 1;

        for &branch_idx in &info.branches {
            let entry = &mut branch_stats[branch_idx as usize];
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
        .enumerate()
        .filter(|(_, (u, s, _, _))| *u > 0 || *s > 0)
        .map(|(i, (unique_size, shared_size, unique_count, shared_count))| {
            BranchWeight {
                branch: branch_names[i].clone(),
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

fn collect_unmerged_blobs(
    repo_path: &Path,
    branch_ref: &str,
    default_branch: &str,
) -> Result<FxHashMap<String, u64>> {
    let mut rev_list = Command::new("git")
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

    let rev_stdout = rev_list.stdout.take().unwrap();
    let cat_stdin = cat_file.stdin.take().unwrap();
    let cat_stdout = cat_file.stdout.take().unwrap();

    let writer_handle = std::thread::spawn(move || {
        let reader = BufReader::new(rev_stdout);
        let mut writer = cat_stdin;
        for line in reader.lines() {
            if let Ok(line) = line {
                if let Some(oid) = line.split_whitespace().next() {
                    let _ = writeln!(writer, "{}", oid);
                }
            }
        }
    });

    let mut blobs: FxHashMap<String, u64> = FxHashMap::default();
    let reader = BufReader::new(cat_stdout);

    for line in reader.lines() {
        if let Ok(line) = line {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[1] == "blob" {
                if let Ok(size) = parts[2].parse::<u64>() {
                    blobs.insert(parts[0].to_string(), size);
                }
            }
        }
    }

    let _ = writer_handle.join();
    let _ = rev_list.wait();
    let _ = cat_file.wait();

    Ok(blobs)
}

fn collect_branches_via_git(repo_path: &Path) -> Result<Vec<(String, String)>> {
    let output = Command::new("git")
        .args([
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "--no-merged=HEAD",
            "refs/heads",
            "refs/remotes",
        ])
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn get_repo_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn test_detect_default_branch() {
        let repo = get_repo_path();
        let result = detect_default_branch(&repo);
        assert!(result.is_ok());
        let branch = result.unwrap();
        assert!(branch == "refs/heads/master" || branch == "refs/heads/main");
    }

    #[test]
    fn test_detect_default_branch_nonexistent_repo() {
        let result = detect_default_branch(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }

    #[test]
    fn test_branch_weight_total_size() {
        let bw = BranchWeight {
            branch: "test".to_string(),
            unique_size: 100,
            shared_size: 50,
            total_size: 150,
            object_count: 10,
            unique_count: 7,
            shared_count: 3,
        };
        assert_eq!(bw.total_size, bw.unique_size + bw.shared_size);
        assert_eq!(bw.object_count, bw.unique_count + bw.shared_count);
    }

    #[test]
    fn test_object_info_branches() {
        let mut info = ObjectInfo {
            size: 1024,
            branches: FxHashSet::default(),
        };
        info.branches.insert(0);
        info.branches.insert(1);
        info.branches.insert(0); // duplicate

        assert_eq!(info.branches.len(), 2);
        assert!(info.branches.contains(&0));
        assert!(info.branches.contains(&1));
    }

    #[test]
    fn test_analyze_branches_on_self() {
        let repo = get_repo_path();
        let default_branch = detect_default_branch(&repo).unwrap();
        let result = analyze_branches(&repo, &default_branch);

        // Should not panic, should return Ok
        assert!(result.is_ok());

        // Results should be sorted by total_size descending
        let branches = result.unwrap();
        for i in 1..branches.len() {
            assert!(branches[i - 1].total_size >= branches[i].total_size);
        }
    }
}
