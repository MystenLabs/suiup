// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result, anyhow};
use std::fs::create_dir_all;

use crate::handlers::install::{install_from_nightly, install_from_release, install_standalone};
use crate::paths::{binaries_dir, get_default_bin_dir};
use crate::registry::{BinaryName, InstallationType};
use crate::types::Version;

/// Install a component with the given parameters
pub async fn install_component(
    name: BinaryName,
    network: String,
    version: Option<Version>,
    nightly: Option<String>,
    debug: bool,
    yes: bool,
    github_token: Option<String>,
) -> Result<()> {
    let config = name.config();

    // Ensure installation directories exist
    let default_bin_dir = get_default_bin_dir();
    create_dir_all(&default_bin_dir).with_context(|| {
        format!(
            "Cannot create default bin directory {}",
            default_bin_dir.display()
        )
    })?;

    let installed_bins_dir = binaries_dir();
    create_dir_all(&installed_bins_dir).with_context(|| {
        format!(
            "Cannot create installed binaries directory {}",
            installed_bins_dir.display()
        )
    })?;

    if !config.supports_debug && debug && nightly.is_none() {
        return Err(anyhow!("Debug flag is only available for the `sui` binary"));
    }

    if nightly.is_some() && version.is_some() {
        return Err(anyhow!(
            "Cannot install from nightly and a release at the same time. Remove the version or the nightly flag"
        ));
    }

    // Handle nightly installs (same for all binary types)
    if let Some(branch) = &nightly {
        install_from_nightly(&name, branch, debug, yes).await?;
        return Ok(());
    }

    // Data-driven dispatch based on config
    match config.installation_type {
        InstallationType::Archive => {
            // For network-based archives, determine the right network
            let effective_network = if config.network_based {
                // If the binary only supports specific networks, use the first/default
                if !config.supported_networks.is_empty()
                    && !config.supported_networks.contains(&network)
                {
                    config.default_network.clone()
                } else {
                    network.clone()
                }
            } else {
                network.clone()
            };

            let target_dir = installed_bins_dir.join(&effective_network);
            create_dir_all(&target_dir)
                .with_context(|| format!("Cannot create directory {}", target_dir.display()))?;

            install_from_release(
                name.as_str(),
                &effective_network,
                version,
                debug,
                yes,
                config,
                github_token,
            )
            .await?;
        }
        InstallationType::Standalone => {
            let standalone_dir = installed_bins_dir.join("standalone");
            create_dir_all(&standalone_dir)
                .with_context(|| format!("Cannot create directory {}", standalone_dir.display()))?;

            // For shared_repo_binary, pass the binary name explicitly
            let binary_name_override = if config.shared_repo_binary {
                Some(name.as_str())
            } else {
                None
            };

            install_standalone(version, config, binary_name_override, yes, github_token).await?;
        }
    }

    Ok(())
}
