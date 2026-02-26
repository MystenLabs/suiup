// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Args;

use crate::handle_commands::handle_cmd;

use super::ComponentCommands;

/// Remove one or more binaries.
#[derive(Args, Debug)]
pub struct Command {
    binary: String,
}

impl Command {
    pub async fn exec(&self, github_token: &Option<String>) -> Result<()> {
        handle_cmd(
            ComponentCommands::Remove {
                binary: self.binary.to_owned(),
            },
            github_token.to_owned(),
        )
        .await
    }
}
