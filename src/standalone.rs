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
pub struct StandaloneRelease {
    pub tag_name: String,
    pub assets: Vec<StandaloneAsset>,
}

#[derive(Deserialize, Debug)]
pub struct StandaloneAsset {
    pub name: String,
    pub browser_download_url: String,
}

pub struct StandaloneInstaller {
    releases: Vec<StandaloneRelease>,
    repo: Repo,
}

impl StandaloneInstaller {
    pub fn new(repo: Repo) -> Self {
        Self {
            releases: Vec::new(),
            repo,
        }
    }

    pub async fn get_releases(&mut self) -> Result<(), Error> {
        let client = reqwest::Client::new();
        let url = format!("https://api.github.com/repos/{}/releases", self.repo);

        if !self.releases.is_empty() {
            return Ok(());
        }

        let releases: Vec<StandaloneRelease> = client
            .get(&url)
            .header("User-Agent", "suiup")
            .send()
            .await?
            .json()
            .await?;
        self.releases = releases;
        Ok(())
    }

    pub fn get_latest_release(&self) -> Result<&StandaloneRelease, Error> {
        println!("Downloading release list");
        let releases = &self.releases;
        releases
            .first()
            .ok_or_else(|| anyhow!("No {} releases found", self.repo.binary_name()))
    }

    /// Download the CLI binary, if it does not exist in the binary folder.
    pub async fn download_version(&mut self, version: Option<String>) -> Result<String, Error> {
        let version = if let Some(v) = version {
            // Ensure version has 'v' prefix for GitHub release tags
            crate::handlers::release::ensure_version_prefix(&v)
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
        let standalone_binary_path = cache_folder.join(format!("{}-{}", self.repo.binary_name(), version));
        #[cfg(target_os = "windows")]
        let standalone_binary_path = cache_folder.join(format!("{}-{}.exe", self.repo.binary_name(),version));

        if standalone_binary_path.exists() {
            println!("Binary {}-{version} already installed. Use `suiup default set standalone {version}` to set the default version to the desired one", self.repo.binary_name());
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
        let asset_name = format!("{}-{}-{}", self.repo.binary_name(), os, arch);

        #[cfg(target_os = "windows")]
        let asset_name = format!("{}.exe", asset_name);

        let asset = release
            .assets
            .iter()
            .find(|a| a.name.starts_with(&asset_name))
            .ok_or_else(|| anyhow!("No compatible binary found for your system: {}-{}", os, arch))?;

        download_file(
            &asset.browser_download_url,
            &standalone_binary_path,
            format!("{}-{version}", self.repo.binary_name()).as_str(),
            None,
        )
        .await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&standalone_binary_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&standalone_binary_path, perms)?;
        }

        Ok(version)
    }
}
