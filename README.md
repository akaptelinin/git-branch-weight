# git-branch-weight

CLI that scans a Git repository and shows how much disk space every **unmerged** branch uses

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
git-branch-weight

Options
  --repo, -r <path>   # path to the Git repository (default = cwd)
  --out, -o <path>    # output directory for the report (default = ./branch-weight-report)
  --branch, -b <name> # default branch name (auto-detects master/main)
```

## Output

```
<dir>/
  branches.json    full stats, biggest first
  branches.tsv     tab-separated for spreadsheets
```

### Example: `branches.json`

```json
[
  {
    "branch": "origin/feature/payments-v2",
    "totalSizeMB": "125.4 MB",
    "uniqueSizeMB": "120.1 MB",
    "sharedSizeMB": "5.3 MB"
  },
  {
    "branch": "origin/hotfix/billing",
    "totalSizeMB": "84.2 MB",
    "uniqueSizeMB": "12.0 MB",
    "sharedSizeMB": "72.2 MB"
  }
]
```

### Example: `branches.tsv`

```
Branch	Total Size	Unique Size	Shared Size	Objects	Unique	Shared
origin/feature/payments-v2	125.4 MB	120.1 MB	5.3 MB	1542	1500	42
origin/hotfix/billing	84.2 MB	12.0 MB	72.2 MB	892	120	772
```

## Performance

| Repository | Branches | Time |
|------------|----------|------|
| ~200k commits, 1400 unmerged branches | 1400 | ~33s |

Uses parallel processing via [rayon](https://github.com/rayon-rs/rayon) and pipes `git rev-list` to `git cat-file` for efficient object enumeration.

## Limits

* Requires `git` CLI in PATH
* Tested on macOS/Linux, should work on Windows with Git-for-Windows
* Measures actual blob sizes, not estimated compressed size

## See also

* [unmerged-branches-weight](https://github.com/nickovchinnikov/unmerged-branches-weight) — Node.js version with estimated compression

## License

MIT
