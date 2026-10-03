#!/usr/bin/env python3
"""Аудит тач-целей web-shell: каждый интерактивный DOM-контрол ≥44×44 CSS px.

W-e (дефект №14 аудита ui-kit, follow-up CR-014; WCAG 2.5.5 / Apple HIG).
В отличие от `web_layout_audit.py` НЕ требует trunk/WASM-сборки: index.html
открывается как статическая страница (DOM-оболочка и CSS живут без WASM),
для динамических контролов тура подкладывается стаб `window.CanvasDeskTour`,
размеры кнопок тултипа тура снимаются со стенда `standalone-test.html`
(он грузит реальный бандл `sdk/web-onboarding/canvasdesk-tour.js`).

Запуск:
    python3 scripts/web_touch_targets_audit.py

Хит-зона = визуальный прямоугольник, растянутый до computed min-width/min-height
(все правила W-e заданы с box-sizing: border-box) и расширенный на 16px по
осям, если у контрола есть ::after-оверлей `inset: -8px` (author-bar, крестик
«Об авторе»). Порог PASS — обе оси ≥ 44.

Зависимости: playwright (python) + chromium.
"""
from __future__ import annotations

import http.server
import socketserver
import subprocess
import sys
import threading
from pathlib import Path

from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parent.parent          # корень репо
WEB_DIR = ROOT / "crates" / "canvas-web"
STANDALONE = ROOT / "sdk" / "web-onboarding" / "standalone-test.html"
MIN = 44.0

# JS-сниппет: перечислить цели и посчитать эффективную хит-зону каждой.
MEASURE_JS = """
(targets) => {
  const numOr0 = (v) => { const n = parseFloat(v); return isNaN(n) ? 0 : n; };
  return targets.map((sel) => {
  const el = document.querySelector(sel);
  if (!el) return { sel, missing: true };
  const r = el.getBoundingClientRect();
  const cs = getComputedStyle(el);
  const box = cs.boxSizing === "border-box";
  const padV = parseFloat(cs.paddingTop) + parseFloat(cs.paddingBottom);
  const padH = parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight);
  const borV = parseFloat(cs.borderTopWidth) + parseFloat(cs.borderBottomWidth);
  const borH = parseFloat(cs.borderLeftWidth) + parseFloat(cs.borderRightWidth);
  const minW = numOr0(cs.minWidth) === 0 ? 0
    : box ? numOr0(cs.minWidth) : numOr0(cs.minWidth) + padH + borH;
  const minH = numOr0(cs.minHeight) === 0 ? 0
    : box ? numOr0(cs.minHeight) : numOr0(cs.minHeight) + padV + borV;
  const after = getComputedStyle(el, "::after");
  const hasAfter = after.content !== "none" && after.position === "absolute";
  const expX = hasAfter ? -(numOr0(after.left) + numOr0(after.right)) : 0;
  const expY = hasAfter ? -(numOr0(after.top) + numOr0(after.bottom)) : 0;
  return {
    sel,
    missing: false,
    visual: [Math.round(r.width * 10) / 10, Math.round(r.height * 10) / 10],
    hit: [
      Math.round((Math.max(r.width, minW) + expX) * 10) / 10,
      Math.round((Math.max(r.height, minH) + expY) * 10) / 10,
    ],
  };
});
};
"""


def serve() -> tuple[socketserver.TCPServer, str]:
    handler = http.server.SimpleHTTPRequestHandler

    class ReusableServer(socketserver.TCPServer):
        allow_reuse_address = True

    # Порт 0 — эфемерный: параллельные запуски и TIME_WAIT не мешают.
    srv = ReusableServer(("127.0.0.1", 0), lambda *a, **kw: handler(*a, directory=str(WEB_DIR), **kw))
    port = srv.server_address[1]
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, f"http://127.0.0.1:{port}/index.html"


def audit(page, url: str, targets: list[str], setup: str | None = None) -> list[dict]:
    page.goto(url, wait_until="load")
    if setup:
        page.evaluate(setup)
    return page.evaluate(MEASURE_JS, targets)


def main() -> int:
    srv, url = serve()
    rows: list[dict] = []
    try:
        with sync_playwright() as p:
            browser = p.chromium.launch()

            # 1) index.html: чистый контекст — пикер первого запуска показан.
            ctx = browser.new_context()
            page = ctx.new_page()
            rows += audit(page, url, [
                "#w6-toolbar #btn-tour", "#w6-toolbar #btn-open",
                "#w6-toolbar #btn-recent", "#w6-toolbar #btn-export",
                "#w6-toolbar #btn-export-html",
                "#author-bar a:nth-of-type(1)", "#author-bar a:nth-of-type(5)",
                "#lang-picker-overlay button[data-lang='ru']",
                "#lang-picker-overlay #role-grid button",
                "#lang-picker-overlay #role-skip",
                "#lang-picker-overlay .consent label",
            ])

            # Эллипсис #btn-recent жив после min-width: 44 (кап 240px из CR-014).
            long_name = "godovoj-finansovyj-model-Q4-verificacija-final.canvas"
            recent = page.evaluate(
                """(name) => {
                  const b = document.getElementById('btn-recent');
                  b.textContent = 'Недавние: ' + name;
                  const r = b.getBoundingClientRect();
                  return [Math.round(r.width * 10) / 10, Math.round(r.height * 10) / 10];
                }""",
                long_name,
            )
            rows.append({"sel": "#btn-recent (длинное имя, эллипсис)", "visual": recent,
                         "hit": recent, "cap": max(recent[0], 0)})

            # 2) «Об авторе»: overlay показан вручную (WASM нет — шим сам откроет).
            rows += audit(page, url, [
                "#about-overlay .close-btn", "#about-overlay .link-row",
            ], setup="window.__canvasdesk_openAbout();")

            # 3) #tour-menu: стаб тура до DOMContentLoaded — меню будет
            # собрано; клик по «Тур» открывает его (иначе rect = 0 у hidden).
            ctx2 = browser.new_context()
            page2 = ctx2.new_page()
            page2.add_init_script(
                "window.CanvasDeskTour = { Tour: class { onSignal(){} run(){} }, scenarios: {} };"
            )
            rows += audit(
                page2, url, ["#tour-menu button[role='menuitem']"],
                setup="document.getElementById('btn-tour').click();",
            )

            # 4) Кнопки тултипа тура — реальный бандл на стенде sdk; сценарий
            # стартует deeplink-хэшем при загрузке (как в regression-тестах).
            page3 = ctx2.new_page()
            rows += audit(
                page3, "file://" + str(STANDALONE) + "#tour=cd-toolbar-tour",
                [".cd-tour-btn.cd-tour-btn_skip", ".cd-tour-btn.cd-tour-btn_primary"],
            )
            # Back появляется со 2-го шага — шаг вперёд и замер.
            page3.click(".cd-tour-btn.cd-tour-btn_primary")
            page3.wait_for_selector(".cd-tour-btn.cd-tour-btn_back", state="attached")
            rows += page3.evaluate(MEASURE_JS, [".cd-tour-btn.cd-tour-btn_back"])
            browser.close()
    finally:
        srv.shutdown()

    print(f"{'контрол':52} {'визуал WxH':>14} {'хит WxH':>14}  вердикт")
    failed = 0
    for r in rows:
        if r.get("missing"):
            print(f"{r['sel']:52} {'—':>14} {'—':>14}  НЕ НАЙДЕН")
            failed += 1
            continue
        ok = r["hit"][0] >= MIN and r["hit"][1] >= MIN
        # спец-строка эллипсиса: проверяем и кап 240px из CR-014
        if "эллипсис" in r["sel"]:
            ok = ok and r["hit"][0] <= 240.5
        failed += 0 if ok else 1
        extra = " (и ≤240px кап)" if "эллипсис" in r["sel"] else ""
        print(f"{r['sel']:52} {r['visual'][0]:>6}x{r['visual'][1]:<7} "
              f"{r['hit'][0]:>6}x{r['hit'][1]:<7}  {'PASS' if ok else 'FAIL <44'}{extra}")

    print(f"\nИТОГ: {'ЧИСТО — все цели ≥44×44' if failed == 0 else f'{failed} ЦЕЛИ НИЖЕ 44px'}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
