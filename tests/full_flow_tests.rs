use assert_cmd::Command;
use std::fs;
use std::path::Path;

fn make_fake_gh(dir: &Path, log_path: &Path) -> std::path::PathBuf {
    let fake = dir.join("gh");
    let script = if cfg!(target_os = "windows") {
        r#"@echo off
echo args: %*
exit 42
"#
    } else {
        &format!(
            r#"#!/bin/sh
echo "fake_gh_args=$*" >> "{}"
exit 42
"#,
            log_path.display()
        )
    };
    fs::write(&fake, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&fake).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake, perms).unwrap();
    }
    fake
}

fn make_fake_sudo(dir: &Path, log_path: &Path) -> std::path::PathBuf {
    let fake = dir.join("sudo");
    let script = if cfg!(target_os = "windows") {
        unimplemented!("Windows not supported for fake sudo")
    } else {
        &format!(
            r#"#!/bin/sh
echo "$*" >> "{}"
if [ "$1" = "-v" ] && [ -n "$FAKE_SUDO_FAIL" ]; then
    exit 1
fi
exit 0
"#,
            log_path.display()
        )
    };
    fs::write(&fake, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&fake).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake, perms).unwrap();
    }
    fake
}

fn setup_fake_gh(home: &Path, fake_gh: &Path) {
    let versions_dir = home.join(".local/share/gh-guard/versions/v2.92.0");
    fs::create_dir_all(&versions_dir).unwrap();
    fs::create_dir_all(home.join(".local/share/gh-guard/bin")).unwrap();

    let dest = versions_dir.join("gh");
    fs::copy(fake_gh, &dest).unwrap();

    let symlink = home.join(".local/share/gh-guard/bin/gh-real");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&dest, &symlink).unwrap();
    }
    #[cfg(not(unix))]
    {
        fs::copy(&dest, &symlink).unwrap();
    }

    let metadata = serde_json::json!({
        "version": "v2.92.0",
        "asset_name": "gh_2.92.0_test.zip",
        "download_url": "https://example.com/fake",
        "sha256": "0000",
        "installed_at": time::OffsetDateTime::now_utc(),
    });
    fs::write(
        versions_dir.join("metadata.json"),
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .unwrap();

    let state = serde_json::json!({
        "last_update_check": time::OffsetDateTime::now_utc(),
        "current_version": "v2.92.0",
        "last_known_latest_version": "v2.92.0",
    });
    fs::write(
        home.join(".local/share/gh-guard/state.json"),
        serde_json::to_string_pretty(&state).unwrap(),
    )
    .unwrap();

    // Disable auto-update in config so ensure_real_gh does not try to download.
    let config_dir = home.join(".config/gh-guard");
    fs::create_dir_all(&config_dir).unwrap();
    let config = r#"[default]
action = "gate"

[updates]
auto_update = false
check_interval = "24h"
pinned_version = ""

[logging]
level = "info"
redact = true
otel_endpoint = ""

[real_gh]
download_source = "github"
release_repo = "cli/cli"

[gate]
method = "sudo"
fresh = true

[[rules]]
match = ["--version"]
action = "allow"

[[rules]]
match = ["--help"]
action = "allow"

[[rules]]
match = ["help"]
action = "allow"

[[rules]]
match = ["completion"]
action = "allow"
"#;
    fs::write(config_dir.join("config.toml"), config).unwrap();
}

fn build_wrapper_cmd(
    home: &Path,
    fake_bin: &Path,
    sudo_log: &Path,
    gh_log: &Path,
    sudo_fail: bool,
) -> Command {
    let mut cmd = Command::cargo_bin("gh").unwrap();
    cmd.env("HOME", home);
    cmd.env("XDG_CONFIG_HOME", home.join(".config"));
    cmd.env("XDG_DATA_HOME", home.join(".local/share"));
    cmd.env("XDG_CACHE_HOME", home.join(".cache"));
    cmd.env("PATH", format!("{}:/usr/bin:/bin", fake_bin.display()));
    cmd.env("FAKE_SUDO_LOG", sudo_log);
    cmd.env("FAKE_GH_LOG", gh_log);
    if sudo_fail {
        cmd.env("FAKE_SUDO_FAIL", "1");
    }
    cmd
}

#[test]
fn allowed_command_does_not_call_sudo() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let fake_bin = home.join("fake_bin");
    fs::create_dir_all(&fake_bin).unwrap();

    let sudo_log = home.join("sudo.log");
    let gh_log = home.join("gh.log");
    let fake_gh = make_fake_gh(&fake_bin, &gh_log);
    let _fake_sudo = make_fake_sudo(&fake_bin, &sudo_log);

    setup_fake_gh(home, &fake_gh);

    let mut cmd = build_wrapper_cmd(home, &fake_bin, &sudo_log, &gh_log, false);
    cmd.arg("--version");
    let assert = cmd.assert();

    // fake gh exits 42, and exec() replaces process, so wrapper exits 42.
    assert.failure().code(42);

    // Fake sudo should NOT have been called.
    assert!(
        !sudo_log.exists(),
        "sudo should not be called for allowed commands"
    );

    // Fake gh should have been invoked.
    assert!(gh_log.exists(), "fake gh should have been invoked");
    let gh_content = fs::read_to_string(&gh_log).unwrap();
    assert!(
        gh_content.contains("--version"),
        "fake gh should receive --version: {}",
        gh_content
    );

    // Verify logs contain decision=allow and absolute path.
    let log_dir = home.join(".local/share/gh-guard/logs");
    let entries = read_jsonl_logs(&log_dir);
    assert!(
        entries.iter().any(|e| {
            e.get("event") == Some(&serde_json::Value::String("policy_decision".to_string()))
                && e.get("decision") == Some(&serde_json::Value::String("allow".to_string()))
        }),
        "log should contain allow decision"
    );
    assert!(
        entries
            .iter()
            .any(|e| e.get("event")
                == Some(&serde_json::Value::String("exec_started".to_string()))),
        "log should contain exec_started"
    );
    let exec_entry = entries
        .iter()
        .find(|e| e.get("event") == Some(&serde_json::Value::String("exec_started".to_string())))
        .unwrap();
    let path = exec_entry.get("real_gh_path").unwrap().as_str().unwrap();
    assert!(
        Path::new(path).is_absolute(),
        "real_gh_path must be absolute: {}",
        path
    );
}

#[test]
fn gated_command_calls_sudo_sequence() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let fake_bin = home.join("fake_bin");
    fs::create_dir_all(&fake_bin).unwrap();

    let sudo_log = home.join("sudo.log");
    let gh_log = home.join("gh.log");
    let fake_gh = make_fake_gh(&fake_bin, &gh_log);
    let _fake_sudo = make_fake_sudo(&fake_bin, &sudo_log);

    setup_fake_gh(home, &fake_gh);

    let mut cmd = build_wrapper_cmd(home, &fake_bin, &sudo_log, &gh_log, false);
    cmd.args(["api", "user"]);
    let assert = cmd.assert();

    // fake gh exits 42, exec() replaces process.
    assert.failure().code(42);

    // Verify sudo was called with exact sequence.
    assert!(
        sudo_log.exists(),
        "sudo should have been called for gated command"
    );
    let sudo_lines: Vec<String> = fs::read_to_string(&sudo_log)
        .unwrap()
        .lines()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        sudo_lines,
        vec!["-k", "-v", "-k"],
        "sudo sequence should be -k, -v, -k: {:?}",
        sudo_lines
    );

    // Verify fake gh received the args.
    assert!(gh_log.exists(), "fake gh should have been invoked");
    let gh_content = fs::read_to_string(&gh_log).unwrap();
    assert!(
        gh_content.contains("api user"),
        "fake gh should receive 'api user': {}",
        gh_content
    );

    // Verify log sequence.
    let log_dir = home.join(".local/share/gh-guard/logs");
    let entries = read_jsonl_logs(&log_dir);
    let events: Vec<String> = entries
        .iter()
        .filter_map(|e| {
            e.get("event")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect();
    assert!(
        events.contains(&"gated_attempt".to_string()),
        "log should contain gated_attempt"
    );
    assert!(
        events.contains(&"gate_authorized".to_string()),
        "log should contain gate_authorized"
    );
    assert!(
        events.contains(&"exec_started".to_string()),
        "log should contain exec_started"
    );
}

#[test]
fn sudo_failure_prevents_real_gh() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let fake_bin = home.join("fake_bin");
    fs::create_dir_all(&fake_bin).unwrap();

    let sudo_log = home.join("sudo.log");
    let gh_log = home.join("gh.log");
    let fake_gh = make_fake_gh(&fake_bin, &gh_log);
    let _fake_sudo = make_fake_sudo(&fake_bin, &sudo_log);

    setup_fake_gh(home, &fake_gh);

    let mut cmd = build_wrapper_cmd(home, &fake_bin, &sudo_log, &gh_log, true);
    cmd.args(["api", "user"]);
    let assert = cmd.assert();

    // Should exit non-zero because sudo -v failed.
    assert.failure();

    // Fake gh should NOT have been invoked.
    assert!(
        !gh_log.exists(),
        "fake gh should not be invoked when sudo fails"
    );

    // Verify gate_denied in logs.
    let log_dir = home.join(".local/share/gh-guard/logs");
    let entries = read_jsonl_logs(&log_dir);
    assert!(
        entries.iter().any(|e| {
            e.get("event") == Some(&serde_json::Value::String("gate_denied".to_string()))
        }),
        "log should contain gate_denied"
    );
}

fn read_jsonl_logs(dir: &Path) -> Vec<serde_json::Map<String, serde_json::Value>> {
    let mut result = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                for line in content.lines() {
                    if let Ok(serde_json::Value::Object(map)) =
                        serde_json::from_str::<serde_json::Value>(line)
                    {
                        result.push(map);
                    }
                }
            }
        }
    }
    result
}
