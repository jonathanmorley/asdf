use std::fmt::Display;
use std::{fs, path::PathBuf, str::FromStr};
use std::collections::HashMap;

use anyhow::{anyhow, Error, Result};
use itertools::Itertools;

use crate::core::latest::get_latest_version;
use crate::installs_path;

pub struct ToolVersions(pub HashMap<String, Vec<ToolVersion>>);

impl FromStr for ToolVersions {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ToolVersions(s
            .lines()
            // Remove comments
            .filter_map(|line| {
                // Remove whitespace before pound sign, the pound sign, and everything after it
                let uncommented = if let Some(pound_index) = line.find("#") {
                    line[..pound_index].trim_end()
                } else {
                    line.trim_end()
                };
            
                if uncommented.is_empty() {
                    None
                } else {
                    Some(uncommented)
                }
            })
            .map(|line| {
                if let Some((plugin_name, versions)) = line.split_once(" ") {
                    // Paths may contain spaces themselves, and so are treated specially.
                    // They do not allow fallthrough
                    if versions.starts_with("path:") {
                        Ok((plugin_name.to_owned(), vec![versions.parse()?]))
                    } else {
                        let tool_versions = versions.split_whitespace().map(ToolVersion::from_str).collect::<Result<Vec<_>>>()?;
                        Ok((plugin_name.to_owned(), tool_versions))
                    }
                } else {
                    Err(anyhow!("Cannot parse .tool-versions line: {}", line))
                }
            })
            .collect::<Result<HashMap<_, _>>>()?
        ))
    }
}

#[derive(Debug, PartialEq)]
pub enum ToolVersion {
    Latest(Option<String>),
    Path(PathBuf),
    Ref(String),
    System,
    Version(String),
}

impl FromStr for ToolVersion {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            Err(anyhow!("Cannot parse empty string as a tool version"))
        } else if s.eq("system") {
            Ok(ToolVersion::System)
        } else if s.starts_with("ref:") {
            Ok(ToolVersion::Ref(s[4..].to_owned()))
        } else if s.eq("latest") {
            Ok(ToolVersion::Latest(None))
        } else if s.starts_with("latest:") {
            Ok(ToolVersion::Latest(Some(s[7..].to_owned())))
        } else {
            Ok(ToolVersion::Version(s.to_owned()))
        }
    }
}

impl Display for ToolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {    
            ToolVersion::Latest(Some(version)) => f.write_fmt(format_args!("latest:{version}")),
            ToolVersion::Latest(None) => f.write_str("latest"),
            ToolVersion::Path(path) => f.write_fmt(format_args!("path:{}", path.to_string_lossy())),
            ToolVersion::Ref(sha) => f.write_fmt(format_args!("ref:{sha}")),
            ToolVersion::System => f.write_str("system"),
            ToolVersion::Version(version) => f.write_str(version)
        }
    }
}

impl ToolVersion {
    pub fn install_type(&self) -> String {
        match self {
            ToolVersion::Latest(_) => "version".to_string(),
            ToolVersion::Path(_) => "path".to_string(),
            ToolVersion::Ref(_) => "ref".to_string(),
            ToolVersion::System => "system".to_string(),
            ToolVersion::Version(_) => "version".to_string(),
        }
    }

    pub fn install_version(&self, plugin_name: &str) -> Result<Option<String>> {
        match self {
            ToolVersion::Latest(version) => {
                get_latest_version(plugin_name, version.as_deref().unwrap_or_default()).map(Some)
            },
            ToolVersion::Path(_) => Ok(None),
            ToolVersion::Ref(version) => Ok(Some(version.to_string())),
            ToolVersion::System => Ok(None),
            ToolVersion::Version(version) => Ok(Some(version.to_string())),
        }
    }

    pub fn install_path(&self, plugin_name: &str) -> Result<Option<PathBuf>> {
        let plugin_dir = installs_path()?.join(plugin_name);
        fs::create_dir_all(&plugin_dir)?;

        Ok(match self {
            ToolVersion::Latest(None) => Some(plugin_dir.join("latest")),
            ToolVersion::Latest(Some(version)) => Some(plugin_dir.join(version)),
            ToolVersion::Path(path) => Some(path.to_owned()),
            ToolVersion::Ref(version) => Some(plugin_dir.join(format!("ref-{}", version))),
            ToolVersion::System => None,
            ToolVersion::Version(version) => Some(plugin_dir.join(version)),
        })
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use serial_test::serial;
    use tempfile::TempDir;
    use tmp_env::set_var;

    // get_install_path should output version path when version is provided
    #[test]
    #[serial]
    fn install_path_with_version() -> Result<()> {
        let tmp_dir = TempDir::new()?;
        let _env = set_var("ASDF_DATA_DIR", tmp_dir.path());

        let tool_version = "1.0.0".parse::<super::ToolVersion>()?;
        let install_path = tool_version.install_path("foo")?;

        assert_eq!(
            install_path,
            Some(tmp_dir.path().join("installs").join("foo").join("1.0.0"))
        );
        assert!(install_path.unwrap().parent().unwrap().is_dir());

        Ok(())
    }
}
