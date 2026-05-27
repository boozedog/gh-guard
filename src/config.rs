use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub default: DefaultSection,
    pub updates: UpdatesSection,
    pub logging: LoggingSection,
    pub real_gh: RealGhSection,
    pub gate: GateSection,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DefaultSection {
    pub action: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UpdatesSection {
    pub auto_update: bool,
    #[serde(default = "default_check_interval")]
    pub check_interval: String,
    #[serde(default)]
    pub pinned_version: String,
}

fn default_check_interval() -> String {
    "24h".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoggingSection {
    pub level: String,
    pub redact: bool,
    #[serde(default)]
    pub otel_endpoint: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RealGhSection {
    pub download_source: String,
    pub release_repo: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GateSection {
    pub method: String,
    pub fresh: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Rule {
    #[serde(rename = "match")]
    pub match_vec: Vec<String>,
    pub action: String,
}

impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        let valid_actions = ["allow", "gate", "deny"];
        if !valid_actions.contains(&self.default.action.as_str()) {
            anyhow::bail!("invalid default action: {}", self.default.action);
        }
        if self.gate.method != "sudo" {
            anyhow::bail!("unsupported gate method: {}", self.gate.method);
        }
        if self.real_gh.download_source != "github" {
            anyhow::bail!(
                "unsupported download source: {}",
                self.real_gh.download_source
            );
        }
        if !self.real_gh.release_repo.contains('/') {
            anyhow::bail!("invalid release repo format: {}", self.real_gh.release_repo);
        }
        for (i, rule) in self.rules.iter().enumerate() {
            if !valid_actions.contains(&rule.action.as_str()) {
                anyhow::bail!("invalid action for rule {}: {}", i, rule.action);
            }
            if rule.match_vec.is_empty() {
                anyhow::bail!("rule {} has empty match array (would match everything)", i);
            }
        }
        Ok(())
    }
}

pub fn default_config() -> Config {
    Config {
        default: DefaultSection {
            action: "gate".to_string(),
        },
        updates: UpdatesSection {
            auto_update: true,
            check_interval: "24h".to_string(),
            pinned_version: "".to_string(),
        },
        logging: LoggingSection {
            level: "info".to_string(),
            redact: true,
            otel_endpoint: "".to_string(),
        },
        real_gh: RealGhSection {
            download_source: "github".to_string(),
            release_repo: "cli/cli".to_string(),
        },
        gate: GateSection {
            method: "sudo".to_string(),
            fresh: true,
        },
        rules: vec![
            Rule {
                match_vec: vec!["--version".to_string()],
                action: "allow".to_string(),
            },
            Rule {
                match_vec: vec!["--help".to_string()],
                action: "allow".to_string(),
            },
            Rule {
                match_vec: vec!["help".to_string()],
                action: "allow".to_string(),
            },
            Rule {
                match_vec: vec!["completion".to_string()],
                action: "allow".to_string(),
            },
        ],
    }
}

pub fn load_or_create() -> anyhow::Result<Config> {
    let path = crate::paths::config_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| crate::error::GuardError::Config(format!("reading config: {e}")))?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| crate::error::GuardError::Config(format!("parsing config: {e}")))?;
        config.validate()?;
        return Ok(config);
    }

    let config = default_config();
    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir)?;
    let toml_str = toml::to_string_pretty(&config)
        .map_err(|e| crate::error::GuardError::Config(format!("serializing config: {e}")))?;
    std::fs::write(&path, &toml_str)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }

    Ok(config)
}
