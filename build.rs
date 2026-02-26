// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let binaries_dir = Path::new(&manifest_dir).join("binaries");
    println!("cargo:rerun-if-changed=binaries/");

    let mut entries: Vec<String> = Vec::new();

    if binaries_dir.is_dir() {
        for entry in fs::read_dir(&binaries_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "toml") {
                let abs_path = path.canonicalize().unwrap();
                let abs_str = abs_path.to_str().unwrap().replace('\\', "/");
                println!("cargo:rerun-if-changed={}", abs_str);
                entries.push(format!("include_str!(\"{}\")", abs_str));
            }
        }
    }

    entries.sort(); // deterministic ordering

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("binary_configs.rs");
    let content = format!(
        "pub const BINARY_CONFIGS: &[&str] = &[\n    {},\n];\n",
        entries.join(",\n    ")
    );
    fs::write(dest_path, content).unwrap();
}
