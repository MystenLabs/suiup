
// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
use crate::{handlers::release::release_list};
use crate::types::Repo;


pub async fn component_release_list(
    repo: Repo,
    github_token: Option<String>
)-> Result<(), anyhow::Error>{

    let releases = release_list(&repo, github_token.clone()).await?.0;

    for release in releases{
        println!("{}",release.tag_name);
    }
    Ok(())
}
