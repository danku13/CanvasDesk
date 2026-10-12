#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""issue68_update.py — отметить V1/V2 в issue #68 и добавить отчёт-комментарий."""
import json
import os
import time
import urllib.request

TOKEN = os.environ["GITHUB_TOKEN"]
REPO = "danku13/CanvasDesk"
NUM = 68
API = f"https://api.github.com/repos/{REPO}/issues/{NUM}"


def req(url, method, payload):
    data = json.dumps(payload).encode() if payload is not None else None
    r = urllib.request.Request(url, method=method, data=data, headers={
        "Authorization": "Bearer " + TOKEN,
        "Accept": "application/vnd.github+json",
        "User-Agent": "canvasdesk-session",
        "Content-Type": "application/json",
    })
    with urllib.request.urlopen(r) as resp:
        raw = resp.read()
        return json.loads(raw) if raw else {}


def get_body():
    return req(API, "GET", None)["body"]


body = get_body()
changed = 0
for marker in ("**V1 — фундамент", "**V2 — миграция кодмодом"):
    if marker in body:
        body = body.replace(f"- [ ] {marker}", f"- [x] {marker}", 1)
        changed += 1
if changed:
    req(API, "PATCH", {"body": body})
    print(f"body: {changed} чекбокса отмечено")
else:
    print("body: маркеры не найдены (уже отмечены?)")

COMMENT = """## Волны V1+V2 выполнены и запушены

**V1 — фундамент (`cf54c55b`)**
- `canvas_ui::measure`: `LineMetrics` + `line_metrics(size)` (hhea-инвариант `ascent+descent == line_height`); хелперы `text_area_vcentered(rect, size)` и `caret_rect(text_area, size, caret_x)` (экспорт `canvas_ui::kit`)
- Рендер: `ScreenText.area_h` + `vcenter_top` — авто-центрирование line-box при slack, бит-паритет при `area_h == line_h`/`0.0`; плумбинг `height` через `OwnedText`/`OwnedScreenText`
- `ROW_LINE_FRAC` 1.1 → `SCREEN_LINE_FACTOR` (1.3); banner на канон
- Спека: `design/rules/09-input.md` — **IN11** (line-box centering), **IN12** (caret geometry), **IN13** (запрет магических офсетов); `docs/ui-kit.md`; снапшоты canvas-ui перегенерированы

**V2 — миграция (`bef8205a`, 3 параллельных агента)**
- Витрина: `kit_ui.rs` 41 + `kit_sections_k4.rs` 24 label-вызова, демо-каретка h 28→12.9, w 1.0→1.5
- Панели: agent/admin/ai_status/graph_builder/explain/handler — 53 label; каретки 3/3 (agent h 26→11.6 — фикс «каретка во всю высоту»; admin w 1.0→1.5 + фикс двойного x-офсета); селекция agent-панели = `caret_rect`-геометрия
- `overlays.rs`: каретки 3/3 (kit_field_view — общий рендер поиска/галереи/шаблонов/rename; K4; настройки TextInput), поиск — строковый бокс вместо `origin.y + 9.0` (текст и каретка на одном центре), онбординг CTA/Prev/Next/Skip, настройки-навигация, 59 label
- Итого: **177 label-вызовов + 7 кареток** на каноне; каретки 26px при кегле 13 устранены как класс

**Гейты**: fmt / clippy `-D warnings` (ui, render, app) / `cargo test -p canvas-app` 910/0 + 12 интеграционных наборов 0 failed / canvas-ui 445/0, canvas-render 420/0 — зелёные. Golden-эталоны не потребовали перезаморозки (пинят слоты раскладки, не draw-геометрию); дельты draw-геометрии задокументированы в worklog (UI-V2-A).

**Осознанно не мигрировано**: топ-анкерные стеки и контент-тела (допустимо по IN11), компонентные line-boxes (уже канон), wheel-меню, `d.icon`-области, ~70 ручных `OwnedScreenText` пушей (docs/flyout/what-if/шаблоны) — остаток для отдельной волны вместе с **V3-гейтами** (линты + WASM L0/L2 + скриншот-дифф LAY8.1).

Приёмка — за владельцем (wasm-сборка в Chrome: витрина, настройки, agent-панель, поиск — текст по центру, каретка ≈ строка текста)."""

req(f"{API}/comments", "POST", {"body": COMMENT})
print("comment: OK")
