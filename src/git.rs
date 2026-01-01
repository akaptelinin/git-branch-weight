use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

pub struct CommitBlobs {
    pub commit: String,
    pub blobs: HashMap<String, u64>,
}

pub trait GitOps: Send + Sync {
    fn get_branches(&self, repo: &Path) -> Result<Vec<(String, String)>>;
    fn get_unmerged_blobs(&self, repo: &Path, branch: &str, exclude: &str) -> Result<HashMap<String, u64>>;
    fn get_unmerged_commits(&self, repo: &Path, branch: &str, exclude: &str) -> Result<Vec<CommitBlobs>>;
    fn detect_default_branch(&self, repo: &Path) -> Result<String>;
}

pub struct RealGit;

impl GitOps for RealGit {
    fn get_branches(&self, repo: &Path) -> Result<Vec<(String, String)>> {
        let output = Command::new("git")
            .args([
                "for-each-ref",
                "--format=%(refname) %(objectname)",
                "--no-merged=HEAD",
                "refs/heads",
                "refs/remotes",
            ])
            .current_dir(repo)
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

    fn get_unmerged_blobs(&self, repo: &Path, branch: &str, exclude: &str) -> Result<HashMap<String, u64>> {
        let mut rev_list = Command::new("git")
            .args(["rev-list", "--objects", branch, "--not", exclude])
            .current_dir(repo)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to spawn git rev-list")?;

        let mut cat_file = Command::new("git")
            .args(["cat-file", "--batch-check=%(objectname) %(objecttype) %(objectsize:disk)"])
            .current_dir(repo)
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

        let mut blobs: HashMap<String, u64> = HashMap::new();
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

    fn detect_default_branch(&self, repo: &Path) -> Result<String> {
        for name in ["refs/heads/master", "refs/heads/main"] {
            let output = Command::new("git")
                .args(["rev-parse", "--verify", name])
                .current_dir(repo)
                .output()?;

            if output.status.success() {
                return Ok(name.to_string());
            }
        }

        anyhow::bail!("Could not detect default branch (master/main). Use --branch to specify.")
    }

    fn get_unmerged_commits(&self, repo: &Path, branch: &str, exclude: &str) -> Result<Vec<CommitBlobs>> {
        let commits_output = Command::new("git")
            .args(["rev-list", branch, "--not", exclude])
            .current_dir(repo)
            .output()
            .context("Failed to run git rev-list")?;

        let commit_list: Vec<String> = String::from_utf8_lossy(&commits_output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

        let mut results: Vec<CommitBlobs> = Vec::new();

        for commit in commit_list {
            let diff_output = Command::new("git")
                .args(["diff-tree", "-r", "--diff-filter=AM", "--no-commit-id", &commit])
                .current_dir(repo)
                .output()?;

            let mut blobs: Vec<String> = Vec::new();
            for line in String::from_utf8_lossy(&diff_output.stdout).lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    blobs.push(parts[3].to_string());
                }
            }

            if !blobs.is_empty() {
                let sizes = self.get_blob_sizes(repo, &blobs)?;
                results.push(CommitBlobs { commit, blobs: sizes });
            }
        }

        Ok(results)
    }
}

impl RealGit {
    fn get_blob_sizes(&self, repo: &Path, oids: &[String]) -> Result<HashMap<String, u64>> {
        if oids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut cat_file = Command::new("git")
            .args(["cat-file", "--batch-check=%(objectname) %(objecttype) %(objectsize:disk)"])
            .current_dir(repo)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .context("Failed to spawn git cat-file")?;

        {
            let stdin = cat_file.stdin.as_mut().unwrap();
            for oid in oids {
                writeln!(stdin, "{}", oid)?;
            }
        }

        let output = cat_file.wait_with_output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut sizes = HashMap::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[1] == "blob" {
                if let Ok(size) = parts[2].parse::<u64>() {
                    sizes.insert(parts[0].to_string(), size);
                }
            }
        }

        Ok(sizes)
    }
}
