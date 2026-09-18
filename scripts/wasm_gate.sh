#!/usr/bin/env bash
# FR-036 (ADR-0011): wasm-гейт — автономная проверка wasm-сборки и исполнения
# расчётного ядра в wasm-рантайме. Цель — агент/разработчик без GUI и Windows
# может проверить ядро целиком: компиляция под продуктовый таргет веб-порта
# (wasm32-unknown-unknown, план M8 §3.1) + исполнение тестов в wasmtime
# (wasm32-wasip1, .cargo/config.toml — runner).
#
#   1/3 check: core/render/widgets компилируются под wasm32-unknown-unknown
#   2/3 build: артефакт — rlib ядра под wasm32-unknown-unknown
#   3/3 test:  тесты canvas-core исполняются под wasm32-wasip1 (wasmtime)
#
# Использование:
#   scripts/wasm_gate.sh           # все три ступени
#   scripts/wasm_gate.sh --check   # только ступень 1 (wasmtime не нужен)
set -euo pipefail
cd "$(dirname "$0")/.."

CRATES="-p canvas-core -p canvas-render -p canvas-widgets"

echo "[wasm-gate 1/3] cargo check --target wasm32-unknown-unknown $CRATES"
cargo check --target wasm32-unknown-unknown $CRATES

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

echo "[wasm-gate 3/3] cargo test --target wasm32-wasip1 -p canvas-core (runner: wasmtime)"
if ! command -v wasmtime >/dev/null 2>&1; then
    echo "  wasmtime не найден: curl https://wasmtime.dev/install.sh -sSf | bash" >&2
    echo "  (или ~/.local/bin в PATH; ступени 1–2 зелёные)" >&2
    exit 2
fi
# Харнесс однопоточный: std::thread на wasip1 не поддержан.
RUST_TEST_THREADS=1 cargo test --target wasm32-wasip1 -p canvas-core

echo "[wasm-gate] OK: ядро собирается под wasm и исполняется в wasm-рантайме"
