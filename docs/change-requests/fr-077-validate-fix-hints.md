# FR-077: Рецепты починки в graph_validate (поле fix)

- **Статус:** в работе
- **Тип:** FR
- **Приоритет:** важно (UX агентного контура — сходимость агента на повторной валидации)
- **Владелец:** агент-сессия (Super Z), по запросу владельца
- **Источник:** `docs/dev-researches/archify-transfer-analysis.md` (карта переноса, T2) — практика Archify «repair receipt»: `{code, severity, subject, evidence, supportedFixes}`
- **Связанные задачи:** FR-032 (контракт кодов v1), CR-013 R5 (рецепт агента), skills v5, `user-docs/agent-recipe.md`
- **Создан:** 2026-09-28
- **Обновлён:** 2026-09-28

## Описание

Валидация модели (`graph_validate`, FR-032) возвращает стабильные коды
`E-*`/`W-*`, но агент обязан знать семантику каждого кода заранее, чтобы
чинить модель: «получил E-PORT-UNKNOWN — и что делать?». Archify решает это
полем `supportedFixes` — к каждой диагностике прилагается список поддерживаемых
контролов починки. Требуется поле `fix` в каждой проблеме отчёта: конкретный
рецепт (шаги существующими MCP-инструментами), выполняемый до повторной
валидации.

## Влияние

| Объект | Что меняется | Где в документации |
|---|---|---|
| ядро | `ValidationIssue` сериализуется с полем `fix` (вычисляется из кода) | `crates/canvas-core/src/validate.rs` (контракт FR-032 расширяется, не ломается) |
| MCP | `graph_validate`/`ln` ответ: issues несут `fix` — автоматически (сериализация ядра) | `crates/canvas-scene/src/mcp.rs:1248` |
| skills | пакет v5: схема ответа + правило «выполняйте fix» | `skills/canvasdesk-model-verify/SKILL.md`, `skills/CHANGELOG.md`, `skills/README.md` |
| рецепт агента | Шаг 6 дополнен полем и правилом цикла починки | `user-docs/agent-recipe.md` |

## Анализ

- `ValidationIssue` (`validate.rs:92`) — derive `Serialize` с `rename_all =
  snake_case`; конструкторов проблемы ~10 мест (validate.rs, тесты) —
  добавление ПОЛЯ структуры раздуло бы каждый.
- Коды — перечисление `IssueCode` (`validate.rs:56`): стабильный контракт,
  `as_str()` → `E-CYCLE`… Рецепт естественным образом keyed по коду —
  контекст (участники цикла, имена портов) уже в `message`.
- `graph_validate` (`canvas-scene/src/mcp.rs:1248`) сериализует issues
  `serde_json::to_value(issue)` как есть — расширенная сериализация ядра
  доезжает до MCP без изменений диспетчера.
- Контракт-тесты skills (`canvas-mcp/src/lib.rs`, `skills_*`) проверяют
  имена инструментов и call-позиции — упоминание `edge_delete` в тексте
  рецептов (без `{`) правила не задевает.

## Требуемые изменения

1. `canvas-core/src/validate.rs`: ручной `impl Serialize for
   ValidationIssue` — шесть полей, `fix: fix_hint(self.code)`; pub-функция
   `fix_hint(code) -> &'static str` с рецептами по всем семи кодам
   (ссылки только на существующие инструменты: edge_delete, edge_create,
   flow_set_kind, node_update_text, node_edit, node_get, template_list);
   докамодуля — раздел «Рецепты починки (FR-077)».
2. Тесты ядра: `issue_serializes_to_mcp_contract` (fix присутствует,
   называет инструмент), `every_code_has_fix_hint` (все семь кодов,
   непустые, без краевых пробелов).
3. `canvas-scene/src/tests.rs`: end-to-end фиксация — E-CYCLE fix
   упоминает edge_delete, W-UNUSED-SLOT fix упоминает `$N`, E-OVERLOAD
   fix непуст.
4. skills v5: SKILL.md (схема + правило), CHANGELOG.md, README.md (версия).
5. `user-docs/agent-recipe.md`: Шаг 6 — поле `fix`, правило «выполняйте fix
   каждой проблемы и повторяйте валидацию до valid: true».
6. `docs/change-requests/index-cr-fr.md`: строка FR-077.

## Точки входа

- `user-docs/agent-recipe.md` Шаг 6 — обновлён (п. 5).
- `skills/UPDATE-PROTOCOL.md` — правила не менялись (состав TOOLS прежний,
  изменилась семантика выхода — версия пакета поднята по протоколу).
- `docs/SPEC.md` §13 (контракты MCP) — расширение ответа задокументировано
  в FR-077; отдельная правка SPEC не требуется (поле аддитивное).

## Проверка

- [x] `cargo test -p canvas-core`: 20/20 validate-тестов (включая два новых).
- [x] `cargo test -p canvas-scene`: 126+6 зелёные, fix-ассерты end-to-end.
- [x] `cargo test -p canvas-mcp`: контракт-тесты skills_sync зелёные
  (упоминания инструментов в рецептах не ломают call-позиции).
- [x] `cargo clippy -p canvas-core -p canvas-scene -p canvas-mcp -- -D warnings`,
  `cargo fmt --check`.
- [ ] Живая MCP-сессия (wasm-гейт `scripts/mcp_wasm_gate.sh`) — вне сессии,
  ручная проверка: `graph_validate` на модели с циклом → в issues[0].fix
  рецепт с edge_delete.

## История изменений

- `2026-09-28` — агент: создан документ (анализ, изменения, проверка), статус `в работе`.
- `2026-09-28` — агент: реализовано (ручной Serialize + fix_hint, тесты ядра/сцены, skills v5, agent-recipe); статус `выполнено`.

## Источники истины

- `crates/canvas-core/src/validate.rs` (impl Serialize, `fix_hint`, докамодуль §Рецепты).
- `crates/canvas-scene/src/tests.rs` (`mcp_graph_validate_*` — fix-ассерты).
- `skills/canvasdesk-model-verify/SKILL.md` (схема ответа), `skills/CHANGELOG.md` (v5).
- Archify: `archify/renderers/shared/diagnostics.mjs` — прецедент формата
  `{code, severity, subject, evidence, supportedFixes}`.
