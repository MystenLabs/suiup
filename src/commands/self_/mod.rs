// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

mod uninstall;
mod update;

use anyhow::Result;
use clap::{Args, Subcommand};

/// Commands for suiup itself.
#[derive(Debug, Args)]
pub struct Command {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Update(update::Command),
    Uninstall(uninstall::Command),
}

impl Command {
    /// Handles the self commands
    pub async fn exec(&self) -> Result<()> {
        match &self.command {
            Commands::Update(cmd) => cmd.exec().await,
            Commands::Uninstall(cmd) => cmd.exec(),
        }
    }
}
