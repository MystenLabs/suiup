// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashSet;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use anyhow::{anyhow, Result};
use tracing::debug;

use crate::commands::BinaryName;
use crate::paths::{default_file_path, get_default_bin_dir};
use crate::types::InstalledBinaries;

/// Remove a component and its associated files
pub async fn remove_component(binary: BinaryName) -> Result<()> {
    let mut installed_binaries = InstalledBinaries::new_async().await?;

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
            if !tokio::fs::try_exists(p).await.unwrap_or(false) {
                println!("Binary {p} does not exist. Aborting the command.");
                return Ok(());
            }
        }
    }

    // Load default binaries
    let default_file = default_file_path().await?;
    let default = tokio::fs::read_to_string(&default_file)
        .await
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
            tokio::fs::remove_file(p).await.map_err(|e| anyhow!("Cannot remove file: {e}"))?;
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
        if tokio::fs::try_exists(&default_bin_path).await.unwrap_or(false) {
            tokio::fs::remove_file(&default_bin_path)
                .await
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
    let mut file = File::create(&default_file)
        .await
        .map_err(|_| anyhow!("Cannot create file: {}", default_file.display()))?;
    file.write_all(serde_json::to_string_pretty(&default_binaries)?.as_bytes()).await?;

    // Update installed binaries metadata
    installed_binaries.remove_binary(&binary.to_string());
    debug!("Removed {binary} from installed_binaries JSON file. Saving updated data");
    installed_binaries.save_to_file_async().await?;

    Ok(())
}
