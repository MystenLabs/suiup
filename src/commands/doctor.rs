// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Args;

use crate::component::ComponentManager;

/// Run diagnostic checks on the environment.
#[derive(Args, Debug)]
pub struct Command {}

impl Command {
    pub async fn exec(&self, github_token: &Option<String>) -> Result<()> {
        let component_manager = ComponentManager::new(github_token.clone());
        component_manager.run_doctor_checks().await
    }
}