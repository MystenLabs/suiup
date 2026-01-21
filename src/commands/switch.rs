// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Args;

use super::default::set::Command as SetCommand;

/// Switch to a different version of an installed binary.
#[derive(Args, Debug)]
pub struct Command {
    /// Binary and network/release to switch to
    /// e.g. 'sui@testnet', 'mvr@main', 'walrus@testnet'
    /// This will use the latest installed version for that network/release
    binary_spec: String,
}

impl Command {
    pub fn exec(&self) -> Result<()> {
        let set_cmd = SetCommand::new(self.binary_spec.clone());
        set_cmd.exec()
    }
}
