// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use clap::{CommandFactory, Parser};
use suiup::commands::Command;
use suiup::paths::initialize;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    initialize()?;

    if std::env::args_os().len() <= 1 {
        let mut help = Command::command();
        help.print_help()?;
        println!();
        return Ok(());
    }

    let cmd = Command::parse();
    if let Err(err) = cmd.exec().await {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }

    Ok(())
}
