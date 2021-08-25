use anyhow::{anyhow, Result};
use asdf::plugin_path;
use regex::Regex;
use std::process::Command;
use structopt::StructOpt;

use crate::commands::list::all_plugin_versions;

#[derive(StructOpt, Debug)]
pub struct LatestCommand {
    plugin_name: String,
    #[structopt(default_value = "[0-9]")]
    query: String,
}

impl LatestCommand {
    pub fn run(&self) -> Result<()> {
        println!("{}", get_latest_version(&self.plugin_name, &self.query)?);
        Ok(())
    }
}

fn get_latest_version(plugin_name: &str, query: &str) -> Result<String> {
    let plugin_path = plugin_path(&plugin_name)?;

    let latest_stable_path = plugin_path.join("bin").join("latest-stable");
    if latest_stable_path.is_file() {
        let versions = Command::new(latest_stable_path)
            .arg(&query)
            .output()?
            .stdout;
        if versions.is_empty() {
            Err(anyhow!(
                "No compatible versions available ({} {})",
                plugin_name,
                query
            ))
        } else {
            return String::from_utf8(versions).map_err(Into::into);
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
