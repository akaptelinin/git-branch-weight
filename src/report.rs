use crate::objects::BranchWeight;
use anyhow::Result;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
struct BranchReport {
    branch: String,
    #[serde(rename = "totalSizeMB")]
    total_size_mb: String,
    #[serde(rename = "uniqueSizeMB")]
    unique_size_mb: String,
    #[serde(rename = "sharedSizeMB")]
    shared_size_mb: String,
    #[serde(rename = "totalSize")]
    total_size: u64,
    #[serde(rename = "uniqueSize")]
    unique_size: u64,
    #[serde(rename = "sharedSize")]
    shared_size: u64,
    #[serde(rename = "objectCount")]
    object_count: usize,
    #[serde(rename = "uniqueObjectCount")]
    unique_object_count: usize,
    #[serde(rename = "sharedObjectCount")]
    shared_object_count: usize,
}

#[derive(Serialize)]
struct BranchReportLight {
    branch: String,
    #[serde(rename = "totalSizeMB")]
    total_size_mb: String,
    #[serde(rename = "uniqueSizeMB")]
    unique_size_mb: String,
    #[serde(rename = "sharedSizeMB")]
    shared_size_mb: String,
}

#[derive(Serialize)]
struct Summary {
    #[serde(rename = "totalBranches")]
    total_branches: usize,
    #[serde(rename = "totalUniqueSize")]
    total_unique_size: u64,
    #[serde(rename = "totalUniqueSizeMB")]
    total_unique_size_mb: String,
    #[serde(rename = "totalSharedSize")]
    total_shared_size: u64,
    #[serde(rename = "totalSharedSizeMB")]
    total_shared_size_mb: String,
}

pub fn write_reports(out_dir: &Path, branches: &[BranchWeight]) -> Result<()> {
    let full_reports: Vec<BranchReport> = branches
        .iter()
        .map(|b| BranchReport {
            branch: b.branch.clone(),
            total_size_mb: format_size_mb(b.total_size),
            unique_size_mb: format_size_mb(b.unique_size),
            shared_size_mb: format_size_mb(b.shared_size),
            total_size: b.total_size,
            unique_size: b.unique_size,
            shared_size: b.shared_size,
            object_count: b.object_count,
            unique_object_count: b.unique_count,
            shared_object_count: b.shared_count,
        })
        .collect();

    let light_reports: Vec<BranchReportLight> = branches
        .iter()
        .map(|b| BranchReportLight {
            branch: b.branch.clone(),
            total_size_mb: format_size_mb(b.total_size),
            unique_size_mb: format_size_mb(b.unique_size),
            shared_size_mb: format_size_mb(b.shared_size),
        })
        .collect();

    let total_unique: u64 = branches.iter().map(|b| b.unique_size).sum();
    let total_shared: u64 = branches.iter().map(|b| b.shared_size).sum();

    let summary = Summary {
        total_branches: branches.len(),
        total_unique_size: total_unique,
        total_unique_size_mb: format_size_mb(total_unique),
        total_shared_size: total_shared,
        total_shared_size_mb: format_size_mb(total_shared),
    };

    let full_path = out_dir.join("branches_full.json");
    let light_path = out_dir.join("branches.json");
    let summary_path = out_dir.join("summary.json");

    fs::write(&full_path, serde_json::to_string_pretty(&full_reports)?)?;
    fs::write(&light_path, serde_json::to_string_pretty(&light_reports)?)?;
    fs::write(&summary_path, serde_json::to_string_pretty(&summary)?)?;

    println!("Summary:");
    println!("  Branches: {}", branches.len());
    println!("  Total unique size: {}", format_size_mb(total_unique));
    println!("  Total shared size: {}", format_size_mb(total_shared));

    Ok(())
}

fn format_size_mb(size: u64) -> String {
    let mb = size as f64 / (1024.0 * 1024.0);
    if mb >= 0.1 {
        format!("{:.1} MB", mb)
    } else if mb >= 0.01 {
        format!("{:.2} MB", mb)
    } else {
        "0 MB".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_mb_large() {
        assert_eq!(format_size_mb(1024 * 1024), "1.0 MB");
        assert_eq!(format_size_mb(1024 * 1024 * 100), "100.0 MB");
        assert_eq!(format_size_mb(1024 * 1024 * 1024), "1024.0 MB");
    }

    #[test]
    fn test_format_size_mb_medium() {
        // 1024 * 100 = 0.0976 MB < 0.1, so uses 2 decimal places
        assert_eq!(format_size_mb(1024 * 100), "0.10 MB");
        assert_eq!(format_size_mb(1024 * 512), "0.5 MB");
    }

    #[test]
    fn test_format_size_mb_small() {
        // 1024 * 10 = 0.00976 MB < 0.01, so shows "0 MB"
        assert_eq!(format_size_mb(1024 * 10), "0 MB");
        // 1024 * 15 = 0.0146 MB >= 0.01, so shows 2 decimals
        assert_eq!(format_size_mb(1024 * 15), "0.01 MB");
    }

    #[test]
    fn test_format_size_mb_zero() {
        assert_eq!(format_size_mb(0), "0 MB");
        assert_eq!(format_size_mb(100), "0 MB");
    }

    #[test]
    fn test_branch_report_serialization() {
        let report = BranchReportLight {
            branch: "test/branch".to_string(),
            total_size_mb: "10.0 MB".to_string(),
            unique_size_mb: "8.0 MB".to_string(),
            shared_size_mb: "2.0 MB".to_string(),
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"branch\":\"test/branch\""));
        assert!(json.contains("\"totalSizeMB\":\"10.0 MB\""));
    }
}
