#!/usr/bin/env bash
# FR-037 (ADR-0012): MCP-wasm-гейт — автономная верификация MCP-слоя в
# wasm-рантайме одной командой, без Windows и GUI. Дополняет гейт ядра
# scripts/wasm_gate.sh (FR-036/ADR-0011): тот отвечает за ядро, этот — за
# контрактный слой (ADR-0004).
#
#   1/3 check:  canvas-scene / canvas-mcp / canvas-mcp-headless компилируются
#               под wasm32-unknown-unknown (MCP-слой в продуктовом веб-порте)
#   2/3 test:   тесты трёх крейтов исполняются под wasm32-wasip1 в wasmtime
#               (scene 53 + мост 13 + headless 12 — включая oracle-гейты
#               эталонов CP1/CP3/CP5 и протокольный цикл)
#   3/3 e2e:    реальная MCP-сессия с headless-сервером в wasmtime
#               (драйвер scripts/mcp_wasm_e2e.py: initialize → tools/list →
#               graph_apply мини-эталон №1 oracle ±1 % → analyze_bottlenecks
#               ρ-гейт CP5 → негативные ветки; лог — target/tmp/)
#
# Использование:
#   scripts/mcp_wasm_gate.sh           # все три ступени
#   scripts/mcp_wasm_gate.sh --check   # только ступень 1 (wasmtime не нужен)
set -euo pipefail
cd "$(dirname "$0")/.."

CRATES="-p canvas-scene -p canvas-mcp -p canvas-mcp-headless"

echo "[mcp-wasm-gate 1/3] cargo check --target wasm32-unknown-unknown $CRATES"
cargo check --target wasm32-unknown-unknown $CRATES

if [ "${1:-}" = "--check" ]; then
    echo "[mcp-wasm-gate] OK (--check): MCP-слой компилируется под wasm"
    exit 0
fi

if ! command -v wasmtime >/dev/null 2>&1; then
    echo "  wasmtime не найден: curl https://wasmtime.dev/install.sh -sSf | bash" >&2
    echo "  (или ~/.local/bin в PATH; ступень 1 зелёная)" >&2
    exit 2
fi

# Харнесс однопоточный: std::thread на wasip1 не поддержан (ADR-0011).
echo "[mcp-wasm-gate 2/3] cargo test --target wasm32-wasip1 $CRATES (runner: wasmtime)"
RUST_TEST_THREADS=1 cargo test --target wasm32-wasip1 $CRATES

echo "[mcp-wasm-gate 3/3] драйвер реальной MCP-сессии (scripts/mcp_wasm_e2e.py)"
python3 scripts/mcp_wasm_e2e.py

echo "[mcp-wasm-gate] OK: MCP-слой (scene/mcp/headless) собирается под wasm, исполняется в wasmtime и проводит полную MCP-сессию"
