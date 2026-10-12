#!/usr/bin/env python3
"""UR-005 rev2 проба: мини-карта + AI-статус-панель ОДНОВРЕМЕННО в web-сборке.

Сценарий владельца (скриншот 2026-10-09): llm настроен (BYOK) → AI-панель
видима всегда → прежнее правило UR-005 прятало мини-карту навсегда.

Проверка: в правом нижнем углу должны обнаружиться ДВА панельных блока —
AI-панель (302×95 @ 12px от краёв) и НАД ней мини-карта (220×140 @ 16px,
зазор 8px). Пиксельный анализ: длинные тёмные прогоны (бордеры панелей).
"""
import base64
import functools
import http.server
import io
import json
import socketserver
import threading
from pathlib import Path

from PIL import Image

DIST = Path("/home/z/my-project/canvasdesk/target/dist")
OUT = Path("/home/z/my-project/shots")
OUT.mkdir(exist_ok=True)
PORT = 8941
W, H = 1440, 900

CONFIG_TOML = (
    'language = "ru"\n'
    "onboarding_done = true\n"
    'theme = "light"\n'
    "\n[llm]\n"
    'provider_suggest = "Byok"\n'
    'provider_graph = "Byok"\n'
    'provider_agent = "Byok"\n'
    'api_key = "sk-probe-minimap"\n'
)


class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass


Handler = functools.partial(Quiet, directory=str(DIST))


class Srv(socketserver.TCPServer):
    allow_reuse_address = True


def line_rows(img, x0, x1, y0, y1, thresh=200, run_frac=0.85):
    """Строки y, где ≥run_frac пикселей темнее thresh (бордеры панелей).

    Светлая тема (форсируется в конфиге): фон канваса белый (250+),
    бордеры панелей ~150-160 — порог 200 отделяет надёжно.
    """
    px = img.load()
    rows = []
    for y in range(y0, y1):
        dark = sum(1 for x in range(x0, x1) if sum(px[x, y][:3]) / 3 < thresh)
        if dark >= (x1 - x0) * run_frac:
            rows.append(y)
    return rows


def cluster(rows, gap=4):
    """Слить соседние строки в кластеры → [(start, end)]."""
    if not rows:
        return []
    out = [[rows[0], rows[0]]]
    for y in rows[1:]:
        if y - out[-1][1] <= gap:
            out[-1][1] = y
        else:
            out.append([y, y])
    return [tuple(c) for c in out]


async def main():
    from playwright.async_api import async_playwright

    srv = Srv(("127.0.0.1", PORT), Handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()

    async with async_playwright() as pw:
        browser = await pw.chromium.launch(
            args=[
                "--use-gl=angle",
                "--use-angle=swiftshader",
                "--enable-unsafe-swiftshader",
                "--enable-webgl",
                "--ignore-gpu-blocklist",
            ]
        )
        page = await browser.new_page(viewport={"width": W, "height": H})
        # ВАЖНО: json.dumps(cfg) — валидный JS-строковый литерал; оборачивание
        # в !r добавляло ЛИТЕРАЛЬНЫЕ кавычки вокруг TOML → парсинг падал,
        # приложение откатывалось на дефолты (AI off → панель скрыта).
        await page.add_init_script(
            f"window.localStorage.setItem('canvasdesk.config', {json.dumps(CONFIG_TOML)});"
            "window.localStorage.setItem('canvasdesk.telemetry.consent', '{\"analytics\":false,\"errors\":false}');"
        )
        errors: list[str] = []
        page.on("pageerror", lambda e: errors.append(f"pageerror: {e}"))
        page.on(
            "console",
            lambda m: errors.append(f"console[{m.type}]: {m.text[:200]}")
            if m.type == "error"
            else None,
        )
        await page.goto(f"http://127.0.0.1:{PORT}/", wait_until="load")
        await page.wait_for_timeout(14000)

        await page.screenshot(path=str(OUT / "rev2_full.png"))
        clip = {"x": W - 460, "y": H - 380, "width": 460, "height": 380}
        png = await page.screenshot(clip=clip)
        (OUT / "rev2_corner.png").write_bytes(png)

        img = Image.open(io.BytesIO(png)).convert("RGB")
        # Логические координаты = координаты клипа со смещением
        ox, oy = W - 460, H - 380
        # Зона поиска бордеров: x канала правого нижнего угла
        rows = line_rows(img, 1220 - ox, 1410 - ox, 0, 380)
        clusters = cluster(rows)
        print("border-row clusters (y logical):",
              [(a + oy, b + oy) for a, b in clusters])

        # Ожидаемая геометрия (лог. px):
        #   AI-панель: y 641..736 (контент: текст/чипы — ≥3% тёмных)
        #   мини-карта: y 744..884 (сплошной бордер — длинный тёмный прогон)
        px = img.load()

        def dark_frac(y, x0, x1, thresh=170):
            return sum(1 for x in range(x0, x1)
                       if sum(px[x, y][:3]) / 3 < thresh) / (x1 - x0)

        content_rows = [y for y in range(630, 741)
                        if dark_frac(y - oy, 1140 - ox, 1415 - ox) > 0.03]
        panel_present = len(content_rows) >= 5
        minimap_top = any(abs(a + oy - 744) <= 5 for a, b in clusters)
        minimap_bot = any(abs(b + oy - 883) <= 6 for a, b in clusters)
        print(f"panel_content_rows={len(content_rows)} (630..740) "
              f"-> panel_present={panel_present}")
        print(f"minimap_top(744)={minimap_top} minimap_bot(883)={minimap_bot}")
        ok = panel_present and minimap_top and minimap_bot
        print("VERDICT:", "PASS — мини-карта и AI-панель видны одновременно"
              if ok else "FAIL — геометрия не совпала")

        # Снимок зоны в base64 для лога
        b64 = base64.b64encode(png).decode()
        (OUT / "rev2_corner.b64").write_text(b64)
        if errors:
            print("console errors:")
            for e in errors[:5]:
                print(" ", e)
        await browser.close()


if __name__ == "__main__":
    import asyncio

    asyncio.run(main())
