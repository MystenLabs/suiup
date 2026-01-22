// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::paths::{binaries_dir, get_default_bin_dir, release_archive_dir};
use crate::{paths::default_file_path, types::Version};
use anyhow::Error;
use anyhow::anyhow;
use flate2::read::GzDecoder;
use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::{fs::File, io::BufReader};

use crate::types::{BinaryVersion, InstalledBinaries};
use std::collections::BTreeMap;
#[cfg(not(windows))]
use std::fs::set_permissions;
#[cfg(not(windows))]
use std::os::unix::fs::PermissionsExt;
use tar::Archive;
use version::extract_version_from_release;

pub mod cleanup;
pub mod download;
pub mod install;
pub mod release;
pub mod self_;
pub mod show;
pub mod update;
pub mod version;
pub mod which;

pub const RELEASES_ARCHIVES_FOLDER: &str = "releases";

pub fn available_components() -> &'static [&'static str] {
    &["sui", "sui-node", "mvr", "walrus", "site-builder", "move-analyzer"]
}

// Main component handling function

/// Updates the default version file with the new installed version.
pub fn update_default_version_file(
    binaries: &Vec<String>,
    network: String,
    version: &str,
    debug: bool,
) -> Result<(), Error> {
    let path = default_file_path()?;
    let file = File::open(&path)?;
    let reader = BufReader::new(file);
    let mut map: BTreeMap<String, (String, Version, bool)> = serde_json::from_reader(reader)?;

    for binary in binaries {
        let b = map.get_mut(binary);
        if let Some(b) = b {
            b.0 = network.clone();
            b.1 = version.to_string();
            b.2 = debug;
        } else {
            map.insert(
                binary.to_string(),
                (network.clone(), version.to_string(), debug),
            );
        }
    }

    let mut file = File::create(path)?;
    file.write_all(serde_json::to_string_pretty(&map)?.as_bytes())?;

    Ok(())
}

/// Prompts the user and asks if they want to update the default version with the one that was just
/// installed.
pub fn update_after_install(
    name: &Vec<String>,
    network: String,
    version: &str,
    debug: bool,
    yes: bool,
) -> Result<(), Error> {
    // First check if the binary exists
    for binary in name {
        let binary_name = if *binary == "sui" && debug {
            format!("{}-debug", binary)
        } else {
            binary.clone()
        };

        let binary_path = if version == "nightly" {
            // cargo install places the binary in a `bin` folder
            binaries_dir()
                .join(&network)
                .join("bin")
                .join(format!("{}-{}", binary_name, version))
        } else {
            binaries_dir()
                .join(&network)
                .join(format!("{}-{}", binary_name, version))
        };

        #[cfg(windows)]
        let mut binary_path = binary_path.clone();
        #[cfg(windows)]
        {
            if binary_path.extension() != Some("exe".as_ref()) {
                let new_binary_path = format!("{}.exe", binary_path.display());
                binary_path.set_file_name(new_binary_path);
            }
        }

        if !binary_path.exists() {
            println!(
                "Binary not found at {}. Skipping default version update.",
                binary_path.display()
            );
            return Ok(());
        }
    }

    let input = if yes {
        "y".to_string()
    } else {
        let prompt = "Do you want to set this new installed version as the default one? [y/N] ";

        print!("{prompt}");
        std::io::stdout().flush().unwrap();

        // Create a mutable String to store the input
        let mut input = String::new();

        // Read the input from the console
        std::io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        // Trim the input and convert to lowercase for comparison
        input.trim().to_lowercase()
    };

    // Check the user's response
    match input.as_str() {
        "y" | "yes" => {
            for binary in name {
                let mut filename = if debug {
                    format!("{}-debug-{}", binary, version)
                } else {
                    format!("{}-{}", binary, version)
                };

                if version.is_empty() {
                    filename = filename.strip_suffix('-').unwrap_or_default().to_string();
                }

                let binary_folder = if version == "nightly" {
                    binaries_dir().join(&network).join("bin")
                } else {
                    binaries_dir().join(&network)
                };

                if !binary_folder.exists() {
                    std::fs::create_dir_all(&binary_folder).map_err(|e| {
                        anyhow!("Cannot create folder {}: {e}", binary_folder.display())
                    })?;
                }

                #[cfg(windows)]
                let filename = format!("{}.exe", filename);

                println!(
                    "Installing binary to {}/{}",
                    binary_folder.display(),
                    filename
                );

                let src = binary_folder.join(&filename);
                let dst = get_default_bin_dir().join(binary);

                println!("Setting {} as default", binary);

                #[cfg(windows)]
                let mut dst = dst.clone();
                #[cfg(windows)]
                {
                    if dst.extension() != Some("exe".as_ref()) {
                        let new_dst = format!("{}.exe", dst.display());
                        dst.set_file_name(new_dst);
                    }
                }

                tracing::debug!("Copying from {} to {}", src.display(), dst.display());

                std::fs::copy(&src, &dst).map_err(|e| {
                    anyhow!(
                        "Error copying {binary} to the default folder (src: {}, dst: {}): {e}",
                        src.display(),
                        dst.display()
                    )
                })?;

                #[cfg(unix)]
                {
                    let mut perms = std::fs::metadata(&dst)?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&dst, perms)?;
                }

                println!("[{network}] {binary}-{version} set as default");
            }
            update_default_version_file(name, network, version, debug)?;
            check_path_and_warn()?;
        }

        "" | "n" | "no" => {
            println!("Keeping the current default version.");
        }
        _ => {
            println!("Invalid input. Please enter 'y' or 'n'.");
            update_after_install(name, network, version, debug, yes)?;
        }
    }
    Ok(())
}

fn check_path_and_warn() -> Result<(), Error> {
    let local_bin = get_default_bin_dir();

    // Check if the bin directory exists in PATH
    if let Ok(path) = env::var("PATH") {
        #[cfg(windows)]
        let path_separator = ';';
        #[cfg(not(windows))]
        let path_separator = ':';

        if !path
            .split(path_separator)
            .any(|p| PathBuf::from(p) == local_bin)
        {
            println!("\nWARNING: {} is not in your PATH", local_bin.display());

            #[cfg(windows)]
            {
                println!("\nTo add it to your PATH:");
                println!("1. Press Win + X and select 'System'");
                println!(
                    "2. Click on 'Advanced system settings (might find it on the right side)'"
                );
                println!("3. Click on 'Environment Variables'");
                println!("4. Under 'User variables', find and select 'Path'");
                println!("5. Click 'Edit'");
                println!("6. Click 'New'");
                println!("7. Add the following path:");
                println!("    %USERPROFILE%\\Local\\bin");
                println!("8. Click 'OK' on all windows");
                println!("9. Restart your terminal\n");
            }

            #[cfg(not(windows))]
            {
                println!("Add one of the following lines depending on your shell:");
                println!("\nFor bash/zsh (~/.bashrc or ~/.zshrc):");
                println!("    export PATH=\"{}:$PATH\"", local_bin.display());
                println!("\nFor fish (~/.config/fish/config.fish):");
                println!("    fish_add_path {}", local_bin.display());
                println!("\nThen restart your shell or run one of:");
                println!("    source ~/.bashrc        # for bash");
                println!("    source ~/.zshrc         # for zsh");
                println!("    source ~/.config/fish/config.fish  # for fish\n");
            }
        }
    }
    Ok(())
}

/// Extracts a component from the release archive. The component's name is identified by the
/// `binary` parameter.
///
/// This extracts the component to the binaries folder under the network from which release comes
/// from, and sets the correct permissions for Unix based systems.
fn extract_component(orig_binary: &str, network: String, filename: &str) -> Result<(), Error> {
    let mut archive_path = release_archive_dir();
    archive_path.push(filename);

    let file = File::open(archive_path.as_path())
        .map_err(|_| anyhow!("Cannot open archive file: {}", archive_path.display()))?;
    let tar = GzDecoder::new(file);
    let mut archive = Archive::new(tar);

    #[cfg(not(windows))]
    let binary = orig_binary.to_string();
    #[cfg(windows)]
    let binary = format!("{}.exe", orig_binary);

    // Check if the current entry matches the file name
    for file in archive
        .entries()
        .map_err(|e| anyhow!("Cannot iterate through archive entries: {e}"))?
    {
        let mut f = file.unwrap();
        if f.path()?.file_name().and_then(|x| x.to_str()) == Some(&binary) {
            println!("Extracting file: {}", &binary);

            let mut output_path = binaries_dir();
            output_path.push(&network);
            if !output_path.is_dir() {
                std::fs::create_dir_all(output_path.as_path())?;
            }
            let version = extract_version_from_release(filename)?;
            let binary_version = format!("{}-{}", orig_binary, version);
            #[cfg(not(windows))]
            output_path.push(&binary_version);
            #[cfg(windows)]
            output_path.push(&format!("{}.exe", binary_version));

            let mut output_file = File::create(&output_path).map_err(|e| {
                anyhow!(
                    "Cannot create output path ({}) for extracting this file {binary_version}: {e}",
                    output_path.display()
                )
            })?;

            std::io::copy(&mut f, &mut output_file).map_err(|e| {
                anyhow!("Cannot copy the file ({orig_binary}) into the output path: {e}")
            })?;
            println!(" '{}' extracted successfully!", &binary);
            #[cfg(not(target_os = "windows"))]
            {
                // Retrieve and apply the original file permissions on Unix-like systems
                if let Ok(permissions) = f.header().mode() {
                    set_permissions(output_path, PermissionsExt::from_mode(permissions)).map_err(
                        |e| {
                            anyhow!(
                                "Cannot apply the original file permissions in a unix system: {e}"
                            )
                        },
                    )?;
                }
            }
            break;
        }
    }

    Ok(())
}

/// Checks if the binaries exist in the binaries folder
pub fn check_if_binaries_exist(
    binary: &str,
    network: String,
    version: &str,
) -> Result<bool, Error> {
    let mut path = binaries_dir();
    path.push(&network);

    let binary_version = if version.is_empty() {
        binary.to_string()
    } else {
        format!("{}-{}", binary, version)
    };

    // Build the final expected binary path (Windows binaries have .exe extension).
    // Previous logic incorrectly pushed both the `.exe` file name AND the plain name as an
    // additional path component on Windows, resulting in a non-existent path like:
    //   <...>/binaries/<network>/binary.exe/binary-version
    // This prevented proper detection of already installed binaries.
    if cfg!(target_os = "windows") {
        path.push(format!("{}.exe", binary_version));
    } else {
        path.push(&binary_version);
    }
    Ok(path.exists())
}

/// Returns a map of installed binaries grouped by network releases
pub fn installed_binaries_grouped_by_network(
    installed_binaries: Option<InstalledBinaries>,
) -> Result<BTreeMap<String, Vec<BinaryVersion>>, Error> {
    let installed_binaries = if let Some(installed_binaries) = installed_binaries {
        installed_binaries
    } else {
        InstalledBinaries::new()?
    };
    let binaries = installed_binaries.binaries();
    let mut files_by_folder: BTreeMap<String, Vec<BinaryVersion>> = BTreeMap::new();

    for b in binaries {
        if let Some(f) = files_by_folder.get_mut(&b.network_release.to_string()) {
            f.push(b.clone());
        } else {
            files_by_folder.insert(b.network_release.to_string(), vec![b.clone()]);
        }
    }

    Ok(files_by_folder)
}

#[cfg(test)]
mod tests {
    use super::check_if_binaries_exist;
    use crate::paths::binaries_dir;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;

    // --- Tests -----------------------------------------------------------------
    // Internal helper (exposed for tests inside this module) to build the final path; this
    // lets us unit test both Windows and non-Windows logic irrespective of the host platform.
    #[cfg(test)]
    fn build_binary_path(
        mut base: std::path::PathBuf,
        binary_version: &str,
        is_windows: bool,
    ) -> std::path::PathBuf {
        if is_windows {
            base.push(format!("{}.exe", binary_version));
        } else {
            base.push(binary_version);
        }
        base
    }

    // Validate helper path construction for both Windows & non-Windows cases.
    #[test]
    fn test_build_binary_path() {
        #[cfg(unix)]
        {
            let base_unix = PathBuf::from("/tmp/suiup/binaries/testnet");
            let p_unix = build_binary_path(base_unix.clone(), "sui-v1.0.0", false);
            assert!(
                p_unix
                    .to_string_lossy()
                    .ends_with("/tmp/suiup/binaries/testnet/sui-v1.0.0")
                    || p_unix
                        .to_string_lossy()
                        .ends_with("\\tmp\\suiup\\binaries\\testnet\\sui-v1.0.0")
            );
        }

        #[cfg(windows)]
        {
            let base_win = PathBuf::from("C:/suiup/binaries/testnet");
            let p_win = build_binary_path(base_win.clone(), "sui-v1.0.0", true);
            assert!(
                p_win.to_string_lossy().ends_with("sui-v1.0.0.exe"),
                "Windows path should end with .exe: {p_win:?}"
            );
            // Ensure we did not append an extra plain (non-.exe) component.
            let components: Vec<_> = p_win.components().collect();
            let last = components.last().unwrap().as_os_str().to_string_lossy();
            assert_eq!(last, "sui-v1.0.0.exe");
        }
    }

    // Functional test (host-platform specific) verifying existence detection works.
    #[test]
    fn test_check_if_binaries_exist_detects_created_file() {
        // Use a temp dir and point XDG/LOCALAPPDATA to it so binaries_dir() resolves inside it.
        let temp = tempfile::TempDir::new().unwrap();
        #[cfg(windows)]
        let (var, original) = ("LOCALAPPDATA", std::env::var("LOCALAPPDATA").ok());
        #[cfg(not(windows))]
        let (var, original) = ("XDG_DATA_HOME", std::env::var("XDG_DATA_HOME").ok());
        crate::set_env_var!(var, temp.path());

        let mut network_dir = binaries_dir();
        network_dir.push("testnet");
        fs::create_dir_all(&network_dir).unwrap();

        let binary_version = "sui-v1.2.3";
        let mut file_path = network_dir.clone();
        if cfg!(windows) {
            file_path.push(format!("{}.exe", binary_version));
        } else {
            file_path.push(binary_version);
        }
        let mut f = File::create(&file_path).unwrap();
        writeln!(f, "test").unwrap();

        let exists = check_if_binaries_exist("sui", "testnet".to_string(), "v1.2.3").unwrap();
        assert!(exists, "Binary should be detected as existing");

        // Restore original environment variable (best effort).
        if let Some(val) = original {
            crate::set_env_var!(var, val);
        } else {
            crate::remove_env_var!(var);
        }
    }
}
