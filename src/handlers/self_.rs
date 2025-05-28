// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::download::detect_os_arch;

use crate::handlers::download::download_file;
use anyhow::{anyhow, Result};
use std::{fmt::Display, process::Command};

use flate2::read::GzDecoder;
use std::fs::File;
use tar::Archive;

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

    fn same(&self, new: &Self) -> bool {
        self.major == new.major && self.minor == new.minor && self.patch == new.patch
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
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(repo)
        .header("User-Agent", "suiup")
        .send()?
        .json::<serde_json::Value>()?;
    let tag = response["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Failed to parse latest version from GitHub response"))?;

    let latest_version = Ver::from_str(tag)?;

    if current_version.same(&latest_version) {
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

    let filename = if os == "Windows" && arch == "arm64" {
        "suiup-Windows-msvc-arm64.zip".to_string()
    } else {
        format!("suiup-{os}-{arch}.tar.gz")
    };

    Ok(filename)
}
