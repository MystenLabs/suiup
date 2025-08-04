// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;
use std::process::{Command, Stdio};

use super::check_if_binaries_exist;
use super::version::extract_version_from_release;
use crate::handlers::download::{download_latest_release, download_release_at_version};
use crate::handlers::{extract_component, update_after_install};
use crate::mvr;
use crate::paths::binaries_dir;
use crate::types::{BinaryVersion, InstalledBinaries, Repo};
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Error;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

pub fn install_binary(
    name: &str,
    network: String,
    version: &str,
    debug: bool,
    binary_path: PathBuf,
    yes: bool,
) -> Result<(), Error> {
    let mut installed_binaries = InstalledBinaries::new()?;
    installed_binaries.add_binary(BinaryVersion {
        binary_name: name.to_string(),
        network_release: network.clone(),
        version: version.to_string(),
        debug,
        path: Some(binary_path.to_string_lossy().to_string()),
    });
    installed_binaries.save_to_file()?;
    update_after_install(&vec![name.to_string()], network, version, debug, yes)?;
    Ok(())
}

// this is used for sui mostly
pub async fn install_from_release(
    name: &str,
    network: &str,
    version_spec: Option<String>,
    debug: bool,
    yes: bool,
    repo: Repo,
    github_token: Option<String>,
) -> Result<(), Error> {
    let filename = match version_spec {
        Some(version) => {
            download_release_at_version(repo, network, &version, github_token.clone()).await?
        }
        None => download_latest_release(repo, network, github_token.clone()).await?,
    };

    let version = extract_version_from_release(&filename)?;
    use crate::config::get_binary_config;
    let bin_config = get_binary_config(name)?;
    
    let binary_name = if debug && bin_config.supports_debug {
        format!("{}-debug", name)
    } else {
        name.to_string()
    };

    if !check_if_binaries_exist(&binary_name, network.to_string(), &version)? {
        println!("Adding binary: {name}-{version}");
        extract_component(&binary_name, network.to_string(), &filename)?;

        let binary_filename = format!("{}-{}", name, version);
        #[cfg(target_os = "windows")]
        let binary_filename = format!("{}.exe", binary_filename);

        let binary_path = binaries_dir().join(network).join(binary_filename);
        install_binary(name, network.to_string(), &version, debug, binary_path, yes)?;
    } else {
        println!("Binary {name}-{version} already installed. Use `suiup default set` to change the default binary.");
    }
    Ok(())
}

/// Compile the code from the main branch or the specified branch.
/// It checks if cargo is installed.
pub async fn install_from_nightly(
    name: &str,
    branch: &str,
    debug: bool,
    yes: bool,
) -> Result<(), Error> {
    println!("Installing {name} from {branch} branch");
    check_cargo_rust_installed()?;

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg}")
            .unwrap()
            .tick_strings(&["-", "\\", "|", "/"]),
    );
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_message("Compiling...please wait");

    use crate::config::{get_config, get_binary_config};
    let config = get_config()?;
    let bin_config = get_binary_config(name)?;
    let repo_url = config.get_repo_url(name)
        .ok_or_else(|| anyhow!("Repository URL not found for {}", name))?;
    let binaries_folder = binaries_dir();
    let binaries_folder_branch = binaries_folder.join(branch);

    let mut args = vec![
        "install", "--locked", "--force", "--git", &repo_url, "--branch", branch,
    ];

    // Special handling for walrus which needs to install from walrus-service package
    if name == "walrus" {
        args.push("walrus-service");
        args.push("--bin");
        args.push("walrus");
    } else {
        args.push(&bin_config.binary_name);
    };

    args.extend(vec!["--root", binaries_folder_branch.to_str().unwrap()]);
    let mut cmd = Command::new("cargo");
    cmd.args(&args);

    let cmd = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .spawn()?;

    let output = cmd.wait_with_output()?;
    pb.finish_with_message("Done!");

    if !output.status.success() {
        let error_message = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Error during installation:\n{}", error_message));
    }

    println!("Installation completed successfully!");
    // bin folder is needed because cargo installs in  /folder/bin/binary_name.
    let orig_binary_path = binaries_folder_branch.join("bin").join(name);

    // rename the binary to `binary_name-nightly`, to keep things in sync across the board

    let dst_name = if debug {
        format!("{}-debug-nightly", name)
    } else {
        format!("{}-nightly", name)
    };
    let dst = binaries_folder_branch.join("bin").join(dst_name);

    #[cfg(windows)]
    let orig_binary_path = orig_binary_path.with_extension("exe");
    #[cfg(windows)]
    let dst = dst.with_extension("exe");

    std::fs::rename(&orig_binary_path, &dst)?;
    install_binary(
        name,
        branch.to_string(),
        "nightly",
        debug,
        dst,
        yes,
    )?;

    Ok(())
}

/// Install MVR CLI
pub async fn install_mvr(version: Option<String>, yes: bool) -> Result<(), Error> {
    use crate::config::get_binary_config;
    let bin_config = get_binary_config("mvr")?;
    let network = bin_config.default_network.clone();
    let binary_name = bin_config.binary_name.clone();
    if !check_if_binaries_exist(
        &binary_name,
        network.clone(),
        &version.clone().unwrap_or_default(),
    )? {
        let mut installer = mvr::MvrInstaller::new();
        let installed_version = installer.download_version(version).await?;

        println!("Adding binary: mvr-{installed_version}");

        let binary_path = binaries_dir()
            .join(&network)
            .join(format!("{}-{}", binary_name, installed_version));
        install_binary(
            &binary_name,
            network,
            &installed_version,
            false,
            binary_path,
            yes,
        )?;
    } else {
        let version = version.unwrap_or_default();
        println!("Binary mvr-{version} already installed. Use `suiup default set mvr {version}` to set the default version to the specified one.");
    }

    Ok(())
}

fn check_cargo_rust_installed() -> Result<(), Error> {
    if let Ok(output) = Command::new("rustc").arg("--version").output() {
        if output.status.success() {
            print!(
                "Rust is installed: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        } else {
            bail!("Rust is not installed");
        }
    } else {
        bail!("Failed to execute rustc command");
    }

    // Check if cargo is installed
    if let Ok(output) = Command::new("cargo").arg("--version").output() {
        if output.status.success() {
            print!(
                "Cargo is installed: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        } else {
            bail!("Cargo is not installed");
        }
    } else {
        bail!("Failed to execute cargo command");
    }
    Ok(())
}
