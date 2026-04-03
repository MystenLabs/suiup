// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use crate::handlers::self_;

#[derive(Args, Debug)]
pub struct Command {
    #[arg(long)]
    target: PathBuf,

    #[arg(long)]
    source: PathBuf,

    #[arg(long)]
    parent_pid: Option<u32>,

    #[arg(long)]
    helper_path: Option<PathBuf>,
}

impl Command {
    pub fn exec(&self) -> Result<()> {
        self_::handle_complete_update(
            &self.target,
            &self.source,
            self.parent_pid,
            self.helper_path.as_deref(),
        )
    }
}
