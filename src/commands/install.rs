// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Args;

use crate::handle_commands::handle_cmd;

use super::ComponentCommands;

/// Install a binary.
#[derive(Args, Debug)]
pub struct Command {
    /// Binary to install with optional version
    /// (e.g. 'sui', 'sui@1.40.1', 'sui@testnet', 'sui@testnet-1.39.3')
    component: String,

    /// Install from a branch in release mode (use --debug for debug mode).
    /// If none provided, main is used. Note that this requires Rust & cargo to be installed.
    #[arg(long, value_name = "branch", default_missing_value = "main", num_args = 0..=1)]
    nightly: Option<String>,

    /// This flag can be used in two ways: 1) to install the debug version of the
    /// binary (only available for sui, default is false; 2) together with `--nightly`
    /// to specify to install from branch in debug mode!
    #[arg(long)]
    debug: bool,

    /// Accept defaults without prompting
    #[arg(short, long)]
    yes: bool,
}

impl Command {
    pub async fn exec(&self, github_token: &Option<String>) -> Result<()> {
        handle_cmd(
            ComponentCommands::Add {
                component: self.component.to_owned(),
                nightly: self.nightly.to_owned(),
                debug: self.debug.to_owned(),
                yes: self.yes.to_owned(),
            },
            github_token.to_owned(),
        )
        .await
    }
}
