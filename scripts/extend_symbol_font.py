#!/usr/bin/env python3
"""Ревизия ctrl+i (2026-10-09, UR-005): расширение сабсета CanvasDesk Symbols.

Проблема: AI-поверхности (агент-панель Ctrl+I, статусная панель AI) используют
глифы ⚡ (U+26A1), ➤ (U+27A4), ⏸ (U+23F8), ▶ (U+25B6) как glyph-fallback
(IconStyle::Glyph — дефолт). Эти глифы отсутствовали и в Noto Sans Display,
и в сабсете CanvasDeskSymbols (21 символ из Noto Sans Symbols/Symbols 2/Math,
оригиналы которых недоступны) — в wasm встроенных лиц нет, системного фолбэка
тоже нет → рисовался тофу (пустой бокс) вместо иконки.

Решение: дополнить сабсет четырьмя глифами. Контуры — оригинальные
геометрические силуэты (простые полигоны в стиле соседних глифов сабсета —
▲ triagup 805×697 @ advance 907), начерченные программно через fontTools
TTGlyphPen; заимствований контуров нет (лицензия файла не меняется — OFL).

Запуск:  python3 scripts/extend_symbol_font.py
Проверка: python3 - <<'EOF' (см. README-блок в конце файла) или cargo test
glyph_symbols_subset_covers_ai_panels (canvas-render).
"""

from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.pens.ttGlyphPen import TTGlyphPen

FONT = Path(__file__).resolve().parent.parent / "assets/fonts/CanvasDeskSymbols-Regular.ttf"

# Точка отсчёта геометрии — левый край силуэтов сабсета (▲/⚙ начинаются с 51).
X0 = 51
# Вертикальный размах крупных символов сабсета (▲: 0..697).
H = 697


def poly_pen(points: list[tuple[int, int]]) -> TTGlyphPen:
    """Замкнутый полигон по списку точек (по часовой в font-координатах)."""
    pen = TTGlyphPen(None)
    pen.moveTo(points[0])
    for p in points[1:]:
        pen.lineTo(p)
    pen.closePath()
    return pen


def glyph_play() -> tuple[TTGlyphPen, int]:
    """▶ U+25B6 — сплошной треугольник вправо (большой сосед ▸ uni25B8)."""
    pts = [(X0, 0), (X0, H), (856, H // 2)]
    return poly_pen(pts), 907


def glyph_pause() -> tuple[TTGlyphPen, int]:
    """⏸ U+23F8 — две вертикальные скруглённые полосы (уровень ▲ по высоте)."""
    pen = TTGlyphPen(None)
    r = 44  # радиус скругления углов полос
    for x_left in (X0, 556):
        x_right = x_left + 300
        # Полоса со скруглением: против часовой не важно — один контур,
        # порядок точек по часовой (y вверх: старт верх-лево → верх-право…).
        pen.moveTo((x_left + r, H))
        pen.lineTo((x_right - r, H))
        pen.qCurveTo((x_right, H), (x_right, H - r))
        pen.lineTo((x_right, r))
        pen.qCurveTo((x_right, 0), (x_right - r, 0))
        pen.lineTo((x_left + r, 0))
        pen.qCurveTo((x_left, 0), (x_left, r))
        pen.lineTo((x_left, H - r))
        pen.qCurveTo((x_left, H), (x_left + r, H))
        pen.closePath()
    return pen, 907


def glyph_zap() -> tuple[TTGlyphPen, int]:
    """⚡ U+26A1 — болт-зигзаг (силуэт «молнии» в стиле Material bolt,
    масштаб — уровень крупных символов сабсета)."""
    # Полигон болта: нижний левый край → выемка → верхняя кромка → ступени
    # правого нижнего крыла. Координаты подобраны под bbox 51..432 × 0..700.
    pts = [
        (199, 0),
        (160, 0),
        (199, 272),
        (63, 272),
        (X0, 289),
        (277, 700),
        (316, 700),
        (277, 428),
        (413, 428),
        (432, 408),
        (277, 0),
    ]
    return poly_pen(pts), 520


def glyph_send() -> tuple[TTGlyphPen, int]:
    """➤ U+27A4 — сплошная стрелка вправо: хвост + треугольная головка
    (один контур, силуэт в стиле ▲/▶ сабсета)."""
    pts = [
        (X0, 300),
        (320, 300),
        (320, 80),
        (900, 400),
        (320, 720),
        (320, 500),
        (X0, 500),
    ]
    # Разворот в clockwise (TrueType-конвенция внешнего контура).
    pts = list(reversed(pts))
    return poly_pen(pts), 957


# (codepoint, glyph-name, factory) — имена глифов согласованы с существующими
# (uniXXXX для символов, мнемоника для треугольников сабсета).
ADDITIONS = [
    (0x26A1, "uni26A1", glyph_zap),
    (0x27A4, "uni27A4", glyph_send),
    (0x23F8, "uni23F8", glyph_pause),
    (0x25B6, "uni25B6", glyph_play),
]


def main() -> None:
    font = TTFont(FONT)
    cmap_tables = [t for t in font["cmap"].tables if t.isUnicode()]
    glyf = font["glyf"]
    glyph_order = font.getGlyphOrder()
    hmtx = font["hmtx"]

    added: list[str] = []
    new_glyphs: list[tuple[str, object, int, int]] = []  # (name, glyph, advance, cp)
    for cp, name, factory in ADDITIONS:
        if any(cp in t.cmap for t in cmap_tables):
            # Идемпотентность: глиф уже есть (повторный запуск — no-op).
            print(f"skip {name} (U+{cp:04X} уже в cmap)")
            continue
        pen, advance = factory()
        # TTGlyphPen.glyph() компилирует контуры в глиф-объект (fontTools).
        new_glyphs.append((name, pen.glyph(), advance, cp))
        glyph_order.append(name)
        added.append(f"{name} U+{cp:04X} adv={advance}")

    if added:
        # Порядок глифов обновляется ДО присваивания (glyf-компиляция при
        # save сверяет glyphOrder и glyphs — рассинхрон = AssertionError).
        font.setGlyphOrder(glyph_order)
        for name, glyph, advance, cp in new_glyphs:
            glyf[name] = glyph
            hmtx[name] = (advance, 0)
            for t in cmap_tables:
                t.cmap[cp] = name
        font.save(FONT)
        print(f"Добавлено: {', '.join(added)}")
    else:
        print("Изменений нет (все глифы уже присутствуют)")


if __name__ == "__main__":
    main()
