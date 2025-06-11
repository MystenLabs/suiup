// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;

/// List all available components
pub async fn list_components() -> Result<()> {
    let components = crate::handlers::available_components();
    println!("Available binaries to install:");
    for component in components {
        println!(" - {}", component);
    }
    Ok(())
}
