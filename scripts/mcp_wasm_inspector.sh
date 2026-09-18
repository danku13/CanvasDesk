#!/usr/bin/env bash
# FR-037 (MW5, ADR-0012): инспектор-сессия владельца — живая ручная проверка
# headless MCP-сервера canvasdesk-mcp-headless (wasm32-wasip1, wasmtime)
# официальным инспектором @modelcontextprotocol/inspector — без Windows и
# named pipe. Устраняет зависимость ручной MCP-приёмки от Windows-машины.
#
# Режимы:
#   scripts/mcp_wasm_inspector.sh                # web UI (по умолчанию):
#                браузер открывает http://127.0.0.1:6274 (URL с токеном
#                печатает сам инспектор), сервер уже вписан (wasmtime run);
#                Connect → вкладка Tools → graph_apply
#   scripts/mcp_wasm_inspector.sh --check        # CLI-приёмка: инспектор сам
#                как MCP-клиент: initialize → tools/list (36) → tools/call
#                graph_apply мини-эталон №1 → oracle ±1 % (те же числа,
#                что scripts/mcp_wasm_e2e.py — FR-037.10 в ACCEPTANCE §29)
#   scripts/mcp_wasm_inspector.sh --skip-build   # не пересобирать модуль
#                (по умолчанию cargo build --target wasm32-wasip1)
#
# Требования: node 18+ (npx), wasmtime, python3. Первый запуск npx скачивает
# инспектор из npm registry (нужна сеть; дальше — кэш). Версия запинена для
# воспроизводимости, переопределяется env:
#   INSPECTOR_PACKAGE=@modelcontextprotocol/inspector@2.7.0
#   WASM_PATH=target/wasm32-wasip1/debug/canvasdesk-mcp-headless.wasm
#   CLIENT_PORT=6274   # порт web UI (пробрасывается инспектору)
#
# Выход (--check): 0 — сессия сошлась; 1 — ассерт/сессия; 2 — окружение/сборка.
set -euo pipefail
cd "$(dirname "$0")/.."

INSPECTOR="${INSPECTOR_PACKAGE:-@modelcontextprotocol/inspector@2.7.0}"
WASM="${WASM_PATH:-target/wasm32-wasip1/debug/canvasdesk-mcp-headless.wasm}"
TMP_DIR=target/tmp
TOOLS_JSON="$TMP_DIR/mcp_inspector_tools.json"
GA_JSON="$TMP_DIR/mcp_inspector_graph_apply.json"

CHECK=0
SKIP_BUILD=0
for arg in "$@"; do
    case "$arg" in
        --check) CHECK=1 ;;
        --skip-build) SKIP_BUILD=1 ;;
        *)
            echo "mcp_wasm_inspector: неизвестный флаг: $arg (--check | --skip-build)" >&2
            exit 2
            ;;
    esac
done

fail_env() {
    echo "mcp_wasm_inspector: $1" >&2
    exit 2
}

command -v npx >/dev/null 2>&1 || fail_env "npx не найден (node 18+, nodejs.org)"
command -v python3 >/dev/null 2>&1 || fail_env "python3 не найден"
command -v wasmtime >/dev/null 2>&1 || fail_env "wasmtime не найден: curl https://wasmtime.dev/install.sh -sSf | bash"

# 1/3 сборка модуля (с кэшем — секунды; --skip-build — доверяем существующему)
if [ "$SKIP_BUILD" -ne 1 ]; then
    echo "[inspector] cargo build --target wasm32-wasip1 -p canvas-mcp-headless"
    cargo build --target wasm32-wasip1 -p canvas-mcp-headless --bin canvasdesk-mcp-headless \
        || fail_env "сборка wasip1 провалилась"
fi
if [ ! -f "$WASM" ]; then
    fail_env "модуль не найден: $WASM (запусти без --skip-build)"
fi
# абсолютный путь: инспектор спавнит сервер-процесс, cwd может отличаться
WASM="$(cd "$(dirname "$WASM")" && pwd)/$(basename "$WASM")"
SERVER_CMD=(wasmtime run "$WASM")

# 2/3 web UI — интерактивная сессия владельца (браузер; остановка — Ctrl+C)
if [ "$CHECK" -ne 1 ]; then
    echo "[inspector] web UI: браузер откроет http://127.0.0.1:${CLIENT_PORT:-6274}"
    echo "[inspector] Connect → Tools (36) → graph_apply; operations мини-эталона №1 —"
    echo "[inspector] MINI_OPS из scripts/mcp_wasm_e2e.py; числа oracle — ACCEPTANCE §29."
    exec npx -y "$INSPECTOR" --web "${SERVER_CMD[@]}"
fi

# 3/3 --check: CLI-приёмка — инспектор как реальный MCP-клиент
mkdir -p "$TMP_DIR"
echo "[inspector] CLI-приёмка (клиент: $INSPECTOR, сервер: ${SERVER_CMD[*]})"

echo "[inspector] 1/2: initialize + tools/list"
if ! npx -y "$INSPECTOR" --cli "${SERVER_CMD[@]}" --method tools/list > "$TOOLS_JSON"; then
    fail_env "инспектор (tools/list) завершился ненулевым кодом — см. вывод выше; первый запуск требует доступа к npm registry"
fi
python3 - "$TOOLS_JSON" <<'PY'
import json
import sys

data = json.load(open(sys.argv[1], encoding="utf-8"))
tools = data.get("tools", [])
names = {tool.get("name") for tool in tools}
assert len(tools) == 36, f"tools/list: {len(tools)} != 36"
assert "graph_apply" in names, "graph_apply отсутствует в списке"
print(f"    tools/list: {len(tools)} инструментов, graph_apply в списке")
PY

echo "[inspector] 2/2: tools/call graph_apply (мини-эталон №1, oracle ±1 %)"
GA_ARGS="$(python3 -c '
import json
import sys
sys.path.insert(0, "scripts")
from mcp_wasm_e2e import MINI_OPS
print(json.dumps({"operations": MINI_OPS}, ensure_ascii=False, separators=(",", ":")))
')"
if ! npx -y "$INSPECTOR" --cli "${SERVER_CMD[@]}" --method tools/call \
    --tool-name graph_apply --tool-args-json "$GA_ARGS" > "$GA_JSON"; then
    fail_env "инспектор (graph_apply) завершился ненулевым кодом — см. вывод выше"
fi
python3 - "$GA_JSON" <<'PY'
import json
import sys

sys.path.insert(0, "scripts")
from mcp_wasm_e2e import ORACLE, close_1pct

data = json.load(open(sys.argv[1], encoding="utf-8"))
sc = data.get("structuredContent")
assert sc and sc.get("ok") is True, f"graph_apply: ok != true ({json.dumps(data)[:200]})"
created = sc.get("created", [])
assert len(created) == 7, f"created: {len(created)} != 7 (4 ноды + 3 ребра)"
ref2id = {entry["ref"]: entry["node_id"] for entry in created if entry.get("ref")}
flow = sc.get("flow", {})
checks = [
    ("peak_rps «Нагрузка»", flow[ref2id["traffic"]]["value"], ORACLE["peak_rps"]),
    ("CDN W (ρ 0.417)", flow[ref2id["cdn"]]["value"], ORACLE["cdn_w"]),
    ("CDN.origin_rps", flow[ref2id["cdn"]]["outputs"]["origin_rps"]["value"], ORACLE["origin_rps"]),
    ("Gateway W (fromOutput)", flow[ref2id["gw"]]["value"], ORACLE["gw_w"]),
    ("смета мини-эталона", flow[ref2id["cost"]]["value"], ORACLE["cost"]),
]
for what, actual, expected in checks:
    assert close_1pct(actual, expected), f"{what}: {actual} != oracle {expected} (±1 %)"
    print(f"    {what}: {actual:.6g} == oracle ±1 %")
PY

echo "[inspector] OK: инспектор подключился (initialize → tools/list → tools/call),"
echo "[inspector] graph_apply из инспектора сходится с oracle ±1 % (FR-037 MW5)"
echo "[inspector] артефакты: $TOOLS_JSON, $GA_JSON"
