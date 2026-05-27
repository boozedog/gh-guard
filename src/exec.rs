use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

pub fn exec_real(real_gh: &Path, args: &[String]) -> anyhow::Result<()> {
    if !real_gh.is_absolute() {
        anyhow::bail!("real gh path must be absolute");
    }

    let self_path = std::env::current_exe()?;
    if is_same_file(real_gh, &self_path)? {
        anyhow::bail!(crate::error::GuardError::Recursion);
    }

    let err = Command::new(real_gh).args(args).exec();

    Err(anyhow::anyhow!("exec failed: {err}"))
}

fn is_same_file(a: &Path, b: &Path) -> anyhow::Result<bool> {
    let meta_a = std::fs::metadata(a)?;
    let meta_b = std::fs::metadata(b)?;
    Ok(meta_a.dev() == meta_b.dev() && meta_a.ino() == meta_b.ino())
}
