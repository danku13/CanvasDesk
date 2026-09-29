#!/usr/bin/env python3
"""
CanvasDesk ← Penpot: чтение текущего дизайна в JSON snapshot.

Экспортирует:
- Имя файла, страниц, дату
- Library colors (id, name, color, opacity)
- Library typographies (id, name, fontFamily, fontWeight, fontSize, lineHeight)
- Все страницы: их boards с координатами и размерами, детьми и их свойствами

Снапшот сохраняется в ./penpot_design_snapshot.json (или по --out).
Используется penpot_diff_design.py для diff.

Запуск:
  PENPOT_MCP_TOKEN=... python scripts/penpot_read_design.py [--out path]
"""
from __future__ import annotations

import argparse
import json
import os
import sys
from datetime import datetime
from typing import Any

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from penpot_mcp import PenpotMCP


READ_JS = r"""
if (!penpot.currentFile) throw new Error("No active Penpot file");

const file = penpot.currentFile;
const lib = penpot.library.local;

// Library colors
const colors = (lib.colors || []).map(c => ({
  id: c.id, name: c.name, color: c.color, opacity: c.opacity,
}));

// Library typographies
const typographies = (lib.typographies || []).map(t => ({
  id: t.id, name: t.name, fontFamily: t.fontFamily,
  fontWeight: t.fontWeight, fontSize: t.fontSize, lineHeight: t.lineHeight,
}));

// Pages + top-level boards
const pages = file.pages.map(p => ({
  id: p.id, name: p.name,
  boards: (p.root?.children || []).filter(c => c.type === "board").map(b => ({
    id: b.id, name: b.name, x: b.x, y: b.y, w: b.width, h: b.height,
    fills: (b.fills || []).map(f => ({ color: f.fillColor, opacity: f.fillOpacity })),
    childrenCount: (b.children || []).length,
    children: (b.children || []).map(c => {
      const o = {
        id: c.id, name: c.name, type: c.type,
        x: c.x, y: c.y, w: c.width, h: c.height,
        parentX: c.parentX, parentY: c.parentY,
      };
      if (c.type === "text") {
        o.text = c.characters;
        o.fontFamily = c.fontFamily;
        o.fontSize = c.fontSize;
        o.fontWeight = c.fontWeight;
        o.lineHeight = c.lineHeight;
      }
      if (c.fills && c.fills.length > 0) {
        o.fills = c.fills.map(f => ({ color: f.fillColor, opacity: f.fillOpacity }));
      }
      if (c.strokes && c.strokes.length > 0) {
        o.strokes = c.strokes.map(s => ({
          color: s.strokeColor, opacity: s.strokeOpacity, width: s.strokeWidth,
        }));
      }
      if (c.borderRadius) o.borderRadius = c.borderRadius;
      return o;
    }),
  })),
}));

return {
  file: { id: file.id, name: file.name, pagesCount: file.pages.length },
  library: { colors, typographies },
  pages,
};
"""


def read_snapshot(client: PenpotMCP) -> dict[str, Any]:
    """Прочитать текущий дизайн из Penpot в структурированный словарь.

    execute_code возвращает строку вида `{"result": <js_object>, "log": "..."}`.
    Нужно распарсить и достать `result`.
    """
    raw = client.execute_code(READ_JS)
    if isinstance(raw, str):
        try:
            data = json.loads(raw)
        except json.JSONDecodeError:
            return {"raw_output": raw}
        # execute_code оборачивает в {result, log}
        if isinstance(data, dict) and "result" in data:
            return data["result"]
        return data
    return raw


def main() -> int:
    parser = argparse.ArgumentParser(
        prog="penpot_read_design",
        description="Экспорт текущего дизайна Penpot в JSON snapshot.",
    )
    parser.add_argument(
        "--out", "-o",
        default="penpot_design_snapshot.json",
        help="Куда сохранить snapshot (по умолчанию ./penpot_design_snapshot.json)",
    )
    parser.add_argument(
        "--verbose", action="store_true",
    )
    args = parser.parse_args()

    try:
        client = PenpotMCP.from_env(verbose=args.verbose)
    except Exception as e:
        print(f"CONFIG ERROR: {e}", file=sys.stderr)
        return 2

    client.initialize()
    client._notify_initialized()

    try:
        snapshot = read_snapshot(client)
    except Exception as e:
        print(f"READ ERROR: {e}", file=sys.stderr)
        return 1

    snapshot["snapshot_at"] = datetime.utcnow().isoformat() + "Z"
    snapshot["snapshot_tool"] = "scripts/penpot_read_design.py v1"

    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(snapshot, f, ensure_ascii=False, indent=2)

    # Краткий summary
    file_info = snapshot.get("file", {})
    library = snapshot.get("library", {})
    pages = snapshot.get("pages", [])
    total_boards = sum(len(p.get("boards", [])) for p in pages)
    total_shapes = sum(
        sum(len(b.get("children", [])) for b in p.get("boards", []))
        for p in pages
    )
    print(f"File: {file_info.get('name')} ({file_info.get('pagesCount', 0)} pages)")
    print(f"Library: {len(library.get('colors', []))} colors, {len(library.get('typographies', []))} typographies")
    print(f"Pages: {len(pages)}")
    for p in pages:
        boards_count = len(p.get("boards", []))
        shapes_count = sum(len(b.get("children", [])) for b in p.get("boards", []))
        print(f"  • {p['name']}: {boards_count} boards, {shapes_count} shapes")
    print(f"Total: {total_boards} boards, {total_shapes} shapes")
    print(f"Snapshot saved: {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
