---
applyTo: "**/*"
description: "Minimal Code Journal workflow rules to keep logs concise, idempotent, and useful."
---

# Code Journal Minimal Rules

## Keep Entries Small

- Prefer short, high-signal entries over long narrative logs.
- Use minimal mode for quick fixes.

## Idempotency

- Do not create duplicate session-start blocks in the same day unless the topic changed.
- Do not duplicate backlog items that already exist.

## Completion Coupling

- When a backlog item is marked done, include a linked session note and one line of evidence (for example, test result or validation step).

## Failure Safe

- If MCP tools are unavailable, continue implementation work and list deferred journal updates in the response.
