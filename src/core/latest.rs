use crate::{call, plugin_path, list_installed_plugins, list_installed_versions};
use anyhow::{anyhow, Result};
use itertools::Itertools;
use regex::Regex;
use std::process::{Command, self};

use crate::core::list::all_plugin_versions;

pub fn get_latest_version(plugin_name: &str, query: &str) -> Result<String> {
    let plugin_path = plugin_path(&plugin_name)?;

    let latest_stable_path = plugin_path.join("bin").join("latest-stable");
    if latest_stable_path.is_file() {
        let versions = call(Command::new(latest_stable_path).arg(&query))?;

        if versions.is_empty() {
            Err(anyhow!(
                "No compatible versions available ({} {})",
                plugin_name,
                query
            ))
        } else {
            Ok(versions)
        }
    } else {
        // pattern from xxenv-latest (https://github.com/momo-lab/xxenv-latest)
        let re = Regex::new(
            r"(^Available versions:|-src|-dev|-latest|-stm|[-\\.]rc|-alpha|-beta|[-\\.]pre|-next|(a|b|c)[0-9]+|snapshot|master)",
        )?;

        all_plugin_versions(plugin_name, Some(query))?
            .into_iter()
            .filter(|version| !re.is_match(version))
            .map(|version| version.replace(r"^\s\+", ""))
            .last()
            .ok_or_else(|| anyhow!(""))
    }
}

pub fn get_all_latest_versions() -> Result<String> {
    let installed_plugins = list_installed_plugins()?;

    if installed_plugins.is_empty() {
        return Ok(String::from("No plugins installed"));
    }

    let mut plugin_versions = Vec::new();

    for plugin in list_installed_plugins()? {
        let plugin_path = plugin_path(&plugin)?;
        let latest_stable_path = plugin_path.join("bin").join("latest-stable");
        
        let version = if latest_stable_path.exists() {
            // We can't filter by a concrete query because different plugins might
            // have different queries.
            call(&mut process::Command::new(&latest_stable_path)).ok()
        } else {
            // pattern from xxenv-latest (https://github.com/momo-lab/xxenv-latest)
            let re = Regex::new(
                r"(^Available versions:|-src|-dev|-latest|-stm|[-\\.]rc|-alpha|-beta|[-\\.]pre|-next|(a|b|c)[0-9]+|snapshot|master)",
            )?;

            all_plugin_versions(&plugin, None)?
                .into_iter()
                .filter(|version| !re.is_match(version))
                .map(|version| version.replace(r"^\s\+", ""))
                .last()
        }.unwrap_or(String::from("unknown"));

        let installed_status = if list_installed_versions(&plugin)?.contains(&version) {
            "installed"
        } else {
            "missing"
        };

        plugin_versions.push(format!("{}\t{}\t{}", plugin, version, installed_status));
    }

    Ok(plugin_versions.into_iter().join("\n"))
}