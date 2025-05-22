// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

mod get;
mod set;

use anyhow::Result;
use clap::{Args, Subcommand};

/// Get or set the default tool version.
#[derive(Debug, Args)]
pub struct Command {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Get(get::Command),
    Set(set::Command),
}

impl Command {
    /// Handles the default commands
    pub fn exec(&self) -> Result<()> {
        match &self.command {
            Commands::Get(cmd) => cmd.exec(),
            Commands::Set(cmd) => cmd.exec(),
        }
    }
}
