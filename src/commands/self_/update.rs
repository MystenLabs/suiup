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
        self_::handle_update().await
    }
}
