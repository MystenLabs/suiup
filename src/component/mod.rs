// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

mod doctor;
mod install;
mod list;
mod remove;

use anyhow::{Result, bail};

use crate::commands::{CommandMetadata, ComponentCommands, parse_component_with_version};
use crate::registry::BinaryRegistry;

/// ComponentManager handles all component-related operations
pub struct ComponentManager {
    github_token: Option<String>,
}

impl ComponentManager {
    /// Create a new ComponentManager instance
    pub fn new(github_token: Option<String>) -> Self {
        Self { github_token }
    }

    /// Handle component commands
    pub async fn handle_command(&self, cmd: ComponentCommands) -> Result<()> {
        match cmd {
            ComponentCommands::Doctor => self.run_doctor_checks().await,
            ComponentCommands::List => self.list_components().await,
            ComponentCommands::Add {
                component,
                nightly,
                debug,
                yes,
            } => {
                let command_metadata = parse_component_with_version(&component)?;
                self.install_component(command_metadata, nightly, debug, yes)
                    .await
            }
            ComponentCommands::Remove { binary } => {
                // Validate binary name against registry
                if !BinaryRegistry::global().contains(&binary) {
                    bail!(
                        "Unknown binary: {}. Use `suiup list` to see available binaries.",
                        binary
                    );
                }
                self.remove_component(&binary).await
            }
            ComponentCommands::Cleanup { all, days, dry_run } => {
                self.handle_cleanup(all, days, dry_run).await
            }
        }
    }

    /// List all available components
    async fn list_components(&self) -> Result<()> {
        list::list_components().await
    }

    /// Install a component
    async fn install_component(
        &self,
        command_metadata: CommandMetadata,
        nightly: Option<String>,
        debug: bool,
        yes: bool,
    ) -> Result<()> {
        let CommandMetadata {
            name,
            network,
            version,
        } = command_metadata;
        install::install_component(
            name,
            network,
            version,
            nightly,
            debug,
            yes,
            self.github_token.clone(),
        )
        .await
    }

    /// Remove a component
    async fn remove_component(&self, binary: &str) -> Result<()> {
        remove::remove_component(binary).await
    }

    /// Run diagnostic checks on the environment
    pub async fn run_doctor_checks(&self) -> Result<()> {
        doctor::run_doctor_checks().await
    }

    /// Handle cleanup operations
    async fn handle_cleanup(&self, all: bool, days: u32, dry_run: bool) -> Result<()> {
        crate::handlers::cleanup::handle_cleanup(all, days, dry_run).await
    }
}
