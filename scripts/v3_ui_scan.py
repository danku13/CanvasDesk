#!/usr/bin/env python3
# -*- coding: utf-8 -*-
r"""v3_ui_scan — статический сканер гейтов V3 правила IN13
(design/rules/09-input.md, §IN13; issue #68 «UI-вёрстка: системный канон
вертикали», волна V3 — по образцу scripts/lay7_scan.py: скан файла ЦЕЛИКОМ,
тестовый код отсекается по mod-границам, комментарии/строки выбрасываются).

Три правила (волна V2 мигрировала 153 label-вызова и 7 кареток на канон
kit::text_area_vcentered/caret_rect; гейт не даёт классу вернуться):

  V3-L1 — «полный-rect label»: вызовы `.label_center(`/`.label_left(`
          (KitDraw) в прод-коде, область которых НЕ выведена из канона:
          в скобках вызова нет `text_area_vcentered` И первый аргумент —
          не переменная, связанная через `let … = … text_area_vcentered …`
          (узаконенная двухстрочная форма). Канон: контрол-подпись строится
          от строкового бокса text_area_vcentered(rect, size); осознанные
          топ-анкерные стеки (IN11) — только с инлайн-маркером `// v3:allow`.

  V3-L2 — «магические вертикальные офсеты» на текст/каретка-сайтах:
          литеральные вертикальные сдвиги `y + N` / `y − N` / `…[1] ± N`
          (N в диапазоне 0.5…12.5 — целые и дробные), ручные центрирующие
          формулы `(h − 14.0) / 2`, `h/2.0 − 7.0`, `(h − x) * 0.5`,
          литеральный dy в push_line(..., N). САЙТ определяется контекстом:
            * та же строка несёт текст-маркер (origin:, caret, text_area,
              font_size, ScreenText/OwnedScreenText, TextAlign, texts.push,
              push_line, .label(, .icon(, label_center/label_left);
            * строка внутри скобочного спана вызова label_center/label_left/
              .label(/.icon( (область/формула передана прямо в вызов);
            * строка внутри многострочного литерала `origin: [ … ]`
              (OwnedScreenText/ScreenText — «~70 ручных пушей», хвост V2-C);
            * строка — let-присваивание идентификатора, потреблённого как
              первый аргумент icon/label-вызова (bolt_rect/area-паттерн
              d.icon-областей, хвост V2).
          НЕ нарушение: переменные смещения (`y += line_h`, `py + dy`),
          токен-офсеты (`y + SPACING_SM`, `dy + DIALOG_TITLE_Y`), 0.0,
          x-офсеты (`[0] ± N`, `x + 8.0`), ширины (`w − 16.0`).

  V3-L3 — «литералы фактора строки»: умножения `* 1.3` / `1.3 *` /
          `* 1.1` / `1.1 *` в контексте строки/шрифта/высоты/кегля
          вне crates/canvas-ui/src/measure.rs (единственный владелец
          SCREEN_LINE_FACTOR). 1.1/1.3 в ином смысле (анимационные/цветовые
          коэффициенты, как hover_fill в support.rs) отсекаются требованием
          контекста line/font/height/size/«h» на той же строке.

Исключения — только инлайн: `// v3:allow <причина>` в конце строки
(прецедент lay7:allow). Для L1 маркер в ЛЮБОЙ строке скобочного спана
вызова гасит вызов (хит L1 — вызов, а не строка); для L2/L3 маркер гасит
только свою строку. Файловых allowlist нет.

Rustfmt-устойчивость: rustfmt переносит хвостовой комментарий с первой
строки многострочного вызова/statement на САМОСТОЯТЕЛЬНУЮ строку сразу
после закрывающей скобки. Такой standalone-маркер считается маркером
ПРЕДЫДУЩЕГО statement: для L1 — гасит вызов, закрытый в пределах 3
строк-событий до маркера; для L2 — гасит хиты на строках в пределах 6
строк до маркера (хит внутри statement, маркер после его закрытия).

Осознанные границы (документированы):
  * crates/canvas-ui/src/measure.rs — владелец канона: из скана исключён
    целиком (формулы text_area_vcentered/caret_rect — ЭТАЛОН, L3 — явно
    по постановке задачи);
  * смещение, проведённое через переменную (push_line(line1, dy) с dy
    из вызывающего кода — wheel-меню), литералом в y-позиции не является:
    ловится только прямой литерал `push_line(line1, 4.0)`;
  * офсеты ВНЕ текст-контекста (раскладка панелей, `(vp − h) / 2` окон) —
    зона LAY-правил, не IN13;
  * statement ≈ физическая строка: перенос `+ 6.0` хвостом на следующую
    строку вне спана вызова/origin-литерала не ловится (как в lay7);
  * let-связывание разрешается по имени ИДЕНТИФИКАТОРА (ближайшее
    предшествующее связывание): переиспользование имени для разных
    геометрий в одной функции даст ложный пропуск — стиль кода это
    исключает.

Запуск:  python3 scripts/v3_ui_scan.py [ПУТЬ ...]  (по умолчанию
         crates/canvas-app/src crates/canvas-ui/src; ПУТЬ — файл/каталог)
Выход:   0 — нарушений нет; 1 — есть (список file:line: fragment).
Negative-тест гейта: bash scripts/v3_ui_lint.sh --selftest (фикстуры вне репо).
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_ROOTS = ("crates/canvas-app/src", "crates/canvas-ui/src")

# Владелец канона вертикали (issue #68, V1): формулы text_area_vcentered/
# caret_rect/line_metrics — эталон, а не нарушение. Исключён из всех правил.
CANON_BASENAME = "measure.rs"

# --- лексер: код без комментариев и строковых литералов ---------------------
# (прецедент lay7_scan.py — блок-комментарии, raw-строки, чар-литералы)


class LexState:
    """Межстрочное состояние лексера (блочные комментарии, raw-строки)."""

    __slots__ = ("block_depth", "str_hash")

    def __init__(self) -> None:
        self.block_depth = 0
        self.str_hash = 0  # k>=1 — внутри r#"…"# из k решёток


RE_ALLOW = re.compile(r"v3:allow\b")


def _is_word(ch: str) -> bool:
    return ch.isalnum() or ch == "_"


def _mask_plain_string(line: str, i: int) -> int:
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
    j = i + 1
    n = len(line)
    k = 0
    while j < n and line[j] == "#":
        k += 1
        j += 1
    if j < n and line[j] == '"':
        return k, j
    return None, None


def _mask_raw_string(line: str, i: int, k: int, st: LexState) -> int:
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
    if k > 0:
        st.str_hash = k
    return n


def _char_literal_end(line: str, i: int) -> int | None:
    n = len(line)
    if i + 1 >= n:
        return None
    c = line[i + 1]
    if c == "\\":
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
        return i + 3
    return None


def mask_line(line: str, st: LexState) -> tuple[str, bool]:
    """Строка → (код без комментариев и содержимого строк, маркер v3:allow)."""
    out: list[str] = []
    marker = False
    i = 0
    n = len(line)

    if st.str_hash:
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


# --- отсечка тест-модов (прецедент lay7_scan.py) -----------------------------

RE_ATTR = re.compile(r"^\s*#\s*\[\s*cfg\s*\((.*)\)\s*\]\s*(.*)$")
RE_MOD_DECL = re.compile(r"^(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*([;{]?)\s*$")
RE_TEST = re.compile(r"\btest\b")


def walk(lines: list[str]):
    """Обход строк файла с пропусканием инлайн cfg(test)-модов."""
    st = LexState()
    pending_attr = False
    pending_mod: str | None = None
    skip_depth = 0
    for idx, raw in enumerate(lines):
        code, marker = mask_line(raw, st)
        if skip_depth > 0:
            skip_depth += code.count("{") - code.count("}")
            if skip_depth <= 0:
                skip_depth = 0
                pending_attr = False
                pending_mod = None
            continue
        stripped = code.strip()
        if not stripped:
            # Пустая по коду строка (пустая либо только комментарий):
            # standalone `// v3:allow` должен быть событием — rustfmt
            # переносит хвостовые маркеры на отдельную строку после
            # закрывающей скобки statement (см. докстринг модуля).
            if marker:
                yield ("line", idx, code, marker)
            continue
        handled = False
        m = RE_ATTR.match(code)
        if m is not None:
            rest = m.group(2).strip()
            if not rest:
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
                        continue
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
                    pending_mod = name
                    continue
            if pending_mod is not None and stripped.startswith("{"):
                skip_depth = max(code.count("{") - code.count("}"), 0)
                pending_attr = False
                pending_mod = None
                continue
            pending_attr = False
            pending_mod = None
        yield ("line", idx, code, marker)


# --- правила -----------------------------------------------------------------

# V3-L1: вызовы KitDraw::label_center/label_left (всегда через приёмник
# `d.`/`p.`/`self.`; декларация `pub fn label_center(` без точки не совпадает).
RE_LABEL_CALL = re.compile(r"\.\s*label_(center|left)\s*\(")
# Спаны для L2: label-вызовы + примитив Painter::label + области иконок.
RE_SPAN_CALL = re.compile(r"\.\s*(label_center|label_left|label|icon)\s*\(")
# Первый аргумент — голый идентификатор (let-разрешение): на строке старта
# после открывающей скобки либо на продолжении (строка начинается с идента).
RE_FIRST_ARG_IDENT = re.compile(r"\(\s*([a-z_][a-z0-9_]*)\s*(?:,|\))")
RE_FIRST_ARG_CONT = re.compile(r"^\s*([a-z_][a-z0-9_]*)\s*(?:,|\))")
# let-связывание: `let ident =` / `let mut ident =` / `let ident: T =`.
RE_LET = re.compile(r"\blet\s+(?:mut\s+)?([a-z_][a-z0-9_]*)\s*:?[^=]*=")

# V3-L2: вертикальный офсет — y-терм (любой идентификатор на y: y/rect.y/head_y/py)
# с ЛИТЕРАЛОМ в его аддитивном выражении (до `,`/скобки): ловит и прямой
# `y + 8.0`, и смешанный `y + MENU_PADDING + 5.0`, и `card[1] + h/2.0 - 8.5`;
# `y + 40.0` (шаг стека вне диапазона) и чисто-переменные `y + line_h` НЕ
# совпадают. Ленивый квантор `[^,()]*?` привязывает литерал к y-терму, а не к
# соседнему x-слагаемому строки. ЦЕЛОЧИСЛЕННЫЕ индексы массивов (`win[3]`,
# `list[12]`) маскируются до скана — их цифры не литералы (кроме `[…]` с
# ровно `1` — сам y-индекс RE_IDX_Y).
RE_Y_OFF = re.compile(
    r"\b[a-z0-9_]*y\b\s*[-+][^,()]*?(\d+(?:\.\d+)?)(?:f32|f64)?"
)
# y-компонента массива-rect'а: `rect[1] + 4.0` / `center[1] - 6.0` /
# смешанный `rect[1] + PAD + 5.0`
RE_IDX_Y = re.compile(
    r"\[\s*1\s*\]\s*[-+][^,()]*?(\d+(?:\.\d+)?)(?:f32|f64)?"
)
# y-компонента с ручной центрирующей формулой в скобках:
# `rect[1] + (rect[3] - 16.0) * 0.5` / `rect[1] + (rect[3] - 12.0) / 2`
RE_IDX_Y_CENTER = re.compile(
    r"\[\s*1\s*\]\s*[-+]\s*\([^()]*[-+]\s*\d[^()]*\)\s*(?:\*\s*0\.5\b|/\s*2(?:\.0)?\b)"
)
# маска целочисленных индексов массивов (НЕ равных ровно 1)
RE_INDEX = re.compile(r"\[\s*(\d+)\s*\]")
# ручные центрирующие формулы: (h - 14.0) / 2, (row.h - 18.0)/2.0
RE_CENTER_DIV = re.compile(
    r"\(\s*[^()]*\b[a-z0-9_]*[hH]\b[^()]*[-+]\s*\d[^()]*\)\s*/\s*2(?:\.0)?\b"
)
# h/2.0 - 7.0
RE_CENTER_HALF = re.compile(
    r"\b[a-z0-9_]*[hH]\b\s*/\s*2(?:\.0)?\s*[-+]\s*\d+(?:\.\d+)?"
)
# (h - 12.5) * 0.5 — центрирование умножением на половину (только ВЫСОТА:
# вариант для ширин — горизонтальная вписка, не вертикаль OFF)
RE_CENTER_MUL = re.compile(
    r"\(\s*[^()]*\b[a-z0-9_]*[hH]\b[^()]*[-+]\s*\d[^()]*\)\s*\*\s*0\.5\b"
)
# литеральный dy позиционным аргументом push_line (wheel-меню: стек 2 строк)
RE_PUSH_DY = re.compile(
    r"\bpush_line\s*\(\s*[^,()]+\s*,\s*[-+]?\s*(\d+(?:\.\d+)?)(?:f32|f64)?\s*\)"
)
# маркеры текст/каретка-сайта на самой строке (вне спанов)
RE_TEXT_MARKER = re.compile(
    r"origin\s*:|\bcaret\b|caret_|text_area|font_size|ScreenText|TextAlign"
    r"|texts\.push|push_line|OwnedScreenText|\.\s*label\s*\(|\.\s*icon\s*\("
    r"|label_center\s*\(|label_left\s*\("
)
# многострочный литерал области текста: `origin: [` без баланса скобок в строке
RE_ORIGIN_OPEN = re.compile(r"origin\s*:\s*\[[^]\]]*$")

OFF_MIN, OFF_MAX = 0.5, 12.5  # диапазон магических вертикальных офсетов (IN13)


def _off_in_range(value: str) -> bool:
    try:
        v = float(value)
    except ValueError:
        return False
    return OFF_MIN <= v <= OFF_MAX


def _mask_indices(code: str) -> str:
    """Целочисленные индексы массивов (кроме ровно 1) → `[…]`:
    цифры индексов — не литералы офсетов (`win[3]`, `list[12]`)."""

    def repl(m: re.Match[str]) -> str:
        return "[…]" if m.group(1) != "1" else m.group(0)

    return RE_INDEX.sub(repl, code)


def l2_kinds(code: str) -> list[str]:
    """Виды L2-нарушений в строке текст-контекста (без комментариев/строк)."""
    code = _mask_indices(code)
    kinds: list[str] = []
    if any(
        _off_in_range(m.group(1))
        for m in RE_Y_OFF.finditer(code)
    ):
        kinds.append("y-offset")
    if any(
        _off_in_range(m.group(1))
        for m in RE_IDX_Y.finditer(code)
    ) or RE_IDX_Y_CENTER.search(code):
        kinds.append("rect[1]-offset")
    if RE_CENTER_DIV.search(code) or RE_CENTER_HALF.search(code) or RE_CENTER_MUL.search(code):
        kinds.append("v-center formula")
    if any(_off_in_range(m.group(1)) for m in RE_PUSH_DY.finditer(code)):
        kinds.append("push_line dy literal")
    return kinds


# V3-L3: множитель фактора строки 1.3/1.1 (двусторонний)
RE_FACTOR = re.compile(r"\*\s*1\.[13]\b|\b1\.[13]\b\s*\*")
# контекст строки/шрифта/высоты/кегля (гасит цветовые/анимационные 1.3/1.1)
RE_FACTOR_CTX = re.compile(r"(?i)line|font|height|size|\bh\b|_h\b")


def l3_hit(code: str) -> bool:
    return bool(RE_FACTOR.search(code) and RE_FACTOR_CTX.search(code))


# --- спаны вызовов / let-связывания / origin-литералов -----------------------


class Span:
    """Скобочный спан вызова label/icon-семейства."""

    __slots__ = ("kind", "start", "end", "first_arg", "lines")

    def __init__(self, kind: str, start: int, first_arg: str | None) -> None:
        self.kind = kind  # label_center | label_left | label | icon
        self.start = start  # индекс строки-старта (0-based)
        self.end = start  # индекс последней строки спана
        self.first_arg = first_arg  # голый идентификатор первого аргумента
        self.lines: list[tuple[int, str, bool]] = []  # (idx, code, marker)

    @property
    def text(self) -> str:
        return " ".join(code for _, code, _ in self.lines)


MAX_SPAN_LINES = 30  # страховка от незакрытых скобок


def collect_spans(events: list[tuple]) -> list[Span]:
    """Спаны label/icon-вызовов по списку событий walk().

    Скобки считаются по коду, очищенному от строк/комментарий, поэтому
    скобки внутри литералов счёт не сбивают. Новые спаны внутри открытого
    не открываются (в дереве вложенных label/icon-вызовов нет).
    """
    spans: list[Span] = []
    open_span: Span | None = None
    depth = 0
    head_done = False  # первый аргумент уже снят
    for ev in events:
        if ev[0] != "line":
            continue
        _, idx, code, marker = ev
        if open_span is None:
            m = RE_SPAN_CALL.search(code)
            if m is None:
                continue
            kind = m.group(1)
            open_paren = code.find("(", m.end() - 1)
            span = Span(kind, idx, None)
            tail = code[open_paren:]
            depth = tail.count("(") - tail.count(")")
            fa = RE_FIRST_ARG_IDENT.match(tail)
            if fa is not None:
                span.first_arg = fa.group(1)
                head_done = True
            elif len(tail) > 1:
                # на строке старта есть хвост аргументов без голого идента —
                # первый аргумент выражение, разрешать нечего
                head_done = True
            # иначе (хвост = ровно `(`) аргументы начинаются со следующей
            # строки — первый аргумент снимается на продолжении
            span.lines.append((idx, code, marker))
            span.end = idx
            if depth <= 0:
                spans.append(span)
                open_span = None
                head_done = False
            else:
                open_span = span
            continue
        # продолжение открытого спана
        if not head_done:
            fa = RE_FIRST_ARG_CONT.match(code)
            if fa is not None:
                open_span.first_arg = fa.group(1)
                head_done = True
        open_span.lines.append((idx, code, marker))
        open_span.end = idx
        depth += code.count("(") - code.count(")")
        if depth <= 0 or len(open_span.lines) >= MAX_SPAN_LINES:
            spans.append(open_span)
            open_span = None
            head_done = False
    return spans


MAX_LET_LINES = 8  # `let x = …;` до 8 строк-событий


class LetBinding:
    __slots__ = ("ident", "start", "end", "lines")

    def __init__(self, ident: str, start: int) -> None:
        self.ident = ident
        self.start = start
        self.end = start
        self.lines: list[tuple[int, str, bool]] = []

    @property
    def text(self) -> str:
        return " ".join(code for _, code, _ in self.lines)


def collect_lets(events: list[tuple]) -> dict[str, list[LetBinding]]:
    """let-связывания идентификаторов (текст до `;`, ≤ MAX_LET_LINES строк)."""
    lets: dict[str, list[LetBinding]] = {}
    open_let: LetBinding | None = None
    for ev in events:
        if ev[0] != "line":
            continue
        _, idx, code, marker = ev
        if open_let is None:
            m = RE_LET.search(code)
            if m is None:
                continue
            binding = LetBinding(m.group(1), idx)
            binding.lines.append((idx, code, marker))
            if code.count(";") > 0:
                lets.setdefault(binding.ident, []).append(binding)
            else:
                open_let = binding
            continue
        open_let.lines.append((idx, code, marker))
        open_let.end = idx
        if code.count(";") > 0 or len(open_let.lines) >= MAX_LET_LINES:
            lets.setdefault(open_let.ident, []).append(open_let)
            open_let = None
    return lets


def resolve_let(
    lets: dict[str, list[LetBinding]], ident: str | None, before: int
) -> LetBinding | None:
    """Ближайшее ПРЕДШЕСТВУЮЩЕЕ let-связывание идентификатора (или None)."""
    if ident is None:
        return None
    candidates = [b for b in lets.get(ident, []) if b.start < before]
    return candidates[-1] if candidates else None


MAX_ORIGIN_LINES = 10  # многострочный `origin: [ … ]` до 10 строк-событий


def origin_continuation_lines(events: list[tuple]) -> set[int]:
    """Строки-продолжения многострочных `origin: [ … ]` ( OwnedScreenText )."""
    out: set[int] = set()
    depth = 0
    in_literal = False
    taken = 0
    for ev in events:
        if ev[0] != "line":
            continue
        _, idx, code, _marker = ev
        if in_literal:
            out.add(idx)
            depth += code.count("[") - code.count("]")
            taken += 1
            if depth <= 0 or taken >= MAX_ORIGIN_LINES:
                in_literal = False
                depth = 0
                taken = 0
            continue
        if RE_ORIGIN_OPEN.search(code):
            in_literal = True
            depth = 1  # открытая `[` стартовой строки
            taken = 0
    return out


# --- скан дерева -------------------------------------------------------------


def gather_files(paths: list[Path]) -> list[Path]:
    files: list[Path] = []
    seen: set[Path] = set()
    for p in paths:
        if p.is_file() and p.suffix == ".rs":
            resolved = p.resolve()
            if resolved not in seen and p.name != CANON_BASENAME:
                seen.add(resolved)
                files.append(p)
        elif p.is_dir():
            for f in sorted(p.rglob("*.rs")):
                if f.is_file() and f.name != CANON_BASENAME:
                    resolved = f.resolve()
                    if resolved not in seen:
                        seen.add(resolved)
                        files.append(f)
        else:
            print(f"[v3-scan] предупреждение: путь не найден: {p}", file=sys.stderr)
    return files


def module_file_candidates(decl_file: Path, name: str) -> set[Path]:
    parent = decl_file.parent
    stem = decl_file.stem
    base = parent if stem in ("lib", "main", "mod") else parent / stem
    return {base / f"{name}.rs", base / name / "mod.rs"}


def scan_file(
    f: Path, lines: list[str]
) -> tuple[list[tuple[int, str]], list[tuple[int, str]], list[tuple[int, str]]]:
    """L1/L2/L3-хиты одного файла (номер строки с 1, фрагмент)."""
    events = [ev for ev in walk(lines) if ev[0] == "line"]
    spans = collect_spans(events)
    lets = collect_lets(events)
    origin_cont = origin_continuation_lines(events)

    l1: list[tuple[int, str]] = []
    l2: list[tuple[int, str]] = []
    l3: list[tuple[int, str]] = []

    # Standalone `// v3:allow`-строки (rustfmt переносит хвостовой маркер
    # с первой строки многострочного statement на строку после закрытия):
    # маркер ПРЕДЫДУЩЕГО statement (см. докстринг модуля).
    standalone_markers: set[int] = {
        idx for _, idx, code, marker in events if marker and not code.strip()
    }

    def allow_after_standalone(idx: int, window: int) -> bool:
        """Standalone-маркер в пределах window строк-событий после idx."""
        return any(n in standalone_markers for n in range(idx + 1, idx + 1 + window))

    # --- V3-L1: label-вызовы вне text_area_vcentered -------------------------
    l1_reported: set[int] = set()
    for span in spans:
        if span.kind not in ("label_center", "label_left"):
            continue
        if any(marker for _, _, marker in span.lines):
            continue  # `// v3:allow <причина>` в спане вызова
        if allow_after_standalone(span.end, 3):
            continue  # rustfmt вынес маркер на standalone-строку после `);`
        if "text_area_vcentered" in span.text:
            continue  # канон: строковый бокс прямо в вызове
        binding = resolve_let(lets, span.first_arg, span.start)
        if binding is not None and "text_area_vcentered" in binding.text:
            continue  # канон: let ta = text_area_vcentered(…); label(ta, …)
        line_no = span.start + 1
        l1_reported.add(span.start)  # 0-based индекс строки-старта
        l1.append((line_no, lines[span.start].strip()))

    # --- V3-L2: магические офсеты на текст/каретка-сайтах --------------------
    # Сайт, уже отчитанный L1 (label-вызов вне канона), не дублируется в L2:
    # это ОДИН дефект «ручная геометрия подписи», хит L1 его несёт.
    l1_span_lines: set[int] = set()
    for span in spans:
        if span.start in l1_reported:
            l1_span_lines.update(idx for idx, _, _ in span.lines)
    context: set[int] = set()
    for span in spans:
        if span.start in l1_reported:
            continue
        context.update(idx for idx, _, _ in span.lines)
        if span.kind in ("label_center", "label_left", "icon"):
            binding = resolve_let(lets, span.first_arg, span.start)
            if binding is not None:
                context.update(idx for idx, _, _ in binding.lines)
    context |= origin_cont
    context -= l1_span_lines

    l2_reported: set[int] = set()
    for _, idx, code, marker in events:
        if idx in l2_reported:
            continue
        if marker:
            continue  # `// v3:allow` гасит только свою строку — но гасит
        if allow_after_standalone(idx, 6):
            continue  # rustfmt-вариант: маркер standalone-строкой после statement
        in_context = idx in context or RE_TEXT_MARKER.search(code) is not None
        if not in_context:
            continue
        kinds = l2_kinds(code)
        if kinds:
            l2_reported.add(idx)
            l2.append((idx + 1, lines[idx].strip()))

    # --- V3-L3: литералы фактора строки вне measure.rs -----------------------
    for _, idx, code, marker in events:
        if marker:
            continue
        if l3_hit(code):
            l3.append((idx + 1, lines[idx].strip()))

    return l1, l2, l3


def scan(
    paths: list[Path],
) -> tuple[
    list[tuple[Path, int, str]],
    list[tuple[Path, int, str]],
    list[tuple[Path, int, str]],
    int,
    int,
]:
    """Два прохода: (1) внешние тест-модули, (2) скан остального."""
    files = gather_files(paths)

    skip: set[Path] = set()
    for f in files:
        lines = f.read_text(encoding="utf-8", errors="replace").splitlines()
        for event in walk(lines):
            if event[0] == "external":
                skip |= {c.resolve() for c in module_file_candidates(f, event[1])}

    l1_all: list[tuple[Path, int, str]] = []
    l2_all: list[tuple[Path, int, str]] = []
    l3_all: list[tuple[Path, int, str]] = []
    scanned = 0
    skipped = 0
    for f in files:
        if f.resolve() in skip:
            skipped += 1
            continue
        scanned += 1
        lines = f.read_text(encoding="utf-8", errors="replace").splitlines()
        l1, l2, l3 = scan_file(f, lines)
        l1_all.extend((f, ln, frag) for ln, frag in l1)
        l2_all.extend((f, ln, frag) for ln, frag in l2)
        l3_all.extend((f, ln, frag) for ln, frag in l3)
    return l1_all, l2_all, l3_all, scanned, skipped


def _display(p: Path) -> str:
    try:
        return p.resolve().relative_to(REPO).as_posix()
    except ValueError:
        return str(p)


def main(argv: list[str]) -> int:
    targets = argv[1:] or list(DEFAULT_ROOTS)
    paths = [p if p.is_absolute() else REPO / p for p in map(Path, targets)]
    l1, l2, l3, scanned, skipped = scan(paths)
    if scanned == 0 and skipped == 0:
        print(
            "[v3-scan] ошибка: не найдено ни одного *.rs по путям "
            f"{' '.join(targets)} — проверьте аргументы (тихо-зелёный гейт недопустим)",
            file=sys.stderr,
        )
        return 1

    print("--- V3-L1: full-rect labels (label_center/label_left вне text_area_vcentered) ---")
    if l1:
        print("FAIL: label-вызовы с областью не из kit::text_area_vcentered (IN13/IN11):")
        for f, ln, frag in l1:
            print(f"  {_display(f)}:{ln}: {frag}")
    else:
        print("OK — label-вызовов вне text_area_vcentered нет")

    print("--- V3-L2: magic vertical offsets at text/caret sites ---")
    if l2:
        print(
            "FAIL: литеральные вертикальные офсеты/ручные v-центры на "
            "текст/каретка-сайтах (IN13):"
        )
        for f, ln, frag in l2:
            print(f"  {_display(f)}:{ln}: {frag}")
    else:
        print("OK — магических вертикальных офсетов на текст-сайтах нет")

    print("--- V3-L3: line-height factor literals (×1.3/×1.1) outside measure.rs ---")
    if l3:
        print("FAIL: литерал фактора строки 1.3/1.1 вне measure.rs (IN11/IN13):")
        for f, ln, frag in l3:
            print(f"  {_display(f)}:{ln}: {frag}")
    else:
        print("OK — литералов фактора строки вне measure.rs нет")

    total = len(l1) + len(l2) + len(l3)
    print()
    if total:
        print(
            f"V3 UI lint: FAIL — нарушений: {total} "
            f"(L1: {len(l1)}, L2: {len(l2)}, L3: {len(l3)}; "
            f"файлов просканировано: {scanned}, тест-модулей пропущено: {skipped})"
        )
        print("  Инлайн-исключение — `// v3:allow <причина>` в конце строки.")
        return 1
    print(
        f"V3 UI lint: OK — нарушений IN13 нет "
        f"(файлов просканировано: {scanned}, тест-модулей пропущено: {skipped})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
