// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, bail, Result};
use clap::Args;
use tracing::debug;

use crate::{
    commands::{parse_component_with_version, BinaryName, CommandMetadata},
    handlers::{installed_binaries_grouped_by_network, update_default_version_file},
    paths::{binaries_dir, get_default_bin_dir},
};

#[cfg(not(windows))]
use std::os::unix::fs::PermissionsExt;

/// Set the default Sui CLI version.
#[derive(Args, Debug)]
pub struct Command {
    /// Binary to be set as default and the version
    /// e.g. 'sui@testnet-1.39.3', 'sui@testnet' --
    /// this will use an installed binary
    /// that has the highest testnet version)
    name: String,

    /// Whether to set the debug version of the binary as default (only available for sui).
    #[arg(long)]
    debug: bool,

    /// Use the nightly version by optionally specifying the branch name (uses main by default).
    /// Use `suiup show` to find all installed binaries
    #[arg(long, value_name = "branch", default_missing_value = "main", num_args = 0..=1)]
    nightly: Option<String>,
}

impl Command {
    pub fn exec(&self) -> Result<()> {
        let Command {
            name,
            debug,
            nightly,
        } = self;

        if name.is_empty() && nightly.is_none() {
            bail!("Invalid number of arguments. Version is required: 'sui@testnet-1.39.3', 'sui@testnet' -- this will use an installed binary that has the highest testnet version. \n For `mvr` only pass the version: `mvr@0.0.5`")
        }

        let CommandMetadata {
            name,
            network,
            version,
        } = parse_component_with_version(name)?;

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
            if version.starts_with("v") {
                version
            } else {
                format!("v{version}")
            }
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
        let name = if *debug {
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

        if *debug {
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
            *debug,
        )?;

        println!("Default binary updated successfully");
        Ok(())
    }
}
