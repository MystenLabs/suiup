// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::io::{self, Write};
use anyhow::Result;

use crate::paths::release_archive_dir;

/// Clean cached download files
pub async fn clean_component(yes: bool) -> Result<()> {
    let cache_dir = release_archive_dir();

    if !cache_dir.exists() {
        println!("Cache directory does not exist. Nothing to clean.");
        return Ok(());
    }

    let cache_size = calculate_directory_size(&cache_dir)?;

    if cache_size == 0 {
        println!("Cache is already empty.");
        return Ok(());
    }

    let cache_size_mb = cache_size as f64 / 1024.0 / 1024.0;

    if !yes {
        println!("\nThis will delete all cached release archives.");
        println!("Installed binaries will not be affected.");
        println!("\nCache location: {}", cache_dir.display());
        println!("Space to be freed: {:.2} MB", cache_size_mb);
        print!("\nProceed? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim().to_lowercase() != "y" {
            println!("Aborted by user.");
            return Ok(());
        }
    }

    println!("\nCleaning cache...");
    fs::remove_dir_all(&cache_dir)?;
    fs::create_dir_all(&cache_dir)?;
    println!("Successfully cleaned cache. Freed {:.2} MB.", cache_size_mb);

    Ok(())
}

fn calculate_directory_size(path: &std::path::Path) -> Result<u64> {
    let mut total_size = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_file() {
            total_size += metadata.len();
        } else if metadata.is_dir() {
            total_size += calculate_directory_size(&entry.path())?;
        }
    }
    Ok(total_size)
}