use std::path::Path;
use std::process::Command;

use crate::plugin_path;
use crate::tool_versions::ToolVersion;
use anyhow::Result;

pub fn plugin_help(plugin_name: &str, tool_version: Option<&ToolVersion>) -> Result<String> {
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

pub fn get_output(
    cmd: &Path,
    plugin_name: &str,
    tool_version: Option<&ToolVersion>,
) -> Result<String> {
    let mut cmd = Command::new(cmd);

    if let Some(tool_version) = tool_version {
        cmd.env("ASDF_INSTALL_TYPE", tool_version.install_type());

        if let Some(install_version) = tool_version.install_version(plugin_name)? {
            cmd.env("ASDF_INSTALL_VERSION", install_version);
        }

        cmd.env("ASDF_INSTALL_PATH", tool_version.install_path(plugin_name)?.unwrap());
    }

    String::from_utf8(cmd.output()?.stdout).map_err(Into::into)
}
