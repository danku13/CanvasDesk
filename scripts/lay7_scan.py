#!/usr/bin/env python3
# -*- coding: utf-8 -*-
r"""lay7_scan — статический сканер правила LAY7 (design/rules/11-layouts.md, §LAY7).

Ремонт гейта LAY-W14 после ревью волны W1..W12
(design/layouts-w1-w12-review.md: §3.1 P1-2 — слепая зона отсечки,
§3.2 P2-5 — allowlist-лазейка, P2-6 — узкие паттерны, §4 — постановка).
Прежний awk-гейт останавливал скан файла на ПЕРВОМ `#[cfg(test)]`, поэтому
декларации тест-модулей в шапках глухо обрубали app.rs на :156 и lib.rs на
:122 — ~35 тыс. строк (весь прод app.rs) оставались вне гейта. Здесь каждый
файл сканируется ЦЕЛИКОМ, а тестовый код отсекается по mod-границам.

Что считается нарушением:
  1. gap-литералы: `gap:` + числовой литерал — целый (`gap: 8`), дробный
     (`gap: 9.0`), отрицательный (`gap: -1.0`), скобочный (`gap:(8.0)`),
     с суффиксом (`gap: 8.0f32`) и разделителями (`1_000.5`).
     Исключения — на уровне КОНКРЕТНОГО совпадения, не строки:
       * значение 0 / 0.0 — нейтральный «ритм без зазора» (раздел
         «Исключения» 11-layouts.md), как и раньше;
       * префиксные идентификаторы collision_gap:/row_gap: (collision/snap,
         не раскладка) не совпадают по построению: \bgap требует границу
         слова перед `gap`, а `_` — словесный символ. Построчный фильтр
         прежнего гейта, глотавший валидный hit в строке
         `row_gap: 2.0, gap: 9.0`, устранён: совпадения ищутся по вхождениям.
  * курсоры: `[a-z_]*x +=` / `[a-z_]*y +=` (включая одиночные x/y и
     разыменование `*x +=`) в строке, содержащей литерал с дробной частью —
     ручной курсор (анти-паттерн LAY2/LAY10). Ловит и прямой `y += 8.0`,
     и смешанный `y += CONST + 6.0` — канонический пример аудита.
     Дробный литерал — с ведущей цифрой (`8.0`, `12.5`, `8.`); голая форма
     `.5` не ловится (неоднозначность с tuple-индексами `.0`/`.1`).

Что НЕ считается нарушением (осознанные границы гейта):
  * код в тестовых модулях:
      а) `#[cfg(test)] mod имя;` — файл имя.rs (или имя/mod.rs) пропускается
         ЦЕЛИКОМ (правила поиска мод-файла Rust: для lib.rs/main.rs/mod.rs —
         рядом с декларацией, для foo.rs — в подкаталоге foo/);
      б) инлайн `#[cfg(test)] mod имя { ... }` — пропуск до парной
         закрывающей скобки: трекинг глубины по коду, очищенному от
         комментариев и строковых литералов (скобки в строках счёт не
         сбивают). После закрытия мода скан продолжается — поэтому
         регресс P1-2 устранён: нарушение в файле ПОСЛЕ декларации
         `#[cfg(test)] mod ...;` остаётся в скане;
  * чисто-переменные курсоры (`cursor_y += step;`) — вне гейта: шаг из
    именованной константы/переменной не создаёт нового off-scale литерала,
    дрейф значения ловят пины и golden-тесты;
  * //-комментарии (первый непробельный символ строки) и содержимое
    строковых литералов ("…", '…', r#"…"# — raw-строки поддерживаются
    базово, включая многострочные);
  * строки с инлайн-исключением `// lay7:allow <причина>` — подавляется
    ТОЛЬКО эта строка. Файловый allowlist прежнего гейта (P2-5) удалён:
    новая точка исключения обязана нести причину и видна в диффе.

Ограничения (задокументированы осознанно):
  * statement ≈ физическая строка: перенос хвоста `+ 6.0;` на следующую
    строку не ловится (как и в прежнем awk-гейте);
  * незакрытый инлайн тест-мод (файл не компилируется) глушит хвост файла —
    такой файл валит cargo-гейты раньше;
  * декларации `#[cfg(test)] mod …;` ВНУТРИ инлайн тест-модов не собираются
    (в дереве canvas-app таких нет);
  * вне скана всё, что не crates/canvas-app/src: например
    canvas-render/text.rs:1390 `gap: 2.0` (content-layout) остаётся вне
    гейта до решения LAY-W21.

Запуск:  python3 scripts/lay7_scan.py [ПУТЬ ...]  (по умолчанию
         crates/canvas-app/src; ПУТЬ — файл или каталог, рекурсивно)
Выход:   0 — нарушений нет; 1 — есть (список file:line: fragment).
Negative-тест гейта: bash scripts/lay7_lint.sh --selftest (фикстуры вне репо).
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_ROOTS = ("crates/canvas-app/src",)

# --- лексер: код без комментариев и строковых литералов ---------------------


class LexState:
    """Межстрочное состояние лексера (блочные комментарии, raw-строки)."""

    __slots__ = ("block_depth", "str_hash")

    def __init__(self) -> None:
        self.block_depth = 0  # глубина /* … */ (с вложенностью)
        self.str_hash = 0  # 0 — вне строки; k>=1 — внутри r#"…"# из k решёток


RE_ALLOW = re.compile(r"lay7:allow\b")


def _is_word(ch: str) -> bool:
    return ch.isalnum() or ch == "_"


def _mask_plain_string(line: str, i: int) -> int:
    r"""i — на открывающей `"`. Вернуть индекс за закрывающей кавычкой.

    Не закрылась до конца строки (в валидном Rust — только backslash-
    продолжение): трактуем как закрытую на EOL — скан следующих строк
    продолжается, т.е. ошибка в БЕЗОПАСНУЮ сторону (ложных пропусков нет).
    """
    j = i + 1
    n = len(line)
    while j < n:
        c = line[j]
        if c == "\\":
            j += 2
            continue
        if c == '"':
            return j + 1
        j += 1
    return n


def _raw_string_hashes(line: str, i: int) -> tuple[int | None, int | None]:
    """line[i] == 'r'. Если это старт raw-строки — вернуть (k, индекс `"`)."""
    j = i + 1
    n = len(line)
    k = 0
    while j < n and line[j] == "#":
        k += 1
        j += 1
    if j < n and line[j] == '"':
        return k, j
    return None, None  # не raw-строка (в т.ч. raw-идентификатор r#type)


def _mask_raw_string(line: str, i: int, k: int, st: LexState) -> int:
    """i — на открывающей `"` raw-строки из k решёток. k=0 — обычная r"…"."""
    n = len(line)
    j = i + 1
    while j < n:
        if line[j] == '"':
            if k == 0:
                return j + 1
            cnt = 0
            while cnt < k and j + 1 + cnt < n and line[j + 1 + cnt] == "#":
                cnt += 1
            if cnt == k:
                return j + 1 + cnt
        j += 1
    if k > 0:  # многострочная raw-строка — регион продолжается
        st.str_hash = k
    return n


def _char_literal_end(line: str, i: int) -> int | None:
    """line[i] == `'`. Вернуть индекс за концом чар-литерала или None
    (если это lifetime: 'a, 'static, '_ — кавычка остаётся обычным кодом)."""
    n = len(line)
    if i + 1 >= n:
        return None
    c = line[i + 1]
    if c == "\\":  # '\n', '\u{1F600}', '\\' — до закрывающей кавычки
        j = i + 2
        while j < n:
            if line[j] == "\\":
                j += 2
                continue
            if line[j] == "'":
                return j + 1
            j += 1
        return None
    if i + 2 < n and line[i + 2] == "'":
        return i + 3  # 'x'
    return None


def mask_line(line: str, st: LexState) -> tuple[str, bool]:
    """Строка → (код без комментариев и содержимого строк, маркер lay7:allow).

    Содержимое строковых/чар-литералов выбрасывается, чтобы скобки и
    подстроки вида `gap:` внутри литералов не влияли ни на трекинг глубины
    скобок, ни на паттерны. Комментарии выбрасываются целиком; маркер
    `// lay7:allow` ищется в // -комментарии строки.
    """
    out: list[str] = []
    marker = False
    i = 0
    n = len(line)

    if st.str_hash:  # хвост многострочной raw-строки
        k = st.str_hash
        closed = False
        j = 0
        while j < n:
            if line[j] == '"':
                cnt = 0
                while cnt < k and j + 1 + cnt < n and line[j + 1 + cnt] == "#":
                    cnt += 1
                if cnt == k:
                    i = j + 1 + cnt
                    st.str_hash = 0
                    closed = True
                    break
            j += 1
        if not closed:
            return "", marker

    while i < n:
        if st.block_depth:
            if line.startswith("*/", i):
                st.block_depth -= 1
                i += 2
            elif line.startswith("/*", i):
                st.block_depth += 1
                i += 2
            else:
                i += 1
            continue
        if line.startswith("//", i):
            if RE_ALLOW.search(line[i:]):
                marker = True
            break
        if line.startswith("/*", i):
            st.block_depth += 1
            i += 2
            continue
        ch = line[i]
        if ch == '"':
            i = _mask_plain_string(line, i)
            continue
        if ch == "r" and (i == 0 or not _is_word(line[i - 1])):
            k, j = _raw_string_hashes(line, i)
            if j is not None:
                i = _mask_raw_string(line, j, k, st)
                continue
        if ch == "'":
            nxt = _char_literal_end(line, i)
            if nxt is not None:
                i = nxt
                continue
        out.append(ch)
        i += 1
    return "".join(out), marker


# --- распознавание тест-модулей ----------------------------------------------

# атрибут `#[cfg(…)]` + хвост строки (может быть пустым)
RE_ATTR = re.compile(r"^\s*#\s*\[\s*cfg\s*\((.*)\)\s*\]\s*(.*)$")
# декларация/открытие модуля: mod имя; | mod имя { | mod имя
RE_MOD_DECL = re.compile(r"^(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*([;{]?)\s*$")
RE_TEST = re.compile(r"\btest\b")  # cfg(test), cfg(all(test, …)), …


def walk(lines: list[str]):
    """Обход строк файла с пропусканием инлайн cfg(test)-модов.

    Yields:
      ("external", имя)              — декларация `#[cfg(test)] mod имя;`
      ("line", idx0, code, marker)   — обычная строка вне инлайн тест-модов
    """
    st = LexState()
    pending_attr = False  # видели `#[cfg(…test…)]` — ждём mod
    pending_mod: str | None = None  # видели `mod имя` без ; и { — ждём `{`
    skip_depth = 0
    for idx, raw in enumerate(lines):
        code, marker = mask_line(raw, st)
        if skip_depth > 0:
            # тело тест-мода: только глубина скобок (код уже очищен)
            skip_depth += code.count("{") - code.count("}")
            if skip_depth <= 0:
                skip_depth = 0
                pending_attr = False
                pending_mod = None
            continue
        stripped = code.strip()
        if not stripped:
            continue  # пустая строка или только комментарий
        handled = False
        m = RE_ATTR.match(code)
        if m is not None:
            rest = m.group(2).strip()
            if not rest:
                # чистая атрибутная строка; не-test атрибуты pending не сбрасывают
                if RE_TEST.search(m.group(1)):
                    pending_attr = True
                handled = True
            elif RE_TEST.search(m.group(1)):
                d = RE_MOD_DECL.match(rest)
                if d is not None:
                    handled = True
                    name, tail = d.group(1), d.group(2)
                    if tail == ";":
                        yield ("external", name)
                    elif tail == "{":
                        skip_depth = max(code.count("{") - code.count("}"), 0)
                        pending_attr = False
                        pending_mod = None
                        continue  # строка открытия — не сканируем
                    else:
                        pending_mod = name
        if handled:
            continue
        if pending_attr or pending_mod is not None:
            d = RE_MOD_DECL.match(code)
            if d is not None:
                name, tail = d.group(1), d.group(2)
                if tail == ";":
                    yield ("external", name)
                    pending_attr = False
                    pending_mod = None
                    continue
                if tail == "{":
                    skip_depth = max(code.count("{") - code.count("}"), 0)
                    pending_attr = False
                    pending_mod = None
                    continue
                if tail == "" and pending_mod is None:
                    pending_mod = name  # `{` ожидается на следующих строках
                    continue
            if pending_mod is not None and stripped.startswith("{"):
                skip_depth = max(code.count("{") - code.count("}"), 0)
                pending_attr = False
                pending_mod = None
                continue
            pending_attr = False
            pending_mod = None
            # не модуль — обычный код, проваливаемся в скан
        yield ("line", idx, code, marker)


# --- паттерны нарушений ------------------------------------------------------

# gap: [((] [-] число — дробный, целый, с `_`, суффиксом f32/f64, радиксом
RE_GAP = re.compile(
    r"\bgap\s*:\s*\(?\s*(-)?\s*"
    r"(0[xob][0-9a-fA-F_]+|\d[\d_]*(?:\.\d[\d_]*)?|\.\d[\d_]*)(?:f32|f64)?"
)
# курсор x/y += (любой [a-z_]*[xy], одиночные x/y, `*x +=` через \b)
RE_CURSOR = re.compile(r"\b(?:[a-z_][a-z0-9_]*[xy]|[xy])\s*\+=")
# литерал с дробной частью в той же строке-statement'е. Ведущая цифра
# обязательна и контекст после `.`/слова/`)` запрещён: иначе tuple-индексы
# `kind_metrics(...).1`, `pos.0.0` дают ложные «дробные литералы»
# (голые `.5` без ведущей цифры курсорным паттерном не ловятся —
# задокументировано в шапке).
RE_FRACTIONAL = re.compile(
    r"(?<![.\w)])\d+\.\d*(?:[eE][+-]?\d+)?(?:f32|f64)?"
)


def _literal_is_zero(lit: str) -> bool:
    """0 / 0.0 / -0.00 / 0x0 — нейтральный ритм; 0x10 и прочие — нет."""
    t = lit.replace("_", "")
    if t.startswith(("0x", "0o", "0b")):
        base = {"0x": 16, "0o": 8, "0b": 2}[t[:2]]
        return int(t[2:], base) == 0
    return float(t) == 0.0


def line_hits(code: str) -> list[str]:
    """Виды нарушений в строке кода (без комментариев/строк)."""
    kinds: list[str] = []
    for m in RE_GAP.finditer(code):
        if _literal_is_zero(m.group(2)):
            continue
        kinds.append("gap")
        break
    if RE_CURSOR.search(code) and RE_FRACTIONAL.search(code):
        kinds.append("cursor")
    return kinds


# --- обход дерева ------------------------------------------------------------


def gather_files(paths: list[Path]) -> list[Path]:
    """*.rs по каждому пути (файл или каталог, рекурсивно), без дубликатов."""
    files: list[Path] = []
    seen: set[Path] = set()
    for p in paths:
        if p.is_file() and p.suffix == ".rs":
            resolved = p.resolve()
            if resolved not in seen:
                seen.add(resolved)
                files.append(p)
        elif p.is_dir():
            for f in sorted(p.rglob("*.rs")):
                if f.is_file():
                    resolved = f.resolve()
                    if resolved not in seen:
                        seen.add(resolved)
                        files.append(f)
        else:
            print(f"[lay7-scan] предупреждение: путь не найден: {p}", file=sys.stderr)
    return files


def module_file_candidates(decl_file: Path, name: str) -> set[Path]:
    """Кандидаты мод-файла для `mod имя;` из decl_file (правила Rust 2018+)."""
    parent = decl_file.parent
    stem = decl_file.stem
    base = parent if stem in ("lib", "main", "mod") else parent / stem
    return {base / f"{name}.rs", base / name / "mod.rs"}


def scan(paths: list[Path]) -> tuple[list[tuple[Path, int, str]], list[tuple[Path, int, str]], int, int]:
    """Два прохода: (1) сбор внешних тест-модулей, (2) скан остального.

    Возвращает (gap_hits, cursor_hits, сканов_файлов, пропущено_тест-файлов);
    хиты — (файл, номер строки с 1, фрагмент).
    """
    files = gather_files(paths)

    # проход 1: декларации `#[cfg(test)] mod имя;` → пропускаемые файлы
    skip: set[Path] = set()
    for f in files:
        lines = f.read_text(encoding="utf-8", errors="replace").splitlines()
        for event in walk(lines):
            if event[0] == "external":
                skip |= {c.resolve() for c in module_file_candidates(f, event[1])}

    # проход 2: скан файлов вне пропуска
    gap_hits: list[tuple[Path, int, str]] = []
    cursor_hits: list[tuple[Path, int, str]] = []
    scanned = 0
    skipped = 0
    for f in files:
        if f.resolve() in skip:
            skipped += 1
            continue
        scanned += 1
        lines = f.read_text(encoding="utf-8", errors="replace").splitlines()
        for event in walk(lines):
            if event[0] != "line":
                continue
            _, idx, code, marker = event
            if marker:
                continue  # инлайн-исключение `// lay7:allow <причина>`
            if not code.strip() or code.lstrip().startswith("//"):
                continue  # строка-комментарий целиком
            raw = lines[idx].strip()
            for kind in line_hits(code):
                (gap_hits if kind == "gap" else cursor_hits).append((f, idx + 1, raw))
    return gap_hits, cursor_hits, scanned, skipped


def _display(p: Path) -> str:
    try:
        return p.resolve().relative_to(REPO).as_posix()
    except ValueError:
        return str(p)


def main(argv: list[str]) -> int:
    targets = argv[1:] or list(DEFAULT_ROOTS)
    paths = [p if p.is_absolute() else REPO / p for p in map(Path, targets)]
    gap_hits, cursor_hits, scanned, skipped = scan(paths)
    if scanned == 0 and skipped == 0:
        print(
            "[lay7-scan] ошибка: не найдено ни одного *.rs по путям "
            f"{' '.join(targets)} — проверьте аргументы (тихо-зелёный гейт недопустим)",
            file=sys.stderr,
        )
        return 1

    print("--- LAY7: gap literals ---")
    if gap_hits:
        print(
            "FAIL: найдены литеральные gap-значения вне 0.0 "
            "(должны быть 0.0 или токены S1):"
        )
        for f, ln, frag in gap_hits:
            print(f"  {_display(f)}:{ln}: {frag}")
    else:
        print("OK — литеральных gap вне 0.0 нет")

    print("--- LAY7: cursors (x/y += с дробным литералом) ---")
    if cursor_hits:
        print("FAIL: найден ручной курсор x/y += с дробным литералом (LAY2/LAY10):")
        for f, ln, frag in cursor_hits:
            print(f"  {_display(f)}:{ln}: {frag}")
    else:
        print("OK — курсоров x/y += с дробным литералом нет")

    total = len(gap_hits) + len(cursor_hits)
    print()
    if total:
        print(
            f"LAY7 lint: FAIL — нарушений: {total} "
            f"(файлов просканировано: {scanned}, тест-модулей пропущено: {skipped})"
        )
        print("  Инлайн-исключение — `// lay7:allow <причина>` в конце строки.")
        return 1
    print(
        f"LAY7 lint: OK — нарушений LAY7 нет "
        f"(файлов просканировано: {scanned}, тест-модулей пропущено: {skipped})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
