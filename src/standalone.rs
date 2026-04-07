// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    handlers::{
        download::{detect_os_arch, download_file},
        release::ensure_version_prefix,
    },
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

    /// Returns the latest version string (e.g. "v0.6.5") without printing or downloading.
    /// Caller must call `get_releases()` first.
    pub fn latest_version(&self) -> Result<String, Error> {
        let release = self
            .releases
            .first()
            .ok_or_else(|| anyhow!("No releases found for {}", self.repo_slug))?;
        standalone_tag_version(&release.tag_name)
            .ok_or_else(|| anyhow!("Cannot extract version from tag: {}", release.tag_name))
    }

    /// Download the CLI binary, if it does not exist in the binary folder.
    pub async fn download_version(
        &mut self,
        version: Option<String>,
        binary_name_str: &str,
    ) -> Result<String, Error> {
        let explicit_version = version.as_deref().map(normalize_standalone_version);
        let version = if let Some(version) = explicit_version.clone() {
            version
        } else {
            if self.releases.is_empty() {
                self.get_releases().await?;
            }
            let latest_release = self.get_latest_release()?.tag_name.clone();
            println!("No version specified. Downloading latest release: {latest_release}");
            standalone_tag_version(&latest_release).unwrap_or(latest_release)
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
            .find(|release| match explicit_version.as_deref() {
                Some(version) => standalone_tag_matches_version(&release.tag_name, version),
                None => true,
            })
            .ok_or_else(|| anyhow!("Version {} not found", version))?;

        let (os, arch) = detect_os_arch()?;
        let asset_names = standalone_asset_name_candidates(binary_name_str, &os, &arch);
        let asset = asset_names
            .iter()
            .find_map(|asset_name| {
                release
                    .assets
                    .iter()
                    .find(|asset| standalone_asset_name_matches(&asset.name, asset_name))
            })
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

fn normalize_standalone_version(version: &str) -> String {
    ensure_version_prefix(version)
}

fn standalone_tag_version(tag: &str) -> Option<String> {
    if tag.starts_with('v') || tag.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Some(ensure_version_prefix(tag));
    }

    tag.rsplit_once("-v")
        .map(|(_, version)| format!("v{version}"))
}

fn standalone_tag_matches_version(tag: &str, version: &str) -> bool {
    standalone_tag_version(tag)
        .as_deref()
        .is_some_and(|tag_version| tag_version == version)
}

fn standalone_os_candidates(os: &str) -> Vec<&str> {
    match os {
        "ubuntu" => vec!["ubuntu", "linux"],
        "macos" => vec!["macos"],
        "windows" => vec!["windows"],
        _ => vec![os],
    }
}

fn standalone_arch_candidates(arch: &str) -> Vec<&str> {
    match arch {
        "arm64" => vec!["arm64", "aarch64"],
        "aarch64" => vec!["aarch64", "arm64"],
        "x86_64" => vec!["x86_64"],
        _ => vec![arch],
    }
}

fn standalone_asset_name_candidates(binary_name: &str, os: &str, arch: &str) -> Vec<String> {
    let os_candidates = standalone_os_candidates(os);
    let arch_candidates = standalone_arch_candidates(arch);
    let mut candidates = Vec::with_capacity(os_candidates.len() * arch_candidates.len());

    for os_candidate in os_candidates {
        for arch_candidate in &arch_candidates {
            let candidate = format!("{binary_name}-{os_candidate}-{arch_candidate}");
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }
    }

    candidates
}

fn standalone_asset_name_matches(asset_name: &str, candidate: &str) -> bool {
    asset_name.starts_with(candidate)
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
    use super::*;

    #[test]
    fn standalone_installer_stores_github_token() {
        let installer = StandaloneInstaller::new("MystenLabs/mvr", Some("token123".to_string()));
        assert_eq!(installer.github_token.as_deref(), Some("token123"));
    }

    #[test]
    fn standalone_tag_version_normalizes_prefixed_tags() {
        assert_eq!(standalone_tag_version("v0.6.4"), Some("v0.6.4".to_string()));
        assert_eq!(standalone_tag_version("0.6.4"), Some("v0.6.4".to_string()));
        assert_eq!(
            standalone_tag_version("seal-v0.6.4"),
            Some("v0.6.4".to_string())
        );
    }

    #[test]
    fn standalone_tag_matches_requested_version() {
        let version = normalize_standalone_version("0.6.4");

        assert!(standalone_tag_matches_version("v0.6.4", &version));
        assert!(standalone_tag_matches_version("seal-v0.6.4", &version));
        assert!(!standalone_tag_matches_version("seal-v0.6.5", &version));
    }

    #[test]
    fn standalone_asset_name_candidates_include_linux_aliases() {
        assert_eq!(
            standalone_asset_name_candidates("seal", "ubuntu", "x86_64"),
            vec![
                "seal-ubuntu-x86_64".to_string(),
                "seal-linux-x86_64".to_string(),
            ]
        );
    }

    #[test]
    fn standalone_asset_name_candidates_include_arm64_aliases() {
        assert_eq!(
            standalone_asset_name_candidates("seal", "macos", "arm64"),
            vec![
                "seal-macos-arm64".to_string(),
                "seal-macos-aarch64".to_string(),
            ]
        );
    }

    #[test]
    fn standalone_asset_name_matching_keeps_existing_behavior() {
        let candidates = standalone_asset_name_candidates("mvr", "ubuntu", "x86_64");
        assert!(
            candidates
                .iter()
                .any(|candidate| standalone_asset_name_matches("mvr-ubuntu-x86_64", candidate))
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| standalone_asset_name_matches("mvr-linux-x86_64", candidate))
        );

        let candidates = standalone_asset_name_candidates("mvr", "macos", "arm64");
        assert!(
            candidates
                .iter()
                .any(|candidate| standalone_asset_name_matches("mvr-macos-arm64", candidate))
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| standalone_asset_name_matches("mvr-macos-aarch64", candidate))
        );
    }

    #[test]
    fn standalone_asset_name_matching_accepts_windows_extensions() {
        let candidates = standalone_asset_name_candidates("seal", "windows", "x86_64");
        assert!(candidates.iter().any(|candidate| {
            standalone_asset_name_matches("seal-windows-x86_64.exe", candidate)
        }));
    }
}
