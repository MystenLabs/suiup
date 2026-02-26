// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use anyhow::{Context, Result, anyhow};
use clap::Args;

use crate::{
    paths::default_file_path,
    types::{Binaries, Version},
};

use crate::commands::print_table;

/// Get the default Sui CLI version.
#[derive(Args, Debug)]
pub struct Command;

impl Command {
    pub fn exec(&self) -> Result<()> {
        let default_path = default_file_path()?;
        let default = std::fs::read_to_string(&default_path)
            .with_context(|| format!("Cannot read default file {}", default_path.display()))?;
        let default: BTreeMap<String, (String, Version, bool)> = serde_json::from_str(&default)
            .map_err(|e| {
                anyhow!(
                    "Cannot deserialize default file {}: {e}",
                    default_path.display()
                )
            })?;
        let binaries = Binaries::from(default);

        println!("\x1b[1mDefault binaries:\x1b[0m");
        print_table(&binaries.binaries);
        Ok(())
    }
}
