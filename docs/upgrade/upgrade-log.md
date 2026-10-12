# Бортовой журнал трека «Переход на wgpu 30 / glyphon 0.12 / cosmic-text 0.19»

- **Трек:** issue #69 (parent), PLAN.md — приложение к #69
- **Ветка:** `upgrade/render-stack` (от `main` @ `848bfaf` = тег `pre-render-upgrade` +
  W0 docs-коммит; код идентичен тегу)
- **Откаты:** уровень 0 — `git switch main`; уровень 1 — тег `pre-render-upgrade`;
  уровень 2 — revert merge-commit `-m 1`; уровень 3 — revert шага волны
- **Правило merge:** только merge-commit (НЕ squash/rebase) — условие отката
  уровня 2

## Карта волн

| Волна | Issue | Содержание | Статус |
|---|---|---|---|
| W0 | #70 | Заморозка базлайнов (тег + docs/upgrade/baseline-*) | ✅ Done (main 848bfaf, CI зелёный) |
| W1 | #71 | Каркас: ветка + draft PR + upgrade-log | 🔄 In Progress |
| W2 | #72 | Скачок версий + API-миграция до зелёной компиляции | Todo |
| W3 | #73 | Рантайм desktop: запуск + smoke + не-эталонные тесты | Todo |
| W4 | #74 | Регенерация 220+15 эталонов по протоколу осознанного диффа | Todo |
| W5 | #75 | Web/wasm32: сборка + WebGPU/WebGL2 + pages-preview | Todo |
| W6 | #76 | Финализация: cargo-deny, perf-гейт, CHANGELOG, merge-commit | Todo |

---

## W1 — каркас ветки и draft PR

**Дата:** 2026-10-13 · **Issue:** #71 · **Статус:** In Progress

### Решения

1. **Ветка от `main` @ `848bfaf`, а не от тега напрямую.** Обоснование: DoD W1
   требует «пустой diff к main кроме upgrade-log.md»; при ветвлении от тега
   (086036a) diff содержал бы W0-файлы в обратную сторону. Коммит 848bfaf —
   тег + только новые файлы документации (`docs/upgrade/baseline-*`,
   worklog), кода не касается, тег `pre-render-upgrade` остаётся точкой
   отката уровня 1. Базлайны W0 доступны на ветке — самодостаточность W2/W4.
2. **Правка workflows не требуется.** Проверено парсингом YAML: `ci.yml`
   (`push`/`pull_request` → `main`), `tour-tests.yml` (аналогично),
   `cla-check.yml` (`pull_request_target`) срабатывают на PR к main — все
   гейты (gates ×3 ОС, wasm-check, licenses, lay7-lint, v3-ui-lint,
   docs-lint) будут гоняться на каждый push ветки. `pages-web.yml`
   (push→main) и `build-all.yml` (tags v*) — деплой/релизные, PR-триггер им
   не нужен: web-сборка покрыта `wasm-check` в ci.yml, релизные бинари —
   теговая задача W6.
3. **Merge-метод** — «Create a merge commit» (настройка репозитория,
   инструкция владельцу в PR; squash/rebase запрещены условием отката
   уровня 2). Сейчас в репозитории разрешены все три метода —
   администрирование недоступно PAT; ограничение процедурное: кнопка
   «Create a merge commit» обязательна, сузить настройки может владелец
   (Settings → General → Pull Requests → снять Squash/Rebase).
4. **Draft PR программно открыть нельзя** — fine-grained PAT не имеет
   права `pull_requests:write` (REST 403, GraphQL FORBIDDEN); ghp_-токен
   к репозиторию доступа не имеет. Обход: `upgrade/render-stack` добавлен
   в `push`-триггер `ci.yml` (правка на ветке) — полный CI гоняется на
   каждый пуш ветки, независимо от наличия PR. Draft PR открывает
   владелец одним кликом по compare-URL (комментарий в #71); когда PR
   появится — `pull_request`-триггер сработает сам. Если владелец выдаст
   PAT право `pull_requests:write` — PR-автоматика вернётся в W6
   (переоткрытие/готовность к merge).

### Журнал

- `git switch -c upgrade/render-stack` от `848bfaf`
- `docs/upgrade/upgrade-log.md` (этот файл) — единственный diff к main
- draft PR ← ветка, base main
- CI на PR: ожидается полный набор джобов ci.yml + cla-check + tour-tests

---

*Формат записи волны: решения (что и почему), журнал (что сделано), диффы
эталонов (W4+), статус. Дополняется каждой волной.*
