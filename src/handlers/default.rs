// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    commands::{parse_component_with_version, BinaryName, CommandMetadata, DefaultCommands},
    handlers::{installed_binaries_grouped_by_network, update_default_version_file},
    paths::{binaries_dir, default_file_path, get_default_bin_dir},
    types::{Binaries, Version},
};
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Error;
use std::collections::HashMap;

use tracing::debug;

#[cfg(not(windows))]
use std::os::unix::fs::PermissionsExt;

/// Handles the default commands
pub fn handle_default(cmd: DefaultCommands) -> Result<(), Error> {
    match cmd {
        DefaultCommands::Get => {
            // let default_binaries = DefaultBinaries::load()?;
            let default = std::fs::read_to_string(default_file_path()?)?;
            let default: HashMap<String, (String, Version, bool)> = serde_json::from_str(&default)?;
            let default_binaries = Binaries::from(default);
            println!("\x1b[1mDefault binaries:\x1b[0m\n{default_binaries}");
        }

        DefaultCommands::Set {
            name,
            debug,
            nightly,
        } => {
            if name.len() != 2 && nightly.is_none() {
                bail!("Invalid number of arguments. Version is required: 'sui testnet-v1.39.3', 'sui testnet' -- this will use an installed binary that has the higest testnet version. \n For `mvr` only pass the version: `mvr v0.0.5`")
            }

            let CommandMetadata {
                name,
                network,
                version,
            } = parse_component_with_version(&name.join(" "))?;

            let network = if name == BinaryName::Mvr {
                if let Some(ref nightly) = nightly {
                    nightly
                } else {
                    "standalone"
                }
            } else {
                &network
            };

            // a map of network --> to BinaryVersion
            let installed_binaries = installed_binaries_grouped_by_network(None)?;
            let binaries = installed_binaries
                .get(network)
                .ok_or_else(|| anyhow!("No binaries installed for {network}"))?;

            // Check if the binary exists in any network
            let binary_exists = installed_binaries
                .values()
                .any(|bins| bins.iter().any(|x| x.binary_name == name.to_string()));
            if !binary_exists {
                bail!("Binary {name} not found in installed binaries. Use `suiup show` to see installed binaries.");
            }

            let version = if let Some(version) = version {
                version
            } else {
                binaries
                    .iter()
                    .filter(|b| b.binary_name == name.to_string())
                    .max_by(|a, b| a.version.cmp(&b.version))
                    .map(|b| b.version.clone())
                    .ok_or_else(|| anyhow!("No version found for {name} in {network}"))?
            };

            // check if the binary for this network and version exists
            let binary_version = format!("{}-{}", name, version);
            debug!("Checking if {binary_version} exists");
            binaries
                .iter()
                .find(|b| {
                    b.binary_name == name.to_string() && b.version == version && b.network_release == network
                })
                .ok_or_else(|| {
                    anyhow!("Binary {binary_version} from {network} release not found. Use `suiup show` to see installed binaries.")
                })?;

            // copy files to default-bin
            let mut dst = get_default_bin_dir();
            let name = if debug {
                format!("{}-debug", name)
            } else {
                format!("{}", name)
            };

            dst.push(&name);

            #[cfg(target_os = "windows")]
            dst.set_extension("exe");

            let mut src = binaries_dir();
            src.push(network);

            if nightly.is_some() {
                // cargo install adds a bin folder to the specified path :-)
                src.push("bin");
            }

            if debug {
                src.push(format!("{}-debug-{}", name, version));
            } else {
                src.push(binary_version);
            }

            #[cfg(target_os = "windows")]
            let filename = src.file_name().expect("Expected binary filename");
            #[cfg(target_os = "windows")]
            src.set_file_name(format!(
                "{}.exe",
                filename
                    .to_str()
                    .expect("Expected binary filename as string")
            ));

            #[cfg(not(target_os = "windows"))]
            {
                if dst.exists() {
                    std::fs::remove_file(&dst)?;
                }

                std::fs::copy(&src, &dst)?;

                #[cfg(unix)]
                {
                    let mut perms = std::fs::metadata(&dst)?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&dst, perms)?;
                }
            }

            #[cfg(target_os = "windows")]
            {
                std::fs::copy(&src, &dst)?;
            }

            update_default_version_file(
                &vec![name.to_string()],
                network.to_string(),
                &version,
                debug,
            )?;
            println!("Default binary updated successfully");
        }
    }

    Ok(())
}
