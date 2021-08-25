use anyhow::{anyhow, Result};
use asdf::{list_installed_versions, plugin_exists, plugin_path, plugins_path};
use regex::Regex;
use std::fs;
use std::process::Command;
use std::str;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]

pub struct ListCommand {
    #[structopt(subcommand)]
    cmd: Option<ListSubCommand>,
    #[structopt(flatten)]
    default: ListInstalledCommand,
}

#[derive(StructOpt, Debug)]
pub struct ListInstalledCommand {
    plugin_name: Option<String>,
    tool_version: Option<String>,
}

#[derive(StructOpt, Debug)]
pub struct ListAllCommand {
    plugin_name: String,
    tool_version: Option<String>,
}

#[derive(StructOpt, Debug)]
pub enum ListSubCommand {
    All(ListAllCommand),
}

impl ListCommand {
    pub fn run(&self) -> Result<()> {
        match &self.cmd {
            Some(ListSubCommand::All(cmd)) => cmd.run(),
            None => {
                if let Some(ref plugin_name) = self.default.plugin_name {
                    if plugin_exists(plugin_name).is_ok() {
                        Self::display_installed_versions(
                            plugin_name,
                            self.default.tool_version.as_deref(),
                        )?;
                        Ok(())
                    } else {
                        Err(anyhow!("Plugin {} not found", plugin_name))
                    }
                } else {
                    let plugins_path = plugins_path()?;

                    if let Ok(plugins) = fs::read_dir(plugins_path) {
                        for plugin in plugins {
                            let plugin_name = plugin?
                                .file_name()
                                .into_string()
                                .map_err(|_| anyhow!("Cannot parse filename as unicode"))?;
                            println!("{}", plugin_name);
                            Self::display_installed_versions(
                                &plugin_name,
                                self.default.tool_version.as_deref(),
                            )?;
                        }
                    } else {
                        println!("No plugins installed");
                    }

                    Ok(())
                }
            }
        }
    }

    fn display_installed_versions(plugin_name: &str, query: Option<&str>) -> Result<()> {
        let mut versions = list_installed_versions(plugin_name)?;

        if let Some(query) = query {
            let re = Regex::new(&format!(r"^\s*{}", query))?;
            versions = versions
                .into_iter()
                .filter(|version| re.is_match(version))
                .collect();

            if versions.is_empty() {
                return Err(anyhow!(
                    "No compatible versions installed ({} {})",
                    plugin_name,
                    query
                ));
            }
        }

        if versions.is_empty() {
            eprintln!("  No versions installed");
        } else {
            for version in versions {
                println!("  {}", version)
            }
        }

        Ok(())
    }
}

impl ListAllCommand {
    pub fn run(&self) -> Result<()> {
        for version in all_plugin_versions(&self.plugin_name, self.tool_version.as_deref())? {
            println!("{}", version);
        }

        Ok(())
    }
}

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
