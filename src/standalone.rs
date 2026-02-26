// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    handlers::download::{detect_os_arch, download_file},
    paths::{binaries_dir, get_suiup_cache_dir},
};
use anyhow::{Context, Error, anyhow};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct StandaloneRelease {
    pub tag_name: String,
    pub assets: Vec<StandaloneAsset>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct StandaloneAsset {
    pub name: String,
    pub browser_download_url: String,
}

pub struct StandaloneInstaller {
    releases: Vec<StandaloneRelease>,
    repo_slug: String,
    github_token: Option<String>,
}

impl StandaloneInstaller {
    pub fn new(repo_slug: &str, github_token: Option<String>) -> Self {
        Self {
            releases: Vec::new(),
            repo_slug: repo_slug.to_string(),
            github_token,
        }
    }

    pub async fn get_releases(&mut self) -> Result<(), Error> {
        let client = reqwest::Client::new();
        let url = format!("https://api.github.com/repos/{}/releases", self.repo_slug);

        if !self.releases.is_empty() {
            return Ok(());
        }

        let mut request = client.get(&url).header("User-Agent", "suiup");
        if let Some(token) = &self.github_token {
            request = request.header("Authorization", format!("token {}", token));
        }

        let response = match request.send().await {
            Ok(response) => response,
            Err(err) => {
                if let Some(cached) = load_cached_standalone_releases(&self.repo_slug)? {
                    self.releases = cached;
                    return Ok(());
                }
                return Err(err).with_context(|| format!("Failed to send request to {url}"));
            }
        };

        let status = response.status();
        if !status.is_success() {
            if let Some(cached) = load_cached_standalone_releases(&self.repo_slug)? {
                self.releases = cached;
                return Ok(());
            }
            let body = response
                .text()
                .await
                .unwrap_or_else(|e| format!("Unable to read response body: {e}"));
            return Err(anyhow!(
                "GitHub API request failed with status {} for {}: {}",
                status,
                url,
                body
            ));
        }

        let releases: Vec<StandaloneRelease> =
            parse_json_response(response, &url, "GitHub releases list").await?;
        save_cached_standalone_releases(&self.repo_slug, &releases)?;
        self.releases = releases;
        Ok(())
    }

    pub fn get_latest_release(&self) -> Result<&StandaloneRelease, Error> {
        println!("Downloading release list");
        let releases = &self.releases;
        releases
            .first()
            .ok_or_else(|| anyhow!("No releases found for {}", self.repo_slug))
    }

    /// Download the CLI binary, if it does not exist in the binary folder.
    pub async fn download_version(
        &mut self,
        version: Option<String>,
        binary_name_str: &String,
    ) -> Result<String, Error> {
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
            std::fs::create_dir_all(&cache_folder).with_context(|| {
                format!("Cannot create cache directory {}", cache_folder.display())
            })?;
        }
        #[cfg(not(windows))]
        let standalone_binary_path = cache_folder.join(format!("{}-{}", binary_name_str, version));
        #[cfg(target_os = "windows")]
        let standalone_binary_path =
            cache_folder.join(format!("{}-{}.exe", binary_name_str, version));

        if standalone_binary_path.exists() {
            println!(
                "Binary {}-{version} already installed. Use `suiup default set standalone {version}` to set the default version to the desired one",
                binary_name_str
            );
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
        let asset_name = format!("{}-{}-{}", binary_name_str, os, arch);

        #[cfg(target_os = "windows")]
        let asset_name = format!("{}.exe", asset_name);

        let asset = release
            .assets
            .iter()
            .find(|a| a.name.starts_with(&asset_name))
            .ok_or_else(|| {
                anyhow!(
                    "No compatible binary found for your system: {}-{}",
                    os,
                    arch
                )
            })?;

        download_file(
            &asset.browser_download_url,
            &standalone_binary_path,
            format!("{}-{version}", binary_name_str).as_str(),
            self.github_token.clone(),
        )
        .await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&standalone_binary_path)
                .with_context(|| {
                    format!(
                        "Cannot read metadata for binary {}",
                        standalone_binary_path.display()
                    )
                })?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&standalone_binary_path, perms).with_context(|| {
                format!(
                    "Cannot set executable permissions on {}",
                    standalone_binary_path.display()
                )
            })?;
        }

        Ok(version)
    }
}

fn standalone_releases_cache_file(repo_slug: &str) -> std::path::PathBuf {
    let sanitized = repo_slug.replace('/', "_");
    get_suiup_cache_dir().join(format!("standalone_releases_{sanitized}.json"))
}

fn load_cached_standalone_releases(
    repo_slug: &str,
) -> Result<Option<Vec<StandaloneRelease>>, Error> {
    let cache_file = standalone_releases_cache_file(repo_slug);
    if !cache_file.exists() {
        return Ok(None);
    }

    let raw = std::fs::read_to_string(&cache_file)
        .with_context(|| format!("Cannot read standalone cache file {}", cache_file.display()))?;
    let releases = serde_json::from_str(&raw).with_context(|| {
        format!(
            "Cannot deserialize standalone cache file {}",
            cache_file.display()
        )
    })?;
    Ok(Some(releases))
}

fn save_cached_standalone_releases(
    repo_slug: &str,
    releases: &[StandaloneRelease],
) -> Result<(), Error> {
    let cache_file = standalone_releases_cache_file(repo_slug);
    if let Some(parent) = cache_file.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Cannot create standalone cache directory {}",
                parent.display()
            )
        })?;
    }

    let payload = serde_json::to_string_pretty(releases)
        .context("Cannot serialize standalone releases cache payload")?;
    std::fs::write(&cache_file, payload).with_context(|| {
        format!(
            "Cannot write standalone cache file {}",
            cache_file.display()
        )
    })?;
    Ok(())
}

async fn parse_json_response<T>(
    response: reqwest::Response,
    request_url: &str,
    response_name: &str,
) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let response_body = response
        .text()
        .await
        .with_context(|| format!("Cannot read {response_name} response body from {request_url}"))?;

    serde_json::from_str(&response_body).map_err(|e| {
        anyhow!(
            "Failed to deserialize {response_name} response from {request_url}: {e}\nResponse body:\n{response_body}"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::StandaloneInstaller;

    #[test]
    fn standalone_installer_stores_github_token() {
        let installer = StandaloneInstaller::new("MystenLabs/mvr", Some("token123".to_string()));
        assert_eq!(installer.github_token.as_deref(), Some("token123"));
    }
}
