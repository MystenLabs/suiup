// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    handlers::installed_binaries_grouped_by_network,
    paths::default_file_path,
    types::{Binaries, Version},
};
use std::collections::HashSet;
use tabled::{
    builder::Builder as TableBuilder,
    settings::{
        Style as TableStyle
    }
};
use anyhow::Error;
use std::collections::BTreeMap;

/// Handles the `show` command
pub fn handle_show() -> Result<(), Error> {
    let default = std::fs::read_to_string(default_file_path()?)?;
    let default: BTreeMap<String, (String, Version, bool)> = serde_json::from_str(&default)?;
    let default_binaries = Binaries::from(default);

    
    let  default_list = default_binaries.binaries
                    .iter()
                    .map(|t|  format!("{}-{}-{}",t.network_release, t.binary_name, t.version))
                    .collect::<HashSet<String>>();
     

    
    let installed_binaries = installed_binaries_grouped_by_network(None)?;

    
    
     let mut install_binaries: BTreeMap<String, Vec<String>> = BTreeMap::new();
     let mut is_default_grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for (network, binaries) in installed_binaries {
        
        for binarie in &binaries {
            let binary = binarie.binary_name.clone();
            let version = binarie.version.clone();
            let binary_version = format!("{}-{}", network, version); 
            let binary_version_key = format!("{}-{}-{}",network, binary, version);

            
            if let Some(f) = install_binaries.get_mut(&binary.to_string()) {
                f.push(binary_version.clone());
            } else {
                install_binaries.insert(binary.to_string(), vec![binary_version]);
            }

            let is_default = default_list.contains(&binary_version_key);
            let default_flag = if is_default {
                "*".to_string()
            }else{
                "".to_string()
            };

            if let Some(f) = is_default_grouped.get_mut(&binary.to_string()) {
                f.push(default_flag);
            } else {
                is_default_grouped.insert(binary.to_string(), vec![default_flag]);
            }
            
            
        }
    }
       
  


    let mut builder = TableBuilder::default();
    builder.set_header(["alias", "release", "default"]);
    for (alias, binaries) in install_binaries {
        let string = binaries.join("\n"); 
        let is_default_list = is_default_grouped[&alias].join("\n");
        builder.push_record(vec![alias.clone(), string.clone(), 
                is_default_list.clone()
        ]);
    }
    let mut table = builder.build();
    table.with(TableStyle::extended());
    println!("{}",table);
    Ok(())
}


