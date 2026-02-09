// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Args;

use crate::handlers::suibase;

/// Update an existing suibase installation.
#[derive(Args, Debug)]
pub struct Command {
    /// Show commands without executing them.
    #[arg(long, short = 'n')]
    dry_run: bool,
    /// Reserved for non-interactive flows. Currently no prompt is used during update.
    #[arg(short, long)]
    yes: bool,
}

impl Command {
    pub fn exec(&self) -> Result<()> {
        suibase::update(suibase::ActionOptions {
            yes: self.yes,
            dry_run: self.dry_run,
        })
    }
}
