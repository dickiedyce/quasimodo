# quasimodo

Native macOS-first Rust implementation inspired by [hunch](https://github.com/es617/hunch), with a local-first LLM provider boundary.

## Requirements

- macOS 15.7.5
- Rust toolchain (`cargo`)
- Ollama running locally (default endpoint: `http://localhost:11434`)

## Build

```bash
cargo build
```

Optional install location used by hook examples:

```bash
./scripts/install_local.sh
```

The script copies:

- `target/debug/quasimodo` -> `$HOME/.local/bin/quasimodo`
- `hooks/quasimodo.zsh` -> `$HOME/.quasimodo/hooks/quasimodo.zsh`

Optional custom destinations:

```bash
QUASIMODO_BIN_DIR="$HOME/.local/bin" QUASIMODO_HOOK_DIR="$HOME/.quasimodo/hooks" ./scripts/install_local.sh
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
# Build/refresh the local retrieval database first
mkdir -p "$HOME/.quasimodo"
cargo run --bin build-bank -- "$HOME/.quasimodo/tldr_bank.db"

# Command generation from plain English
cargo run -- --prompt "find files changed in the last hour" --bank "$HOME/.quasimodo/tldr_bank.db"

# Show CLI help
cargo run -- --help

# Command-not-found helper mode
cargo run -- --notfound ip --bank "$HOME/.quasimodo/tldr_bank.db"

# Error explanation helper mode
cargo run -- --explain "Command: git push -- Exit code: 128"

# Pipe mode
echo "find files changed in the last hour" | cargo run -- --stdin --bank "$HOME/.quasimodo/tldr_bank.db"

# Optional: majority-vote consistency mode
cargo run -- --prompt "show disk usage" --samples 3 --temperature 0.3 --bank "$HOME/.quasimodo/tldr_bank.db"

# Multi-turn with persisted history
cargo run -- --prompt "show largest files" --history-file ./session.json --system "Return only shell commands"
cargo run -- --prompt "now limit to current folder" --history-file ./session.json --system "Return only shell commands"

# Teach a corrected example (stored permanently; beats TLDR entries in search)
quasimodo --teach "date 90 days ago" \
          --command "date -v -90d '+%Y-%m-%d'" \
          --bank "$HOME/.quasimodo/tldr_bank.db"

# List all taught examples
quasimodo --list-taught --bank "$HOME/.quasimodo/tldr_bank.db"

# Delete the first taught example whose description matches a substring
quasimodo --delete-taught "90 days" --bank "$HOME/.quasimodo/tldr_bank.db"

# Quality benchmark (A/B: no-retry vs retry)
cargo run --bin quality_benchmark -- "$HOME/.quasimodo/tldr_bank.db"
```

## Teaching Corrections

If quasimodo returns a wrong or suboptimal command, store the correct one with `--teach`:

```bash
quasimodo --teach "<natural language description>" \
          --command "<correct shell command>" \
          --bank "$HOME/.quasimodo/tldr_bank.db"
```

User-taught examples are stored in a separate `user_examples` table and always ranked above TLDR entries in retrieval. They survive `build-bank` rebuilds.

To inspect all stored user-taught examples:

```bash
quasimodo --list-taught --bank "$HOME/.quasimodo/tldr_bank.db"
```

Output format is one tab-separated line per entry: `<description><TAB><command>`.

To remove a stored user-taught example, pass a description substring. The first matching entry in description order is deleted:

```bash
quasimodo --delete-taught "90 days" --bank "$HOME/.quasimodo/tldr_bank.db"
```

Retrieval precedence is now three-tier:

1. User overrides (`--teach` examples)
2. macOS TLDR examples (`pages/osx`)
3. Common TLDR examples (`pages/common`)

`build-bank` ingests only `pages/osx` and `pages/common` so Linux-only commands do not outrank macOS-native commands.

Example — macOS `date` arithmetic:

```bash
quasimodo --teach "what is the date in 3 weeks" \
          --command "date -v +3w '+%Y-%m-%d'" \
          --bank "$HOME/.quasimodo/tldr_bank.db"

quasimodo --teach "date 90 days ago" \
          --command "date -v -90d '+%Y-%m-%d'" \
          --bank "$HOME/.quasimodo/tldr_bank.db"
```

## zsh Hooks

Add the following lines to your `~/.zshrc` so hooks load automatically in every interactive shell:

```bash
# optional (these are also the hook defaults):
# export QUASIMODO_BIN="$HOME/.local/bin/quasimodo"
# export QUASIMODO_BANK="$HOME/.quasimodo/tldr_bank.db"
# optional:
# export QUASIMODO_SYSTEM="Return only shell commands"
# export QUASIMODO_HISTORY="$HOME/.quasimodo/session.json"
# export QUASIMODO_TRAP_ERRORS="1"
# export QUASIMODO_KEY="^]"
# export QUASIMODO_ALT_KEY=""
source "$HOME/.quasimodo/hooks/quasimodo.zsh"
```

Recommended single-key binding on macOS terminals:

```bash
export QUASIMODO_KEY="^]"
export QUASIMODO_ALT_KEY=""
source "$HOME/.quasimodo/hooks/quasimodo.zsh"
```

Key notation examples:

- `^]` = Ctrl+]
- `^G` = Ctrl+G
- `^X^G` = Ctrl+X then Ctrl+G

Reload your shell after saving:

```bash
source ~/.zshrc
```

This enables:

1. `Ctrl+]` (or your configured key) rewrites natural language in your shell buffer into a command.
2. `command_not_found_handler` suggestions via `--notfound`.
3. `TRAPZERR` one-line explanations via `--explain`.

If you use `./scripts/setup_aliases.sh`, it also installs these shell shortcuts:

- `qq` for `--prompt`
- `qd` for `--describe`
- `qe` for `--explain`
- `qh` for `--help`
- `qt` for `--teach`
- `qlt` for `--list-taught`
- `qrm` for `--delete-taught`
- `qnf` for `--notfound`
- `qb` for `build-bank`

To disable error trapping/explanations while keeping other hooks enabled:

```bash
export QUASIMODO_TRAP_ERRORS="0"
source "$HOME/.quasimodo/hooks/quasimodo.zsh"
```

If your key binding does not trigger, run this quick check:

```bash
quasimodo --prompt "show current directory" --bank "$HOME/.quasimodo/tldr_bank.db"
```

If that fails, verify Ollama is running and that your DB exists at `$HOME/.quasimodo/tldr_bank.db`.

## Regenerate the DB

Rebuild the local retrieval database whenever you want fresher examples:

```bash
mkdir -p "$HOME/.quasimodo"
cargo run --bin build-bank -- "$HOME/.quasimodo/tldr_bank.db"
```

If your `~/.zshrc` uses `QUASIMODO_BANK="$HOME/.quasimodo/tldr_bank.db"`, hooks will automatically use the regenerated database on the next shell session.

## Design Notes

- Local-only endpoint validation is enforced (localhost / 127.0.0.1).
- Provider behavior is isolated behind the `ProviderAdapter` trait.
- Health check uses Ollama `/api/tags`.
- Generation uses Ollama `/api/generate` with non-streaming JSON payload.
