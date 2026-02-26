// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Result, anyhow};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, MutexGuard};
use suiup::paths::{
    get_cache_home, get_config_home, get_data_home, get_default_bin_dir, initialize,
};
use suiup::{remove_env_var, set_env_var};
use tempfile::TempDir;

pub struct TestEnv {
    pub temp_dir: TempDir,
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub bin_dir: PathBuf,
    original_env: Vec<(String, Option<String>)>,
    _env_guard: MutexGuard<'static, ()>,
}

static ENV_VARS_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
static ZIP_FILES_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

impl TestEnv {
    pub fn new() -> Result<Self> {
        let env_guard = ENV_VARS_MUTEX.lock().expect("failed to lock env mutex");
        let temp_dir = TempDir::new()?;
        let base = temp_dir.path();

        let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("HOME directory is not set"))?;

        let data_home = get_data_home();
        let config_home = get_config_home();
        let cache_home = get_cache_home();
        let bin_home = get_default_bin_dir();

        let data_dir = if let Ok(path) = data_home.strip_prefix(&home_dir) {
            base.join(path)
        } else {
            base.join(data_home)
        };

        let config_dir = if let Ok(path) = config_home.strip_prefix(&home_dir) {
            base.join(path)
        } else {
            base.join(config_home)
        };

        let cache_dir = if let Ok(path) = cache_home.strip_prefix(&home_dir) {
            base.join(path)
        } else {
            base.join(cache_home)
        };

        let bin_dir = if let Ok(path) = bin_home.strip_prefix(&home_dir) {
            base.join(path)
        } else {
            base.join(bin_home)
        };

        // Create directories
        std::fs::create_dir_all(&data_dir)?;
        std::fs::create_dir_all(&config_dir)?;
        std::fs::create_dir_all(&cache_dir)?;
        std::fs::create_dir_all(&bin_dir)?;

        assert!(data_dir.exists());
        assert!(config_dir.exists());
        assert!(cache_dir.exists());
        assert!(bin_dir.exists());

        // Store original env vars
        let vars_to_capture = [
            "LOCALAPPDATA",
            "HOME",
            "XDG_DATA_HOME",
            "XDG_CONFIG_HOME",
            "XDG_CACHE_HOME",
            "PATH",
        ];

        let original_env = vars_to_capture
            .into_iter()
            .map(|var| (var.to_string(), env::var(var).ok()))
            .collect();

        // Set test env vars
        #[cfg(windows)]
        set_env_var!("LOCALAPPDATA", &data_dir); // it is the same for data and config
        #[cfg(not(windows))]
        set_env_var!("XDG_DATA_HOME", &data_dir);
        #[cfg(not(windows))]
        set_env_var!("XDG_CONFIG_HOME", &config_dir);
        #[cfg(not(windows))]
        set_env_var!("XDG_CACHE_HOME", &cache_dir);

        // Add bin dir to PATH
        let path = env::var("PATH").unwrap_or_default();
        #[cfg(windows)]
        let new_path = format!("{};{}", bin_dir.display(), path);
        #[cfg(not(windows))]
        let new_path = format!("{}:{}", bin_dir.display(), path);
        set_env_var!("PATH", new_path);

        Ok(Self {
            temp_dir,
            data_dir,
            config_dir,
            cache_dir,
            bin_dir,
            original_env,
            _env_guard: env_guard,
        })
    }

    pub fn initialize_paths(&self) -> Result<(), anyhow::Error> {
        initialize()?;
        Ok(())
    }

    pub fn copy_testnet_releases_to_cache(&self) -> Result<()> {
        let _guard = ZIP_FILES_MUTEX
            .lock()
            .expect("failed to lock zip-files mutex");
        // Create cache directory if it doesn't exist
        std::fs::create_dir_all(&self.cache_dir)?;

        let (os, arch) = detect_os_arch_for_tests();
        let testnet_v1_39_3 = format!("sui-testnet-v1.39.3-{os}-{arch}.tgz");
        let testnet_v1_40_1 = format!("sui-testnet-v1.40.1-{os}-{arch}.tgz");
        let walrus_v1_18_2 = format!("walrus-testnet-v1.18.2-{os}-{arch}.tgz");

        let data_path = PathBuf::new().join("tests").join("data");

        let releases_dir = self.cache_dir.join("suiup").join("releases");
        std::fs::create_dir_all(&releases_dir)?;

        let sui_139_dst = releases_dir.join(&testnet_v1_39_3);
        let sui_140_dst = releases_dir.join(&testnet_v1_40_1);
        let walrus_dst = releases_dir.join(&walrus_v1_18_2);

        copy_cached_archive(&data_path.join(&testnet_v1_39_3), &sui_139_dst)?;
        copy_cached_archive(&data_path.join(&testnet_v1_40_1), &sui_140_dst)?;
        copy_cached_archive(&data_path.join(&walrus_v1_18_2), &walrus_dst)?;

        Ok(())
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        for (key, value) in &self.original_env {
            if let Some(value) = value {
                set_env_var!(key, value);
            } else {
                remove_env_var!(key);
            }
        }
    }
}

fn detect_os_arch_for_tests() -> (&'static str, &'static str) {
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "ubuntu"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "macos"
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        if os == "macos" { "arm64" } else { "aarch64" }
    } else {
        "x86_64"
    };

    (os, arch)
}

fn copy_cached_archive(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        return Ok(());
    }

    if src.exists() {
        std::fs::copy(src, dst)?;
    }

    Ok(())
}

// Mock HTTP client for testing
#[cfg(test)]
pub mod mock_http {
    use mockall::mock;
    use reqwest::Response;

    mock! {
        pub HttpClient {
            async fn get(&self, url: String) -> reqwest::Result<Response>;
        }
    }
}
