/// Sensitive content filter — blocks prompts containing credential-like tokens.
/// Command existence validation — checks whether the first word of a suggested
/// command is a known executable on the current PATH.

const SENSITIVE_PATTERNS: &[&str] = &[
    "password", "token", "secret", "bearer", "api-key", "api_key",
];

pub fn is_sensitive(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    SENSITIVE_PATTERNS.iter().any(|pat| lower.contains(pat))
}

/// Returns the first whitespace-separated word of `command`.
pub fn command_name(command: &str) -> &str {
    command.split_whitespace().next().unwrap_or(command)
}

/// Returns true if `name` exists on PATH or is a known shell built-in.
pub fn command_exists(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    // Shell built-ins that won't appear as files on PATH
    const BUILTINS: &[&str] = &[
        "cd", "echo", "exit", "export", "source", "alias", "unalias",
        "set", "unset", "read", "exec", "eval", "true", "false",
        "pwd", "type", "which", "history", "jobs", "fg", "bg",
        "kill", "wait", "break", "continue", "return", "shift",
    ];
    if BUILTINS.contains(&name) {
        return true;
    }

    // Check PATH via `which`
    std::process::Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
