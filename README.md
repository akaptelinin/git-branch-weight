# git-branch-weight

Blazingly kind of slow CLI that scans a Git repository and shows how much disk space every **unmerged** branch uses

---

## Why

Over-grown feature branches slow down `git clone` and waste storage. Knowing the worst offenders lets you delete or squash them first

## How it works

1. Collects all branches not merged into the default branch (master/main)
2. For each branch, finds all blob objects not reachable from the default branch
3. Calculates unique size (blobs only in this branch) and shared size (blobs in multiple unmerged branches)
4. Sorts branches by total size descending

## Install

```bash
cargo install --git https://github.com/akaptelinin/git-branch-weight
```

Or build from source:

```bash
git clone https://github.com/akaptelinin/git-branch-weight
cd git-branch-weight
cargo build --release
```

## Usage

```bash
git-branch-weight [OPTIONS]

Options:
  -r, --repo <path>     Path to Git repository (default: current dir)
  -o, --out <path>      Output directory (default: ./unmerged-branches-size-report)
  -B, --branch <name>   Default branch (auto-detects master/main)
  -d, --details <N>     Analyze top N branches for per-commit breakdown
  -y, --no-prompt       Disable interactive prompts
```

## Output

```
<dir>/
  branches.json           Light report (branch + sizes)
  branches_full.json      Full report (+ object counts)
  summary.json            Totals across all branches
  branches_with_commits.json   Per-commit breakdown (with --details)
```

### Example: `branches.json`

```json
[
  {
    "branch": "origin/feature/payments-v2",
    "totalSizeMB": "12.5 MB",
    "uniqueSizeMB": "10.1 MB",
    "sharedSizeMB": "2.4 MB"
  }
]
```

### Example: `branches_with_commits.json` (with `--details`)

```json
[
  {
    "branch": "origin/feature/payments-v2",
    "totalSizeMB": "12.5 MB",
    "totalSize": 13107200,
    "commits": [
      {"commit": "abc123...", "sizeMB": "8.2 MB", "size": 8598323},
      {"commit": "def456...", "sizeMB": "2.1 MB", "size": 2202009}
    ]
  }
]
```

## Performance

| Repository | Branches | Time |
|------------|----------|------|
| ~200k commits, 1400 unmerged branches | 1400 | ~33s |

Uses parallel processing via [rayon](https://github.com/rayon-rs/rayon) and pipes `git rev-list` to `git cat-file` for efficient object enumeration.

## Notes

* Requires `git` CLI in PATH
* Tested on macOS/Linux, should work on Windows with Git-for-Windows
* Uses `objectsize:disk` — actual packed/compressed size in the repository

## License

MIT
