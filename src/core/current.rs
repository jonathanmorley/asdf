use anyhow::{anyhow, Result};
use itertools::Itertools;

use crate::{plugin_exists, find_versions, version_exists, VersionSpecifier, VersionSource, plugin_path, asdf_config_value};

pub fn get_current_version(plugin_name: &str) -> Result<()> {
  plugin_exists(plugin_name)?;

  let search_path = std::env::current_dir()?;
  let versions = find_versions(plugin_name, &search_path)?;

  let uninstalled_versions = if let Some(VersionSpecifier { versions, .. }) = &versions {
    versions.into_iter().filter_map(|version| version_exists(plugin_name, version).err()).collect()
  } else {
    vec![]
  };

  check_for_deprecated_plugin(&plugin_name)?;

  match versions {
    Some(VersionSpecifier { versions, source }) => if uninstalled_versions.is_empty() {
      match source {
        VersionSource::ToolVersion(path) | VersionSource::Legacy(path) => {
          println!("{:15} {:15} {:10}", plugin_name, versions.iter().join(" "), path.to_string_lossy());
          Ok(())
        },
        VersionSource::EnvVar(var) => {
          println!("{:15} {:15} {:10}", plugin_name, versions.iter().join(" "), var);
          Ok(())
        }
      }
    } else {
      let description = format!(r#"Not installed. Run "asdf install {plugin_name} {}""#, versions[0]);
      println!("{plugin_name:15} {:15} {description:10}", versions.iter().join(" "));
      Err(anyhow!(""))
    },
    None => {
      let description = format!(r#"No version is set. Run "asdf <global|shell|local> {plugin_name} <version>""#);
      println!("{plugin_name:15} {:15} {description:10}", "______");
      Err(anyhow!("No plugin version set"))
    }
  }
}

// Warn if the plugin isn't using the updated legacy file api.
fn check_for_deprecated_plugin(plugin_name: &str) -> Result<()> {
  let plugin_path = plugin_path(plugin_name)?;
  let legacy_config = asdf_config_value("legacy_version_file")?;
  let deprecated_script = plugin_path.join("bin").join("get-version-from-legacy-file");
  let new_script = plugin_path.join("bin").join("list-legacy-filenames");

  if legacy_config == Some(String::from("yes")) && deprecated_script.exists() && !new_script.exists() {
    eprintln!("Heads up! It looks like your {plugin_name} plugin is out of date. You can update it with:\n");
    eprintln!("  asdf plugin-update {plugin_name}\n");
  }

  Ok(())
}
