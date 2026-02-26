// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Args;

use crate::handle_commands::handle_cmd;

use super::ComponentCommands;

/// List available binaries to install.
#[derive(Args, Debug)]
pub struct Command;

impl Command {
    pub async fn exec(&self, github_token: Option<&str>) -> Result<()> {
        handle_cmd(ComponentCommands::List, github_token).await
    }
}
