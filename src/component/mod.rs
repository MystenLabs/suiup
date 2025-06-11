// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

mod installer;
mod lister;
mod remover;

use anyhow::Result;

use crate::commands::{parse_component_with_version, BinaryName, CommandMetadata, ComponentCommands};

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
            ComponentCommands::Remove { binary } => self.remove_component(binary).await,
        }
    }

    /// List all available components
    async fn list_components(&self) -> Result<()> {
        lister::list_components().await
    }

    /// Install a component
    async fn install_component(
        &self,
        command_metadata: CommandMetadata,
        nightly: Option<String>,
        debug: bool,
        yes: bool,
    ) -> Result<()> {
        let CommandMetadata { name, network, version } = command_metadata;
        installer::install_component(name, network, version, nightly, debug, yes, self.github_token.clone()).await
    }

    /// Remove a component
    async fn remove_component(&self, binary: BinaryName) -> Result<()> {
        remover::remove_component(binary).await
    }
}
