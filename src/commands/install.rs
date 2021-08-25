use anyhow::{anyhow, Result};
use asdf::list_installed_plugins;
use std::env;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct InstallCommand {
    plugin_name: Option<String>,
    tool_version: Option<String>,
    extra_args: Vec<String>,
}

impl InstallCommand {
    pub fn run(&self) -> Result<()> {
        match (&self.plugin_name, &self.tool_version) {
            (None, None) => install_local_tool_versions(),
            (Some(ref plugin_name), None) => install_one_local_tool(plugin_name),
            (Some(ref plugin_name), Some(ref tool_version)) => {
                install_tool_version(&plugin_name, &tool_version, &self.extra_args)
            }
            _ => Err(anyhow!("Unexpected arguments")),
        }
    }
}

fn install_local_tool_versions() -> Result<()> {
    let plugins = list_installed_plugins()?;

    if plugins.is_empty() {
        return Err(anyhow!("Install plugins first to be able to install tools"));
    }

    let search_path = env::current_dir()?;
    let mut some_tools_installed = false;

    for plugin in plugins {}
    Ok(())
}

fn install_one_local_tool(plugin_name: &str) -> Result<()> {
    Ok(())
}

fn install_tool_version(
    plugin_name: &str,
    tool_version: &str,
    extra_args: &[String],
) -> Result<()> {
    Ok(())
}
