// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::download::detect_os_arch;

use crate::handlers::download::download_file;
use anyhow::{anyhow, Result};
use std::{fmt::Display, process::Command};
use tokio::task;

use flate2::read::GzDecoder;
use serde::Deserialize;
use std::fs::File;
use tar::Archive;

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
    let response = client
        .get("https://api.github.com/repos/MystenLabs/suiup/releases/latest")
        .header("User-Agent", "suiup")
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow!("Failed to fetch latest version from GitHub"));
    }

    let release: GitHubRelease = response.json().await?;
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
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(anyhow::anyhow!("Invalid version format"));
        }
        let major = if parts[0].starts_with('v') {
            parts[0][1..].parse::<usize>()?
        } else {
            parts[0].parse::<usize>()?
        };

        let minor = parts[1].parse::<usize>()?;
        let patch = parts[2].parse::<usize>()?;
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
    let current_version = Command::new(&current_exe).arg("--version").output()?.stdout;
    let current_version = String::from_utf8(current_version)?.trim().to_string();

    if current_version.is_empty() {
        return Err(anyhow::anyhow!(
            "Failed to get current version for suiup binary. Please update manually."
        ));
    }

    let split = current_version.split(" ").collect::<Vec<_>>();

    if split.len() != 2 {
        return Err(anyhow::anyhow!(
            "Failed to parse current version for suiup binary. Please update manually."
        ));
    }

    let current_version = Ver::from_str(split[1])?;

    // find the latest version on github in releases
    let repo = "https://api.github.com/repos/MystenLabs/suiup/releases/latest";
    let client = reqwest::Client::new();
    let response = client
        .get(repo)
        .header("User-Agent", "suiup")
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;
    let tag = response["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Failed to parse latest version from GitHub response"))?;

    let latest_version = Ver::from_str(tag)?;

    if current_version == latest_version {
        println!("suiup is already up to date");
        return Ok(());
    } else {
        println!("Updating to latest version: {}", latest_version);
    }

    // download the latest version from github
    // https://github.com/MystenLabs/suiup/releases/download/v0.0.1/suiup-Linux-musl-x86_64.tar.gz

    let archive_name = find_archive_name()?;
    let url =
        format!("https://github.com/MystenLabs/suiup/releases/download/{tag}/{archive_name}",);

    let temp_dir = tempfile::tempdir()?;
    let archive_path = temp_dir.path().join(&archive_name);
    download_file(&url, &temp_dir.path().join(archive_name), "suiup", None).await?;

    // extract the archive
    let file = File::open(archive_path.as_path())
        .map_err(|_| anyhow!("Cannot open archive file: {}", archive_path.display()))?;
    let tar = GzDecoder::new(file);
    let mut archive = Archive::new(tar);
    archive
        .unpack(temp_dir.path())
        .map_err(|_| anyhow!("Cannot unpack archive file: {}", archive_path.display()))?;

    #[cfg(not(windows))]
    let binary = "suiup";
    #[cfg(windows)]
    let binary = "suiup.exe";

    // replace the current binary with the new one
    let binary_path = temp_dir.path().join(binary);
    std::fs::copy(binary_path, current_exe)?;

    println!("suiup updated to version {}", latest_version);
    // cleanup
    temp_dir.close()?;
    Ok(())
}

pub fn handle_uninstall() -> Result<()> {
    let current_exe = std::env::current_exe()?;
    if current_exe.exists() {
        std::fs::remove_file(current_exe)?;
        println!("suiup uninstalled");
    } else {
        println!("suiup is not installed");
    }
    Ok(())
}

fn find_archive_name() -> Result<String> {
    let (os, arch) = detect_os_arch()?;

    let os = match os.as_str() {
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
        assert!(!(v1 < v2));
        assert!(!(v1 > v2));

        // Test complex comparisons
        let v0_0_4 = Ver::from_str("0.0.4").unwrap();
        let v0_0_3 = Ver::from_str("0.0.3").unwrap();
        assert!(v0_0_3 < v0_0_4);
        assert!(v0_0_4 > v0_0_3);

        // Test the specific case from the bug report
        let current = Ver::from_str("0.0.4").unwrap();
        let latest = Ver::from_str("0.0.3").unwrap();
        assert!(!(current < latest)); // Current is newer, should not show warning
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
