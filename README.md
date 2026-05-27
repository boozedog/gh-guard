# gh-guard

A security wrapper around the [GitHub CLI (`gh`)](https://cli.github.com).
`gh-guard` installs as `gh`, intercepts every invocation, applies a configurable
policy, and only delegates to the real `gh` binary when approved.

## Threat model

`gh-guard` assumes an attacker may have:

- Your shell / terminal access (can run arbitrary `gh` commands)
- Your GitHub token (already in `gh`'s keychain or config)

`gh-guard` does **not** protect against:

- Someone with root access to your machine
- Someone who can edit `~/.config/gh-guard/config.toml`
- Someone who replaces the wrapper binary or the real `gh` binary directly

`gh-guard` is a **policy wrapper**, not complete endpoint security.

## What it does

1. **Intercepts every `gh` invocation**
2. **Parses the command** to determine what subcommand is being run
3. **Applies policy rules** from `~/.config/gh-guard/config.toml`
4. **For gated commands**: challenges with `sudo` (password prompt) before
   allowing execution
5. **For denied commands**: rejects immediately
6. **For allowed commands**: delegates to the real `gh` binary without prompt
7. **Downloads and manages** the real `gh` binary automatically

## Default policy

By default, only read-only safe commands are allowed without gating:

- `--version`
- `--help`
- `help`
- `completion`

Everything else is **gated** (requires sudo password). You can customize rules in
`config.toml`.

## Install

### From source

```bash
git clone https://github.com/yourname/gh-guard
cd gh-guard
cargo build --release
cp target/release/gh /usr/local/bin/gh   # or ~/bin/gh
```

Make sure `/usr/local/bin` (or wherever you install) comes **before** the real
`gh` in your `PATH`.

### Nix

```bash
nix build .#default
```

### Homebrew (when published)

```bash
brew install gh-guard
```

## Configuration

`gh-guard` reads `~/.config/gh-guard/config.toml`.

An example config is shipped at `config/example.toml`:

```bash
mkdir -p ~/.config/gh-guard
cp config/example.toml ~/.config/gh-guard/config.toml
```

Minimal example:

```toml
[default]
action = "gate"

[updates]
auto_update = true
check_interval = "24h"
pinned_version = ""

[real_gh]
download_source = "github"
release_repo = "cli/cli"

[gate]
method = "sudo"
fresh = true

[[rules]]
match = ["pr", "list"]
action = "allow"

[[rules]]
match = ["api"]
action = "deny"
```

### Rules

Rules are evaluated in order of specificity. Longer `match` arrays take
precedence. On ties, the safest action wins: `deny` > `gate` > `allow`.

## Guard commands

`gh-guard` provides its own subcommands under `gh guard`:

- `gh guard status` — show current config and real gh path
- `gh guard doctor` — validate setup and check connectivity
- `gh guard version` — show gh-guard version
- `gh guard config print` — print current config
- `gh guard update` — force update check

## Logging

All policy decisions, gate attempts, and exec events are logged to:
`~/.local/share/gh-guard/logs/gh-guard-YYYY-MM-DD.jsonl`

Sensitive flags (`--body`, `-H`, `-f`, `-F`) are redacted by default.

## Security invariants

- Real `gh` is always invoked by **absolute path** (never via PATH search)
- Gated commands use `sudo -k` → `sudo -v` → `sudo -k` (credentials are
  invalidated immediately)
- The wrapper refuses to run as root
- If a pinned version is set and cannot be installed, the wrapper **fails closed**
- Recursion is detected by comparing inode/device of the wrapper and real gh
- Archive extraction only writes the expected `gh` binary and sanitizes all
  paths to prevent traversal

## License

MIT
