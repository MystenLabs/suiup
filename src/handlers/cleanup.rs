use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};

use crate::paths::release_archive_dir;

/// Handles the `cleanup` command
pub fn handle_cleanup(all: bool, days: u32, dry_run: bool) -> Result<()> {
    let release_archive_dir = release_archive_dir();
    println!(
        "Release archives directory: {}",
        release_archive_dir.display()
    );

    if !release_archive_dir.exists() {
        println!("Release archives directory does not exist, nothing to clean up.");
        return Ok(());
    }

    // Calculate total size before cleanup
    let total_size_before = calculate_dir_size(&release_archive_dir)?;
    println!(
        "Current cache size: {}",
        format_file_size(total_size_before)
    );

    if all {
        if dry_run {
            println!("Would remove all release archives in cache directory (dry run)");
        } else {
            println!("Removing all release archives in cache directory...");
            if release_archive_dir.exists() {
                fs::remove_dir_all(&release_archive_dir).with_context(|| {
                    format!(
                        "Cannot remove release archive directory {}",
                        release_archive_dir.display()
                    )
                })?;
                fs::create_dir_all(&release_archive_dir).with_context(|| {
                    format!(
                        "Cannot recreate release archive directory {}",
                        release_archive_dir.display()
                    )
                })?;
            }
            println!("Cache cleared successfully.");
        }
        return Ok(());
    }

    // Calculate cutoff duration
    let cutoff_duration = Duration::from_secs(60 * 60 * 24 * days as u64); // days to seconds
    let mut cleaned_size = 0;
    let mut files_removed = 0;

    println!("Removing release archives older than {} days...", days);

    // Process release_archive_dir
    if release_archive_dir.exists() {
        let entries = fs::read_dir(&release_archive_dir).with_context(|| {
            format!(
                "Cannot read release archive directory {}",
                release_archive_dir.display()
            )
        })?;
        for entry in entries {
            let entry = entry.with_context(|| {
                format!(
                    "Cannot read an entry in directory {}",
                    release_archive_dir.display()
                )
            })?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            // Get file metadata and age
            let metadata = fs::metadata(&path)
                .with_context(|| format!("Cannot read metadata for {}", path.display()))?;
            let modified_time = metadata.modified()?;
            let age = SystemTime::now().duration_since(modified_time)?;

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
                    fs::remove_file(&path)
                        .with_context(|| format!("Cannot remove file {}", path.display()))?;
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
            "Cleanup complete. {} files removed, {} freed",
            files_removed,
            format_file_size(cleaned_size)
        );

        let total_size_after = calculate_dir_size(&release_archive_dir)?;
        println!("New cache size: {}", format_file_size(total_size_after));
    }

    Ok(())
}

fn calculate_dir_size(dir: &PathBuf) -> Result<u64> {
    if !dir.exists() {
        return Ok(0);
    }

    let mut total_size = 0;
    for entry in
        fs::read_dir(dir).with_context(|| format!("Cannot read directory {}", dir.display()))?
    {
        let entry = entry
            .with_context(|| format!("Cannot read an entry in directory {}", dir.display()))?;
        let path = entry.path();

        if path.is_file() {
            total_size += fs::metadata(&path)
                .with_context(|| format!("Cannot read metadata for {}", path.display()))?
                .len();
        } else if path.is_dir() {
            total_size += calculate_dir_size(&path)?;
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
