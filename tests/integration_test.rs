use std::path::PathBuf;
use std::process::Command;

fn get_repo_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn test_cli_runs_on_self() {
    let repo = get_repo_path();
    let output = Command::new("cargo")
        .args(["run", "--", "--repo", repo.to_str().unwrap(), "--out", "/tmp/test-output"])
        .current_dir(&repo)
        .output()
        .expect("Failed to run CLI");

    assert!(output.status.success(), "CLI failed: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn test_cli_help() {
    let repo = get_repo_path();
    let output = Command::new("cargo")
        .args(["run", "--", "--help"])
        .current_dir(&repo)
        .output()
        .expect("Failed to run CLI");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("git-branch-weight"));
    assert!(stdout.contains("--repo"));
    assert!(stdout.contains("--out"));
}

#[test]
fn test_cli_nonexistent_repo() {
    let repo = get_repo_path();
    let output = Command::new("cargo")
        .args(["run", "--", "--repo", "/nonexistent/path"])
        .current_dir(&repo)
        .output()
        .expect("Failed to run CLI");

    assert!(!output.status.success());
}

#[test]
fn test_output_files_created() {
    let repo = get_repo_path();
    let out_dir = "/tmp/test-branch-weight-output";

    // Clean up
    let _ = std::fs::remove_dir_all(out_dir);

    let output = Command::new("cargo")
        .args(["run", "--", "--repo", repo.to_str().unwrap(), "--out", out_dir])
        .current_dir(&repo)
        .output()
        .expect("Failed to run CLI");

    assert!(output.status.success());

    // Check files exist
    assert!(std::path::Path::new(out_dir).join("branches.json").exists());
    assert!(std::path::Path::new(out_dir).join("branches_full.json").exists());
    assert!(std::path::Path::new(out_dir).join("summary.json").exists());

    // Clean up
    let _ = std::fs::remove_dir_all(out_dir);
}

#[test]
fn test_output_json_valid() {
    let repo = get_repo_path();
    let out_dir = "/tmp/test-branch-weight-json";

    let _ = std::fs::remove_dir_all(out_dir);

    let output = Command::new("cargo")
        .args(["run", "--", "--repo", repo.to_str().unwrap(), "--out", out_dir])
        .current_dir(&repo)
        .output()
        .expect("Failed to run CLI");

    assert!(output.status.success());

    // Parse JSON to verify it's valid
    let branches_json = std::fs::read_to_string(
        std::path::Path::new(out_dir).join("branches.json")
    ).expect("Failed to read branches.json");

    let parsed: serde_json::Value = serde_json::from_str(&branches_json)
        .expect("Invalid JSON in branches.json");

    assert!(parsed.is_array());

    let _ = std::fs::remove_dir_all(out_dir);
}
