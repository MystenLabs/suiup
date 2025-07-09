// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::handlers::release::find_last_release_by_network;
use crate::handlers::version::extract_version_from_release;
use crate::types::Repo;
use crate::{handlers::release::release_list, paths::release_archive_dir, types::Release};
use anyhow::{anyhow, bail, Error};
use futures_util::StreamExt;
use indicatif::{HumanBytes, ProgressBar, ProgressStyle};
use reqwest::{
    header::{HeaderMap, HeaderValue, USER_AGENT},
    Client,
};
use std::{cmp::min, io::Write, path::PathBuf, time::Instant};
use std::fs::File;
use std::io::Read;
use md5::Context;

/// Detects the current OS and architecture
pub fn detect_os_arch() -> Result<(String, String), Error> {
    let os = match whoami::platform() {
        whoami::Platform::Linux => "ubuntu",
        whoami::Platform::Windows => "windows",
        whoami::Platform::MacOS => "macos",
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

    // releases on GitHub are prefixed with `v` before the major.minor.patch version
    let version = if version.starts_with("v") {
        version.to_string()
    } else {
        format!("v{version}")
    };

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
            headers.insert(
                "Authorization",
                HeaderValue::from_str(&format!("token {}", token)).unwrap(),
            );
        }

        let url = format!("https://api.github.com/repos/{repo}/releases/tags/{}", tag);
        let response = client.get(&url).headers(headers).send().await?;

        if !response.status().is_success() {
            bail!("release {tag} not found");
        }

        let release: Release = response.json().await?;
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
    let releases = release_list(&repo, github_token.clone()).await?;

    let (os, arch) = detect_os_arch()?;

    let last_release = find_last_release_by_network(releases.0, network)
        .await
        .ok_or_else(|| anyhow!("Could not find last release"))?;

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
    if let Some(token) = github_token {
        if url.contains("github.com") {
            request = request.header("Authorization", format!("token {}", token));
        }
    }

    let response = request.send().await?;

    let response = response.error_for_status();

    if let Err(ref e) = response {
        bail!("Encountered unexpected error: {e}");
    }

    let response = response.unwrap();

    if !response.status().is_success() {
        return Err(anyhow!("Failed to download: {}", response.status()));
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
        if download_to.metadata()?.len() == total_size {
            // Check md5 if .md5 file exists
            let md5_path = download_to.with_extension("md5");
            if md5_path.exists() {
                let mut file = File::open(download_to)?;
                let mut hasher = Context::new();
                let mut buffer = [0u8; 8192];
                loop {
                    let n = file.read(&mut buffer)?;
                    if n == 0 { break; }
                    hasher.consume(&buffer[..n]);
                }
                let result = hasher.compute();
                let local_md5 = format!("{:x}", result);
                let expected_md5 = std::fs::read_to_string(md5_path)?.trim().to_string();
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
        std::fs::remove_file(download_to)?;
    }

    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("Downloading release: {spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}")
        .unwrap()
        .progress_chars("=>-"));

    let mut file = std::fs::File::create(download_to)?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    let start = Instant::now();

    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk)?;
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
        let mut file = File::open(download_to)?;
        let mut hasher = Context::new();
        let mut buffer = [0u8; 8192];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 { break; }
            hasher.consume(&buffer[..n]);
        }
        let result = hasher.compute();
        let local_md5 = format!("{:x}", result);
        let expected_md5 = std::fs::read_to_string(md5_path)?.trim().to_string();
        if local_md5 != expected_md5 {
            return Err(anyhow!(format!("MD5 check failed for {}: expected {}, got {}", name, expected_md5, local_md5)));
        } else {
            println!("MD5 check passed for {name}");
        }
    }

    Ok(name.to_string())
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
