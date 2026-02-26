// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::download::detect_os_arch;

use crate::handlers::download::download_file;
use anyhow::{Context, Result, anyhow};
use std::{fmt::Display, process::Command};
use tokio::task;

use flate2::read::GzDecoder;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::fs::File;
use tar::Archive;
use zip::ZipArchive;

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
}

pub fn check_for_updates() {
    task::spawn(check_for_updates_impl());
}

async fn check_for_updates_impl() -> Option<()> {
    let current_exe = std::env::current_exe().ok()?;
    let output = std::process::Command::new(current_exe)
        .arg("--version")
        .output()
        .ok()?;

    let version_output = String::from_utf8(output.stdout).ok()?;
    let version = version_output.split_whitespace().nth(1)?;
    let current_version = Ver::from_str(version).ok()?;

    let latest_version = get_latest_version().await.ok()?;

    if current_version < latest_version {
        eprintln!(
            "\n⚠️  A new version of suiup is available: v{} → v{}",
            current_version, latest_version
        );
        eprintln!("   Run 'suiup self update' to update to the latest version.\n");
    }
    Some(())
}

async fn get_latest_version() -> Result<Ver> {
    let client = reqwest::Client::new();
    let url = "https://api.github.com/repos/MystenLabs/suiup/releases/latest";
    let response = client
        .get(url)
        .header("User-Agent", "suiup")
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
            "Failed to fetch latest version from GitHub (status {}) for {}: {}",
            status,
            url,
            body
        ));
    }

    let release: GitHubRelease =
        parse_json_response(response, url, "GitHub latest release").await?;
    Ver::from_str(&release.tag_name)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Ver {
    major: usize,
    minor: usize,
    patch: usize,
}

impl Ver {
    fn from_str(s: &str) -> Result<Self> {
        let mut parts = s.trim_start_matches('v').split('.');
        let (Some(major), Some(minor), Some(patch), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(anyhow::anyhow!("Invalid version format"));
        };

        let major = major.parse::<usize>()?;
        let minor = minor.parse::<usize>()?;
        let patch = patch.parse::<usize>()?;
        Ok(Ver {
            major,
            minor,
            patch,
        })
    }
}

impl Display for Ver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

pub async fn handle_update() -> Result<()> {
    // find the current binary version
    let current_exe = std::env::current_exe()?;
    let version_output = Command::new(&current_exe).arg("--version").output()?.stdout;
    let version_output = String::from_utf8(version_output)?;
    let current_version = version_output
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Failed to parse current version for suiup binary. Please update manually."
            )
        })
        .and_then(Ver::from_str)?;
    let latest_version = get_latest_version().await?;
    let tag = format!("v{latest_version}");

    if current_version == latest_version {
        println!("suiup is already up to date");
        return Ok(());
    }
    println!("Updating to latest version: {}", latest_version);

    // download the latest version from github
    // https://github.com/MystenLabs/suiup/releases/download/v0.0.1/suiup-Linux-musl-x86_64.tar.gz

    let archive_name = find_archive_name()?;
    let url =
        format!("https://github.com/MystenLabs/suiup/releases/download/{tag}/{archive_name}",);

    let temp_dir = tempfile::tempdir()?;
    let archive_path = temp_dir.path().join(&archive_name);
    download_file(&url, &temp_dir.path().join(&archive_name), "suiup", None).await?;

    // extract the archive based on file extension
    if archive_name.ends_with(".zip") {
        // Handle ZIP extraction
        let file = File::open(archive_path.as_path())
            .with_context(|| format!("Cannot open archive file {}", archive_path.display()))?;
        let mut archive = ZipArchive::new(file)
            .with_context(|| format!("Cannot read zip archive {}", archive_path.display()))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).with_context(|| {
                format!(
                    "Cannot read entry at index {} from zip archive {}",
                    i,
                    archive_path.display()
                )
            })?;
            let outpath = temp_dir.path().join(file.name());

            if file.is_dir() {
                std::fs::create_dir_all(&outpath).with_context(|| {
                    format!("Cannot create extraction directory {}", outpath.display())
                })?;
            } else {
                if let Some(parent) = outpath.parent() {
                    std::fs::create_dir_all(parent).with_context(|| {
                        format!(
                            "Cannot create parent directory for extraction {}",
                            parent.display()
                        )
                    })?;
                }
                let mut outfile = File::create(&outpath).with_context(|| {
                    format!("Cannot create extracted file {}", outpath.display())
                })?;
                std::io::copy(&mut file, &mut outfile).with_context(|| {
                    format!("Cannot write extracted file {}", outpath.display())
                })?;
            }
        }
    } else {
        // Handle tar.gz extraction
        let file = File::open(archive_path.as_path())
            .with_context(|| format!("Cannot open archive file {}", archive_path.display()))?;
        let tar = GzDecoder::new(file);
        let mut archive = Archive::new(tar);
        archive.unpack(temp_dir.path()).with_context(|| {
            format!(
                "Cannot unpack archive file {} into {}",
                archive_path.display(),
                temp_dir.path().display()
            )
        })?;
    }

    #[cfg(not(windows))]
    let binary = "suiup";
    #[cfg(windows)]
    let binary = "suiup.exe";

    // replace the current binary with the new one
    let binary_path = temp_dir.path().join(binary);
    std::fs::copy(&binary_path, &current_exe).with_context(|| {
        format!(
            "Cannot replace current executable {} with {}",
            current_exe.display(),
            binary_path.display()
        )
    })?;

    println!("suiup updated to version {}", latest_version);
    // cleanup
    temp_dir.close()?;
    Ok(())
}

pub fn handle_uninstall() -> Result<()> {
    let current_exe = std::env::current_exe()?;
    if current_exe.exists() {
        std::fs::remove_file(&current_exe).with_context(|| {
            format!(
                "Cannot remove installed executable {}",
                current_exe.display()
            )
        })?;
        println!("suiup uninstalled");
    } else {
        println!("suiup is not installed");
    }
    Ok(())
}

async fn parse_json_response<T>(
    response: reqwest::Response,
    request_url: &str,
    response_name: &str,
) -> Result<T>
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

fn find_archive_name() -> Result<String> {
    let (os, arch) = detect_os_arch()?;

    let os = match os.as_str() {
        "ubuntu" => "Linux-musl",
        "linux" => "Linux-musl",
        "windows" => "Windows",
        "macos" => "macOS",
        _ => &os,
    };

    let arch = match arch.as_str() {
        "x86_64" => "x86_64",
        "aarch64" => "arm64",
        _ => &arch,
    };

    #[cfg(not(target_os = "windows"))]
    let filename = format!("suiup-{os}-{arch}.tar.gz");
    #[cfg(target_os = "windows")]
    let filename = format!("suiup-{os}-msvc-{arch}.zip");

    Ok(filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ver_from_str_valid_versions() {
        // Test basic version parsing
        let v1 = Ver::from_str("1.2.3").unwrap();
        assert_eq!(v1.major, 1);
        assert_eq!(v1.minor, 2);
        assert_eq!(v1.patch, 3);

        // Test version with 'v' prefix
        let v2 = Ver::from_str("v1.2.3").unwrap();
        assert_eq!(v2.major, 1);
        assert_eq!(v2.minor, 2);
        assert_eq!(v2.patch, 3);

        // Test zero versions
        let v3 = Ver::from_str("0.0.0").unwrap();
        assert_eq!(v3.major, 0);
        assert_eq!(v3.minor, 0);
        assert_eq!(v3.patch, 0);

        // Test larger version numbers
        let v4 = Ver::from_str("v10.20.30").unwrap();
        assert_eq!(v4.major, 10);
        assert_eq!(v4.minor, 20);
        assert_eq!(v4.patch, 30);
    }

    #[test]
    fn test_ver_from_str_invalid_versions() {
        // Test invalid formats
        assert!(Ver::from_str("1.2").is_err());
        assert!(Ver::from_str("1.2.3.4").is_err());
        assert!(Ver::from_str("1").is_err());
        assert!(Ver::from_str("").is_err());
        assert!(Ver::from_str("a.b.c").is_err());
        assert!(Ver::from_str("1.a.3").is_err());
        assert!(Ver::from_str("v1.2.c").is_err());
    }

    #[test]
    fn test_ver_equality() {
        let v1 = Ver::from_str("1.2.3").unwrap();
        let v2 = Ver::from_str("v1.2.3").unwrap();
        let v3 = Ver::from_str("1.2.4").unwrap();

        assert_eq!(v1, v2);
        assert_eq!(v2, v1);
        assert_ne!(v1, v3);
        assert_ne!(v3, v1);
    }

    #[test]
    fn test_ver_ordering() {
        // Test major version differences
        let v1_0_0 = Ver::from_str("1.0.0").unwrap();
        let v2_0_0 = Ver::from_str("2.0.0").unwrap();
        assert!(v1_0_0 < v2_0_0);
        assert!(v2_0_0 > v1_0_0);

        // Test minor version differences
        let v1_1_0 = Ver::from_str("1.1.0").unwrap();
        let v1_2_0 = Ver::from_str("1.2.0").unwrap();
        assert!(v1_1_0 < v1_2_0);
        assert!(v1_2_0 > v1_1_0);

        // Test patch version differences
        let v1_1_1 = Ver::from_str("1.1.1").unwrap();
        let v1_1_2 = Ver::from_str("1.1.2").unwrap();
        assert!(v1_1_1 < v1_1_2);
        assert!(v1_1_2 > v1_1_1);

        // Test same versions
        let v1 = Ver::from_str("1.2.3").unwrap();
        let v2 = Ver::from_str("v1.2.3").unwrap();
        assert!(v1 <= v2);
        assert!(v1 >= v2);

        // Test complex comparisons
        let v0_0_4 = Ver::from_str("0.0.4").unwrap();
        let v0_0_3 = Ver::from_str("0.0.3").unwrap();
        assert!(v0_0_3 < v0_0_4);
        assert!(v0_0_4 > v0_0_3);

        // Test the specific case from the bug report
        let current = Ver::from_str("0.0.4").unwrap();
        let latest = Ver::from_str("0.0.3").unwrap();
        assert!(current >= latest); // Current is newer, should not show warning
        assert!(latest < current); // Latest is older than current
    }

    #[test]
    fn test_ver_display() {
        let v1 = Ver::from_str("1.2.3").unwrap();
        assert_eq!(format!("{}", v1), "1.2.3");

        let v2 = Ver::from_str("v10.20.30").unwrap();
        assert_eq!(format!("{}", v2), "10.20.30");

        let v3 = Ver::from_str("0.0.0").unwrap();
        assert_eq!(format!("{}", v3), "0.0.0");
    }
}
