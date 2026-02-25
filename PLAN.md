# polyrc — Project Plan

## What is this?

`polyrc` converts AI coding agent configurations between tools using a neutral interlingua IR.
Instead of N² converters, we use hub-and-spoke: every format maps to/from a canonical IR.

```
cursor → IR → claude
windsurf → IR → copilot
gemini → IR → antigravity
```

## Status

- [x] Name reserved on crates.io (`cargo publish` done)
- [x] Repo created: github.com/tbuechler/polyrc (verify username)
- [x] Format research complete → `docs/formats.md`
- [ ] Define IR schema (TOML or YAML)
- [ ] Define converter trait/API
- [ ] Implement parsers (one per format)
- [ ] Implement writers (one per format)
- [ ] CLI: `polyrc convert --from cursor --to claude`

## Tools in scope (v1)

| Tool | Read | Write | Notes |
|---|---|---|---|
| Cursor | `.cursor/rules/*.mdc` | same | YAML frontmatter |
| Windsurf | `.windsurf/rules/*.md` | same | plain markdown |
| Copilot | `.github/copilot-instructions.md` | same | plain markdown |
| Claude Code | `CLAUDE.md` | same | plain markdown |
| Gemini CLI | `GEMINI.md` | same | plain markdown |
| Antigravity | `.agent/rules/*.md` | same | plain markdown |

## Next step: IR Schema

The IR is a TOML/YAML file (or Rust struct) representing a single rule with:

```
scope:      user | project | path
activation: always | glob | on_demand | ai_decides
globs:      optional list of glob patterns
name:       optional string
description: optional string (used as AI trigger in some tools)
content:    raw markdown string
```

A config is a collection of rules + tool metadata.

## Converter API (sketch)

```rust
trait Parser {
    fn parse(path: &Path) -> Result<Vec<Rule>>;
}

trait Writer {
    fn write(rules: &[Rule], target: &Path) -> Result<()>;
}

// One impl per tool: CursorParser, WindsurfParser, ClaudeParser, etc.
```

## CLI design

```
polyrc convert --from cursor --to claude [--scope project|user]
polyrc convert --from gemini --to copilot --output .github/
polyrc list-formats
```

## Out of scope for v1

- Antigravity Skills (on-demand structured packages)
- Antigravity Workflows
- Windsurf Memories (auto-generated, not user-authored)
- Copilot Prompt files
- Syncing (keeping configs in parallel across tools)

## Interlingua note

The IR format is called the "interlingua" — a pivot format nothing uses natively,
that exists only as a translation hub. The pattern is hub-and-spoke (2N transformers)
rather than all-pairs (N² transformers). See: compiler IR, translation interlingua.
