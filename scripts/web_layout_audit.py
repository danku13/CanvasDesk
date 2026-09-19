#!/usr/bin/env python3
"""Браузерный аудит вёрстки web-оболочки CanvasDesk (CR-014).

Проверяет на серии вьюпортов (телефон/планшет/десктоп), что DOM-панель
хранилища (#w6-toolbar) и канвас НЕ покидают экран (главный инвариант
CR-014 «never-off-screen») и не наезжают на зоны GPU-панелей:

  1. Горизонтальное переполнение документа (scrollWidth > innerWidth) — FAIL.
  2. Канвас занимает окно целиком (100vw/100dvh) — FAIL при расхождении.
  3. Тулбар целиком в экране (x ≥ 0, right ≤ vw, bottom ≤ vh) — и на старте,
     и с длинной подписью «Недавние: <имя файла>» — FAIL при выезде.
  4. Зона тулбара не пересекает зону миникарты (право-низ, 220×140 + 16) — FAIL.
  5. Зона тулбара не пересекает зону панели поиска (топ-центр, 460px):
     vw < 1200 — FAIL (тулбар обязан быть в левом нижнем углу); vw ≥ 1200 —
     WARN (допустимая косметика: перекрыт только правый край поля ввода,
     ряды результатов ниже, ввод клавиатурой работает; условие — длинное
     имя канваса + открытым поиск; CR-014 «Известные ограничения»).
  6. Ошибки страницы (pageerror) — FAIL (вёрстка не маскирует падения).

Запуск (после `trunk build` в crates/canvas-web и веб-сервера на dist):
    python3 -m http.server 8090 -d target/dist &
    python3 scripts/web_layout_audit.py          # http://localhost:8090
    WEB_SMOKE_URL=https://… python3 scripts/web_layout_audit.py

WebGPU-канвас в headless не снимается скриншотом (ограничение из W4) —
оракул DOM: getBoundingClientRect + scrollWidth (как web_smoke.py).
"""
import asyncio
import os
import sys

from playwright.async_api import async_playwright

BASE = os.environ.get("WEB_SMOKE_URL", "http://localhost:8090")
FLAGS = [
    "--enable-unsafe-webgpu",
    "--use-angle=swiftshader",
    "--enable-features=Vulkan",
]
VIEWPORTS = [
    (375, 667, "iPhone SE"),
    (390, 844, "iPhone 14"),
    (768, 1024, "iPad"),
    (1024, 768, "iPad landscape"),
    (1200, 800, "breakpoint+1"),
    (1280, 800, "laptop"),
    (1920, 1080, "desktop"),
]
# Панель поиска: PANEL_WIDTH=460, PANEL_SIDE_MARGIN=12, PANEL_TOP_MARGIN=12,
# высота инпута 36 + паддинги 8 (search_ui.rs)
SEARCH_PANEL = {"width": 460.0, "side": 12.0, "top": 12.0, "input_h": 36.0, "pad": 8.0}
# Миникарта: MINIMAP_W=220, MINIMAP_H=140, MINIMAP_MARGIN=16 (minimap_pass.rs)
MINIMAP = {"w": 220.0, "h": 140.0, "margin": 16.0}
# Точка смены раскладки тулбара (index.html: топ-право ↔ лево-низ)
TOOLBAR_BREAKPOINT = 1200.0

MEASURE_JS = """
() => {
  const vw = window.innerWidth, vh = window.innerHeight;
  const doc = document.documentElement;
  const out = {
    vw, vh,
    scrollW: Math.max(doc.scrollWidth, document.body.scrollWidth),
    scrollH: Math.max(doc.scrollHeight, document.body.scrollHeight),
    canvas: null, toolbar: null, buttons: {},
  };
  const canvas = document.querySelector('body > canvas');
  if (canvas) {
    const r = canvas.getBoundingClientRect();
    out.canvas = {x: r.x, y: r.y, w: r.width, h: r.height};
  }
  const tb = document.getElementById('w6-toolbar');
  if (tb) {
    const r = tb.getBoundingClientRect();
    out.toolbar = {x: r.x, y: r.y, w: r.width, h: r.height, right: r.right, bottom: r.bottom};
    for (const id of ['btn-open', 'btn-recent', 'btn-export']) {
      const b = document.getElementById(id);
      if (b) {
        const br = b.getBoundingClientRect();
        out.buttons[id] = {x: br.x, y: br.y, w: br.width, h: br.height};
      }
    }
  }
  return out;
}
"""

# Длинное имя файла: воспроизводит подпись set_recent_label (toolbar.rs)
LONG_NAME = "Недавние: godovoj-finansovyj-model-Q4-verificacija-final.canvas"


async def measure(page):
    return await page.evaluate(MEASURE_JS)


def panel_rect(vw):
    w = min(SEARCH_PANEL["width"], vw - 2 * SEARCH_PANEL["side"])
    if w <= 0:
        return None
    x0 = (vw - w) / 2.0
    top = SEARCH_PANEL["top"]
    bottom = top + SEARCH_PANEL["pad"] + SEARCH_PANEL["input_h"] + SEARCH_PANEL["pad"]
    return (x0, top, x0 + w, bottom)


def minimap_rect(vw, vh):
    """Зона миникарты (право-низ, логические px); None — окно меньше 252×172."""
    if vw < MINIMAP["w"] + 2 * MINIMAP["margin"] or vh < MINIMAP["h"] + 2 * MINIMAP["margin"]:
        return None
    x1 = vw - MINIMAP["margin"]
    y1 = vh - MINIMAP["margin"]
    return (x1 - MINIMAP["w"], y1 - MINIMAP["h"], x1, y1)


def intersects(a, b):
    return a and b and not (a[2] <= b[0] or b[2] <= a[0] or a[3] <= b[1] or b[3] <= a[1])


async def main():
    failures = 0
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(args=FLAGS)
        for w, h, label in VIEWPORTS:
            page = await browser.new_page(viewport={"width": w, "height": h})
            errors = []
            page.on("pageerror", lambda e: errors.append(str(e)))
            await page.goto(BASE, wait_until="load")
            await page.wait_for_timeout(2500)  # boot wasm + init GPU
            m1 = await measure(page)

            # Деф.4: длинная подпись «Недавние» (эллипсис + never-off-screen)
            await page.evaluate(
                "(t) => document.getElementById('btn-recent').textContent = t", LONG_NAME
            )
            await page.wait_for_timeout(150)
            m2 = await measure(page)
            await page.close()

            print(f"\n=== {label} ({w}x{h}) ===")
            for tag, m in (("старт", m1), ("длинное имя", m2)):
                ovf = m["scrollW"] - m["vw"]
                tb = m["toolbar"]
                btn_open = m["buttons"].get("btn-open")
                issues = []
                warns = []
                if ovf > 0:
                    issues.append(f"ГПЕРЕПОЛНЕНИЕ doc: +{ovf}px")
                if m["canvas"] and (abs(m["canvas"]["w"] - m["vw"]) > 1 or abs(m["canvas"]["h"] - m["vh"]) > 1):
                    issues.append(
                        f"канвас не в окно: {m['canvas']['w']}x{m['canvas']['h']} при {m['vw']}x{m['vh']}"
                    )
                if tb:
                    if tb["x"] < -0.5:
                        issues.append(f"тулбар за левым краем: x={tb['x']:.0f}")
                    if tb["right"] > m["vw"] + 0.5:
                        issues.append(f"тулбар за правым краем: right={tb['right']:.0f}")
                    if tb["bottom"] > m["vh"] + 0.5:
                        issues.append(f"тулбар за нижним краем: bottom={tb['bottom']:.0f}")
                    tb_rect = (tb["x"], tb["y"], tb["right"], tb["bottom"])
                    pr = panel_rect(m["vw"])
                    if pr and intersects((pr[0], pr[1], pr[2], pr[3]), tb_rect):
                        if m["vw"] < TOOLBAR_BREAKPOINT:
                            issues.append(
                                f"тулбар пересекает зону панели поиска "
                                f"({tb_rect[0]:.0f}..{tb_rect[2]:.0f} vs панель {pr[0]:.0f}..{pr[2]:.0f})"
                            )
                        else:
                            warns.append(
                                f"косметика: правый край поля ввода поиска под тулбаром "
                                f"({tb_rect[0]:.0f}..{tb_rect[2]:.0f} vs панель {pr[0]:.0f}..{pr[2]:.0f})"
                            )
                    mr = minimap_rect(m["vw"], m["vh"])
                    if mr and intersects((mr[0], mr[1], mr[2], mr[3]), tb_rect):
                        issues.append(
                            f"тулбар пересекает зону миникарты "
                            f"({tb_rect[0]:.0f}..{tb_rect[2]:.0f} vs миникарта {mr[0]:.0f}..{mr[2]:.0f})"
                        )
                if btn_open and btn_open["x"] < -0.5:
                    issues.append(f"кнопка «Открыть» за левым краем: x={btn_open['x']:.0f}")
                status = "OK" if not issues else "FAIL: " + "; ".join(issues)
                if warns:
                    status += " | WARN: " + "; ".join(warns)
                if issues:
                    failures += 1
                tbw = f"{tb['w']:.0f}x{tb['h']:.0f}@x={tb['x']:.0f}" if tb else "нет"
                print(
                    f"  [{tag}] тулбар {tbw} | doc {m['scrollW']}x{m['scrollH']} "
                    f"при окне {m['vw']}x{m['vh']} -> {status}"
                )
            if errors:
                print(f"  PAGEERROR: {errors[:2]}")
                failures += 1
        await browser.close()
    print(f"\n==== ИТОГ: {'ЧИСТО' if failures == 0 else f'{failures} ЗАМЕЧАНИЙ'} ====")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
