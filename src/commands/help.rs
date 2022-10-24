use anyhow::{anyhow, Result};
use asdr::asdf_data_dir;
use asdr::core::help::plugin_help;
use asdr::tool_versions::ToolVersion;
use structopt::StructOpt;

use crate::Command as AsdfCommand;

#[derive(StructOpt, Debug)]
pub struct HelpCommand {
    plugin_name: Option<String>,
    tool_version: Option<ToolVersion>,
}

impl HelpCommand {
    pub fn run(&self) -> Result<()> {
        if let Some(ref plugin_name) = self.plugin_name {
            let plugin_path = asdf_data_dir()?.join("plugins").join(plugin_name);

            if plugin_path.is_dir() {
                let overview_path = plugin_path.join("bin").join("help.overview");

                if overview_path.is_file() {
                    println!("{}", plugin_help(&plugin_name, self.tool_version.as_ref())?);
                    Ok(())
                } else {
                    Err(anyhow!("No documentation for plugin {}", plugin_name))
                }
            } else {
                Err(anyhow!("No rust plugin named {}", plugin_name))
            }
        } else {
            AsdfCommand::clap().print_long_help()?;
            println!();
            Ok(())
        }
    }
}
