# polyrc

Convert AI coding agent configurations between tools — Cursor, Windsurf, Claude Code, GitHub Copilot, Gemini CLI, and Google Antigravity.

## Concept

Different AI coding agents use different config formats (`.cursor/rules/*.mdc`, `CLAUDE.md`, `.windsurf/rules/*.md`, etc.) for the same underlying information: coding style, project conventions, file structure rules, tone preferences.

`polyrc` defines a neutral **interlingua** (intermediate representation) and converts between all formats using hub-and-spoke: only 2N converters instead of N².

```
cursor  ──┐                    ┌──▶ claude
windsurf ─┤──▶  polyrc IR  ────┤──▶ copilot
gemini  ──┘                    └──▶ antigravity
```

Rules are stored as structured YAML in a **local git-backed store**, so your conventions travel with you across machines.

---

## Supported formats

| Format | Config location | Notes |
|---|---|---|
| **Cursor** | `.cursor/rules/*.mdc` | YAML frontmatter: `description`, `globs`, `alwaysApply` |
| **Windsurf** | `.windsurf/rules/*.md` | Plain markdown; 6k char/file, 12k total limits |
| **GitHub Copilot** | `.github/copilot-instructions.md` + `.github/instructions/*.instructions.md` | `applyTo` frontmatter for path-scoped rules |
| **Claude Code** | `CLAUDE.md` + `.claude/rules/*.md` | Single file or per-rule directory |
| **Gemini CLI** | `GEMINI.md` | Single file |
| **Google Antigravity** | `.agent/rules/*.md` | Also checks legacy `.agents/rules/` |

---

## Installation

```bash
cargo install polyrc
```

Requires git to be installed for store operations.

---

## Quick start

### One-shot conversion (no store)

```bash
# Convert Cursor rules to Claude Code format
polyrc convert --from cursor --to claude

# Convert Gemini rules to Windsurf (different directories)
polyrc convert --from gemini --to windsurf --input ~/my-project --output ~/my-project

# Preview without writing
polyrc convert --from cursor --to copilot --dry-run

# List supported formats
polyrc list-formats
```

### With a store

The store is a local git repo that persists your rules as interlingua YAML. All format conversions go through the store, giving you version history and cross-machine sync.

**Set up the store:**

```bash
# Local-only store
polyrc init

# Clone an existing store from a remote repo
polyrc init --repo git@github.com:you/my-rules

# Custom store location
polyrc init --repo git@github.com:you/my-rules --store ~/dev/my-rules
```

**Save rules to the store:**

```bash
# Read Cursor rules from the current project, save to store under project "myapp"
polyrc push-format --format cursor --project myapp

# From a different directory
polyrc push-format --format claude --project myapp --input ~/projects/myapp
```

Each `push-format` automatically commits the changes to the local git repo.

**Apply rules from the store:**

```bash
# Write stored rules for "myapp" as Claude Code format
polyrc pull-format --format claude --project myapp

# Into a specific directory
polyrc pull-format --format cursor --project myapp --output ~/projects/myapp
```

**Convert via store (push + pull in one step):**

```bash
polyrc convert --from cursor --to claude --project myapp
```

**Sync with a remote:**

```bash
# Push local store commits to origin
polyrc push-store

# Pull from origin (applies IR-level merge on conflicts)
polyrc pull-store
```

**Manage projects:**

```bash
polyrc project list
polyrc project rename myapp my-renamed-app
```

---

## Workflow: new machine setup

```bash
# Install
cargo install polyrc

# Clone your central rules repo
polyrc init --repo git@github.com:you/my-rules

# Apply your rules for a project
cd ~/projects/myapp
polyrc pull-format --format cursor --project myapp
```

## Workflow: sync changes back

```bash
cd ~/projects/myapp
# After editing .cursor/rules/...

polyrc push-format --format cursor --project myapp
# → converts to IR, saves to ~/polyrc/store, git commits

polyrc push-store
# → git push origin
```

---

## The interlingua (IR)

Each rule is stored as a YAML file in `~/polyrc/store/rules/<project>/`:

```yaml
scope: project          # user | project | path
activation: always      # always | glob | on_demand | ai_decides
name: typescript-style
content: |
  Use TypeScript strict mode. Prefer interfaces over type aliases for object shapes.
  Always add explicit return types to exported functions.
id: 3f2a1b4c-...        # stable UUID assigned on first push
project: myapp
source_format: cursor
created_at: "2026-02-25T10:00:00Z"
updated_at: "2026-02-25T10:00:00Z"
store_version: "1"
```

Optional fields: `globs` (for glob-activated rules), `description` (for AI-decided rules).

**Content is opaque** — polyrc wraps markdown but never parses or modifies it.

---

## Scope and activation

Rules carry two axes of metadata through all conversions:

| Scope | Meaning |
|---|---|
| `user` | User-global rules (`~/polyrc/store/rules/_user/`) |
| `project` | Project-wide rules (default) |
| `path` | Path/glob-scoped rules |

| Activation | Meaning |
|---|---|
| `always` | Always injected into context |
| `glob` | Injected when a matching file is open |
| `on_demand` | User must invoke manually |
| `ai_decides` | AI decides based on `description` |

Filter by scope on any command:

```bash
polyrc push-format --format cursor --project myapp --scope project
polyrc pull-format --format claude --scope user
```

---

## Store merge

When `pull-store` encounters conflicting rules (same rule edited on two machines), polyrc applies an IR-level merge:

- Rules are matched by stable UUID.
- Last-write-wins by `updated_at` timestamp.
- Conflicts are reported as warnings — no silent data loss.

---

## License

MIT
