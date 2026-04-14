---
applyTo: "**/*"
description: "Guidance for implementing a native macOS 15.7.5-compatible version of hunch with local-first LLM runtime choices and Rust/Go/Zig preference."
---

# Native macOS hunch Implementation Rules

## Objective

Build this repository as a native implementation of the ideas in `https://github.com/es617/hunch` that runs on macOS 15.7.5 without Docker or cloud-only dependencies.

## Platform Constraints

- Target: macOS 15.7.5.
- Prefer native binaries and local services.
- Avoid solutions that require Linux-only runtime behavior.

## LLM Runtime Preference

- Prefer Ollama as the default local inference runtime.
- Keep the LLM provider behind a small adapter boundary so alternatives can be swapped later.
- If Ollama is not sufficient for a required capability, choose another local/native option.
- Do not use hosted LLM APIs as a fallback for this project.

## Language Preference

- Use implementation languages in this order:
  1. Rust
  2. Go
  3. Zig
- Do not introduce Node.js or Python as primary runtime dependencies.

## Architecture Guidance

- Keep components modular:
  - Core logic independent from transport/UI.
  - Provider integration isolated behind interfaces.
  - Configuration centralized and explicit.
- Prefer simple local IPC/HTTP patterns over distributed orchestration.

## Dependency Policy

- Choose mature, well-maintained libraries.
- Minimize transitive dependencies and startup complexity.
- Document any dependency that weakens native macOS portability.

## Delivery Expectations

- Include clear build and run steps for macOS.
- Favor reproducible local setup.
- Add tests for integration boundaries, especially LLM provider adapters.
