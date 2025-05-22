// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;

mod commands;
mod handle_commands;
mod handlers;
mod mvr;
mod paths;
mod types;

use commands::Command;
use paths::*;
use tracing::error;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    initialize()?;

    let cmd = Command::parse();
    if let Err(err) = cmd.exec().await {
        error!("{}", err);
        std::process::exit(1);
    }

    Ok(())
}
