const SENSITIVE_LONG_FLAGS: &[&str] = &[
    "--body",
    "--body-file",
    "--notes",
    "--notes-file",
    "--field",
    "--raw-field",
    "--header",
    "--jq",
    "--template",
];

const SENSITIVE_SHORT_FLAGS: &[char] = &['F', 'f', 'H'];

const SENSITIVE_FLAGS: &[&str] = &[
    "--body",
    "--body-file",
    "--notes",
    "--notes-file",
    "--field",
    "-F",
    "--raw-field",
    "-f",
    "--header",
    "-H",
    "--jq",
    "--template",
];

pub fn redact_argv(argv: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut i = 0;

    while i < argv.len() {
        let arg = &argv[i];

        // Exact match (separate value form): --body value, -F value
        if SENSITIVE_FLAGS.contains(&arg.as_str()) {
            result.push(arg.clone());
            i += 1;
            if i < argv.len() {
                result.push("[REDACTED]".to_string());
                i += 1;
            }
            continue;
        }

        // Inline long form: --body=secret, --field=foo
        let mut redacted = false;
        for flag in SENSITIVE_LONG_FLAGS {
            let prefix = format!("{}=", flag);
            if arg.starts_with(&prefix) {
                result.push(format!("{}=[REDACTED]", flag));
                redacted = true;
                break;
            }
        }
        if redacted {
            i += 1;
            continue;
        }

        // Inline short form: -Fvalue, -fkey=val, -HAuthorization:...
        if arg.len() > 2 && arg.starts_with('-') && !arg.starts_with("--") {
            let first_char = arg.chars().nth(1).unwrap();
            if SENSITIVE_SHORT_FLAGS.contains(&first_char) {
                result.push(format!("-{}[REDACTED]", first_char));
                i += 1;
                continue;
            }
        }

        result.push(arg.clone());
        i += 1;
    }

    result
}
