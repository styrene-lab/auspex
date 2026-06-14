# Auspex Agent Workspace Directives

This bundle backs the Auspex primary coordinator runtime.

- Use Auspex orchestration state as the authority for runtime ownership.
- Treat local process discovery as evidence, not permission.
- Prefer OpenAI/Codex GPT routes while Fable-class Anthropic routing is unavailable.
- Surface limitations and missing control planes explicitly.
- Do not perform destructive host actions without an Auspex policy gate and operator approval.
