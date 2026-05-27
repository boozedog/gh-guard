use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

pub fn exec_real(real_gh: &Path, args: &[String]) -> anyhow::Result<()> {
    validate_exec(real_gh)?;

    let err = Command::new(real_gh).args(args).exec();

    Err(anyhow::anyhow!("exec failed: {err}"))
}

/// Validates that `real_gh` is safe to exec:
/// 1. Path must be absolute (never search PATH).
/// 2. Must not be the wrapper itself (recursion guard).
pub fn validate_exec(real_gh: &Path) -> anyhow::Result<()> {
    if !real_gh.is_absolute() {
        anyhow::bail!("real gh path must be absolute");
    }

    let self_path = std::env::current_exe()?;
    if is_same_file(real_gh, &self_path)? {
        anyhow::bail!(crate::error::GuardError::Recursion);
    }

    Ok(())
}

pub fn is_same_file(a: &Path, b: &Path) -> anyhow::Result<bool> {
    let meta_a = std::fs::metadata(a)?;
    let meta_b = std::fs::metadata(b)?;
    Ok(meta_a.dev() == meta_b.dev() && meta_a.ino() == meta_b.ino())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_exec_rejects_relative_path() {
        let result = validate_exec(Path::new("gh"));
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("absolute"));
    }

    #[test]
    fn test_validate_exec_detects_recursion() {
        let self_path = std::env::current_exe().unwrap();
        let result = validate_exec(&self_path);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("recursion"));
    }

    #[test]
    fn test_is_same_file_same_path() {
        let tmp = std::env::temp_dir().join("gh_guard_test_same_file");
        std::fs::write(&tmp, "test").unwrap();
        assert!(is_same_file(&tmp, &tmp).unwrap());
        std::fs::remove_file(&tmp).unwrap();
    }

    #[test]
    fn test_is_same_file_different_files() {
        let a = std::env::temp_dir().join("gh_guard_test_file_a");
        let b = std::env::temp_dir().join("gh_guard_test_file_b");
        std::fs::write(&a, "a").unwrap();
        std::fs::write(&b, "b").unwrap();
        assert!(!is_same_file(&a, &b).unwrap());
        std::fs::remove_file(&a).unwrap();
        std::fs::remove_file(&b).unwrap();
    }
}
