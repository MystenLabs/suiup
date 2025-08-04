// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use std::fs::create_dir_all;

use crate::handlers::install::{install_from_nightly, install_from_release, install_mvr};
use crate::paths::{binaries_dir, get_default_bin_dir};
use crate::types::{Repo, Version};
use crate::config::get_binary_config;

/// Install a component with the given parameters
pub async fn install_component(
    name: String,
    network: String,
    version: Option<Version>,
    nightly: Option<String>,
    debug: bool,
    yes: bool,
    github_token: Option<String>,
) -> Result<()> {
    // Ensure installation directories exist
    let default_bin_dir = get_default_bin_dir();
    create_dir_all(&default_bin_dir)?;

    let installed_bins_dir = binaries_dir();
    create_dir_all(&installed_bins_dir)?;

    let bin_config = get_binary_config(&name)?;
    
    if !bin_config.supports_debug && debug && nightly.is_none() {
        return Err(anyhow!("Debug flag is only available for binaries that support it (currently only 'sui')"));
    }

    if nightly.is_some() && version.is_some() {
        return Err(anyhow!(
            "Cannot install from nightly and a release at the same time. Remove the version or the nightly flag"
        ));
    }

    // Use the appropriate network for the binary
    let effective_network = if bin_config.supported_networks.is_empty() {
        bin_config.default_network.clone()
    } else {
        network.clone()
    };
    
    create_dir_all(installed_bins_dir.join(&effective_network))?;
    
    if let Some(branch) = nightly {
        install_from_nightly(&name, &branch, debug, yes).await?;
    } else if name == "mvr" {
        // Special handling for MVR which has its own installer
        install_mvr(version, yes).await?;
    } else {
        let repo = Repo::from_binary_name(&name)?;
        install_from_release(
            &name,
            &effective_network,
            version,
            debug,
            yes,
            repo,
            github_token,
        )
        .await?;
    }

    Ok(())
}
