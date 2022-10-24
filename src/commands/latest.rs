use anyhow::Result;
use asdr::core::latest::{get_latest_version, get_all_latest_versions};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct LatestCommand {
    #[structopt(required_unless = "all")]
    plugin_name: Option<String>,
    #[structopt(default_value = "[0-9]")]
    query: String,
    #[structopt(long, conflicts_with = "plugin_name")]
    _all: bool,
}

impl LatestCommand {
    pub fn run(&self) -> Result<()> {
        if let Some(plugin_name) = &self.plugin_name {
            println!("{}", get_latest_version(&plugin_name, &self.query)?);
        } else {
            println!("{}", get_all_latest_versions()?);
        }
        
        Ok(())
    }
}
