#!/usr/bin/env python3
"""Дебаг: какой конфиг реально применяет web-приложение (3 варианта)."""
import asyncio
import functools
import http.server
import json
import socketserver
import threading
from pathlib import Path

from playwright.async_api import async_playwright

DIST = Path("/home/z/my-project/canvasdesk/target/dist")
PORT = 8943
W, H = 1440, 900


class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *a):
        pass


Handler = functools.partial(Quiet, directory=str(DIST))


class Srv(socketserver.TCPServer):
    allow_reuse_address = True


VARIANTS = {
    "base": 'language = "ru"\nonboarding_done = true\n',
    "llm_min": (
        'language = "ru"\nonboarding_done = true\n\n[llm]\n'
        'provider_suggest = "Byok"\n'
    ),
    "llm_full": (
        'language = "ru"\nonboarding_done = true\n\n[llm]\n'
        'provider_suggest = "Byok"\n'
        'provider_graph = "Byok"\n'
        'provider_agent = "Byok"\n'
        'api_key = "sk-probe"\n'
    ),
}


async def run(pw, name, cfg):
    browser = await pw.chromium.launch(
        args=["--use-gl=angle", "--use-angle=swiftshader",
              "--enable-unsafe-swiftshader", "--enable-webgl",
              "--ignore-gpu-blocklist"]
    )
    page = await browser.new_page(viewport={"width": W, "height": H})
    logs: list[str] = []
    page.on("console", lambda m: logs.append(f"{m.type}: {m.text[:160]}"))
    await page.add_init_script(
        f"window.localStorage.setItem('canvasdesk.config', {json.dumps(cfg)!r});"
        "window.localStorage.setItem('canvasdesk.telemetry.consent', '{\"analytics\":false,\"errors\":false}');"
    )
    await page.goto(f"http://127.0.0.1:{PORT}/", wait_until="load")
    await page.wait_for_timeout(9000)
    stored = await page.evaluate("() => localStorage.getItem('canvasdesk.config')")
    same = stored == cfg
    print(f"--- {name}: config_kept={same}")
    if not same and stored:
        print("  rewritten ->", stored[:300].replace("\n", "\\n"))
    interesting = [l for l in logs if "config" in l.lower() or "toml" in l.lower()
                   or "error" in l.lower() or "warn" in l.lower()]
    for l in interesting[:6]:
        print("  log:", l)
    await page.screenshot(path=f"/home/z/my-project/shots/dbg_{name}.png")
    await browser.close()


async def main():
    srv = Srv(("127.0.0.1", PORT), Handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    async with async_playwright() as pw:
        for name, cfg in VARIANTS.items():
            await run(pw, name, cfg)


if __name__ == "__main__":
    asyncio.run(main())
