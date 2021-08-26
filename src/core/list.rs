use crate::{plugin_exists, plugin_path};
use anyhow::{anyhow, Result};
use regex::Regex;
use std::process::Command;
use std::str;

pub fn all_plugin_versions(plugin_name: &str, tool_version: Option<&str>) -> Result<Vec<String>> {
    let plugin_path = plugin_path(plugin_name)?;

    if plugin_exists(plugin_name).is_ok() {
        let output = Command::new(plugin_path.join("bin").join("list-all")).output()?;

        if output.status.success() {
            let stdout = String::from_utf8(output.stdout)?;
            let versions = stdout.split(' ');

            let filtered_versions: Vec<_> = if let Some(ref query) = tool_version {
                let re = Regex::new(&format!(r"^\s*{}", query))?;

                versions
                    .filter(|line| re.is_match(line))
                    .map(String::from)
                    .collect()
            } else {
                versions.map(String::from).collect()
            };

            if filtered_versions.is_empty() {
                Err(anyhow!(
                    "No compatible versions available ({} {})",
                    plugin_name,
                    tool_version.to_owned().unwrap_or_default()
                ))
            } else {
                return Ok(filtered_versions);
            }
        } else {
            Err(anyhow!(
                "Plugin {}'s list-all callback script failed with output:\n{}\n{}\n",
                plugin_name,
                str::from_utf8(&output.stderr)?,
                str::from_utf8(&output.stdout)?
            ))
        }
    } else {
        Err(anyhow!("Plugin {} not found", plugin_name))
    }
}
