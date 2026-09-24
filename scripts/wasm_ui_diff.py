#!/usr/bin/env python3
"""Пиксельный дифф двух PNG-скриншотов — оракул wasm-сценария
(docs/WASM-TESTING.md, уровень L2).

Использование: wasm_ui_diff.py a.png b.png [допуск]

Печатает число изменённых пикселей и завершается кодом 0. Пиксель считается
изменённым, если максимум RGB-дельты превышает допуск (по умолчанию 8 —
гасит шум кодека/композитора). Интерпретация: ~0 — кадры совпали (панель
осталась на месте); сотни тысяч — событие (открытие/закрытие) произошло;
~3–4 тыс. — косметика «залипшего» hover (кадр не перерисовался по
mouse-move), к логике отношения не имеет (прецедент в worklog 2026-09-25).
"""
import sys

import numpy as np
from PIL import Image, ImageChops


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    a = Image.open(sys.argv[1]).convert("RGB")
    b = Image.open(sys.argv[2]).convert("RGB")
    if a.size != b.size:
        print(f"размеры различаются: {a.size} != {b.size}")
        return 2
    tol = int(sys.argv[3]) if len(sys.argv) > 3 else 8
    diff = np.asarray(ImageChops.difference(a, b), dtype=np.int16).max(axis=2)
    print(int((diff > tol).sum()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
