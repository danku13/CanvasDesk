#!/usr/bin/env python3
"""Аудит вложенных захватов глобального MEASURE_FS (Mutex<FontSystem>).

Класс бага (прецеденты: a11f6c9 2026-09-26 — галерея; хотфикс 2026-10-11 —
чип/менеджер C3/C4 + kit_field_view): функция, держащая guard
`measure_font_system()`, вызывает функцию, лочащую тот же глобальный Mutex
внутри себя. Последствия: wasm32 (std no_threads) — паника «cannot
recursively acquire mutex» + abort; натив — дедлок кадра RedrawRequested.

Эвристика (консервативная, с ручной разборкой):
  1. Собираем все функции, чьё тело содержит вызов `measure_font_system()`
     — «лочащие» (locker).
  2. Во всех телах функций ищем вызовы locker'ов (по простому имени —
     коллизии имён возможны, это сознательная плоскость шума).
  3. Помечаем пару, если вызывающая функция САМА лочит
     (`measure_font_system()` в её теле) — кандидат на вложенный захват.
  4. Каждое попадание разбирается вручную (скоупы блоков не трекаются);
     подтверждённые безопасные пары — в ALLOWLIST с причиной.

Выход: список кандидатов; exit 1, если есть неподтверждённые.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CRATES = ROOT / "crates"

FN_RE = re.compile(
    r"(?:(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:unsafe\s+)?(?:async\s+)?fn\s+)(\w+)\s*[<\(]"
)
LOCK_RE = re.compile(r"measure_font_system\s*\(\s*\)")

# Подтверждённо безопасные попадания (ручная разборка): "file:line" → причина.
# ЗАПОЛНЯЕТСЯ ПОСЛЕ РАЗБОРКИ; удаление записи = повторная разборка.
ALLOWLIST: dict[str, str] = {
    # Ручная разборка 2026-10-11 (хотфикс MEASURE_FS, issue #56). Ключ —
    # «файл :: вызывающая -> locker» (стабилен к сдвигу строк). Все записи —
    # проверенные ПОСЛЕДОВАТЕЛЬНЫЕ захваты: блок-скоуп/ветка закрывается
    # до вызова locker'а, либо вызов идёт ДО взятия guard'а.
    "crates/canvas-render/src/text.rs :: desc_zone_expanded_shows_full_text -> measure_body_height": (
        "guard в отдельном блоке (комментарий «гвард дропаем до вызова "
        "меры» прямо в тесте), вызов locker'а после закрытия блока"
    ),
    "crates/canvas-app/src/app/input.rs :: click_settings -> modal_layout_scrolled_with_settings": (
        "вызов без guard'а; guard взят позже в теле (другая ветка)"
    ),
    "crates/canvas-app/src/app/input.rs :: on_mouse_wheel -> modal_layout_with_settings": (
        "guard'ы ранних веток (scheme gallery) завершаются return до "
        "ветки settings — не живут в момент вызова"
    ),
    "crates/canvas-app/src/app/overlays.rs :: settings_overlay -> modal_layout_scrolled_with_settings": (
        "guard — в блоке hotkeys (закрыт до вызова; см. комментарий "
        "про hotkeys_left_offset над guard'ом)"
    ),
    "crates/canvas-app/src/app/stage.rs :: stage_frame -> template_chip_label_w": (
        "вызов ДО guard'а (комментарий: «template_chip_label_w берёт "
        "guard пула сам»)"
    ),
    "crates/canvas-app/src/app/tooltip.rs :: box_hugs_text_and_lines_stack -> layout_tooltips": (
        "layout_tooltips вызван ДО guard'а (guard ниже — для собственных "
        "замеров)"
    ),
    "crates/canvas-app/src/app/tooltip.rs :: long_token_box_grows_no_silent_clip -> layout_tooltips": (
        "layout_tooltips вызван ДО guard'а (guard ниже — для собственных "
        "замеров)"
    ),
    "crates/canvas-app/src/app/ui_registry.rs :: fill_hit_rects -> modal_layout_scrolled_with_settings": (
        "guard — в match-arm'е id::HOTKEYS (закрыт до arm'а id::SETTINGS, "
        "где вызов locker'а)"
    ),
}


def strip_noise(src: str) -> str:
    """Убрать комментарии и строковые литералы (грубая машина состояний)."""
    out: list[str] = []
    i, n = 0, len(src)
    state = "code"
    while i < n:
        c = src[i]
        nxt = src[i + 1] if i + 1 < n else ""
        if state == "code":
            if c == "/" and nxt == "/":
                state = "line"
                i += 2
                continue
            if c == "/" and nxt == "*":
                state = "block"
                i += 2
                continue
            if c == '"':
                state = "str"
                i += 1
                continue
            out.append(c)
            i += 1
        elif state == "line":
            if c == "\n":
                out.append("\n")
                state = "code"
            i += 1
        elif state == "block":
            if c == "*" and nxt == "/":
                state = "code"
                i += 2
                continue
            if c == "\n":
                out.append("\n")
            i += 1
        elif state == "str":
            if c == "\\":
                i += 2
                continue
            if c == '"':
                state = "code"
            i += 1
    return "".join(out)


def fn_bodies(src: str) -> list[tuple[str, int, str]]:
    """[(имя, строка начала, тело)] — тело от сигнатурной `{` до парной `}`."""
    result: list[tuple[str, int, str]] = []
    for m in FN_RE.finditer(src):
        name = m.group(1)
        line = src.count("\n", 0, m.start()) + 1
        # FN_RE заканчивается на «(» параметров: сначала добрать
        # сбалансированный список параметров (типы вида [f32; 2] содержат
        # «;» — наивный сканер принимал его за bodyless-объявление),
        # затем — до первой «{» (тип возврата скобок не открывает).
        j = m.end()
        depth = 1  # уже съели «(» из FN_RE
        while j < len(src) and depth > 0:
            if src[j] == "(":
                depth += 1
            elif src[j] == ")":
                depth -= 1
            j += 1
        while j < len(src) and src[j] != "{":
            if src[j] == ";":
                j = 0  # bodyless (trait-объявление) — пометить и выйти
                break
            j += 1
        if j == 0 or j >= len(src):
            continue
        depth = 0
        k = j
        while k < len(src):
            if src[k] == "{":
                depth += 1
            elif src[k] == "}":
                depth -= 1
                if depth == 0:
                    break
            k += 1
        result.append((name, line, src[j + 1 : k]))
    return result


def line_of(src: str, offset: int) -> int:
    return src.count("\n", 0, offset) + 1


def main() -> int:
    files = sorted(CRATES.rglob("*.rs"))
    # 1. собрать locker'ы: имя fn → файл (коллизии имён склеиваются —
    #    аудит по имени, разборка ручная).
    lockers: set[str] = set()
    per_file_fns: dict[Path, list[tuple[str, int, str]]] = {}
    for path in files:
        src = strip_noise(path.read_text(encoding="utf-8", errors="replace"))
        fns = fn_bodies(src)
        per_file_fns[path] = fns
        for _name, _line, body in fns:
            if LOCK_RE.search(body):
                lockers.add(_name)

    findings: list[str] = []
    for path, fns in per_file_fns.items():
        rel = path.relative_to(ROOT)
        for name, _line, body in fns:
            if not LOCK_RE.search(body):
                continue  # вызывающая не держит guard — вызовы locker'ов безопасны
            for locker in sorted(lockers):
                if locker == name:
                    continue
                for cm in re.finditer(
                    r"(?<![\w.:])" + re.escape(locker) + r"\s*\(", body
                ):
                    ln = _line + body.count("\n", 0, cm.start())
                    key = f"{rel} :: {name} -> {locker}"
                    if key in ALLOWLIST:
                        continue
                    findings.append(
                        f"{key} (строка {ln}): держит guard и вызывает "
                        f"locker — проверить скоупы (вложенный захват?)"
                    )

    lockers_list = ", ".join(sorted(lockers))
    print(f"Лочащие функции (measure_font_system в теле): {lockers_list}")
    print(f"Файлов просканировано: {len(files)}")
    if findings:
        print(f"\nКандидаты на вложенный захват ({len(findings)}):")
        for f in findings:
            print(f"  {f}")
        print(
            "\nРазобрать каждое попадание вручную: блок-скоупы не трекаются.\n"
            "Безопасные пары — добавить в ALLOWLIST с причиной; реальные\n"
            "вложения — чинить передачей (m, fs) параметром (конвенция\n"
            "wrap_lines_with_more, palette.rs §CR-015)."
        )
        return 1
    print("Вложенных захватов не найдено (с учётом ALLOWLIST).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
