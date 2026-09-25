#!/usr/bin/env bash
# FR-036 (ADR-0011): wasm-гейт — автономная проверка wasm-сборки и исполнения
# расчётного ядра в wasm-рантайме. Цель — агент/разработчик без GUI и Windows
# может проверить ядро целиком: компиляция под продуктовый таргет веб-порта
# (wasm32-unknown-unknown, план M8 §3.1) + исполнение тестов в wasmtime
# (wasm32-wasip1, .cargo/config.toml — runner).
# FR-037/MW2: в гейт включён мост canvas-mcp (исполнение его тестов —
# ступень 3, wasmtime, локально — прецедент ADR-0011).
# M8/W12 (wasm-port §6.1 п.4): canvas-web — продуктовый web-слой в
# ступени 1 (компиляция; бандл/деплой — scripts/web_bundle.sh и
# Pages-workflow, там же размер бандла в логе §8.8).
#
#   1/3 check: core/render/widgets/mcp/web компилируются под wasm32-unknown-unknown
#   2/3 build: артефакт — rlib ядра под wasm32-unknown-unknown
#   3/3 test:  тесты canvas-core и моста canvas-mcp исполняются под wasm32-wasip1 (wasmtime)
#
# FR-WASM-02 §7 (panic-guard): отдельно ступень 1.5 — компиляция wasm-тестов
# canvas-render (чистая логика clamp_surface_extent и др.). Без установленного
# wasm-bindgen-test-runner это «только компилируется»; при наличии бинарника
# (cargo install wasm-bindgen-cli) — исполняется в браузере headless.
#
# Использование:
#   scripts/wasm_gate.sh           # все три ступени
#   scripts/wasm_gate.sh --check   # только ступень 1 (wasmtime не нужен)
set -euo pipefail
cd "$(dirname "$0")/.."

CRATES="-p canvas-core -p canvas-render -p canvas-widgets -p canvas-mcp -p canvas-web"

echo "[wasm-gate 1/3] cargo check --target wasm32-unknown-unknown $CRATES"
cargo check --target wasm32-unknown-unknown $CRATES

# FR-WASM-02 §7: проверка компиляции wasm-тестов canvas-render. Без --tests
# cargo не компилирует tests/-директорию; здесь мы убеждаемся, что
# wasm-bindgen_test-макрос раскрывается без ошибок под wasm32-unknown-unknown.
echo "[wasm-gate 1.5/3] cargo check --target wasm32-unknown-unknown --tests -p canvas-render (wasm-тесты компилируются)"
cargo check --target wasm32-unknown-unknown --tests -p canvas-render

# FR-WASM-02 §7: если доступен wasm-bindgen-test-runner — исполняем wasm-тесты
# в браузере (по умолчанию Chrome headless через chromedriver). runner должен
# совпадать по версии с wasm-bindgen в Cargo.lock (0.2.127 в дереве на W4).
# Установка: cargo install wasm-bindgen-cli --version 0.2.127.
if command -v wasm-bindgen-test-runner >/dev/null 2>&1; then
    echo "[wasm-gate 1.6/3] cargo test --target wasm32-unknown-unknown -p canvas-render --test wasm_clamp (wasm-bindgen-test-runner)"
    CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER="wasm-bindgen-test-runner" \
        cargo test --target wasm32-unknown-unknown -p canvas-render --test wasm_clamp
else
    echo "[wasm-gate 1.6/3] wasm-bindgen-test-runner не найден — wasm-тесты скомпилированы, но не исполнены"
    echo "  установить: cargo install wasm-bindgen-cli --version 0.2.127"
    echo "  затем: CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \\"
    echo "         cargo test --target wasm32-unknown-unknown -p canvas-render --test wasm_clamp"
fi

echo "[wasm-gate 2/3] cargo build --target wasm32-unknown-unknown -p canvas-core (артефакт rlib)"
cargo build --target wasm32-unknown-unknown -p canvas-core
RLIB="target/wasm32-unknown-unknown/debug/libcanvas_core.rlib"
if [ -f "$RLIB" ]; then
    echo "  артефакт: $RLIB ($(du -h "$RLIB" | cut -f1))"
fi

if [ "${1:-}" = "--check" ]; then
    echo "[wasm-gate] OK (--check): компиляция под wasm зелёная"
    exit 0
fi

# Список ступени 3 явный (не $CRATES): render/widgets под wasip1 не
# тестируются — их wgpu-тесты требуют GPU-адаптер, которого в wasmtime нет.
echo "[wasm-gate 3/3] cargo test --target wasm32-wasip1 -p canvas-core -p canvas-mcp (runner: wasmtime)"
if ! command -v wasmtime >/dev/null 2>&1; then
    echo "  wasmtime не найден: curl https://wasmtime.dev/install.sh -sSf | bash" >&2
    echo "  (или ~/.local/bin в PATH; ступени 1–2 зелёные)" >&2
    exit 2
fi
# Харнесс однопоточный: std::thread на wasip1 не поддержан.
RUST_TEST_THREADS=1 cargo test --target wasm32-wasip1 -p canvas-core -p canvas-mcp

echo "[wasm-gate] OK: ядро и мост canvas-mcp собираются под wasm и исполняются в wasm-рантайме"
