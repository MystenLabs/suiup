// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use tracing::debug;

use crate::commands::BinaryName;
use crate::paths::{default_file_path, get_default_bin_dir};
use crate::types::InstalledBinaries;

/// Remove a component and its associated files
pub async fn remove_component(binary: BinaryName) -> Result<()> {
    let mut installed_binaries = InstalledBinaries::new()?;

    let binaries_to_remove = installed_binaries
        .binaries()
        .iter()
        .filter(|b| binary.to_string() == b.binary_name)
        .collect::<Vec<_>>();

    if binaries_to_remove.is_empty() {
        println!("No binaries found to remove");
        return Ok(());
    }

    println!("Binaries to remove: {binaries_to_remove:?}");

    // Verify all binaries exist before removing any
    for p in &binaries_to_remove {
        if let Some(p) = p.path.as_ref() {
            if !PathBuf::from(p).exists() {
                println!("Binary {p} does not exist. Aborting the command.");
                return Ok(());
            }
        }
    }

    // Load default binaries
    let default_file = default_file_path()?;
    let default = std::fs::read_to_string(&default_file)
        .map_err(|_| anyhow!("Cannot read file {}", default_file.display()))?;
    let mut default_binaries: std::collections::BTreeMap<String, (String, String, bool)> =
        serde_json::from_str(&default).map_err(|_| {
            anyhow!("Cannot decode default binary file to JSON. Is the file corrupted?")
        })?;

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
    File::create(&default_file)
        .map_err(|_| anyhow!("Cannot create file: {}", default_file.display()))?
        .write_all(serde_json::to_string_pretty(&default_binaries)?.as_bytes())?;

    // Update installed binaries metadata
    installed_binaries.remove_binary(&binary.to_string());
    debug!("Removed {binary} from installed_binaries JSON file. Saving updated data");
    installed_binaries.save_to_file()?;

    Ok(())
}
