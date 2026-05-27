use anyhow::Context;
use reqwest::blocking::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{copy, Read};
use std::path::Path;

const GITHUB_API: &str = "https://api.github.com";

#[derive(Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
}

pub fn get_latest_release(repo: &str) -> anyhow::Result<Release> {
    let client = Client::new();
    let url = format!("{}/repos/{}/releases/latest", GITHUB_API, repo);
    let resp = client
        .get(&url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "gh-guard")
        .send()
        .with_context(|| format!("fetching release from {}", url))?;

    if !resp.status().is_success() {
        anyhow::bail!("GitHub API returned {}", resp.status());
    }

    let release: Release = resp.json().context("parsing release JSON")?;
    Ok(release)
}

pub fn get_release_by_tag(repo: &str, tag: &str) -> anyhow::Result<Release> {
    let client = Client::new();
    let url = format!("{}/repos/{}/releases/tags/{}", GITHUB_API, repo, tag);
    let resp = client
        .get(&url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "gh-guard")
        .send()
        .with_context(|| format!("fetching release {} from {}", tag, url))?;

    if !resp.status().is_success() {
        anyhow::bail!("GitHub API returned {}", resp.status());
    }

    let release: Release = resp.json().context("parsing release JSON")?;
    Ok(release)
}

pub fn select_asset(assets: &[Asset]) -> anyhow::Result<&Asset> {
    let (os, arch) = platform_id();
    let pattern_os = format!("_{}_", os);
    let pattern_arch = format!("_{}", arch);

    let candidate = assets
        .iter()
        .find(|a| {
            a.name.contains(&pattern_os)
                && a.name.contains(&pattern_arch)
                && (a.name.ends_with(".tar.gz") || a.name.ends_with(".zip"))
        })
        .ok_or_else(|| anyhow::anyhow!("no asset found for platform {} {}", os, arch))?;

    Ok(candidate)
}

fn platform_id() -> (&'static str, &'static str) {
    let os = if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "amd64"
    };
    (os, arch)
}

pub fn download_asset(url: &str, dest: &Path) -> anyhow::Result<()> {
    let client = Client::new();
    let mut resp = client
        .get(url)
        .header("User-Agent", "gh-guard")
        .send()
        .context("downloading asset")?;

    if !resp.status().is_success() {
        anyhow::bail!("download failed: {}", resp.status());
    }

    std::fs::create_dir_all(dest.parent().unwrap())?;
    let mut file = File::create(dest)?;
    copy(&mut resp, &mut file)?;
    Ok(())
}

pub fn verify_sha256(file_path: &Path, expected: &str) -> anyhow::Result<()> {
    let mut file = File::open(file_path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let hash = hex::encode(hasher.finalize());
    if hash != expected {
        anyhow::bail!("checksum mismatch: expected {}, got {}", expected, hash);
    }
    Ok(())
}

pub fn extract_archive(archive: &Path, dest: &Path) -> anyhow::Result<std::path::PathBuf> {
    let name = archive.to_string_lossy();
    if name.ends_with(".zip") {
        extract_zip(archive, dest)
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        extract_tar_gz(archive, dest)
    } else {
        anyhow::bail!("unsupported archive format: {}", archive.display())
    }
}

fn extract_zip(archive: &Path, dest: &Path) -> anyhow::Result<std::path::PathBuf> {
    let file = File::open(archive)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name();

        if entry.is_dir() || !is_gh_binary_entry(name) {
            continue;
        }

        let out_path = dest.join(sanitize_archive_path(name)?);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&out_path)?;
        std::io::copy(&mut entry, &mut out)?;
    }

    find_gh_binary(dest)
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> anyhow::Result<std::path::PathBuf> {
    let file = File::open(archive)?;
    let dec = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);

    for entry_result in archive.entries()? {
        let mut entry = entry_result?;
        let path = entry.path()?;
        let name = path.to_string_lossy();

        if entry.header().entry_type().is_dir() || !is_gh_binary_entry(&name) {
            continue;
        }

        let out_path = dest.join(sanitize_archive_path(&name)?);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        entry.unpack(&out_path)?;
    }

    find_gh_binary(dest)
}

/// Returns true for archive paths that are the `gh` binary.
/// GitHub CLI releases place it at `*/bin/gh` (or just `gh` in edge cases).
fn is_gh_binary_entry(name: &str) -> bool {
    name.ends_with("/bin/gh") || name == "gh"
}

/// Sanitize an archive entry path by stripping `..`, absolute prefixes,
/// and other path-traversal components.
fn sanitize_archive_path(name: &str) -> anyhow::Result<std::path::PathBuf> {
    let path = std::path::Path::new(name);
    let mut clean = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => clean.push(part),
            std::path::Component::CurDir => {}
            _ => anyhow::bail!("invalid archive path component: {:?}", component),
        }
    }
    Ok(clean)
}

fn find_gh_binary(dir: &Path) -> anyhow::Result<std::path::PathBuf> {
    use walkdir::WalkDir;
    for entry in WalkDir::new(dir).max_depth(5) {
        let entry = entry?;
        let path = entry.path();
        if path.file_name() == Some(std::ffi::OsStr::new("gh")) && entry.metadata()?.is_file() {
            return Ok(path.to_path_buf());
        }
    }
    anyhow::bail!("gh binary not found in extracted archive")
}

pub fn extract_checksum(checksums_path: &Path, asset_name: &str) -> anyhow::Result<String> {
    let content = std::fs::read_to_string(checksums_path)?;
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[1] == asset_name {
            return Ok(parts[0].to_string());
        }
    }
    anyhow::bail!("checksum for {} not found in checksums file", asset_name)
}
