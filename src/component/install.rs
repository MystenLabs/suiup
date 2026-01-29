// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result, anyhow};
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
            let walrus_dir = installed_bins_dir.join(network.clone());
            create_dir_all(&walrus_dir)
                .with_context(|| format!("Cannot create directory {}", walrus_dir.display()))?;
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
            let mainnet_dir = installed_bins_dir.join("mainnet");
            create_dir_all(&mainnet_dir)
                .with_context(|| format!("Cannot create directory {}", mainnet_dir.display()))?;
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
            let standalone_dir = installed_bins_dir.join("standalone");
            create_dir_all(&standalone_dir)
                .with_context(|| format!("Cannot create directory {}", standalone_dir.display()))?;
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
                    None,
                    yes,
                    github_token,
                )
                .await?;
            }
        }
        (BinaryName::LedgerSigner | BinaryName::YubikeySigner, nightly) => {
            create_dir_all(installed_bins_dir.join("standalone"))?;
            if let Some(branch) = nightly {
                install_from_nightly(&name, branch, debug, yes).await?;
            } else {
                install_standalone(version, Repo::Signers, Some(name), yes).await?;
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
