// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

mod doctor;
mod install;
mod uninstall;
mod update;

use anyhow::Result;
use clap::{Args, Subcommand};

/// Manage suibase installation and maintenance scripts.
#[derive(Debug, Args)]
pub struct Command {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Install(install::Command),
    Update(update::Command),
    Uninstall(uninstall::Command),
    Doctor(doctor::Command),
}

impl Command {
    pub async fn exec(&self) -> Result<()> {
        match &self.command {
            Commands::Install(cmd) => cmd.exec(),
            Commands::Update(cmd) => cmd.exec(),
            Commands::Uninstall(cmd) => cmd.exec(),
            Commands::Doctor(cmd) => cmd.exec(),
        }
    }
}
