// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    handlers::installed_binaries_grouped_by_network,
    paths::default_file_path,
    types::{Binaries, Version},
};
use anyhow::Error;
use std::collections::BTreeMap;

use crate::commands::print_table;

/// Handles the `show` command
pub fn handle_show() -> Result<(), Error> {
    let default = std::fs::read_to_string(default_file_path()?)?;
    let default: BTreeMap<String, (String, Version, bool)> = serde_json::from_str(&default)?;
    let default_binaries = Binaries::from(default);

    // Default binaries table

    println!("\x1b[1mDefault binaries:\x1b[0m");
    print_table(&default_binaries.binaries);

    // Installed binaries table
    let installed_binaries = installed_binaries_grouped_by_network(None)?;
    let binaries = installed_binaries.into_iter().flat_map(|(_,binaries)| {
        binaries.to_owned()
    }).collect();
    println!("\x1b[1mInstalled binaries:\x1b[0m");
    print_table(&binaries);

    Ok(())
}
