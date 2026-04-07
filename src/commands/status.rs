// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Args;

use crate::handlers::status::handle_status;

/// Check for available updates for installed binaries.
#[derive(Args, Debug)]
pub struct Command;

impl Command {
    pub async fn exec(&self, github_token: Option<&str>) -> Result<()> {
        handle_status(github_token.map(str::to_owned)).await
    }
}
