use tokio::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::Result;

use crate::paths::release_archive_dir;

/// Handles the `cleanup` command
pub async fn handle_cleanup(all: bool, days: u32, dry_run: bool) -> Result<()> {
    let release_archive_dir = release_archive_dir();
    println!(
        "Release archives directory: {}",
        release_archive_dir.display()
    );

    if !fs::try_exists(&release_archive_dir).await.unwrap_or(false) {
        println!("Release archives directory does not exist, nothing to clean up.");
        return Ok(());
    }

    // Calculate total size before cleanup
    let total_size_before = calculate_dir_size_async(&release_archive_dir).await?;
    println!(
        "Current cache size: {}",
        format_file_size(total_size_before)
    );

    if all {
        if dry_run {
            println!("Would remove all release archives in cache directory (dry run)");
        } else {
            println!("Removing all release archives in cache directory...");
            if fs::try_exists(&release_archive_dir).await.unwrap_or(false) {
                fs::remove_dir_all(&release_archive_dir).await?;
                fs::create_dir_all(&release_archive_dir).await?;
            }
            println!("{}", "Cache cleared successfully.");
        }
        return Ok(());
    }

    // Calculate cutoff duration
    let cutoff_duration = Duration::from_secs(60 * 60 * 24 * days as u64); // days to seconds
    let mut cleaned_size = 0;
    let mut files_removed = 0;

    println!("Removing release archives older than {} days...", days);

    // Process release_archive_dir
    if fs::try_exists(&release_archive_dir).await.unwrap_or(false) {
        let mut entries = fs::read_dir(&release_archive_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                if let Ok(metadata) = entry.metadata().await {
                    if let Ok(modified_time) = metadata.modified() {
                        if let Ok(age) = SystemTime::now().duration_since(modified_time) {
                            // Convert to days for display
                            let days_old = age.as_secs() / (60 * 60 * 24);

                            if age > cutoff_duration {
                                let file_size = metadata.len();
                                cleaned_size += file_size;
                                files_removed += 1;

                                if dry_run {
                                    println!(
                                        "Would remove: {} ({} days old, {})",
                                        path.display(),
                                        days_old,
                                        format_file_size(file_size)
                                    );
                                } else {
                                    println!(
                                        "Removing: {} ({} days old, {})",
                                        path.display(),
                                        days_old,
                                        format_file_size(file_size)
                                    );
                                    fs::remove_file(path).await?;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Report results
    if dry_run {
        println!(
            "Would remove {} files totaling {} (dry run)",
            files_removed,
            format_file_size(cleaned_size)
        );
    } else {
        println!(
            "{} {} files removed, {} freed",
            "Cleanup complete.",
            files_removed,
            format_file_size(cleaned_size)
        );

        let total_size_after = calculate_dir_size(&release_archive_dir)?;
        println!("New cache size: {}", format_file_size(total_size_after));
    }

    Ok(())
}

fn calculate_dir_size(dir: &PathBuf) -> Result<u64> {
    let mut total_size = 0;
    if dir.exists() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                total_size += std::fs::metadata(&path)?.len();
            } else if path.is_dir() {
                total_size += calculate_dir_size(&path)?;
            }
        }
    }
    Ok(total_size)
}

async fn calculate_dir_size_async(dir: &PathBuf) -> Result<u64> {
    let mut total_size = 0;
    if fs::try_exists(dir).await.unwrap_or(false) {
        let mut entries = fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let metadata = entry.metadata().await?;
            if metadata.is_file() {
                total_size += metadata.len();
            } else if metadata.is_dir() {
                total_size += Box::pin(calculate_dir_size_async(&path)).await?;
            }
        }
    }
    Ok(total_size)
}

/// Format file size in human readable format
fn format_file_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB", "EB"];

    if size == 0 {
        return "0 B".to_string();
    }

    let base = 1024_f64;
    let exponent = (size as f64).log(base).floor() as usize;
    let value = size as f64 / base.powi(exponent as i32);

    let unit = UNITS[exponent.min(UNITS.len() - 1)];

    if value < 10.0 {
        format!("{:.2} {}", value, unit)
    } else if value < 100.0 {
        format!("{:.1} {}", value, unit)
    } else {
        format!("{:.0} {}", value, unit)
    }
}
