// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Args;

use crate::handle_commands::handle_cmd;

use super::ComponentCommands;

/// Clean cached download files.
#[derive(Args, Debug)]
pub struct Command {
    #[arg(short, long, help = "Skip confirmation prompt")]
    pub yes: bool,
}

impl Command {
    pub async fn exec(&self, github_token: &Option<String>) -> Result<()> {
        handle_cmd(
            ComponentCommands::Clean {
                yes: self.yes,
            },
            github_token.to_owned(),
        )
        .await
    }
}