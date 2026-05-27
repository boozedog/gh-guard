use std::env;
use std::path::PathBuf;

fn home_dir() -> anyhow::Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("HOME environment variable not set"))
}

fn xdg_config_home() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().unwrap_or_default().join(".config"))
}

fn xdg_data_home() -> PathBuf {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().unwrap_or_default().join(".local/share"))
}

fn xdg_cache_home() -> PathBuf {
    env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().unwrap_or_default().join(".cache"))
}

pub fn config_dir() -> PathBuf {
    xdg_config_home().join("gh-guard")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn data_dir() -> PathBuf {
    xdg_data_home().join("gh-guard")
}

pub fn cache_dir() -> PathBuf {
    xdg_cache_home().join("gh-guard")
}

pub fn versions_dir() -> PathBuf {
    data_dir().join("versions")
}

pub fn bin_dir() -> PathBuf {
    data_dir().join("bin")
}

pub fn real_gh_symlink() -> PathBuf {
    bin_dir().join("gh-real")
}

pub fn logs_dir() -> PathBuf {
    data_dir().join("logs")
}

pub fn state_path() -> PathBuf {
    data_dir().join("state.json")
}

pub fn lock_path() -> PathBuf {
    data_dir().join("state.lock")
}

pub fn ensure_dirs() -> anyhow::Result<()> {
    std::fs::create_dir_all(config_dir())?;
    std::fs::create_dir_all(versions_dir())?;
    std::fs::create_dir_all(bin_dir())?;
    std::fs::create_dir_all(logs_dir())?;
    std::fs::create_dir_all(cache_dir().join("downloads"))?;
    Ok(())
}
