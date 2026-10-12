#!/usr/bin/env python3
"""Проба мини-карты v3: схема с контентом + открытие ctrl+i (agent panel),
проверка пикселей зоны мини-карты до/во время/после."""
import http.server
import io
import json
import socketserver
import threading
from pathlib import Path

from PIL import Image

DIST = Path("/home/z/my-project/canvasdesk/target/dist")
OUT = Path("/home/z/my-project/shots")
PORT = 8935

CONFIG_TOML = 'language = "ru"\nonboarding_done = true\n'


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **kw):
        super().__init__(*a, directory=str(DIST), **kw)

    def log_message(self, *a):
        pass


def zone_stats(png_bytes, label):
    img = Image.open(io.BytesIO(png_bytes)).convert("RGB")
    (OUT / f"v3_{label}.png").write_bytes(png_bytes)
    w, h = img.size
    # Статистика яркости по правой нижней зоне
    px = list(img.getdata())
    avg = tuple(round(sum(c[i] for c in px) / len(px)) for i in range(3))
    print(f"{label}: zone {w}x{h}, avg rgb {avg}")


async def main():
    from playwright.async_api import async_playwright

    srv = socketserver.TCPServer(("127.0.0.1", PORT), Handler)
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
        page = await browser.new_page(viewport={"width": 1440, "height": 900})
        await page.add_init_script(
            f"window.localStorage.setItem('canvasdesk.config', {json.dumps(CONFIG_TOML)!r});"
            "window.localStorage.setItem('canvasdesk.telemetry.consent', '{\"analytics\":false,\"errors\":false}');"
        )
        errors = []
        page.on("pageerror", lambda e: errors.append(str(e)))
        # Схема с контентом через deep-link
        await page.goto(
            f"http://127.0.0.1:{PORT}/?template=com.canvasdesk.scheme.runway",
            wait_until="load",
        )
        await page.wait_for_timeout(12000)

        clip = {"x": 1440 - 320, "y": 900 - 230, "width": 320, "height": 230}
        zone_stats(await page.screenshot(clip=clip), "scheme")

        # Клик в центр зоны мини-карты — камера должна прыгнуть
        await page.mouse.click(1440 - 140, 900 - 100)
        await page.wait_for_timeout(1200)
        zone_stats(await page.screenshot(clip=clip), "after_click")

        # Ctrl+I — agent panel поверх правой стороны
        await page.keyboard.press("Control+i")
        await page.wait_for_timeout(1500)
        await page.screenshot(path=str(OUT / "v3_ctrl_i_full.png"))
        zone_stats(await page.screenshot(clip=clip), "ctrl_i")

        # Ctrl+I повторно — закрыть
        await page.keyboard.press("Control+i")
        await page.wait_for_timeout(800)
        zone_stats(await page.screenshot(clip=clip), "ctrl_i_closed")

        print("errors:", errors[:10])
        await browser.close()
    srv.shutdown()


if __name__ == "__main__":
    import asyncio

    asyncio.run(main())
