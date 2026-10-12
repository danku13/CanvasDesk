# V3 lint baseline (issue #68, волна V3) — 2026-10-12

Скан ДО миграции V3-B: 171 хит (L1: 51, L2: 120, L3: 0), 102 файла.
Миграция V3-B (3 агента + координаторские фиксы): ~126 сайтов мигрировано на kit::text_area_vcentered/caret_rect, 45 осознанных сайтов аннотировано `// v3:allow` (топ-анкерные стеки IN11 / компонентные строковые боксы IN13 / арт онбординга).
Патч сканера в ходе волны: rustfmt-устойчивость standalone-маркеров (rustfmt переносит хвостовой комментарий за закрывающую скобку).

Финальное состояние (после V3-B):
```
--- V3-L1: full-rect labels (label_center/label_left вне text_area_vcentered) ---
OK — label-вызовов вне text_area_vcentered нет
--- V3-L2: magic vertical offsets at text/caret sites ---
OK — магических вертикальных офсетов на текст-сайтах нет
--- V3-L3: line-height factor literals (×1.3/×1.1) outside measure.rs ---
OK — литералов фактора строки вне measure.rs нет

V3 UI lint: OK — нарушений IN13 нет (файлов просканировано: 102, тест-модулей пропущено: 3)
```
Selftest: 26/26 OK. Нативные гейты: fmt OK, clippy -p canvas-app -D warnings OK, test canvas-ui/canvas-render/canvas-app 910+интеграции 0 failed.
