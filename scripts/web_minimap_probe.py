#!/usr/bin/env python3
"""Проба мини-карты в web-сборке v2: язык и онбординг через localStorage,
скриншоты правого нижнего угла + пиксельный анализ зоны мини-карты."""
import base64
import http.server
import io
import json
import socketserver
import threading
from pathlib import Path

DIST = Path("/home/z/my-project/canvasdesk/target/dist")
OUT = Path("/home/z/my-project/shots")
OUT.mkdir(exist_ok=True)
PORT = 8932

CONFIG_TOML = (
    'language = "ru"\n'
    'onboarding_done = true\n'
)


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **kw):
        super().__init__(*a, directory=str(DIST), **kw)

    def log_message(self, *a):
        pass


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
        errors: list[str] = []
        page.on("pageerror", lambda e: errors.append(f"pageerror: {e}"))
        page.on(
            "console",
            lambda m: errors.append(f"console[{m.type}]: {m.text[:300]}")
            if m.type == "error"
            else None,
        )
        await page.goto(f"http://127.0.0.1:{PORT}/", wait_until="load")
        await page.wait_for_timeout(12000)

        state = await page.evaluate(
            "() => ({picker: !!document.getElementById('lang-picker-overlay'),"
            " canvasW: (document.querySelector('body>canvas')||{}).width,"
            " dpr: window.devicePixelRatio})"
        )
        print("state:", json.dumps(state))

        await page.screenshot(path=str(OUT / "web2_full.png"))
        # Пиксельный срез зоны мини-карты: canvas WebGL — читаем через
        # скриншот страницы (preserveDrawingBuffer включён FR-091 v2).
        png = await page.screenshot(
            clip={"x": 1440 - 300, "y": 900 - 220, "width": 300, "height": 220}
        )
        (OUT / "web2_minimap_zone.png").write_bytes(png)
        b64 = base64.b64encode(png).decode()
        (OUT / "web2_minimap_zone.b64").write_text(b64)

        # Скриншот после клика мимо (снять выделения) + пан
        await page.mouse.click(700, 450)
        await page.wait_for_timeout(1500)
        await page.screenshot(path=str(OUT / "web2_after_click.png"))
        png2 = await page.screenshot(
            clip={"x": 1440 - 300, "y": 900 - 220, "width": 300, "height": 220}
        )
        (OUT / "web2_minimap_zone2.png").write_bytes(png2)

        print("errors:", len(errors))
        for e in errors[:20]:
            print("  ", e)
        await browser.close()
    srv.shutdown()


if __name__ == "__main__":
    import asyncio

    asyncio.run(main())
