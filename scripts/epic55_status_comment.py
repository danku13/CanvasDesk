#!/usr/bin/env python3
"""Post Wave M completion status comment to epic #55 (CanvasDesk)."""
import os
import json
import urllib.request

TOKEN = os.environ.get("GITHUB_PAT", "")  # секрет вынесен в окружение (push protection)
URL = "https://api.github.com/repos/danku13/CanvasDesk/issues/55/comments"

BODY = """## Wave M — статус: реализация завершена 6/6, эпик ожидает M4-приёмки

Аудит статуса эпика (сессия сверки, 2026-10-11) — по факту закрытия всех sub-issues.

### Карта завершения sub-issues → main

| Sub-issue | Этап | Коммит в main | Статус |
|---|---|---|---|
| #49 | M0 · аудит 12 модалей + дизайн-контракт 80%/mobile-style | `d22924b` | ✅ closed (completed) |
| #50 | M1 · kit-примитив `modal_rect_flex` + `LayoutTier` | `49be540` | ✅ closed (completed) |
| #51 | M2 · флаг `modals_adaptive` + тумблер RU/EN + G4 | `cd4b705` | ✅ closed (completed) |
| #52 | M3-a · dialog, migrate, canvas_manager, gallery | `1dc509e` | ✅ closed (completed) |
| #53 | M3-b · settings, kit_gallery (+F1/F2), admin_panel | `0b6d52e` | ✅ closed (completed) |
| #54 | M3-c · stage, explain, autolink, onboarding, ai_onboarding | `77fa204` | ✅ closed (completed) |

### Верификация этой сессии
- Все 6 sub-issues закрыты `state_reason: completed`; финальные отчёты сессий A/B/C/D в комментариях, гейты зелёные ×2 (fmt / check / test / clippy / wasm / lay7 / doc_lint).
- Каждый фича-коммит — предка HEAD `main` (`002dd0c`); после `77fa204` ни один коммит не трогал ядро Wave M (`modal.rs`, `kit.rs`, `settings.rs`, `ui_registry.rs`).
- Структурный спот-чек main: примитив `modal_rect_flex` + `LayoutTier`/`layout_tier`, флаг `settings.modals_adaptive`, `SettingsRow::ModalsAdaptive`, `lint_modals_adaptive`, а также артефакты M3-b/c — `main_stage_rect_flex`, `window_rect_flex`, `dialog_rect_flex`/`rows_layout_flex`, `card_layout_flex`/`privacy_scroll`, `admin_panel_flex`, `sidebar_viewport` — все на месте.
- Project v2: sub-issues #49–54 в статусе Done; статус эпика скорректирован **Todo → Test** (соответствует ожиданию M4-приёмки).

### Осталось до закрытия эпика
- **M4 — приёмка владельцем**: 12 модалей × флаг on/off × {1280×800, 1024×640, 800×560, 375, 768, 1440} × RU/EN.
- **M5 — демонтаж старых веток рендера**: вне серии (отдельный запрос владельца), на закрытие эпика не влияет.

Эпик оставлен открытым до завершения M4; после приёмки — закрыть как completed."""

payload = json.dumps({"body": BODY}).encode("utf-8")
req = urllib.request.Request(
    URL,
    data=payload,
    headers={
        "Authorization": f"Bearer {TOKEN}",
        "Accept": "application/vnd.github+json",
        "Content-Type": "application/json",
        "User-Agent": "wave-m-audit-session",
    },
    method="POST",
)
with urllib.request.urlopen(req) as resp:
    result = json.loads(resp.read().decode("utf-8"))

print("COMMENT_OK", result.get("html_url"))
