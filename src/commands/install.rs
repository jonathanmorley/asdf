use anyhow::{anyhow, Result};
use asdr::core::installs::{
    install_local_tool_versions, install_one_local_tool, install_tool_version,
};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct InstallCommand {
    plugin_name: Option<String>,
    tool_version: Option<String>,
    #[structopt(long)]
    keep_download: bool,
}

impl InstallCommand {
    pub fn run(&self) -> Result<()> {
        match (&self.plugin_name, &self.tool_version) {
            (None, None) => install_local_tool_versions(),
            (Some(ref plugin_name), None) => install_one_local_tool(plugin_name),
            (Some(ref plugin_name), Some(ref tool_version)) => {
                install_tool_version(&plugin_name, &tool_version, self.keep_download)
            }
            _ => Err(anyhow!("Unexpected arguments")),
        }
    }
}
