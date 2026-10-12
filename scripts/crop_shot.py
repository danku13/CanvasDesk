#!/usr/bin/env python3
"""Кроп проблемных зон скриншота витрины для инвентаризации дефектов."""
from PIL import Image

src = "/home/z/my-project/upload/pasted_image_1791658822501.png"
out = "/home/z/my-project/download"
img = Image.open(src)
print("size:", img.size)
W, H = img.size
sx, sy = W / 989.0, H / 702.0  # масштаб от моих оценок в пикселях файла

def crop(name, x0, y0, x1, y1, scale=2.0):
    box = (int(x0*sx), int(y0*sy), int(x1*sx), int(y1*sy))
    c = img.crop(box)
    c = c.resize((int(c.width*scale), int(c.height*scale)), Image.LANCZOS)
    p = f"{out}/{name}.png"
    c.save(p)
    print("saved", p, c.size)

# 1. Сайдбар целиком (переполнение подписей, дубли, обрезание снизу)
crop("crop_sidebar", 40, 100, 360, 660, 2.2)
# 2. Стык сайдбар/контент в зоне «Список и скролл» (y 190-250)
crop("crop_joint_list", 40, 185, 700, 320, 2.2)
# 3. Таблица (бейдж ИСТОЧНИК, Σ чека, фокус-ряд)
crop("crop_table", 250, 380, 990, 470, 2.2)
# 4. Тулбар (чипы, плотность)
crop("crop_toolbar", 40, 40, 990, 115, 2.2)
