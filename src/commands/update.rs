// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Args;

use crate::handlers::update::handle_update;

/// Update binary.
#[derive(Args, Debug)]
pub struct Command {
    /// Binary to update (e.g. 'sui', 'mvr', 'walrus'). By default, it updates the currently
    /// active binary for its network and sets the new version as the default. To update a
    /// specific network, use the `sui@testnet` form.
    name: String,

    /// Deprecated: an update always sets the new version as the default, so this has no effect.
    #[arg(short, long)]
    yes: bool,
}

impl Command {
    pub async fn exec(&self, github_token: Option<&str>) -> Result<()> {
        handle_update(self.name.clone(), self.yes, github_token.map(str::to_owned)).await
    }
}
