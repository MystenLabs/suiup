// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    handlers::installed_binaries_grouped_by_network,
    paths::default_file_path,
    types::{Binaries, Version},
};
use anyhow::Error;
use std::collections::BTreeMap;
use prettytable::{Table, row};

/// Handles the `show` command
pub fn handle_show() -> Result<(), Error> {
    let default = std::fs::read_to_string(default_file_path()?)?;
    let default: BTreeMap<String, (String, Version, bool)> = serde_json::from_str(&default)?;
    let default_binaries = Binaries::from(default);

    // Default binaries table
    let mut default_table = Table::new();
    default_table.add_row(row!["Network", "Binary", "Version", "Debug"]);
    for b in &default_binaries.binaries {
        default_table.add_row(row![
            b.network_release,
            b.binary_name,
            b.version,
            if b.debug { "Yes" } else { "No" }
        ]);
    }
    println!("\x1b[1mDefault binaries:\x1b[0m");
    default_table.printstd();

    // Installed binaries table
    let installed_binaries = installed_binaries_grouped_by_network(None)?;
    let mut installed_table = Table::new();
    installed_table.add_row(row!["Network", "Binary", "Version", "Debug"]);
    for (network, binaries) in installed_binaries {
        for b in binaries {
            installed_table.add_row(row![
                network.to_string(),
                b.binary_name,
                b.version,
                if b.debug { "Yes" } else { "No" }
            ]);
        }
    }
    println!("\x1b[1mInstalled binaries:\x1b[0m");
    installed_table.printstd();

    Ok(())
}
