# gh-guard Implementation Guide

## 1. Project Summary

`gh-guard` is a secure, drop-in wrapper for the official GitHub CLI. The installed executable is named `gh`, so users and scripts continue to invoke `gh ...` normally. The wrapper enforces a local policy before delegating to a managed, versioned copy of the real GitHub CLI.

The primary security model is whitelist-first:

- commands explicitly allowed by policy run immediately;
- all other commands are gated by a fresh local `sudo` authentication challenge;
- after successful authorization, the real `gh` runs as the original user, not as root.

The wrapper must never invoke `gh` by name when delegating. It must always execute the managed real GitHub CLI by absolute path.

## 2. Core Invariants

These should be treated as non-negotiable implementation rules.

1. The wrapper binary is installed as `gh`.
2. The real GitHub CLI is stored separately in a versioned data directory.
3. Delegation always uses an absolute path to the managed real binary.
4. The wrapper never runs `sudo gh ...`.
5. Gated commands use `sudo` only as an authorization challenge.
6. After authorization, the real `gh` executes as the original user.
7. Unknown commands are gated by default.
8. Logs are local JSON Lines and redact sensitive values by default.
9. No emergency environment-variable bypass exists.
10. Config, cache, and data paths use Linux/XDG conventions on both Linux and macOS.

## 3. Supported Platforms

Version 1 targets:

- Linux amd64
- Linux arm64
- macOS amd64
- macOS arm64

Version 1 does not target Windows.

## 4. Storage Layout

Use XDG/Linux-style paths on both Linux and macOS.

### Config

```text
$XDG_CONFIG_HOME/gh-guard/config.toml
```

Fallback:

```text
~/.config/gh-guard/config.toml
```

### Data

```text
$XDG_DATA_HOME/gh-guard/
```

Fallback:

```text
~/.local/share/gh-guard/
```

Suggested layout:

```text
~/.local/share/gh-guard/
  versions/
    v2.92.0/
      gh
      metadata.json
    v2.93.0/
      gh
      metadata.json
  bin/
    gh-real -> ../versions/v2.93.0/gh
  logs/
    gh-guard.jsonl
  state.json
```

### Cache

```text
$XDG_CACHE_HOME/gh-guard/
```

Fallback:

```text
~/.cache/gh-guard/
```

Suggested layout:

```text
~/.cache/gh-guard/
  downloads/
  releases.json
```

## 5. Recommended Rust Project Structure

```text
gh-guard/
  Cargo.toml
  IMPLEMENTATION.md
  README.md
  flake.nix
  src/
    main.rs
    config.rs
    paths.rs
    policy.rs
    command.rs
    gate.rs
    exec.rs
    gh_manager.rs
    download.rs
    logging.rs
    redact.rs
    guard_commands.rs
    error.rs
  config/
    default.toml
  packaging/
    homebrew/
```

Module responsibilities:

- `main.rs`: top-level control flow.
- `paths.rs`: XDG path resolution.
- `config.rs`: config loading, default creation, validation.
- `policy.rs`: rule model and decision engine.
- `command.rs`: argv parsing and command-path extraction.
- `gate.rs`: fresh sudo authorization challenge.
- `exec.rs`: absolute-path exec of real `gh`.
- `gh_manager.rs`: real GitHub CLI version management.
- `download.rs`: release API, asset download, checksum verification, extraction.
- `logging.rs`: local JSONL and optional OpenTelemetry setup.
- `redact.rs`: argv/env/config redaction helpers.
- `guard_commands.rs`: wrapper-reserved `gh guard ...` commands.
- `error.rs`: common error types.

## 6. Default Configuration

On first run, if no config exists, create this file with permissions `0600` where possible.

```toml
[default]
action = "gate"

[updates]
auto_update = true
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
match = ["version"]
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

[[rules]]
match = ["auth", "login"]
action = "allow"

[[rules]]
match = ["auth", "status"]
action = "allow"

[[rules]]
match = ["auth", "logout"]
action = "allow"

[[rules]]
match = ["repo", "view"]
action = "allow"

[[rules]]
match = ["pr", "list"]
action = "allow"

[[rules]]
match = ["pr", "view"]
action = "allow"

[[rules]]
match = ["pr", "checkout"]
action = "allow"

[[rules]]
match = ["issue", "list"]
action = "allow"

[[rules]]
match = ["issue", "view"]
action = "allow"
```

## 7. Policy Model

### Actions

Support these actions from the beginning:

```text
allow
```

Run the real GitHub CLI immediately.

```text
gate
```

Require a fresh `sudo` authorization challenge, then run the real GitHub CLI as the original user.

```text
deny
```

Refuse to run the command. This may not be used heavily in v1, but the action should exist from the start.

### Matching Semantics

Rules use tokenized command matching.

A rule matches when its `match` array is a prefix of the parsed command path.

Examples:

```toml
[[rules]]
match = ["pr", "view"]
action = "allow"
```

Allows:

```bash
gh pr view 123
gh pr view 123 --json title,body
```

Does not allow:

```bash
gh pr create
```

### Precedence

If multiple rules match:

1. Prefer the most specific rule, meaning the rule with the longest `match` array.
2. If two matching rules have equal specificity, choose the safest action:

```text
deny > gate > allow
```

If no rules match, use `[default].action`, which should be `gate` in the generated config.

## 8. Command Parsing

The wrapper should classify commands using argv tokens, not raw string prefix matching.

Important cases:

```bash
gh pr list
```

Command path:

```text
["pr", "list"]
```

```bash
gh --repo owner/repo pr list
```

Command path:

```text
["pr", "list"]
```

```bash
gh -R owner/repo pr view 123
```

Command path:

```text
["pr", "view"]
```

```bash
gh --version
```

Command path:

```text
["--version"]
```

```bash
gh api repos/owner/repo
```

Command path:

```text
["api"]
```

For v1, global flags may be ignored for policy classification, except standalone commands like `--version` and `--help`.

The parser should know common global flags that take values:

- `--repo`, `-R`
- `--hostname`
- `--config-dir`
- `--help`, `-h`

Unknown leading flags should be handled conservatively. If command extraction is ambiguous, gate.

## 9. Gating Flow

For gated commands, the wrapper performs a fresh sudo authorization challenge internally.

The flow is:

```text
policy decision = gate
  ↓
log gated_attempt
  ↓
run: sudo -k
  ↓
run: sudo -v
  ↓
if sudo succeeds:
    log gate_authorized
    exec real gh by absolute path as current user
else:
    log gate_denied
    exit non-zero
```

The real `gh` must not be run through sudo.

Correct:

```text
sudo -k
sudo -v
exec /home/user/.local/share/gh-guard/bin/gh-real ...
```

Incorrect:

```text
sudo gh ...
sudo /usr/bin/env gh ...
sudo sh -c 'gh ...'
sudo /home/user/.local/share/gh-guard/bin/gh-real ...
```

The final incorrect example avoids recursion, but it runs the real GitHub CLI as root, which can change config/auth behavior and create root-owned files. Do not do this for the main gated path.

## 10. Executing the Real GitHub CLI

On Unix, use `std::os::unix::process::CommandExt::exec` so the wrapper process is replaced by the real GitHub CLI.

The exec function should require an absolute path.

Pseudo-code:

```rust
fn exec_real_gh(real_gh: &Path, args: &[String]) -> anyhow::Result<()> {
    if !real_gh.is_absolute() {
        anyhow::bail!("real gh path must be absolute");
    }

    let err = std::process::Command::new(real_gh)
        .args(args)
        .exec();

    Err(err.into())
}
```

Never delegate through PATH lookup.

## 11. Real GitHub CLI Management

The wrapper automatically downloads and updates the official GitHub CLI.

### Version Strategy

- If `updates.pinned_version` is non-empty, install and use that version.
- If unpinned, use the latest GitHub CLI release.
- Auto-update is enabled by default.
- Update checks should be throttled by `updates.check_interval`, default `24h`.
- If network access is unavailable but a cached real `gh` exists, continue using the cached version and warn/log.
- If network access is unavailable and no cached real `gh` exists, fail closed with a clear error.

### Download Source

Default release repository:

```text
github.com/cli/cli
```

Download examples:

```text
https://github.com/cli/cli/releases/download/v2.92.0/gh_2.92.0_macOS_arm64.zip
https://github.com/cli/cli/releases/download/v2.92.0/gh_2.92.0_linux_amd64.tar.gz
```

The asset name format differs by platform and version. Implement robust asset selection by querying release assets and matching OS/architecture rather than hardcoding a single URL.

### Platform Mapping

Rust target information should map to GitHub CLI asset naming roughly as follows:

```text
macos + aarch64  -> macOS_arm64.zip
macos + x86_64   -> macOS_amd64.zip
linux + aarch64  -> linux_arm64.tar.gz
linux + x86_64   -> linux_amd64.tar.gz
```

### Checksum Verification

Verify downloaded archives before extraction.

Use release-provided checksums or digests when available. Store verification metadata in the installed version directory.

Metadata example:

```json
{
  "version": "v2.93.0",
  "asset_name": "gh_2.93.0_linux_amd64.tar.gz",
  "download_url": "https://github.com/cli/cli/releases/download/v2.93.0/gh_2.93.0_linux_amd64.tar.gz",
  "sha256": "...",
  "installed_at": "2026-05-27T12:00:00Z"
}
```

### Atomic Installation

Install downloads atomically:

1. Download archive to cache.
2. Verify checksum.
3. Extract into a temporary directory under the data directory.
4. Locate the real `gh` executable inside the extracted archive.
5. Move the prepared version directory into `versions/vX.Y.Z`.
6. Update `bin/gh-real` symlink atomically.

Avoid leaving partially installed versions active.

## 12. GitHub Enterprise Server Support

The wrapper should be seamless with GHES.

For v1:

- Do not hardcode `github.com` assumptions into command classification.
- Preserve the user environment when execing real `gh`.
- Allow normal `gh` GHES usage, including:

```bash
gh auth login --hostname github.company.com
gh repo view HOST/ORG/REPO
GH_HOST=github.company.com gh repo view ORG/REPO
```

The managed real GitHub CLI can still be downloaded from `cli/cli` releases unless a future config option changes this.

Redact GHES-related token environment variables:

- `GH_TOKEN`
- `GITHUB_TOKEN`
- `GH_ENTERPRISE_TOKEN`
- `GITHUB_ENTERPRISE_TOKEN`

Future policy granularity may add host-aware rules:

```toml
[[rules]]
match = ["api"]
hosts = ["github.company.com"]
methods = ["GET"]
action = "allow"
```

## 13. Logging

### Local Logging

Always write local JSON Lines logs.

Default path:

```text
~/.local/share/gh-guard/logs/gh-guard.jsonl
```

The log file should be created with restrictive permissions, ideally `0600`.

### Events

Log at least these event types:

- `invocation`
- `config_created`
- `config_loaded`
- `policy_decision`
- `gated_attempt`
- `gate_authorized`
- `gate_denied`
- `deny`
- `exec_started`
- `exec_failed`
- `download_started`
- `download_verified`
- `download_failed`
- `update_check_started`
- `update_check_skipped`
- `update_check_failed`
- `update_installed`
- `using_cached_gh`
- `error`

### Fields

Include these fields where applicable:

- timestamp, RFC3339
- event
- argv, redacted
- command_path
- policy decision
- matched rule
- real gh path
- real gh version
- duration
- exit code, if available
- username
- hostname
- pid
- cwd
- config path

Example:

```json
{
  "timestamp": "2026-05-27T12:00:00Z",
  "event": "policy_decision",
  "argv": ["pr", "create", "--title", "hello", "--body", "[REDACTED]"],
  "command_path": ["pr", "create"],
  "decision": "gate",
  "matched_rule": null,
  "default_action": "gate",
  "pid": 12345,
  "cwd": "/home/alice/project"
}
```

### Redaction

Redaction is enabled by default.

Redact values for sensitive flags, including:

- `--body`
- `--body-file`
- `--notes`
- `--notes-file`
- `--field`
- `-F`
- `--raw-field`
- `-f`
- `--header`
- `-H`
- `--jq`
- `--template`

Also redact obvious token-like values and known secret environment variables.

Do not log full environment variables by default.

### OpenTelemetry

Optional remote logging/tracing can be added when `logging.otel_endpoint` is set. This should not block local execution if unavailable unless explicitly configured in the future.

## 14. Wrapper-Reserved Commands

Reserve the `guard` subcommand namespace.

These commands are handled by the wrapper and never passed through to the real GitHub CLI:

```bash
gh guard status
gh guard doctor
gh guard update
gh guard version
gh guard real-path
gh guard config path
gh guard config init
gh guard config print
gh guard logs path
```

Suggested behavior:

### `gh guard status`

Print:

- config path
- data path
- cache path
- real gh path
- real gh version
- update status
- default policy action
- number of policy rules

### `gh guard doctor`

Check:

- config parse/validation
- data directory permissions
- log file permissions
- real gh exists and is executable
- symlink points to an installed version
- broad dangerous allow rules
- network access to release source, if possible

Warn on broad allow rules such as:

```toml
match = ["api"]
action = "allow"
```

```toml
match = ["auth", "token"]
action = "allow"
```

```toml
match = ["repo"]
action = "allow"
```

### `gh guard update`

Force an update check and install the selected version.

### `gh guard real-path`

Print the absolute path to the managed real GitHub CLI.

## 15. Installation and Packaging

Support these installation methods:

- Cargo
- Homebrew
- Nix flake, compatible with NixOS and nix-darwin

The installed executable should be named `gh`.

For development, the Rust package may be named `gh-guard`, but packaging should install/copy/symlink the final binary as `gh`.

### Cargo

Possible approach:

```bash
cargo install --path .
```

Then manually link or copy the produced `gh-guard` binary to a location earlier in PATH as `gh`.

Alternatively, configure the binary target name as `gh` in `Cargo.toml`:

```toml
[[bin]]
name = "gh"
path = "src/main.rs"
```

### Homebrew

The formula should install the wrapper as `gh`. Because this intentionally conflicts with the official GitHub CLI formula, document the conflict clearly.

### Nix

The flake should expose packages for both Linux and Darwin.

Suggested outputs:

```text
packages.${system}.default
packages.${system}.gh-guard
apps.${system}.default
devShells.${system}.default
```

The package should install a binary named `gh`.

Because the wrapper downloads the real GitHub CLI into user data at runtime, the Nix package itself does not need to contain the official `gh` binary for v1.

## 16. Suggested Dependencies

Likely Rust crates:

- `anyhow` or `thiserror`
- `serde`
- `serde_json`
- `toml`
- `reqwest`, blocking or async
- `sha2`
- `hex`
- `zip`
- `tar`
- `flate2`
- `tracing`
- `tracing-subscriber`
- `time` or `chrono`
- `hostname`
- `whoami`
- `tempfile`
- `fs2`, for update/install locking

Use `std::os::unix::process::CommandExt` for exec on supported platforms.

## 17. Concurrency and Locking

Multiple `gh` invocations may happen at the same time.

Use a lock file for operations that mutate shared state:

- first-run download
- update check state writes
- version installation
- symlink updates

Suggested lock path:

```text
~/.local/share/gh-guard/state.lock
```

Non-mutating policy decisions and normal exec should not hold the lock longer than necessary.

## 18. Failure Behavior

### Missing Config

Create default config and continue.

### Invalid Config

Fail closed. Do not run the real GitHub CLI.

### No Cached Real GitHub CLI and Network Fails

Fail closed with a clear error explaining that the real GitHub CLI could not be installed.

### Cached Real GitHub CLI Exists and Update Check Fails

Warn, log `update_check_failed`, log `using_cached_gh`, and continue.

### Sudo Authorization Fails

Log `gate_denied` and exit non-zero.

### Command Parsing Ambiguous

Gate.

## 19. Testing Checklist

### Policy

- Allowed command runs without sudo.
- Unknown command is gated.
- `auth token` is gated by default.
- `api` is gated by default.
- Most-specific rule wins.
- Equal-specificity tie uses `deny > gate > allow`.
- Ambiguous parse gates.

### Gating

- Gated command runs `sudo -k` and `sudo -v`.
- Gated command does not run `sudo gh`.
- Gated command does not run real `gh` as root.
- Failed sudo authorization prevents exec.

### Exec

- Real binary is executed by absolute path.
- PATH lookup is never used for real `gh`.
- Args are preserved exactly when passed to real `gh`.
- Environment is preserved for GHES compatibility.

### Download and Update

- First run downloads correct platform asset.
- Checksum verification succeeds for valid asset.
- Checksum mismatch fails closed.
- Pinned version is respected.
- Auto-update installs newer version when available.
- Network failure uses cached version if present.

### Logging

- Logs are JSON Lines.
- Sensitive args are redacted.
- Gated attempts and authorization results are both logged.
- Log file permissions are restrictive.

### Wrapper Commands

- `gh guard status` works without real `gh` if possible.
- `gh guard update` forces update.
- `gh guard real-path` prints absolute managed path.
- `gh guard ...` never passes through to real `gh`.

## 20. Implementation Milestones

Recommended order:

1. Create Rust project and binary target named `gh`.
2. Implement XDG/Linux-style path resolution.
3. Implement config structs, default config creation, and validation.
4. Implement command-path extraction.
5. Implement policy rule matching.
6. Implement `gh guard status` and `gh guard config path`.
7. Implement absolute-path exec using a temporary configured real binary path.
8. Implement sudo gate flow.
9. Implement JSONL logging with redaction.
10. Implement real GitHub CLI download for one platform.
11. Generalize download to all target platforms.
12. Implement checksum verification.
13. Implement versioned installs and `gh-real` symlink.
14. Implement auto-update interval logic.
15. Add GHES pass-through/redaction tests.
16. Add Homebrew packaging.
17. Add Nix flake.
18. Expand documentation and examples.

## 21. Security Notes

This wrapper reduces risk from unauthorized or accidental use of the GitHub CLI, especially mutating commands and direct token access through `gh auth token`.

It does not fully solve all local token-exfiltration risks. If a malicious process can read token-bearing environment variables or directly access the GitHub CLI credential store, it may bypass this wrapper. The wrapper is best understood as a strong local policy gate around normal `gh` invocation, not as a complete endpoint security system.

Do not store GitHub tokens in the wrapper.

Do not print tokens.

Do not log raw secrets.

Do not add an environment-variable bypass.

## 22. Final v1 Behavior Summary

Default v1 behavior should be:

```text
gh pr list
  -> allowed
  -> exec real gh as user

 gh pr create
  -> gated
  -> sudo -k
  -> sudo -v
  -> exec real gh as user

 gh auth token
  -> gated
  -> sudo -k
  -> sudo -v
  -> exec real gh as user

 gh api ...
  -> gated
  -> sudo -k
  -> sudo -v
  -> exec real gh as user

 gh guard status
  -> handled by wrapper
  -> never passed through
```

The wrapper is named `gh`, but the real GitHub CLI is always invoked by absolute managed path, never by name.
