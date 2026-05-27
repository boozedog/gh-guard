pub fn is_guard_command(args: &[String]) -> bool {
    !args.is_empty() && args[0] == "guard"
}

pub fn run(args: &[String], config: &crate::config::Config) -> anyhow::Result<()> {
    let sub = args.get(1).map(|s| s.as_str());
    match sub {
        Some("status") => status(config),
        Some("doctor") => doctor(config),
        Some("update") => update(config),
        Some("version") => version(),
        Some("real-path") => real_path(),
        Some("config") => config_cmd(&args[2..]),
        Some("logs") => logs_cmd(&args[2..]),
        _ => {
            anyhow::bail!("unknown guard command. Usage: gh guard [status|doctor|update|version|real-path|config|logs]")
        }
    }
}

fn status(config: &crate::config::Config) -> anyhow::Result<()> {
    let real_gh = crate::paths::real_gh_symlink();
    let real_gh_target = std::fs::read_link(&real_gh).ok();
    let real_gh_version = real_gh_target.as_ref().and_then(|t| {
        let meta = t.parent()?.join("metadata.json");
        let content = std::fs::read_to_string(meta).ok()?;
        let meta: crate::metadata::Metadata = serde_json::from_str(&content).ok()?;
        Some(meta.version)
    });

    println!("config path:  {}", crate::paths::config_path().display());
    println!("data path:    {}", crate::paths::data_dir().display());
    println!("cache path:   {}", crate::paths::cache_dir().display());
    println!("real gh path: {}", real_gh.display());
    if let Some(target) = real_gh_target {
        println!("real gh target: {}", target.display());
    }
    if let Some(ver) = real_gh_version {
        println!("real gh version: {}", ver);
    }
    println!("default policy: {}", config.default.action);
    println!("policy rules: {}", config.rules.len());

    Ok(())
}

fn doctor(config: &crate::config::Config) -> anyhow::Result<()> {
    println!("Running doctor checks...");

    if let Err(e) = config.validate() {
        println!("❌ config validation failed: {}", e);
    } else {
        println!("✅ config valid");
    }

    let data_dir = crate::paths::data_dir();
    if data_dir.exists() {
        println!("✅ data directory exists: {}", data_dir.display());
    } else {
        println!("❌ data directory missing: {}", data_dir.display());
    }

    let symlink = crate::paths::real_gh_symlink();
    if symlink.exists() {
        let target = std::fs::read_link(&symlink)?;
        if target.exists() {
            println!("✅ real gh exists: {}", target.display());
        } else {
            println!("❌ real gh symlink target missing: {}", target.display());
        }
    } else {
        println!("❌ real gh symlink missing: {}", symlink.display());
    }

    for rule in &config.rules {
        if rule.match_vec.len() == 1 && rule.action == "allow" {
            let cmd = &rule.match_vec[0];
            if ["api", "repo", "auth", "pr", "issue"].contains(&cmd.as_str()) {
                println!(
                    "⚠️  broad allow rule detected: {:?} -> allow",
                    rule.match_vec
                );
            }
        }
    }

    match crate::download::get_latest_release(&config.real_gh.release_repo) {
        Ok(_) => println!("✅ can reach GitHub releases"),
        Err(e) => println!("⚠️  cannot reach GitHub releases: {}", e),
    }

    Ok(())
}

fn update(config: &crate::config::Config) -> anyhow::Result<()> {
    println!("Checking for updates...");
    crate::gh_manager::update(config)?;
    println!("Update complete.");
    Ok(())
}

fn version() -> anyhow::Result<()> {
    println!("gh-guard {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}

fn real_path() -> anyhow::Result<()> {
    let path = crate::paths::real_gh_symlink();
    if path.exists() {
        let target = std::fs::read_link(&path)?;
        println!("{}", target.display());
    } else {
        anyhow::bail!("real gh not installed");
    }
    Ok(())
}

fn config_cmd(args: &[String]) -> anyhow::Result<()> {
    let sub = args.first().map(|s| s.as_str());
    match sub {
        Some("path") => {
            println!("{}", crate::paths::config_path().display());
        }
        Some("init") => {
            let _ = crate::config::load_or_create()?;
            println!(
                "Config initialized at {}",
                crate::paths::config_path().display()
            );
        }
        Some("print") => {
            let config = crate::config::load_or_create()?;
            println!("{}", toml::to_string_pretty(&config)?);
        }
        _ => {
            anyhow::bail!("unknown config command. Usage: gh guard config [path|init|print]")
        }
    }
    Ok(())
}

fn logs_cmd(args: &[String]) -> anyhow::Result<()> {
    let sub = args.first().map(|s| s.as_str());
    match sub {
        Some("path") => {
            println!("{}", crate::paths::logs_dir().display());
        }
        _ => {
            anyhow::bail!("unknown logs command. Usage: gh guard logs [path]")
        }
    }
    Ok(())
}
