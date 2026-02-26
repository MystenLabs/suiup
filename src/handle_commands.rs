// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use anyhow::Error;

use crate::commands::ComponentCommands;
use crate::component::ComponentManager;

/// Handle component commands by delegating to the ComponentManager
pub async fn handle_cmd(cmd: ComponentCommands, github_token: Option<&str>) -> Result<(), Error> {
    let manager = ComponentManager::new(github_token.map(str::to_owned));
    manager.handle_command(cmd).await
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
