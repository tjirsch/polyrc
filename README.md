# polyrc

Convert AI coding agent configurations between tools — Cursor, Windsurf, Claude Code, GitHub Copilot, Cline, and more.

## Concept

Different AI coding agents use different config formats (`.cursorrules`, `CLAUDE.md`, `windsurf_rules`, etc.) for the same underlying information: coding style, conventions, file structure rules, tone preferences. `polyrc` defines a neutral intermediate representation and converts between formats.

```
cursor → polyrc IR → claude
windsurf → polyrc IR → copilot
```

## Status

Early development. Name reserved.

## License

MIT
