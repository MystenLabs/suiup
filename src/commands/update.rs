// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Args;

use crate::handlers::update::handle_update;

/// Update binary.
#[derive(Args, Debug)]
pub struct Command {
    /// Binary to update (e.g. 'sui', 'mvr', 'walrus'). By default, it will update the default
    /// binary version. For updating a specific release, use the `sui@testnet` form.
    name: String,

    /// Accept defaults without prompting
    #[arg(short, long)]
    yes: bool,
}

impl Command {
    pub async fn exec(&self, github_token: &Option<String>) -> Result<()> {
        handle_update(
            self.name.to_owned(),
            self.yes.to_owned(),
            github_token.to_owned(),
        )
        .await
    }
}
