use gh_guard::{command, config, exec, gate, gh_manager, guard_commands, logging, policy, redact};
use std::env;

fn main() {
    if let Err(e) = run() {
        eprintln!("gh-guard: {}", e);
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    gh_guard::paths::ensure_dirs()?;
    logging::init_global_logger()?;

    let config = config::load_or_create()?;
    logging::global_log(
        "config_loaded",
        std::collections::HashMap::from([(
            "config_path".to_string(),
            serde_json::Value::String(gh_guard::paths::config_path().to_string_lossy().into()),
        )]),
    )?;

    if guard_commands::is_guard_command(&args) {
        return guard_commands::run(&args, &config);
    }

    let redacted_argv = if config.logging.redact {
        redact::redact_argv(&args)
    } else {
        args.clone()
    };
    logging::global_log(
        "invocation",
        std::collections::HashMap::from([(
            "argv".to_string(),
            serde_json::Value::Array(
                redacted_argv
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        )]),
    )?;

    let command_path = match command::extract_command_path(&args) {
        Some(p) => p,
        None => {
            logging::global_log(
                "policy_decision",
                std::collections::HashMap::from([
                    (
                        "decision".to_string(),
                        serde_json::Value::String("gate".to_string()),
                    ),
                    (
                        "reason".to_string(),
                        serde_json::Value::String("no_command_path".to_string()),
                    ),
                ]),
            )?;
            if let Err(e) = gate::challenge() {
                let _ = logging::global_log("gate_denied", std::collections::HashMap::new());
                return Err(e);
            }
            logging::global_log("gate_authorized", std::collections::HashMap::new())?;
            let real_gh = gh_manager::ensure_real_gh(&config)?;
            return exec::exec_real(&real_gh, &args);
        }
    };

    let (action, matched_rule) = policy::decide(&config, &command_path);
    let mut decision_fields = std::collections::HashMap::from([
        (
            "command_path".to_string(),
            serde_json::Value::Array(
                command_path
                    .iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ),
        ),
        (
            "decision".to_string(),
            serde_json::Value::String(format!("{:?}", action).to_lowercase()),
        ),
        (
            "default_action".to_string(),
            serde_json::Value::String(config.default.action.clone()),
        ),
    ]);
    if let Some(rule) = matched_rule {
        decision_fields.insert("matched_rule".to_string(), serde_json::Value::String(rule));
    }
    logging::global_log("policy_decision", decision_fields)?;

    match action {
        policy::Action::Allow => {
            let real_gh = gh_manager::ensure_real_gh(&config)?;
            logging::global_log(
                "exec_started",
                std::collections::HashMap::from([(
                    "real_gh_path".to_string(),
                    serde_json::Value::String(real_gh.to_string_lossy().into()),
                )]),
            )?;
            exec::exec_real(&real_gh, &args)?;
        }
        policy::Action::Gate => {
            logging::global_log("gated_attempt", std::collections::HashMap::new())?;
            if let Err(e) = gate::challenge() {
                let _ = logging::global_log("gate_denied", std::collections::HashMap::new());
                return Err(e);
            }
            logging::global_log("gate_authorized", std::collections::HashMap::new())?;
            let real_gh = gh_manager::ensure_real_gh(&config)?;
            logging::global_log(
                "exec_started",
                std::collections::HashMap::from([(
                    "real_gh_path".to_string(),
                    serde_json::Value::String(real_gh.to_string_lossy().into()),
                )]),
            )?;
            exec::exec_real(&real_gh, &args)?;
        }
        policy::Action::Deny => {
            logging::global_log("deny", std::collections::HashMap::new())?;
            anyhow::bail!(gh_guard::error::GuardError::Denied);
        }
    }

    Ok(())
}
