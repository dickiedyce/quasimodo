use crate::bank::BankEntry;

pub struct PromptBuilder {
    examples: Vec<BankEntry>,
}

impl PromptBuilder {
    pub fn new(examples: Vec<BankEntry>) -> Self {
        Self { examples }
    }

    pub fn system_prompt(&self) -> String {
        let mut parts = vec![
            "You are a shell command assistant for macOS. \
             Output only the exact shell command, nothing else. \
             Do not include explanations, markdown, or backticks.\n\
             Here are examples of correct shell commands:\n"
                .to_string(),
        ];

        for entry in &self.examples {
            parts.push(format!("Q: {}\nA: {}\n", entry.description, entry.command));
        }

        parts.join("\n")
    }
}

pub fn strip_markdown(raw: &str) -> String {
    let trimmed = raw.trim();

    // Strip fenced code blocks: ```[lang]\n...\n```
    if let Some(inner) = trimmed
        .strip_prefix("```")
        .and_then(|s| s.strip_suffix("```"))
    {
        // Remove optional language hint on first line
        let without_hint = if let Some(nl) = inner.find('\n') {
            let first = &inner[..nl];
            // If the first line is only a language hint (no spaces, no shell chars), skip it
            if first.chars().all(|c| c.is_ascii_alphabetic()) {
                &inner[nl + 1..]
            } else {
                inner
            }
        } else {
            inner
        };
        return without_hint.trim().to_string();
    }

    // Strip inline backticks: `command`
    if trimmed.starts_with('`') && trimmed.ends_with('`') && trimmed.len() > 1 {
        return trimmed[1..trimmed.len() - 1].to_string();
    }

    trimmed.to_string()
}
