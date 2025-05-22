// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Args;

use crate::handlers::which::handle_which;

/// Show the path where default binaries are installed.
#[derive(Args, Debug)]
pub struct Command;

impl Command {
    pub fn exec(&self) -> Result<()> {
        handle_which()
    }
}
