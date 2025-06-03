// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Error;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tracing::debug;

use crate::commands::parse_component_with_version;
use crate::commands::BinaryName;
use crate::commands::ComponentCommands;
use crate::handlers::available_components;
use crate::handlers::install::{install_from_nightly, install_from_release, install_mvr};
use crate::paths::*;
use crate::types::InstalledBinaries;
use crate::types::Repo;
use crate::types::Version;
use std::fs::create_dir_all;
use crate::handlers::list::component_release_list;

pub async fn handle_cmd(cmd: ComponentCommands, github_token: Option<String>) -> Result<(), Error> {
    match cmd {
        ComponentCommands::List {
            component,
        }=>{
             let command_metadata =
                parse_component_with_version(&component).map_err(|e| anyhow!("{e}"))?;
            let name = command_metadata.name;
            if !available_components().contains(&name.to_string().as_str()) {
                bail!("Binary {} does not exist", name);
            }
            match &name{
                BinaryName::Walrus => {
                    component_release_list(
                         Repo::Walrus,
                         github_token.clone()
                    )
                    .await?;
                }
                BinaryName::Mvr =>{
                    component_release_list(
                         Repo::Mvr,
                         github_token.clone()
                    )
                    .await?;
                }
                _=>{
                    component_release_list(
                         Repo::Sui,
                         github_token.clone()
                    )
                    .await?;
                }
            }
            // println!("Available binaries to install:");
            // for component in components {
            //     println!(" - {}", component);
            // }
        }
        ComponentCommands::Add {
            component,
            nightly,
            debug,
            yes,
        } => {
            // Ensure installation directories exist
            let default_bin_dir = get_default_bin_dir();
            create_dir_all(&default_bin_dir)?;

            let installed_bins_dir = binaries_dir();
            create_dir_all(&installed_bins_dir)?;

            let command_metadata =
                parse_component_with_version(&component).map_err(|e| anyhow!("{e}"))?;
            let name = command_metadata.name;
            let network = command_metadata.network;
            let version = command_metadata.version;
            let available_components = available_components();
            if !available_components.contains(&name.to_string().as_str()) {
                bail!("Binary {} does not exist", name);
            }

            if name != BinaryName::Sui && debug && nightly.is_none() {
                bail!("Debug flag is only available for the `sui` binary");
            }

            if nightly.is_some() && version.is_some() {
                bail!("Cannot install from nightly and a release at the same time. Remove the version or the nightly flag");
            }

            match (&name, &nightly) {
                (BinaryName::Walrus, nightly) => {
                    create_dir_all(installed_bins_dir.join(network.clone()))?;
                    if let Some(branch) = nightly {
                        install_from_nightly(&name, branch, debug, yes).await?;
                    } else {
                        install_from_release(
                            name.to_string().as_str(),
                            &network,
                            version,
                            debug,
                            yes,
                            Repo::Walrus,
                            github_token.clone(),
                        )
                        .await?;
                    }
                }
                (BinaryName::Mvr, nightly) => {
                    create_dir_all(installed_bins_dir.join("standalone"))?;
                    if let Some(branch) = nightly {
                        install_from_nightly(&name, branch, debug, yes).await?;
                    } else {
                        install_mvr(version, yes).await?;
                    }
                }
                (_, Some(branch)) => {
                    install_from_nightly(&name, branch, debug, yes).await?;
                }
                _ => {
                    install_from_release(
                        name.to_string().as_str(),
                        &network,
                        version,
                        debug,
                        yes,
                        Repo::Sui,
                        github_token.clone(),
                    )
                    .await?;
                }
            }
        }
        ComponentCommands::Remove { binary } => {
            let mut installed_binaries = InstalledBinaries::new()?;

            let binaries_to_remove = installed_binaries
                .binaries()
                .iter()
                .filter(|b| binary.to_string() == b.binary_name)
                .collect::<Vec<_>>();

            if binaries_to_remove.is_empty() {
                println!("No binaries found to remove");
                return Ok(());
            }

            println!("Binaries to remove: {binaries_to_remove:?}");

            for p in &binaries_to_remove {
                if let Some(p) = p.path.as_ref() {
                    if !PathBuf::from(p).exists() {
                        println!("Binary {p} does not exist. Aborting the command.");

                        return Ok(());
                    }
                }
            }

            let default_file = default_file_path()?;
            let default = std::fs::read_to_string(&default_file)
                .map_err(|_| anyhow!("Cannot read file {}", default_file.display()))?;
            let mut default_binaries: BTreeMap<String, (String, Version, bool)> =
                serde_json::from_str(&default).map_err(|_| {
                    anyhow!("Cannot decode default binary file to JSON. Is the file corrupted?")
                })?;

            // Remove the installed binaries folder
            for binary in &binaries_to_remove {
                if let Some(p) = binary.path.as_ref() {
                    println!("Found binary path: {p}");
                    debug!("Removing binary: {p}");
                    std::fs::remove_file(p).map_err(|e| anyhow!("Cannot remove file: {e}"))?;
                    debug!("File removed: {p}");
                    println!("Removed binary: {} from {p}", binary.binary_name);
                }
            }

            // Remove the binaries from the default-bin folder
            let default_binaries_to_remove = binaries_to_remove
                .iter()
                .map(|x| &x.binary_name)
                .collect::<HashSet<_>>();
            for binary in default_binaries_to_remove {
                let default_bin_path = get_default_bin_dir().join(binary);
                if default_bin_path.exists() {
                    std::fs::remove_file(default_bin_path)
                        .map_err(|e| anyhow!("Cannot remove file: {e}"))?;
                }

                default_binaries.remove(binary);
                debug!("Removed {binary} from default binaries JSON file");
            }

            // Remove from default binaries metadata file
            File::create(&default_file)
                .map_err(|_| anyhow!("Cannot create file: {}", default_file.display()))?
                .write_all(serde_json::to_string_pretty(&default_binaries)?.as_bytes())?;

            // Remove from installed_binaries metadata file
            installed_binaries.remove_binary(&binary.to_string());
            debug!("Removed {binary} from installed_binaries JSON file. Saving updated data");
            // Save file
            installed_binaries.save_to_file()?;
        }
    }
    Ok(())
}

// pub(crate) fn print_completion_instructions(shell: &Shell) {
//     match shell {
//         Shell::Bash => {
//             println!("\nTo install bash completions:");
//             println!("1. Create completion directory if it doesn't exist:");
//             println!("    mkdir -p ~/.local/share/bash-completion/completions");
//             println!("2. Add completions to the directory:");
//             println!(
//                 "    suiup completion bash > ~/.local/share/bash-completion/completions/suiup"
//             );
//             println!("\nMake sure you have bash-completion installed and loaded in your ~/.bashrc");
//         }
//         Shell::Fish => {
//             println!("\nTo install fish completions:");
//             println!("1. Create completion directory if it doesn't exist:");
//             println!("    mkdir -p ~/.config/fish/completions");
//             println!("2. Add completions to the directory:");
//             println!("    suiup completion fish > ~/.config/fish/completions/suiup.fish");
//         }
//         Shell::Zsh => {
//             println!("\nTo install zsh completions:");
//             println!("1. Create completion directory if it doesn't exist:");
//             println!("    mkdir -p ~/.zsh/completions");
//             println!("2. Add completions to the directory:");
//             println!("    suiup completion zsh > ~/.zsh/completions/_suiup");
//             println!("3. Add the following to your ~/.zshrc:");
//             println!("    fpath=(~/.zsh/completions $fpath)");
//             println!("    autoload -U compinit; compinit");
//         }
//         _ => {}
//     }
// }
