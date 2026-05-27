pub fn extract_command_path(args: &[String]) -> Option<Vec<String>> {
    let mut path = Vec::new();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];

        if arg == "--version" {
            return Some(vec!["--version".to_string()]);
        }
        if arg == "--help" || arg == "-h" {
            return Some(vec!["--help".to_string()]);
        }

        if arg.starts_with('-') {
            // Known value-taking global flags
            if arg == "--repo" || arg == "--hostname" || arg == "--config-dir" {
                i += 1;
                if i < args.len() {
                    i += 1;
                }
                continue;
            }
            if arg == "-R" {
                i += 1;
                if i < args.len() {
                    i += 1;
                }
                continue;
            }
            if arg.starts_with("-R") && arg.len() > 2 {
                // -Rvalue form
                i += 1;
                continue;
            }
            // Unknown flag: skip just the flag itself.
            // If it takes a value, the value will be collected into the path,
            // which makes matching more conservative (usually a gate).
            i += 1;
            continue;
        }

        path.push(arg.clone());
        i += 1;
    }

    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let args = vec!["--version".to_string()];
        assert_eq!(
            extract_command_path(&args),
            Some(vec!["--version".to_string()])
        );
    }

    #[test]
    fn test_pr_list() {
        let args = vec!["pr".to_string(), "list".to_string()];
        assert_eq!(
            extract_command_path(&args),
            Some(vec!["pr".to_string(), "list".to_string()])
        );
    }

    #[test]
    fn test_with_repo_flag() {
        let args = vec![
            "--repo".to_string(),
            "owner/repo".to_string(),
            "pr".to_string(),
            "list".to_string(),
        ];
        assert_eq!(
            extract_command_path(&args),
            Some(vec!["pr".to_string(), "list".to_string()])
        );
    }

    #[test]
    fn test_with_short_repo_flag() {
        let args = vec![
            "-R".to_string(),
            "owner/repo".to_string(),
            "pr".to_string(),
            "view".to_string(),
        ];
        assert_eq!(
            extract_command_path(&args),
            Some(vec!["pr".to_string(), "view".to_string()])
        );
    }

    #[test]
    fn test_with_combined_short_repo_flag() {
        let args = vec![
            "-Rowner/repo".to_string(),
            "pr".to_string(),
            "list".to_string(),
        ];
        assert_eq!(
            extract_command_path(&args),
            Some(vec!["pr".to_string(), "list".to_string()])
        );
    }

    #[test]
    fn test_empty() {
        let args: Vec<String> = vec![];
        assert_eq!(extract_command_path(&args), None);
    }

    #[test]
    fn test_flags_only() {
        let args = vec!["--repo".to_string(), "foo".to_string()];
        assert_eq!(extract_command_path(&args), None);
    }
}
