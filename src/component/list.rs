// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use comfy_table::*;

/// List all available components
pub async fn list_components() -> Result<()> {
    let components = crate::handlers::available_components();
    let mut table = Table::new();
    let format = "  ── ══      ──    ";
    table
        .load_preset(format)
        .set_header(vec![Cell::new("Available Binaries")])
        .add_rows(
            components
                .iter()
                .map(|component| vec![Cell::new(component)])
                .collect::<Vec<Vec<Cell>>>(),
        );
    println!("{table}");
    Ok(())
}
