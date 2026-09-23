#!/usr/bin/env python3
"""FR-037 (MW4, ADR-0012): драйвер реальной MCP-сессии с headless-сервером
canvasdesk-mcp-headless в wasmtime (wasm32-wasip1) — без Windows и GUI.

Сценарий (гейты эталонов, те же числа, что lib-тесты canvas-mcp-headless):

  1. initialize          → эхо protocolVersion 2025-06-18, serverInfo canvasdesk
  2. tools/list          → 40 инструментов (X6 FR-048: explain_number)
  3. graph_apply         → мини-эталон №1 (ADR-0005/0006): flow oracle ±1 %
  4. analyze_bottlenecks → CP5 ρ-лестница: 0.417 none → DAU×2 warn 0.833
                           → DAU×5.35 overload 2.229 (бейджи канваса)
  4a. schemes_list/apply → PRD-0008 Q5 v2: галерея агенту, оракулы 5000/0.625
  4b. lineage            → FR-048 X2: дерево происхождения (calc/via)
  4c. flow_recalc        → MCP-parity: активный what-if (416.67) + авто-строки
  5. негативные ветки    → isError неизвестного инструмента, −32601
                           неизвестного метода, batch из 2, схемы/lineage
  6. notification        → тишина (следующий ответ — уже на ping)

Лог сессии сохраняется для разбора падений: target/tmp/mcp_wasm_session.log
(каждый запрос/ответ с временными метками).

Использование:
  python3 scripts/mcp_wasm_e2e.py                # build wasip1 + wasmtime + сценарий
  python3 scripts/mcp_wasm_e2e.py --skip-build   # бинарник уже собран
  python3 scripts/mcp_wasm_e2e.py --wasm <путь>  # нестандартный путь модуля
  python3 scripts/mcp_wasm_e2e.py --runner "…"   # например, нативный запуск:
                                                 # --runner "" --wasm ./target/debug/canvasdesk-mcp-headless
Выход: 0 — сессия сошлась; 1 — ассерт; 2 — сборка/запуск.
"""

from __future__ import annotations

import json
import os
import select
import subprocess
import sys
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WASM_DEFAULT = os.path.join(
    REPO, "target", "wasm32-wasip1", "debug", "canvasdesk-mcp-headless.wasm"
)
LOG_PATH = os.path.join(REPO, "target", "tmp", "mcp_wasm_session.log")
READ_TIMEOUT = 60.0  # с — на один ответ (wasmtime отвечает мгновенно)

# --- Мини-эталон №1 (ADR-0005/0006) — тот же батч, что в lib-тестах ---
MINI_OPS = [
    {
        "op": "node_create_note", "ref": "traffic", "x": 0, "y": 0, "width": 280,
        "text": "dau = 200000\nsess = 3\nreq = 10 req\npeak = 3\n"
                "avg_rps = dau × sess × req / 86400 sec\npeak_rps = avg_rps × peak",
    },
    {
        "op": "template_instantiate", "ref": "cdn", "template": "com.canvasdesk.cdn",
        "x": 360, "y": 0, "params": {"cache_hit": 0.9, "origin_latency": 20},
    },
    {
        "op": "template_instantiate", "ref": "gw", "template": "com.canvasdesk.api-gateway",
        "x": 720, "y": 0, "params": {"latency_budget": 5, "auth_overhead": 2},
    },
    {"op": "param_set", "ref": "traffic", "param": "dau", "value": 200000},
    {"op": "edge_create", "fromRef": "traffic", "toRef": "cdn", "kind": "value", "toParam": "rps"},
    {"op": "edge_create", "fromRef": "cdn", "toRef": "gw", "kind": "value",
     "fromOutput": "origin_rps", "toParam": "rps"},
    {"op": "node_move", "ref": "cdn", "x": 400, "y": 40},
    {"op": "node_move", "ref": "gw", "x": 760, "y": 40},
    {
        "op": "node_create_note", "ref": "cost", "x": 0, "y": 320,
        "text": "cdn_cost = 50 $\n gw_cost = 36 $\n total = cdn_cost + gw_cost",
    },
    {"op": "edge_create", "fromRef": "cost", "toRef": "cdn"},
]

TRAFFIC_TEXT = (
    "dau = 200000\nsess = 3\nreq = 10 req\npeak = 3\n"
    "avg_rps = dau × sess × req / 86400 sec\npeak_rps = avg_rps × peak"
)

# Oracle-числа эталона №1 (ADR-0006): ±1 %
ORACLE = {
    "peak_rps": 208.3333,
    "cdn_w": 0.0342857,
    "origin_rps": 20.8333,
    "gw_w": 0.0032,
    "cost": 86.0,
    "rho_base": 0.4167,
    "rho_x2": 0.8333,
    "rho_x535": 2.2292,
}

LOG = None


def log(message: str) -> None:
    stamp = time.strftime("%H:%M:%S")
    line = f"[{stamp}] {message}"
    print(line)
    if LOG is not None:
        LOG.write(line + "\n")
        LOG.flush()


def close_1pct(actual: float, expected: float) -> bool:
    return abs(actual - expected) <= max(abs(expected) * 0.01, 1e-9)


def assert_close(actual: float, expected: float, what: str) -> None:
    if not close_1pct(actual, expected):
        raise AssertionError(f"{what}: {actual} != oracle {expected} (±1 %)")
    log(f"    {what}: {actual:.4g} == oracle ±1 %")


def build(wasm_path: str) -> None:
    log(f"[build] cargo build --target wasm32-wasip1 -p canvas-mcp-headless")
    result = subprocess.run(
        [
            "cargo", "build", "--target", "wasm32-wasip1",
            "-p", "canvas-mcp-headless", "--bin", "canvasdesk-mcp-headless",
        ],
        cwd=REPO,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(f"сборка wasip1 провалилась (код {result.returncode})")
    if not os.path.isfile(wasm_path):
        raise RuntimeError(f"модуль не найден: {wasm_path}")


class Session:
    """JSON-RPC-сессия поверх stdio-процесса headless-сервера."""

    def __init__(self, command: list[str]) -> None:
        log(f"[spawn] {' '.join(command)}")
        self.proc = subprocess.Popen(
            command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            cwd=REPO,
        )
        self.next_id = 1

    def request(self, method: str, params: dict | None = None, log_tag: str = "") -> dict:
        """Отправить запрос, дождаться и распарсить ответ."""
        request_id = self.next_id
        self.next_id += 1
        envelope = {"jsonrpc": "2.0", "id": request_id, "method": method}
        if params is not None:
            envelope["params"] = params
        line = json.dumps(envelope, ensure_ascii=False)
        log(f"→ {log_tag or method}: {line[:160]}{'…' if len(line) > 160 else ''}")
        assert self.proc.stdin is not None
        self.proc.stdin.write(line + "\n")
        self.proc.stdin.flush()
        reply_line = self._readline()
        log(f"← {log_tag or method}: {reply_line[:160]}{'…' if len(reply_line) > 160 else ''}")
        reply = json.loads(reply_line)
        if reply.get("id") != request_id:
            raise AssertionError(
                f"id не вернулся: {reply.get('id')} != {request_id}: {reply_line[:300]}"
            )
        return reply

    def raw(self, line: str, log_tag: str) -> dict:
        """Отправить сырую строку (негативные ветки, batch)."""
        log(f"→ {log_tag}: {line[:160]}")
        assert self.proc.stdin is not None
        self.proc.stdin.write(line + "\n")
        self.proc.stdin.flush()
        reply_line = self._readline()
        log(f"← {log_tag}: {reply_line[:160]}")
        return json.loads(reply_line)

    def _readline(self) -> str:
        assert self.proc.stdout is not None
        ready, _, _ = select.select([self.proc.stdout], [], [], READ_TIMEOUT)
        if not ready:
            raise TimeoutError(f"нет ответа за {READ_TIMEOUT} с")
        line = self.proc.stdout.readline()
        if not line:
            stderr = ""
            if self.proc.poll() is not None and self.proc.stderr is not None:
                stderr = self.proc.stderr.read()[:500]
            raise AssertionError(f"сервер закрыл stdout (stderr: {stderr})")
        return line.strip()

    def close(self) -> int:
        """EOF stdin — штатный выход сервера; вернуть exit-код."""
        assert self.proc.stdin is not None
        self.proc.stdin.close()
        code = self.proc.wait(timeout=READ_TIMEOUT)
        stderr_tail = ""
        if self.proc.stderr is not None:
            try:
                # неблокирующе дочитать хвост диагностики (там не протокол)
                ready, _, _ = select.select([self.proc.stderr], [], [], 1.0)
                if ready:
                    stderr_tail = self.proc.stderr.read(400)
            except Exception:  # noqa: BLE001 — диагностика не критична
                pass
        if stderr_tail.strip():
            log(f"[stderr-хвост] {stderr_tail.strip()[:200]}")
        log(f"[exit] код {code} (EOF stdin — штатный выход)")
        return code


def result_of(reply: dict) -> dict:
    """result envelope; паника на JSON-RPC error."""
    if "error" in reply:
        raise AssertionError(f"JSON-RPC error: {reply['error']}")
    result = reply.get("result", {})
    if result.get("isError") is True:
        raise AssertionError(f"isError: {json.dumps(result)[:300]}")
    return result


def structured(result: dict) -> dict:
    payload = result.get("structuredContent")
    if payload is None:
        raise AssertionError(f"нет structuredContent: {json.dumps(result)[:300]}")
    return payload


def text_payload(result: dict):
    """FR-034: массивные инструменты (nodes_list/schemes_list) не имеют
    structuredContent — JSON приходит в text-контенте."""
    content = result.get("content", [])
    if not content:
        raise AssertionError(f"нет content: {json.dumps(result)[:300]}")
    return json.loads(content[0].get("text", "null"))


def node_report(report: dict, node_id: str) -> dict:
    for entry in report.get("nodes", []):
        if entry.get("id") == node_id:
            return entry
    raise AssertionError(f"нода {node_id} в отчёте: {json.dumps(report)[:300]}")


def ref_id(created: list, ref: str) -> str:
    for entry in created:
        if entry.get("ref") == ref:
            return entry["node_id"]
    raise AssertionError(f"ref {ref} в created")


def run_scenario(session: Session) -> None:
    # --- 1. initialize: эхо версии 2025-06-18 ---
    reply = session.request(
        "initialize",
        {"protocolVersion": "2025-06-18", "capabilities": {},
         "clientInfo": {"name": "mcp_wasm_e2e"}},
        log_tag="initialize",
    )
    init = result_of(reply)
    assert init.get("protocolVersion") == "2025-06-18", init
    assert init.get("serverInfo", {}).get("name") == "canvasdesk", init
    log("    initialize: эхо 2025-06-18, serverInfo canvasdesk")

    # --- 2. tools/list: 40 инструментов (X6 FR-048 добавил explain_number) ---
    reply = session.request("tools/list", log_tag="tools/list")
    tools = result_of(reply).get("tools", [])
    assert len(tools) == 40, f"tools/list: {len(tools)} != 40"
    log(f"    tools/list: {len(tools)} инструментов")

    # --- 3. graph_apply: мини-эталон №1, flow oracle ±1 % ---
    reply = session.request(
        "tools/call",
        {"name": "graph_apply", "arguments": {"operations": MINI_OPS}},
        log_tag="graph_apply",
    )
    payload = structured(result_of(reply))
    assert payload.get("ok") is True, payload
    created = payload.get("created", [])
    assert len(created) == 7, f"created: {len(created)} != 7 (4 ноды + 3 ребра)"
    flow = payload.get("flow", {})
    traffic, cdn, gw, cost = (ref_id(created, r) for r in ("traffic", "cdn", "gw", "cost"))
    assert_close(flow[traffic]["value"], ORACLE["peak_rps"], "peak_rps «Нагрузка»")
    assert_close(flow[cdn]["value"], ORACLE["cdn_w"], "CDN W (ρ 0.417)")
    assert_close(
        flow[cdn]["outputs"]["origin_rps"]["value"], ORACLE["origin_rps"], "CDN.origin_rps"
    )
    assert_close(flow[gw]["value"], ORACLE["gw_w"], "Gateway W (fromOutput)")
    assert_close(flow[cost]["value"], ORACLE["cost"], "смета мини-эталона")

    # --- 4. analyze_bottlenecks: ρ-лестница CP5 ---
    reply = session.request(
        "tools/call", {"name": "analyze_bottlenecks", "arguments": {}},
        log_tag="analyze базовая",
    )
    report = structured(result_of(reply))
    assert report["thresholds"]["warn_util"] == 0.7
    assert report["thresholds"]["critical_util"] == 0.9
    cdn_report = node_report(report, cdn)
    assert cdn_report["severity"] == "none", cdn_report
    assert_close(cdn_report["utilization"], ORACLE["rho_base"], "CDN ρ базовая")
    assert cdn_report["badge"] == "42% · W: 34 ms", cdn_report

    for factor, dau, severity, rho, badge, tag in (
        (2, 400000, "warn", ORACLE["rho_x2"], "83% · W: 120 ms", "×2"),
        (5.35, 1070000, "overload", ORACLE["rho_x535"], "OVERLOAD 223%", "×5.35"),
    ):
        text = TRAFFIC_TEXT.replace("200000", str(dau), 1)
        session.request(
            "tools/call",
            {"name": "node_update_text", "arguments": {"id": traffic, "text": text}},
            log_tag=f"node_update_text {tag}",
        )
        reply = session.request(
            "tools/call", {"name": "analyze_bottlenecks", "arguments": {}},
            log_tag=f"analyze {tag}",
        )
        report = structured(result_of(reply))
        cdn_report = node_report(report, cdn)
        assert cdn_report["severity"] == severity, f"DAU {tag}: {cdn_report}"
        assert_close(cdn_report["utilization"], rho, f"CDN ρ DAU {tag}")
        assert cdn_report["badge"] == badge, f"бейдж DAU {tag}: {cdn_report}"

    # --- 4a. PRD-0008 (Q5 v2): галерея схем агенту (те же пакеты, что Ctrl+T) ---
    reply = session.request(
        "tools/call", {"name": "schemes_list", "arguments": {}},
        log_tag="schemes_list",
    )
    schemes = text_payload(result_of(reply))
    assert isinstance(schemes, list) and len(schemes) == 6, schemes
    scheme_ids = [s.get("id") for s in schemes]
    assert "com.canvasdesk.scheme.intro-calculations" in scheme_ids, scheme_ids
    log(f"    schemes_list: {len(schemes)} пакетов (как в галерее)")

    reply = session.request(
        "tools/call",
        {"name": "schemes_apply",
         "arguments": {"id": "com.canvasdesk.scheme.intro-calculations", "x": 0, "y": 900}},
        log_tag="schemes_apply",
    )
    applied = structured(result_of(reply))
    assert applied.get("applied") == "com.canvasdesk.scheme.intro-calculations", applied
    assert len(applied.get("nodes", [])) == 6, applied
    assert len(applied.get("edges", [])) == 4, applied
    scheme_flow = applied.get("flow", {})
    scheme_values = [e.get("value") for e in scheme_flow.values() if isinstance(e, dict)]
    assert 5000.0 in scheme_values, scheme_values
    assert 0.625 in scheme_values, scheme_values
    log("    schemes_apply: вставка + оракулы PRD-0008 (load 5000 / share 0.625)")

    # --- 4b. FR-048 X2: lineage — дерево происхождения цифры ---
    reply = session.request(
        "tools/call", {"name": "lineage", "arguments": {"node_id": cdn}},
        log_tag="lineage итога CDN",
    )
    tree = structured(result_of(reply))
    assert tree["root"]["node_id"] == cdn, tree["root"]
    assert tree["root"]["line"] is None, tree["root"]
    nodes = tree["nodes"]
    root = nodes[0]
    assert root["node_id"] == cdn and root["kind"] == "calc", root
    children = root.get("children", [])
    assert any(c.get("via", {}).get("edge_id") for c in children), root
    log(f"    lineage: calc-корень, {len(nodes)} узлов, via-рёбра подсветки")

    # --- 4c. MCP-parity: flow_recalc = активный what-if + авто-строки Р-4 ---
    reply = session.request(
        "tools/call",
        {"name": "whatif_set_override",
         "arguments": {"node_id": traffic, "line": 0, "expr": "dau = 400000"}},
        log_tag="whatif подмена DAU ×2",
    )
    assert "scenario" in structured(result_of(reply))
    flow = structured(result_of(session.request(
        "tools/call", {"name": "flow_recalc", "arguments": {}},
        log_tag="flow_recalc активный",
    )))
    assert_close(flow[traffic]["value"], 416.6667, "flow_recalc = активная подмена (не база)")

    # авто-строки FR-050 Р-4: value-ребро без toParam — производная строка
    session.request(
        "tools/call",
        {"name": "edge_create",
         "arguments": {"from": traffic, "to": cost, "kind": "value", "fromOutput": "peak_rps"}},
        log_tag="value-ребро наблюдателю",
    )
    flow = structured(result_of(session.request(
        "tools/call", {"name": "flow_recalc", "arguments": {}},
        log_tag="flow с авто-строками",
    )))
    auto = flow[cost].get("autoRows", [])
    assert len(auto) == 1 and auto[0]["field"] == "peak_rps", auto
    assert_close(auto[0]["value"], 416.6667, "авто-строка — активное значение")

    # сброс подмен — база возвращается (1070000 → peak_rps 1114.58)
    session.request(
        "tools/call", {"name": "whatif_reset", "arguments": {}}, log_tag="whatif_reset",
    )
    flow = structured(result_of(session.request(
        "tools/call", {"name": "flow_recalc", "arguments": {}},
        log_tag="flow_recalc база",
    )))
    assert_close(flow[traffic]["value"], 1114.5833, "база после сброса подмен")

    # --- 5. Негативные ветки ---
    # новые инструменты: неизвестная схема и несуществующая нода lineage
    reply = session.raw(
        json.dumps({
            "jsonrpc": "2.0", "id": session.next_id, "method": "tools/call",
            "params": {"name": "schemes_apply", "arguments": {"id": "com.canvasdesk.scheme.no-such"}},
        }),
        log_tag="schemes_apply неизвестная схема",
    )
    assert reply.get("result", {}).get("isError") is True, reply
    reply = session.raw(
        json.dumps({
            "jsonrpc": "2.0", "id": session.next_id, "method": "tools/call",
            "params": {"name": "lineage", "arguments": {"node_id": "no-such"}},
        }),
        log_tag="lineage неизвестная нода",
    )
    assert reply.get("result", {}).get("isError") is True, reply
    log("    схемы/lineage негативные ветки → isError")

    reply = session.raw(
        json.dumps({
            "jsonrpc": "2.0", "id": session.next_id, "method": "tools/call",
            "params": {"name": "no_such_tool", "arguments": {}},
        }),
        log_tag="неизвестный инструмент",
    )
    result = reply.get("result", {})
    assert result.get("isError") is True, result
    log("    неизвестный инструмент → isError (MCP-идиома)")

    reply = session.raw(
        json.dumps({"jsonrpc": "2.0", "id": session.next_id, "method": "workspace/symbol"}),
        log_tag="неизвестный метод",
    )
    assert reply.get("error", {}).get("code") == -32601, reply
    log("    неизвестный метод → −32601")

    # batch из 2 (спек 2025-03-26): ping + tools/list — два ответа по порядку
    batch = json.dumps([
        {"jsonrpc": "2.0", "id": session.next_id, "method": "ping"},
        {"jsonrpc": "2.0", "id": session.next_id + 1, "method": "tools/list"},
    ])
    session.next_id += 2
    log(f"→ batch: {batch[:120]}")
    assert session.proc.stdin is not None
    session.proc.stdin.write(batch + "\n")
    session.proc.stdin.flush()
    first = json.loads(session._readline())
    second = json.loads(session._readline())
    log(f"← batch[0]: id {first.get('id')}, result {{}}")
    log(f"← batch[1]: id {second.get('id')}, tools {len(second.get('result', {}).get('tools', []))}")
    assert first.get("result") == {}, first
    assert len(second.get("result", {}).get("tools", [])) == 40, second

    # --- 6. notification — тишина: следующий ответ уже на ping ---
    notification = json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"})
    log(f"→ notification (ответа не ждём): {notification[:100]}")
    assert session.proc.stdin is not None
    session.proc.stdin.write(notification + "\n")
    session.proc.stdin.flush()
    reply = session.request("ping", log_tag="ping после notification")
    assert reply.get("result") == {}, reply
    log("    notification промолчал — следующий ответ на ping")


def main() -> int:
    global LOG
    args = sys.argv[1:]
    skip_build = "--skip-build" in args
    wasm_path = WASM_DEFAULT
    if "--wasm" in args:
        wasm_path = os.path.abspath(args[args.index("--wasm") + 1])
    runner = "wasmtime run"
    if "--runner" in args:
        runner = args[args.index("--runner") + 1]

    os.makedirs(os.path.dirname(LOG_PATH), exist_ok=True)
    LOG = open(LOG_PATH, "w", encoding="utf-8")
    log(f"[mcp_wasm_e2e] лог сессии: {LOG_PATH}")

    try:
        if not skip_build and runner.startswith("wasmtime"):
            build(wasm_path)
        elif not os.path.isfile(wasm_path) and runner.startswith("wasmtime"):
            raise RuntimeError(f"модуль не найден ({wasm_path}); запуск без --skip-build")

        command = ([part for part in runner.split() if part] + [wasm_path])
        session = Session(command)
        try:
            run_scenario(session)
            code = session.close()
            if code != 0:
                raise AssertionError(f"сервер завершился с кодом {code} (ожидался 0 по EOF)")
        finally:
            if session.proc.poll() is None:
                session.proc.kill()
    except (AssertionError, RuntimeError, TimeoutError) as error:
        log(f"[FAIL] {error}")
        print(f"mcp_wasm_e2e: FAIL — {error}", file=sys.stderr)
        return 1
    except OSError as error:
        log(f"[FAIL] запуск не удался: {error}")
        print(f"mcp_wasm_e2e: запуск не удался — {error}", file=sys.stderr)
        return 2

    log("[OK] полная MCP-сессия в wasmtime сошлась (oracle ±1 %, ρ-гейт CP5, негативные ветки)")
    print("mcp_wasm_e2e: OK — сессия сошлась, лог: " + LOG_PATH)
    return 0


if __name__ == "__main__":
    sys.exit(main())
