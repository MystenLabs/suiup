// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Args;

use crate::handlers::self_;

/// Update suiup itself.
#[derive(Args, Debug)]
pub struct Command;

impl Command {
    pub async fn exec(&self) -> Result<()> {
        // Get GitHub token from config if available
        let github_token = match crate::handlers::config::ConfigHandler::new() {
            Ok(config_handler) => {
                let config = config_handler.get_config();
                config.github_token.clone()
            }
            Err(_) => None,
        };
        
        self_::handle_update(github_token).await
    }
}
