use std::path::Path;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_default_config_gates_auth_status() {
    let config = gh_guard::config::default_config();
    let (action, _) =
        gh_guard::policy::decide(&config, &["auth".to_string(), "status".to_string()]);
    assert_eq!(
        action,
        gh_guard::policy::Action::Gate,
        "auth status should be gated by default"
    );
}

#[test]
fn test_default_config_allows_version() {
    let config = gh_guard::config::default_config();
    let (action, _) = gh_guard::policy::decide(&config, &["--version".to_string()]);
    assert_eq!(
        action,
        gh_guard::policy::Action::Allow,
        "--version should be allowed by default"
    );
}

#[test]
fn test_exec_rejects_relative_path() {
    let result = gh_guard::exec::validate_exec(Path::new("gh"));
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("absolute"),
        "error should mention absolute path: {}",
        msg
    );
}

#[test]
fn test_exec_detects_recursion() {
    let self_path = std::env::current_exe().unwrap();
    let result = gh_guard::exec::validate_exec(&self_path);
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("recursion"),
        "error should mention recursion: {}",
        msg
    );
}

#[test]
fn test_gate_check_not_root() {
    gh_guard::gate::check_not_root().expect("should pass when not root");
}

/// Pinned version mismatch must fail closed when the update cannot be fetched.
#[test]
fn test_pinned_version_fails_closed_on_update_failure() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();

    // Point XDG dirs at the temp location.
    std::env::set_var("HOME", home);
    std::env::set_var("XDG_CONFIG_HOME", home.join(".config"));
    std::env::set_var("XDG_DATA_HOME", home.join(".local/share"));
    std::env::set_var("XDG_CACHE_HOME", home.join(".cache"));

    // Build the expected directory tree.
    std::fs::create_dir_all(home.join(".config/gh-guard")).unwrap();
    std::fs::create_dir_all(home.join(".local/share/gh-guard/versions/v2.93.0")).unwrap();
    std::fs::create_dir_all(home.join(".local/share/gh-guard/bin")).unwrap();

    // Create a fake gh binary to act as the cached version.
    let fake_gh = home.join(".local/share/gh-guard/versions/v2.93.0/gh");
    std::fs::write(&fake_gh, "#!/bin/sh\necho 2.93.0").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&fake_gh).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&fake_gh, perms).unwrap();
    }

    // Symlink gh-real -> fake_gh.
    let symlink = home.join(".local/share/gh-guard/bin/gh-real");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&fake_gh, &symlink).unwrap();
    }
    #[cfg(not(unix))]
    {
        std::fs::copy(&fake_gh, &symlink).unwrap();
    }

    // Write state showing the cached version is different from the pinned one.
    let state = gh_guard::state::State {
        last_update_check: Some(time::OffsetDateTime::now_utc()),
        current_version: Some("v2.93.0".to_string()),
        last_known_latest_version: Some("v2.93.0".to_string()),
    };
    state.save().unwrap();

    // Configure a pinned version that does not exist (forces update failure).
    let mut config = gh_guard::config::default_config();
    config.updates.pinned_version = "v0.0.0-fake".to_string();
    config.updates.auto_update = true;

    // update() will fail because the release does not exist.
    // With a pinned version set, ensure_real_gh must fail closed rather than
    // fall back to the cached (wrong) binary.
    let result = gh_guard::gh_manager::ensure_real_gh(&config);
    assert!(
        result.is_err(),
        "pinned version mismatch should fail closed when update fails"
    );
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("pinned version") || msg.contains("could not be installed"),
        "error should mention pinned version: {}",
        msg
    );
}
