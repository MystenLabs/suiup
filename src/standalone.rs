// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// use crate::handle_commands::{binaries_folder, detect_os_arch, download_file};
use crate::{
    handlers::download::{detect_os_arch, download_file},
    paths::binaries_dir,
    types::Repo,
};
use anyhow::{Context, Error, anyhow};
use serde::Deserialize;
use serde::de::DeserializeOwned;

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
    github_token: Option<String>,
}

impl StandaloneInstaller {
    pub fn new(repo: Repo, github_token: Option<String>) -> Self {
        Self {
            releases: Vec::new(),
            repo,
            github_token,
        }
    }

    pub async fn get_releases(&mut self) -> Result<(), Error> {
        let client = reqwest::Client::new();
        let url = format!("https://api.github.com/repos/{}/releases", self.repo);

        if !self.releases.is_empty() {
            return Ok(());
        }

        let mut request = client.get(&url).header("User-Agent", "suiup");
        if let Some(token) = &self.github_token {
            request = request.header("Authorization", format!("token {}", token));
        }

        let response = request
            .send()
            .await
            .with_context(|| format!("Failed to send request to {url}"))?;

        let status = response.status();
        if !status.is_success() {
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
    use crate::types::Repo;

    #[test]
    fn standalone_installer_stores_github_token() {
        let installer = StandaloneInstaller::new(Repo::Mvr, Some("token123".to_string()));
        assert_eq!(installer.github_token.as_deref(), Some("token123"));
    }
}
