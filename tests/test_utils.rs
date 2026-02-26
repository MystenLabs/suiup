// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use lazy_static::lazy_static;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{env, sync::Mutex};
use suiup::paths::{
    binaries_dir, get_cache_home, get_config_home, get_data_home, get_default_bin_dir,
    get_suiup_cache_dir, initialize,
};
use suiup::set_env_var;
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
        })
    }

    pub fn initialize_paths(&self) -> Result<(), anyhow::Error> {
        initialize()?;
        self.seed_standalone_mvr_cache()?;
        Ok(())
    }

    pub fn copy_testnet_releases_to_cache(&self) -> Result<()> {
        let _guard = ZIP_FILES_MUTEX.lock().unwrap();
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

        copy_or_generate_archive(
            &data_path.join(&testnet_v1_39_3),
            &sui_139_dst,
            vec![
                ("sui", script_for_binary_version("sui", "1.39.3")),
                ("sui-debug", script_for_binary_version("sui", "1.39.3")),
            ],
        )?;

        copy_or_generate_archive(
            &data_path.join(&testnet_v1_40_1),
            &sui_140_dst,
            vec![
                ("sui", script_for_binary_version("sui", "1.40.1")),
                ("sui-debug", script_for_binary_version("sui", "1.40.1")),
            ],
        )?;

        copy_or_generate_archive(
            &data_path.join(&walrus_v1_18_2),
            &walrus_dst,
            vec![("walrus", script_for_binary_version("walrus", "1.18.2"))],
        )?;

        Ok(())
    }

    fn seed_standalone_mvr_cache(&self) -> Result<()> {
        let standalone_dir = binaries_dir().join("standalone");
        std::fs::create_dir_all(&standalone_dir)?;

        create_mock_executable(
            &standalone_dir.join(mock_binary_filename("mvr", "v0.0.4")),
            "mvr",
            "0.0.4",
        )?;
        create_mock_executable(
            &standalone_dir.join(mock_binary_filename("mvr", "v0.0.5")),
            "mvr",
            "0.0.5",
        )?;

        let releases_cache = get_suiup_cache_dir().join("standalone_releases_MystenLabs_mvr.json");
        if let Some(parent) = releases_cache.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let (os, arch) = detect_os_arch_for_tests();
        let payload = serde_json::json!([
            {
                "tag_name": "v0.0.5",
                "assets": [{
                    "name": format!("mvr-{os}-{arch}"),
                    "browser_download_url": "https://example.invalid/mvr-v0.0.5"
                }]
            },
            {
                "tag_name": "v0.0.4",
                "assets": [{
                    "name": format!("mvr-{os}-{arch}"),
                    "browser_download_url": "https://example.invalid/mvr-v0.0.4"
                }]
            }
        ]);

        std::fs::write(&releases_cache, serde_json::to_string_pretty(&payload)?)?;
        Ok(())
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

fn mock_binary_filename(name: &str, version: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        format!("{name}-{version}.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        format!("{name}-{version}")
    }
}

fn script_for_binary_version(binary: &str, version: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        format!("@echo off\r\necho {binary} {version}\r\n")
    }
    #[cfg(not(target_os = "windows"))]
    {
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo \"{binary} {version}\"\nelse\n  echo \"{binary} {version}\"\nfi\n"
        )
    }
}

fn create_mock_executable(path: &Path, binary: &str, version: &str) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = File::create(path)?;
    file.write_all(script_for_binary_version(binary, version).as_bytes())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms)?;
    }

    Ok(())
}

fn create_mock_tgz(path: &Path, entries: Vec<(&str, String)>) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(path)?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar = tar::Builder::new(encoder);

    for (name, content) in entries {
        let bytes = content.as_bytes();
        let mut header = tar::Header::new_gnu();
        header.set_path(name)?;
        header.set_size(bytes.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append(&header, bytes)?;
    }

    tar.finish()?;
    Ok(())
}

fn copy_or_generate_archive(src: &Path, dst: &Path, entries: Vec<(&str, String)>) -> Result<()> {
    if dst.exists() {
        return Ok(());
    }

    if src.exists() {
        std::fs::copy(src, dst)?;
    } else {
        create_mock_tgz(dst, entries)?;
    }

    Ok(())
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        // Restore original env vars
        for (var, val) in &self.original_env {
            set_env_var!(var, val);
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
