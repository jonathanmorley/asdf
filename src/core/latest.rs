use crate::{call, plugin_path};
use anyhow::{anyhow, Result};
use regex::Regex;
use std::process::Command;

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
            .ok_or_else(|| anyhow!("No versions found"))
    }
}
