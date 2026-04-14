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
}

impl CliArgs {
    pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut model = "llama3.2".to_string();
        let mut endpoint = "http://localhost:11434".to_string();
        let mut prompt: Option<String> = None;

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
                other => return Err(format!("unknown flag: {other}")),
            }
        }

        Ok(Self {
            model,
            endpoint,
            prompt: prompt.ok_or("--prompt is required")?,
        })
    }
}

pub fn run(args: &CliArgs, adapter: &dyn ProviderAdapter) -> Result<String, ProviderError> {
    let req = GenerateRequest {
        model: args.model.clone(),
        prompt: args.prompt.clone(),
    };
    adapter.generate(&req).map(|r| r.text)
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
                text: format!("echo:{}", req.prompt),
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
        };

        let result = run(&args, &EchoAdapter).unwrap();
        assert_eq!(result, "echo:hello");
    }

    #[test]
    fn run_propagates_adapter_error() {
        let args = CliArgs {
            model: "llama3.2".to_string(),
            endpoint: "http://localhost:11434".to_string(),
            prompt: "hello".to_string(),
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
