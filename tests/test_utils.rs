// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use lazy_static::lazy_static;
use std::path::PathBuf;
use std::{env, sync::Mutex};
use suiup::paths::{
    get_cache_home, get_config_home, get_data_home, get_default_bin_dir, initialize,
};
use tempfile::TempDir;

#[derive(Debug)]
pub struct TestEnv {
    pub temp_dir: TempDir,
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub bin_dir: PathBuf,
    original_env: Vec<(String, String)>,
}

lazy_static! {
    static ref ZIP_FILES_MUTEX: Mutex<()> = Mutex::new(());
}

impl TestEnv {
    pub fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let base = temp_dir.path();

        let home_dir = dirs::home_dir().unwrap();

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
        let vars_to_capture = vec![
            "LOCALAPPDATA",
            "TEMP",
            "HOME",
            "XDG_DATA_HOME",
            "XDG_CONFIG_HOME",
            "XDG_CACHE_HOME",
            "PATH",
        ];

        let original_env = vars_to_capture
            .into_iter()
            .filter_map(|var| env::var(var).ok().map(|val| (var.to_string(), val)))
            .collect();

        // Set test env vars
        #[cfg(windows)]
        env::set_var("LOCALAPPDATA", &data_dir); // it is the same for data and config
        #[cfg(windows)]
        env::set_var("TEMP", &cache_dir);
        #[cfg(not(windows))]
        env::set_var("XDG_DATA_HOME", &data_dir);
        #[cfg(not(windows))]
        env::set_var("XDG_CONFIG_HOME", &config_dir);
        #[cfg(not(windows))]
        env::set_var("XDG_CACHE_HOME", &cache_dir);

        // Add bin dir to PATH
        let path = env::var("PATH").unwrap_or_default();
        #[cfg(windows)]
        let new_path = format!("{};{}", bin_dir.display(), path);
        #[cfg(not(windows))]
        let new_path = format!("{}:{}", bin_dir.display(), path);
        env::set_var("PATH", new_path);

        Ok(Self {
            temp_dir,
            data_dir,
            config_dir,
            cache_dir,
            bin_dir,
            original_env,
        })
    }

    pub fn initialize_paths(&self) -> Result<(), anyhow::Error> {
        initialize()
    }

    pub fn copy_testnet_releases_to_cache(&self) -> Result<()> {
        let _guard = ZIP_FILES_MUTEX.lock().unwrap();
        // Create cache directory if it doesn't exist
        std::fs::create_dir_all(&self.cache_dir)?;

        let testnet_v1_39_3 = "sui-testnet-v1.39.3-macos-arm64.tgz";
        let testnet_v1_40_1 = "sui-testnet-v1.40.1-macos-arm64.tgz";

        let data_path = PathBuf::new().join("tests").join("data");

        let testnet_v1_39_3_path = data_path.join(testnet_v1_39_3);
        let testnet_v1_40_1_path = data_path.join(testnet_v1_40_1);

        // On CI / first run we do not have these files, so we skip copying them
        if !testnet_v1_39_3_path.exists() || !testnet_v1_40_1_path.exists() {
            return Ok(());
        }

        assert!(
            testnet_v1_39_3_path.exists(),
            "Something went wrong, release archives for test data are missing"
        );
        assert!(
            testnet_v1_40_1_path.exists(),
            "Something went wrong, release archives for test data are missing"
        );

        let releases_dir = self.cache_dir.join("suiup").join("releases");
        std::fs::create_dir_all(&releases_dir)?;

        std::fs::copy(
            testnet_v1_39_3_path,
            self.cache_dir
                .join("suiup")
                .join("releases")
                .join(testnet_v1_39_3),
        )?;
        std::fs::copy(
            testnet_v1_40_1_path,
            self.cache_dir
                .join("suiup")
                .join("releases")
                .join(testnet_v1_40_1),
        )?;

        Ok(())
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        // Restore original env vars
        for (var, val) in &self.original_env {
            env::set_var(var, val);
        }
    }
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
