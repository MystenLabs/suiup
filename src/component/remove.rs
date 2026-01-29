// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use tracing::debug;

use crate::commands::BinaryName;
use crate::fs_utils::{read_json_file, write_json_file};
use crate::paths::{default_file_path, get_default_bin_dir};
use crate::types::InstalledBinaries;

/// Remove a component and its associated files
pub async fn remove_component(binary: BinaryName) -> Result<()> {
    let mut installed_binaries = InstalledBinaries::new()?;

    let binaries_to_remove = installed_binaries
        .binaries()
        .iter()
        .filter(|b| {
            if binary == BinaryName::Signers {
                b.binary_name == "ledger-signer" || b.binary_name == "yubikey-signer"
            } else {
                binary.to_string() == b.binary_name
            }
        })
        .collect::<Vec<_>>();

    if binaries_to_remove.is_empty() {
        println!("No binaries found to remove");
        return Ok(());
    }

    if binary == BinaryName::Signers {
        let found_names: HashSet<&str> = binaries_to_remove
            .iter()
            .map(|b| b.binary_name.as_str())
            .collect();

        for &expected in &["ledger-signer", "yubikey-signer"] {
            if !found_names.contains(expected) {
                println!(
                    "Warning: {} was not found among installed binaries, so it cannot be removed.",
                    expected
                );
            }
        }
    }

    println!("Binaries to remove: {binaries_to_remove:?}");

    // Verify all binaries exist before removing any
    for p in &binaries_to_remove {
        if let Some(p) = p.path.as_ref()
            && !PathBuf::from(p).exists()
        {
            println!("Binary {p} does not exist. Aborting the command.");
            return Ok(());
        }
    }

    // Load default binaries
    let default_file = default_file_path()?;
    let mut default_binaries: std::collections::BTreeMap<String, (String, String, bool)> =
        read_json_file(&default_file)?;

    // Remove the installed binaries
    for binary in &binaries_to_remove {
        if let Some(p) = binary.path.as_ref() {
            println!("Found binary path: {p}");
            debug!("Removing binary: {p}");
            std::fs::remove_file(p).map_err(|e| anyhow!("Cannot remove file: {e}"))?;
            debug!("File removed: {p}");
            println!("Removed binary: {} from {p}", binary.binary_name);
        }
    }

    // Remove the binaries from the default-bin folder
    let default_binaries_to_remove = binaries_to_remove
        .iter()
        .map(|x| &x.binary_name)
        .collect::<HashSet<_>>();

    for binary in default_binaries_to_remove {
        let default_bin_path = get_default_bin_dir().join(binary);
        if default_bin_path.exists() {
            std::fs::remove_file(&default_bin_path)
                .map_err(|e| anyhow!("Cannot remove file: {e}"))?;
            debug!(
                "Removed {} from default binaries folder",
                default_bin_path.display()
            );
        }

        default_binaries.remove(binary);
        debug!("Removed {binary} from default binaries JSON file");
    }

    // Update default binaries file
    write_json_file(&default_file, &default_binaries)?;

    // Update installed binaries metadata
    if binary == BinaryName::Signers {
        installed_binaries.remove_binary("ledger-signer");
        installed_binaries.remove_binary("yubikey-signer");
    } else {
        installed_binaries.remove_binary(&binary.to_string());
    }
    debug!("Removed {binary} from installed_binaries JSON file. Saving updated data");
    installed_binaries.save_to_file()?;

    Ok(())
}
