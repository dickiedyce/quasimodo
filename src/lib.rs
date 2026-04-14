pub mod bank;
pub mod filter;
pub mod notfound;
pub mod prompt;

use bank::{Bank, Retriever};
use filter::{command_exists, command_name, is_sensitive};
use prompt::{PromptBuilder, strip_markdown};
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateRequest {
    pub model: String,
    pub prompt: String,
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
}

impl OllamaAdapter {
    fn health_url(&self) -> String {
        format!("{}/api/tags", self.endpoint.trim_end_matches('/'))
    }

    fn generate_url(&self) -> String {
        format!("{}/api/generate", self.endpoint.trim_end_matches('/'))
    }

    fn build_generate_payload(req: &GenerateRequest) -> String {
        json!({
            "model": req.model,
            "prompt": req.prompt,
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
}

// --- CLI ---

pub struct CliArgs {
    pub model: String,
    pub endpoint: String,
    pub prompt: String,
    pub bank_path: Option<String>,
    pub notfound: bool,
}

impl CliArgs {
    pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut model = "llama3.2".to_string();
        let mut endpoint = "http://localhost:11434".to_string();
        let mut prompt: Option<String> = None;
        let mut bank_path: Option<String> = None;
        let mut notfound = false;

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
                other => return Err(format!("unknown flag: {other}")),
            }
        }

        Ok(Self {
            model,
            endpoint,
            prompt: prompt.ok_or("--prompt is required")?,
            bank_path,
            notfound,
        })
    }
}

pub fn run(args: &CliArgs, adapter: &dyn ProviderAdapter) -> Result<String, ProviderError> {
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

    let system = if let Some(ref path) = args.bank_path {
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

    let full_prompt = match system {
        Some(ref sys) => format!("{sys}\nQ: {}\nA:", args.prompt),
        None => args.prompt.clone(),
    };

    let req = GenerateRequest {
        model: args.model.clone(),
        prompt: full_prompt,
    };

    let raw = adapter.generate(&req).map(|r| strip_markdown(&r.text))?;

    // Command existence validation: retry once if the binary doesn't exist.
    if !command_exists(command_name(&raw)) {
        let retry_prompt = format!(
            "{} (previous answer '{}' was not a valid command, try again)",
            args.prompt, raw
        );
        let retry_req = GenerateRequest {
            model: args.model.clone(),
            prompt: retry_prompt,
        };
        return adapter.generate(&retry_req).map(|r| strip_markdown(&r.text));
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
        };

        assert!(matches!(run(&args, &UnavailableAdapter), Err(ProviderError::Unavailable)));
    }

    #[test]
    fn cli_args_parse_all_flags() {
        let raw = [
            "--model", "llama3.2",
            "--prompt", "hello world",
            "--endpoint", "http://localhost:11434",
        ]
        .iter()
        .map(|s| s.to_string());

        let args = CliArgs::parse(raw).unwrap();
        assert_eq!(args.model, "llama3.2");
        assert_eq!(args.prompt, "hello world");
        assert_eq!(args.endpoint, "http://localhost:11434");
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
