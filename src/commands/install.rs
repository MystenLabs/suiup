// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Args;

use super::ComponentCommands;
use crate::handle_commands::handle_cmd;
use crate::handlers::config::ConfigHandler;

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
    /// Custom installation path for the binary
    #[arg(long, value_name = "path")]
    path: Option<String>,
}

impl Command {
    pub async fn exec(&self, github_token: &Option<String>) -> Result<()> {
        let component = if self.component.contains('@') || self.component.contains('=') {
            self.component.to_owned()
        } else {
            // If no version specified, use default network from config
            let config_handler = ConfigHandler::new()?;
            let config = config_handler.get_config();
            format!("{}@{}", self.component, config.default_network)
        };

        handle_cmd(
            ComponentCommands::Add {
                component,
                nightly: self.nightly.to_owned(),
                debug: self.debug.to_owned(),
                yes: self.yes.to_owned(),
                path: self.path.to_owned(),
            },
            github_token.to_owned(),
        )
        .await
    }
}
