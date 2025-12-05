// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use clap::Args;
use std::path::PathBuf;

/// Patch a binary with Nix runtime dependencies (Linux only).
#[derive(Args, Debug)]
pub struct Command {
    /// Path to the binary to patch
    #[arg(value_name = "BINARY")]
    pub binary: PathBuf,
}

impl Command {
    pub fn exec(&self) -> Result<()> {
        use crate::patchelf::patch_binary;

        if !self.binary.exists() {
            return Err(anyhow!("Binary not found: {}", self.binary.display()));
        }

        patch_binary(&self.binary)?;
        Ok(())
    }
}
