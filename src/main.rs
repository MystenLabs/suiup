// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use suiup::commands::Command;
use suiup::handlers::self_::check_for_updates;
use suiup::paths::initialize;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    initialize()?;

    // Check for updates in the background
    check_for_updates();

    let cmd = Command::parse();
    if let Err(err) = cmd.exec().await {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }

    Ok(())
}
