#!/usr/bin/env python3
"""FR-050 Р-2 (этап D): генерация наклонного производного моно-шрифта.

CanvasDesk Mono Oblique — oblique-производная Noto Sans Mono Regular
(SIL OFL 1.1): все глифы наклоняются на 11° (x' = x + tan(11°)·y),
авансы и вертикальные метрики НЕ меняются (моно-колонка и выравнивание
строк сохраняются байт-в-байт с прямым начертанием — раскладка тела
не может разъехаться). Производная.rename: «Noto» — Reserved Font Name
OFL, поэтому семейство называется «CanvasDesk Mono Oblique»; копирайт
и лицензия сохранены (assets/fonts/OFL-CanvasDeskMonoOblique.txt).

Запуск: python3 scripts/gen_oblique_font.py
Выход: assets/fonts/CanvasDeskMonoOblique.ttf (idempotent).
"""

import math
from pathlib import Path

from fontTools.misc.transform import Transform
from fontTools.pens.transformPen import TransformPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "assets" / "fonts" / "NotoSansMono-Regular.ttf"
DST = ROOT / "assets" / "fonts" / "CanvasDeskMonoOblique.ttf"

ANGLE_DEG = 11.0
FAMILY = "CanvasDesk Mono Oblique"
PS_NAME = "CanvasDeskMonoOblique-Regular"

NAME_RECORDS = {
    1: FAMILY,                      # Семейство
    2: "Regular",                   # Подсемейство (стиль Regular: отдельное
                                    # семейство, стиль-матчинг не нужен)
    3: f"{PS_NAME};generated;oblique-of-NotoSansMono-Regular",  # Unique ID
    4: FAMILY,                      # Полное имя
    6: PS_NAME,                     # PostScript-имя
    16: FAMILY,                     # Типографское семейство
    17: "Regular",                  # Типографское подсемейство
}


def main() -> None:
    font = TTFont(SRC)
    assert "glyf" in font, "ожидался TrueType (glyf) контурный шрифт"
    glyph_set = font.getGlyphSet()
    glyf = font["glyf"]
    tan = math.tan(math.radians(ANGLE_DEG))
    # Наклон вправо: верх глифа уезжает по x на tan·y (y растёт вверх).
    skew = Transform(1, 0, tan, 1, 0, 0)

    for name in font.getGlyphOrder():
        glyph = glyph_set[name]
        pen = TTGlyphPen(glyph_set)
        glyph.draw(TransformPen(pen, skew))
        glyf[name] = pen.glyph()
        # hmtx не трогаем: авансы при наклоне не меняются.

    post = font["post"]
    post.italicAngle = -ANGLE_DEG  # право-наклонный курсив — отрицательный угол
    post.formatType = 0x00030000 if post.formatType == 0x00020000 else post.formatType

    name_table = font["name"]
    for name_id, value in NAME_RECORDS.items():
        for record in name_table.names:
            if record.nameID == name_id:
                record.string = value.encode(record.getEncoding()) \
                    if hasattr(record, "getEncoding") else value
        if not any(r.nameID == name_id for r in name_table.names):
            name_table.setName(value, name_id, 3, 1, 0x409)  # Windows/Unicode

    font.save(DST)
    size = DST.stat().st_size
    print(f"OK: {DST.name} {size / 1024:.1f} КБ, угол {ANGLE_DEG}°, "
          f"семейство «{FAMILY}»")


if __name__ == "__main__":
    main()
