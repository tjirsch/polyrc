# polyrc

A Rust CLI tool that converts AI coding agent configurations between tools using a neutral interlingua IR.

## What we're building

Instead of N² converters between every pair of tools, polyrc uses hub-and-spoke:
every format maps to/from a canonical intermediate representation (IR).

```
cursor → IR → claude
windsurf → IR → copilot
gemini → IR → antigravity
```

## Project context

- **Language**: Rust (2024 edition)
- **Crate type**: binary (`polyrc` CLI)
- **Published**: reserved on crates.io at v0.1.0

Read `PLAN.md` for full project plan and next steps.
Read `docs/formats.md` for the complete format survey of all 6 tools.

## Tools in scope (v1)

Cursor, Windsurf, GitHub Copilot, Claude Code, Gemini CLI, Google Antigravity.

## Current status

Format research is complete. Next task: define the IR schema and converter trait.

## Key design decisions

- IR is a collection of `Rule` structs — one per logical rule, with scope/activation metadata
- Content is opaque markdown — the IR wraps it, does not parse it
- Scope: `user | project | path`
- Activation: `always | glob | on_demand | ai_decides`
- Converter pattern: `Parser` trait (read) + `Writer` trait (write), one impl per tool

## Common commands

```bash
cargo build
cargo test
cargo run -- --help
