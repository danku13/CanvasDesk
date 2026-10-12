#!/usr/bin/env python3
"""UI-V1-FOUNDATION (issue #68): механический пламбинг `height` в литералы
OwnedScreenText по crates/canvas-app/src.

Правила (бриф волны V1):
- дрейн из OwnedText (field-by-field `origin: t.origin`) -> `height: t.height`;
- конверсия PaintItem::Text (`origin: [area.x, area.y]`) -> `height: area.h`;
- прямые пушы текстов (origin с ручной ординатой / чистые строки) ->
  `height: 0.0` («нет slack'а -> топ-анкер», бит-паритет).

Скрипт идемпотентен: литералы с уже вставленным `height:` пропускаются.
"""
import re
import sys
from pathlib import Path

ROOT = Path("/home/z/my-project/crates/canvas-app/src")

PAT = re.compile(r"OwnedScreenText \{")


def literal_span(text: str, open_idx: int) -> tuple[int, int]:
    """[start, end) тела литерала от '{' до балансной '}' (включительно)."""
    depth = 0
    i = open_idx + len("OwnedScreenText ")  # указывает на '{'
    assert text[i] == "{"
    start = i
    while i < len(text):
        c = text[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return start, i + 1
        i += 1
    raise ValueError("unbalanced literal")


def main() -> int:
    changed = 0
    stats = {"t.height": 0, "area.h": 0, "0.0": 0}
    for path in sorted(ROOT.rglob("*.rs")):
        src = path.read_text()
        if "OwnedScreenText {" not in src:
            continue
        out = []
        last = 0
        for m in PAT.finditer(src):
            open_idx = m.start()
            start, end = literal_span(src, open_idx)
            body = src[start:end]
            if re.search(r"\bheight\s*:", body):
                continue  # уже проставлен (идемпотентность)
            if "origin: t.origin" in body or "text: t.text" in body:
                val = "t.height"
            elif "origin: [area.x, area.y]" in body:
                val = "area.h"
            else:
                val = "0.0"
            stats[val] += 1
            out.append(src[last:start + 1])  # до '{' включительно
            out.append(f" height: {val},")
            last = start + 1
        out.append(src[last:])
        new = "".join(out)
        if new != src:
            path.write_text(new)
            changed += 1
            print(f"updated {path}")
    print(f"files changed: {changed}, values: {stats}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
