// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use suiup::commands::Command;
use suiup::paths::initialize;
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
