#!/usr/bin/env python3
"""
CanvasDesk — Penpot MCP-клиент (reusable).

Токен НИКОГДА не хранится в коде. Источник — env-переменные:
  PENPOT_MCP_URL    — полный URL с userToken (приоритет)
  PENPOT_MCP_TOKEN  — только токен (URL собирается с дефолтным хостом)
  PENPOT_MCP_HOST   — переопределение хоста (по умолчанию https://design.penpot.app)

Использование как CLI:
  python scripts/penpot_mcp.py init                   # проверить handshake
  python scripts/penpot_mcp.py tools list            # список инструментов
  python scripts/penpot_mcp.py tools call <name> --args '{"k":"v"}'
  python scripts/penpot_mcp.py code 'return penpot.page.name;'
  python scripts/penpot_mcp.py api-info              # документация по API
  python scripts/penpot_mcp.py seed design-system    # залить дизайн-систему CanvasDesk

Использование как Python-библиотека:
  from scripts.penpot_mcp import PenpotMCP
  client = PenpotMCP.from_env()
  client.initialize()
  client.execute_code("return penpot.page.name;")

См. docs/penpot-mcp.md для подробностей.
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time
import urllib.error
import urllib.request
from typing import Any


DEFAULT_HOST = "https://design.penpot.app"
PROTOCOL_VERSION = "2024-11-05"
CLIENT_INFO = {"name": "canvasdesk-mcp", "version": "1.0.0"}

BROWSER_HEADERS = {
    "Content-Type": "application/json",
    "Accept": "application/json, text/event-stream",
    "User-Agent": (
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
    ),
    # Origin/Referer — для прохождения Cloudflare-фейса design.penpot.app
    "Origin": "https://design.penpot.app",
    "Referer": "https://design.penpot.app/",
}


class PenpotMCPError(RuntimeError):
    """Базовая ошибка MCP-клиента."""


class PenpotMCPConfigError(PenpotMCPError):
    """Ошибка конфигурации — нет токена в env."""


def _resolve_endpoint() -> str:
    """Собрать URL MCP-эндпоинта из env-переменных."""
    url = os.environ.get("PENPOT_MCP_URL")
    if url:
        return url
    token = os.environ.get("PENPOT_MCP_TOKEN")
    if not token:
        raise PenpotMCPConfigError(
            "PENPOT_MCP_URL или PENPOT_MCP_TOKEN обязаны быть установлены в env. "
            "См. docs/penpot-mcp.md."
        )
    host = os.environ.get("PENPOT_MCP_HOST", DEFAULT_HOST)
    return f"{host}/mcp/stream?userToken={token}"


def _parse_sse(body: str) -> list[dict[str, Any]]:
    """Разобрать SSE-ответ: 'event: X\\n\\ndata: Y\\n\\n' → [{event, data}]."""
    events: list[dict[str, Any]] = []
    cur_event: str | None = None
    cur_data_lines: list[str] = []

    def flush() -> None:
        nonlocal cur_event, cur_data_lines
        if not cur_data_lines:
            return
        data_str = "\n".join(cur_data_lines)
        try:
            data: Any = json.loads(data_str)
        except json.JSONDecodeError:
            data = data_str
        events.append({"event": cur_event, "data": data})
        cur_event = None
        cur_data_lines = []

    for line in body.split("\n"):
        if line.startswith("event: "):
            cur_event = line[7:].strip()
        elif line.startswith("data: "):
            cur_data_lines.append(line[6:])
        elif line.strip() == "":
            flush()
        else:
            # Прочий текст — игнорируется (комментарии SSE и т.п.)
            pass
    flush()
    return events


class PenpotMCP:
    """Минимальный MCP-клиент для Penpot remote server.

    Сессия умирает вместе с объектом — каждый запуск скрипта делает
    новый initialize. Если нужно переиспользовать session_id между
    вызовами, держите один PenpotMCP на всю сессию.
    """

    def __init__(self, endpoint_url: str, verbose: bool = False):
        self._url = endpoint_url
        self._verbose = verbose
        self._session_id: str | None = None
        self._request_id = 1
        self._initialized = False

    @classmethod
    def from_env(cls, verbose: bool = False) -> "PenpotMCP":
        return cls(_resolve_endpoint(), verbose=verbose)

    def _post(self, payload: dict, timeout: int = 60) -> dict[str, Any]:
        headers = dict(BROWSER_HEADERS)
        if self._session_id:
            headers["Mcp-Session-Id"] = self._session_id
        data = json.dumps(payload).encode("utf-8")
        req = urllib.request.Request(
            self._url, data=data, headers=headers, method="POST"
        )
        try:
            with urllib.request.urlopen(req, timeout=timeout) as resp:
                body = resp.read().decode("utf-8", errors="replace")
                return {
                    "status": resp.status,
                    "content_type": resp.headers.get("Content-Type", ""),
                    "session_id": resp.headers.get("Mcp-Session-Id"),
                    "events": _parse_sse(body),
                    "raw_body": body,
                }
        except urllib.error.HTTPError as e:
            body = e.read().decode("utf-8", errors="replace")
            return {
                "status": e.code,
                "content_type": e.headers.get("Content-Type", ""),
                "session_id": e.headers.get("Mcp-Session-Id"),
                "events": _parse_sse(body),
                "raw_body": body,
            }

    def initialize(self) -> dict[str, Any]:
        """Шаг 1: initialize. Сохраняет session_id для последующих вызовов."""
        if self._initialized:
            return {"session_id": self._session_id, "initialized": True}
        payload = {
            "jsonrpc": "2.0",
            "id": self._request_id,
            "method": "initialize",
            "params": {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {"roots": {"listChanged": False}},
                "clientInfo": CLIENT_INFO,
            },
        }
        self._request_id += 1
        result = self._post(payload, timeout=30)
        if result.get("status") != 200:
            raise PenpotMCPError(
                f"initialize failed: status={result.get('status')} "
                f"body={result.get('raw_body', '')[:300]}"
            )
        # Сохраняем session_id
        if result.get("session_id"):
            self._session_id = result["session_id"]
        # Извлекаем результат
        for ev in result.get("events", []):
            data = ev.get("data")
            if isinstance(data, dict) and "result" in data:
                res = data["result"]
                self._initialized = True
                if self._verbose:
                    print(
                        f"[penpot-mcp] initialized: protocol={res.get('protocolVersion')} "
                        f"server={res.get('serverInfo')}",
                        file=sys.stderr,
                    )
                return res
        raise PenpotMCPError(f"initialize: no result event in response: {result}")

    def _notify_initialized(self) -> None:
        """Шаг 2: notifications/initialized (без ответы)."""
        if not self._session_id:
            raise PenpotMCPError("no session_id — вызовите initialize() сначала")
        payload = {
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {},
        }
        result = self._post(payload, timeout=15)
        # Допускаем 202 Accepted без тела — это норма для notification
        if result.get("status") not in (202, 200):
            if self._verbose:
                print(
                    f"[penpot-mcp] notify_initialized: status={result.get('status')} "
                    f"(202/200 expected)",
                    file=sys.stderr,
                )

    def _ensure_initialized(self) -> None:
        if not self._initialized:
            self.initialize()
            self._notify_initialized()

    def tools_list(self) -> list[dict[str, Any]]:
        """Список доступных инструментов."""
        self._ensure_initialized()
        payload = {"jsonrpc": "2.0", "id": self._request_id, "method": "tools/list"}
        self._request_id += 1
        result = self._post(payload, timeout=30)
        for ev in result.get("events", []):
            data = ev.get("data")
            if isinstance(data, dict) and "result" in data:
                return data["result"].get("tools", [])
        raise PenpotMCPError(f"tools/list failed: {result.get('raw_body', '')[:300]}")

    def call_tool(self, name: str, arguments: dict[str, Any] | None = None) -> Any:
        """Вызвать инструмент по имени. Возвращает 'content' из ответа."""
        self._ensure_initialized()
        payload = {
            "jsonrpc": "2.0",
            "id": self._request_id,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments or {}},
        }
        self._request_id += 1
        result = self._post(payload, timeout=120)
        for ev in result.get("events", []):
            data = ev.get("data")
            if not isinstance(data, dict):
                continue
            if "result" in data:
                return data["result"]
            if "error" in data:
                err = data["error"]
                raise PenpotMCPError(
                    f"tool '{name}' error: code={err.get('code')} "
                    f"message={err.get('message')} data={err.get('data')}"
                )
        raise PenpotMCPError(f"tool call '{name}' no result: {result.get('raw_body', '')[:300]}")

    def execute_code(self, js_code: str) -> Any:
        """Исполнить JavaScript в контексте Penpot plugin."""
        result = self.call_tool("execute_code", {"code": js_code})
        # MCP возвращает content-массив — извлечём текст
        content = result.get("content", []) if isinstance(result, dict) else []
        for c in content:
            if isinstance(c, dict) and c.get("type") == "text":
                return c.get("text", "")
        return result

    def high_level_overview(self) -> str:
        result = self.call_tool("high_level_overview", {})
        content = result.get("content", []) if isinstance(result, dict) else []
        for c in content:
            if isinstance(c, dict) and c.get("type") == "text":
                return c.get("text", "")
        return ""

    def penpot_api_info(self) -> str:
        result = self.call_tool("penpot_api_info", {})
        content = result.get("content", []) if isinstance(result, dict) else []
        for c in content:
            if isinstance(c, dict) and c.get("type") == "text":
                return c.get("text", "")
        return ""

    # ───────────────── CLI ─────────────────

    @staticmethod
    def _cli() -> int:
        parser = argparse.ArgumentParser(
            prog="penpot_mcp",
            description="CanvasDesk MCP-клиент для Penpot. Токен — через env.",
        )
        parser.add_argument(
            "--verbose", action="store_true", help="Отладочный вывод в stderr"
        )
        sub = parser.add_subparsers(dest="cmd", required=True)

        sub.add_parser("init", help="Только initialize — проверить handshake")

        sub.add_parser("tools-list", help="Список инструментов")

        p_call = sub.add_parser("tools-call", help="Вызвать инструмент по имени")
        p_call.add_argument("name", help="Имя инструмента (см. tools-list)")
        p_call.add_argument(
            "--args", default="{}", help='JSON-аргументы: \'{"k":"v"}\''
        )

        p_code = sub.add_parser("code", help="Исполнить JS в Penpot plugin context")
        p_code.add_argument("js", help="JavaScript код (или - для stdin)")

        sub.add_parser("api-info", help="Документация по Penpot Plugin API")
        sub.add_parser("overview", help="High-level overview по Penpot")

        sub.add_parser(
            "seed-design-system",
            help="Залить CanvasDesk design-system в активный файл Penpot",
        )

        args = parser.parse_args()

        try:
            client = PenpotMCP.from_env(verbose=args.verbose)
        except PenpotMCPConfigError as e:
            print(f"CONFIG ERROR: {e}", file=sys.stderr)
            return 2

        try:
            if args.cmd == "init":
                info = client.initialize()
                print(json.dumps(info, ensure_ascii=False, indent=2))
                return 0
            if args.cmd == "tools-list":
                client.initialize()
                client._notify_initialized()
                tools = client.tools_list()
                for t in tools:
                    print(f"• {t.get('name'):30s} — {t.get('description', '')[:100]}")
                return 0
            if args.cmd == "tools-call":
                client.initialize()
                client._notify_initialized()
                try:
                    args_obj = json.loads(args.args)
                except json.JSONDecodeError as e:
                    print(f"bad --args JSON: {e}", file=sys.stderr)
                    return 3
                result = client.call_tool(args.name, args_obj)
                print(json.dumps(result, ensure_ascii=False, indent=2))
                return 0
            if args.cmd == "code":
                js = args.js
                if js == "-":
                    js = sys.stdin.read()
                client.initialize()
                client._notify_initialized()
                out = client.execute_code(js)
                print(out if isinstance(out, str) else json.dumps(out, ensure_ascii=False, indent=2))
                return 0
            if args.cmd == "api-info":
                client.initialize()
                client._notify_initialized()
                info = client.penpot_api_info()
                print(info[:8000])
                return 0
            if args.cmd == "overview":
                client.initialize()
                client._notify_initialized()
                overview = client.high_level_overview()
                print(overview[:8000])
                return 0
            if args.cmd == "seed-design-system":
                # Lazy-import чтобы не тащить зависимость от сидера для CLI-only use
                try:
                    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
                    from penpot_seed_design_system import seed_design_system
                except ImportError as e:
                    print(f"сидер недоступен: {e}", file=sys.stderr)
                    return 4
                client.initialize()
                client._notify_initialized()
                result = seed_design_system(client)
                print(json.dumps(result, ensure_ascii=False, indent=2))
                return 0
        except PenpotMCPError as e:
            print(f"MCP ERROR: {e}", file=sys.stderr)
            return 1
        return 0


if __name__ == "__main__":
    sys.exit(PenpotMCP._cli())
