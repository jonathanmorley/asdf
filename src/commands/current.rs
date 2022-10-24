use anyhow::Result;
use asdr::{core::current::get_current_version, list_installed_plugins};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct CurrentCommand {
    plugin_name: Option<String>
}

impl CurrentCommand {
  pub fn run(&self) -> Result<()> {
    if let Some(plugin_name) = &self.plugin_name {
      get_current_version(&plugin_name)?;
    } else {
      for plugin_name in list_installed_plugins()? {
        // ignore must use here, we dont care about errors
        get_current_version(&plugin_name);
      }
    }

    Ok(())
  }
}
