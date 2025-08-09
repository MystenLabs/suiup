// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Error;
use std::collections::BTreeMap;
use std::env;
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::PathBuf;

use crate::handlers::RELEASES_ARCHIVES_FOLDER;
use crate::types::InstalledBinaries;

#[cfg(not(windows))]
const XDG_DATA_HOME: &str = "XDG_DATA_HOME";
#[cfg(not(windows))]
const XDG_CONFIG_HOME: &str = "XDG_CONFIG_HOME";
#[cfg(not(windows))]
const XDG_CACHE_HOME: &str = "XDG_CACHE_HOME";
#[cfg(not(windows))]
const HOME: &str = "HOME";

pub fn get_data_home() -> PathBuf {
    #[cfg(windows)]
    {
        env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let mut home =
                    PathBuf::from(env::var_os("USERPROFILE").expect("USERPROFILE not set"));
                home.push("AppData");
                home.push("Local");
                home
            })
    }

    #[cfg(not(windows))]
    {
        env::var_os(XDG_DATA_HOME)
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let mut home = PathBuf::from(env::var_os(HOME).expect("HOME not set"));
                home.push(".local");
                home.push("share");
                home
            })
    }
}

pub fn get_config_home() -> PathBuf {
    #[cfg(windows)]
    {
        env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let mut home =
                    PathBuf::from(env::var_os("USERPROFILE").expect("USERPROFILE not set"));
                home.push("AppData");
                home.push("Local");
                home
            })
    }

    #[cfg(not(windows))]
    {
        env::var_os(XDG_CONFIG_HOME)
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let mut home = PathBuf::from(env::var_os("HOME").expect("HOME not set"));
                home.push(".config");
                home
            })
    }
}

pub fn get_cache_home() -> PathBuf {
    #[cfg(windows)]
    {
        env::var_os("TEMP").map(PathBuf::from).unwrap_or_else(|| {
            let mut home = PathBuf::from(env::var_os("USERPROFILE").expect("USERPROFILE not set"));
            home.push("AppData");
            home.push("Local");
            home.push("Temp");
            home
        })
    }

    #[cfg(not(windows))]
    {
        env::var_os(XDG_CACHE_HOME)
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let mut home = PathBuf::from(env::var_os("HOME").expect("HOME not set"));
                home.push(".cache");
                home
            })
    }
}

pub fn get_suiup_data_dir() -> PathBuf {
    get_data_home().join("suiup")
}

pub fn get_suiup_config_dir() -> PathBuf {
    get_config_home().join("suiup").join("config")
}

pub fn get_old_suiup_config_dir() -> PathBuf {
    get_config_home().join("suiup")
}

/// Migrate configuration files from old directory structure to new one
/// This handles migration from ~/.config/suiup/*.json to ~/.config/suiup/config/*.json
fn migrate_config_files() -> Result<(), Error> {
    use std::fs;

    let old_config_dir = get_old_suiup_config_dir();
    let new_config_dir = get_suiup_config_dir();

    // If old directory doesn't exist, no migration needed
    if !old_config_dir.exists() {
        return Ok(());
    }

    // If new directory already exists and has files, skip migration
    if new_config_dir.exists() {
        let default_version_file = new_config_dir.join("default_version.json");
        let installed_binaries_file = new_config_dir.join("installed_binaries.json");
        if default_version_file.exists() || installed_binaries_file.exists() {
            // Already migrated, nothing to do
            return Ok(());
        }
    }

    // Ensure new config directory exists
    create_dir_all(&new_config_dir).map_err(|e| {
        anyhow::anyhow!(
            "Failed to create new config directory {}: {}",
            new_config_dir.display(),
            e
        )
    })?;

    // Find all JSON files in old directory to migrate
    let json_files = ["default_version.json", "installed_binaries.json"];
    let mut migrated_files = Vec::new();
    let mut failed_migrations = Vec::new();

    for filename in &json_files {
        let old_file = old_config_dir.join(filename);
        let new_file = new_config_dir.join(filename);

        if old_file.exists() && !new_file.exists() {
            match fs::copy(&old_file, &new_file) {
                Ok(_) => {
                    // Verify the copied file exists and has content
                    if let Ok(metadata) = fs::metadata(&new_file) {
                        if metadata.len() > 0 {
                            migrated_files.push(filename.to_string());
                            eprintln!(
                                "✓ Migrated config file: {} -> {}",
                                old_file.display(),
                                new_file.display()
                            );
                        } else {
                            failed_migrations.push(format!("{}: copied file is empty", filename));
                        }
                    } else {
                        failed_migrations
                            .push(format!("{}: failed to verify copied file", filename));
                    }
                }
                Err(e) => {
                    failed_migrations.push(format!("{}: {}", filename, e));
                }
            }
        }
    }

    if !migrated_files.is_empty() {
        eprintln!(
            "Configuration migration completed. Migrated {} file(s): {}",
            migrated_files.len(),
            migrated_files.join(", ")
        );
        eprintln!(
            "Note: Original files remain in {} for backup",
            old_config_dir.display()
        );
    }

    if !failed_migrations.is_empty() {
        eprintln!("Warning: Some files could not be migrated:");
        for failure in failed_migrations {
            eprintln!("  - {}", failure);
        }
    }

    Ok(())
}

/// Clean up old configuration files after successful migration
/// This removes JSON files from the old config directory if they exist in the new one
pub fn cleanup_old_config_files() -> Result<(), Error> {
    use std::fs;

    let old_config_dir = get_old_suiup_config_dir();
    let new_config_dir = get_suiup_config_dir();

    // Only proceed if both directories exist
    if !old_config_dir.exists() || !new_config_dir.exists() {
        return Ok(());
    }

    let json_files = ["default_version.json", "installed_binaries.json"];
    let mut cleaned_files = Vec::new();
    let mut cleanup_failures = Vec::new();

    for filename in &json_files {
        let old_file = old_config_dir.join(filename);
        let new_file = new_config_dir.join(filename);

        // Only remove old file if new file exists and has content
        if old_file.exists() && new_file.exists() {
            match fs::metadata(&new_file) {
                Ok(new_metadata) if new_metadata.len() > 0 => match fs::remove_file(&old_file) {
                    Ok(_) => {
                        cleaned_files.push(filename.to_string());
                        eprintln!("✓ Cleaned up old config file: {}", old_file.display());
                    }
                    Err(e) => {
                        cleanup_failures.push(format!("{}: {}", filename, e));
                    }
                },
                _ => {
                    eprintln!(
                        "Skipping cleanup of {} - new file doesn't exist or is empty",
                        filename
                    );
                }
            }
        }
    }

    if !cleaned_files.is_empty() {
        eprintln!(
            "Cleanup completed. Removed {} old config file(s): {}",
            cleaned_files.len(),
            cleaned_files.join(", ")
        );
    }

    if !cleanup_failures.is_empty() {
        eprintln!("Warning: Some old files could not be cleaned up:");
        for failure in cleanup_failures {
            eprintln!("  - {}", failure);
        }
    }

    // Try to remove the old config directory if it's empty (only contains suiup subdirs)
    if let Ok(entries) = fs::read_dir(&old_config_dir) {
        let remaining_files: Vec<_> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                // Only count regular files, ignore directories
                entry.file_type().map(|ft| ft.is_file()).unwrap_or(false)
            })
            .collect();

        if remaining_files.is_empty() {
            match fs::remove_dir(&old_config_dir) {
                Ok(_) => eprintln!(
                    "✓ Removed empty old config directory: {}",
                    old_config_dir.display()
                ),
                Err(_) => {} // Silently ignore if we can't remove it
            }
        }
    }

    Ok(())
}

pub fn get_suiup_cache_dir() -> PathBuf {
    get_cache_home().join("suiup")
}

pub fn get_default_bin_dir() -> PathBuf {
    #[cfg(windows)]
    {
        let mut path = PathBuf::from(env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA not set"));
        path.push("bin");
        if !path.exists() {
            std::fs::create_dir_all(&path).unwrap();
        }
        path
    }

    #[cfg(not(windows))]
    {
        env::var_os("SUIUP_DEFAULT_BIN_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let mut path = PathBuf::from(env::var_os(HOME).expect("HOME not set"));
                path.push(".local");
                path.push("bin");
                path
            })
    }
}

pub fn get_config_file(name: &str) -> PathBuf {
    get_suiup_config_dir().join(name)
}

/// Returns the path to the default version file
pub fn default_file_path() -> Result<PathBuf, Error> {
    let path = get_config_file("default_version.json");
    if !path.exists() {
        let mut file = File::create(&path)?;
        let default = BTreeMap::<String, (String, String)>::new();
        let default_str = serde_json::to_string_pretty(&default)?;
        file.write_all(default_str.as_bytes())?;
    }
    Ok(path)
}

/// Returns the path to the installed binaries file
pub fn installed_binaries_file() -> Result<PathBuf, Error> {
    let path = get_config_file("installed_binaries.json");
    if !path.exists() {
        // We'll need to adjust this reference after moving more code
        InstalledBinaries::create_file(&path)?;
    }
    Ok(path)
}

pub fn release_archive_dir() -> PathBuf {
    get_suiup_cache_dir().join(RELEASES_ARCHIVES_FOLDER)
}

/// Returns the path to the binaries folder
pub fn binaries_dir() -> PathBuf {
    get_suiup_data_dir().join("binaries")
}

pub fn initialize() -> Result<(), Error> {
    // Migrate configuration files from old directory structure
    migrate_config_files()?;

    create_dir_all(get_suiup_config_dir())?;
    create_dir_all(get_suiup_data_dir())?;
    create_dir_all(get_suiup_cache_dir())?;
    create_dir_all(binaries_dir())?;
    create_dir_all(release_archive_dir())?;
    create_dir_all(get_default_bin_dir())?;
    default_file_path()?;
    installed_binaries_file()?;
    Ok(())
}
