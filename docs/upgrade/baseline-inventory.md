# Baseline-инвентаризация рендер-стека (W0)

- **Дата:** 2026-10-13
- **Волна:** W0 (issue #70, трек #69) — заморозка базлайнов до любых изменений
- **База:** `main` @ `086036a` (тег `pre-render-upgrade`)
- **Тулчейн:** rustc 1.99.0 (stable, 2026-09-28); MSRV workspace = 1.80
- **Назначение:** контроль полноты правок в W2 (миграция API) и свера диффов
  эталонов в W4/W6. Все счётчики ниже сняты машинно (команды в §5) с точного
  коммита — воспроизводимы.

## 1. Контрольные числа

| Метрика | Значение |
|---|---|
| Golden-снапшотов (`crates/canvas-ui/tests/snapshot/*.txt`) | **220** |
| HTML5-эталонов (`crates/canvas-ui/tests/html5_demos/*.txt`) | **15** |
| Коммит базлайна | `086036a` |
| Тег отката | `pre-render-upgrade` |

## 2. Точные пины (Cargo.lock @ 086036a)

| Крейт | Версия | Цель (W2) |
|---|---|---|
| wgpu | **22.1.0** | 30.x |
| glyphon | **0.6.0** | 0.12.0 |
| cosmic-text | **0.12.1** | 0.19.0 |
| swash | 0.1.19 | 0.2.x (транзитивно за cosmic-text) |
| etagere | 0.2.15 | по дереву glyphon 0.12 |
| fontdb | 0.16.2 | по дереву cosmic-text 0.19 |
| winit | 0.30.13 | не трогаем (вне трека) |

Workspace-пины (`Cargo.toml [workspace.dependencies]`): `wgpu = "22"`,
`glyphon = "0.6"`, `cosmic-text = "0.12"` — комментарий «та же версия, что
тянет glyphon 0.6» подтверждает координированную связку.

## 3. cosmic-text: точки интеграции

### 3.1 Ломающиеся вызовы (миграция 0.12 → 0.19)

Сигнатуры меняются: `set_text` (добавляется `Option<Align>`),
`set_size`/`set_metrics` (уходят параметры шрифта — источник теперь в
`Metrics`), `Attrs`/`Family` — по месту.

| Файл | set_text | set_size | set_metrics | всего |
|---|---|---|---|---|
| `canvas-render/src/text.rs` | 18 | 21 | 0 | **39** |
| `canvas-render/src/edit.rs` | 1 | 5 | 1 | **7** |
| `canvas-ui/src/shaper.rs` | 1 | 1 | 0 | **2** |
| `canvas-ui/src/measure.rs` | 0 | 0 | 0 | 0* |
| `canvas-app/src/app.rs` | 3 | 0 | 0 | **3** |
| **Итого ломающихся call-sites** | | | | **51** |

\* `measure.rs` содержит `FontSystem::new` + загрузку встроенного шрифта —
требует проверки типов (не сигнатур вызовов) в W2.

### 3.2 Выживающий, но критичный API (проверить типы/семантику в W2)

| Паттерн | Сайты (src, вне тестов) |
|---|---|
| `Buffer::new(` | 27 (text.rs ×23, edit.rs ×2, shaper.rs ×1, + 2 тест-файла) |
| `FontSystem::new(` | shaper.rs (ленивый слот), measure.rs, + тесты/компоненты (гарантированно стеклянных мест нет — см. §5) |
| `set_wrap(`/`shape_until_scroll(` | сохранены сигнатуры (0.19), сверить семантику ShapeLines |
| `Editor`/`EDIT`-трейт (edit.rs) | совместим через `From<&mut Buffer>` → `BufferRef` |

### 3.3 Ширина зацепления по коду (файлы с `cosmic_text::` в src)

- `canvas-render`: text.rs, edit.rs, renderer.rs, row_grid.rs
- `canvas-ui`: shaper.rs, measure.rs, layout.rs, layout/flex.rs, row_guides.rs, paint.rs, component/ ×17
- `canvas-app`: 20 файлов (app.rs, support.rs, stage.rs, ui_registry.rs, *_ui.rs …)

**Тип `FontSystem` пересекает границы крейтов** (canvas-render ↔ canvas-ui ↔
canvas-app) — причина, почему обновление cosmic-text невозможно без
одновременного обновления glyphon (тот пиннит свой cosmic-text в Cargo.toml).

## 4. wgpu: точки интеграции

| Паттерн | Сайты |
|---|---|
| `Instance::new`/`Instance::default` | 6: `gpu.rs`, `renderer.rs` ×4, `tests/grid_gl_smoke.rs` (в v29 заменён на `create()`; `InstanceDescriptor::default()` удалён) |
| `SurfaceConfiguration` | `renderer.rs` ×2 (+ места использования конфига) |
| `frame.present()` | 1: `renderer.rs:2539` (в v30 переносится в `Queue::present`) |
| Пайплайн-модули (`RenderPipeline`) | 8: cards, grid, guides, icon_pipeline, minimap_pass, sectors, thumbs, widget_pass |
| WGSL-шейдеры | 8: cards, grid, guides, icons, minimap, sectors, thumbs, world_icons (в v30: `@interpolate(flat)` обязателен) |
| Сэмплеры (`create_sampler`) | 4: minimap_pass, widget_pass, icon_pipeline, thumbs |

## 5. glyphon: API-поверхность

glyphon используется в 13 файлах (canvas-render — ядро: text.rs, edit.rs,
renderer.rs, cards.rs, contrast.rs, lib.rs; canvas-ui — paint.rs; canvas-app —
overlays, support, kit_ui, palette; тесты — fit_note_repro, itog_label_metrics).

Задействованный API (по сверке исходников 0.6.0 ↔ 0.12): `TextRenderer::{new,
prepare, render}`, `TextAtlas::new`, `Viewport`, `TextAtlas`-кеш — сигнатуры
идентичны, прямых правок ≈ 0. Риск — только транзитивные типы wgpu
(`Device`, `Queue`, `TextureFormat`) и версия `etagere`.

## 6. Методика подсчёта (воспроизводимость)

```bash
# ломающиеся вызовы cosmic-text
rg -c '\.set_text\(|\.set_size\(|\.set_metrics\(' <файл>
# выживающие
rg -c '\.set_wrap\(|\.shape_until_scroll\(|Buffer::new\(|FontSystem::new\(' <файл>
# wgpu
rg -c 'Instance::new\(|Instance::default\(' crates/
rg -n '\.present\(\)' crates/
rg -c 'create_sampler' crates/canvas-render/src/
ls crates/canvas-render/src/shaders/*.wgsl | wc -l
# контрольные числа
ls crates/canvas-ui/tests/snapshot/*.txt | wc -l   # 220
ls crates/canvas-ui/tests/html5_demos/*.txt | wc -l # 15
```

В W2 после миграции повторить §3.1 (счётчик должен уйти в 0 по старым
сигнатурам) и §4 (замены `Instance::new` → `create`, `present` →
`Queue::present`). В W6 — сверить контрольные числа §1 (220/15 должны
остаться, изменения эталонов — только осознанные через протокол W4).
