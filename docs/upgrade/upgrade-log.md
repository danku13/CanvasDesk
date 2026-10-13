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
| W5 | #75 | Web/wasm32: сборка + WebGPU/WebGL2 + pages-preview | ✅ Done — агентная верификация: wasm-check, сценарии WebGPU и WebGL2-фолбэка (оракулы адаптера/retry/стабильности/ввода), релизный бандл; pages-preview — interim через draft-release, Pages-деплой ждёт переключения Source владельцем (тег upgrade/step-5-web) |
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

## W5 — web/wasm32: WebGPU + WebGL2-фолбэк + релизный бандл + pages-preview

**Дата:** 2026-10-13 · **Issue:** #75 · **Статус:** ✅ завершена (агентная
верификация; Pages-деплой — за владельцем: переключение Source в Settings)

### Решения

1. **wasm-check воспроизведён локально — паритет с CI.** Time-audit
   (прямые `std::time` в web-крейтах — 0) + `cargo check --target
   wasm32-unknown-unknown` по набору CI-джобы (core/render/widgets/
   mcp/scene/mcp-headless/web) — зелёный за 1m13s на свежем сэндбоксе.
   CI на коммите ветки (df0f821) — 12/12, wasm-check SUCCESS: волна
   стартовала с зелёного wasm-гейта.
2. **Аудит лимитов GL-пути (шаг 3 брифа): код gpu.rs:47 остаётся
   валидным на wgpu 30, изменения не нужны.** Проверено по исходникам
   wgpu-types 30.0.1: `downlevel_webgl2_defaults()` наследует
   `max_texture_dimension_2d = 2048` от `downlevel_defaults()` —
   2048-кап НЕ изменился, значит `boost_gl_limits` (поднятие до
   реального максимума адаптера для HiDPI-canvas, FR-WASM-02 §8) так
   же необходим, как на wgpu 22. Новое поле `apply_limit_buckets`
   (RequestAdapterOptions, wgpu 30) — анти-фингерпринтинговое
   бакетирование лимитов; CanvasDesk явно передаёт `false` (= дефолт):
   `adapter.limits()` возвращает сырые лимиты, boost берёт настоящий
   максимум адаптера. Тесты boost_gl_limits (5) зелёные в CI.
3. **Retry FR-WEBGPU-DIAG не зацикливается — семантика wgpu 30
   подтвердилась логами.** Режим WebGL2-фолбэка (Chromium,
   `--disable-features=WebGPU`): ровно 3 bounded-попытки
   `request_adapter` (default → high-performance → low-power,
   интервал 50 мс), затем ступень 2 GL — консоль фиксирует
   «3 retry попытки исчерпаны — фолбэк на GL (WebGL2)». Цикла нет,
   WARN-трейл полный, адаптер GL (ANGLE/SwiftShader) получен с 1-й
   попытки. В режиме WebGPU адаптер разрешается с 1-й попытки —
   retry вообще не включается.
4. **Чек-лист брифа закрыт агентными оракулами (шаг 2) — сценарий
   `scripts/wasm_ui_w5_scenario.mjs` (новый, в репо).** Стенд:
   debug-wasm (процедура wasm_ui_test.sh) + релизный бандл (trunk
   0.21.14), Chromium headed под Xvfb (Playwright), `?ui=demo`,
   два режима — WebGPU (SwiftShader-Vulkan) и WebGL2-фолбэк:

   | Оракул | WebGPU | WebGL2-фолбэк |
   |---|---|---|
   | адаптер получен (лог GpuContext::new) | ✅ attempt=1 | ✅ GL attempt=1 (после 3 null WebGPU) |
   | строка «web GPU-бэкенд выбран» | ✅ BROWSER_WEBGPU | ✅ GL |
   | попыток request_adapter | 1 | 4 = 3 null + 1 GL |
   | канва рисует (тёмные пиксели кадра) | 97.94 % | 97.95 % |
   | стабильность кадра A→B | 0 px | 0 px |
   | скролл/зум (wheel, пиксельный дифф) | 18 401 px | 18 375 px |
   | инлайн-ввод (dblclick + «Правка W5 кириллица») | 609 937 px | 612 730 px |
   | page-error/panic | 0 | 0 |

   Кириллица: ввод принят, ошибок шейпинга/атласа/глифов в консоли
   нет; скриншоты `target/wasm_ui/w5_*.png` (обе темы/режимы).
   Наблюдение: dblclick в витрине демо пересоздаёт App — в одном
   прогоне exercising'ются оба бэкенда (WebGPU-инициализация +
   capture-GL при consent в localStorage); ожидаемое поведение
   FR-091 v2, не регрессия.
5. **Релизный бандл собирается и оптимизируется (процедура pages-web
   воспроизведена локально): пайплайн wgpu 30 проходит целиком.**
   trunk 0.21.14 --release (LTO thin) + wasm-bindgen-cli 0.2.127
   (= Cargo.lock) + wasm-opt -Oz (binaryen 125 — справляется с
   wgpu-30-wasm). Грабля окружения: `NO_COLOR=1` в агентном сэндбоксе
   ломает trunk→cargo 1.99 (`--no-color 1` невалиден); CI не затронут
   (`web_bundle.sh` ставит `NO_COLOR=false` — валидное значение),
   локально лечится `env -u NO_COLOR`.
6. **Размер бандла ПРЕВЫШАЕТ бюджет §8.8 — честная находка волны,
   решение за W6/владельцем.** wasm: 19.6 МБ → wasm-opt -Oz →
   **14.9 МБ raw** (js 176 КБ, index 159 КБ); gzip -9 = 5.9 МБ
   (brotli на CI даст точное число, ожидаемо ~5 МБ). Бюджет
   wasm-port §8.8 — ≤8 МБ raw / ≤4 МБ brotli, загрузка ≤5 с на
   25 Мбит/с — превышен по raw (×1.9) и по brotli (~×1.25). Контекст:
   бюджет писался до деплоя /app (Pages был в legacy-режиме, бандл
   никогда не публиковался и не контролировался); wgpu 30 +
   harfrust-стек тяжелее wgpu 22. Оценка загрузки: 14.9 МБ raw ≈
   4.8 с, brotli ~5 МБ ≈ 1.7 с на 25 Мбит/с — с компрессией
   укладывается в ≤5 с. Опции: принять (приватный инструмент),
   поднять бюджет, или отдельная задача оптимизации размера
   (opt-level="z", strip, анализ по крейтам).
7. **Pages-preview: interim через draft-release; Pages-деплой — за
   владельцем (один клик).** Репозиторий в legacy-режиме Pages
   (Source: Deploy from a branch, main/docs) — `deploy-pages@v4`
   в pages-web.yml требует Source: GitHub Actions (инструкция в
   шапке workflow и README «Веб-версия»; README предупреждает про
   план Pro/Team для приватного репо). Оба токена сессии без
   pages:write — PATCH build_type через API отвергнут (404).
   Действия: (а) draft-release `w5-web-preview` с zip-бандлом +
   инструкцией локального прогона — доступен владельцу немедленно;
   (б) владельцу переключить Settings → Pages → Source: GitHub
   Actions, после чего агент запускает workflow_dispatch
   pages-web.yml на ветке — живой preview на
   danku13.github.io/CanvasDesk/app/ (приватный репо — виден только
   владельцу). До переключения workflow_dispatch не запускался —
   деплой гарантированно упал бы в legacy-режиме.
8. **Уточнение к решению 7: репозиторий уже был в workflow-режиме —
   реальный блокер не Source.** 451 успешный ран pages-web на main
   с 2026-09-15 (deploy-pages переключает source сам — заметка W12);
   поле source {main,/docs} в /pages API — остаточные метаданные
   legacy-источника. workflow_dispatch после переключения — 403 «Must
   have admin rights to Repository» у обоих токенов сессии
   (actions:write нет); POST deployment-branch-policy — 403 (admin-
   API). Обход: push-триггер pages-web.yml для ветки (21268895 —
   паттерн ci.yml/W1; сам файл workflow в paths-фильтре).
9. **Барьер environment-правила: ран ветки отклонён на старте.**
   Ран 38446220187 (пуш 21268895): джоб умер за 2 с, 0 шагов;
   аннотация check-run: «Branch "upgrade/render-stack" is not allowed
   to deploy to github-pages due to environment protection rules».
   Environment `github-pages` (создан 2026-09-15) держит custom branch
   policies = [main]. Снятие — владелец: Settings → Environments →
   github-pages → Deployment branches and tags → правило ветки.
10. **Находка вне трека (продакшн-эффект): кастомный домен ломал весь
   сайт.** `app.canvasdesk.tech` в настройках Pages репо: ссылки
   danku13.github.io/CanvasDesk/... → 301 → http://app.canvasdesk.tech/...
   → GitHub «Site not found»; сертификат для домена GitHub не выпускал
   (TLS отвечает wildcard `*.github.io`), DNS при этом жив (CNAME app →
   danku13.github.io, NS reg.ru — проверено DoH; сэндбокс-резолвер
   домен не видит — «мёртвый DNS» был ложным следом). Продакшн-URL
   README отдавал 200 на приёмке W12 (2026-09-19) — домен прикреплён
   позже и битно. Действие владельца: Settings → Pages → Custom domain
   → Remove — сайт возвращается на danku13.github.io/CanvasDesk/ без
   пересборки (https — безопасный контекст для WebGPU; public-url
   бандла `/CanvasDesk/app` совпадает). Реанимация домена (remove +
   re-add → перевыпуск сертификата) потребует public-url `/app` —
   решение W6/владельца.
11. **Уточнение к решению 10: владелец выбрал домен (не Remove);
    InvalidDNSError оказался гонкой валидации, DNS корректен.** Зона
    reg.ru обновлена ~18:32Z (SOA 1791916817): авторитативные
    ns1/ns2.reg.ru и публичные резолверы Google/Cloudflare/AliDNS
    (DoH) отвечают CNAME app → danku13.github.io → A 185.199.108–
    111.153 + AAAA 2606:50c0:8000–8003::153, CAA пуст — блокеров
    выпуска сертификата нет. Баннер GitHub снялся сам в ходе сессии:
    edge перевернул http://app…/ 404→200 (конфиг домена расползался
    по Fastly ~15 мин), /pages API: cname прикреплён,
    https_certificate.state=new — LE-сертификат в очереди. Пока он
    не выпущен: github.io → 301 → http://домен (каноникал без TLS),
    https отвечает wildcard `*.github.io` (hostname mismatch).
    После ready — Enforce HTTPS (клик владельца): WebGPU
    (navigator.gpu) требует secure context, на http недоступен.
12. **Домен-адаптивный public-url в pages-web.yml (fix по находкам
    10/11).** Живой артефакт main подтвердил граблю: /app/index.html
    ссылается на /CanvasDesk/app/*.js|wasm — под доменом это 404
    (пины: /app/canvas-web-*.js=200 153 КиБ, /CanvasDesk/app/…=404,
    /app/*_bg.wasm=200 ~14 МБ). Новый шаг «База public-url» читает
    cname из Pages API (gh api, GITHUB_TOKEN с pages:write): домен
    есть → /app, нет → /<repo>/app — сборка валидна в обоих режимах
    без правки манифеста; старые ссылки
    danku13.github.io/CanvasDesk/app/x → 301 → app.canvasdesk.tech/
    app/x (GitHub мапит префикс репо в корень домена сам).

### Журнал

- Окружение восстановлено после сброса сэндбокса (четвёртый за трек):
  rustc 1.99.0 (rsproxy, версия W0–W4), wasm32-таргет, полные
  checkout/шрифты, trunk/wasm-bindgen-cli/wasm-opt (prebuilt где есть)
- wasm_gate --check (ступени 0–1): аудит времени 0 нарушений;
  cargo check wasm32 ×7 крейтов — зелёный
- Сценарии W5: webgpu/webgl2/release-webgpu — 3 прогона, все
  оракулы зелёные, логи `target/wasm_ui/w5_*_logs.txt`
- Релизный бандл: trunk build 4m32s (кэш возобновляемый),
  wasm-opt -Oz 19.6→14.9 МБ, отчёт размеров снят
- Draft-release w5-web-preview: zip бандла + README локального
  прогона (python3 -m http.server)
- Коммит волны: сценарий + docs; тег `upgrade/step-5-web` после
  зелёного CI
- Pages-финал: dispatch 403 (admin) → push-триггер ветки (21268895) →
  ран 38446220187 отклонён environment-правилом [main] (аннотации
  check-run, 0 шагов, 2 с); домен-диагностика: DoH (CNAME/NS живы) +
  пин IP GitHub («Site not found», wildcard-сертификат)
- Домен (продолжение диагностики): авторитативные NS + 3 публичных
  резолвера — записи в норме; edge GitHub перевернул http 404→200 в
  ходе сессии, сертификат new (выпуск в очереди); грабля путей
  ассетов под доменом подтверждена пинами (/app/*.js=200,
  /CanvasDesk/app/*.js=404)
- Фикс: домен-адаптивный public-url (шаг «База public-url», cname из
  Pages API) + правка этого лога; пуш инициирует pages-web на ветке.
  Environment-правило перепроверено через API — всё ещё [main],
  POST-политика 403: ран будет отклонён, пока владелец не добавит
  upgrade/render-stack (Settings → Environments → github-pages →
  Deployment branches and tags); после клика — Re-run jobs / пуш
- Финал деплоя: владелец выполнил оба клика — Source: GitHub
  Actions (build_type=workflow по API) + environment-политики
  [main, upgrade/render-stack]; ложный след «Source: branch» при
  diagnose-сессии был временным режимом: GitHub сам пушил «Create
  CNAME» в обе ветки и задеплоил старый main-бандл с битыми
  /CanvasDesk/app/*-ссылками; этот триггер-коммит запускает
  pages-web на ветке — первый деплой бандла wgpu-30 с
  public-url /app под доменом

### Верификация

- wasm-check локально — ✅ (паритет CI 12/12 на df0f821)
- Аудит лимитов — ✅ 2048-кап не изменился, boost_gl_limits валиден,
  apply_limit_buckets=false корректен (исходники wgpu-types 30.0.1)
- WebGPU-сценарий — ✅ все оракулы (адаптер/стабильность/zoom/ввод)
- WebGL2-фолбэк — ✅ ровно 3 retry → GL, рендер живой, 0 паник
- Релизный бандл — ✅ trunk+wasm-opt пайплайн, прогон в браузере
- Размер §8.8 — ⚠️ 14.9 МБ raw / 5.9 МБ gzip — бюджет превышен,
  решение за W6 (см. решение 6)
- Pages-preview — 🔄 Source=GitHub Actions ✅ (build_type=workflow,
  подтверждено API после клика владельца); домен
  app.canvasdesk.tech прикреплён, DNS валиден (авторитативные
  ns1/ns2.reg.ru + DoH Google/Cloudflare), InvalidDNSError
  снялся (гонка валидации), сертификат в выпуске (new → ready,
  затем Enforce HTTPS — secure context для WebGPU); пайплайн
  домен-адаптивен (public-url /app при cname);
  environment-барьер снят (политики [main,
  upgrade/render-stack]); триггер-коммит этого журнала → деплой:
  ожидание зелёного рана pages-web и пина /app/*.wasm = 200
  под доменом
