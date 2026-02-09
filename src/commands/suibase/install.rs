// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Args;

use crate::handlers::suibase;

/// Install suibase from the upstream repository.
#[derive(Args, Debug)]
pub struct Command;

impl Command {
    pub fn exec(&self) -> Result<()> {
        suibase::install()
    }
}
