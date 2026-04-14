/// Integration tests that exercise a live Ollama instance.
///
/// These tests skip automatically when Ollama is not reachable on
/// localhost:11434 so they are safe to run in any environment.
use quasimodo::{GenerateRequest, OllamaAdapter, ProviderAdapter};

fn ollama_reachable() -> bool {
    std::net::TcpStream::connect("127.0.0.1:11434").is_ok()
}

#[test]
fn ollama_reports_available_when_running() {
    if !ollama_reachable() {
        return;
    }

    let adapter = OllamaAdapter::new("http://localhost:11434").unwrap();
    assert_eq!(adapter.is_local_available().unwrap(), true);
}

#[test]
fn ollama_generate_returns_non_empty_text() {
    if !ollama_reachable() {
        return;
    }

    let adapter = OllamaAdapter::new("http://localhost:11434").unwrap();
    let req = GenerateRequest {
        model: "llama3.2".to_string(),
        prompt: "Reply with only the word pong.".to_string(),
        temperature: 0.0,
    };

    let response = adapter.generate(&req).unwrap();
    assert!(!response.text.is_empty(), "expected non-empty response from Ollama");
}
