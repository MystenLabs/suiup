// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use std::fs::create_dir_all;

use crate::commands::BinaryName;
use crate::handlers::install::{install_from_nightly, install_from_release, install_mvr};
use crate::handlers::config::ConfigHandler;
use crate::paths::{binaries_dir, get_default_bin_dir};
use crate::types::{Repo, Version, InstalledBinaries};
use crate::handlers::cleanup::{auto_cleanup_cache, CacheConfig};

/// Tool status management for enable/disable functionality
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ToolStatus {
    pub name: String,
    pub enabled: bool,
}

/// Detect existing version of a binary from installed binaries
fn detect_existing_version(name: &BinaryName, network: &str) -> Result<Option<Version>> {
    match InstalledBinaries::new() {
        Ok(installed_binaries) => {
            let binaries = installed_binaries.binaries();

            // Look for existing installation of this binary on the same network
            for binary in binaries {
                if binary.binary_name == name.to_string() && binary.network_release == network {
                    return Ok(Some(binary.version.clone()));
                }
            }

            // If not found on the specific network, look for any version
            for binary in binaries {
                if binary.binary_name == name.to_string() {
                    return Ok(Some(binary.version.clone()));
                }
            }

            Ok(None)
        }
        Err(_) => Ok(None), // If we can't read installed binaries, return None
    }
}

/// Set tool enable/disable status
fn set_tool_status(name: &BinaryName, enabled: bool) -> Result<()> {
    println!("Setting {} tool status to: {}", name, if enabled { "enabled" } else { "disabled" });
    // For now, this is a placeholder. In a real implementation, this might:
    // - Update a configuration file
    // - Modify PATH entries
    // - Set environment variables
    // - Update shell profiles
    Ok(())
}

/// Install a component with the given parameters
pub async fn install_component(
    name: BinaryName,
    network: String,
    mut version: Option<Version>,
    nightly: Option<String>,
    debug: bool,
    yes: bool,
    path: Option<String>,
    enable: bool,
    disable: bool,
    auto_detect: bool,
    github_token: Option<String>,
) -> Result<()> {
    // Auto-detect existing version if requested and no version specified
    if auto_detect && version.is_none() && nightly.is_none() {
        version = detect_existing_version(&name, &network)?;
        if let Some(ref detected_version) = version {
            println!("Auto-detected existing version: {}", detected_version);
        }
    }

    // Handle custom installation path - priority: command line arg > config file > default
    let install_path = if let Some(custom_path) = path {
        std::path::PathBuf::from(custom_path)
    } else {
        // Check config file for install_path setting
        match ConfigHandler::new() {
            Ok(config_handler) => {
                let config = config_handler.get_config();
                if let Some(ref config_path) = config.install_path {
                    std::path::PathBuf::from(config_path)
                } else {
                    get_default_bin_dir()
                }
            }
            Err(_) => get_default_bin_dir(), // Fallback to default if config can't be loaded
        }
    };

    // Ensure installation directories exist
    create_dir_all(&install_path)?;

    let installed_bins_dir = binaries_dir();
    create_dir_all(&installed_bins_dir)?;

    if name != BinaryName::Sui && debug && nightly.is_none() {
        return Err(anyhow!("Debug flag is only available for the `sui` binary"));
    }

    if nightly.is_some() && version.is_some() {
        return Err(anyhow!(
            "Cannot install from nightly and a release at the same time. Remove the version or the nightly flag"
        ));
    }

    match (&name, &nightly) {
        (BinaryName::Walrus, nightly) => {
            create_dir_all(installed_bins_dir.join(network.clone()))?;
            if let Some(branch) = nightly {
                install_from_nightly(&name, branch, debug, yes).await?;
            } else {
                install_from_release(
                    name.to_string().as_str(),
                    &network,
                    version,
                    debug,
                    yes,
                    Repo::Walrus,
                    github_token,
                )
                .await?;
            }
        }
        (BinaryName::WalrusSites, nightly) => {
            create_dir_all(installed_bins_dir.join("mainnet"))?;
            if let Some(branch) = nightly {
                install_from_nightly(&name, branch, debug, yes).await?;
            } else {
                install_from_release(
                    name.to_string().as_str(),
                    "mainnet",
                    version,
                    debug,
                    yes,
                    Repo::WalrusSites,
                    github_token,
                )
                .await?;
            }
        }

        (BinaryName::Mvr, nightly) => {
            create_dir_all(installed_bins_dir.join("standalone"))?;
            if let Some(branch) = nightly {
                install_from_nightly(&name, branch, debug, yes).await?;
            } else {
                install_mvr(version, yes).await?;
            }
        }
        (_, Some(branch)) => {
            install_from_nightly(&name, branch, debug, yes).await?;
        }
        _ => {
            install_from_release(
                name.to_string().as_str(),
                &network,
                version,
                debug,
                yes,
                Repo::Sui,
                github_token,
            )
            .await?;
        }
    }

    // Handle tool enable/disable status after successful installation
    if enable {
        set_tool_status(&name, true)?;
    } else if disable {
        set_tool_status(&name, false)?;
    }

    // Run automatic cache cleanup after installation
    let cache_config = CacheConfig::from_config().unwrap_or_else(|_| CacheConfig::default());
    if let Err(e) = auto_cleanup_cache(&cache_config).await {
        println!("Warning: Auto cleanup failed: {}", e);
        // Don't fail the installation if cleanup fails
    }

    println!("Installation completed successfully!");

    Ok(())
}
