use crate::git::GitOps;
use anyhow::Result;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub size: u64,
    pub branches: FxHashSet<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BranchWeight {
    pub branch: String,
    pub unique_size: u64,
    pub shared_size: u64,
    pub total_size: u64,
    pub object_count: usize,
    pub unique_count: usize,
    pub shared_count: usize,
}

pub fn analyze_branches<G: GitOps>(
    git: &G,
    repo_path: &Path,
    default_branch: &str,
) -> Result<Vec<BranchWeight>> {
    let branches = git.get_branches(repo_path)?;
    let branch_count = branches.len();
    println!("Found {} branches to analyze", branch_count);

    if branch_count == 0 {
        return Ok(Vec::new());
    }

    let branch_names: Vec<String> = branches.iter().map(|(name, _)| name.clone()).collect();

    println!("Collecting unmerged objects from branches...");

    let partial_maps: Vec<(u32, FxHashMap<String, u64>)> = branches
        .par_iter()
        .enumerate()
        .filter_map(|(i, (_, tip))| {
            if i % 100 == 0 {
                eprintln!("Processing branch {}/{}", i, branch_count);
            }

            git.get_unmerged_blobs(repo_path, tip, default_branch)
                .ok()
                .filter(|m| !m.is_empty())
                .map(|m| {
                    let fx_map: FxHashMap<String, u64> = m.into_iter().collect();
                    (i as u32, fx_map)
                })
        })
        .collect();

    println!("Merging {} branch results...", partial_maps.len());
    let object_map = merge_branch_objects(partial_maps);

    println!("Calculating branch weights...");
    let results = calculate_weights(&branch_names, &object_map);

    println!("Found {} branches with unmerged objects", results.len());

    Ok(results)
}

#[derive(Debug, Clone)]
pub struct CommitWeight {
    pub commit: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct BranchDetail {
    pub branch: String,
    pub total_size: u64,
    pub commits: Vec<CommitWeight>,
}

pub fn analyze_branch_details<G: GitOps>(
    git: &G,
    repo_path: &Path,
    branches: &[BranchWeight],
    default_branch: &str,
    top_n: usize,
) -> Result<Vec<BranchDetail>> {
    let top_branches: Vec<_> = branches.iter().take(top_n).collect();

    let details: Vec<BranchDetail> = top_branches
        .par_iter()
        .filter_map(|bw| {
            let branch_ref = format!("refs/remotes/{}", bw.branch);
            let commits = git.get_unmerged_commits(repo_path, &branch_ref, default_branch).ok()?;

            let commit_weights: Vec<CommitWeight> = commits
                .into_iter()
                .map(|cb| {
                    let size: u64 = cb.blobs.values().sum();
                    CommitWeight { commit: cb.commit, size }
                })
                .filter(|cw| cw.size > 0)
                .collect();

            let total: u64 = commit_weights.iter().map(|c| c.size).sum();

            Some(BranchDetail {
                branch: bw.branch.clone(),
                total_size: total,
                commits: commit_weights,
            })
        })
        .collect();

    Ok(details)
}

fn merge_branch_objects(
    partial_maps: Vec<(u32, FxHashMap<String, u64>)>,
) -> FxHashMap<String, ObjectInfo> {
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

    object_map
}

fn calculate_weights(
    branch_names: &[String],
    object_map: &FxHashMap<String, ObjectInfo>,
) -> Vec<BranchWeight> {
    let branch_count = branch_names.len();
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

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MockGit {
        branches: Vec<(String, String)>,
        blobs: HashMap<String, HashMap<String, u64>>,
    }

    impl GitOps for MockGit {
        fn get_branches(&self, _repo: &Path) -> Result<Vec<(String, String)>> {
            Ok(self.branches.clone())
        }

        fn get_unmerged_blobs(&self, _repo: &Path, branch: &str, _exclude: &str) -> Result<HashMap<String, u64>> {
            Ok(self.blobs.get(branch).cloned().unwrap_or_default())
        }

        fn get_unmerged_commits(&self, _repo: &Path, _branch: &str, _exclude: &str) -> Result<Vec<crate::git::CommitBlobs>> {
            Ok(Vec::new())
        }

        fn detect_default_branch(&self, _repo: &Path) -> Result<String> {
            Ok("refs/heads/master".to_string())
        }
    }

    #[test]
    fn test_empty_repo() {
        let mock = MockGit {
            branches: vec![],
            blobs: HashMap::new(),
        };

        let result = analyze_branches(&mock, Path::new("/fake"), "refs/heads/master").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_branch_unique_objects() {
        let mut blobs = HashMap::new();
        blobs.insert("abc123".to_string(), HashMap::from([
            ("obj1".to_string(), 1000u64),
            ("obj2".to_string(), 2000u64),
        ]));

        let mock = MockGit {
            branches: vec![("feature/test".to_string(), "abc123".to_string())],
            blobs,
        };

        let result = analyze_branches(&mock, Path::new("/fake"), "refs/heads/master").unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].branch, "feature/test");
        assert_eq!(result[0].unique_size, 3000);
        assert_eq!(result[0].shared_size, 0);
        assert_eq!(result[0].unique_count, 2);
    }

    #[test]
    fn test_shared_objects_between_branches() {
        let mut blobs = HashMap::new();

        blobs.insert("branch1".to_string(), HashMap::from([
            ("shared_obj".to_string(), 1000u64),
            ("unique1".to_string(), 500u64),
        ]));

        blobs.insert("branch2".to_string(), HashMap::from([
            ("shared_obj".to_string(), 1000u64),
            ("unique2".to_string(), 700u64),
        ]));

        let mock = MockGit {
            branches: vec![
                ("feature/a".to_string(), "branch1".to_string()),
                ("feature/b".to_string(), "branch2".to_string()),
            ],
            blobs,
        };

        let result = analyze_branches(&mock, Path::new("/fake"), "refs/heads/master").unwrap();

        assert_eq!(result.len(), 2);

        let branch_a = result.iter().find(|b| b.branch == "feature/a").unwrap();
        let branch_b = result.iter().find(|b| b.branch == "feature/b").unwrap();

        assert_eq!(branch_a.unique_size, 500);
        assert_eq!(branch_a.shared_size, 1000);

        assert_eq!(branch_b.unique_size, 700);
        assert_eq!(branch_b.shared_size, 1000);
    }

    #[test]
    fn test_results_sorted_by_total_size() {
        let mut blobs = HashMap::new();
        blobs.insert("small".to_string(), HashMap::from([("o1".to_string(), 100u64)]));
        blobs.insert("large".to_string(), HashMap::from([("o2".to_string(), 9000u64)]));
        blobs.insert("medium".to_string(), HashMap::from([("o3".to_string(), 500u64)]));

        let mock = MockGit {
            branches: vec![
                ("small-branch".to_string(), "small".to_string()),
                ("large-branch".to_string(), "large".to_string()),
                ("medium-branch".to_string(), "medium".to_string()),
            ],
            blobs,
        };

        let result = analyze_branches(&mock, Path::new("/fake"), "refs/heads/master").unwrap();

        assert_eq!(result[0].branch, "large-branch");
        assert_eq!(result[1].branch, "medium-branch");
        assert_eq!(result[2].branch, "small-branch");
    }

    #[test]
    fn test_branch_with_no_objects_excluded() {
        let mut blobs = HashMap::new();
        blobs.insert("has_objects".to_string(), HashMap::from([("o1".to_string(), 100u64)]));
        blobs.insert("empty".to_string(), HashMap::new());

        let mock = MockGit {
            branches: vec![
                ("with-objects".to_string(), "has_objects".to_string()),
                ("empty-branch".to_string(), "empty".to_string()),
            ],
            blobs,
        };

        let result = analyze_branches(&mock, Path::new("/fake"), "refs/heads/master").unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].branch, "with-objects");
    }
}
