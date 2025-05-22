// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use anyhow::Result;
use clap::Args;

use crate::{
    paths::default_file_path,
    types::{Binaries, Version},
};

/// Get the default Sui CLI version.
#[derive(Args, Debug)]
pub struct Command;

impl Command {
    pub fn exec(&self) -> Result<()> {
        // let default_binaries = DefaultBinaries::load()?;
        let default = std::fs::read_to_string(default_file_path()?)?;
        let default: BTreeMap<String, (String, Version, bool)> = serde_json::from_str(&default)?;
        let binaries = Binaries::from(default);

        println!("\x1b[1mDefault binaries:\x1b[0m\n{binaries}");
        Ok(())
    }
}
