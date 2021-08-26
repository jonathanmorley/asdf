use std::{fs, path::PathBuf, str::FromStr};

use anyhow::{anyhow, Error, Result};

use crate::core::latest::get_latest_version;
use crate::installs_path;

#[derive(Debug)]
pub enum ToolVersion {
    Latest(Option<String>),
    Version(String),
    Path(PathBuf),
    Ref(String),
}

impl FromStr for ToolVersion {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            Err(anyhow!("Cannot parse empty string as a tool version"))
        } else {
            if s.starts_with("ref:") {
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
}

impl ToolVersion {
    pub fn install_type(&self) -> String {
        match self {
            &ToolVersion::Latest(_) => "version".to_string(),
            ToolVersion::Version(_) => "version".to_string(),
            ToolVersion::Path(_) => "path".to_string(),
            ToolVersion::Ref(_) => "ref".to_string(),
        }
    }

    pub fn install_version(&self, plugin_name: &str) -> Result<Option<String>> {
        match self {
            ToolVersion::Latest(version) => {
                get_latest_version(plugin_name, version.as_deref().unwrap_or_default()).map(Some)
            }
            ToolVersion::Version(version) => Ok(Some(version.to_string())),
            ToolVersion::Path(_) => Ok(None),
            ToolVersion::Ref(version) => Ok(Some(version.to_string())),
        }
    }

    pub fn install_path(&self, plugin_name: &str) -> Result<PathBuf> {
        let plugin_dir = installs_path()?.join(plugin_name);
        fs::create_dir_all(&plugin_dir)?;

        Ok(match self {
            ToolVersion::Latest(None) => plugin_dir.join("latest"),
            ToolVersion::Latest(Some(version)) => plugin_dir.join(version),
            ToolVersion::Version(version) => plugin_dir.join(version),
            ToolVersion::Path(path) => path.to_owned(),
            ToolVersion::Ref(version) => plugin_dir.join(format!("ref-{}", version)),
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
            tmp_dir.path().join("installs").join("foo").join("1.0.0")
        );
        assert!(install_path.parent().unwrap().is_dir());

        Ok(())
    }
}
