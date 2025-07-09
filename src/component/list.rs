// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use prettytable::{Table, row};

/// List all available components
pub async fn list_components() -> Result<()> {
    let components = crate::handlers::available_components();
    let mut table = Table::new();
    table.add_row(row!["Available Binaries"]);
    for component in components {
        table.add_row(row![component]);
    }
    table.printstd();
    Ok(())
}
