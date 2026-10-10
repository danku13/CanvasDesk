# MCP Skills Update Protocol

The skills in `skills/` are a derivative of the CanvasDesk MCP tool
registry. The sources of truth are the `TOOLS` constant in
`crates/canvas-mcp/src/lib.rs` (the same thing the live server's
`tools/list` returns) and the tool semantics in
`crates/canvas-scene/src/mcp.rs`. When MCP functionality changes, the
skills are updated IN THE SAME commit — the contract test catches any
desync.

## The skills_sync contract test (the canvas-mcp crate)

The `skills_*` tests in `crates/canvas-mcp/src/lib.rs` embed all the
markdown files of the package (`include_str!`) and check four rules:

1. **Catalog completeness:** every name from `TOOLS` is present in
   `skills/canvasdesk-mcp/references/tools.md`.
2. **Skill coverage:** every name from `TOOLS` is mentioned in at least
   one `SKILL.md` (not only in the catalog) — a new tool must fall into
   the responsibility zone of one of the skills.
3. **README counter:** `skills/README.md` contains the current count
   («N инструмент…» / “N tools”).
4. **Call positions are valid:** every markdown backtick token followed
   by `{` (the call form `<tool>` {params}) must be a tool name from
   `TOOLS` or a `graph_apply` operation from the `CALL_POSITION_OPS`
   list in the test (currently `param_set` — a batch operation, not an
   MCP tool). It catches calls of deleted/renamed tools.

Run: `cargo test -p canvas-mcp skills` (part of the full `cargo test`
gate and CI).

## Step-by-step protocol when MCP changes

1. **You changed the registry or the semantics** (`TOOLS` in canvas-mcp,
   the tool logic in canvas-scene/src/mcp.rs, response formats).
2. **Catalog:** update `skills/canvasdesk-mcp/references/tools.md` —
   add/fix the tool's row; check the grouping
   (exploration/nodes/edges/batch/templates/verification/what-if).
3. **Responsibility zone:** update the affected `SKILL.md`. A new tool
   must be mentioned in at least one `SKILL.md` (rule 2), ideally with
   its call signature and one or two examples. A response format change
   → update the structure example in `canvasdesk-model-verify` (flow)
   or in the corresponding skill.
4. **Counter:** fix the “N tools” count in `skills/README.md` and the
   package version there.
5. **CHANGELOG:** an entry in `skills/CHANGELOG.md` (version, date,
   what changed, related tools).
6. **Gates:** `cargo test -p canvas-mcp` (skills_sync green), then the
   full repository routine — fmt, clippy, all tests,
   `scripts/wasm_gate.sh`, `scripts/mcp_wasm_gate.sh`.

## Special cases

- **Renaming/deleting a tool:** rule 4 will catch calls of the old name
  in call position; check the prose and tables manually
  (`rg "старое_имя" skills/`). Update the catalog, the skills and the
  CHANGELOG in one commit.
- **A new `graph_apply` operation** (one that does not coincide with a
  tool name): add it to `CALL_POSITION_OPS` in the test — deliberately,
  with a comment.
- **Error code changes** (`graph_validate`, batch operations): update
  the tables in `canvasdesk-model-build`/`canvasdesk-model-verify` —
  the codes are declared as a stable contract of the recipe.
- **New schemes/templates in the registries** (assets/): the skills do
  not list them by name (the agent reads `schemes_list`/`template_list`
  at runtime); update the skills only when tool semantics change.

## Skill style (maintain)

- Language — English (owner language policy 2026-10-10, issue #24;
  previously Russian); tool/field names — as in the registry (Latin
  script).
- Calls in the canonical form: `<tool>` {params} — one spelling across
  the package (rule 4 uses it).
- Numeric examples — only canonical references (ADR-0005 Instagram MVP,
  gallery schemes) with ±1 % oracles; the numbers are updated together
  with the scene tests.
- Frontmatter: `name` (the identifier), `description` (triggers RU+EN),
  `version` (an integer, +1 on a semantic change of the skill).
- The size of a `SKILL.md` — up to ~250 lines; details go into
  references/.
