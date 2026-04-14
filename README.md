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

## CLI Usage

```bash
# Command generation from plain English
cargo run -- --prompt "find files changed in the last hour" --bank ./tldr_bank.db

# Command-not-found helper mode
cargo run -- --notfound ip --bank ./tldr_bank.db

# Error explanation helper mode
cargo run -- --explain "Command: git push -- Exit code: 128"

# Pipe mode
echo "find files changed in the last hour" | cargo run -- --stdin --bank ./tldr_bank.db

# Optional: majority-vote consistency mode
cargo run -- --prompt "show disk usage" --samples 3 --temperature 0.3 --bank ./tldr_bank.db
```

## zsh Hooks

```bash
export QUASIMODO_BIN="$PWD/target/debug/quasimodo"
export QUASIMODO_BANK="$PWD/tldr_bank.db"
source "$PWD/hooks/quasimodo.zsh"
```

This enables:

1. `Ctrl+G` to rewrite natural language in your shell buffer into a command.
2. `command_not_found_handler` suggestions via `--notfound`.
3. `TRAPZERR` one-line explanations via `--explain`.

## Design Notes

- Local-only endpoint validation is enforced (localhost / 127.0.0.1).
- Provider behavior is isolated behind the `ProviderAdapter` trait.
- Health check uses Ollama `/api/tags`.
- Generation uses Ollama `/api/generate` with non-streaming JSON payload.
