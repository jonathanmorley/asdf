use anyhow::Result;
use asdf::core::latest::get_latest_version;
use structopt::StructOpt;

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
