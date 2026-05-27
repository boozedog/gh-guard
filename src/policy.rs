use crate::config::{Config, Rule};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Allow,
    Gate,
    Deny,
}

pub fn decide(config: &Config, command_path: &[String]) -> (Action, Option<String>) {
    let matches: Vec<&Rule> = config
        .rules
        .iter()
        .filter(|r| is_prefix(&r.match_vec, command_path))
        .collect();

    if matches.is_empty() {
        let action = parse_action(&config.default.action);
        return (action, None);
    }

    let max_len = matches.iter().map(|r| r.match_vec.len()).max().unwrap();
    let best: Vec<&Rule> = matches
        .into_iter()
        .filter(|r| r.match_vec.len() == max_len)
        .collect();

    if best.len() == 1 {
        let action = parse_action(&best[0].action);
        let rule_desc = format!("{:?}", best[0].match_vec);
        return (action, Some(rule_desc));
    }

    // Tie-break: deny > gate > allow
    let action = best
        .iter()
        .map(|r| parse_action(&r.action))
        .max_by_key(|a| match a {
            Action::Deny => 3,
            Action::Gate => 2,
            Action::Allow => 1,
        })
        .unwrap_or_else(|| parse_action(&config.default.action));

    let rule_desc = format!("{:?}", best[0].match_vec);
    (action, Some(rule_desc))
}

fn is_prefix(rule: &[String], path: &[String]) -> bool {
    if rule.len() > path.len() {
        return false;
    }
    rule.iter().zip(path.iter()).all(|(a, b)| a == b)
}

fn parse_action(s: &str) -> Action {
    match s {
        "allow" => Action::Allow,
        "gate" => Action::Gate,
        "deny" => Action::Deny,
        _ => Action::Gate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        Config, DefaultSection, GateSection, LoggingSection, RealGhSection, Rule, UpdatesSection,
    };

    fn make_config(rules: Vec<Rule>) -> Config {
        Config {
            default: DefaultSection {
                action: "gate".to_string(),
            },
            updates: UpdatesSection {
                auto_update: false,
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
            rules,
        }
    }

    #[test]
    fn test_allow_exact() {
        let config = make_config(vec![Rule {
            match_vec: vec!["pr".to_string(), "list".to_string()],
            action: "allow".to_string(),
        }]);
        let (action, _) = decide(&config, &["pr".to_string(), "list".to_string()]);
        assert_eq!(action, Action::Allow);
    }

    #[test]
    fn test_allow_prefix() {
        let config = make_config(vec![Rule {
            match_vec: vec!["pr".to_string(), "list".to_string()],
            action: "allow".to_string(),
        }]);
        let (action, _) = decide(
            &config,
            &["pr".to_string(), "list".to_string(), "123".to_string()],
        );
        assert_eq!(action, Action::Allow);
    }

    #[test]
    fn test_gate_default() {
        let config = make_config(vec![]);
        let (action, _) = decide(&config, &["issue".to_string(), "create".to_string()]);
        assert_eq!(action, Action::Gate);
    }

    #[test]
    fn test_specific_over_general() {
        let config = make_config(vec![
            Rule {
                match_vec: vec!["pr".to_string()],
                action: "gate".to_string(),
            },
            Rule {
                match_vec: vec!["pr".to_string(), "view".to_string()],
                action: "allow".to_string(),
            },
        ]);
        let (action, _) = decide(&config, &["pr".to_string(), "view".to_string()]);
        assert_eq!(action, Action::Allow);
    }

    #[test]
    fn test_tie_break_safest() {
        let config = make_config(vec![
            Rule {
                match_vec: vec!["repo".to_string(), "view".to_string()],
                action: "allow".to_string(),
            },
            Rule {
                match_vec: vec!["repo".to_string(), "view".to_string()],
                action: "gate".to_string(),
            },
        ]);
        let (action, _) = decide(&config, &["repo".to_string(), "view".to_string()]);
        assert_eq!(action, Action::Gate);
    }

    #[test]
    fn test_deny() {
        let config = make_config(vec![Rule {
            match_vec: vec!["api".to_string()],
            action: "deny".to_string(),
        }]);
        let (action, _) = decide(&config, &["api".to_string()]);
        assert_eq!(action, Action::Deny);
    }

    #[test]
    fn test_default_config_gates_auth_status() {
        let config = crate::config::default_config();
        let (action, _) = decide(&config, &["auth".to_string(), "status".to_string()]);
        assert_eq!(
            action,
            Action::Gate,
            "auth status should be gated by default"
        );
    }

    #[test]
    fn test_default_config_allows_version() {
        let config = crate::config::default_config();
        let (action, _) = decide(&config, &["--version".to_string()]);
        assert_eq!(
            action,
            Action::Allow,
            "--version should be allowed by default"
        );
    }
}
