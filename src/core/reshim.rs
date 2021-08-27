use anyhow::Result;
use is_executable::IsExecutable;
use itertools::Itertools;
use std::{collections::HashSet, env, ffi::OsStr, fs, os::unix::prelude::PermissionsExt, path::{Path, PathBuf}};

use crate::{asdf_data_dir, asdf_run_hook, list_installed_plugins, list_installed_versions, plugin_executables, plugin_exists, plugin_installs_path, plugin_shims, shims_path};

pub fn reshim_plugins() -> Result<()> {
  for plugin_name in list_installed_plugins()? {
    reshim_plugin(&plugin_name, None)?;
  }

  Ok(())
}

pub fn reshim_plugin(plugin_name: &str, full_version: Option<&str>) -> Result<()> {
    plugin_exists(&plugin_name)?;
    ensure_shims_dir()?;

    if let Some(ref full_version) = full_version {
        // generate for the whole package version
        asdf_run_hook(
            &format!("pre_asdf_reshim_{}", plugin_name),
            &[&full_version],
            vec![
                ("plugin_name", OsStr::new(&plugin_name)),
                ("full_version", OsStr::new(&full_version)),
                // There are more available via bash because of the variable non-locality
            ],
        )?;
        generate_shims_for_version(plugin_name, full_version)?;
        asdf_run_hook(
            &format!("post_asdf_reshim_{}", plugin_name),
            &[&full_version],
            vec![
                ("plugin_name", OsStr::new(&plugin_name)),
                ("full_version", OsStr::new(&full_version)),
                // There are more available via bash because of the variable non-locality
            ],
        )?;
    } else {
        // generate for all versions of the package
        let plugin_installs_path = plugin_installs_path(&plugin_name)?;
        for version in list_installed_versions(&plugin_name)? {
            let full_version_name = PathBuf::from(&version)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .replace("ref-", "ref:");
            asdf_run_hook(
                &format!("pre_asdf_reshim_{}", plugin_name),
                &[&full_version_name],
                vec![
                    ("full_version_name", OsStr::new(&full_version_name)),
                    ("plugin_installs_path", plugin_installs_path.as_os_str()),
                    ("plugin_name", OsStr::new(&plugin_name)),
                    ("full_version", OsStr::new("")),
                    // There are more available via bash because of the variable non-locality
                ],
            )?;
            generate_shims_for_version(plugin_name, &full_version_name)?;
            remove_obsolete_shims(plugin_name, &full_version_name)?;
            asdf_run_hook(
                &format!("post_asdf_reshim_{}", plugin_name),
                &[&full_version_name],
                vec![
                    ("full_version_name", OsStr::new(&full_version_name)),
                    ("plugin_installs_path", plugin_installs_path.as_os_str()),
                    ("plugin_name", OsStr::new(&plugin_name)),
                    ("full_version", OsStr::new("")),
                    // There are more available via bash because of the variable non-locality
                ],
            )?;
        }
    }

    Ok(())
}

pub fn ensure_shims_dir() -> Result<()> {
    // Create shims dir if doesn't exist
    let shims_path = shims_path()?;
    if !shims_path.is_dir() {
        fs::create_dir(shims_path)?;
    }

    Ok(())
}

pub fn write_shim_script(plugin_name: &str, version: &str, executable_path: &Path) -> Result<()> {
  if !executable_path.is_executable() {
    return Ok(())
  }

  let executable_name = executable_path.file_name().unwrap();
  let shim_path = asdf_data_dir()?.join("shims").join(&executable_name);
  
  let shim_contents = if shim_path.is_file() {
    let contents = fs::read_to_string(&shim_path)?;
    contents.replacen("exec ", &format!("# asdf-plugin: {} {}\nexec ", plugin_name, version), 1)
  } else {
    format!(r#"#!/usr/bin/env bash
# asdf-plugin: {} {}
exec {} exec "{}" "$@"
"#, plugin_name, version, env::current_exe()?.as_os_str().to_string_lossy(), executable_name.to_string_lossy())
  };

  fs::write(&shim_path, shim_contents)?;

  fs::set_permissions(shim_path, PermissionsExt::from_mode(0o755))?;

  Ok(())
}

pub fn generate_shims_for_version(plugin_name: &str, full_version: &str) -> Result<()> {
  let all_executable_paths = plugin_executables(plugin_name, full_version)?;
  for executable_path in all_executable_paths {
    write_shim_script(plugin_name, full_version, &executable_path)?;
  }

  Ok(())
}

fn remove_obsolete_shims(plugin_name: &str, full_version: &str) -> Result<()> {
  let shims = plugin_shims(plugin_name, full_version)?
    .into_iter()
    .map(|shim| shim.file_name().unwrap_or(shim.as_os_str()).to_owned())
    .collect::<HashSet<_>>();

  let exec_names = plugin_executables(plugin_name, full_version)?
    .into_iter()
    .map(|exec| exec.file_name().unwrap_or(exec.as_os_str()).to_owned())
    .collect::<HashSet<_>>();

  // lines only in formatted_shims
  for shim_name in shims.difference(&exec_names) {
    remove_shim_for_version(plugin_name, full_version, shim_name)?;
  }

  Ok(())
}

fn remove_shim_for_version(plugin_name: &str, version: &str, shim: &OsStr) -> Result<()> {
  let shim_path_buf = PathBuf::from(shim);
  let shim_name = shim_path_buf.file_name().unwrap_or(shim);
  let shim_path = shims_path()?.join(shim_name);

  let count_installed = list_installed_versions(plugin_name)?.len();

  let shim_contents = fs::read_to_string(&shim_path)?
    .lines()
    .filter(|line| line.ne(&format!("# asdf-plugin: {} {}", plugin_name, version)))
    .join("\n");

  if !shim_contents.contains("# asdf-plugin:") || count_installed == 0 {
    fs::remove_file(&shim_path)?;
  } else {
    fs::write(&shim_path, shim_contents)?;
  }

  Ok(())
}