// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::commands::TABLE_STYLE;
use crate::registry::BinaryRegistry;
use anyhow::Result;
use comfy_table::*;

/// List all available components
pub fn list_components() -> Result<()> {
    let registry = BinaryRegistry::global();
    let mut table = Table::new();
    table
        .load_style(TABLE_STYLE)
        .set_header(vec![
            Cell::new("Binary"),
            Cell::new("Description"),
            Cell::new("Notes"),
        ])
        .add_rows(
            registry
                .all()
                .iter()
                .map(|config| {
                    vec![
                        Cell::new(&config.name),
                        Cell::new(&config.description),
                        Cell::new(if config.experimental {
                            "Experimental; breaking changes expected; not recommended for production"
                        } else {
                            ""
                        }),
                    ]
                })
                .collect::<Vec<Vec<Cell>>>(),
        );
    println!("{table}");
    Ok(())
}
