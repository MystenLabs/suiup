// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::handlers::{
    available_components, default_version_for, installed_binaries_grouped_by_network,
    release::{last_release_for_network, release_list},
};
use crate::{
    commands::{CommandMetadata, ComponentCommands, parse_component_with_version},
    handle_commands::handle_cmd,
    registry::InstallationType,
    types::InstalledBinaries,
};
use anyhow::{Error, bail};

/// Handles the `update` command
pub async fn handle_update(
    binary_name: String,
    // An update always adopts the newly-installed version as the default, so the caller's
    // `--yes` is redundant here (kept on the CLI for backward compatibility).
    _yes: bool,
    github_token: Option<String>,
) -> Result<(), Error> {
    if binary_name.is_empty() {
        bail!("Invalid number of arguments for `update` command");
    }

    let CommandMetadata {
        name,
        network,
        version,
    } = parse_component_with_version(&binary_name)?;

    if version.is_some() {
        bail!("Update should be done without a version. Use `suiup install` to specify a version");
    }

    if !available_components().contains(&name.as_str()) {
        bail!("Invalid component name: {}", name);
    }

    // A bare `@network` (e.g. `sui@mainnet`) yields `version = None` and reaches here, so the
    // presence of `@` means the user pinned a network explicitly. A `@version` would have been
    // rejected by the guard above.
    let explicit_network = binary_name.contains('@').then(|| network.clone());

    let config = name.config();
    let installed_binaries = InstalledBinaries::new()?;
    let binaries = installed_binaries.binaries();
    if !binaries.iter().any(|x| x.binary_name == name.as_str()) {
        bail!(
            "Binary {name} not found in installed binaries. Use `suiup show` to see installed binaries and `suiup install` to install the binary."
        )
    }

    // Standalone binaries and non-network-based binaries: just re-install. Network is not
    // meaningful here, but preserve the installed debug flag.
    if !config.network_based || config.installation_type == InstallationType::Standalone {
        let debug = binaries
            .iter()
            .find(|x| x.binary_name == name.as_str())
            .map(|x| x.debug)
            .unwrap_or(false);
        handle_cmd(
            ComponentCommands::Add {
                component: binary_name,
                debug,
                nightly: None,
                // Always make the updated version the default.
                yes: true,
            },
            github_token.as_deref(),
        )
        .await?;
        return Ok(());
    }

    let binaries_by_network = installed_binaries_grouped_by_network(Some(installed_binaries))?;

    // The networks this binary is currently installed on.
    let installed_networks: Vec<String> = binaries_by_network
        .iter()
        .filter(|(_, bins)| bins.iter().any(|b| b.binary_name == name.as_str()))
        .map(|(net, _)| net.clone())
        .collect();

    // Resolve the single network to update: an explicit `@network`, else the active default,
    // else the sole installed network (error if the choice is ambiguous).
    let target_network = if let Some(net) = explicit_network {
        net
    } else if let Some((net, _, _)) = default_version_for(name.as_str())? {
        net
    } else if let [only] = installed_networks.as_slice() {
        only.clone()
    } else {
        bail!(
            "{name} is installed on multiple networks ({}). Specify one, e.g. `suiup update {name}@mainnet`.",
            installed_networks.join(", ")
        );
    };

    // The local record for the target network (highest version), with its debug flavor.
    let local = binaries_by_network
        .get(&target_network)
        .map(|bins| {
            bins.iter()
                .filter(|b| b.binary_name == name.as_str())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let local = local
        .iter()
        .max_by(|a, b| a.version.cmp(&b.version))
        .copied();
    let Some(local) = local else {
        bail!(
            "{name} is not installed for the {target_network} network. Installed networks: {}.",
            if installed_networks.is_empty() {
                "none".to_string()
            } else {
                installed_networks.join(", ")
            }
        );
    };
    let (local_version, debug) = (local.version.clone(), local.debug);

    // Compare against the latest release for that one network and re-install only if outdated.
    let releases = release_list(&config.repository, github_token.clone())
        .await?
        .0;
    let latest = last_release_for_network(&releases, &target_network).await?.1;
    if local_version == latest {
        println!("[{target_network} release] {name} is up to date");
        return Ok(());
    }

    println!(
        "[{target_network} release] {name} is outdated. Local: {local_version}, Latest: {latest}"
    );
    println!("Updating {name} to {latest} from {target_network} release");
    // Reconstruct an explicit `name@network-version` spec so the update targets this network
    // (and debug flavor) instead of the default network.
    handle_cmd(
        ComponentCommands::Add {
            component: format!("{name}@{target_network}-{latest}"),
            debug,
            nightly: None,
            // Always make the updated version the default.
            yes: true,
        },
        github_token.as_deref(),
    )
    .await?;

    Ok(())
}
