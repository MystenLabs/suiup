// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Result, anyhow};
use std::fs::create_dir_all;

use crate::commands::BinaryName;
use crate::handlers::install::{install_from_nightly, install_from_release, install_standalone};
use crate::paths::{binaries_dir, get_default_bin_dir};
use crate::types::{Repo, Version};

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
    // Ensure installation directories exist
    let default_bin_dir = get_default_bin_dir();
    create_dir_all(&default_bin_dir)?;

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
                install_standalone(
                    version,
                    match name {
                        BinaryName::Mvr => Repo::Mvr,
                        _ => {
                            return Err(anyhow!("Invalid binary name for standalone installation"));
                        }
                    },
                    yes,
                )
                .await?;
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

    Ok(())
}
