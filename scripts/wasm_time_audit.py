#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""wasm_time_audit — статический гейт «времени» web-порта (W1, wasm-порт §2 п.7).

Диагноз класса бага (FR-079 S3-fix, 2026-09-30): `std::time::Instant::now()`
и `std::time::SystemTime::now()` на wasm32-unknown-unknown КОМПИЛИРУЮТСЯ,
но паникуют в рантайме («time not implemented on this platform» →
wasm-ловушка unreachable → приложение зависает). cargo check и wasip1-тесты
такой код не ловят: компиляция зелёная, а на wasip1 часы работают.

Правило: в крейтах, попадающих в web-бандл, монотонное и календарное время
— только через alias `canvas_core::time::{Instant, SystemTime, UNIX_EPOCH}`
(web_time). Аудит ищет прямые обращения к std и разрешает исключения только
из явного allowlist'а ниже.

Что считается нарушением:
  * `std::time::Instant` / `std::time::SystemTime` по полному пути;
  * `use std::time::{..., Instant, ...}` / `{..., SystemTime, ...}`;
  * `use std::time::Instant;` / `use std::time::SystemTime;`.

Что НЕ нарушение:
  * `std::time::Duration` — на wasm32 работает (чистый тип, не часы);
  * код внутри top-level `#[cfg(...test...)] mod ... { }` — в бандл не
    попадает (нативные/wasip1-раннеры имеют работающие часы). Границы
    региона: column-0 атрибут cfg с test → column-0 закрывающая скобка
    (rustfmt-дисциплина: содержимое модуля с отступом, CI-гейт fmt).
    Сырые многострочные строки с column-0 `}` могут закрыть регион раньше
    — это ложное срабатывание в БЕЗОПАСНУЮ сторону (консервативно);
  * файлы из ALLOWLIST — осознанные исключения с обоснованием.

Запуск:  python3 scripts/wasm_time_audit.py   (0 — чисто, 1 — нарушения)
Встроен в scripts/wasm_gate.sh ступенью 0 (до дорогой компиляции) и в CI
(job wasm-check, шаг до cargo check).
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Крейты, попадающие в web-бандл (wasm-гейт, ступень 1) — их src/ аудитим.
WASM_CRATES = [
    "crates/canvas-core/src",
    "crates/canvas-app/src",
    "crates/canvas-scene/src",
    "crates/canvas-render/src",
    "crates/canvas-widgets/src",
    "crates/canvas-web/src",
    "crates/canvas-suggest/src",
]

# canvas-mcp вне скоупа осознанно: компилируется под wasm в гейте, но
# исполняется только под wasm32-wasip1 (wasmtime, mcp_wasm_e2e), где
# SystemTime работает; в web-бандл не входит.

# Явные исключения (относительный путь → обоснование). Новая запись —
# только вместе с причиной в коммите.
ALLOWLIST: dict[str, str] = {
    # feature l1-laya — натив-only по дизайну (sidecar-процесс + ureq);
    # web-сборка фичу не включает (wasm-гейт проверяет default-фичи)
    "crates/canvas-suggest/src/laya/sidecar.rs": (
        "feature l1-laya: sidecar-процесс нативен по дизайну, web-сборка "
        "фичу не включает"
    ),
}

# --- шаблоны ---------------------------------------------------------------

# Полный путь: std::time::Instant::now(), std::time::SystemTime::now() и т.п.
RE_PATH = re.compile(r"\bstd::time::(Instant|SystemTime)\b")
# use-строка: use std::time::X; / use std::time::{A, B};
RE_USE = re.compile(r"\buse\s+std::time::(?:(\w+)|\{([^}]*)\})\s*;")
USE_NAMES = {"Instant", "SystemTime"}

# column-0 атрибут cfg с test внутри (cfg(test), cfg(all(test, ...)), …)
RE_CFG_TEST_ATTR = re.compile(r'^#\[cfg\((?:[^\[\]]*\btest\b[^\[\]]*)\)\]\s*$')
# column-0 объявление инлайн-модуля: mod tests { / pub mod foo {
RE_MOD_OPEN = re.compile(r"^(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+[^;]*\{\s*$")


def strip_comments(line: str, in_block: bool) -> tuple[str, bool]:
    """Убрать // и /* */ из строки (для матчинга — строки не трогаем)."""
    if in_block:
        end = line.find("*/")
        if end == -1:
            return "", True
        return strip_comments(line[end + 2:], False)
    out = []
    i = 0
    while i < len(line):
        if line.startswith("//", i):
            break
        if line.startswith("/*", i):
            nested = strip_comments(line[i + 2:], True)
            out.append(nested[0])
            return "".join(out), nested[1]
        out.append(line[i])
        i += 1
    return "".join(out), False


def test_regions(lines: list[str]) -> list[tuple[int, int]]:
    """Диапазоны (0-based, включительно) top-level cfg-test модулей.

    Правило: column-0 `#[cfg(...test...)]` + следующая значимая строка —
    column-0 `mod ... {` → регион до первой column-0 строки `}` (закрывающая
    скобка модуля; rustfmt держит содержимое с отступом). Если скобка не
    найдена — модуль незакрыт (сломанный файл): регион до EOF.
    """
    regions: list[tuple[int, int]] = []
    i = 0
    n = len(lines)
    while i < n:
        if RE_CFG_TEST_ATTR.match(lines[i]):
            j = i + 1
            # между атрибутом и mod допустимы только пустые строки и
            # другие атрибуты (doc, allow, …) — все column-0
            while j < n and (not lines[j].strip() or lines[j].startswith("#[")):
                if lines[j].startswith("pub") or lines[j].startswith("mod"):
                    break
                j += 1
            if j < n and RE_MOD_OPEN.match(lines[j]):
                k = j + 1
                close = None
                while k < n:
                    if lines[k].rstrip() == "}":
                        close = k
                        break
                    k += 1
                regions.append((i, close if close is not None else n - 1))
                i = (close if close is not None else n - 1) + 1
                continue
        i += 1
    return regions


def in_region(regions: list[tuple[int, int]], line_no: int) -> bool:
    return any(a <= line_no <= b for a, b in regions)


def audit() -> list[str]:
    violations: list[str] = []
    for crate_src in WASM_CRATES:
        root = REPO / crate_src
        if not root.is_dir():
            violations.append(f"[аудит] нет каталога {crate_src} — обнови WASM_CRATES")
            continue
        for rs in sorted(root.rglob("*.rs")):
            rel = rs.relative_to(REPO).as_posix()
            if rel in ALLOWLIST:
                continue
            lines = rs.read_text(encoding="utf-8").splitlines()
            regions = test_regions(lines)
            in_block = False
            for idx, raw in enumerate(lines):
                code, in_block = strip_comments(raw, in_block)
                if not code.strip():
                    continue
                if in_region(regions, idx):
                    continue
                if RE_PATH.search(code):
                    violations.append(f"{rel}:{idx + 1}: прямой std::time → {code.strip()}")
                    continue
                m = RE_USE.search(code)
                if m:
                    names: set[str] = set()
                    if m.group(1):
                        names = {m.group(1)}
                    elif m.group(2):
                        names = {p.strip() for p in m.group(2).split(",") if p.strip()}
                    if names & USE_NAMES:
                        violations.append(
                            f"{rel}:{idx + 1}: use std::time с Instant/SystemTime → {code.strip()}"
                        )
    return violations


def main() -> int:
    bad = audit()
    if not bad:
        print("[wasm-time-audit] OK: прямых std::time::Instant/SystemTime в web-крейтах нет")
        print("  (время — только через canvas_core::time; allowlist-исключений: %d)" % len(ALLOWLIST))
        return 0
    print("[wasm-time-audit] НАРУШЕНИЯ W1 (%d):" % len(bad))
    for v in bad:
        print("  " + v)
    print()
    print("  std::time::Instant/SystemTime паникуют на wasm32-unknown-unknown")
    print("  («time not implemented on this platform» → фриз web-приложения).")
    print("  Замени на canvas_core::time::{Instant, SystemTime, UNIX_EPOCH}.")
    print("  Осознанное исключение — файл в ALLOWLIST этого скрипта + причина в коммите.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
