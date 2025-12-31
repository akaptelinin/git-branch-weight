# git-branch-weight

Analyze disk space used by unmerged Git branches. Find which branches contribute the most to repository size.

## Installation

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
git-branch-weight --repo /path/to/repo --out /path/to/output
```

### Options

| Option | Description |
|--------|-------------|
| `--repo <PATH>` | Path to Git repository (default: current directory) |
| `--out <PATH>` | Output directory for reports (default: `./branch-weight-report`) |
| `--branch <REF>` | Default branch to compare against (auto-detects master/main) |

## Output

Creates two files in the output directory:

### branches.json

```json
[
  {
    "branch": "origin/feature-branch",
    "totalSizeMB": "125.4 MB",
    "uniqueSizeMB": "120.1 MB",
    "sharedSizeMB": "5.3 MB"
  }
]
```

### branches.tsv

```
Branch	Total Size	Unique Size	Shared Size	Objects	Unique	Shared
origin/feature-branch	125.4 MB	120.1 MB	5.3 MB	1542	1500	42
```

## How it works

1. Lists all branches not merged into the default branch
2. For each branch, finds all blob objects not in the default branch
3. Calculates unique size (objects only in this branch) and shared size (objects in multiple unmerged branches)
4. Sorts branches by total size descending

## Performance

~33 seconds on a repository with 1400 unmerged branches.

Uses parallel processing via [rayon](https://github.com/rayon-rs/rayon) and pipes `git rev-list` to `git cat-file` for efficient object enumeration.

## License

MIT
