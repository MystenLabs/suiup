// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Error};
use std::collections::BTreeMap;
use std::env;
use std::fs::{File, create_dir_all};
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
    get_config_home().join("suiup")
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
            std::fs::create_dir_all(&path).unwrap_or_else(|e| {
                panic!(
                    "Cannot create default bin directory {}: {e}",
                    path.display()
                )
            });
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
        if let Some(parent) = path.parent() {
            create_dir_all(parent).with_context(|| {
                format!(
                    "Cannot create parent directory for default file {}",
                    parent.display()
                )
            })?;
        }

        let mut file = File::create(&path)
            .with_context(|| format!("Cannot create default version file {}", path.display()))?;
        let default = BTreeMap::<String, (String, String)>::new();
        let default_str = serde_json::to_string_pretty(&default)
            .context("Cannot serialize default version file content")?;
        file.write_all(default_str.as_bytes())
            .with_context(|| format!("Cannot write default version file {}", path.display()))?;
    }
    Ok(path)
}

/// Returns the path to the installed binaries file
pub fn installed_binaries_file() -> Result<PathBuf, Error> {
    let path = get_config_file("installed_binaries.json");
    if !path.exists() {
        if let Some(parent) = path.parent() {
            create_dir_all(parent).with_context(|| {
                format!(
                    "Cannot create parent directory for installed binaries file {}",
                    parent.display()
                )
            })?;
        }
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
    let config_dir = get_suiup_config_dir();
    create_dir_all(&config_dir)
        .with_context(|| format!("Cannot create config directory {}", config_dir.display()))?;
    let data_dir = get_suiup_data_dir();
    create_dir_all(&data_dir)
        .with_context(|| format!("Cannot create data directory {}", data_dir.display()))?;
    let cache_dir = get_suiup_cache_dir();
    create_dir_all(&cache_dir)
        .with_context(|| format!("Cannot create cache directory {}", cache_dir.display()))?;
    let binaries_directory = binaries_dir();
    create_dir_all(&binaries_directory).with_context(|| {
        format!(
            "Cannot create binaries directory {}",
            binaries_directory.display()
        )
    })?;
    let releases_directory = release_archive_dir();
    create_dir_all(&releases_directory).with_context(|| {
        format!(
            "Cannot create release archive directory {}",
            releases_directory.display()
        )
    })?;
    let default_bin_dir = get_default_bin_dir();
    create_dir_all(&default_bin_dir).with_context(|| {
        format!(
            "Cannot create default bin directory {}",
            default_bin_dir.display()
        )
    })?;
    default_file_path()?;
    installed_binaries_file()?;
    Ok(())
}
