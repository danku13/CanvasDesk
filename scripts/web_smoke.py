#!/usr/bin/env python3
"""Браузерный дым M8/W5 (запуск: trunk serve -p 8000; WEB_SMOKE_URL=<url> scripts/web_smoke.py) (ввод и редактирование, wasm-port §6.1 трек A).

Приёмка W5 в headless-Chromium (Chromium+swiftshader, как в дыму W4):
  1. Базовый запуск: каркас, async-init WebGPU-рендер, фокус канваса
     (W5: document.activeElement — <canvas>), ошибок страницы нет.
  2. Клавиатура: двойной клик — новая заметка + инлайн-редактор; кириллица
     печатается (Key::Character), Ctrl+A/Ctrl+B/Ctrl+Z не роняют модуль,
     Enter коммитит.
  3. ?stress=5000: стресс-сцена сеется, рендер живёт, rAF-fps меряется.
  4. ?stress=abc: битый параметр — warn, обычный запуск (деградация).

Скриншоты WebGPU-канваса в headless не снимаются (известное ограничение,
задокументировано в W4) — оракул: консоль (tracing-слой canvas-web),
DOM (activeElement) и счётчик rAF.
"""
import asyncio
import os
import sys
import time

from playwright.async_api import async_playwright

BASE = os.environ.get("WEB_SMOKE_URL", "http://localhost:8000")
FLAGS = [
    "--enable-unsafe-webgpu",
    "--use-angle=swiftshader",
    "--enable-features=Vulkan",
]
READY = "рендер инициализирован"
STRESS = "нагрузочный режим ?stress"


async def wait_console(console_msgs, needle, timeout_s):
    """Ждать появления подстроки в собранной консоли."""
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        if any(needle in m for m in console_msgs):
            return True
        await asyncio.sleep(0.3)
    return False


async def collect_raf_fps(page, window_ms=2500):
    """Измерить fps по счётчику requestAnimationFrame (страница целиком)."""
    return await page.evaluate(
        """(windowMs) => new Promise((resolve) => {
            let n = 0;
            const t0 = performance.now();
            function tick() {
                n += 1;
                if (performance.now() - t0 < windowMs) requestAnimationFrame(tick);
                else resolve((n * 1000) / (performance.now() - t0));
            }
            requestAnimationFrame(tick);
        })""",
        window_ms,
    )


async def main() -> int:
    failures = []
    async with async_playwright() as p:
        browser = await p.chromium.launch(args=FLAGS)
        page = await browser.new_page()
        console_msgs: list[str] = []
        page_errors: list[str] = []
        page.on("console", lambda m: console_msgs.append(f"{m.type}: {m.text}"))
        page.on("pageerror", lambda e: page_errors.append(str(e)))

        # --- 1. Базовый запуск + фокус канваса (W5) ---
        await page.goto(BASE, wait_until="load")
        ok = await wait_console(console_msgs, READY, 25)
        print(("PASS" if ok else "FAIL"), "рендер инициализирован (async WebGPU)")
        if not ok:
            failures.append("нет лога инициализации рендера")
        active = await page.evaluate("document.activeElement?.tagName ?? 'none'")
        print(("PASS" if active == "CANVAS" else "FAIL"), f"activeElement={active}")
        if active != "CANVAS":
            failures.append(f"фокус не на канвасе: {active}")

        # --- 2. Клавиатура: онбординг → Esc → заметка → кириллица → коммит ---
        # FR-028: на первом запуске модальный онбординг глушит канвас-ввод
        # (в т.ч. клавиатуру) — Esc «Пропустить». Позитивная проверка
        # клавиатурного тракта через DOM-курсор (App::sync_cursor_icon):
        # редактор — cursor=text, Space+ЛКМ — cursor=grabbing. До закрытия
        # онбординга курсор обязан молчать (негативный контроль), после —
        # жить. Если клавиши НЕ доходят до App, курсор не меняется.
        await page.mouse.move(500, 250)
        await asyncio.sleep(0.3)
        cur_before = await page.evaluate("document.querySelector('canvas')?.style.cursor")
        await page.keyboard.down(" ")
        await asyncio.sleep(0.3)
        await page.mouse.down()
        await asyncio.sleep(0.4)
        cur_blocked = await page.evaluate(
            "document.querySelector('canvas')?.style.cursor"
        )
        await page.mouse.up()
        await page.keyboard.up(" ")
        await asyncio.sleep(0.3)
        print(
            ("PASS" if cur_blocked != "grabbing" else "FAIL"),
            f"онбординг глушит ввод (cursor={cur_blocked!r})",
        )
        if cur_blocked == "grabbing":
            failures.append("онбординг не блокирует ввод — неожиданно")
        await page.keyboard.press("Escape")  # «Пропустить»
        await asyncio.sleep(0.5)
        await page.mouse.dblclick(640, 300)
        await asyncio.sleep(0.8)
        cur_edit = await page.evaluate("document.querySelector('canvas')?.style.cursor")
        await page.keyboard.type("привет", delay=60)
        await page.keyboard.press("Shift+ArrowLeft")  # выделение кириллицы
        await page.keyboard.press("Control+b")  # жирный (Ctrl+B)
        await page.keyboard.press("Control+a")  # выделить всё
        await page.keyboard.press("Control+z")  # undo в редакторе
        await page.keyboard.press("Enter")  # коммит
        await asyncio.sleep(0.6)
        cur_after = await page.evaluate("document.querySelector('canvas')?.style.cursor")
        print(
            ("PASS" if cur_edit == "text" else "FAIL"),
            f"двойной клик → инлайн-редактор (cursor={cur_edit!r}, до: {cur_before!r})",
        )
        if cur_edit != "text":
            failures.append(f"редактор не открылся/курсор не text: {cur_edit}")
        print(
            ("PASS" if cur_after == cur_before else "FAIL"),
            f"Enter коммитит (cursor вернулся: {cur_after!r})",
        )
        if cur_after != cur_before:
            failures.append(f"после Enter курсор {cur_after!r}, ожидался {cur_before!r}")
        # Space+ЛКМ: клавиша → App (space_pressed) + мышь → panning →
        # grabbing-курсор (App::panning = middle || space&&left). Пан —
        # жест с движением: при Space+ЛКМ on_left_button уходит в ранний
        # return (панорамирование), курсор синкается в on_cursor_moved —
        # поэтому после нажатия двигаем мышь, как в реальном жесте.
        await page.keyboard.down("Shift")  # шум: модификатор не должен мешать
        await page.keyboard.up("Shift")
        await page.keyboard.down(" ")
        await asyncio.sleep(0.3)
        await page.mouse.down()
        await asyncio.sleep(0.2)
        await page.mouse.move(660, 320)
        await asyncio.sleep(0.4)
        cur_space = await page.evaluate("document.querySelector('canvas')?.style.cursor")
        await page.mouse.up()
        await page.keyboard.up(" ")
        await asyncio.sleep(0.3)
        print(
            ("PASS" if cur_space == "grabbing" else "FAIL"),
            f"Space доходит до App (cursor={cur_space!r})",
        )
        if cur_space != "grabbing":
            failures.append(f"Space не поднял panning-курсор: {cur_space!r}")
        await page.mouse.dblclick(640, 300)  # возврат в редактор
        await asyncio.sleep(0.5)
        await page.keyboard.press("Escape")
        await asyncio.sleep(0.4)
        # Двойной клик по центру заметки — пере-открыть редактор и копипаст
        await page.mouse.dblclick(640, 300)
        await asyncio.sleep(0.5)
        await page.keyboard.press("Control+a")
        await page.keyboard.press("Control+c")  # copy через WebClipboard
        await page.keyboard.press("Control+v")  # paste из кэша
        await page.keyboard.press("Enter")
        await asyncio.sleep(0.6)
        print(
            ("PASS" if not page_errors else "FAIL"),
            f"клавиатура/редактор без pageerror ({len(page_errors)})",
        )
        if page_errors:
            failures.append(f"pageerror: {page_errors[:3]}")

        # --- 3. ?stress=5000 — стресс-сцена + fps ---
        console_msgs.clear()
        page_errors.clear()
        await page.goto(f"{BASE}/?stress=5000", wait_until="load")
        ok = await wait_console(console_msgs, STRESS, 25)
        print(("PASS" if ok else "FAIL"), "?stress=5000 сеется (лог nodes=5000)")
        if not ok:
            failures.append("нет лога стресс-режима")
        ok = await wait_console(console_msgs, READY, 40)
        print(("PASS" if ok else "FAIL"), "рендер инициализирован на стресс-сцене")
        if not ok:
            failures.append("нет инициализации рендера на стресс-сцене")
        await asyncio.sleep(1.5)  # дать сцене прогреться (атлас шрифтов)
        fps = await collect_raf_fps(page)
        print(f"INFO rAF-fps на ?stress=5000: {fps:.0f}")
        print(
            ("PASS" if not page_errors else "FAIL"),
            f"стресс без pageerror ({len(page_errors)})",
        )
        if page_errors:
            failures.append(f"стресс pageerror: {page_errors[:3]}")

        # --- 4. Битый параметр — деградация, обычный запуск ---
        console_msgs.clear()
        await page.goto(f"{BASE}/?stress=abc", wait_until="load")
        ok = await wait_console(console_msgs, "битый URL-параметр", 25)
        print(("PASS" if ok else "FAIL"), "?stress=abc — warn + деградация")
        if not ok:
            failures.append("нет warn про битый URL-параметр")
        ok = await wait_console(console_msgs, READY, 30)
        print(("PASS" if ok else "FAIL"), "после warn рендер живёт")
        if not ok:
            failures.append("рендер не поднялся после битого параметра")

        await browser.close()

    print("\n=== ИТОГ ===")
    if failures:
        for f in failures:
            print("FAIL:", f)
        return 1
    print("SMOKE OK")
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
