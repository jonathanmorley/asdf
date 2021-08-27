use crate::{asdf_config_value, asdf_run_hook, call, core::reshim::reshim_plugin, download_path, find_versions, install_path, list_installed_plugins, plugin_exists, plugin_path, tool_version::ToolVersion};
use anyhow::{anyhow, Result};
use num_cpus;
use std::{env, ffi::OsStr, fs, process};

pub fn concurrency() -> usize {
    num_cpus::get()
}

pub fn install_one_local_tool(plugin_name: &str) -> Result<()> {
    Ok(())
}

pub fn install_local_tool_versions() -> Result<()> {
    let plugins = list_installed_plugins()?;

    if plugins.is_empty() {
        return Err(anyhow!("Install plugins first to be able to install tools"));
    }

    let search_path = env::current_dir()?;
    let mut some_tools_installed = false;

    for plugin in plugins {
        if let Some(plugin_versions) = find_versions(&plugin, &search_path)? {
            some_tools_installed = true;
            for plugin_version in plugin_versions.version.split(' ') {
                install_tool_version(&plugin, plugin_version, false)?;
            }
        }
    }

    if !some_tools_installed {
        Err(anyhow!("Either specify a tool & version in the command\nOR add .tool-versions file in this directory\nor in a parent directory"))
    } else {
        Ok(())
    }
}

pub fn install_tool_version(
    plugin_name: &str,
    full_version: &str,
    keep_download: bool,
) -> Result<()> {
    let plugin_path = plugin_path(plugin_name)?;
    plugin_exists(plugin_name)?;

    if full_version == "system" {
        return Ok(());
    }

    let tool_version: ToolVersion = full_version.parse()?;
    let install_type = tool_version.install_type();
    let version = tool_version.install_version(plugin_name)?.unwrap();

    let install_path = install_path(plugin_name, &install_type, &version)?;
    let download_path = download_path(plugin_name, &install_type, &version)?;
    let concurrency = concurrency();

    // trap 'handle_cancel $install_path' INT

    if install_path.is_dir() {
        println!("{} {} is already installed", plugin_name, full_version);
        return Ok(());
    }

    let download_bin = plugin_path.join("bin").join("download");
    if download_bin.is_file() {
        // Not a legacy plugin
        // Run the download script
        fs::create_dir(&download_path.clone().unwrap())?;

        asdf_run_hook(
            &format!("pre_asdf_install_{}", plugin_name),
            &[full_version],
            vec![
                ("concurrency", OsStr::new(&concurrency.to_string())),
                ("download_path", download_path.clone().unwrap().as_os_str()),
                ("install_path", install_path.as_os_str()),
                ("version", OsStr::new(&version)),
                ("full_version", OsStr::new(&full_version)),
                ("install_type", OsStr::new(&install_type)),
                (
                    "keep_download",
                    OsStr::new(if keep_download { "true" } else { "" }),
                ),
                ("plugin_path", plugin_path.as_os_str()),
                (
                    "flags",
                    OsStr::new(if keep_download { "--keep-download" } else { "" }),
                ),
                ("plugin_name", OsStr::new(&plugin_name)),
                // There are more available via bash because of the variable non-locality
            ],
        )?;

        call(process::Command::new(&download_bin).envs(vec![
            ("ASDF_INSTALL_TYPE", OsStr::new(&install_type)),
            ("ASDF_INSTALL_VERSION", OsStr::new(&version)),
            ("ASDF_INSTALL_PATH", install_path.as_os_str()),
            (
                "ASDF_DOWNLOAD_PATH",
                download_path.clone().unwrap().as_os_str(),
            ),
        ]))?;
    }

    fs::create_dir(&install_path)?;
    let install_bin = plugin_path.join("bin").join("install");
    call(process::Command::new(install_bin).envs(vec![
        ("ASDF_INSTALL_TYPE", OsStr::new(&install_type)),
        ("ASDF_INSTALL_VERSION", OsStr::new(&version)),
        ("ASDF_INSTALL_PATH", install_path.as_os_str()),
        (
            "ASDF_DOWNLOAD_PATH",
            download_path.clone().unwrap().as_os_str(),
        ),
        ("ASDF_CONCURRENCY", OsStr::new(&concurrency.to_string())),
    ]))?;

    let always_keep_download = asdf_config_value("always_keep_download")?.unwrap_or_default();
    if !keep_download && !always_keep_download.eq("yes") && download_path.clone().unwrap().is_dir()
    {
        fs::remove_dir_all(download_path.clone().unwrap())?;
    }

    reshim_plugin(plugin_name, Some(full_version))?;

    asdf_run_hook(
        &format!("post_asdf_install_{}", plugin_name),
        &[full_version],
        vec![
            (
                "always_keep_download",
                OsStr::new(&always_keep_download.to_string()),
            ),
            ("install_exit_code", OsStr::new(&0.to_string())),
            ("download_exit_code", OsStr::new(&0.to_string())),
            ("concurrency", OsStr::new(&concurrency.to_string())),
            ("download_path", download_path.clone().unwrap().as_os_str()),
            ("install_path", install_path.as_os_str()),
            ("version", OsStr::new(&version)),
            ("full_version", OsStr::new(&full_version)),
            ("install_type", OsStr::new(&install_type)),
            (
                "keep_download",
                OsStr::new(if keep_download { "true" } else { "" }),
            ),
            ("plugin_path", plugin_path.as_os_str()),
            (
                "flags",
                OsStr::new(if keep_download { "--keep-download" } else { "" }),
            ),
            ("plugin_name", OsStr::new(&plugin_name)),
            // There are more available via bash because of the variable non-locality
        ],
    )?;

    

    Ok(())
}
