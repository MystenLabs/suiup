// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Args;

use crate::handlers::show::handle_show;

/// Show installed and active binaries.
#[derive(Args, Debug)]
pub struct Command {
    /// Show only default binaries
    #[arg(long)]
    default: bool,
}

impl Command {
    pub fn exec(&self) -> Result<()> {
        handle_show(self.default)
    }
}
