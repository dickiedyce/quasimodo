---
name: start-session-codejournal
description: "Start a coding session with project context and a minimal daily log entry."
argument-hint: Project name and one-line goal
---

Use this prompt to begin work with lightweight Code Journal discipline.

## Steps

1. Identify the project from the prompt argument.
2. Load context with `mcp_codejournal_project_context`.
3. If the project is missing, list projects and continue coding without journal operations.
4. Append a minimal daily note entry:

```markdown
## <Project> -- <topic>

- Started: <HH:MM>
- Goal: <one-line goal>
```

5. Read open backlog items and present the top 3 by priority/impact.
6. Keep output concise: context summary, first action, and risks.

## Minimal mode

For tiny tasks, skip long summaries and log only start + one completion line.
