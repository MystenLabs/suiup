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

/// Returns the path to the configuration file
pub fn config_file_path() -> Result<PathBuf, Error> {
    Ok(get_config_file("config.json"))
}

pub fn release_archive_dir() -> PathBuf {
    get_suiup_cache_dir().join(RELEASES_ARCHIVES_FOLDER)
}

/// Returns the path to the binaries folder
pub fn binaries_dir() -> PathBuf {
    get_suiup_data_dir().join("binaries")
}

pub fn initialize() -> Result<(), Error> {
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
