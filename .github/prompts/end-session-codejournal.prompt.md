---
name: end-session-codejournal
description: "Close a coding session with outcomes, linked notes, and backlog housekeeping."
argument-hint: Project name and brief topic
---

Use this prompt to end work with a compact, high-signal session record.

## Steps

1. Identify completed goals and outstanding follow-ups.
2. Create a session note at `Projects/<Project>/YYYY-MM-DD <topic>.md` with:
   - Context
   - Goals (checkboxes)
   - Work Log (timestamped)
   - Outcomes
3. Append to daily note:

```markdown
- <HH:MM> -- <what was completed>
- Session note: [[Projects/<Project>/YYYY-MM-DD <topic>]]
```

4. Backlog housekeeping:
   - Mark finished item(s) done.
   - Add any newly discovered follow-up item(s).
5. If MCP tools fail, do not block coding work; report deferred journal actions clearly.

## Minimal mode

For small tasks, write a short Outcomes section and skip verbose work logs.
