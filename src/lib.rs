pub mod core;
pub mod tool_versions;

use anyhow::{anyhow, Result};
use dirs;
use is_executable::IsExecutable;
use std::collections::HashMap;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::DirEntry;
use std::path::{Path, PathBuf};
use std::process;
use std::{fs, str};
use tool_versions::{ToolVersion, ToolVersions};
use which::which_in;

#[derive(Debug, PartialEq)]
pub struct VersionSpecifier {
    pub version: String,
    source: VersionSource,
}

#[derive(Debug, PartialEq)]
pub enum VersionSource {
    ToolVersion(PathBuf),
    Legacy(PathBuf),
    EnvVar(String),
}

// asdf_version

// asdf_dir
pub fn asdf_dir() -> Result<PathBuf> {
    if let Some(var) = std::env::var_os("ASDF_DIR") {
        return Ok(var.into());
    } else {
        let exe_path = std::env::current_exe()?;
        return Ok(exe_path
            .parent()
            .ok_or(anyhow!("No parent found"))?
            .parent()
            .ok_or(anyhow!("No parent found"))?
            .parent()
            .ok_or(anyhow!("No parent found"))?
            .to_path_buf());
    }
}

// asdf_repository_url

// asdf_data_dir
pub fn asdf_data_dir() -> Result<PathBuf> {
    env::var_os("ASDF_DATA_DIR")
        .map(Into::into)
        .or_else(|| dirs::home_dir().map(|home| home.join(".asdf")))
        .ok_or_else(|| anyhow!("Cannot find asdf data directory"))
}

// get_install_path
pub fn install_path(plugin_name: &str, install_type: &str, version: &str) -> Result<PathBuf> {
    let plugin_dir = plugin_installs_path(plugin_name)?;
    fs::create_dir_all(&plugin_dir)?;

    Ok(match install_type {
        "version" => plugin_dir.join(version),
        "path" => PathBuf::from(version),
        other => plugin_dir.join(format!("{}-{}", other, version)),
    })
}

// get_download_path
pub fn download_path(
    plugin_name: &str,
    install_type: &str,
    version: &str,
) -> Result<Option<PathBuf>> {
    let downloads_path = plugin_downloads_path(plugin_name)?;
    fs::create_dir_all(&downloads_path)?;

    Ok(match install_type {
        "version" => Some(downloads_path.join(version)),
        "path" => None,
        other => Some(downloads_path.join(format!("{}-{}", other, version))),
    })
}

// list_installed_versions
pub fn list_installed_versions(plugin_name: &str) -> Result<Vec<String>> {
    let plugin_installs_path = plugin_installs_path(plugin_name)?;

    if plugin_installs_path.is_dir() {
        let mut versions = fs::read_dir(plugin_installs_path)?
            .map(|result| {
                result.map_err(Into::into).and_then(|entry| {
                    entry
                        .file_name()
                        .into_string()
                        .map(|version| version.replace("^ref-", "ref:"))
                        .map_err(|_| anyhow!("Cannot parse filename as unicode"))
                })
            })
            .collect::<Result<Vec<_>>>()?;
        versions.sort();

        Ok(versions)
    } else {
        Ok(vec![])
    }
}

// check_if_plugin_exists
pub fn plugin_exists(plugin_name: &str) -> Result<()> {
    if plugin_name.is_empty() {
        Err(anyhow!("No plugin given"))
    } else {
        if !plugin_path(plugin_name)?.is_dir() {
            Err(anyhow!("No such plugin: {}", plugin_name))
        } else {
            Ok(())
        }
    }
}

// check_if_version_exists
pub fn version_exists(plugin_name: &str, version: &str) -> Result<()> {
    plugin_exists(plugin_name)?;

    if let Some(install_path) = find_install_path(plugin_name, version)? {
        if !install_path.is_dir() {
            return Err(anyhow!(""));
        }
    }

    Ok(())
}

// version_not_installed_text

// get_plugin_path
pub fn plugin_path(plugin_name: &str) -> Result<PathBuf> {
    Ok(plugins_path()?.join(plugin_name))
}

// display_error

// get_version_in_dir
pub fn version_in_dir(
    plugin_name: &str,
    search_path: &Path,
    legacy_filenames: &[PathBuf],
) -> Result<Option<VersionSpecifier>> {
    let tool_versions_path = search_path.join(".tool-versions");
    let asdf_version = parse_asdf_version_file(&tool_versions_path, plugin_name)?;

    if let Some(asdf_version) = asdf_version {
        return Ok(Some(VersionSpecifier {
            version: asdf_version,
            source: VersionSource::ToolVersion(tool_versions_path),
        }));
    }

    for legacy_filename in legacy_filenames {
        let legacy_file_path = search_path.join(legacy_filename);
        let legacy_version = parse_legacy_version_file(&legacy_file_path, plugin_name)?;

        if let Some(legacy_version) = legacy_version {
            return Ok(Some(VersionSpecifier {
                version: legacy_version,
                source: VersionSource::Legacy(legacy_file_path),
            }));
        }
    }

    Ok(None)
}

// find_versions
pub fn find_versions(plugin_name: &str, search_path: &Path) -> Result<Option<VersionSpecifier>> {
    let version = version_from_env(plugin_name)?;

    if let Some(version) = version {
        return Ok(Some(VersionSpecifier {
            version,
            source: VersionSource::EnvVar(env_var_for_plugin(plugin_name)),
        }));
    }

    let legacy_config = asdf_config_value("legacy_version_file")?;
    let legacy_list_filenames_script = plugin_path(plugin_name)?
        .join("bin")
        .join("list-legacy-filenames");

    let legacy_filenames: Vec<PathBuf> =
        if Some(String::from("yes")) == legacy_config && legacy_list_filenames_script.is_file() {
            call(&mut process::Command::new(&legacy_list_filenames_script))?
                .split_whitespace()
                .map(Into::into)
                .collect()
        } else {
            vec![]
        };

    let mut current_path = Some(search_path);
    while let Some(path) = current_path {
        if let Some(version) = version_in_dir(plugin_name, path, &legacy_filenames)? {
            return Ok(Some(version));
        } else {
            current_path = path.parent();
        }
    }

    if let Some(home) = dirs::home_dir() {
        if let Some(version) = version_in_dir(plugin_name, &home, &legacy_filenames)? {
            return Ok(Some(version));
        }
    }

    if let Some(asdf_default_tool_versions_filename) =
        env::var_os("ASDF_DEFAULT_TOOL_VERSIONS_FILENAME")
    {
        let asdf_default_tool_versions_path = PathBuf::from(asdf_default_tool_versions_filename);

        if asdf_default_tool_versions_path.is_file() {
            let versions = parse_asdf_version_file(&asdf_default_tool_versions_path, plugin_name)?;

            if let Some(versions) = versions {
                return Ok(Some(VersionSpecifier {
                    version: versions,
                    source: VersionSource::ToolVersion(asdf_default_tool_versions_path),
                }));
            }
        }
    }

    Ok(None)
}

// display_no_version_set

// get_version_from_env
fn version_from_env(plugin_name: &str) -> Result<Option<String>> {
    let version_env_var = env_var_for_plugin(plugin_name);

    env::var_os(&version_env_var)
        .map(OsString::into_string)
        .transpose()
        .map_err(|_| anyhow!("Cannot parse env var: {} as unicode", version_env_var))
}

// find_install_path
pub fn find_install_path(plugin_name: &str, version: &str) -> Result<Option<PathBuf>> {
    if version == "system" {
        Ok(None)
    } else {
        let split = version.splitn(2, ':').collect::<Vec<_>>();

        match split.len() {
            1 => install_path(plugin_name, "version", version).map(Some),
            2 => {
                let (version_type, version) = (split[0], split[1]);

                match version_type {
                    "ref" => install_path(plugin_name, "ref", &version).map(Some),
                    // This is for people who have the local source already compiled
                    // Like those who work on the language, etc
                    // We'll allow specifying path:/foo/bar/project in .tool-versions
                    // And then use the binaries there
                    "path" => Ok(Some(PathBuf::from(version))),
                    _ => install_path(plugin_name, "version", version).map(Some),
                }
            }
            _ => Err(anyhow!("Unknown version specifier: {}", version)),
        }
    }
}

// get_custom_executable_path

pub fn asdf_config_file() -> Result<PathBuf> {
    env::var_os("ASDF_CONFIG_FILE")
        .map(Into::into)
        .or_else(|| dirs::home_dir().map(|home| home.join(".asdfrc")))
        .ok_or_else(|| anyhow!("Cannot find asdf config file"))
}

pub fn shims_path() -> Result<PathBuf> {
    Ok(asdf_data_dir()?.join("shims"))
}

pub fn plugins_path() -> Result<PathBuf> {
    Ok(asdf_data_dir()?.join("plugins"))
}

pub fn installs_path() -> Result<PathBuf> {
    Ok(asdf_data_dir()?.join("installs"))
}

pub fn plugin_installs_path(plugin_name: &str) -> Result<PathBuf> {
    Ok(installs_path()?.join(plugin_name))
}

pub fn downloads_path() -> Result<PathBuf> {
    Ok(asdf_data_dir()?.join("downloads"))
}

pub fn plugin_downloads_path(plugin_name: &str) -> Result<PathBuf> {
    Ok(downloads_path()?.join(plugin_name))
}

pub fn find_file_upwards(name: &str) -> Result<Option<PathBuf>> {
    let cwd = env::current_dir()?;
    let mut search_path = Some(cwd.as_path());

    while let Some(path) = search_path {
        if path.join(name).is_file() {
            return Ok(Some(path.join(name)));
        }

        search_path = path.parent();
    }

    Ok(None)
}

pub fn list_installed_plugins() -> Result<Vec<String>> {
    let plugins_path = plugins_path()?;

    if plugins_path.is_dir() {
        let mut plugins = fs::read_dir(plugins_path)?
            .map(|result| {
                result.map_err(Into::into).and_then(|entry| {
                    entry
                        .file_name()
                        .into_string()
                        .map_err(|_| anyhow!("Cannot parse filename as unicode"))
                })
            })
            .collect::<Result<Vec<_>>>()?;
        plugins.sort();

        Ok(plugins)
    } else {
        Ok(vec![])
    }
}

pub fn asdf_config_value_from_file(config_path: &Path, key: &str) -> Result<String> {
    if !config_path.is_file() {
        Err(anyhow!("File not found: {:?}", config_path))
    } else {
        fs::read_to_string(config_path)?
            .lines()
            .filter_map(|line| {
                let split = line.splitn(2, '=').collect::<Vec<_>>();
                if 2 == split.len() {
                    Some((split[0].trim(), split[1].trim()))
                } else {
                    None
                }
            })
            .find(|(k, _)| k == &key)
            .map(|(_, value)| value.to_string())
            .ok_or_else(|| anyhow!("Key {} not found in config file {:?}", key, config_path))
    }
}

pub fn asdf_config_value(key: &str) -> Result<Option<String>> {
    let local_config_path = find_file_upwards(".asdfrc")?;
    if let Some(local_config_path) = local_config_path {
        if let Ok(value) = asdf_config_value_from_file(&local_config_path, key) {
            return Ok(Some(value));
        }
    }

    let config_path = asdf_config_file()?;
    if let Ok(value) = asdf_config_value_from_file(&config_path, key) {
        return Ok(Some(value));
    }

    // Defaults
    if key == "legacy_version_file" {
        return Ok(Some(String::from("no")));
    }

    Ok(None)
}

pub fn remove_tool_version_comments(line: &str) -> Option<&str> {
    // Remove whitespace before pound sign, the pound sign, and everything after it
    let uncommented = if let Some(pound_index) = line.find("#") {
        line[..pound_index].trim_end()
    } else {
        line.trim_end()
    };

    if uncommented.is_empty() {
        None
    } else {
        Some(line)
    }
}

pub fn parse_tool_version_line(line: &str) -> Option<(&str, &str)> {
    // Remove whitespace before pound sign, the pound sign, and everything after it
    let uncommented = if let Some(pound_index) = line.find("#") {
        line[..pound_index].trim_end()
    } else {
        line
    };

    let split = uncommented.splitn(2, " ").collect::<Vec<_>>();
    match split.len() {
        2 => Some((split[0], split[1])),
        _ => None,
    }
}

pub fn parse_tool_versions_file(file_path: &Path) -> Result<ToolVersions> {
    fs::read_to_string(file_path)?.parse()
}

pub fn parse_asdf_version_file(file_path: &Path, plugin_name: &str) -> Result<Option<String>> {
    if file_path.is_file() {
        Ok(fs::read_to_string(file_path)?
            .lines()
            .filter_map(parse_tool_version_line)
            .find(|(name, _)| name == &plugin_name)
            .map(|(_, version)| version.to_owned()))
    } else {
        Ok(None)
    }
}

pub fn parse_legacy_version_file(file_path: &Path, plugin_name: &str) -> Result<Option<String>> {
    let plugin_path = plugin_path(plugin_name)?;
    let parse_legacy_script = plugin_path.join("bin").join("parse-legacy-file");

    if file_path.is_file() {
        if parse_legacy_script.is_file() {
            call(process::Command::new(parse_legacy_script).arg(file_path)).map(Some)
        } else {
            fs::read_to_string(file_path).map(Some).map_err(Into::into)
        }
    } else {
        Ok(None)
    }
}

pub fn call(command: &mut process::Command) -> Result<String> {
    let output = command.output()?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout)?;
        Ok(String::from(stdout.trim_end()))
    } else {
        Err(anyhow!(
            "{}\n{}\n",
            str::from_utf8(&output.stdout)?,
            str::from_utf8(&output.stderr)?
        ))
    }
}

pub fn version_not_installed_text(plugin_name: &str, version: &str) -> String {
    format!("version {} is not installed for {}", version, plugin_name)
}

fn env_var_for_plugin(plugin_name: &str) -> String {
    format!("ASDF_{}_VERSION", plugin_name.to_uppercase())
}

pub fn preset_version_for(plugin_name: &str) -> Result<Option<String>> {
    Ok(find_versions(plugin_name, &env::current_dir()?)?.map(|spec| spec.version))
}

pub fn preset_versions(shim_name: &str) -> Result<Vec<String>> {
    shim_plugins(shim_name)?
        .into_iter()
        .filter_map(|plugin| preset_version_for(&plugin).transpose())
        .collect()
}

pub fn select_from_preset_version(shim_name: &str) -> Result<Option<String>> {
    let shim_versions = shim_versions(shim_name)?;
    if !shim_versions.is_empty() {
        let preset_versions = preset_versions(shim_name)?;
        Ok(preset_versions
            .into_iter()
            .find(|preset_version| preset_version.contains(&shim_versions.join(" "))))
    } else {
        Ok(None)
    }
}

pub fn executable_path(
    plugin_name: &str,
    version: &str,
    executable_path: &Path,
) -> Result<PathBuf> {
    version_exists(plugin_name, version)?;

    if version == "system" {
        let shims_path = shims_path()?;
        let filtered_path = env::var_os("PATH")
            .map(|paths| {
                env::join_paths(env::split_paths(&paths).filter(|path| path != &shims_path))
            })
            .transpose()?;
        let cmd = executable_path
            .file_name()
            .unwrap_or(executable_path.as_os_str());
        which_in(cmd, filtered_path, env::current_dir()?).map_err(Into::into)
    } else {
        if let Some(install_path) = find_install_path(plugin_name, version)? {
            Ok(install_path.join(executable_path))
        } else {
            Err(anyhow!("Plugin version not found"))
        }
    }
}

pub fn find_tool_versions() -> Result<Option<PathBuf>> {
    find_file_upwards(".tool-versions")
}

// This also normalizes intermediate components
pub fn resolve_symlink(symlink: &Path) -> Result<PathBuf> {
    symlink.canonicalize().map_err(Into::into)
}

pub fn asdf_run_hook<E, Ek, Ev>(hook_name: &str, args: &[&str], envs: E) -> Result<()>
where
    E: IntoIterator<Item = (Ek, Ev)>,
    Ek: AsRef<OsStr>,
    Ev: AsRef<OsStr>,
{
    if let Some(hook_cmd) = asdf_config_value(hook_name)? {
        let output = call(
            process::Command::new("bash")
                .args([&["-c", &hook_cmd, "bash"], args].concat())
                .envs(envs),
        )?;

        println!("{}", output);
    }

    Ok(())
}

pub fn list_plugin_bin_paths(
    plugin_name: &str,
    version: &str,
    install_type: &str,
) -> Result<Vec<String>> {
    let plugin_path = plugin_path(plugin_name)?;
    let install_path = install_path(plugin_name, install_type, version)?;

    let list_bin_paths_path = plugin_path.join("bin").join("list-bin-paths");
    if list_bin_paths_path.is_file() {
        call(process::Command::new(list_bin_paths_path).envs(vec![
            ("ASDF_INSTALL_TYPE", OsStr::new(install_type)),
            ("ASDF_INSTALL_VERSION", OsStr::new(version)),
            ("ASDF_INSTALL_PATH", install_path.as_os_str()),
        ]))
        .map(|output| output.split(' ').map(|part| part.to_string()).collect())
    } else {
        Ok(vec![String::from("bin")])
    }
}

pub fn plugin_shims(plugin_name: &str, full_version: &str) -> Result<Vec<PathBuf>> {
    Ok(fs::read_dir(asdf_data_dir()?.join("shims"))?
        .into_iter()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            {
                fs::read_to_string(path).map(|contents| {
                    contents.contains(&format!("# asdf-plugin: {} {}", plugin_name, full_version))
                })
            }
            .is_ok()
        })
        .collect())
}

pub fn list_plugin_exec_paths(plugin_name: &str, full_version: &str) -> Result<Vec<PathBuf>> {
    plugin_exists(plugin_name)?;

    let tool_version: ToolVersion = full_version.parse()?;
    let install_type = tool_version.install_type();
    let version = tool_version.install_version(plugin_name)?.unwrap();

    let plugin_shims_path = plugin_path(plugin_name)?.join("shims");

    let mut plugin_exec_paths = vec![];
    if plugin_shims_path.is_dir() {
        plugin_exec_paths.push(plugin_shims_path);
    }

    let bin_paths = list_plugin_bin_paths(plugin_name, &version, &install_type)?;

    let install_path = install_path(plugin_name, &install_type, &version)?;

    for bin_path in bin_paths {
        plugin_exec_paths.push(install_path.join(bin_path));
    }

    Ok(plugin_exec_paths)
}

pub fn plugin_executables(plugin_name: &str, full_version: &str) -> Result<Vec<PathBuf>> {
    let exec_paths = list_plugin_exec_paths(plugin_name, full_version)?;
    let all_bin_paths = list_plugin_exec_paths(plugin_name, full_version)?;

    let mut plugin_executables = vec![];
    for bin_path in all_bin_paths {
        for entry in fs::read_dir(bin_path)? {
            let entry = entry?;

            if entry.path().is_executable() {
                plugin_executables.push(entry.path());
            }
        }
    }

    Ok(plugin_executables)
}

pub fn shim_plugin_versions(executable: &str) -> Result<Vec<String>> {
    let executable_path = PathBuf::from(executable);
    let executable_name = executable_path
        .file_name()
        .unwrap_or(&OsStr::new(executable));
    let shim_path = shims_path()?.join(executable_name);

    if shim_path.is_executable() {
        Ok(fs::read_to_string(shim_path)?
            .lines()
            .filter(|line| line.starts_with("# asdf-plugin: "))
            .map(|line| line[15..].to_string())
            .collect())
    } else {
        Err(anyhow!("asdf: unknown shim {:?}", executable_name))
    }
}

pub fn shim_plugins(executable: &str) -> Result<Vec<String>> {
    Ok(shim_plugin_versions(executable)?
        .into_iter()
        .map(|version| version.split(' ').next().unwrap().to_string())
        .collect())
}

pub fn shim_versions(shim_name: &str) -> Result<Vec<String>> {
    let mut versions = shim_plugin_versions(shim_name)?;
    let mut system_versions = versions
        .clone()
        .into_iter()
        .map(|version| format!("{} system", version.split(' ').next().unwrap()))
        .collect();
    versions.append(&mut system_versions);

    Ok(versions)
}

pub fn select_version(shim_name: &str) -> Result<Option<String>> {
    // First, we get the all the plugins where the
    // current shim is available.
    // Then, we iterate on all versions set for each plugin
    // Note that multiple plugin versions can be set for a single plugin.
    // These are separated by a space. e.g. python 3.7.2 2.7.15
    // For each plugin/version pair, we check if it is present in the shim
    let search_path = env::current_dir()?;
    let shim_versions = shim_versions(shim_name)?;
    let plugins = shim_plugins(shim_name)?;

    for plugin_name in plugins {
        if let Some(version_spec) = find_versions(&plugin_name, &search_path)? {
            let usable_plugin_versions = version_spec.version.split(' ').collect::<Vec<_>>();
            for plugin_version in usable_plugin_versions {
                for plugin_and_version in &shim_versions {
                    let splitted = plugin_and_version.split(' ').collect::<Vec<_>>();
                    let (plugin_shim_name, plugin_shim_version) = (splitted[0], splitted[1]);

                    if plugin_name == plugin_shim_name {
                        if plugin_version == plugin_shim_version
                            || plugin_version.starts_with("path:")
                        {
                            return Ok(Some(format!("{} {}", plugin_name, plugin_version)));
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

pub fn with_shim_executable(shim: &Path, shim_exec: &str) -> Result<()> {
    let shim_name = shim.file_name().unwrap_or(shim.as_os_str());
    let shim_path = shims_path()?.join(&shim_name);

    if !shim_path.is_file() {
        return Err(anyhow!(
            "unknown command: {:?}. Perhaps you have to reshim?",
            shim_name
        ));
    }

    let selected_version = select_version(&shim_name.to_str().unwrap())?
        .or_else(|| select_from_preset_version(shim_name.to_str().unwrap()).unwrap());

    Ok(())

    // local shim_name
    // shim_name=$(basename "$1")
    // local shim_exec="${2}"

    // if [ ! -f "$(asdf_data_dir)/shims/${shim_name}" ]; then
    //   printf "%s %s %s\\n" "unknown command:" "${shim_name}." "" >&2
    //   return 1
    // fi

    // local selected_version
    // selected_version="$(select_version "$shim_name")"

    // if [ -z "$selected_version" ]; then
    //   selected_version="$(select_from_preset_version "$shim_name")"
    // fi

    // if [ -n "$selected_version" ]; then
    //   local plugin_name
    //   local full_version
    //   local plugin_path

    //   IFS=' ' read -r plugin_name full_version <<<"$selected_version"
    //   plugin_path=$(get_plugin_path "$plugin_name")

    //   run_within_env() {
    //     local path
    //     path=$(sed -e "s|$(asdf_data_dir)/shims||g; s|::|:|g" <<<"$PATH")

    //     executable_path=$(PATH=$path command -v "$shim_name")

    //     if [ -x "${plugin_path}/bin/exec-path" ]; then
    //       install_path=$(find_install_path "$plugin_name" "$full_version")
    //       executable_path=$(get_custom_executable_path "${plugin_path}" "${install_path}" "${executable_path:-${shim_name}}")
    //     fi

    //     "$shim_exec" "$plugin_name" "$full_version" "$executable_path"
    //   }

    //   with_plugin_env "$plugin_name" "$full_version" run_within_env
    //   return $?
    // fi

    // (
    //   local preset_plugin_versions
    //   preset_plugin_versions=()
    //   local closest_tool_version
    //   closest_tool_version=$(find_tool_versions)

    //   local shim_plugins
    //   IFS=$'\n' read -rd '' -a shim_plugins <<<"$(shim_plugins "$shim_name")"
    //   for shim_plugin in "${shim_plugins[@]}"; do
    //     local shim_versions
    //     local version_string
    //     version_string=$(get_preset_version_for "$shim_plugin")
    //     IFS=' ' read -r -a shim_versions <<<"$version_string"
    //     local usable_plugin_versions
    //     for shim_version in "${shim_versions[@]}"; do
    //       preset_plugin_versions+=("$shim_plugin $shim_version")
    //     done
    //   done

    //   if [ -n "${preset_plugin_versions[*]}" ]; then
    //     printf "%s %s\\n" "No preset version installed for command" "$shim_name"
    //     printf "%s\\n\\n" "Please install a version by running one of the following:"
    //     for preset_plugin_version in "${preset_plugin_versions[@]}"; do
    //       printf "%s %s\\n" "asdf install" "$preset_plugin_version"
    //     done
    //     printf "\\n%s %s\\n" "or add one of the following versions in your config file at" "$closest_tool_version"
    //   else
    //     printf "%s %s\\n" "No version set for command" "$shim_name"
    //     printf "%s %s\\n" "Consider adding one of the following versions in your config file at" "$closest_tool_version"
    //   fi
    //   shim_plugin_versions "${shim_name}"
    // ) >&2

    // return 126
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        fs::{self},
        os::unix::prelude::PermissionsExt,
        path::{Path, PathBuf},
    };

    use anyhow::Result;
    use copy_dir::copy_dir;
    use serial_test::serial;
    use tempfile::TempDir;
    use tmp_env::{set_current_dir, set_var, CurrentDir, CurrentEnv};
    use which::{which, which_in};

    struct TestContext {
        home_dir: TempDir,
        project_dir: TempDir,
        _home_env: CurrentEnv,
        _path_env: CurrentEnv,
        _pwd: CurrentDir,
    }

    fn setup() -> Result<TestContext> {
        let home_dir = TempDir::new()?;

        for dir in &["plugins", "installs", "shims", "tmp"] {
            fs::create_dir_all(home_dir.path().join(".asdf").join(dir))?;
        }

        install_mock_plugin("dummy", &home_dir.path().join(".asdf"))?;
        for version in &["0.1.0", "0.2.0"] {
            install_mock_plugin_version("dummy", version, &home_dir.path().join(".asdf"))?;
        }

        let project_dir = TempDir::new_in(&home_dir)?;
        fs::create_dir_all(&project_dir.path())?;

        let path = std::env::var_os("PATH").unwrap_or_default();
        let paths = std::env::split_paths(&path);
        let mut new_paths = vec![home_dir.path().join(".asdf").join("shims")];
        new_paths.extend(paths);

        Ok(TestContext {
            _home_env: set_var("HOME", &home_dir.path()),
            _path_env: set_var("PATH", env::join_paths(new_paths)?),
            _pwd: set_current_dir(home_dir.path())?,
            home_dir,
            project_dir,
        })
    }

    fn install_mock_plugin(plugin_name: &str, location: &Path) -> Result<()> {
        copy_dir(
            PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
                .join("test/fixtures/dummy_plugin"),
            location.join("plugins").join(plugin_name),
        )?;

        Ok(())
    }

    fn install_mock_plugin_version(
        plugin_name: &str,
        plugin_version: &str,
        location: &Path,
    ) -> Result<()> {
        fs::create_dir_all(
            location
                .join("installs")
                .join(plugin_name)
                .join(plugin_version),
        )
        .map_err(Into::into)
    }

    // get_install_path should output version path when version is provided
    #[test]
    #[serial]
    fn install_path_with_version() -> Result<()> {
        let context = setup()?;

        let install_path = super::install_path("foo", "version", "1.0.0")?;

        assert_eq!(
            install_path,
            context
                .home_dir
                .path()
                .join(".asdf")
                .join("installs")
                .join("foo")
                .join("1.0.0")
        );
        assert!(install_path.parent().unwrap().is_dir());

        Ok(())
    }

    // get_install_path should output custom path when custom install type is provided
    #[test]
    #[serial]
    fn install_path_with_custom() -> Result<()> {
        let context = setup()?;

        let install_path = super::install_path("foo", "custom", "1.0.0")?;

        assert_eq!(
            install_path,
            context
                .home_dir
                .path()
                .join(".asdf")
                .join("installs")
                .join("foo")
                .join("custom-1.0.0")
        );
        assert!(install_path.parent().unwrap().is_dir());

        Ok(())
    }

    // get_install_path should output path when path version is provided
    #[test]
    #[serial]
    fn install_path_with_path() -> Result<()> {
        let context = setup()?;

        let install_path = super::install_path("foo", "path", "/some/path")?;

        assert_eq!(install_path, PathBuf::from("/some/path"));
        assert!(context
            .home_dir
            .path()
            .join(".asdf")
            .join("installs")
            .join("foo")
            .is_dir());

        Ok(())
    }

    // get_download_path should output version path when version is provided
    #[test]
    #[serial]
    fn download_path_with_version() -> Result<()> {
        let context = setup()?;

        let download_path = super::download_path("foo", "version", "1.0.0")?;

        assert_eq!(
            download_path,
            Some(
                context
                    .home_dir
                    .path()
                    .join(".asdf")
                    .join("downloads")
                    .join("foo")
                    .join("1.0.0")
            )
        );
        assert!(context
            .home_dir
            .path()
            .join(".asdf")
            .join("downloads")
            .join("foo")
            .is_dir());

        Ok(())
    }

    // get_download_path should output custom path when custom download type is provided
    #[test]
    #[serial]
    fn download_path_with_custom() -> Result<()> {
        let context = setup()?;

        let download_path = super::download_path("foo", "custom", "1.0.0")?;

        assert_eq!(
            download_path,
            Some(
                context
                    .home_dir
                    .path()
                    .join(".asdf")
                    .join("downloads")
                    .join("foo")
                    .join("custom-1.0.0")
            )
        );
        assert!(context
            .home_dir
            .path()
            .join(".asdf")
            .join("downloads")
            .join("foo")
            .is_dir());

        Ok(())
    }

    // get_download_path should output nothing when path version is provided
    #[test]
    #[serial]
    fn download_path_with_path() -> Result<()> {
        let context = setup()?;

        let download_path = super::download_path("foo", "path", "/some/path")?;

        assert_eq!(download_path, None);
        assert!(context
            .home_dir
            .path()
            .join(".asdf")
            .join("downloads")
            .join("foo")
            .is_dir());

        Ok(())
    }

    // check_if_version_exists should exit with 1 if plugin does not exist
    #[test]
    #[serial]
    fn version_exists_without_plugin() -> Result<()> {
        let _context = setup()?;

        let version_exists = super::version_exists("inexistent", "1.0.0").unwrap_err();

        assert_eq!(version_exists.to_string(), "No such plugin: inexistent");

        Ok(())
    }

    // check_if_version_exists should exit with 1 if version does not exist
    #[test]
    #[serial]
    fn version_exists_without_version() -> Result<()> {
        let _context = setup()?;

        let version_exists = super::version_exists("dummy", "1.0.0").unwrap_err();

        assert_eq!(version_exists.to_string(), "");

        Ok(())
    }

    // version_not_installed_text is correct
    #[test]
    fn version_not_installed_text() {
        let version_not_installed_text = super::version_not_installed_text("dummy", "1.0.0");
        assert_eq!(
            version_not_installed_text,
            "version 1.0.0 is not installed for dummy"
        );
    }

    // check_if_version_exists should be noop if version exists
    #[test]
    #[serial]
    fn version_exists_with_version() -> Result<()> {
        let _context = setup()?;

        let version_exists = super::version_exists("dummy", "0.1.0").unwrap();

        assert_eq!(version_exists, ());

        Ok(())
    }

    // check_if_version_exists should be noop if version is system
    #[test]
    #[serial]
    fn version_exists_with_system() -> Result<()> {
        let context = setup()?;

        fs::create_dir_all(
            context
                .home_dir
                .path()
                .join(".asdf")
                .join("plugins")
                .join("foo"),
        )?;

        let version_exists = super::version_exists("foo", "system").unwrap();

        assert_eq!(version_exists, ());

        Ok(())
    }

    // check_if_version_exists should be ok for ref:version install
    #[test]
    #[serial]
    fn version_exists_with_ref() -> Result<()> {
        let context = setup()?;

        fs::create_dir_all(
            context
                .home_dir
                .path()
                .join(".asdf")
                .join("plugins")
                .join("foo"),
        )?;
        fs::create_dir_all(
            context
                .home_dir
                .path()
                .join(".asdf")
                .join("installs")
                .join("foo")
                .join("ref-master"),
        )?;

        let version_exists = super::version_exists("foo", "ref:master").unwrap();

        assert_eq!(version_exists, ());

        Ok(())
    }

    // check_if_plugin_exists should exit with 1 when plugin is empty string
    #[test]
    #[serial]
    fn plugin_exists_with_empty_string() -> Result<()> {
        let _context = setup()?;

        let plugin_exists = super::plugin_exists("").unwrap_err();

        assert_eq!(plugin_exists.to_string(), "No plugin given");

        Ok(())
    }

    // check_if_plugin_exists should be noop if plugin exists
    #[test]
    #[serial]
    fn plugin_exists_with_plugin() -> Result<()> {
        let _context = setup()?;

        let plugin_exists = super::plugin_exists("dummy")?;

        assert_eq!(plugin_exists, ());

        Ok(())
    }

    // parse_asdf_version_file should output version
    #[test]
    #[serial]
    fn parse_asdf_version_file_with_version() -> Result<()> {
        let context = setup()?;
        let tool_versions_path = context.project_dir.path().join(".tool-versions");
        fs::write(&tool_versions_path, "dummy 0.1.0")?;

        let version = super::parse_asdf_version_file(&tool_versions_path, "dummy")?;

        assert_eq!(version, Some(String::from("0.1.0")));

        Ok(())
    }

    // parse_asdf_version_file should output path on project with spaces
    #[test]
    #[serial]
    fn parse_asdf_version_file_with_spaces_in_project() -> Result<()> {
        let context = setup()?;
        let project_dir = context.project_dir.path().join("outer space");
        fs::create_dir_all(&project_dir)?;

        let tool_versions_path = project_dir.join(".tool-versions");
        fs::write(&tool_versions_path, "dummy 0.1.0")?;

        let version = super::parse_asdf_version_file(&tool_versions_path, "dummy")?;

        assert_eq!(version, Some(String::from("0.1.0")));

        Ok(())
    }

    // parse_asdf_version_file should output path version with spaces
    #[test]
    #[serial]
    fn parse_asdf_version_file_with_spaces_in_version() -> Result<()> {
        let context = setup()?;
        let tool_versions_path = context.project_dir.path().join(".tool-versions");
        fs::write(&tool_versions_path, "dummy path:/some/dummy path")?;

        let version = super::parse_asdf_version_file(&tool_versions_path, "dummy")?;

        assert_eq!(version, Some(String::from("path:/some/dummy path")));

        Ok(())
    }

    // find_versions should return .tool-versions if legacy is disabled
    #[test]
    #[serial]
    fn find_versions_without_legacy() -> Result<()> {
        let context = setup()?;
        fs::write(
            context.project_dir.path().join(".tool-versions"),
            "dummy 0.1.0",
        )?;
        fs::write(context.project_dir.path().join(".dummy-version"), "0.2.0")?;

        let find_versions = super::find_versions("dummy", context.project_dir.path())?;

        assert_eq!(
            find_versions,
            Some(super::VersionSpecifier {
                version: String::from("0.1.0"),
                source: super::VersionSource::ToolVersion(
                    context.project_dir.path().join(".tool-versions")
                )
            })
        );

        Ok(())
    }

    // find_versions should return the legacy file if supported
    #[test]
    #[serial]
    fn find_versions_with_legacy() -> Result<()> {
        let context = setup()?;
        fs::write(
            context.home_dir.path().join(".asdfrc"),
            "legacy_version_file = yes",
        )?;
        fs::write(
            context.home_dir.path().join(".tool-versions"),
            "dummy 0.1.0",
        )?;

        fs::write(context.project_dir.path().join(".dummy-version"), "0.2.0")?;

        let find_versions = super::find_versions("dummy", context.project_dir.path())?;

        assert_eq!(
            find_versions,
            Some(super::VersionSpecifier {
                version: String::from("0.2.0"),
                source: super::VersionSource::Legacy(
                    context.project_dir.path().join(".dummy-version")
                )
            })
        );

        Ok(())
    }

    // find_versions skips .tool-version file that don't list the plugin
    #[test]
    #[serial]
    fn find_versions_with_tool_version_without_plugin() -> Result<()> {
        let context = setup()?;
        let tool_versions = context.home_dir.path().join(".tool-versions");
        fs::write(tool_versions, "dummy 0.1.0")?;
        fs::write(
            context.project_dir.path().join(".tool-versions"),
            "another_plugin 0.3.0",
        )?;

        let find_versions = super::find_versions("dummy", context.project_dir.path())?;

        assert_eq!(
            find_versions,
            Some(super::VersionSpecifier {
                version: String::from("0.1.0"),
                source: super::VersionSource::ToolVersion(
                    context.home_dir.path().join(".tool-versions")
                )
            })
        );

        Ok(())
    }

    // find_versions should return .tool-versions if unsupported
    #[test]
    #[serial]
    fn find_versions_without_legacy_filenames() -> Result<()> {
        let context = setup()?;
        let tool_versions = context.home_dir.path().join(".tool-versions");
        fs::write(tool_versions, "dummy 0.1.0")?;
        fs::write(context.project_dir.path().join(".dummy-version"), "0.2.0")?;
        fs::write(
            context.home_dir.path().join(".asdfrc"),
            "legacy_version_file = yes",
        )?;
        fs::remove_file(
            context
                .home_dir
                .path()
                .join(".asdf")
                .join("plugins")
                .join("dummy")
                .join("bin")
                .join("list-legacy-filenames"),
        )?;

        let find_versions = super::find_versions("dummy", context.project_dir.path())?;

        assert_eq!(
            find_versions,
            Some(super::VersionSpecifier {
                version: String::from("0.1.0"),
                source: super::VersionSource::ToolVersion(
                    context.home_dir.path().join(".tool-versions")
                )
            })
        );

        Ok(())
    }

    // find_versions should return the version set by envrionment variable
    #[test]
    #[serial]
    fn find_versions_with_env() -> Result<()> {
        let context = setup()?;
        let _env = set_var("ASDF_DUMMY_VERSION", "0.2.0");

        let find_versions = super::find_versions("dummy", context.project_dir.path())?;

        assert_eq!(
            find_versions,
            Some(super::VersionSpecifier {
                version: String::from("0.2.0"),
                source: super::VersionSource::EnvVar(String::from("ASDF_DUMMY_VERSION"))
            })
        );

        Ok(())
    }

    // asdf_data_dir should return user dir if configured
    #[test]
    #[serial]
    fn asdf_data_dir_with_env() -> Result<()> {
        let _context = setup()?;
        let _env = set_var("ASDF_DATA_DIR", "/tmp/wadus");

        let asdf_data_dir = super::asdf_data_dir()?;

        assert_eq!(asdf_data_dir, PathBuf::from("/tmp/wadus"));

        Ok(())
    }

    // asdf_data_dir should return ~/.asdf when ASDF_DATA_DIR is not set
    #[test]
    #[serial]
    fn asdf_data_dir_with_home() -> Result<()> {
        assert_eq!(
            super::asdf_data_dir()?,
            dirs::home_dir().unwrap().join(".asdf")
        );

        Ok(())
    }

    // check_if_plugin_exists should work with a custom data directory
    #[test]
    #[serial]
    fn plugin_exists_with_custom_data_dir() -> Result<()> {
        let context = setup()?;
        let custom_data_dir = context.home_dir.path().join("asdf-data");
        let _env = set_var("ASDF_DATA_DIR", &custom_data_dir);
        fs::create_dir_all(&custom_data_dir.join("plugins"))?;
        fs::create_dir_all(&custom_data_dir.join("installs"))?;
        install_mock_plugin("dummy2", &custom_data_dir)?;
        install_mock_plugin_version("dummy2", "0.1.0", &custom_data_dir)?;

        let plugin_exists = super::plugin_exists("dummy2")?;

        assert_eq!(plugin_exists, ());

        Ok(())
    }

    // find_versions should return \$ASDF_DEFAULT_TOOL_VERSIONS_FILENAME if set
    #[test]
    #[serial]
    fn find_versions_with_default_env() -> Result<()> {
        let context = setup()?;
        let default_tool_versions_filename =
            context.project_dir.path().join("global-tool-versions");
        let _env = set_var(
            "ASDF_DEFAULT_TOOL_VERSIONS_FILENAME",
            &default_tool_versions_filename,
        );
        fs::write(&default_tool_versions_filename, "dummy 0.1.0")?;

        let find_versions = super::find_versions("dummy", context.project_dir.path())?;

        assert_eq!(
            find_versions,
            Some(super::VersionSpecifier {
                version: String::from("0.1.0"),
                source: super::VersionSource::ToolVersion(default_tool_versions_filename)
            })
        );

        Ok(())
    }

    // find_versions should check \$HOME legacy files before \$ASDF_DEFAULT_TOOL_VERSIONS_FILENAME
    #[test]
    #[serial]
    fn find_versions_with_legacy_file_in_home() -> Result<()> {
        let context = setup()?;
        let default_tool_versions_filename =
            context.project_dir.path().join("global-tool-versions");
        let _env = set_var(
            "ASDF_DEFAULT_TOOL_VERSIONS_FILENAME",
            &default_tool_versions_filename,
        );
        fs::write(&default_tool_versions_filename, "dummy 0.2.0")?;
        let dummy_version_path = context.home_dir.path().join(".dummy-version");
        fs::write(&dummy_version_path, "dummy 0.1.0")?;
        fs::write(
            context.home_dir.path().join(".asdfrc"),
            "legacy_version_file = yes",
        )?;

        let find_versions = super::find_versions("dummy", context.project_dir.path())?;

        assert_eq!(
            find_versions,
            Some(super::VersionSpecifier {
                version: String::from(" 0.1.0"),
                source: super::VersionSource::Legacy(dummy_version_path)
            })
        );

        Ok(())
    }

    // get_preset_version_for returns the current version
    #[test]
    #[serial]
    fn preset_version_for_with_version() -> Result<()> {
        let context = setup()?;
        let _cwd = set_current_dir(&context.project_dir)?;
        fs::write(
            context.project_dir.path().join(".tool-versions"),
            "dummy 0.2.0",
        )?;

        let preset_version = super::preset_version_for("dummy")?;

        assert_eq!(preset_version, Some(String::from("0.2.0")));

        Ok(())
    }

    // get_preset_version_for returns the global version from home when project is outside of home
    #[test]
    #[serial]
    fn preset_version_for_with_global_version() -> Result<()> {
        let context = setup()?;
        fs::write(
            context.home_dir.path().join(".tool-versions"),
            "dummy 0.1.0",
        )?;

        let project_dir = TempDir::new()?;
        let _cwd = set_current_dir(&project_dir.path())?;

        let preset_version = super::preset_version_for("dummy")?;

        assert_eq!(preset_version, Some(String::from("0.1.0")));

        Ok(())
    }

    // get_preset_version_for returns the tool version from env if ASDF_{TOOL}_VERSION is defined
    #[test]
    #[serial]
    fn preset_version_for_with_env() -> Result<()> {
        let context = setup()?;
        let _cwd = set_current_dir(&context.project_dir.path())?;
        fs::write(
            context.project_dir.path().join(".tool-versions"),
            "dummy 0.2.0",
        )?;
        let _env = set_var("ASDF_DUMMY_VERSION", "3.0.0");

        let preset_version = super::preset_version_for("dummy")?;

        assert_eq!(preset_version, Some(String::from("3.0.0")));

        Ok(())
    }

    // get_preset_version_for should return branch reference version
    #[test]
    #[serial]
    fn preset_version_for_with_ref() -> Result<()> {
        let context = setup()?;
        let _cwd = set_current_dir(&context.project_dir.path())?;
        fs::write(
            context.project_dir.path().join(".tool-versions"),
            "dummy ref:master",
        )?;

        let preset_version = super::preset_version_for("dummy")?;

        assert_eq!(preset_version, Some(String::from("ref:master")));

        Ok(())
    }

    // get_preset_version_for should return path version
    #[test]
    #[serial]
    fn preset_version_for_with_path() -> Result<()> {
        let context = setup()?;
        let _cwd = set_current_dir(&context.project_dir.path())?;
        fs::write(
            context.project_dir.path().join(".tool-versions"),
            "dummy path:/some/place with spaces",
        )?;

        let preset_version = super::preset_version_for("dummy")?;

        assert_eq!(
            preset_version,
            Some(String::from("path:/some/place with spaces"))
        );

        Ok(())
    }

    // get_executable_path for system version should return system path
    #[test]
    #[serial]
    fn executable_path_with_system() -> Result<()> {
        let context = setup()?;
        fs::create_dir_all(
            context
                .home_dir
                .path()
                .join(".asdf")
                .join("plugins")
                .join("foo"),
        )?;

        let executable_path = super::executable_path("foo", "system", &PathBuf::from("ls"))?;

        assert_eq!(executable_path, which("ls")?);

        Ok(())
    }

    // get_executable_path for system version should not use asdf shims
    #[test]
    #[serial]
    fn executable_path_with_system_no_shim() -> Result<()> {
        let context = setup()?;
        fs::create_dir_all(
            context
                .home_dir
                .path()
                .join(".asdf")
                .join("plugins")
                .join("foo"),
        )?;
        let dummy_exe_path = context
            .home_dir
            .path()
            .join(".asdf")
            .join("shims")
            .join("dummy_executable");
        fs::write(&dummy_exe_path, "")?;
        fs::set_permissions(&dummy_exe_path, PermissionsExt::from_mode(0o755))?;

        assert_eq!(
            which_in("dummy_executable", env::var_os("PATH"), env::current_dir()?)?,
            dummy_exe_path
        );

        let executable_path =
            super::executable_path("foo", "system", &PathBuf::from("dummy_executable"));

        assert_eq!(
            executable_path.unwrap_err().to_string(),
            "cannot find binary path"
        );

        Ok(())
    }

    // get_executable_path for non system version should return relative path from plugin
    #[test]
    #[serial]
    fn executable_path_without_system() -> Result<()> {
        let context = setup()?;
        fs::create_dir_all(
            context
                .home_dir
                .path()
                .join(".asdf")
                .join("plugins")
                .join("foo"),
        )?;
        let bin_path = context
            .home_dir
            .path()
            .join(".asdf")
            .join("installs")
            .join("foo")
            .join("1.0.0")
            .join("bin");
        fs::create_dir_all(&bin_path)?;
        fs::write(&bin_path.join("dummy"), "")?;
        fs::set_permissions(&bin_path.join("dummy"), PermissionsExt::from_mode(0o755))?;

        let executable_path = super::executable_path("foo", "1.0.0", &PathBuf::from("bin/dummy"))?;

        assert_eq!(executable_path, bin_path.join("dummy"));

        Ok(())
    }

    // get_executable_path for ref:version installed version should resolve to ref-version
    #[test]
    #[serial]
    fn executable_path_with_ref() -> Result<()> {
        let context = setup()?;
        fs::create_dir_all(
            context
                .home_dir
                .path()
                .join(".asdf")
                .join("plugins")
                .join("foo"),
        )?;
        let bin_path = context
            .home_dir
            .path()
            .join(".asdf")
            .join("installs")
            .join("foo")
            .join("ref-master")
            .join("bin");
        fs::create_dir_all(&bin_path)?;
        fs::write(&bin_path.join("dummy"), "")?;
        fs::set_permissions(&bin_path.join("dummy"), PermissionsExt::from_mode(0o755))?;

        let executable_path =
            super::executable_path("foo", "ref:master", &PathBuf::from("bin/dummy"))?;

        assert_eq!(executable_path, bin_path.join("dummy"));

        Ok(())
    }

    // find_tool_versions will find a .tool-versions path if it exists in current directory
    #[test]
    #[serial]
    fn find_tool_versions_in_cwd() -> Result<()> {
        let context = setup()?;
        let tool_versions_path = context.project_dir.path().join(".tool-versions");
        fs::write(&tool_versions_path, "dummy 0.1.0")?;
        let real_tool_versions_path = tool_versions_path.canonicalize()?;
        let _cwd = set_current_dir(context.project_dir.path())?;

        let tool_versions = super::find_tool_versions()?;

        assert_eq!(tool_versions, Some(real_tool_versions_path));

        Ok(())
    }

    // find_tool_versions will find a .tool-versions path if it exists in parent directory
    #[test]
    #[serial]
    fn find_tool_versions_in_parent() -> Result<()> {
        let context = setup()?;
        let tool_versions_path = context.project_dir.path().join(".tool-versions");
        fs::write(&tool_versions_path, "dummy 0.1.0")?;
        let real_tool_versions_path = tool_versions_path.canonicalize()?;
        fs::create_dir_all(context.project_dir.path().join("child"))?;
        let _cwd = set_current_dir(context.project_dir.path().join("child"))?;

        let tool_versions = super::find_tool_versions()?;

        assert_eq!(tool_versions, Some(real_tool_versions_path));

        Ok(())
    }

    // get_version_from_env returns the version set in the environment variable
    #[test]
    #[serial]
    fn version_from_env_with_env() -> Result<()> {
        let _context = setup()?;
        let _env = set_var("ASDF_DUMMY_VERSION", "0.1.0");

        let tool_versions = super::version_from_env("dummy")?;

        assert_eq!(tool_versions, Some(String::from("0.1.0")));

        Ok(())
    }

    // get_version_from_env returns nothing when environment variable is not set
    #[test]
    #[serial]
    fn version_from_env_without_env() -> Result<()> {
        let _context = setup()?;

        let tool_versions = super::version_from_env("dummy")?;

        assert_eq!(tool_versions, None);

        Ok(())
    }

    // resolve_symlink converts the symlink path to the real file path
    #[test]
    #[serial]
    fn resolve_symlink() -> Result<()> {
        let _context = setup()?;
        let foo_path = env::current_dir()?.join("foo");
        fs::write(&foo_path, "")?;
        std::os::unix::fs::symlink(&foo_path, "bar")?;

        let tool_versions = super::resolve_symlink(&PathBuf::from("bar"))?;

        assert_eq!(tool_versions, PathBuf::from(foo_path));

        Ok(())
    }

    // resolve_symlink converts the symlink path to the real file path
    #[test]
    #[serial]
    fn resolve_symlink_with_relative_directory() -> Result<()> {
        let _context = setup()?;
        fs::create_dir_all(PathBuf::from("foo"))?;
        fs::create_dir("baz")?;
        let bar_path = env::current_dir()?.join("baz").join("bar");
        let _cwd = set_current_dir(PathBuf::from("baz"))?;
        std::os::unix::fs::symlink(PathBuf::from("..").join("foo"), &bar_path)?;

        let tool_versions = super::resolve_symlink(&PathBuf::from("bar"))?;

        assert_eq!(
            tool_versions,
            PathBuf::from(env::current_dir()?.parent().unwrap().join("foo"))
        );

        Ok(())
    }

    // resolve_symlink converts relative symlink path to the real file path
    #[test]
    #[serial]
    fn resolve_symlink_with_relative_file() -> Result<()> {
        let _context = setup()?;
        let foo_path = PathBuf::from("foo");
        let bar_path = PathBuf::from("bar");
        fs::write(&foo_path, "")?;
        std::os::unix::fs::symlink(foo_path, &bar_path)?;

        let tool_versions = super::resolve_symlink(&bar_path)?;

        assert_eq!(
            tool_versions,
            PathBuf::from(env::current_dir()?.join("foo"))
        );

        Ok(())
    }

    // strip_tool_version_comments removes lines that only contain comments
    #[test]
    fn parse_tool_version_line_with_comment_only_line_with_newline() -> Result<()> {
        let tool_version_line = super::parse_tool_version_line(&"# comment line\n");

        assert_eq!(tool_version_line, None);

        Ok(())
    }

    // strip_tool_version_comments removes lines that only contain comments, without newline
    #[test]
    fn parse_tool_version_line_with_comment_only_line() -> Result<()> {
        let tool_version_line = super::parse_tool_version_line(&"# comment line");

        assert_eq!(tool_version_line, None);

        Ok(())
    }

    // strip_tool_version_comments removes trailing comments on lines containing version information
    #[test]
    fn parse_tool_version_line_with_inline_comment_with_newline() -> Result<()> {
        let tool_version_line = super::parse_tool_version_line(&"ruby 2.0.0 # inline comment\n");

        assert_eq!(tool_version_line, Some(("ruby", "2.0.0")));

        Ok(())
    }

    // strip_tool_version_comments removes trailing comments on lines containing version information even with missing newline
    #[test]
    fn parse_tool_version_line_with_inline_comment() -> Result<()> {
        let tool_version_line = super::parse_tool_version_line(&"ruby 2.0.0 # inline comment");

        assert_eq!(tool_version_line, Some(("ruby", "2.0.0")));

        Ok(())
    }

    // strip_tool_version_comments removes all comments from the version file
    #[test]
    fn parse_tool_version_line_with_lots_of_comments() -> Result<()> {
        let test_file =
            "ruby 2.0.0 # inline comment\n# comment line\nerlang 18.2.1 # inline comment\n";

        let versions = test_file
            .lines()
            .filter_map(super::parse_tool_version_line)
            .collect::<Vec<_>>();

        assert_eq!(versions, vec![("ruby", "2.0.0"), ("erlang", "18.2.1")]);

        Ok(())
    }

    // @test "" {
    //   cd $PROJECT_DIR
    //   echo "dummy 0.1.0" > $PROJECT_DIR/.tool-versions
    //   mkdir -p $ASDF_DIR/installs/dummy/0.1.0/bin
    //   touch $ASDF_DIR/installs/dummy/0.1.0/bin/test-dash
    //   chmod +x $ASDF_DIR/installs/dummy/0.1.0/bin/test-dash
    //   run asdf reshim dummy 0.1.0

    //   message="callback invoked"

    //   function callback() {
    //     echo $message
    //   }

    //   run with_shim_executable test-dash callback
    //   [ "$status" -eq 0 ]
    //   [ "$output" = "$message" ]
    // }

    // with_shim_executable doesn't crash when executable names contain dashes
    #[test]
    #[serial]
    fn with_shim_executable_with_exe_with_dashes() -> Result<()> {
        let context = setup()?;
        let _cwd = set_current_dir(&context.project_dir)?;
        let tool_version_path = context.project_dir.path().join(".tool-version");
        fs::write(tool_version_path, "dummy 0.1.0")?;
        let bin_path = context
            .home_dir
            .path()
            .join(".asdf")
            .join("installs")
            .join("dummy")
            .join("0.1.0")
            .join("bin");
        fs::create_dir_all(&bin_path)?;
        // touch $ASDF_DIR/installs/dummy/0.1.0/bin/test-dash
        fs::write(&bin_path.join("test-dash"), "")?;
        fs::set_permissions(
            &bin_path.join("test-dash"),
            PermissionsExt::from_mode(0o755),
        )?;

        //let tool_versions = super::with_shim_executable(&bar_path)?;

        //assert_eq!(
        //    tool_versions,
        //     PathBuf::from(env::current_dir()?.join("foo"))
        // );

        Ok(())
    }

    #[test]
    #[serial]
    fn list_installed_versions() -> Result<()> {
        let _context = setup()?;

        assert_eq!(
            super::list_installed_versions("dummy")?,
            vec![String::from("0.1.0"), String::from("0.2.0")]
        );

        Ok(())
    }

    #[test]
    #[serial]
    fn list_installed_plugins() -> Result<()> {
        let context = setup()?;
        for plugin in &["mock_plugin_2", "mock_plugin_1"] {
            install_mock_plugin(plugin, &context.home_dir.path().join(".asdf"))?;
        }

        assert_eq!(
            super::list_installed_plugins()?,
            vec![
                String::from("dummy"),
                String::from("mock_plugin_1"),
                String::from("mock_plugin_2")
            ]
        );

        Ok(())
    }
}
