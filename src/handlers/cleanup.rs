use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::Result;

use crate::handlers::config::ConfigHandler;
use crate::paths::release_archive_dir;

/// Cache configuration settings
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub max_size_mb: u64,
    pub max_age_days: u32,
    pub auto_cleanup_enabled: bool,
}

impl CacheConfig {
    /// Create CacheConfig from the application configuration
    pub fn from_config() -> Result<Self> {
        let config_handler = ConfigHandler::new()?;
        let config = config_handler.get_config();
        
        Ok(Self {
            max_size_mb: config.max_cache_size / (1024 * 1024), // Convert bytes to MB
            max_age_days: config.cache_days,
            auto_cleanup_enabled: config.auto_cleanup,
        })
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size_mb: 1024, // 1GB default max cache size
            max_age_days: 30,   // 30 days default max age
            auto_cleanup_enabled: true,
        }
    }
}

/// Get current cache statistics
pub fn get_cache_stats() -> Result<CacheStats> {
    let release_archive_dir = release_archive_dir();
    let total_size = calculate_dir_size(&release_archive_dir)?;
    let file_count = count_files(&release_archive_dir)?;
    
    Ok(CacheStats {
        total_size_bytes: total_size,
        file_count,
        directory_path: release_archive_dir,
    })
}

/// Cache statistics structure
#[derive(Debug)]
pub struct CacheStats {
    pub total_size_bytes: u64,
    pub file_count: usize,
    pub directory_path: PathBuf,
}

/// Auto cleanup based on cache policy
pub async fn auto_cleanup_cache(config: &CacheConfig) -> Result<()> {
    if !config.auto_cleanup_enabled {
        return Ok(());
    }

    let cache_stats = get_cache_stats()?;
    let size_mb = cache_stats.total_size_bytes / (1024 * 1024);
    
    if size_mb > config.max_size_mb {
        println!("Cache size ({} MB) exceeds limit ({} MB), running auto cleanup...", 
                 size_mb, config.max_size_mb);
        handle_cleanup(false, config.max_age_days, false).await?;
    }
    
    Ok(())
}

/// Count files in directory recursively
fn count_files(dir: &PathBuf) -> Result<usize> {
    let mut count = 0;
    if dir.exists() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                count += 1;
            } else if path.is_dir() {
                count += count_files(&path)?;
            }
        }
    }
    Ok(count)
}

/// Advanced cleanup handler with new options
pub async fn handle_cleanup_advanced(all: bool, days: u32, dry_run: bool, stats: bool, smart: bool) -> Result<()> {
    // If only stats requested, show them and exit
    if stats {
        match get_cache_stats() {
            Ok(stats) => {
                println!("=== Cache Statistics ===");
                println!("Directory: {}", stats.directory_path.display());
                println!("Total files: {}", stats.file_count);
                println!("Total size: {}", format_file_size(stats.total_size_bytes));
                
                let config = CacheConfig::from_config().unwrap_or_else(|_| CacheConfig::default());
                println!("Size limit: {} MB", config.max_size_mb);
                
                let size_mb = stats.total_size_bytes / (1024 * 1024);
                if size_mb > config.max_size_mb {
                    println!("⚠️  Cache size exceeds limit!");
                } else {
                    println!("✅ Cache size within limits");
                }
                println!("========================");
                return Ok(());
            }
            Err(e) => {
                println!("Error: Could not get cache statistics: {}", e);
                return Err(e);
            }
        }
    }

    // Use smart cleanup strategy if requested
    if smart {
        return smart_cleanup(days, dry_run).await;
    }

    // Default cleanup behavior
    handle_cleanup(all, days, dry_run).await
}

/// Smart cleanup strategy - removes oldest files first to maintain size limits
pub async fn smart_cleanup(max_age_days: u32, dry_run: bool) -> Result<()> {
    let release_archive_dir = release_archive_dir();
    let config = CacheConfig::from_config().unwrap_or_else(|_| CacheConfig::default());
    
    println!("Running smart cleanup strategy...");
    
    if !release_archive_dir.exists() {
        println!("Release archives directory does not exist, nothing to clean up.");
        return Ok(());
    }

    // Get all files with their metadata
    let mut file_entries = Vec::new();
    collect_files_recursively(&release_archive_dir, &mut file_entries)?;
    
    // Sort by modification time (oldest first)
    file_entries.sort_by_key(|entry| entry.modified_time);
    
    let total_size_before = file_entries.iter().map(|entry| entry.size).sum::<u64>();
    let size_mb_before = total_size_before / (1024 * 1024);
    
    println!("Current cache size: {} MB", size_mb_before);
    println!("Size limit: {} MB", config.max_size_mb);
    
    let mut cleaned_size = 0;
    let mut files_removed = 0;
    let mut remaining_size = total_size_before;
    
    let cutoff_duration = Duration::from_secs(60 * 60 * 24 * max_age_days as u64);
    
    for entry in file_entries {
        let should_remove = if remaining_size / (1024 * 1024) > config.max_size_mb {
            // Over size limit, remove this file regardless of age
            true
        } else {
            // Under size limit, only remove if over age limit
            entry.age > cutoff_duration
        };
        
        if should_remove {
            let days_old = entry.age.as_secs() / (60 * 60 * 24);
            cleaned_size += entry.size;
            files_removed += 1;
            remaining_size -= entry.size;
            
            if dry_run {
                println!(
                    "Would remove: {} ({} days old, {})",
                    entry.path.display(),
                    days_old,
                    format_file_size(entry.size)
                );
            } else {
                println!(
                    "Removing: {} ({} days old, {})",
                    entry.path.display(),
                    days_old,
                    format_file_size(entry.size)
                );
                fs::remove_file(&entry.path)?;
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
            "Smart cleanup complete. {} files removed, {} freed",
            files_removed,
            format_file_size(cleaned_size)
        );
        
        let final_size_mb = remaining_size / (1024 * 1024);
        println!("New cache size: {} MB", final_size_mb);
        
        if final_size_mb <= config.max_size_mb {
            println!("✅ Cache size now within limits");
        } else {
            println!("⚠️  Cache size still exceeds limits - consider more aggressive cleanup");
        }
    }
    
    Ok(())
}

#[derive(Debug)]
struct FileEntry {
    path: PathBuf,
    size: u64,
    modified_time: SystemTime,
    age: Duration,
}

fn collect_files_recursively(dir: &PathBuf, entries: &mut Vec<FileEntry>) -> Result<()> {
    if dir.exists() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Ok(metadata) = fs::metadata(&path) {
                    if let Ok(modified_time) = metadata.modified() {
                        if let Ok(age) = SystemTime::now().duration_since(modified_time) {
                            entries.push(FileEntry {
                                path,
                                size: metadata.len(),
                                modified_time,
                                age,
                            });
                        }
                    }
                }
            } else if path.is_dir() {
                collect_files_recursively(&path, entries)?;
            }
        }
    }
    Ok(())
}
pub async fn handle_cleanup(all: bool, days: u32, dry_run: bool) -> Result<()> {
    let release_archive_dir = release_archive_dir();
    
    // Show current cache statistics first
    match get_cache_stats() {
        Ok(stats) => {
            println!("=== Cache Statistics ===");
            println!("Directory: {}", stats.directory_path.display());
            println!("Total files: {}", stats.file_count);
            println!("Total size: {}", format_file_size(stats.total_size_bytes));
            
            let config = CacheConfig::from_config().unwrap_or_else(|_| CacheConfig::default());
            println!("Size limit: {} MB", config.max_size_mb);
            println!("========================");
        }
        Err(e) => println!("Warning: Could not get cache statistics: {}", e),
    }

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
                fs::remove_dir_all(&release_archive_dir)?;
                fs::create_dir_all(&release_archive_dir)?;
            }
            println!( "Cache cleared successfully.");
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
        let entries = fs::read_dir(&release_archive_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            // Get file metadata and age
            let metadata = fs::metadata(&path)?;
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
                    fs::remove_file(path)?;
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
    if !dir.exists() {
        return Ok(0);
    }

    let mut total_size = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            total_size += fs::metadata(&path)?.len();
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
