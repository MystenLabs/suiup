// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// use crate::handle_commands::{binaries_folder, detect_os_arch, download_file};
use crate::{
    handlers::download::{detect_os_arch, download_file},
    paths::binaries_dir,
    types::Repo,
};
use anyhow::{anyhow, Error};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct MvrRelease {
    pub tag_name: String,
    pub assets: Vec<MvrAsset>,
}

#[derive(Deserialize, Debug)]
pub struct MvrAsset {
    pub name: String,
    pub browser_download_url: String,
}

pub struct MvrInstaller {
    releases: Vec<MvrRelease>,
}

impl Default for MvrInstaller {
    fn default() -> Self {
        Self::new()
    }
}

impl MvrInstaller {
    pub fn new() -> Self {
        Self {
            releases: Vec::new(),
        }
    }

    pub async fn get_releases(&mut self) -> Result<(), Error> {
        let client = reqwest::Client::new();
        let url = format!("https://api.github.com/repos/{}/releases", Repo::Mvr);

        if !self.releases.is_empty() {
            return Ok(());
        }

        let releases: Vec<MvrRelease> = client
            .get(&url)
            .header("User-Agent", "suiup")
            .send()
            .await?
            .json()
            .await?;
        self.releases = releases;
        Ok(())
    }

    pub fn get_latest_release(&self) -> Result<&MvrRelease, Error> {
        println!("Downloading release list");
        let releases = &self.releases;
        releases
            .first()
            .ok_or_else(|| anyhow!("No MVR releases found"))
    }

    /// Download the MVR CLI binary, if it does not exist in the binary folder.
    pub async fn download_version(&mut self, version: Option<String>) -> Result<String, Error> {
        let version = if let Some(v) = version {
            // releases on GitHub are prefixed with `v` before the major.minor.patch version
            if v.starts_with("v") {
                v
            } else {
                format!("v{v}")
            }
        } else {
            if self.releases.is_empty() {
                self.get_releases().await?;
            }
            let latest_release = self.get_latest_release()?.tag_name.clone();
            println!("No version specified. Downloading latest release: {latest_release}");
            latest_release
        };

        let cache_folder = binaries_dir().join("standalone");
        if !cache_folder.exists() {
            std::fs::create_dir_all(&cache_folder)?;
        }
        #[cfg(not(windows))]
        let mvr_binary_path = cache_folder.join(format!("mvr-{}", version));
        #[cfg(target_os = "windows")]
        let mvr_binary_path = cache_folder.join(format!("mvr-{}.exe", version));

        if mvr_binary_path.exists() {
            println!("Binary mvr-{version} already installed. Use `suiup default set mvr {version}` to set the default version to the desired one");
            return Ok(version);
        }

        if self.releases.is_empty() {
            self.get_releases().await?;
        }

        let release = self
            .releases
            .iter()
            .find(|r| r.tag_name == version)
            .ok_or_else(|| anyhow!("Version {} not found", version))?;

        let (os, arch) = detect_os_arch()?;
        let asset_name = format!("mvr-{}-{}", os, arch);

        #[cfg(target_os = "windows")]
        let asset_name = format!("{}.exe", asset_name);

        let asset = release
            .assets
            .iter()
            .find(|a| a.name.starts_with(&asset_name))
            .ok_or_else(|| anyhow!("No compatible binary found for your system"))?;

        download_file(
            &asset.browser_download_url,
            &mvr_binary_path,
            format!("mvr-{}", version).as_str(),
            None,
        )
        .await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&mvr_binary_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&mvr_binary_path, perms)?;
        }

        Ok(version)
    }
}
