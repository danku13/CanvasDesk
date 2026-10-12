#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Шпаргалка в #64: реальные physical-разрешения MacBook -> логические вьюпорты."""
import os
import json, urllib.request, urllib.error

PAT = os.environ.get("GITHUB_PAT", "")  # секрет вынесен в окружение (push protection)
REPO = "danku13/CanvasDesk"
MARKER = "<!-- macbook-physical-logical-mapping -->"

COMMENT = MARKER + """
## Шпаргалка: реальные разрешения MacBook → логические вьюпорты

Приложение видит **логические** пиксели = physical / scale factor. На всех Mac ниже дефолтный Retina-скейл = **2×**, поэтому физическое разрешение само по себе не тестируется — важен его логический эквивалент:

| Модель | Physical (нативное) | Дефолтный скейл | Логический вьюпорт | Каноническая точка приёмки |
|---|---|---|---|---|
| MacBook Air M1 | 2560×1600 | 2× | **1280×800** | = LAY8.1 1280×800 точь-в-точь |
| MacBook Air M2/M3/M4 | 2560×1664 | 2× | **1280×832** | ширина = канон 1280; высота +32px vs 800 — тот же непрерывный Expanded |
| MacBook Pro 14" M1–M4 | 3024×1964 | 2× | **1512×982** | полоса 1440–1920; контрольная точка из блока выше |
| MacBook Pro 16" M1–M4 | 3456×2234 | 2× | **1728×1117** | полоса 1440–1920; «аналог» из блока выше |

Максимальные штатные опции «больше пространства»: 13" Air → 1680×1050, MBP 14" → 1728×1117, MBP 16" → 1920×1200 — всё внутри полосы логических ширин [1280, 1920].

**Вывод:** любой MacBook M1–M4 при любом штатном масштабе даёт логическую ширину 1280–1920 → единый непрерывный Expanded-тир (≥1280, без верхней границы), модали стоят на max-клампах и центрируются, stage/explain — 80% eff. Отдельные вьюпорты для MacBook в матрице не нужны: точка LAY8.1 1280×800 покрывает Air M1 (дефолт совпадает 1:1), 1512×982 покрывает MBP 14", а Air M2–M4 (1280×832) и MBP 16" (1728×1117) сидят между каноническими точками непрерывного тира.

2K/4K для полноты: 2K 2560×1440 @1× → лог. 2560×1440 (Expanded, точка в блоке выше); 4K 3840×2160 @2× → лог. 1920×1080 (= CR-014), @150% → лог. 2560×1440 (= точка блока выше), @100% → лог. 3840×2160 (Expanded по построению; физическая плотность UI — контекст CR-028, вне скоупа Wave M).
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

# защита от дубля: маркер не должен уже присутствовать в комментариях
st, comments = rest("GET", f"/repos/{REPO}/issues/64/comments")
assert st == 200, st
dup = any(MARKER in c.get("body", "") for c in comments)
assert not dup, "comment already posted"

st, res = rest("POST", f"/repos/{REPO}/issues/64/comments", {"body": COMMENT})
print(f"{'OK' if st in (200,201) else 'FAIL'} comment #64: {st}, id={res.get('id')}")
