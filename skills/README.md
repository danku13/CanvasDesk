# CanvasDesk MCP Skills — package for AI agents

A skill package for AI agents (MCP clients) working with CanvasDesk — a
visual mathematical modeling system. The package is synchronized with the
MCP tool registry: **43 tools**; the match is verified by the `skills_sync`
contract test in the `canvas-mcp` crate (see
[UPDATE-PROTOCOL.md](UPDATE-PROTOCOL.md)).

The package is self-contained: it can be copied wholesale, published as a
separate repository, or distributed as a catalog inside the CanvasDesk
repository. Links inside skills are relative (they work when a skill folder
is copied entirely).

## Contents

| Skill | Purpose |
|---|---|
| `canvasdesk-mcp` | Connection (transport, offline/reconnect), invariants, canvas exploration, the "task → skill" map |
| `canvasdesk-model-build` | Model building: nodes, value edges with port addressing, the atomic graph_apply batch, the Instagram MVP reference |
| `canvasdesk-model-verify` | Verification: flow_recalc, lineage, explain_number (textual explanation), graph_validate (error codes), analyze_bottlenecks |
| `canvasdesk-whatif` | What-if scenarios: substitutions, deltas, apply/reset |

The full signature catalog — `canvasdesk-mcp/references/tools.md`.

## Installation for an agent

A skill activates when its folder is placed in the agent's skill directory:

1. Copy the needed folders (or the whole `skills/`) into your agent's skill
   directory (e.g. `~/.claude/skills/` or the equivalent).
2. Point the MCP host at the CanvasDesk server:

```json
{ "mcpServers": { "canvasdesk": { "command": "canvasdesk", "args": ["mcp"] } } }
```

3. Standalone option: `command = "canvasdesk-mcp"` (no arguments); if the
   application is unavailable, the bridge starts the GUI neighbor itself
   (`--no-spawn` disables this).

Skills can be installed selectively: each is self-sufficient. The base
`canvasdesk-mcp` is always recommended — the others reference its
invariants.

## Keeping the package current

CanvasDesk MCP tools evolve; the skills are a derivative of the registry.
Rules and the step-by-step update protocol —
[UPDATE-PROTOCOL.md](UPDATE-PROTOCOL.md); change history —
[CHANGELOG.md](CHANGELOG.md). Synchronization check:

```
cargo test -p canvas-mcp skills
```

## Version

Package v7 — 43 tools; synchronized with the `TOOLS` registry (main).
The package version is bumped on any change to the tool set or semantics
(see CHANGELOG).
