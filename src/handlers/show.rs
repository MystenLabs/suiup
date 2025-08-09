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

/// Load default binaries from configuration file
fn load_default_binaries() -> Result<Binaries, Error> {
    let default = std::fs::read_to_string(default_file_path()?)?;
    let default: BTreeMap<String, (String, Version, bool)> = serde_json::from_str(&default)?;
    Ok(Binaries::from(default))
}

/// Load installed binaries grouped by network
fn load_installed_binaries() -> Result<Vec<crate::types::BinaryVersion>, Error> {
    let installed_binaries = installed_binaries_grouped_by_network(None)?;
    let binaries = installed_binaries
        .into_iter()
        .flat_map(|(_, binaries)| binaries.to_owned())
        .collect();
    Ok(binaries)
}

/// Display a section with title and binaries table
fn display_binaries_section(title: &str, binaries: &Vec<crate::types::BinaryVersion>) {
    println!("\x1b[1m{}:\x1b[0m", title);
    print_table(binaries);
}

/// Handles the `show` command
pub fn handle_show(default_only: bool) -> Result<(), Error> {
    // Load and display default binaries
    let default_binaries = load_default_binaries()?;
    display_binaries_section("Default binaries", &default_binaries.binaries);

    // Only show installed binaries if --default flag is not set
    if !default_only {
        let installed_binaries = load_installed_binaries()?;
        display_binaries_section("Installed binaries", &installed_binaries);
    }

    Ok(())
}
