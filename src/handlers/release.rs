// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::anyhow;
use anyhow::bail;
use anyhow::{Context, Error};
use reqwest::header::ETAG;
use reqwest::header::IF_NONE_MATCH;
use serde::de::DeserializeOwned;

use crate::handlers::version::extract_version_from_release;
use crate::paths::get_suiup_cache_dir;
use crate::types::Release;

/// Fetches the list of releases from the GitHub repository
pub async fn release_list(
    repo_slug: &str,
    github_token: Option<String>,
) -> Result<(Vec<Release>, Option<String>), anyhow::Error> {
    let release_url = format!("https://api.github.com/repos/{}/releases", repo_slug);
    let client = reqwest::Client::new();
    let mut request = client.get(&release_url).header("User-Agent", "suiup");

    // Add authorization header if token is provided
    if let Some(token) = github_token {
        request = request.header("Authorization", format!("token {}", token));
    }

    // Add ETag for caching
    if let Ok(etag) = read_etag_file(repo_slug) {
        request = request.header(IF_NONE_MATCH, etag);
    }

    let response = match request.send().await {
        Ok(response) => response,
        Err(err) => {
            if let Some((releases, etag)) = load_cached_release_list(repo_slug)
                .map_err(|e| anyhow!("Cannot load release list from cache: {e}"))?
            {
                return Ok((releases, Some(etag)));
            }
            return Err(err).with_context(|| format!("Failed to send request to {release_url}"));
        }
    };

    // note this only works with authenticated requests. Should add support for that later.
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        // If nothing has changed, return an empty list and the existing ETag
        if let Some((releases, etag)) = load_cached_release_list(repo_slug)
            .map_err(|e| anyhow!("Cannot load release list from cache: {e}"))?
        {
            return Ok((releases, Some(etag)));
        }
    }

    let etag = response
        .headers()
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let status = response.status();
    if !status.is_success() {
        if let Some((releases, etag)) = load_cached_release_list(repo_slug)
            .map_err(|e| anyhow!("Cannot load release list from cache: {e}"))?
        {
            return Ok((releases, Some(etag)));
        }
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("Unable to read response body: {e}"));
        bail!("GitHub API request failed with status {}: {}", status, body);
    }

    let releases: Vec<Release> =
        parse_json_response(response, &release_url, "GitHub releases list").await?;
    save_release_list(repo_slug, &releases, etag.clone())?;

    Ok((releases, etag))
}

fn read_etag_file(repo_slug: &str) -> Result<String, anyhow::Error> {
    let repo_name = repo_slug.replace("/", "_");
    let filename = format!("etag_{}.txt", repo_name);
    let etag_file = get_suiup_cache_dir().join(filename);
    if etag_file.exists() {
        std::fs::read_to_string(&etag_file)
            .with_context(|| format!("Cannot read ETag file {}", etag_file.display()))
    } else {
        Ok("".to_string())
    }
}

/// Finds the last release for a given network
pub async fn find_last_release_by_network(
    releases: Vec<Release>,
    network: &str,
) -> Option<Release> {
    releases
        .into_iter()
        .find(|r| r.assets.iter().any(|a| a.name.contains(network)))
}

fn save_release_list(
    repo_slug: &str,
    releases: &[Release],
    etag: Option<String>,
) -> Result<(), anyhow::Error> {
    println!("Saving releases list to cache");
    let repo_name = repo_slug.replace("/", "_");
    let etag_filename = format!("etag_{}.txt", repo_name);
    let releases_filename = format!("releases_{}.txt", repo_name);
    let cache_dir = get_suiup_cache_dir();
    std::fs::create_dir_all(&cache_dir)
        .with_context(|| format!("Could not create cache directory {}", cache_dir.display()))?;

    let cache_file = cache_dir.join(releases_filename);
    let etag_file = cache_dir.join(etag_filename);

    let cache_content = serde_json::to_string_pretty(releases)
        .context("Could not serialize GitHub releases for cache file")?;

    std::fs::write(&cache_file, cache_content).with_context(|| {
        format!(
            "Could not write cache releases file {}",
            cache_file.display()
        )
    })?;
    if let Some(etag) = etag {
        std::fs::write(&etag_file, etag)
            .with_context(|| format!("Could not write ETag file {}", etag_file.display()))?;
    }
    Ok(())
}

fn load_cached_release_list(
    repo_slug: &str,
) -> Result<Option<(Vec<Release>, String)>, anyhow::Error> {
    let repo_name = repo_slug.replace("/", "_");
    let etag_filename = format!("etag_{}.txt", repo_name);
    let releases_filename = format!("releases_{}.txt", repo_name);
    let cache_file = get_suiup_cache_dir().join(releases_filename);
    let etag_file = get_suiup_cache_dir().join(etag_filename);

    if cache_file.exists() && etag_file.exists() {
        let raw_cache_content = std::fs::read_to_string(&cache_file)
            .with_context(|| format!("Cannot read cache file {}", cache_file.display()))?;
        let cache_content: Vec<Release> =
            serde_json::from_str(&raw_cache_content).map_err(|e| {
                anyhow!(
                    "Cannot deserialize releases cache file {}: {e}",
                    cache_file.display()
                )
            })?;
        let etag_content = std::fs::read_to_string(&etag_file)
            .with_context(|| format!("Cannot read ETag file {}", etag_file.display()))?;

        Ok(Some((cache_content, etag_content)))
    } else {
        Ok(None)
    }
}

async fn parse_json_response<T>(
    response: reqwest::Response,
    request_url: &str,
    response_name: &str,
) -> Result<T, anyhow::Error>
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

pub async fn last_release_for_network<'a>(
    releases: &'a [Release],
    network: &'a str,
) -> Result<(&'a str, String), Error> {
    if let Some(release) = releases
        .iter()
        .find(|r| r.assets.iter().any(|a| a.name.contains(network)))
    {
        Ok((
            network,
            extract_version_from_release(release.assets[0].name.as_str())?,
        ))
    } else {
        bail!("No release found for {network}")
    }
}

/// Find all networks that have a specific version available
pub fn find_networks_with_version(releases: &[Release], version: &str) -> Vec<String> {
    let version = ensure_version_prefix(version);

    let networks = ["testnet", "devnet", "mainnet"];
    let mut available_networks = Vec::new();

    for network in networks {
        let tag = format!("{}-{}", network, version);
        if releases
            .iter()
            .any(|r| r.assets.iter().any(|a| a.name.contains(&tag)))
        {
            available_networks.push(network.to_string());
        }
    }

    available_networks
}

/// Ensures version has 'v' prefix (adds it if missing)
/// This normalizes towards the GitHub release tag format
pub fn ensure_version_prefix(version: &str) -> String {
    if version.starts_with("v") {
        version.to_string()
    } else {
        format!("v{version}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Asset, Release};

    fn create_test_release(asset_names: Vec<&str>) -> Release {
        Release {
            assets: asset_names
                .into_iter()
                .map(|name| Asset {
                    name: name.to_string(),
                    browser_download_url: format!("https://example.com/{}", name),
                })
                .collect(),
        }
    }

    #[test]
    fn test_find_networks_with_version() {
        let releases = vec![
            create_test_release(vec!["sui-testnet-v1.53.0-linux-x86_64.tgz"]),
            create_test_release(vec!["sui-devnet-v1.53.0-linux-x86_64.tgz"]),
            create_test_release(vec!["sui-testnet-v1.52.0-linux-x86_64.tgz"]),
            create_test_release(vec!["walrus-mainnet-v1.54.0-linux-x86_64.tgz"]),
        ];

        // Test finding version 1.53.0
        let networks = find_networks_with_version(&releases, "1.53.0");
        assert_eq!(networks.len(), 2);
        assert!(networks.contains(&"testnet".to_string()));
        assert!(networks.contains(&"devnet".to_string()));

        // Test finding version with 'v' prefix
        let networks = find_networks_with_version(&releases, "v1.53.0");
        assert_eq!(networks.len(), 2);
        assert!(networks.contains(&"testnet".to_string()));
        assert!(networks.contains(&"devnet".to_string()));

        // Test finding version that doesn't exist
        let networks = find_networks_with_version(&releases, "1.99.0");
        assert!(networks.is_empty());

        // Test finding version that exists only in one network
        let networks = find_networks_with_version(&releases, "1.52.0");
        assert_eq!(networks.len(), 1);
        assert!(networks.contains(&"testnet".to_string()));
    }

    #[test]
    fn test_ensure_version_prefix() {
        assert_eq!(ensure_version_prefix("1.53.0"), "v1.53.0");
        assert_eq!(ensure_version_prefix("v1.53.0"), "v1.53.0");
        assert_eq!(ensure_version_prefix("0.1.2"), "v0.1.2");
        assert_eq!(ensure_version_prefix("v2.0.0"), "v2.0.0");
    }
}
