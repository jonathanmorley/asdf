use anyhow::Result;
use asdf::core::reshim::{reshim_plugin, reshim_plugins};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct ReshimCommand {
    plugin_name: Option<String>,
    full_version: Option<String>,
}

impl ReshimCommand {
    pub fn run(&self) -> Result<()> {
        match self.plugin_name {
            Some(ref plugin_name) => reshim_plugin(plugin_name, self.full_version.as_deref()),
            None => reshim_plugins(),
        }
    }
}
