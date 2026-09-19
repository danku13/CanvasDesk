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

        # --- 5. W6: хранение — сеяние OPFS, автосейв, reopen, ?canvas= ---
        # UX-поток плана §4.2: первый запуск сеет /default.canvas; правки
        # уходят автосейвом (debounce 2 с) в OPFS; перезагрузка открывает
        # верхний «недавний» без пикера (OPFS разрешений не требует).
        # Оракулы — INFO-логи canvas_web/canvas_scene (уровень из ?log=).
        # Свежий контекст = чистый OPFS/IndexedDB origin'а (секции 1–4
        # уже сеяли default.canvas в профиле основной страницы).
        context = await browser.new_context()
        page = await context.new_page()
        page.on("console", lambda m: console_msgs.append(f"{m.type}: {m.text}"))
        page.on("pageerror", on_pageerror)
        console_msgs.clear()
        page_errors.clear()
        await page.goto(f"{BASE}/?log=debug", wait_until="load")
        ok = await wait_console(console_msgs, "сеется новый канвас", 25)
        print(("PASS" if ok else "FAIL"), "первый запуск: default.canvas сеется в OPFS")
        if not ok:
            failures.append("нет лога сеяния OPFS (первый запуск)")
        ok = await wait_console(console_msgs, READY, 30)
        if not ok:
            failures.append("рендер не поднялся после сеяния OPFS")
        print(("PASS" if ok else "FAIL"), "рендер инициализирован после сеяния")
        # Правка → коммит → автосейв через debounce 2 с: «канвас сохранён»
        await page.keyboard.press("Escape")  # онбординг → «Пропустить»
        await asyncio.sleep(0.4)
        await page.mouse.dblclick(640, 300)
        await asyncio.sleep(0.8)
        await dispatch_text(page, "w6 автосейв")
        await page.keyboard.press("Enter")
        await asyncio.sleep(0.5)
        ok = await wait_console(console_msgs, "канвас сохранён", 10)
        print(("PASS" if ok else "FAIL"), "автосейв в OPFS (лог «канвас сохранён»)")
        if not ok:
            failures.append("нет лога автосейва после правки (debounce 2 с)")
        # F5 → reopen из недавних без пикера: «стартовый канвас из недавних»
        console_msgs.clear()
        await page.goto(f"{BASE}/?log=debug", wait_until="load")
        ok = await wait_console(console_msgs, "стартовый канвас из недавних", 25)
        print(("PASS" if ok else "FAIL"), "F5: reopen из недавних (без пикера)")
        if not ok:
            failures.append("нет лога reopen из недавних")
        ok = await wait_console(console_msgs, "канвас загружен из OPFS", 10)
        print(("PASS" if ok else "FAIL"), "reopen: содержимое прочитано из OPFS")
        if not ok:
            failures.append("нет лога загрузки из OPFS")
        ok = await wait_console(console_msgs, READY, 30)
        if not ok:
            failures.append("рендер не поднялся после reopen")

        # --- 5b. W9 (часть 1): wheel-меню шаблонов Shift+кликом ---
        # Тракт FR-018: Shift+клик по пустому месту поднимает wheel-меню
        # категорий (оракул canvas_app «wheel-меню шаблонов»). Меню
        # рендерится до палитры: текстовый оверлей палитры на SwiftShader
        # бьёт известный лимит mappedAtCreation (см. 5d) — палитра
        # проверяется в 5d атомарно, до ломкого кадра.
        console_msgs.clear()
        # После reopen онбординг может подняться снова (FR-028, счётчик
        # defer'ов в localStorage) — он глушит КЛАВИАТУРУ канваса
        # (мышь проходит). Esc «Пропустить» до wheel-меню (как в W5/W7).
        await page.keyboard.press("Escape")
        await asyncio.sleep(0.4)
        await page.keyboard.down("Shift")
        await page.mouse.click(250, 470)  # пустое место левее-ниже заметок
        await page.keyboard.up("Shift")
        ok = await wait_console(console_msgs, "wheel-меню шаблонов", 10)
        print(("PASS" if ok else "FAIL"), "Shift+клик: wheel-меню категорий")
        if not ok:
            failures.append("wheel-меню не открылось")
        await page.keyboard.press("Escape")  # закрыть wheel-меню
        await asyncio.sleep(0.3)
        print(
            ("PASS" if not page_errors else "FAIL"),
            f"W9-сценарии без pageerror ({len(page_errors)})",
        )
        if page_errors:
            failures.append(f"W9 pageerror: {page_errors[:3]}")

        # --- 5c. W8: Numi-формулы — calc-строка и бейдж ошибки ---
        # Тракт FR-013: заметка «кв = 5» → коммит → recompute_flow →
        # построчный результат (оракул canvas_scene «пересчёт потока»:
        # values/lines/errors); строка «2 +» — диагностика → error-бейдж.
        # Поток значений по рёбрам — тот же чистый propagate_with_lines
        # (canvas-core/flow), верифицирован wasip1-тестами и MCP e2e
        # оракулом ±1 %; вставленный шаблон (5d) считает $param-лист.
        await page.mouse.dblclick(400, 430)
        await asyncio.sleep(0.8)
        await dispatch_text(page, "кв = 5")
        await page.keyboard.press("Enter")
        await asyncio.sleep(0.5)
        ok = await wait_console(console_msgs, "пересчёт потока", 10)
        values = lines = 0
        for m in reversed(console_msgs):
            if "пересчёт потока" in m and "values=" in m:
                values = int(m.split("values=")[1].split()[0])
                lines = int(m.split("lines=")[1].split()[0])
                break
        print(("PASS" if ok and values >= 1 and lines >= 1 else "FAIL"),
              f"Numi: calc-строка считает (values={values}, lines={lines})")
        if not (ok and values >= 1 and lines >= 1):
            failures.append(f"Numi-пересчёт: values={values}, lines={lines}")
        await page.mouse.dblclick(400, 540)
        await asyncio.sleep(0.8)
        await dispatch_text(page, "2 +")
        await page.keyboard.press("Enter")
        await asyncio.sleep(0.5)
        errors = 0
        for m in reversed(console_msgs):
            if "пересчёт потока" in m and "errors=" in m:
                errors = int(m.split("errors=")[1].split()[0])
                break
        print(("PASS" if errors >= 1 else "FAIL"),
              f"Numi: бейдж ошибки на «2 +» (errors={errors})")
        if errors < 1:
            failures.append(f"нет error-исхода: errors={errors}")
        print(
            ("PASS" if not page_errors else "FAIL"),
            f"W8-сценарии без pageerror ({len(page_errors)})",
        )
        if page_errors:
            failures.append(f"W8 pageerror: {page_errors[:3]}")

        # --- 5d. W9 (часть 2): палитра Ctrl+P + вставка Enter ---
        # Тракт FR-018/024/025: Ctrl+P разворачивает постоянный док и
        # фокусирует поиск (реестр include_dir не пуст), Enter вставляет
        # выбранную строку в центр (instantiate_template_at → модель).
        #
        # Атомарность: первый ЖЕ кадр палитры на SwiftShader бьёт лимит
        # createBuffer/mappedAtCreation (известный артефакт среды, W7;
        # на аппаратном WebGPU его нет) — необработанное исключение
        # разрывает rAF-насос winit и все СЛЕДУЮЩИЕ события умирают.
        # Оба ключевых события (Ctrl+P-аккорд и Enter) диспетчатся
        # СИНТЕТИЧЕСКИ одним evaluate: обработчики ключей winit
        # исполняются в rAF-тике ДО RedrawRequested, так что оба оракула
        # («шаблонная палитра», «шаблон вставлен») успевают выйти.
        # Синтетика обязательна и для Ctrl+P: реальная trusted-клавиша
        # поднимает браузерную печать (акселератор Chromium) — шим
        # index.html (M8/W9) гасит её для продуктовых пользователей.
        console_msgs.clear()
        await page.evaluate("""() => {
            const el = document.querySelector('canvas');
            const ev = (type, init) => el.dispatchEvent(
                new KeyboardEvent(type, Object.assign({bubbles: true}, init)));
            ev('keydown', {key: 'Control', code: 'ControlLeft', ctrlKey: true});
            ev('keydown', {key: 'p', code: 'KeyP', ctrlKey: true});
            ev('keyup', {key: 'p', code: 'KeyP', ctrlKey: true});
            ev('keyup', {key: 'Control', code: 'ControlLeft'});
            ev('keydown', {key: 'Enter', code: 'Enter'});
            ev('keyup', {key: 'Enter', code: 'Enter'});
        }""")
        await asyncio.sleep(1.0)
        ok = await wait_console(console_msgs, "шаблонная палитра", 5)
        tpl_count = 0
        for m in reversed(console_msgs):
            if "шаблонная палитра" in m and "templates=" in m:
                tpl_count = int(m.split("templates=")[1].split()[0])
                break
        print(("PASS" if ok and tpl_count >= 15 else "FAIL"),
              f"Ctrl+P: палитра открыта, реестр не пуст (templates={tpl_count})")
        if not (ok and tpl_count >= 15):
            failures.append(f"палитра шаблонов: ok={ok}, templates={tpl_count}")
        ok = await wait_console(console_msgs, "шаблон вставлен", 5)
        tpl_id = next(
            (m.split("template=")[1].split()[0] for m in reversed(console_msgs)
             if "шаблон вставлен" in m and "template=" in m),
            "?",
        )
        print(("PASS" if ok else "FAIL"), f"Enter: шаблон вставлен (id={tpl_id})")
        if not ok:
            failures.append("нет лога вставки шаблона")

        # ?canvas=имя — именованный старт по ссылке; файла нет → сеется
        # (ещё один свежий контекст: чистый OPFS — имени точно нет)
        context2 = await browser.new_context()
        page = await context2.new_page()
        page.on("console", lambda m: console_msgs.append(f"{m.type}: {m.text}"))
        page.on("pageerror", on_pageerror)
        console_msgs.clear()
        page_errors.clear()
        await page.goto(f"{BASE}/?canvas=w6-имя&log=debug", wait_until="load")
        ok = await wait_console(console_msgs, "стартовый канвас из URL", 25)
        print(("PASS" if ok else "FAIL"), "?canvas= парсится (лог «стартовый канвас из URL»)")
        if not ok:
            failures.append("нет лога ?canvas= старта")
        ok = await wait_console(console_msgs, "сеется новый канвас", 10)
        print(("PASS" if ok else "FAIL"), "?canvas=w6-имя — файла нет, сеется новый")
        if not ok:
            failures.append("нет сеяния именованного канваса")
        ok = await wait_console(console_msgs, READY, 30)
        print(("PASS" if ok else "FAIL"), "рендер живёт на именованном канвасе")
        if not ok:
            failures.append("нет рендера на именованном канвасе")
        print(
            ("PASS" if not page_errors else "FAIL"),
            f"W6-сценарии без pageerror ({len(page_errors)})",
        )
        if page_errors:
            failures.append(f"W6 pageerror: {page_errors[:3]}")
        # --- 6. W11: виджеты снапшотом — реестр в памяти, LOD-placeholder,
        #     widget_state в localStorage (переживает F5) ---
        # Приёмка W11 (план §6): виджет-нода не падает и рендерится
        # плейсхолдером (?stress-widgets → LOD-лог target=Placeholder);
        # реестр инициализируется в памяти («реестр виджетов готов»);
        # widget_state восстанавливается из localStorage после
        # перезагрузки (INFO-оракул «localStorage готов keys=N»).
        # Создание/удаление через контекстное меню — ручная приёмка
        # (меню рисуется в wgpu-канвасе, headless-клики вслепую хрупки).
        context3 = await browser.new_context()
        page = await context3.new_page()
        page.on("console", lambda m: console_msgs.append(f"{m.type}: {m.text}"))
        page.on("pageerror", on_pageerror)
        console_msgs.clear()
        page_errors.clear()
        await page.goto(f"{BASE}/?stress-widgets=3&log=debug", wait_until="load")
        ok = await wait_console(console_msgs, "реестр виджетов готов", 25)
        print(("PASS" if ok else "FAIL"), "W11: реестр виджетов инициализирован (память)")
        if not ok:
            failures.append("нет лога «реестр виджетов готов» на web")
        ok = await wait_console(console_msgs, "нагрузочные виджеты добавлен", 10)
        print(("PASS" if ok else "FAIL"), "W11: ?stress-widgets=3 сеется")
        if not ok:
            failures.append("нет лога сеяния стресс-виджетов")
        ok = await wait_console(console_msgs, "смена LOD-состояния", 15)
        placeholder = any(
            "смена LOD-состояния" in m and "target=Placeholder" in m
            for m in console_msgs
        )
        print(("PASS" if ok and placeholder else "FAIL"),
              "W11: виджет-нода — LOD Placeholder (плейсхолдер, без падения)")
        if not (ok and placeholder):
            failures.append("нет LOD-решения Placeholder для виджет-нод")
        ok = await wait_console(console_msgs, READY, 30)
        print(("PASS" if ok else "FAIL"), "W11: рендер живёт со виджет-нодами")
        if not ok:
            failures.append("рендер не поднялся на сцене с виджет-нодами")
        # widget_state: сеем JSON (формат W11) → перезагрузка → чтение
        await page.evaluate(
            "localStorage.setItem('canvasdesk.widget_state',"
            " JSON.stringify({'widget-1': {'draft': 'привет'},"
            " 'widget-2': {'k': 'v2'}}))"
        )
        console_msgs.clear()
        await page.reload(wait_until="load")
        ok = await wait_console(console_msgs, "widget_state: localStorage готов", 25)
        keys = -1
        for m in reversed(console_msgs):
            if "widget_state: localStorage готов" in m and "keys=" in m:
                keys = int(m.split("keys=")[1].split()[0])
                break
        print(("PASS" if ok and keys == 2 else "FAIL"),
              f"W11: widget_state пережил F5 (восстановлено keys={keys})")
        if not (ok and keys == 2):
            failures.append(f"widget_state не восстановлен из localStorage: keys={keys}")
        ok = await wait_console(console_msgs, READY, 30)
        if not ok:
            failures.append("рендер не поднялся после F5 (W11)")
        print(
            ("PASS" if not page_errors else "FAIL"),
            f"W11-сценарии без pageerror ({len(page_errors)})",
        )
        if page_errors:
            failures.append(f"W11 pageerror: {page_errors[:3]}")
        await context3.close()

        # --- 7. W10: превью картинок — DOM-drop → OPFS files/ → атлас ---
        # Приёмка W10 (план §6): PNG/JPEG file-ноды с превью; заглушка
        # для прочих типов. Синтетический DragEvent с DataTransfer
        # (Chromium конструирует drop с files): 1×1 PNG + txt — PNG
        # декодируется (INFO-оракул «превью готово width=… height=…»),
        # txt — честная заглушка (DEBUG «превью не удалось»); оба файла
        # материализуются в OPFS files/ (INFO «файл сохранён в OPFS»).
        context4 = await browser.new_context()
        page = await context4.new_page()
        page.on("console", lambda m: console_msgs.append(f"{m.type}: {m.text}"))
        page.on("pageerror", on_pageerror)
        console_msgs.clear()
        page_errors.clear()
        await page.goto(f"{BASE}/?log=debug", wait_until="load")
        ok = await wait_console(console_msgs, READY, 25)
        if not ok:
            failures.append("W10: рендер не поднялся до drop")
        print(("PASS" if ok else "FAIL"), "W10: старт до drop")
        await page.keyboard.press("Escape")  # онбординг → «Пропустить»
        await asyncio.sleep(0.4)
        await page.evaluate(
            """() => {
                const b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
                const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
                const png = new File([bytes], "картинка w10.png", { type: "image/png" });
                const txt = new File(["заметка"], "заметка w10.txt", { type: "text/plain" });
                const dt = new DataTransfer();
                dt.items.add(png);
                dt.items.add(txt);
                window.dispatchEvent(new DragEvent("drop", {
                    dataTransfer: dt, clientX: 600, clientY: 320,
                }));
            }"""
        )
        ok = await wait_console(console_msgs, "файлы приняты в OPFS", 10)
        print(("PASS" if ok else "FAIL"), "W10: drop принят (файлы → OPFS files/)")
        if not ok:
            failures.append("нет лога приёма файлов (DragData::Paths)")
        saved = sum("файл сохранён в OPFS" in m for m in console_msgs)
        print(("PASS" if saved == 2 else "FAIL"),
              f"W10: PNG и txt материализованы в OPFS ({saved}/2)")
        if saved != 2:
            failures.append(f"в OPFS сохранено {saved}/2 файлов")
        ok = await wait_console(console_msgs, "превью готово", 15)
        print(("PASS" if ok else "FAIL"), "W10: PNG декодирован (превью в атласе)")
        if not ok:
            failures.append("нет лога «превью готово» для PNG")
        stub = any("превью не удалось" in m for m in console_msgs)
        print(("PASS" if stub else "FAIL"), "W10: txt — честная заглушка (None)")
        if not stub:
            failures.append("нет DEBUG-лога заглушки для txt")
        print(
            ("PASS" if not page_errors else "FAIL"),
            f"W10-сценарии без pageerror ({len(page_errors)})",
        )
        if page_errors:
            failures.append(f"W10 pageerror: {page_errors[:3]}")
        await context4.close()

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
