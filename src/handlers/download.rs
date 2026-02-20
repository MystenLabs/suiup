// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::handlers::release::{
    ensure_version_prefix, find_last_release_by_network, find_networks_with_version,
};
use crate::handlers::version::extract_version_from_release;
use crate::types::Repo;
use crate::{handlers::release::release_list, paths::release_archive_dir, types::Release};
use anyhow::{Context, Error, anyhow, bail};
use futures_util::StreamExt;
use indicatif::{HumanBytes, ProgressBar, ProgressStyle};
use md5::Context as Md5Context;
use reqwest::{
    Client,
    header::{HeaderMap, HeaderValue, USER_AGENT},
};
use serde::de::DeserializeOwned;
use std::fs::File;
use std::io::Read;
use std::{cmp::min, io::Write, path::PathBuf, time::Instant};

use tracing::debug;

/// Generate helpful error message with network suggestions
/// Note: This is only applicable for sui and walrus. MVR binary is standalone, not tied to a network.
fn generate_network_suggestions_error(
    repo: &Repo,
    releases: &[Release],
    version: Option<&str>,
    requested_network: &str,
) -> anyhow::Error {
    let binary_name = repo.binary_name();

    // MVR is a standalone binary, not tied to networks like sui and walrus
    if matches!(repo, Repo::Mvr) {
        if let Some(version) = version {
            return anyhow!(
                "{binary_name} version {version} not found. {binary_name} is a standalone binary \
                - try: suiup install {binary_name} {version}",
            );
        } else {
            return anyhow!(
                "{binary_name} release not found. {binary_name} is a standalone binary \
                - try: suiup install {binary_name}"
            );
        }
    }

    if let Some(version) = version {
        // For specific version requests, check if version exists in other networks
        let available_networks = find_networks_with_version(releases, version);

        if !available_networks.is_empty() {
            let suggestions: Vec<String> = available_networks
                .iter()
                .map(|net| format!("suiup install {}@{}-{}", binary_name, net, version))
                .collect();

            anyhow!(
                "Release {}-{} not found. However, version {} is available for other networks:\n\nTry one of these commands:\n  {}",
                requested_network,
                version,
                version,
                suggestions.join("\n  ")
            )
        } else {
            anyhow!("Release {}-{} not found", requested_network, version)
        }
    } else {
        // For latest release requests, check what networks are available
        let available_networks: Vec<String> = ["testnet", "devnet", "mainnet"]
            .iter()
            .filter(|&net| {
                releases
                    .iter()
                    .any(|r| r.assets.iter().any(|a| a.name.contains(net)))
            })
            .map(|s| s.to_string())
            .collect();

        if !available_networks.is_empty() {
            let suggestions: Vec<String> = available_networks
                .iter()
                .map(|net| format!("suiup install {}@{}", binary_name, net))
                .collect();

            anyhow!(
                "No releases found for {} network. Available networks:\n\nTry one of these commands:\n  {}",
                requested_network,
                suggestions.join("\n  ")
            )
        } else {
            anyhow!("Could not find any releases")
        }
    }
}

/// Detects the current OS and architecture
pub fn detect_os_arch() -> Result<(String, String), Error> {
    let os = match whoami::platform() {
        whoami::Platform::Linux => "ubuntu",
        whoami::Platform::Windows => "windows",
        whoami::Platform::Mac => "macos",
        _ => bail!("Unsupported OS. Supported only: Linux, Windows, MacOS"),
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" if os == "macos" => "arm64",
        "aarch64" => "aarch64",
        _ => bail!("Unsupported architecture. Supported only: x86_64, aarch64"),
    };

    println!("Detected: {os}-{arch}...");
    Ok((os.to_string(), arch.to_string()))
}

/// Downloads a release with a specific version
/// The network is used to filter the release
pub async fn download_release_at_version(
    repo: Repo,
    network: &str,
    version: &str,
    github_token: Option<String>,
) -> Result<String, anyhow::Error> {
    let (os, arch) = detect_os_arch()?;

    // Ensure version has 'v' prefix for GitHub release tags
    let version = ensure_version_prefix(version);

    let tag = format!("{}-{}", network, version);

    println!("Searching for release with tag: {}...", tag);
    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();

    let releases = release_list(&repo, github_token.clone()).await?.0;

    if let Some(release) = releases
        .iter()
        .find(|r| r.assets.iter().any(|a| a.name.contains(&tag)))
    {
        download_asset_from_github(release, &os, &arch, github_token).await
    } else {
        headers.insert(USER_AGENT, HeaderValue::from_static("suiup"));

        // Add authorization header if token is provided
        if let Some(token) = &github_token {
            let auth_header = HeaderValue::from_str(&format!("token {}", token))
                .map_err(|e| anyhow!("Invalid GitHub token for Authorization header: {e}"))?;
            headers.insert("Authorization", auth_header);
        }

        let url = format!("https://api.github.com/repos/{repo}/releases/tags/{}", tag);
        let response = client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .with_context(|| format!("Failed to send request to {url}"))?;

        if !response.status().is_success() {
            return Err(generate_network_suggestions_error(
                &repo,
                &releases,
                Some(&version),
                network,
            ));
        }

        let release: Release = parse_json_response(response, &url, "GitHub release").await?;
        download_asset_from_github(&release, &os, &arch, github_token).await
    }
}

/// Downloads the latest release for a given network
pub async fn download_latest_release(
    repo: Repo,
    network: &str,
    github_token: Option<String>,
) -> Result<String, anyhow::Error> {
    println!("Downloading release list");
    debug!("Downloading release list for repo: {repo} and network: {network}");
    let releases = release_list(&repo, github_token.clone()).await?;

    let (os, arch) = detect_os_arch()?;

    let last_release = find_last_release_by_network(releases.0.clone(), network)
        .await
        .ok_or_else(|| generate_network_suggestions_error(&repo, &releases.0, None, network))?;

    println!(
        "Last {network} release: {}",
        extract_version_from_release(&last_release.assets[0].name)?
    );

    download_asset_from_github(&last_release, &os, &arch, github_token).await
}

pub async fn download_file(
    url: &str,
    download_to: &PathBuf,
    name: &str,
    github_token: Option<String>,
) -> Result<String, Error> {
    let client = Client::new();

    // Start with a basic request
    let mut request = client.get(url).header("User-Agent", "suiup");

    // Add authorization header if token is provided and the URL is from GitHub
    if let Some(token) = github_token
        && url.contains("github.com")
    {
        request = request.header("Authorization", format!("token {}", token));
    }

    let response = request
        .send()
        .await
        .with_context(|| format!("Failed to send download request to {url}"))?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("Unable to read response body: {e}"));
        bail!("Failed to download (status {}): {}", status, body);
    }

    let mut total_size = response.content_length().unwrap_or(0);
    //walrus is on google storage, so different content length header
    if total_size == 0 {
        total_size = response
            .headers()
            .get("x-goog-stored-content-length")
            .and_then(|c| c.to_str().ok())
            .and_then(|c| c.parse::<u64>().ok())
            .unwrap_or(0);
    }

    if download_to.exists() {
        if download_to
            .metadata()
            .with_context(|| {
                format!(
                    "Cannot read metadata for existing file {}",
                    download_to.display()
                )
            })?
            .len()
            == total_size
        {
            // Check md5 if .md5 file exists
            let md5_path = download_to.with_extension("md5");
            if md5_path.exists() {
                let mut file = File::open(download_to).with_context(|| {
                    format!("Cannot open file for MD5 check {}", download_to.display())
                })?;
                let mut hasher = Md5Context::new();
                let mut buffer = [0u8; 8192];
                loop {
                    let n = file.read(&mut buffer).with_context(|| {
                        format!("Cannot read file for MD5 check {}", download_to.display())
                    })?;
                    if n == 0 {
                        break;
                    }
                    hasher.consume(&buffer[..n]);
                }
                let result = hasher.finalize();
                let local_md5 = format!("{:x}", result);
                let expected_md5 = std::fs::read_to_string(&md5_path)
                    .with_context(|| format!("Cannot read MD5 file {}", md5_path.display()))?
                    .trim()
                    .to_string();
                if local_md5 == expected_md5 {
                    println!("Found {name} in cache, md5 verified");
                    return Ok(name.to_string());
                } else {
                    println!("MD5 mismatch for {name}, re-downloading...");
                }
            } else {
                println!("Found {name} in cache (no md5 to check)");
                return Ok(name.to_string());
            }
        }
        std::fs::remove_file(download_to).with_context(|| {
            format!(
                "Cannot remove stale cached file before re-download {}",
                download_to.display()
            )
        })?;
    }

    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("Downloading release: {spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}")
        .unwrap()
        .progress_chars("=>-"));

    let mut file = std::fs::File::create(download_to)
        .with_context(|| format!("Cannot create download file {}", download_to.display()))?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    let start = Instant::now();

    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk)
            .with_context(|| format!("Cannot write to download file {}", download_to.display()))?;
        let new = min(downloaded + (chunk.len() as u64), total_size);
        downloaded = new;
        pb.set_position(new);

        let elapsed = start.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            let speed = downloaded as f64 / elapsed;
            pb.set_message(format!("Speed: {}/s", HumanBytes(speed as u64)));
        }
    }

    pb.finish_with_message("Download complete");

    // After download, check md5 if .md5 file exists
    let md5_path = download_to.with_extension("md5");
    if md5_path.exists() {
        let mut file = File::open(download_to).with_context(|| {
            format!(
                "Cannot open downloaded file for MD5 check {}",
                download_to.display()
            )
        })?;
        let mut hasher = Md5Context::new();
        let mut buffer = [0u8; 8192];
        loop {
            let n = file.read(&mut buffer).with_context(|| {
                format!(
                    "Cannot read downloaded file for MD5 check {}",
                    download_to.display()
                )
            })?;
            if n == 0 {
                break;
            }
            hasher.consume(&buffer[..n]);
        }
        let result = hasher.finalize();
        let local_md5 = format!("{:x}", result);
        let expected_md5 = std::fs::read_to_string(&md5_path)
            .with_context(|| format!("Cannot read MD5 file {}", md5_path.display()))?
            .trim()
            .to_string();
        if local_md5 != expected_md5 {
            return Err(anyhow!(format!(
                "MD5 check failed for {}: expected {}, got {}",
                name, expected_md5, local_md5
            )));
        } else {
            println!("MD5 check passed for {name}");
        }
    }

    Ok(name.to_string())
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

/// Downloads the archived release from GitHub and returns the file name
/// The `network, os, and arch` parameters are used to retrieve the correct release for the target
/// architecture and OS
async fn download_asset_from_github(
    release: &Release,
    os: &str,
    arch: &str,
    github_token: Option<String>,
) -> Result<String, anyhow::Error> {
    let asset = release
        .assets
        .iter()
        .find(|&a| a.name.contains(arch) && a.name.contains(os.to_string().to_lowercase().as_str()))
        .ok_or_else(|| anyhow!("Asset not found for {os}-{arch}"))?;

    let url = asset.clone().browser_download_url;
    let name = asset.clone().name;
    let path = release_archive_dir();
    let mut file_path = path.clone();
    file_path.push(&asset.name);

    download_file(&url, &file_path, &name, github_token).await
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
    fn test_binary_name() {
        assert_eq!(Repo::Sui.binary_name(), "sui");
        assert_eq!(Repo::Walrus.binary_name(), "walrus");
        assert_eq!(Repo::Mvr.binary_name(), "mvr");
    }

    #[test]
    fn test_generate_network_suggestions_error_with_version() {
        let releases = vec![
            create_test_release(vec!["sui-devnet-v1.53.0-linux-x86_64.tgz"]),
            create_test_release(vec!["sui-mainnet-v1.53.0-linux-x86_64.tgz"]),
        ];

        let error =
            generate_network_suggestions_error(&Repo::Sui, &releases, Some("1.53.0"), "testnet");
        let error_msg = error.to_string();

        assert!(error_msg.contains("Release testnet-1.53.0 not found"));
        assert!(error_msg.contains("version 1.53.0 is available for other networks"));
        assert!(error_msg.contains("suiup install sui@devnet-1.53.0"));
        assert!(error_msg.contains("suiup install sui@mainnet-1.53.0"));
    }

    #[test]
    fn test_generate_network_suggestions_error_without_version() {
        let releases = vec![
            create_test_release(vec!["sui-devnet-v1.53.0-linux-x86_64.tgz"]),
            create_test_release(vec!["walrus-mainnet-v1.54.0-linux-x86_64.tgz"]),
        ];

        let error = generate_network_suggestions_error(&Repo::Sui, &releases, None, "testnet");
        let error_msg = error.to_string();

        assert!(error_msg.contains("No releases found for testnet network"));
        assert!(error_msg.contains("Available networks"));
        assert!(error_msg.contains("suiup install sui@devnet"));
        assert!(error_msg.contains("suiup install sui@mainnet"));
    }

    #[test]
    fn test_generate_network_suggestions_error_mvr_with_version() {
        let releases = vec![];
        let error =
            generate_network_suggestions_error(&Repo::Mvr, &releases, Some("1.0.0"), "standalone");
        let error_msg = error.to_string();

        assert!(error_msg.contains("mvr version 1.0.0 not found"));
        assert!(error_msg.contains("mvr is a standalone binary"));
        assert!(error_msg.contains("suiup install mvr 1.0.0"));
    }

    #[test]
    fn test_generate_network_suggestions_error_mvr_without_version() {
        let releases = vec![];
        let error = generate_network_suggestions_error(&Repo::Mvr, &releases, None, "standalone");
        let error_msg = error.to_string();

        assert!(error_msg.contains("mvr release not found"));
        assert!(error_msg.contains("mvr is a standalone binary"));
        assert!(error_msg.contains("suiup install mvr"));
    }
}
