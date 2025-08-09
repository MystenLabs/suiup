// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Error};
use lazy_static::lazy_static;

lazy_static! {
    static ref VERSION_REGEX: regex::Regex = regex::Regex::new(r"v\d+\.\d+\.\d+").unwrap();
}

/// Extracts the version from a release filename
pub fn extract_version_from_release(release: &str) -> Result<String, Error> {
    let captures = VERSION_REGEX
        .captures(release)
        .ok_or_else(|| anyhow!("Could not extract version from release"))?;

    Ok(captures.get(0).unwrap().as_str().to_string())
}
