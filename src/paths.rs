// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Error;
use std::collections::BTreeMap;
use std::env;
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
pub async fn default_file_path() -> Result<PathBuf, Error> {
    let path = get_config_file("default_version.json");
    if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
        let mut file = tokio::fs::File::create(&path).await?;
        let default = BTreeMap::<String, (String, String)>::new();
        let default_str = serde_json::to_string_pretty(&default)?;
        tokio::io::AsyncWriteExt::write_all(&mut file, default_str.as_bytes()).await?;
    }
    Ok(path)
}

/// Returns the path to the installed binaries file
pub async fn installed_binaries_file() -> Result<PathBuf, Error> {
    let path = get_config_file("installed_binaries.json");
    if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
        // We'll need to adjust this reference after moving more code
        InstalledBinaries::create_file_async(&path).await?;
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

pub async fn initialize() -> Result<(), Error> {
    tokio::fs::create_dir_all(get_suiup_config_dir()).await?;
    tokio::fs::create_dir_all(get_suiup_data_dir()).await?;
    tokio::fs::create_dir_all(get_suiup_cache_dir()).await?;
    tokio::fs::create_dir_all(binaries_dir()).await?;
    tokio::fs::create_dir_all(release_archive_dir()).await?;
    
    let default_bin_dir = get_default_bin_dir();
    if !tokio::fs::try_exists(&default_bin_dir).await.unwrap_or(false) {
        tokio::fs::create_dir_all(&default_bin_dir).await?;
    }
    
    default_file_path().await?;
    installed_binaries_file().await?;
    Ok(())
}
