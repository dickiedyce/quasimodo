pub mod bank;
pub mod filter;
pub mod notfound;
pub mod prompt;

use bank::{Bank, Retriever};
use filter::{command_exists, command_name, is_sensitive};
use prompt::{PromptBuilder, strip_markdown};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;

#[derive(Debug, Clone, PartialEq)]
pub struct GenerateRequest {
    pub model: String,
    pub prompt: String,
    pub temperature: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatRequest {
    pub model: String,
    pub prompt: String,
    pub temperature: f32,
    pub system_prompt: Option<String>,
    pub history: Vec<ChatMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateResponse {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    InvalidConfig(String),
    Unavailable,
    Transport(String),
    Parse(String),
}

pub trait ProviderAdapter {
    fn is_local_available(&self) -> Result<bool, ProviderError>;
    fn generate(&self, req: &GenerateRequest) -> Result<GenerateResponse, ProviderError>;

    fn generate_chat(&self, req: &ChatRequest) -> Result<GenerateResponse, ProviderError> {
        let mut flattened = String::new();

        if let Some(system) = &req.system_prompt {
            flattened.push_str(system);
            flattened.push_str("\n\n");
        }

        for msg in &req.history {
            flattened.push_str(&msg.role);
            flattened.push_str(": ");
            flattened.push_str(&msg.content);
            flattened.push('\n');
        }

        flattened.push_str(&req.prompt);

        self.generate(&GenerateRequest {
            model: req.model.clone(),
            prompt: flattened,
            temperature: req.temperature,
        })
    }
}

pub struct OllamaAdapter {
    endpoint: String,
}

impl OllamaAdapter {
    pub fn new(endpoint: impl Into<String>) -> Result<Self, ProviderError> {
        let endpoint = endpoint.into();

        if !is_local_endpoint(&endpoint) {
            return Err(ProviderError::InvalidConfig(
                "ollama endpoint must be local".to_string(),
            ));
        }

        Ok(Self { endpoint })
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

impl ProviderAdapter for OllamaAdapter {
    fn is_local_available(&self) -> Result<bool, ProviderError> {
        let url = self.health_url();
        match ureq::get(&url).call() {
            Ok(resp) => Ok(resp.status() == 200),
            Err(ureq::Error::Status(code, _)) => Ok(code == 200),
            Err(err) => Err(ProviderError::Transport(err.to_string())),
        }
    }

    fn generate(&self, req: &GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        let url = self.generate_url();
        let payload = Self::build_generate_payload(req);
        let response_text = ureq::post(&url)
            .set("Content-Type", "application/json")
            .send_string(&payload)
            .map_err(|err| ProviderError::Transport(err.to_string()))?
            .into_string()
            .map_err(|err| ProviderError::Transport(err.to_string()))?;

        Self::parse_generate_response(&response_text)
    }

    fn generate_chat(&self, req: &ChatRequest) -> Result<GenerateResponse, ProviderError> {
        let url = self.chat_url();
        let payload = Self::build_chat_payload(req);
        let response_text = ureq::post(&url)
            .set("Content-Type", "application/json")
            .send_string(&payload)
            .map_err(|err| ProviderError::Transport(err.to_string()))?
            .into_string()
            .map_err(|err| ProviderError::Transport(err.to_string()))?;

        Self::parse_chat_response(&response_text)
    }
}

impl OllamaAdapter {
    fn health_url(&self) -> String {
        format!("{}/api/tags", self.endpoint.trim_end_matches('/'))
    }

    fn generate_url(&self) -> String {
        format!("{}/api/generate", self.endpoint.trim_end_matches('/'))
    }

    fn chat_url(&self) -> String {
        format!("{}/api/chat", self.endpoint.trim_end_matches('/'))
    }

    fn build_generate_payload(req: &GenerateRequest) -> String {
        json!({
            "model": req.model,
            "prompt": req.prompt,
            "temperature": req.temperature,
            "stream": false
        })
        .to_string()
    }

    fn parse_generate_response(raw: &str) -> Result<GenerateResponse, ProviderError> {
        let parsed: Value =
            serde_json::from_str(raw).map_err(|err| ProviderError::Parse(err.to_string()))?;
        let text = parsed
            .get("response")
            .and_then(Value::as_str)
            .ok_or_else(|| ProviderError::Parse("missing response field".to_string()))?;

        Ok(GenerateResponse {
            text: text.to_string(),
        })
    }

    fn build_chat_payload(req: &ChatRequest) -> String {
        let mut messages = Vec::new();

        if let Some(system) = &req.system_prompt {
            messages.push(json!({"role": "system", "content": system}));
        }

        for msg in &req.history {
            messages.push(json!({"role": msg.role, "content": msg.content}));
        }

        messages.push(json!({"role": "user", "content": req.prompt}));

        json!({
            "model": req.model,
            "messages": messages,
            "stream": false,
            "options": {
                "temperature": req.temperature
            }
        })
        .to_string()
    }

    fn parse_chat_response(raw: &str) -> Result<GenerateResponse, ProviderError> {
        let parsed: Value =
            serde_json::from_str(raw).map_err(|err| ProviderError::Parse(err.to_string()))?;

        let text = parsed
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(Value::as_str)
            .or_else(|| parsed.get("response").and_then(Value::as_str))
            .ok_or_else(|| ProviderError::Parse("missing message content".to_string()))?;

        Ok(GenerateResponse {
            text: text.to_string(),
        })
    }
}

// --- CLI ---

pub struct CliArgs {
    pub model: String,
    pub endpoint: String,
    pub prompt: String,
    pub bank_path: Option<String>,
    pub notfound: bool,
    pub explain: bool,
    pub samples: usize,
    pub temperature: f32,
    pub stdin: bool,
    pub system_prompt: Option<String>,
    pub history_file: Option<String>,
    pub quality_retry: bool,
    pub teach_description: Option<String>,
    pub teach_command: Option<String>,
    pub describe: bool,
}

impl CliArgs {
    pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut model = "llama3.2".to_string();
        let mut endpoint = "http://localhost:11434".to_string();
        let mut prompt: Option<String> = None;
        let mut bank_path: Option<String> = None;
        let mut notfound = false;
        let mut explain = false;
        let mut samples: usize = 1;
        let mut temperature: f32 = 0.0;
        let mut stdin = false;
        let mut system_prompt: Option<String> = None;
        let mut history_file: Option<String> = None;
        let mut quality_retry = true;
        let mut teach_description: Option<String> = None;
        let mut teach_command: Option<String> = None;
        let mut describe = false;

        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--model" => {
                    model = args.next().ok_or("--model requires a value")?;
                }
                "--prompt" => {
                    prompt = Some(args.next().ok_or("--prompt requires a value")?);
                }
                "--endpoint" => {
                    endpoint = args.next().ok_or("--endpoint requires a value")?;
                }
                "--bank" => {
                    bank_path = Some(args.next().ok_or("--bank requires a path")?);
                }
                "--notfound" => {
                    notfound = true;
                    // consume the command-not-found command name as the prompt
                    if prompt.is_none() {
                        prompt = Some(args.next().ok_or("--notfound requires a command name")?);
                    }
                }
                "--explain" => {
                    explain = true;
                    if prompt.is_none() {
                        prompt = Some(args.next().ok_or("--explain requires a value")?);
                    }
                }
                "--samples" => {
                    let raw = args.next().ok_or("--samples requires a value")?;
                    samples = raw.parse().map_err(|_| "--samples must be an integer")?;
                    if samples == 0 {
                        return Err("--samples must be >= 1".to_string());
                    }
                }
                "--temperature" => {
                    let raw = args.next().ok_or("--temperature requires a value")?;
                    temperature = raw
                        .parse()
                        .map_err(|_| "--temperature must be a float")?;
                }
                "--stdin" => {
                    stdin = true;
                }
                "--system" => {
                    system_prompt = Some(args.next().ok_or("--system requires a value")?);
                }
                "--history-file" => {
                    history_file = Some(args.next().ok_or("--history-file requires a path")?);
                }
                "--no-quality-retry" => {
                    quality_retry = false;
                }
                "--teach" => {
                    teach_description =
                        Some(args.next().ok_or("--teach requires a description")?);
                }
                "--command" => {
                    teach_command = Some(args.next().ok_or("--command requires a value")?);
                }
                "--describe" => {
                    describe = true;
                    if prompt.is_none() {
                        prompt = Some(args.next().ok_or("--describe requires a command")?);
                    }
                }
                other => return Err(format!("unknown flag: {other}")),
            }
        }

        let prompt = if let Some(prompt) = prompt {
            prompt
        } else if stdin || teach_description.is_some() {
            String::new()
        } else {
            return Err("--prompt is required".to_string());
        };

        if describe && explain {
            return Err("--describe and --explain cannot be used together".to_string());
        }

        Ok(Self {
            model,
            endpoint,
            prompt,
            bank_path,
            notfound,
            explain,
            samples,
            temperature,
            stdin,
            system_prompt,
            history_file,
            quality_retry,
            teach_description,
            teach_command,
            describe,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ConversationFile {
    messages: Vec<ChatMessage>,
}

fn load_history(path: &str) -> Vec<ChatMessage> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };

    serde_json::from_str::<ConversationFile>(&raw)
        .map(|f| f.messages)
        .unwrap_or_default()
}

fn save_history(path: &str, messages: &[ChatMessage]) -> Result<(), ProviderError> {
    let doc = ConversationFile {
        messages: messages.to_vec(),
    };
    let raw = serde_json::to_string_pretty(&doc)
        .map_err(|err| ProviderError::Parse(err.to_string()))?;
    fs::write(path, raw).map_err(|err| ProviderError::Transport(err.to_string()))
}

pub fn run(args: &CliArgs, adapter: &dyn ProviderAdapter) -> Result<String, ProviderError> {
    // --teach mode: store a user-provided example in the bank; no LLM call needed.
    if let Some(ref description) = args.teach_description {
        let command = args
            .teach_command
            .as_deref()
            .ok_or_else(|| ProviderError::InvalidConfig("--teach requires --command".to_string()))?;
        let bank_path = args
            .bank_path
            .as_deref()
            .ok_or_else(|| ProviderError::InvalidConfig("--teach requires --bank".to_string()))?;
        let bank = Bank::open(bank_path)
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
        bank.teach(description, command)
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
        return Ok(format!("taught: {description} -> {command}"));
    }

    // Sensitive filter: never send credential-containing prompts to the model.
    if is_sensitive(&args.prompt) {
        return Err(ProviderError::InvalidConfig(
            "prompt contains sensitive content".to_string(),
        ));
    }

    // --notfound mode: resolve via static maps before calling the model.
    if args.notfound {
        use notfound::{suggest_not_found, NotFoundSuggestion};
        let bank = args.bank_path.as_deref().and_then(|p| Bank::open(p).ok());
        let suggestion = suggest_not_found(&args.prompt, bank.as_ref());
        return Ok(match suggestion {
            NotFoundSuggestion::Typo(s) => format!("did you mean: {s}"),
            NotFoundSuggestion::Install { formula } => {
                format!("not installed: brew install {formula}")
            }
            NotFoundSuggestion::MacOsEquivalent(s) => format!("macOS equivalent: {s}"),
            NotFoundSuggestion::Unknown => format!("command not found: {}", args.prompt),
        });
    }

    let bank_system = if let Some(ref path) = args.bank_path {
        Bank::open(path)
            .ok()
            .and_then(|bank| {
                let retriever = Retriever::new(&bank, 8);
                retriever.retrieve(&args.prompt).ok().map(|examples| {
                    PromptBuilder::new(examples).system_prompt()
                })
            })
    } else {
        None
    };

    let merged_system = match (&args.system_prompt, &bank_system) {
        (Some(a), Some(b)) => Some(format!("{a}\n\n{b}")),
        (Some(a), None) => Some(a.clone()),
        (None, Some(b)) => Some(b.clone()),
        (None, None) => None,
    };

    let use_chat = merged_system.is_some() || args.history_file.is_some();

    let history = args
        .history_file
        .as_deref()
        .map(load_history)
        .unwrap_or_default();

    let final_prompt = if args.explain {
        format!(
            "This is a Unix/macOS shell command or error output. \
             Explain briefly what went wrong and suggest a concrete fix. \
             Shell command or error: {}",
            args.prompt
        )
    } else if args.describe {
        format!(
            "Describe in one plain English sentence what this Unix/macOS shell command does: {}",
            args.prompt
        )
    } else {
        args.prompt.clone()
    };

    let generate_once = |prompt_text: &str| -> Result<String, ProviderError> {
        if use_chat {
            let req = ChatRequest {
                model: args.model.clone(),
                prompt: prompt_text.to_string(),
                temperature: args.temperature,
                system_prompt: merged_system.clone(),
                history: history.clone(),
            };
            adapter.generate_chat(&req).map(|r| strip_markdown(&r.text))
        } else {
            let req = GenerateRequest {
                model: args.model.clone(),
                prompt: prompt_text.to_string(),
                temperature: args.temperature,
            };
            adapter.generate(&req).map(|r| strip_markdown(&r.text))
        }
    };

    let raw_unchecked = if args.samples > 1 && !args.explain {
        use std::collections::HashMap;

        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut first: Option<String> = None;

        for _ in 0..args.samples {
            let sample = generate_once(&final_prompt)?;
            if first.is_none() {
                first = Some(sample.clone());
            }
            *counts.entry(sample).or_insert(0) += 1;
        }

        counts
            .into_iter()
            .max_by_key(|(_, n)| *n)
            .map(|(s, _)| s)
            .or(first)
            .unwrap_or_default()
    } else {
        generate_once(&final_prompt)?
    };

    let raw = if args.explain {
        raw_unchecked
    } else {
        normalize_command_output(&args.prompt, &raw_unchecked)
    };

    if args.explain || args.describe {
        return Ok(one_line_explanation(&raw));
    }

    if args.quality_retry && command_quality_score(&args.prompt, &raw) < 0 {
        let retry_prompt = format!(
            "{} (previous answer '{}' was low quality for this request; {})",
            args.prompt,
            raw,
            retry_guidance(&args.prompt)
        );
        let retry = normalize_command_output(&args.prompt, &generate_once(&retry_prompt)?);
        if command_quality_score(&args.prompt, &retry) > command_quality_score(&args.prompt, &raw) {
            if let Some(path) = &args.history_file {
                let mut next_history = history.clone();
                next_history.push(ChatMessage {
                    role: "user".to_string(),
                    content: args.prompt.clone(),
                });
                next_history.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: retry.clone(),
                });
                save_history(path, &next_history)?;
            }
            return Ok(retry);
        }
    }

    // Command existence validation: retry once if the binary doesn't exist.
    if !command_exists(command_name(&raw)) {
        let retry_prompt = format!(
            "{} (previous answer '{}' was not a valid command, try again)",
            args.prompt, raw
        );
        return generate_once(&retry_prompt);
    }

    if let Some(path) = &args.history_file {
        let mut next_history = history;
        next_history.push(ChatMessage {
            role: "user".to_string(),
            content: args.prompt.clone(),
        });
        next_history.push(ChatMessage {
            role: "assistant".to_string(),
            content: raw.clone(),
        });
        save_history(path, &next_history)?;
    }

    Ok(raw)
}

// --- helpers ---

fn is_local_endpoint(endpoint: &str) -> bool {
    endpoint.starts_with("http://localhost")
        || endpoint.starts_with("https://localhost")
        || endpoint.starts_with("http://127.0.0.1")
        || endpoint.starts_with("https://127.0.0.1")
}

fn one_line_explanation(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .to_string()
}

fn normalize_command_output(prompt: &str, text: &str) -> String {
    extract_command_candidate(prompt, text).unwrap_or_else(|| text.trim().to_string())
}

fn extract_command_candidate(prompt: &str, text: &str) -> Option<String> {
    let mut best: Option<(i32, String)> = None;

    for raw in text.lines() {
        for line in candidate_fragments(raw) {
            let score = candidate_score(prompt, &line);
            if score <= 0 {
                continue;
            }

            match &best {
                Some((best_score, _)) if *best_score >= score => {}
                _ => best = Some((score, line)),
            }
        }
    }

    best.map(|(_, line)| line)
}

fn candidate_fragments(raw: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut line = raw.trim().to_string();
    if line.is_empty() {
        return candidates;
    }

    if line.starts_with("- ") || line.starts_with("* ") {
        line = line[2..].trim().to_string();
    }
    if let Some(dot) = line.find('.') {
        if dot < 3 && line[..dot].chars().all(|c| c.is_ascii_digit()) {
            line = line[dot + 1..].trim().to_string();
        }
    }

    if line.starts_with('`') && line.ends_with('`') && line.len() > 1 {
        line = line[1..line.len() - 1].trim().to_string();
    }

    candidates.push(line.clone());

    // Also extract inline backticked fragments from prose-heavy lines.
    let mut rest = raw;
    while let Some(start) = rest.find('`') {
        let tail = &rest[start + 1..];
        if let Some(end) = tail.find('`') {
            let fragment = tail[..end].trim();
            if !fragment.is_empty() {
                candidates.push(fragment.to_string());
            }
            rest = &tail[end + 1..];
        } else {
            break;
        }
    }

    candidates
}

fn candidate_score(prompt: &str, candidate: &str) -> i32 {
    let c = candidate.trim();
    if c.is_empty() {
        return -10;
    }
    let p = prompt.to_lowercase();

    // Hard reject obvious prose/explanations.
    let lc = c.to_lowercase();
    let prose_starts = [
        "here", "to ", "if ", "you ", "this ", "that ", "note", "method",
        "steps", "example", "please", "the ", "an ", "a ",
    ];
    if prose_starts.iter().any(|p| lc.starts_with(p)) {
        return -5;
    }

    let mut score = 0;

    if command_exists(command_name(c)) {
        score += 4;
    }

    // Command-ish structure indicators.
    if c.contains(" -") {
        score += 1;
    }
    if c.contains('|') || c.contains('>') || c.contains('<') || c.contains("$") {
        score += 1;
    }
    if c.contains("&&") || c.contains("||") {
        score += 1;
    }

    // Penalize clearly sentence-like lines.
    if c.ends_with(':') {
        score -= 2;
    }
    if c.split_whitespace().count() > 14 {
        score -= 2;
    }

    // Penalize Linux-centric network commands on macOS unless explicitly requested.
    if (p.contains("network") || p.contains("port") || p.contains("dns") || p.contains("interface"))
        && (lc.starts_with("ss ") || lc == "ss" || lc.starts_with("ip ") || lc == "ip")
    {
        score -= 4;
    }

    // Use existing intent scoring as secondary signal.
    score + command_quality_score(prompt, c)
}

fn command_quality_score(prompt: &str, command: &str) -> i32 {
    let p = prompt.to_lowercase();
    let c = command.to_lowercase();
    let mut score = 0;

    // Baseline: commands that exist locally are better than unknown binaries.
    if command_exists(command_name(command)) {
        score += 2;
    } else {
        score -= 2;
    }

    // Penalize clearly unrelated heavy-tool domains unless requested.
    if c.starts_with("docker") && !p.contains("docker") {
        score -= 2;
    }
    if c.starts_with("kubectl") && !p.contains("kubernetes") && !p.contains("k8s") {
        score -= 2;
    }

    // Intent-specific boosts/penalties.
    if p.contains("disk") && p.contains("usage") {
        if c.starts_with("du") || c.starts_with("df") {
            score += 4;
        } else {
            score -= 3;
        }
    }

    if p.contains("changed") && p.contains("file") {
        if c.starts_with("find") || c.starts_with("git") {
            score += 4;
        } else {
            score -= 3;
        }
    }

    if p.contains("open") && p.contains("port") {
        if c.starts_with("lsof") || c.starts_with("netstat") || c.starts_with("nc") || c.starts_with("nmap") || c.starts_with("telnet") {
            score += 4;
        } else {
            score -= 3;
        }
    }

    if p.contains("listen") && p.contains("port") {
        if c.starts_with("lsof") || c.starts_with("netstat") {
            score += 4;
        }
        if c.starts_with("ss") {
            score -= 4;
        }
    }

    if p.contains("dns") || p.contains("resolve") {
        if c.starts_with("dig") || c.starts_with("nslookup") || c.starts_with("host") {
            score += 5;
        } else {
            score -= 3;
        }
    }

    if p.contains("interface") {
        if c.starts_with("ifconfig") || c.starts_with("networksetup") {
            score += 4;
        }
        if c.starts_with("ip") {
            score -= 4;
        }
    }

    score
}

fn retry_guidance(prompt: &str) -> &'static str {
    let p = prompt.to_lowercase();

    if p.contains("dns") || p.contains("resolve") {
        "return exactly one macOS shell command with no prose; prefer dig, nslookup, or host"
    } else if p.contains("open") && p.contains("port") {
        "return exactly one shell command with no prose; prefer nc, lsof, netstat, nmap, or telnet"
    } else if p.contains("listen") && p.contains("port") {
        "return exactly one macOS shell command with no prose; prefer lsof or netstat, not ss"
    } else if p.contains("interface") || p.contains("network interfaces") {
        "return exactly one macOS shell command with no prose; prefer ifconfig or networksetup"
    } else {
        "return exactly one more relevant shell command with no prose"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_local_endpoints() {
        let result = OllamaAdapter::new("https://api.example.com");

        assert!(matches!(
            result,
            Err(ProviderError::InvalidConfig(ref msg)) if msg == "ollama endpoint must be local"
        ));
    }

    #[test]
    fn accepts_localhost_endpoint() {
        let adapter = OllamaAdapter::new("http://localhost:11434").unwrap();

        assert_eq!(adapter.endpoint(), "http://localhost:11434");
    }

    #[test]
    fn builds_expected_ollama_urls() {
        let adapter = OllamaAdapter::new("http://localhost:11434/").unwrap();

        assert_eq!(adapter.health_url(), "http://localhost:11434/api/tags");
        assert_eq!(adapter.generate_url(), "http://localhost:11434/api/generate");
    }

    #[test]
    fn builds_generate_payload_with_required_fields() {
        let req = GenerateRequest {
            model: "llama3.2".to_string(),
            prompt: "hello".to_string(),
            temperature: 0.0,
        };

        let payload = OllamaAdapter::build_generate_payload(&req);
        assert!(payload.contains("\"model\":\"llama3.2\""));
        assert!(payload.contains("\"prompt\":\"hello\""));
        assert!(payload.contains("\"stream\":false"));
    }

    #[test]
    fn parses_generate_response_text() {
        let raw = r#"{"response":"hi there","done":true}"#;

        let parsed = OllamaAdapter::parse_generate_response(raw).unwrap();
        assert_eq!(parsed.text, "hi there");
    }

    #[test]
    fn builds_chat_payload_with_system_and_history() {
        let req = ChatRequest {
            model: "llama3.2".to_string(),
            prompt: "next question".to_string(),
            temperature: 0.2,
            system_prompt: Some("You are concise".to_string()),
            history: vec![ChatMessage {
                role: "user".to_string(),
                content: "first question".to_string(),
            }],
        };

        let payload = OllamaAdapter::build_chat_payload(&req);
        assert!(payload.contains("\"messages\""));
        assert!(payload.contains("\"role\":\"system\""));
        assert!(payload.contains("\"You are concise\""));
        assert!(payload.contains("\"first question\""));
        assert!(payload.contains("\"next question\""));
    }

    #[test]
    fn parses_chat_response_text() {
        let raw = r#"{"message":{"role":"assistant","content":"use git status"}}"#;
        let parsed = OllamaAdapter::parse_chat_response(raw).unwrap();
        assert_eq!(parsed.text, "use git status");
    }

    // --- mock adapters for contract tests ---

    struct EchoAdapter;

    impl ProviderAdapter for EchoAdapter {
        fn is_local_available(&self) -> Result<bool, ProviderError> {
            Ok(true)
        }

        fn generate(&self, req: &GenerateRequest) -> Result<GenerateResponse, ProviderError> {
            Ok(GenerateResponse {
                text: format!("echo {}", req.prompt),
            })
        }
    }

    struct UnavailableAdapter;

    impl ProviderAdapter for UnavailableAdapter {
        fn is_local_available(&self) -> Result<bool, ProviderError> {
            Ok(false)
        }

        fn generate(&self, _req: &GenerateRequest) -> Result<GenerateResponse, ProviderError> {
            Err(ProviderError::Unavailable)
        }
    }

    // --- adapter contract tests ---

    #[test]
    fn adapter_contract_availability_returns_bool() {
        assert_eq!(EchoAdapter.is_local_available().unwrap(), true);
        assert_eq!(UnavailableAdapter.is_local_available().unwrap(), false);
    }

    #[test]
    fn adapter_contract_generate_returns_result() {
        let req = GenerateRequest {
            model: "m".to_string(),
            prompt: "p".to_string(),
            temperature: 0.0,
        };

        assert!(EchoAdapter.generate(&req).is_ok());
        assert!(matches!(
            UnavailableAdapter.generate(&req),
            Err(ProviderError::Unavailable)
        ));
    }

    // --- CLI tests ---

    #[test]
    fn run_delegates_to_adapter_and_returns_text() {
        let args = CliArgs {
            model: "llama3.2".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            prompt: "hello".to_string(),
            bank_path: None,
            notfound: false,
            explain: false,
            samples: 1,
            temperature: 0.0,
            stdin: false,
            system_prompt: None,
            history_file: None,
            quality_retry: true,
            teach_description: None,
            teach_command: None,
            describe: false,
        };

        let result = run(&args, &EchoAdapter).unwrap();
        assert_eq!(result, "echo hello");
    }

    #[test]
    fn run_propagates_adapter_error() {
        let args = CliArgs {
            model: "llama3.2".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            prompt: "hello".to_string(),
            bank_path: None,
            notfound: false,
            explain: false,
            samples: 1,
            temperature: 0.0,
            stdin: false,
            system_prompt: None,
            history_file: None,
            quality_retry: true,
            teach_description: None,
            teach_command: None,
            describe: false,
        };

        assert!(matches!(run(&args, &UnavailableAdapter), Err(ProviderError::Unavailable)));
    }

    #[test]
    fn cli_args_parse_all_flags() {
        let raw = [
            "--model", "llama3.2",
            "--prompt", "hello world",
            "--endpoint", "http://localhost:11434",
            "--bank", "./tldr_bank.db",
            "--system", "You are terse",
            "--history-file", "./session.json",
        ]
        .iter()
        .map(|s| s.to_string());

        let args = CliArgs::parse(raw).unwrap();
        assert_eq!(args.model, "llama3.2");
        assert_eq!(args.prompt, "hello world");
        assert_eq!(args.endpoint, "http://localhost:11434");
        assert_eq!(args.bank_path.as_deref(), Some("./tldr_bank.db"));
        assert_eq!(args.system_prompt.as_deref(), Some("You are terse"));
        assert_eq!(args.history_file.as_deref(), Some("./session.json"));
    }

    #[test]
    fn cli_args_applies_defaults_for_model_and_endpoint() {
        let raw = ["--prompt", "hello"].iter().map(|s| s.to_string());
        let args = CliArgs::parse(raw).unwrap();

        assert_eq!(args.model, "llama3.2");
        assert_eq!(args.endpoint, "http://localhost:11434");
    }

    #[test]
    fn cli_args_fails_without_prompt() {
        let raw = ["--model", "llama3.2"].iter().map(|s| s.to_string());
        assert!(CliArgs::parse(raw).is_err());
    }

    #[test]
    fn cli_args_parse_explain_mode() {
        let raw = ["--explain", "Command: git push -- Exit code: 128"]
            .iter()
            .map(|s| s.to_string());

        let args = CliArgs::parse(raw).unwrap();
        assert!(args.explain);
        assert_eq!(args.prompt, "Command: git push -- Exit code: 128");
    }

    #[test]
    fn cli_args_parse_samples_and_temperature() {
        let raw = [
            "--prompt",
            "hello",
            "--samples",
            "3",
            "--temperature",
            "0.3",
        ]
        .iter()
        .map(|s| s.to_string());

        let args = CliArgs::parse(raw).unwrap();
        assert_eq!(args.samples, 3);
        assert_eq!(args.temperature, 0.3);
    }

    #[test]
    fn cli_args_defaults_samples_and_temperature() {
        let raw = ["--prompt", "hello"].iter().map(|s| s.to_string());
        let args = CliArgs::parse(raw).unwrap();
        assert_eq!(args.samples, 1);
        assert_eq!(args.temperature, 0.0);
        assert!(args.quality_retry);
    }

    #[test]
    fn cli_args_parse_no_quality_retry_flag() {
        let raw = ["--prompt", "hello", "--no-quality-retry"]
            .iter()
            .map(|s| s.to_string());
        let args = CliArgs::parse(raw).unwrap();
        assert!(!args.quality_retry);
    }

    #[test]
    fn cli_args_parse_stdin_mode_without_prompt() {
        let raw = ["--stdin"].iter().map(|s| s.to_string());
        let args = CliArgs::parse(raw).unwrap();
        assert!(args.stdin);
        assert_eq!(args.prompt, "");
    }

    #[test]
    fn cli_args_still_requires_prompt_without_stdin() {
        let raw = ["--model", "llama3.2"].iter().map(|s| s.to_string());
        assert!(CliArgs::parse(raw).is_err());
    }

    struct CyclingAdapter {
        idx: std::cell::Cell<usize>,
    }

    impl CyclingAdapter {
        fn new() -> Self {
            Self {
                idx: std::cell::Cell::new(0),
            }
        }
    }

    impl ProviderAdapter for CyclingAdapter {
        fn is_local_available(&self) -> Result<bool, ProviderError> {
            Ok(true)
        }

        fn generate(&self, _req: &GenerateRequest) -> Result<GenerateResponse, ProviderError> {
            let i = self.idx.get();
            self.idx.set(i + 1);
            let text = match i {
                0 => "ls -la",
                1 => "ls -la",
                _ => "pwd",
            };
            Ok(GenerateResponse {
                text: text.to_string(),
            })
        }
    }

    #[test]
    fn run_uses_majority_vote_when_samples_gt_1() {
        let args = CliArgs {
            model: "llama3.2".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            prompt: "list files".to_string(),
            bank_path: None,
            notfound: false,
            explain: false,
            samples: 3,
            temperature: 0.3,
            stdin: false,
            system_prompt: None,
            history_file: None,
            quality_retry: true,
            teach_description: None,
            teach_command: None,
            describe: false,
        };

        let adapter = CyclingAdapter::new();
        let out = run(&args, &adapter).unwrap();
        assert_eq!(out, "ls -la");
    }

    #[test]
    fn run_explain_mode_skips_command_validation() {
        let args = CliArgs {
            model: "llama3.2".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            prompt: "Command: git push -- Exit code: 128".to_string(),
            bank_path: None,
            notfound: false,
            explain: true,
            samples: 1,
            temperature: 0.0,
            stdin: false,
            system_prompt: None,
            history_file: None,
            quality_retry: true,
            teach_description: None,
            teach_command: None,
            describe: false,
        };

        let result = run(&args, &EchoAdapter).unwrap();
        assert!(result.starts_with("echo "));
    }

    struct MultilineExplainAdapter;

    impl ProviderAdapter for MultilineExplainAdapter {
        fn is_local_available(&self) -> Result<bool, ProviderError> {
            Ok(true)
        }

        fn generate(&self, _req: &GenerateRequest) -> Result<GenerateResponse, ProviderError> {
            Ok(GenerateResponse {
                text: "line one explanation\nline two detail".to_string(),
            })
        }
    }

    #[test]
    fn run_explain_mode_returns_single_line() {
        let args = CliArgs {
            model: "llama3.2".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            prompt: "Command: git push -- Exit code: 128".to_string(),
            bank_path: None,
            notfound: false,
            explain: true,
            samples: 1,
            temperature: 0.0,
            stdin: false,
            system_prompt: None,
            history_file: None,
            quality_retry: true,
            teach_description: None,
            teach_command: None,
            describe: false,
        };

        let result = run(&args, &MultilineExplainAdapter).unwrap();
        assert_eq!(result, "line one explanation");
    }

    struct LowThenBetterAdapter {
        idx: std::cell::Cell<usize>,
    }

    impl LowThenBetterAdapter {
        fn new() -> Self {
            Self {
                idx: std::cell::Cell::new(0),
            }
        }
    }

    impl ProviderAdapter for LowThenBetterAdapter {
        fn is_local_available(&self) -> Result<bool, ProviderError> {
            Ok(true)
        }

        fn generate(&self, _req: &GenerateRequest) -> Result<GenerateResponse, ProviderError> {
            let i = self.idx.get();
            self.idx.set(i + 1);
            let text = if i == 0 { "echo hi" } else { "du -sh ." };
            Ok(GenerateResponse {
                text: text.to_string(),
            })
        }
    }

    #[test]
    fn run_retries_when_response_quality_is_low_for_prompt_intent() {
        let args = CliArgs {
            model: "llama3.2".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            prompt: "show disk usage".to_string(),
            bank_path: None,
            notfound: false,
            explain: false,
            samples: 1,
            temperature: 0.0,
            stdin: false,
            system_prompt: None,
            history_file: None,
            quality_retry: true,
            teach_description: None,
            teach_command: None,
            describe: false,
        };

        let out = run(&args, &LowThenBetterAdapter::new()).unwrap();
        assert_eq!(out, "du -sh .");
    }

    #[test]
    fn extract_command_candidate_from_verbose_text() {
        let verbose = r#"
Here is how to test if a port is open:
1. Use netcat:
nc -z example.com 443
2. Or use nmap.
"#;

        let extracted = normalize_command_output("test if port 443 is open", verbose);
        assert_eq!(extracted, "nc -z example.com 443");
    }

    #[test]
    fn extract_command_candidate_from_dns_prose() {
        let verbose = r#"
The easiest way to resolve DNS is:
nslookup github.com
This should return the address.
"#;

        let extracted = normalize_command_output("resolve github.com dns", verbose);
        assert_eq!(extracted, "nslookup github.com");
    }

    #[test]
    fn extract_inline_backticked_network_command_from_prose() {
        let verbose = "Use `nc -z example.com 443` to test whether the port is open.";

        let extracted = normalize_command_output("test if port 443 is open on example.com", verbose);
        assert_eq!(extracted, "nc -z example.com 443");
    }

    #[test]
    fn retry_guidance_is_dns_specific() {
        assert!(retry_guidance("resolve github.com dns").contains("dig"));
        assert!(retry_guidance("resolve github.com dns").contains("nslookup"));
    }

    struct ChatOnlyAdapter;

    impl ProviderAdapter for ChatOnlyAdapter {
        fn is_local_available(&self) -> Result<bool, ProviderError> {
            Ok(true)
        }

        fn generate(&self, _req: &GenerateRequest) -> Result<GenerateResponse, ProviderError> {
            Err(ProviderError::Unavailable)
        }

        fn generate_chat(&self, req: &ChatRequest) -> Result<GenerateResponse, ProviderError> {
            Ok(GenerateResponse {
                text: format!("echo {}", req.prompt),
            })
        }
    }

    #[test]
    fn run_uses_chat_path_when_system_prompt_is_set() {
        let args = CliArgs {
            model: "llama3.2".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            prompt: "hello".to_string(),
            bank_path: None,
            notfound: false,
            explain: false,
            samples: 1,
            temperature: 0.0,
            stdin: false,
            system_prompt: Some("You are concise".to_string()),
            history_file: None,
            quality_retry: true,
            teach_description: None,
            teach_command: None,
            describe: false,
        };

        let out = run(&args, &ChatOnlyAdapter).unwrap();
        assert_eq!(out, "echo hello");
    }

    #[test]
    fn run_persists_history_file() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("quasimodo-history-{now}.json"));

        let args = CliArgs {
            model: "llama3.2".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            prompt: "hello history".to_string(),
            bank_path: None,
            notfound: false,
            explain: false,
            samples: 1,
            temperature: 0.0,
            stdin: false,
            system_prompt: Some("You are concise".to_string()),
            history_file: Some(path.to_string_lossy().to_string()),
            quality_retry: true,
            teach_description: None,
            teach_command: None,
            describe: false,
        };

        let out = run(&args, &ChatOnlyAdapter).unwrap();
        assert_eq!(out, "echo hello history");

        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("\"role\": \"user\""));
        assert!(saved.contains("\"hello history\""));
        assert!(saved.contains("\"role\": \"assistant\""));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn teach_via_cli_stores_example_and_returns_confirmation() {
        use crate::bank::Bank;
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("quasimodo-teach-{now}.db"));
        let db_path = path.to_string_lossy().to_string();

        // Initialise schema.
        let _ = Bank::open(&db_path).unwrap();

        let args = CliArgs::parse(
            vec![
                "--teach".to_string(),
                "date 90 days ago".to_string(),
                "--command".to_string(),
                "date -v -90d '+%Y-%m-%d'".to_string(),
                "--bank".to_string(),
                db_path.clone(),
            ]
            .into_iter(),
        )
        .unwrap();

        assert_eq!(args.teach_description.as_deref(), Some("date 90 days ago"));
        assert_eq!(
            args.teach_command.as_deref(),
            Some("date -v -90d '+%Y-%m-%d'")
        );

        let result = run(&args, &EchoAdapter).unwrap();
        assert!(result.contains("taught"), "expected 'taught' in: {result}");

        let bank = Bank::open(&db_path).unwrap();
        let hits = bank.search("date 90 days ago", 3).unwrap();
        assert!(
            hits.iter().any(|e| e.command.contains("date -v")),
            "taught example not found in search results"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn describe_mode_sends_description_prompt_and_returns_single_line() {
        let args = CliArgs {
            model: "llama3.2".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            prompt: "find . -mmin -60".to_string(),
            bank_path: None,
            notfound: false,
            explain: false,
            describe: true,
            samples: 1,
            temperature: 0.0,
            stdin: false,
            system_prompt: None,
            history_file: None,
            quality_retry: true,
            teach_description: None,
            teach_command: None,
        };

        // EchoAdapter echoes the prompt; one_line_explanation picks the first line.
        let result = run(&args, &EchoAdapter).unwrap();
        // The prompt sent to the adapter should contain "Describe" and the command.
        assert!(
            result.starts_with("echo "),
            "expected echo adapter output, got: {result}"
        );
    }

    #[test]
    fn describe_parse_sets_flag_and_captures_command() {
        let args = CliArgs::parse(
            vec!["--describe".to_string(), "find . -mmin -60".to_string()].into_iter(),
        )
        .unwrap();
        assert!(args.describe);
        assert_eq!(args.prompt, "find . -mmin -60");
    }
}

#[cfg(test)]
mod bank_tests {
    use crate::bank::{Bank, BankEntry, Retriever};

    fn sample_entries() -> Vec<BankEntry> {
        vec![
            BankEntry {
                description: "find files changed in the last hour".to_string(),
                command: "find . -mmin -60".to_string(),
            },
            BankEntry {
                description: "show disk usage of current directory".to_string(),
                command: "du -sh .".to_string(),
            },
            BankEntry {
                description: "list open network ports".to_string(),
                command: "lsof -i -n -P".to_string(),
            },
        ]
    }

    #[test]
    fn bank_stores_and_counts_entries() {
        let bank = Bank::open_in_memory().unwrap();

        for entry in sample_entries() {
            bank.insert(&entry).unwrap();
        }

        assert_eq!(bank.len().unwrap(), 3);
    }

    #[test]
    fn bank_search_returns_relevant_results() {
        let bank = Bank::open_in_memory().unwrap();

        for entry in sample_entries() {
            bank.insert(&entry).unwrap();
        }

        let results = bank.search("find files hour", 3).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].command, "find . -mmin -60");
    }

    #[test]
    fn bank_search_respects_limit() {
        let bank = Bank::open_in_memory().unwrap();

        for entry in sample_entries() {
            bank.insert(&entry).unwrap();
        }

        let results = bank.search("find disk ports", 2).unwrap();
        assert!(results.len() <= 2);
    }

    #[test]
    fn retriever_returns_top_n_for_query() {
        let bank = Bank::open_in_memory().unwrap();

        for entry in sample_entries() {
            bank.insert(&entry).unwrap();
        }

        let retriever = Retriever::new(&bank, 2);
        let results = retriever.retrieve("disk usage").unwrap();

        assert!(!results.is_empty());
        assert!(results.len() <= 2);
    }
}

#[cfg(test)]
mod prompt_tests {
    use crate::bank::BankEntry;
    use crate::prompt::{PromptBuilder, strip_markdown};

    fn entries() -> Vec<BankEntry> {
        vec![
            BankEntry {
                description: "find files changed in the last hour".to_string(),
                command: "find . -mmin -60".to_string(),
            },
        ]
    }

    #[test]
    fn system_prompt_contains_example_description_and_command() {
        let builder = PromptBuilder::new(entries());
        let prompt = builder.system_prompt();

        assert!(prompt.contains("find files changed in the last hour"));
        assert!(prompt.contains("find . -mmin -60"));
    }

    #[test]
    fn system_prompt_contains_instruction_header() {
        let builder = PromptBuilder::new(entries());
        let prompt = builder.system_prompt();

        assert!(prompt.to_lowercase().contains("shell") || prompt.contains("command"));
    }

    #[test]
    fn strip_markdown_removes_backtick_fences() {
        assert_eq!(strip_markdown("```\nfind . -mmin -60\n```"), "find . -mmin -60");
    }

    #[test]
    fn strip_markdown_removes_inline_backticks() {
        assert_eq!(strip_markdown("`ls -la`"), "ls -la");
    }

    #[test]
    fn strip_markdown_leaves_plain_commands_unchanged() {
        assert_eq!(strip_markdown("du -sh ."), "du -sh .");
    }

    #[test]
    fn strip_markdown_removes_shell_language_hint() {
        assert_eq!(strip_markdown("```shell\ndu -sh .\n```"), "du -sh .");
        assert_eq!(strip_markdown("```bash\ndu -sh .\n```"), "du -sh .");
    }
}

#[cfg(test)]
mod filter_tests {
    use crate::filter::{command_exists, command_name, is_sensitive};

    #[test]
    fn detects_password_keyword() {
        assert!(is_sensitive("my password is hunter2"));
    }

    #[test]
    fn detects_token_keyword() {
        assert!(is_sensitive("use token abc123"));
    }

    #[test]
    fn detects_bearer_case_insensitive() {
        assert!(is_sensitive("Authorization: Bearer xyz"));
    }

    #[test]
    fn allows_normal_prompts() {
        assert!(!is_sensitive("find files changed in the last hour"));
        assert!(!is_sensitive("show disk usage"));
    }

    #[test]
    fn extracts_command_name_from_full_command() {
        assert_eq!(command_name("find . -mmin -60"), "find");
        assert_eq!(command_name("du -sh ."), "du");
        assert_eq!(command_name("ls"), "ls");
    }

    #[test]
    fn command_exists_finds_common_binaries() {
        assert!(command_exists("ls"));
        assert!(command_exists("echo"));
    }

    #[test]
    fn command_exists_rejects_nonsense() {
        assert!(!command_exists("xqz_not_a_real_command_7382"));
    }
}

#[cfg(test)]
mod notfound_tests {
    use crate::notfound::{NotFoundSuggestion, levenshtein, suggest_not_found};

    #[test]
    fn levenshtein_identical_strings() {
        assert_eq!(levenshtein("git", "git"), 0);
    }

    #[test]
    fn levenshtein_one_transposition() {
        assert_eq!(levenshtein("gti", "git"), 2);
    }

    #[test]
    fn levenshtein_one_insertion() {
        assert_eq!(levenshtein("gi", "git"), 1);
    }

    #[test]
    fn suggests_typo_for_gti() {
        let result = suggest_not_found("gti", None);
        assert_eq!(result, NotFoundSuggestion::Typo("git".to_string()));
    }

    #[test]
    fn suggests_brew_install_for_known_tool() {
        let result = suggest_not_found("ncdu", None);
        assert_eq!(
            result,
            NotFoundSuggestion::Install { formula: "ncdu".to_string() }
        );
    }

    #[test]
    fn suggests_macos_equivalent_for_ip() {
        let result = suggest_not_found("ip", None);
        assert_eq!(
            result,
            NotFoundSuggestion::MacOsEquivalent("ifconfig".to_string())
        );
    }

    #[test]
    fn returns_unknown_for_unrecognised_command() {
        let result = suggest_not_found("xqz_not_real_7382", None);
        assert_eq!(result, NotFoundSuggestion::Unknown);
    }
}
