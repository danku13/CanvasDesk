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
| W1 | #71 | Каркас: ветка + draft PR + upgrade-log | 🔄 In Progress (ждёт клика владельца по draft PR) |
| W2 | #72 | Скачок версий + API-миграция до зелёной компиляции | ✅ Done (тег upgrade/step-2-compile, CI 12/12) |
| W3 | #73 | Рантайм desktop: запуск + smoke + не-эталонные тесты | ✅ Done (lavapipe-рантайм в сэндбоксе, тег upgrade/step-3-runtime; ручной чек-лист — в #73, ждёт подписи владельца) |
| W4 | #74 | Регенерация 220+15 эталонов по протоколу осознанного диффа | ✅ Done — документированный НОУП: дифф-кандидатов 0, эталоны не тронуты (тег upgrade/step-4-snapshots; сводная таблица — #74, в PR-описание при открытии) |
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

---

---

## W3 — рантайм на desktop

**Дата:** 2026-10-13 · **Issue:** #73 · **Статус:** ✅ верификация агента
завершена; ручной чек-лист desktop ожидает подписи владельца в #73

### Решения

1. **Software-Vulkan (lavapipe) в агентном сэндбоксе — рантайм-прогон
   без реального GPU.** mesa-vulkan-drivers 25.0.7 (Debian 13 trixie)
   извлечён из .deb без root (сэндбокс не даёт sudo):
   `VK_ICD_FILENAMES=lvp_icd.json` + `LD_LIBRARY_PATH` → wgpu 30
   `Instance::new_without_display_handle()` находит CPU-адаптер llvmpipe
   (Vulkan 1.4). Впервые headless-тесты идут по РЕАЛЬНОМУ GPU-пути в
   агентной сессии — ловушка «ложной зелёности» из решения 8 (W2,
   скип без адаптера) закрыта и для агентных прогонов, не только CI.
2. **AtlasFull / LRU-интеракция верифицирована (шаг 3 брифа) — корректна.**
   Сверено с исходниками glyphon 0.12 (text_atlas.rs/viewport.rs):
   `trim()` очищает `glyphs_in_use` (НЕ освобождает пакер), эвикция —
   в `try_allocate` по LRU для глифов вне `glyphs_in_use`; `grow()`
   удваивает атлас до max texture dimension, AtlasFull — только на
   максимуме при полностью занятых глифах. CanvasDesk: viewport.update
   1×/кадр (text.rs:3337; пишет uniform только при смене разрешения —
   сверено с viewport.rs 0.12), atlas.trim() в конце кадра после ВСЕХ
   групп (text.rs:5500) — межкадровая эвикция работает, retry-once в
   prepare_group (text.rs:535) — безопасность. Паритет семантики 0.6
   (старый комментарий кода подтверждает тот же дизайн). Рантайм:
   0 AtlasFull-логов, 0 паник во всех прогонах. Частота повторов
   ожидаемо НЕ выросла — увеличивать ничего не нужно.
3. **Perf-дрейф к W0-референсу — нулевой (ранний сигнал для W4/W6):**
   perf_baseline 171.4 → 174.2 μs (+1.6 %), perf_flex 176.1 → 172.1 μs
   (−2.3 %) — в пределах порога ±20 % (R6); абсолютные бюджеты < 1000 μs
   PASS с запасом ~×5.8. Относительные чеки к dev-эталонам 45.1/29.4 μs
   красные — известная машинная зависимость (задокументирована в
   baseline-perf.md W0), baseline-файлы НЕ обновлялись.
4. **Рантайм на реальном железе:** подтверждение владельца — сборка и
   запуск ветки на desktop («собрал, вроде всё работает», TG 2026-10-13)
   + macOS CI artifacts-джоба (WindowServer) зелёная на каждом пуш
   ветки. Ручной чек-лист (шаг 2 брифа) оформлен комментарием в #73 —
   ожидает подписи владельца; regression-фиксов по итогам чек-листа нет
   (жалоб на рантайм не поступало).
5. **Единственный рантайм-фикс волны — тест-инфраструктурный: гонка
   глобального флага pointer_coarse в параллельном libtest (flake
   macOS CI).** Первый пуш W3 (df1072c, docs-only!) упал на macOS в
   3 hit-тестах (palette layout_hit_and_triggers: Trigger(0) вместо Bar;
   whatif overrides_list hits: Some(0) вместо Some(1); clamps: 16 вместо
   17) — при том что идентичный код в W2-ране проходил (сверка логов:
   все 3 … ok). Корень: POINTER_COARSE — глобальный AtomicBool;
   тесты-переключатели (lay_w8_*, whatif coarse, touch_targets — 5
   файлов) выставляют true на время прогона, непиненные hit-тесты того
   же тест-бинаря читают флаг параллельно → coarse-расширение зон
   FR-097 переворачивает граничные ассерты (все 3 падения объясняются
   на 100 %). Тайминг-зависимость: 2-vCPU сэндбокс и W2-ран проходили,
   macOS-раннер (много ядер) — 50/50. Фикс — по корню: POINTER_COARSE
   переведён в thread_local (web_bridge.rs) — писатель (canvas-web
   matchMedia) и читатели (hit-тесты) живут на главном потоке UI,
   продуктовая семантика не меняется; каждый тест libtest получает
   собственный поток → гонка устранена структурно. Пиннинг в 3 тестах
   отвергнут: не закрывает окно гонки (переключатель валиден
   параллельно с пином). Верификация: canvas-app lib 910/0 × 4 прогона,
   canvas-core web_bridge roundtrip ok, fmt чист.

### Журнал

- Окружение агента восстановлено после сброса сэндбокса: rsproxy
  (rustup + sparse-реестр), полный checkout, core.filemode off
- lavapipe: .deb скачан с deb.debian.org, распакован dpkg-deb -x
  (no root); A/B-проверка skip-пути: без ICD «GPU-адаптер недоступен»,
  с ICD — реальный прогон
- Полный прогон `cargo test --workspace --no-fail-fast` НА АДАПТЕРЕ:
  **3443 passed / 0 failed / 4 ignored** (96 таргетов — суперсет брифового
  «не-эталонного» прогона: эталоны тоже зелёные); 0 GPU-скипов:
  15/16 GPU-бинарей canvas-render прошли реальный путь wgpu 30
  (adapter → device → offscreen render → readback → pixel-asserts);
  grid_gl_smoke скип — требует Backends::GL (паритет ubuntu-CI,
  macOS-CI покрывает)
- Снапшоты 221/0 и html5 16/0 — эталоны НЕ дрейфнули → W4 ожидает
  регенерация-ноуп (подтверждение вывода W2)
- Perf: медианы 174.2/172.1 μs (200 итераций × 20 прогревочных,
  debug-профиль, 2 vCPU — сопоставимо с W0)
- CI ветки @ 14c6592: success (run 38324098019); первый W3-пуш df1072c
  (docs-only) поймал флейк macOS (решение 5) — фикс thread_local вторым
  коммитом волны; после зелёного CI — тег `upgrade/step-3-runtime`
- Флейк-верификация: упавшие 3 теста сверены с логами зелёного W2-рана
  (38322569741 macOS-джоба 115053804687: все 3 … ok на идентичном коде)
  → пре-существующий флейк, НЕ регрессия апгрейда (дифф волны — только
  docs + web_bridge.rs)

### Верификация

- `cargo test --workspace --no-fail-fast` + lavapipe — ✅ 3443/0/4
- Счётчики: snapshot 221/0 · html5_demos 16/0 · editing_suppress 1/0
- perf_baseline / perf_flex — ✅ бюджеты, дрейф +1.6 % / −2.3 % (< 20 %)
- AtlasFull-логи/паники — ✅ 0/0
- Ожидаемые брифом падения не реализовались: grid_gl_smoke/render_smoke
  НЕ упали (render_smoke реально прошёл GPU-путь на lavapipe);
  itog_label_metrics/fit_note_repro зелёные — чинка в W4 не требуется
- Флейк macOS CI (решение 5): canvas-app lib 910 passed / 0 failed × 4
  (1 основной + 3 повторных), W2-лог сверен — причина не в апгрейде

---

---

## W4 — метрики эталонов: осознанная регенерация → документированный НОУП

**Дата:** 2026-10-13 · **Issue:** #74 · **Статус:** ✅ завершена —
регенерация НЕ потребовалась (дифф-кандидатов 0), эталоны не тронуты

### Решения

1. **Протокол выполнен до шага анализа включительно — и остановился
   на пустом списке кандидатов.** Прогоны без UPDATE на обоих коммитах
   волны W3 (df1072c и ddd4b0d, включая полный workspace и целевые
   прогоны): `snapshot` **221/0** (220 файлов + счётчик), `html5_demos`
   **16/0** (15 файлов + счётчик) — упавших эталонных тестов НЕТ,
   дифф-кандидатов НЕТ. Шаги регенерации (CANVAS_UI_UPDATE_*) не
   выполнялись — нечего регенерировать.
2. **Таблица диффов — пустая по всем категориям брифа:**

   | Категория | Сдвигов | Вердикт |
   |---|---|---|
   | empty-line-metrics (0.19) | 0 | не реализовался |
   | mono-rounding (0.16) | 0 | не реализовался |
   | harfrust-cluster (0.15) | 0 | не реализовался |
   | pixel-font-1px (0.19) | 0 | не реализовался |
   | unknown | 0 | — |

   Прогноз брифа («ожидаемо изменится ПОДМНОЖЕСТВО эталонов») НЕ
   реализовался: корпус CanvasDesk — RU/EN кириллица/латиница 2–3
   семейства, предсказуемые шейпы; HarfRust (форк rustybuzz) на таком
   корпусе даёт идентичные глифовые последовательности и advances;
   высоты пустых строк корпуса в `Measured.height` не сдвинулись;
   моно-ширины таблиц не изменились (округление 0.16 не переключило
   ни одну колонку). Категория unknown пуста — разбирать нечего.
3. **Контрольные числа брифа выполнены тривиально:** файлов-эталонов
   ровно **220** (snapshot) и ровно **15** (html5) — git-статус
   эталонов чист, ни один файл не перезаписан (проверено на ddd4b0d).
   Регенерация-«вслепую» не проводилась — протокол соблюдён.
4. **Сводная таблица диффов для PR — в комментарии #74** (готовый текст
   для описания PR); перенос в PR-описание — при открытии draft PR
   владельцем (заблокирован с W1: PAT без pull_requests:write).
5. **Perf-контроль волны:** perf_baseline 174.2 μs (+1.6 % к W0),
   perf_flex 172.1 μs (−2.3 %) — снято в W3 на том же коммите; порог
   ±20 % (R6) не тронут; абсолютные бюджеты < 1000 μs PASS.

### Журнал

- Полный прогон workspace на ddd4b0d (lavapipe): 3443/0/4 —
  включая эталоны: snapshot 221/0, html5_demos 16/0 (второй прогон —
  подтверждение стабильности)
- Целевой контрольный прогон: snapshot 221/0 (3.5 c), html5 16/0
- Счёт файлов эталонов: 220/15 — ровно контрольные числа, изменений 0
- CANVAS_UI_UPDATE_SNAPSHOTS / CANVAS_UI_UPDATE_HTML5 — не
  выставлялись ни разу за волну (журнал консоли)
- Коммит волны: docs-only (upgrade-log §W4 + worklog) → тег
  `upgrade/step-4-snapshots` после зелёного CI

### Верификация

- `cargo test --workspace --no-fail-fast` на ddd4b0d — ✅ 3443/0/4
- Эталоны без UPDATE — ✅ 0 упавших = 0 дифф-кандидатов (протокол §1)
- Категория unknown — ✅ пуста (протокол §3 выполнен вакуумно)
- Файлы 220/15 — ✅ нетронуты (протокол §4 контрольные числа)
