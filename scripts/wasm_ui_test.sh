#!/usr/bin/env bash
# Уровень L2 рецепта docs/WASM-TESTING.md: собрать web-стенд canvas-web и
# прогнать UI-сценарий в Chromium (WebGPU/SwiftShader) под Xvfb.
# Самопроверка UI агентом без Windows/GUI (AGENTS.md «Самопроверка UI на WASM»).
#
# Использование:
#   scripts/wasm_ui_test.sh                  # сборка + сценарий по умолчанию
#   scripts/wasm_ui_test.sh --no-build       # переиспользовать target/dist
#   ITEM="1035,117" BODY="1078,708" LABEL=about scripts/wasm_ui_test.sh
#
# Параметры сценария (SKIP/HELP/ITEM/BODY/BACKDROP/LABEL/PORT) — env,
# см. шапку scripts/wasm_ui_scenario.mjs.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

if [ "${1:-}" != "--no-build" ]; then
    echo "[wasm-ui 1/3] cargo build -p canvas-web --target wasm32-unknown-unknown"
    cargo build -p canvas-web --target wasm32-unknown-unknown

    echo "[wasm-ui 2/3] wasm-bindgen --target web (CLI-версия обязана совпадать с крейтом в Cargo.lock)"
    wasm-bindgen --target web --out-dir target/dist \
        target/wasm32-unknown-unknown/debug/canvas_web.wasm
    cp crates/canvas-web/index.html target/dist/
    # Инъекция init-глю canvas_web — то, что rust-пайплайн trunk делает сам
    # (эквивалентна: без неё страница грузится, но App не стартует).
    python3 - target/dist/index.html <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1])
html = p.read_text(encoding="utf-8")
if "canvas_web.js" not in html:
    p.write_text(html.replace("</body>", """  <script type="module">
  // Ручная сборка (эквивалент rust-пайплайна trunk): init глю canvas_web
  import init from './canvas_web.js';
  init();
</script>
</body>""", 1), encoding="utf-8")
PY
    echo "  стенд готов: target/dist"
fi

# headed под Xvfb ОБЯЗАТЕЛЕН: headless captureScreenshot не композитит
# WebGPU-канвас (docs/WASM-TESTING.md §4, грабля №1).
# Xvfb запускается вручную: xvfb-run в среде ломается (нет xauth — найдено
# первым прогоном рецепта). Дисплей — WASM_UI_DISPLAY или первый свободный
# из 90–99; Xvfb гасится ловушкой по выходу.
SCREEN="${WASM_UI_SCREEN:-1280x800}x24"
DISP="${WASM_UI_DISPLAY:-}"
if [ -z "$DISP" ]; then
    for n in 90 91 92 93 94 95 96 97 98 99; do
        [ -e "/tmp/.X11-unix/X$n" ] || { DISP=":$n"; break; }
    done
fi
[ -n "$DISP" ] || { echo "[wasm-ui] нет свободного X-дисплея 90–99" >&2; exit 1; }

Xvfb "$DISP" -screen 0 "$SCREEN" >/dev/null 2>&1 &
XVFB_PID=$!
trap 'kill "$XVFB_PID" 2>/dev/null || true' EXIT
for _ in $(seq 1 50); do
    [ -e "/tmp/.X11-unix/X${DISP#:}" ] && break
    sleep 0.1
done

echo "[wasm-ui 3/3] сценарий: ${SCENARIO:-scripts/wasm_ui_scenario.mjs} (X$DISP)"
DISPLAY="$DISP" node "${SCENARIO:-scripts/wasm_ui_scenario.mjs}"
