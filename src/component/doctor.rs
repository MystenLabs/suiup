// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::paths::{
    default_file_path, get_default_bin_dir, get_suiup_data_dir, installed_binaries_file,
};
use crate::types::InstalledBinaries;
use anyhow::Result;
use colored::Colorize;
use std::env;
use std::process::Command;

pub async fn run_doctor_checks() -> Result<()> {
    println!("\n{}", "Suiup Environment Doctor".bold());
    println!("------------------------");

    let mut warnings = 0;
    let mut errors = 0;

    let mut check = |message: &str, result: Result<String, String>| match result {
        Ok(info) if info.is_empty() => println!("[{}] {}", "✓".green(), message),
        Ok(info) => println!("[{}] {} {}", "✓".green(), message, info.dimmed()),
        Err(e) => {
            if e.starts_with("WARN:") {
                warnings += 1;
                println!(
                    "[{}] {}",
                    "!".yellow(),
                    e.strip_prefix("WARN:").unwrap_or(&e).trim()
                );
            } else {
                errors += 1;
                println!(
                    "[{}] {}",
                    "✗".red(),
                    e.strip_prefix("ERROR:").unwrap_or(&e).trim()
                );
            }
        }
    };

    check("suiup data directory exists", check_suiup_data_dir());
    check_path_variables(&mut check);
    check_config_files(&mut check);
    check_dependencies(&mut check);
    check_network_connectivity(&mut check).await;

    println!("\n{}", "Checkup complete.".bold());
    if errors > 0 {
        println!(
            "{}",
            format!("Found {} error(s) and {} warning(s).", errors, warnings).red()
        );
    } else if warnings > 0 {
        println!("{}", format!("Found {} warning(s).", warnings).yellow());
    } else {
        println!("{}", "Your environment looks good!".green());
    }

    Ok(())
}

fn check_suiup_data_dir() -> Result<String, String> {
    let path = get_suiup_data_dir();
    if path.exists() && path.is_dir() {
        Ok(format!("at {}", path.display()))
    } else {
        Err(format!(
            "ERROR: suiup data directory not found at {}",
            path.display()
        ))
    }
}

fn check_path_variables(check: &mut impl FnMut(&str, Result<String, String>)) {
    let default_bin_dir = get_default_bin_dir();
    check(
        "Default binary directory",
        Ok(format!("is {}", default_bin_dir.display())),
    );

    match env::var("PATH") {
        Ok(path_var) => {
            let paths: Vec<_> = env::split_paths(&path_var).collect();
            if !paths.contains(&default_bin_dir) {
                check(
                    "Default binary directory in PATH",
                    Err(
                        "WARN: Not found in PATH. Binaries managed by suiup may not be accessible."
                            .to_string(),
                    ),
                );
            } else {
                check("Default binary directory in PATH", Ok("".to_string()));

                // Check PATH order
                let cargo_bin_dir = dirs::home_dir().map(|p| p.join(".cargo/bin"));
                if let Some(cargo_bin) = cargo_bin_dir
                    && paths.contains(&cargo_bin)
                {
                    let suiup_pos = paths.iter().position(|p| p == &default_bin_dir);
                    let cargo_pos = paths.iter().position(|p| p == &cargo_bin);
                    if let (Some(s_pos), Some(c_pos)) = (suiup_pos, cargo_pos) {
                        if s_pos > c_pos {
                            check(
                                "PATH order",
                                Err(format!(
                                    "WARN: Default binary directory ({}) is after cargo's binary directory ({}). This may cause conflicts if you have also installed sui via `cargo install`.",
                                    default_bin_dir.display(),
                                    cargo_bin.display()
                                )),
                            );
                        } else {
                            check("PATH order", Ok("is correct".to_string()));
                        }
                    }
                }
            }
        }
        Err(_) => {
            check(
                "PATH variable",
                Err("ERROR: Could not read PATH environment variable.".to_string()),
            );
        }
    }
}

fn check_config_files(check: &mut impl FnMut(&str, Result<String, String>)) {
    let installed_path = installed_binaries_file();
    match installed_path {
        Ok(path) => {
            if !path.exists() {
                check(
                    "Installed binaries config",
                    Err(format!("WARN: File not found at {}", path.display())),
                );
            } else {
                match InstalledBinaries::read_from_file() {
                    Ok(_) => check("Installed binaries config", Ok("is valid".to_string())),
                    Err(e) => check(
                        "Installed binaries config",
                        Err(format!("ERROR: Failed to parse: {}", e)),
                    ),
                }
            }
        }
        Err(e) => check(
            "Installed binaries config",
            Err(format!("ERROR: Could not get path: {}", e)),
        ),
    }

    let default_path = default_file_path();
    match default_path {
        Ok(path) => {
            if !path.exists() {
                check(
                    "Default version config",
                    Err(format!("WARN: File not found at {}", path.display())),
                );
            } else {
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        let result: Result<serde_json::Value, _> = serde_json::from_str(&content);
                        if result.is_ok() {
                            check("Default version config", Ok("is valid".to_string()));
                        } else {
                            check(
                                "Default version config",
                                Err("ERROR: Failed to parse as valid JSON.".to_string()),
                            );
                        }
                    }
                    Err(e) => check(
                        "Default version config",
                        Err(format!("ERROR: Failed to read: {}", e)),
                    ),
                }
            }
        }
        Err(e) => check(
            "Default version config",
            Err(format!("ERROR: Could not get path: {}", e)),
        ),
    }
}

fn check_dependencies(check: &mut impl FnMut(&str, Result<String, String>)) {
    // Check for rustc
    match Command::new("rustc").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            check("rustc", Ok(version));
        }
        _ => check(
            "rustc",
            Err("WARN: Not found. Required for --nightly builds.".to_string()),
        ),
    }

    // Check for cargo
    match Command::new("cargo").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            check("cargo", Ok(version));
        }
        _ => check(
            "cargo",
            Err("WARN: Not found. Required for --nightly builds.".to_string()),
        ),
    }

    // Check for git
    match Command::new("git").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            check("git", Ok(version));
        }
        _ => check(
            "git",
            Err("WARN: Not found. Required for --nightly builds.".to_string()),
        ),
    }
}

async fn check_network_connectivity(check: &mut impl FnMut(&str, Result<String, String>)) {
    let client = reqwest::Client::new();

    match client
        .get("https://api.github.com")
        .header("User-Agent", "suiup")
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            check("GitHub API connectivity", Ok("".to_string()))
        }
        _ => check(
            "GitHub API connectivity",
            Err("ERROR: Cannot connect to GitHub API. Downloads will fail.".to_string()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_check_suiup_data_dir_exists() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path();
        fs::create_dir_all(data_dir.join("suiup")).unwrap();

        // Temporarily set the data dir to our temp dir to control where get_suiup_data_dir looks.
        let original_data_home;
        #[cfg(windows)]
        {
            original_data_home = std::env::var("LOCALAPPDATA");
            std::env::set_var("LOCALAPPDATA", data_dir.to_str().unwrap());
        }
        #[cfg(not(windows))]
        {
            original_data_home = std::env::var("XDG_DATA_HOME");
            std::env::set_var("XDG_DATA_HOME", data_dir.to_str().unwrap());
        }

        let result = check_suiup_data_dir();
        assert!(result.is_ok());
        assert!(result.unwrap().contains("at"));

        // Restore original env var
        #[cfg(windows)]
        {
            if let Ok(val) = original_data_home {
                std::env::set_var("LOCALAPPDATA", val);
            } else {
                std::env::remove_var("LOCALAPPDATA");
            }
        }
        #[cfg(not(windows))]
        {
            if let Ok(val) = original_data_home {
                std::env::set_var("XDG_DATA_HOME", val);
            } else {
                std::env::remove_var("XDG_DATA_HOME");
            }
        }
    }

    #[test]
    fn test_check_suiup_data_dir_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path();

        // Temporarily set the data dir to our temp dir to control where get_suiup_data_dir looks.
        let original_data_home;
        #[cfg(windows)]
        {
            original_data_home = std::env::var("LOCALAPPDATA");
            std::env::set_var("LOCALAPPDATA", data_dir.to_str().unwrap());
        }
        #[cfg(not(windows))]
        {
            original_data_home = std::env::var("XDG_DATA_HOME");
            std::env::set_var("XDG_DATA_HOME", data_dir.to_str().unwrap());
        }

        let path = crate::paths::get_suiup_data_dir();
        println!("Testing path: {}", path.display());
        println!("Path exists: {}", path.exists());
        let result = check_suiup_data_dir();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("suiup data directory not found")
        );

        // Restore original env var
        #[cfg(windows)]
        {
            if let Ok(val) = original_data_home {
                std::env::set_var("LOCALAPPDATA", val);
            } else {
                std::env::remove_var("LOCALAPPDATA");
            }
        }
        #[cfg(not(windows))]
        {
            if let Ok(val) = original_data_home {
                std::env::set_var("XDG_DATA_HOME", val);
            } else {
                std::env::remove_var("XDG_DATA_HOME");
            }
        }
    }
}
