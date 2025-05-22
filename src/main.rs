// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Error;
use clap::Parser;

mod commands;
mod handle_commands;
mod handlers;
mod mvr;
mod paths;
mod types;
use commands::{Commands, ComponentCommands, Suiup};
use handle_commands::handle_cmd;
use handlers::default::handle_default;
use handlers::self_::handle_self;
use handlers::show::handle_show;
use handlers::update::handle_update;
use handlers::which::handle_which;
use paths::*;

#[tokio::main]
async fn main() -> Result<(), Error> {
    initialize()?;
    let args = Suiup::parse();
    let github_token = args.github_token.clone();

    match args.command {
        Commands::Default(cmd) => handle_default(cmd)?,
        Commands::Install {
            component,
            nightly,
            debug,
            yes,
        } => {
            handle_cmd(
                ComponentCommands::Add {
                    component,
                    nightly,
                    debug,
                    yes,
                },
                github_token,
            )
            .await?
        }
        Commands::Remove { binary } => {
            handle_cmd(ComponentCommands::Remove { binary }, github_token).await?
        }
        Commands::List => handle_cmd(ComponentCommands::List, github_token).await?,
        Commands::Self_(cmd) => handle_self(cmd).await?,
        Commands::Show => handle_show()?,
        Commands::Update { name, yes } => handle_update(name, yes, github_token).await?,
        Commands::Which => handle_which()?,
    }
    Ok(())
}
