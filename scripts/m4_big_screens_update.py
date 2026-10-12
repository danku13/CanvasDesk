#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Добавить в #64 (M4) блок «Большие экраны (2K / 4K / MacBook)»."""
import os
import json, urllib.request, urllib.error

PAT = os.environ.get("GITHUB_PAT", "")  # секрет вынесен в окружение (push protection)
REPO = "danku13/CanvasDesk"

SECTION = """
## Большие экраны (2K / 4K / MacBook) — дополнение от 2026-10-11
Тиры непрерывны (compact ≤767 / medium 768–1279 / **expanded ≥1280 без верхней границы**), формула `clamp(0.8×(viewport−margin), min, max)` непрерывна — разрешений «вне списка» в коде не существует, точки приёмки — контрольные для глаз. Справка по потолкам adaptive-веток: settings 880×640, kit_gallery 900×720, admin 1120×720, autolink 760×760, dialog 440, migrate 480, canvas_manager 640, gallery 660, ai_onboarding = контент; **stage и explain потолка не имеют** (max = eff, всегда 80%). После ≈1150–1450px ширины (по модалям) окна стоят на max-клампах и центрируются — осознанный контракт «канонических пропорций».

- [ ] **1920×1080** (есть в CR-014 эпика; все 12 модалей, флаг ON, быстрый проход): окно на max, центр eff, backdrop не перехватывает хиты
- [ ] **2560×1440 (2K)** — выборочно: settings, admin_panel, kit_gallery, stage: центрирование, канонические пропорции, backdrop читаем
- [ ] **MacBook лог. 1512×982** (MBP 14" default; аналог 1728×1117 MBP 16") — выборочно: settings + stage: expanded-ветка, ничего не режется
- [ ] **4K @ 150–200% масштаба ОС** (лог. 2560×1440 / 1920×1080) — смоук: эквивалент соответствующих логических вьюпортов
- Примечание: **4K @ 100%** (лог. 3840×2160) — по формуле корректно (max-клампы), но физическая плотность UI — вопрос DPR/масштабирования всего приложения (контекст CR-028), вне скоупа Wave M; при желании завести отдельный issue.
"""

def rest(method, path, body=None):
    req = urllib.request.Request(f"https://api.github.com{path}", method=method)
    req.add_header("Authorization", f"Bearer {PAT}")
    req.add_header("Accept", "application/vnd.github+json")
    data = json.dumps(body).encode() if body is not None else None
    if data: req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, data) as r:
            return r.status, json.loads(r.read() or b"{}")
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read() or b"{}")

st, iss = rest("GET", f"/repos/{REPO}/issues/64")
assert st == 200, st
body = iss["body"]
assert "Большие экраны" not in body, "section already present"
anchor = "## DoD"
assert anchor in body
body = body.replace(anchor, SECTION.strip("\n") + "\n\n" + anchor)
st, _ = rest("PATCH", f"/repos/{REPO}/issues/64", {"body": body})
print(f"{'OK' if st in (200,201) else 'FAIL'} PATCH #64: {st}")
