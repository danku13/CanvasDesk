#!/usr/bin/env python3
"""Браузерный дым M8/W5 (ввод и редактирование, wasm-port §6.1 трек A).

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



async def dispatch_key(page, key, ctrl=False, shift=False, target="canvas"):
    """Синтетический KeyboardEvent на канвас (эмуляция реальной клавиатуры).

    Playwright keyboard.type() для кириллицы шлёт insertText (без keydown),
    а winit-web слушает только keydown — реальная RU-клавиатура даёт
    keydown с key="ф" (раскладку применяет ОС/браузер). Диспатч
    KeyboardEvent точно воспроизводит этот контракт.
    """
    init = f"{{key: {key!r}, bubbles: true, ctrlKey: {str(ctrl).lower()}, shiftKey: {str(shift).lower()}}}"
    init = init.replace("'", '"')
    await page.evaluate(
        f"""(t) => {{
            const el = document.querySelector(t);
            el.dispatchEvent(new KeyboardEvent('keydown', {init}));
            el.dispatchEvent(new KeyboardEvent('keyup', {init}));
        }}""",
        target,
    )


async def dispatch_text(page, text, target="canvas"):
    """Посимвольный набор текста (кириллица — как с реальной клавиатуры)."""
    for ch in text:
        await dispatch_key(page, ch, target=target)
        await asyncio.sleep(0.04)


async def main() -> int:
    failures = []
    async with async_playwright() as p:
        browser = await p.chromium.launch(args=FLAGS)
        page = await browser.new_page()
        console_msgs: list[str] = []
        page_errors: list[str] = []
        swiftshader_noise = 0

        def on_pageerror(e):
            # Известный артефакт SwiftShader (headless, software WebGPU):
            # glyphon создаёт внутренний буфер mappedAtCreation=true,
            # SwiftShader ограничивает такие аллокации 4 KiB. На реальных
            # GPU (продуктовый таргет — Chromium 113+ с аппаратным WebGPU)
            # ограничения нет; в дым не считаем падением, но считаем.
            nonlocal swiftshader_noise
            if "createBuffer failed" in str(e) and "mappedAtCreation" in str(e):
                swiftshader_noise += 1
                return
            page_errors.append(str(e))

        page.on("console", lambda m: console_msgs.append(f"{m.type}: {m.text}"))
        page.on("pageerror", on_pageerror)

        # --- 1. Базовый запуск + фокус канваса (W5) ---
        await page.goto(f"{BASE}/?log=debug", wait_until="load")  # DEBUG: оракул поисковых логов
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
        await dispatch_text(page, "привет мир")  # кириллица → Key::Character
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
        # Копипаст в редакторе (WebClipboard): copy → paste из кэша.
        # (до поискового прыжка: после полёта камера уведёт заметку с (640,300))
        await page.mouse.dblclick(640, 300)
        await asyncio.sleep(0.5)
        await page.keyboard.press("Control+a")
        await page.keyboard.press("Control+c")  # copy через WebClipboard
        await page.keyboard.press("Control+v")  # paste из кэша
        await page.keyboard.press("Enter")  # коммит (текст удвоился)
        await asyncio.sleep(0.6)
        # Сквозная проверка (W5+W7): текст заметки дошёл до модели —
        # Ctrl+F → запрос «привет» → scan_scene находит ноду → rows>=1
        console_msgs.clear()
        await page.keyboard.press("Control+f")
        await asyncio.sleep(0.4)
        await dispatch_text(page, "привет")
        ok = await wait_console(console_msgs, "поиск завершён", 10)
        rows = 0
        for m in reversed(console_msgs):
            if "поиск завершён" in m and "rows=" in m:
                rows = int(m.split("rows=")[1].split()[0])
                break
        print(("PASS" if ok and rows >= 1 else "FAIL"),
              f"текст заметки в модели: поиск находит (rows={rows})")
        if not (ok and rows >= 1):
            failures.append(f"поиск не нашёл заметку: rows={rows}")
        await page.keyboard.press("Enter")  # прыжок к найденной ноде
        await asyncio.sleep(1.0)
        await page.keyboard.press("Escape")  # закрыть панель
        await asyncio.sleep(0.3)
        print(
            ("PASS" if not page_errors else "FAIL"),
            f"клавиатура/редактор без pageerror ({len(page_errors)})",
        )
        if page_errors:
            failures.append(f"pageerror: {page_errors[:3]}")

        # --- 3. ?stress=5000 + ?log=debug — стресс-сцена, fps, поиск ---
        console_msgs.clear()
        page_errors.clear()
        await page.goto(f"{BASE}/?stress=5000&log=debug", wait_until="load")
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

        # --- 3b. W7: Ctrl+F по 5000-нод сцене, переход к результату ---
        # Тракт: Ctrl+F → панель; ввод → debounce 200мс → Query → MemSearch
        # → SearchEvent::Ready → proxy (лог canvas_web «backend ответил») →
        # apply_search_hits → scan_scene по заголовкам/телам (лог canvas_app
        # «поиск завершён rows=») → Enter → полёт камеры к ноде.
        # Стресс-заголовки: «смета #N …» — запрос «смета» бьёт ~625 нод.
        await page.keyboard.press("Escape")  # онбординг → «Пропустить»
        await asyncio.sleep(0.4)
        await page.keyboard.press("Control+f")
        await asyncio.sleep(0.4)
        await dispatch_text(page, "смета")
        ok = await wait_console(console_msgs, "поисковый backend ответил", 10)
        print(("PASS" if ok else "FAIL"), "Query → MemSearch → SearchEvent (proxy)")
        if not ok:
            failures.append("нет ответа поискового backend'а (READY-лог)")
        ok = await wait_console(console_msgs, "поиск завершён", 10)
        rows = 0
        for m in reversed(console_msgs):
            if "поиск завершён" in m and "rows=" in m:
                rows = int(m.split("rows=")[1].split()[0])
                break
        print(("PASS" if ok and rows > 0 else "FAIL"),
              f"scan_scene по заголовкам/телам (rows={rows})")
        if not (ok and rows > 0):
            failures.append(f"поиск не дал строк: rows={rows}")
        await page.keyboard.press("Enter")  # прыжок к результату (полёт)
        await asyncio.sleep(1.2)
        print(
            ("PASS" if not page_errors else "FAIL"),
            f"прыжок к результату без pageerror ({len(page_errors)})",
        )
        if page_errors:
            failures.append(f"поиск pageerror: {page_errors[:3]}")
        await page.keyboard.press("Escape")  # закрыть панель

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

    if swiftshader_noise:
        print(f"INFO SwiftShader mappedAtCreation-артефакт (не падение): {swiftshader_noise}")
    print("\n=== ИТОГ ===")
    if failures:
        for f in failures:
            print("FAIL:", f)
        return 1
    print("SMOKE OK")
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
