// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    commands::{parse_component_with_version, CommandMetadata, ComponentCommands},
    handle_commands::handle_cmd,
    types::InstalledBinaries,
};
use crate::{
    handlers::{
        available_components, installed_binaries_grouped_by_network,
        release::{last_release_for_network, release_list},
    },
    types::Repo,
};
use anyhow::{bail, Error};

/// Handles the `update` command
pub async fn handle_update(
    binary_name: String,
    yes: bool,
    github_token: Option<String>,
) -> Result<(), Error> {
    if binary_name.is_empty() {
        bail!("Invalid number of arguments for `update` command");
    }

    let CommandMetadata { name, version, .. } = parse_component_with_version(&binary_name)?;

    if version.is_some() {
        bail!("Update should be done without a version. Use `suiup install` to specify a version");
    }

    if !available_components().contains(&name.to_string()) {
        bail!("Invalid component name: {}", name);
    }

    let installed_binaries = InstalledBinaries::new()?;
    let binaries = installed_binaries.binaries();
    if !binaries.iter().any(|x| x.binary_name == name) {
        bail!("Binary {name} not found in installed binaries. Use `suiup show` to see installed binaries and `suiup install` to install the binary.")
    }
    let binaries_by_network = installed_binaries_grouped_by_network(Some(installed_binaries))?;

    let mut network_local_last_version: Vec<(String, String)> = vec![];

    for (network, binaries) in &binaries_by_network {
        let last_version = binaries
            .iter()
            .filter(|x| x.binary_name == name)
            .collect::<Vec<_>>();
        if last_version.is_empty() {
            continue;
        }
        let last_version = if last_version.len() > 1 {
            last_version
                .iter()
                .max_by(|a, b| a.version.cmp(&b.version))
                .unwrap()
        } else {
            last_version.first().unwrap()
        };
        network_local_last_version.push((network.clone(), last_version.version.clone()));
    }
    // map of network and last version known locally

    // find the last local version of the name binary, for each network
    // then find the last release for each network and compare the versions

    use crate::config::get_binary_config;
    let bin_config = get_binary_config(&name)?;
    
    if bin_config.supported_networks.is_empty() {
        handle_cmd(
            ComponentCommands::Add {
                component: binary_name,
                debug: false,
                nightly: None,
                yes,
            },
            github_token,
        )
        .await?;
        return Ok(());
    }

    if name == "walrus" {
        handle_cmd(
            ComponentCommands::Add {
                component: binary_name,
                debug: false,
                nightly: None,
                yes,
            },
            github_token,
        )
        .await?;
        return Ok(());
    }

    let repo = Repo::from_binary_name(&name)?;
    let releases = release_list(&repo, github_token.clone()).await?.0;
    let mut to_update = vec![];
    for (n, v) in &network_local_last_version {
        let last_release = last_release_for_network(&releases, n).await?;
        let last_version = last_release.1;
        if v == &last_version {
            println!("[{n} release] {name} is up to date");
        } else {
            println!("[{n} release] {name} is outdated. Local: {v}, Latest: {last_version}");
            to_update.push((n, last_version));
        }
    }

    for (n, v) in to_update.iter() {
        println!("Updating {name} to {v} from {n} release");
        handle_cmd(
            ComponentCommands::Add {
                component: binary_name.clone(),
                debug: false,
                nightly: None,
                yes,
            },
            github_token.clone(),
        )
        .await?;
    }

    Ok(())
}
