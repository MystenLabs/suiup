// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::paths::get_default_bin_dir;
use anyhow::Error;

/// Handles the `which` command
pub fn handle_which() -> Result<(), Error> {
    let default_bin = get_default_bin_dir();
    println!("{}", default_bin.display());
    Ok(())
}
