// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn read_json_file<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let s = std::fs::read_to_string(path)
        .map_err(|e| anyhow!("Cannot read from file {}: {e}", path.display()))?;
    serde_json::from_str(&s)
        .map_err(|e| anyhow!("Cannot deserialize from file {}: {e}", path.display()))
}

pub fn write_json_file<T: Serialize>(path: &Path, data: &T) -> Result<()> {
    let s = serde_json::to_string_pretty(data).map_err(|e| {
        anyhow!(
            "Cannot serialize data to write to file {}: {e}",
            path.display()
        )
    })?;
    let mut file =
        File::create(path).map_err(|e| anyhow!("Cannot create file {}: {e}", path.display()))?;
    file.write_all(s.as_bytes())
        .map_err(|e| anyhow!("Cannot write to {}: {e}", path.display()))?;
    Ok(())
}
