use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Result};
use asdf::tool_version::ToolVersion;
use asdf::{asdf_data_dir, plugin_path};
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
                Err(anyhow!("No plugin named {}", plugin_name))
            }
        } else {
            AsdfCommand::clap().print_long_help()?;
            println!();
            Ok(())
        }
    }
}

fn plugin_help(plugin_name: &str, tool_version: Option<&ToolVersion>) -> Result<String> {
    let plugin_bin_path = plugin_path(plugin_name)?.join("bin");

    let mut help_messages = String::new();

    let overview_path = plugin_bin_path.join("help.overview");
    help_messages.push_str(&get_output(&overview_path, plugin_name, tool_version)?);

    for help_type in &["deps", "config", "links"] {
        let help_path = plugin_bin_path.join(format!("help.{}", help_type));

        if help_path.is_file() {
            help_messages.push_str(&get_output(&help_path, plugin_name, tool_version)?);
        }
    }

    Ok(help_messages)
}

fn get_output(cmd: &Path, plugin_name: &str, tool_version: Option<&ToolVersion>) -> Result<String> {
    let mut cmd = Command::new(cmd);

    if let Some(tool_version) = tool_version {
        cmd.env("ASDF_INSTALL_TYPE", tool_version.install_type());

        if let Some(install_version) = tool_version.install_version() {
            cmd.env("ASDF_INSTALL_VERSION", install_version);
        }

        cmd.env("ASDF_INSTALL_PATH", tool_version.install_path(plugin_name)?);
    }

    String::from_utf8(cmd.output()?.stdout).map_err(Into::into)
}
