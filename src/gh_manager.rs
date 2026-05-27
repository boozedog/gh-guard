use anyhow::Context;
use fs4::fs_std::FileExt;
use std::fs;
use std::path::PathBuf;

use crate::config::Config;
use crate::download::{
    download_asset, extract_archive, extract_checksum, get_latest_release, get_release_by_tag,
    select_asset, verify_sha256,
};
use crate::metadata::Metadata;
use crate::paths;
use crate::state::State;

pub fn ensure_real_gh(config: &Config) -> anyhow::Result<PathBuf> {
    let symlink = paths::real_gh_symlink();

    if symlink.exists() {
        let target = fs::read_link(&symlink)?;
        if target.exists() {
            let needs_update = if !config.updates.pinned_version.is_empty() {
                let state = State::load()?;
                state.current_version.as_ref() != Some(&config.updates.pinned_version)
            } else {
                should_update(config)?
            };

            if needs_update {
                let _ = crate::logging::global_log(
                    "update_check_started",
                    std::collections::HashMap::new(),
                );
                match update(config) {
                    Ok(()) => {
                        let target = fs::read_link(&symlink)?;
                        if target.exists() {
                            return Ok(target);
                        }
                    }
                    Err(e) => {
                        let _ = crate::logging::global_log(
                            "update_check_failed",
                            std::collections::HashMap::from([(
                                "error".to_string(),
                                serde_json::Value::String(e.to_string()),
                            )]),
                        );
                        if !config.updates.pinned_version.is_empty() {
                            anyhow::bail!(
                                "pinned version {} could not be installed: {}",
                                config.updates.pinned_version,
                                e
                            );
                        }
                        let _ = crate::logging::global_log(
                            "using_cached_gh",
                            std::collections::HashMap::new(),
                        );
                        eprintln!("gh-guard: update check failed: {}", e);
                        if target.exists() {
                            return Ok(target);
                        }
                    }
                }
            } else {
                return Ok(target);
            }
        }
    }

    install(config)?;
    let target = fs::read_link(&symlink)?;
    if !target.exists() {
        anyhow::bail!(crate::error::GuardError::RealGhMissing(
            "symlink target missing after install".to_string()
        ));
    }

    // Defense-in-depth: verify pinned version after first install
    if !config.updates.pinned_version.is_empty() {
        let state = State::load()?;
        if state.current_version.as_ref() != Some(&config.updates.pinned_version) {
            anyhow::bail!(
                "pinned version {} is not active after install (current: {:?})",
                config.updates.pinned_version,
                state.current_version
            );
        }
    }

    Ok(target)
}

fn should_update(config: &Config) -> anyhow::Result<bool> {
    if !config.updates.auto_update {
        return Ok(false);
    }

    let state = State::load()?;
    let interval = parse_duration(&config.updates.check_interval)?;

    if let Some(last) = state.last_update_check {
        let now = time::OffsetDateTime::now_utc();
        let diff = now - last;
        if diff < interval {
            return Ok(false);
        }
    }

    Ok(true)
}

fn parse_duration(s: &str) -> anyhow::Result<time::Duration> {
    if s.is_empty() {
        return Ok(time::Duration::hours(24));
    }
    let num_str: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    let unit: String = s.chars().skip_while(|c| c.is_ascii_digit()).collect();
    let num: i64 = num_str.parse().context("invalid duration number")?;

    match unit.as_str() {
        "h" | "H" => Ok(time::Duration::hours(num)),
        "m" | "M" => Ok(time::Duration::minutes(num)),
        "d" | "D" => Ok(time::Duration::days(num)),
        _ => anyhow::bail!("invalid duration unit: {}", unit),
    }
}

pub fn update(config: &Config) -> anyhow::Result<()> {
    let lock_file = fs::File::create(paths::lock_path())?;
    lock_file.lock_exclusive()?;

    let state = State::load()?;

    let release = if config.updates.pinned_version.is_empty() {
        get_latest_release(&config.real_gh.release_repo)?
    } else {
        get_release_by_tag(&config.real_gh.release_repo, &config.updates.pinned_version)?
    };
    let target_version = release.tag_name.clone();

    if Some(target_version.clone()) == state.current_version {
        let mut state = state;
        state.last_update_check = Some(time::OffsetDateTime::now_utc());
        state.save()?;
        return Ok(());
    }

    let asset = select_asset(&release.assets)?;
    let cache_path = paths::cache_dir().join("downloads").join(&asset.name);

    let _ = crate::logging::global_log(
        "download_started",
        std::collections::HashMap::from([
            (
                "version".to_string(),
                serde_json::Value::String(target_version.clone()),
            ),
            (
                "asset".to_string(),
                serde_json::Value::String(asset.name.clone()),
            ),
        ]),
    );

    download_asset(&asset.browser_download_url, &cache_path)?;

    let checksums_asset = release
        .assets
        .iter()
        .find(|a| a.name.ends_with("_checksums.txt"))
        .context("no checksums file found in release")?;
    let checksums_path = paths::cache_dir()
        .join("downloads")
        .join(&checksums_asset.name);
    download_asset(&checksums_asset.browser_download_url, &checksums_path)?;
    let expected = extract_checksum(&checksums_path, &asset.name)?;
    verify_sha256(&cache_path, &expected)?;

    let _ = crate::logging::global_log(
        "download_verified",
        std::collections::HashMap::from([
            (
                "version".to_string(),
                serde_json::Value::String(target_version.clone()),
            ),
            (
                "asset".to_string(),
                serde_json::Value::String(asset.name.clone()),
            ),
        ]),
    );

    let temp_dir = tempfile::tempdir_in(paths::data_dir())?;
    let extracted_gh = extract_archive(&cache_path, temp_dir.path())?;

    let version_dir = paths::versions_dir().join(&target_version);
    fs::create_dir_all(&version_dir)?;
    let dest_bin = version_dir.join("gh");
    fs::copy(&extracted_gh, &dest_bin)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dest_bin)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest_bin, perms)?;
    }

    let metadata = Metadata {
        version: target_version.clone(),
        asset_name: asset.name.clone(),
        download_url: asset.browser_download_url.clone(),
        sha256: expected,
        installed_at: time::OffsetDateTime::now_utc(),
    };
    let meta_path = version_dir.join("metadata.json");
    fs::write(&meta_path, serde_json::to_string_pretty(&metadata)?)?;

    let symlink = paths::real_gh_symlink();
    let new_symlink = paths::bin_dir().join("gh-real.new");
    if new_symlink.exists() {
        fs::remove_file(&new_symlink)?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&dest_bin, &new_symlink)?;
    }
    #[cfg(not(unix))]
    {
        fs::copy(&dest_bin, &new_symlink)?;
    }
    fs::rename(&new_symlink, &symlink)?;

    let _ = crate::logging::global_log(
        "update_installed",
        std::collections::HashMap::from([(
            "version".to_string(),
            serde_json::Value::String(target_version.clone()),
        )]),
    );

    let mut state = state;
    state.last_update_check = Some(time::OffsetDateTime::now_utc());
    state.current_version = Some(target_version.clone());
    state.last_known_latest_version = if config.updates.pinned_version.is_empty() {
        Some(target_version)
    } else {
        state.last_known_latest_version
    };
    state.save()?;

    Ok(())
}

fn install(config: &Config) -> anyhow::Result<()> {
    update(config)
}
