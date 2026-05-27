use std::process::Command;

pub fn check_not_root() -> anyhow::Result<()> {
    let uid = unsafe { libc::getuid() };
    if uid == 0 {
        anyhow::bail!(crate::error::GuardError::RefuseRoot);
    }
    Ok(())
}

/// Gate challenge: sudo -k → sudo -v → sudo -k
///
/// The sequence ensures:
/// 1. Any previously cached sudo credentials are invalidated.
/// 2. User must authenticate fresh.
/// 3. Credentials are immediately invalidated again so they don't linger.
pub fn challenge() -> anyhow::Result<()> {
    check_not_root()?;

    let status = Command::new("sudo")
        .arg("-k")
        .status()
        .map_err(|e| crate::error::GuardError::Other(format!("sudo not available: {e}")))?;
    if !status.success() {
        anyhow::bail!(crate::error::GuardError::GateDenied);
    }

    let status = Command::new("sudo")
        .arg("-v")
        .status()
        .map_err(|e| crate::error::GuardError::Other(format!("sudo not available: {e}")))?;
    if !status.success() {
        anyhow::bail!(crate::error::GuardError::GateDenied);
    }

    // Immediately invalidate cached credentials so they don't linger.
    let _ = Command::new("sudo").arg("-k").status();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_not_root_passes_when_not_root() {
        // Tests should never run as root.
        check_not_root().expect("check_not_root should pass when UID != 0");
    }
}
