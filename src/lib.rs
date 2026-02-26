// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

pub mod commands;
pub mod component;
pub mod fs_utils;
pub mod handle_commands;
pub mod handlers;
pub mod paths;
pub mod registry;
pub mod standalone;
pub mod types;

/// Macro to safely wrap `std::env::set_var` calls in an unsafe block.
/// This centralizes the unsafe operation and improves code readability.
///
/// # Example
/// ```
/// set_env_var!("XDG_DATA_HOME", "/path/to/data");
/// ```
#[macro_export]
macro_rules! set_env_var {
    ($key:expr, $value:expr) => {
        {
            let key = $key;
            let value = $value;
            unsafe {
                std::env::set_var(key, value);
            }
        }
    };
}

/// Macro to safely wrap `std::env::remove_var` calls in an unsafe block.
/// This centralizes the unsafe operation and improves code readability.
///
/// # Example
/// ```
/// remove_env_var!("XDG_DATA_HOME", "/path/to/data");
/// ```
#[macro_export]
macro_rules! remove_env_var {
    ($key:expr) => {
        {
            let key = $key;
            unsafe {
                std::env::remove_var(key);
            }
        }
    };
}
