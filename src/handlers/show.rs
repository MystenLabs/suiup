// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    handlers::installed_binaries_grouped_by_network_async,
    paths::default_file_path,
    types::{Binaries, Version},
};
use anyhow::Error;
use std::collections::BTreeMap;

use crate::commands::print_table;

/// Handles the `show` command
pub async fn handle_show_async(default_only: bool) -> Result<(), Error> {
    let default = tokio::fs::read_to_string(default_file_path().await?).await?;
    let default: BTreeMap<String, (String, Version, bool)> = serde_json::from_str(&default)?;
    let default_binaries = Binaries::from(default);

    // Default binaries table
    println!("\x1b[1mDefault binaries:\x1b[0m");
    print_table(&default_binaries.binaries);

    // Only show installed binaries if --default flag is not set
    if !default_only {
        // Installed binaries table
        let installed_binaries = installed_binaries_grouped_by_network_async(None).await?;
        let binaries = installed_binaries
            .into_iter()
            .flat_map(|(_, binaries)| binaries.to_owned())
            .collect();
        println!("\x1b[1mInstalled binaries:\x1b[0m");
        print_table(&binaries);
    }

    Ok(())
}

/// Handles the `show` command (sync version)
pub fn handle_show(default_only: bool) -> Result<(), Error> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(handle_show_async(default_only))
}