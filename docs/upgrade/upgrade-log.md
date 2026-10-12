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

---

## W2 — скачок версий + API-миграция до зелёной компиляции

**Дата:** 2026-10-13 · **Issue:** #72 · **Статус:** ✅ ЗАВЕРШЕНА — CI run
38322569741 полностью зелёный (12/12 джобов: gates ×3 ОС, wasm-check,
licenses, docs/lay7/v3-ui линты, llm-proxy, artifacts ×3)

### Решения

1. **Коммиты 2a/2b объединены в один атомарный deps-W2.** Раздельные коммиты
   (bump+cosmic-text / wgpu) НЕ компилируются по отдельности: уже после bump
   wgpu 30 ломает gpu.rs (Maintain, TexelCopy*, PollType), а cosmic-text 0.19
   ломает text.rs — «скачок» по определению трека неделим. Откат: revert
   коммита целиком (уровень 3) или тег `pre-render-upgrade` (уровень 1).
2. **Suboptimal-кадр рисуется** (`Success | Suboptimal => frame`): паритет
   поведения wgpu 22, где Suboptimal приходил как Ok(frame). Переконфигурация
   — ветками Outdated/Lost на следующем кадре (configure при живом кадре
   паникует — документация wgpu 30).
3. **highlight(): `.next()`** — итератор спанов 0.19 свёрнут до первого
   (паритет прежнего Option<(f32,f32)>). Мульти-спановая отрисовка выделений
   (улучшение для bidi) — осознанное решение W4, задокументировано.
4. **Desktop Instance без display-handle** (`new_without_display_handle()`) —
   паритет wgpu 22 (тогда поля display не существовало). wgpu-hal GLES имеет
   фолбэки XCB/surfaceless/default. РИСК W3: GL на Wayland может потребовать
   проброса OwnedDisplayHandle от EventLoop через параметр Renderer::new —
   решается по факту рантайм-проверки.
5. **MSRV 1.80→1.89 разблокировал clippy-линты** на СТАРОМ коде: 15 правок за
   пределами миграции (map_or→is_none_or ×11, chunks_exact→as_chunks ×4,
   % →is_multiple_of ×1) в canvas-core/ui/app/web — иначе CI-гейт
   `clippy -D warnings` красный.
6. **deny.toml:** rustybuzz (RUSTSEC-2026-0206) ушёл из дерева (harfrust в
   cosmic-text 0.19) — игнор снят. paste (RUSTSEC-2024-0436) ВОЗВРАЩЁН:
   wgpu 30 его больше не тянет, но `--all-features` получает через
   statrs 0.17 → nalgebra 0.32 → simba 0.8 (фича statrs в canvas-core);
   ttf-parser 0.25.1 — одна копия, игнор сохранён.
7. **Рантайм-адаптация 0.19 (сверх брифа — для зелёного CI):**
   - `Action::Indent` при `Selection::None` теперь вставляет таб НА
     КУРСОРЕ (0.12 индентил строку) — FR-100 требует индент строки:
     ручной путь в edit.rs (вставка пробелов в начало строки, каретка
     сдвигается; путь с выделением — по-прежнему Action::Indent)
   - `LayoutRun::highlight()` 0.19 размечает выбранными строки ЗА
     пределами диапазона курсора (нет клампа по line_i) — фильтр
     `run.line_i < start.line || run.line_i > end.line` в
     decoration_quads (text.rs) и рендере выделения (edit.rs)
8. **Скрытый баг платформ (главный урок волны): glyphon 0.12 не шейпит
   буферы неявно.** glyphon 0.6 шейпил TextArea-буферы внутри prepare;
   0.12 читает `layout_runs(&self)` без авто-шейпинга. `EditingSession::new`
   не вызывал shape_until_scroll (new_title — вызывал) → тело сессии
   НЕ РИСОВАЛОСЬ на платформах с GPU (macOS/Windows). Убежище ложной
   зелёности: ubuntu-раннер без адаптера СКИПАЛ headless-тест (return =
   pass). Диагностика через CI (runs=[]), фикс — явный
   `buffer.shape_until_scroll` в new(). CI: `cargo test --no-fail-fast`
   добавлен в gates (полная картина падений за цикл).
9. **MSRV-линты в windows-gated коде** (canvas-shell: chunks_exact →
   as_chunks ×2) — локально невидимы (cfg(windows)), пойманы gates
   windows-latest.

### Журнал правок (машинная миграция + ручные)

| Домен | Правки | Файлы |
|---|---|---|
| cosmic-text set_* -fs | 41 call-site | text.rs, edit.rs, shaper.rs, тесты |
| set_text/set_rich_text (attrs по & + Align) | 24 | text.rs, edit.rs, shaper.rs, тесты |
| Attrs non-Copy → .clone() | 12 | text.rs, edit.rs |
| highlight → итератор (.next()) | 2 | text.rs, edit.rs |
| wgpu Instance | 4 сайта (default()→конструкторы) | gpu.rs, renderer.rs ×3, grid_gl_smoke |
| request_adapter Option→Result | 3 match | gpu.rs |
| request_device -trace | 1 | gpu.rs |
| Maintain→PollType | 15 | gpu.rs + 13 тестов + render_smoke |
| ImageCopy*→TexelCopy* | 24 | gpu.rs + 5 модулей + 13 тестов |
| Option-обёртки (bind_group_layouts/buffers/entry_point) | 40 | 8 пайплайн-модулей |
| push_constant_ranges→immediate_size | 11 | 8 пайплайн-модулей |
| multiview→multiview_mask | 9 | 8 пайплайн-модулей |
| SurfaceConfiguration +color_space | 1 | renderer.rs |
| RenderPassColorAttachment +depth_slice | 16 | gpu.rs, renderer.rs + тесты |
| get_current_texture → CurrentSurfaceTexture | 1 match | renderer.rs |
| present → Queue::present | 1 | renderer.rs |
| get_mapped_range → Result | 16 | gpu.rs + 13 тестов |
| MSRV-линты (map_or/as_chunks/is_multiple_of) | 15 | canvas-core ×5, ui ×1, app ×5, web ×1, render ×3 |
| WGSL @interpolate(flat) | **0** (все варьинги f32) | 8 шейдеров — аудит чист |

### Верификация

- **CI ветки: run 38322569741 — 12/12 джобов SUCCESS** (gates
  ubuntu/windows/macos, wasm-check, licenses, docs-lint, lay7-lint,
  v3-ui-lint, llm-proxy, artifacts ×3)
- `cargo test --workspace` — ✅ **3443 passed / 0 failed** (снапшоты 221 и
  html5 16 — зелёные, метрики шейпинга не дрейфнули: W4 ожидает
  регенерация-ноуп, см. ниже)
- `cargo check --workspace --all-targets` — ✅ зелёный
- `cargo clippy --workspace --all-targets -- -D warnings` — ✅ зелёный
- `cargo fmt --all` — ✅
- `cargo tree -i cosmic-text` → **единственная** cosmic-text v0.19.0 ✅
- `cargo tree -i wgpu` → **единственный** wgpu v30.0.1 ✅
- WGSL-аудит: int-варьингов нет, @interpolate не требуется ✅
- Рантайм и эталоны — НЕ тронуты (полоса W2: только компиляция); тесты
  запускаются в W3/W4

*Формат записи волны: решения (что и почему), журнал (что сделано), диффы
эталонов (W4+), статус. Дополняется каждой волной.*
