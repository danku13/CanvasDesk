#!/usr/bin/env python3
"""
Сравнение двух snapshot'ов дизайна Penpot.

Показывает:
- новые/удалённые library colors
- новые/удалённые library typographies
- новые/удалённые страницы
- новые/удалённые/изменённые boards (по имени)
- изменения в составе children boards (новые/удалённые shapes)
- изменения в свойствах shapes (text, fills, sizes)

Запуск:
  python scripts/penpot_diff_design.py --old snap1.json --new snap2.json
  python scripts/penpot_diff_design.py --new-from-penpot  # текущий vs последний snapshot

Workflow для sync:
  1. Перед изменениями: python scripts/penpot_read_design.py --out before.json
  2. Внести правки в Penpot
  3. python scripts/penpot_read_design.py --out after.json
  4. python scripts/penpot_diff_design.py --old before.json --new after.json
"""
from __future__ import annotations

import argparse
import json
import os
import sys
from typing import Any


def diff_lists(old: list, new: list, key: str = "name") -> dict[str, Any]:
    """Сравнение списков по ключу. Возвращает added/removed/changed."""
    old_map = {x[key]: x for x in old}
    new_map = {x[key]: x for x in new}
    added = [n for n in new if n[key] not in old_map]
    removed = [o for o in old if o[key] not in new_map]
    changed = []
    for k, new_v in new_map.items():
        if k in old_map:
            old_v = old_map[k]
            diffs = _diff_dict(old_v, new_v)
            if diffs:
                changed.append({"name": k, "changes": diffs})
    return {"added": added, "removed": removed, "changed": changed}


def _diff_dict(old: dict, new: dict, skip: set = None) -> dict[str, Any]:
    """Сравнение двух словарей (без глубокой рекурсии — только верхний уровень)."""
    skip = skip or set()
    out = {}
    all_keys = set(old.keys()) | set(new.keys())
    for k in all_keys:
        if k in skip: continue
        ov, nv = old.get(k), new.get(k)
        if ov != nv:
            out[k] = {"old": ov, "new": nv}
    return out


def diff_snapshot(old_snap: dict, new_snap: dict) -> dict[str, Any]:
    """Главный diff между двумя snapshot'ами."""
    out = {}

    # File info
    out["file"] = _diff_dict(old_snap.get("file", {}), new_snap.get("file", {}))

    # Library colors
    old_colors = old_snap.get("library", {}).get("colors", [])
    new_colors = new_snap.get("library", {}).get("colors", [])
    out["library_colors"] = diff_lists(old_colors, new_colors, "name")

    # Library typographies
    old_t = old_snap.get("library", {}).get("typographies", [])
    new_t = new_snap.get("library", {}).get("typographies", [])
    out["library_typographies"] = diff_lists(old_t, new_t, "name")

    # Pages (by name)
    old_pages = {p["name"]: p for p in old_snap.get("pages", [])}
    new_pages = {p["name"]: p for p in new_snap.get("pages", [])}

    pages_diff = {
        "added": [n for n in new_pages if n not in old_pages],
        "removed": [o for o in old_pages if o not in new_pages],
        "changed": [],
    }
    for name, new_p in new_pages.items():
        if name in old_pages:
            old_p = old_pages[name]
            # Diff boards
            old_boards = {b["name"]: b for b in old_p.get("boards", [])}
            new_boards = {b["name"]: b for b in new_p.get("boards", [])}
            boards_diff = diff_lists(old_p.get("boards", []), new_p.get("boards", []), "name")

            # Children diff for unchanged boards
            children_diff = []
            for bname, new_b in new_boards.items():
                if bname in old_boards:
                    old_b = old_boards[bname]
                    old_kids = {c["id"]: c for c in old_b.get("children", [])}
                    new_kids = {c["id"]: c for c in new_b.get("children", [])}
                    added_k = len(new_kids) - len(set(new_kids) & set(old_kids))
                    removed_k = len(old_kids) - len(set(old_kids) & set(new_kids))
                    changed_k = 0
                    changed_props = []
                    for cid, new_k in new_kids.items():
                        if cid in old_kids:
                            d = _diff_dict(old_kids[cid], new_k, skip={"id"})
                            if d:
                                changed_k += 1
                                if len(changed_props) < 5:
                                    changed_props.append({"id": cid, "changes": d})
                    if added_k or removed_k or changed_k:
                        children_diff.append({
                            "board": bname,
                            "shapes_added": added_k,
                            "shapes_removed": removed_k,
                            "shapes_changed": changed_k,
                            "sample_changes": changed_props,
                        })
            if boards_diff["added"] or boards_diff["removed"] or boards_diff["changed"] or children_diff:
                pages_diff["changed"].append({
                    "page": name,
                    "boards_diff": boards_diff,
                    "children_diff": children_diff,
                })

    out["pages"] = pages_diff
    return out


def summarize(diff: dict[str, Any]) -> str:
    """Текстовое саммари diff'а."""
    lines = []

    f = diff.get("file", {})
    if f:
        lines.append(f"File changes: {len(f)} fields")

    lc = diff.get("library_colors", {})
    if lc.get("added") or lc.get("removed") or lc.get("changed"):
        lines.append(f"\nLibrary colors: +{len(lc['added'])} -{len(lc['removed'])} ~{len(lc['changed'])}")
        for c in lc["added"][:5]:
            lines.append(f"  + {c.get('name')} ({c.get('color')})")
        for c in lc["removed"][:5]:
            lines.append(f"  - {c.get('name')} ({c.get('color')})")
        for c in lc["changed"][:5]:
            lines.append(f"  ~ {c['name']}: {c['changes']}")

    lt = diff.get("library_typographies", {})
    if lt.get("added") or lt.get("removed") or lt.get("changed"):
        lines.append(f"\nLibrary typographies: +{len(lt['added'])} -{len(lt['removed'])} ~{len(lt['changed'])}")
        for t in lt["added"][:5]:
            lines.append(f"  + {t.get('name')} ({t.get('fontFamily')} {t.get('fontWeight')})")

    p = diff.get("pages", {})
    if p.get("added") or p.get("removed") or p.get("changed"):
        lines.append(f"\nPages: +{len(p['added'])} -{len(p['removed'])} ~{len(p['changed'])}")
        for pn in p["added"][:3]:
            lines.append(f"  + {pn}")
        for pn in p["removed"][:3]:
            lines.append(f"  - {pn}")
        for pc in p["changed"]:
            bd = pc.get("boards_diff", {})
            ch = pc.get("children_diff", [])
            total_added_shapes = sum(c.get("shapes_added", 0) for c in ch)
            total_removed_shapes = sum(c.get("shapes_removed", 0) for c in ch)
            total_changed_shapes = sum(c.get("shapes_changed", 0) for c in ch)
            lines.append(f"  ~ page «{pc['page']}»:")
            if bd.get("added"):
                lines.append(f"      +boards: {[b['name'] for b in bd['added']]}")
            if bd.get("removed"):
                lines.append(f"      -boards: {[b['name'] for b in bd['removed']]}")
            if bd.get("changed"):
                lines.append(f"      ~boards: {[b['name'] for b in bd['changed']]}")
            if ch:
                lines.append(f"      shapes: +{total_added_shapes} -{total_removed_shapes} ~{total_changed_shapes}")

    if not lines:
        return "✓ No changes — snapshots are identical"
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(
        prog="penpot_diff_design",
        description="Сравнение двух Penpot design snapshot'ов.",
    )
    parser.add_argument("--old", required=True, help="Старый snapshot (JSON)")
    parser.add_argument("--new", required=True, help="Новый snapshot (JSON)")
    parser.add_argument("--json", action="store_true", help="Вывести полный diff как JSON")
    args = parser.parse_args()

    with open(args.old, "r", encoding="utf-8") as f:
        old_snap = json.load(f)
    with open(args.new, "r", encoding="utf-8") as f:
        new_snap = json.load(f)

    diff = diff_snapshot(old_snap, new_snap)

    if args.json:
        print(json.dumps(diff, ensure_ascii=False, indent=2))
    else:
        print(summarize(diff))
    return 0


if __name__ == "__main__":
    sys.exit(main())
