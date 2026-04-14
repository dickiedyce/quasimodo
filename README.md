# quasimodo

Native macOS-first Rust implementation inspired by hunch, with a local-first LLM provider boundary.

## Requirements

- macOS 15.7.5
- Rust toolchain (`cargo`)
- Ollama running locally (default endpoint: `http://localhost:11434`)

## Build

```bash
cargo build
```

## Test

```bash
cargo test
```

## Run Against Local Ollama

Current library code provides an Ollama adapter boundary and request/response handling.

Typical local workflow:

1. Start Ollama locally.
2. Ensure your selected model is available in Ollama.
3. Use `OllamaAdapter::new("http://localhost:11434")` and call `generate` with a `GenerateRequest`.

## Design Notes

- Local-only endpoint validation is enforced (localhost / 127.0.0.1).
- Provider behavior is isolated behind the `ProviderAdapter` trait.
- Health check uses Ollama `/api/tags`.
- Generation uses Ollama `/api/generate` with non-streaming JSON payload.
