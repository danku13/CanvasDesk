//! canvas-mcp-headless (FR-037 MW3, ADR-0012): headless MCP-сервер без
//! Windows и GUI — [`HeadlessSession`] за [`AppTransport`] сшивает
//! MCP-инструменты canvas-scene с протокольным мостом canvas-mcp
//! in-process. Канал stdio и исполнение в wasmtime — bin
//! `canvasdesk-mcp-headless` (run_stdio_with_transport); волна 2 M8
//! (WebSocket-мост) получит здесь готовую серверную сторону.
//!
//! Сессия in-memory: файловых операций нет, модель собирается
//! `graph_apply`, как в гейтах эталонов CP1/CP3/CP5 (ADR-0012).

use std::path::PathBuf;

use canvas_core::templates::TemplateRegistry;
use canvas_core::Canvas;
use canvas_mcp::AppTransport;
use canvas_scene::{mcp_dispatch, mcp_unwrap_call, SceneState};
use serde_json::Value;

/// In-process сессия приложения для MCP-моста: модель сцены (без окна,
/// pipe и ФС) + встроенный реестр шаблонов (`include_dir` — wasm-OK).
///
/// `send_line` воспроизводит конверт приложения (`on_mcp_wake` в
/// canvas-app) буквально: `parse_envelope → (id) → mcp_unwrap_call →
/// mcp_dispatch → build_result | build_call_error`; notification
/// (id == None) — тишина; битый конверт — `build_error` с null-id.
/// Один источник семантики — обе стороны обязаны сходиться на одних
/// кейсах (ADR-0012). Viewport-зеркало живёт в `SceneState` — синк с
/// Camera не нужен (окна нет, viewport_get/set работают напрямую).
pub struct HeadlessSession {
    scene: SceneState,
    templates: TemplateRegistry,
    /// Ответ последнего `send_line` (один запрос — один ответ, строже
    /// протокола не требуется: мост ждёт ровно один recv на send).
    /// `None` после notification: `recv_line` вернёт None — мост
    /// трактует это как таймаут, ровно как прод-приложение, которое
    /// на notification не отвечает.
    reply: Option<String>,
}

impl Default for HeadlessSession {
    fn default() -> Self {
        Self::new()
    }
}

impl HeadlessSession {
    /// In-memory сессия на пустом канвасе.
    pub fn new() -> Self {
        Self {
            scene: SceneState::new(Canvas::default(), PathBuf::new()),
            templates: TemplateRegistry::builtin(),
            reply: None,
        }
    }

    /// Модель сцены (инспекция драйвером/тестами; MW6-файловый режим).
    pub fn scene(&self) -> &SceneState {
        &self.scene
    }

    /// Мутабельный доступ к модели (драйвер, тесты).
    pub fn scene_mut(&mut self) -> &mut SceneState {
        &mut self.scene
    }
}

impl AppTransport for HeadlessSession {
    /// Принять строку-запрос (полный JSON-RPC конверт tools/call).
    fn send_line(&mut self, line: &str) -> Result<(), String> {
        let reply = match canvas_mcp::parse_envelope(line) {
            // Notification (id == None) — отвечать нечему (тишина)
            Ok(request) if request.id.is_none() => None,
            Ok(request) => {
                let id = request.id.unwrap_or(Value::Null);
                let (method, params) = mcp_unwrap_call(&request.method, &request.params);
                match mcp_dispatch(&mut self.scene, &self.templates, &method, &params) {
                    Ok(value) => Some(canvas_mcp::build_result(&id, &value)),
                    Err(message) => Some(canvas_mcp::build_call_error(&id, &message)),
                }
            }
            // Битый конверт — JSON-RPC-ошибка (id из частичного разбора,
            // иначе null); конверт ошибки разворачивается мостом в isError
            Err(err) => Some(canvas_mcp::build_error(
                err.id.as_ref(),
                err.code,
                &err.message,
            )),
        };
        self.reply = reply;
        Ok(())
    }

    /// Ответ на последний запрос (один на send — строже не требуется).
    fn recv_line(&mut self) -> Option<String> {
        self.reply.take()
    }

    /// Сессия in-process — соединение всегда живо.
    fn is_connected(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_mcp::{handle_input, handle_line, HandleOutcome};
    use serde_json::json;

    /// Oracle-допуск ±1 % (ADR-0006, как в гейтах эталонов).
    fn assert_close(actual: f64, expected: f64, what: &str) {
        let tolerance = (expected * 0.01).abs().max(1e-9);
        assert!(
            (actual - expected).abs() <= tolerance,
            "{what}: {actual} != {expected} (±1 %)"
        );
    }

    /// Полный протокольный цикл tools/call через мост: конверт →
    /// send_line (сессия) → dispatch → recv_line → unwrap_app_payload →
    /// build_call_result. Возвращает `result` ответа (не конверт).
    /// Паникует на isError — для негативных веток есть `call_raw`.
    fn call(transport: &mut Option<HeadlessSession>, name: &str, arguments: &str) -> Value {
        let line = format!(
            r#"{{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{{"name":"{name}","arguments":{arguments}}}}}"#
        );
        let result = call_raw(transport, &line);
        if result.get("isError").and_then(Value::as_bool) == Some(true) {
            panic!("tools/call {name} — isError: {result}");
        }
        result
    }

    /// tools/call по сырой строке конверта: возвращает `result` (isError
    /// проходит насквозь — негативные ветки).
    fn call_raw(transport: &mut Option<HeadlessSession>, line: &str) -> Value {
        let HandleOutcome::Reply(reply) = handle_line(line, transport) else {
            panic!("tools/call должен ответить: {line}");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("JSON-RPC ответ");
        assert_eq!(parsed["id"], 7, "id вернулся: {parsed}");
        parsed["result"].clone()
    }

    /// Мини-эталон №1 (ADR-0005/0006): Нагрузка → CDN → Gateway.
    /// Тот же батч, что в гейте CP3 (canvas-scene tests:
    /// graph_apply_assembles_mini_reference_with_oracle) — один источник
    /// oracle-чисел для нативных и wasm-прогонов.
    const MINI_OPS: &str = r#"[
        {"op":"node_create_note","ref":"traffic","x":0,"y":0,"width":280,"text":"dau = 200000\nsess = 3\nreq = 10 req\npeak = 3\navg_rps = dau × sess × req / 86400 s\npeak_rps = avg_rps × peak"},
        {"op":"template_instantiate","ref":"cdn","template":"com.canvasdesk.cdn","x":360,"y":0,"params":{"cache_hit":0.9,"origin_latency":20}},
        {"op":"template_instantiate","ref":"gw","template":"com.canvasdesk.api-gateway","x":720,"y":0,"params":{"latency_budget":5,"auth_overhead":2}},
        {"op":"param_set","ref":"traffic","param":"dau","value":200000},
        {"op":"edge_create","fromRef":"traffic","toRef":"cdn","kind":"value","toParam":"rps"},
        {"op":"edge_create","fromRef":"cdn","toRef":"gw","kind":"value","fromOutput":"origin_rps","toParam":"rps"},
        {"op":"node_move","ref":"cdn","x":400,"y":40},
        {"op":"node_move","ref":"gw","x":760,"y":40},
        {"op":"node_create_note","ref":"cost","x":0,"y":320,"text":"cdn_cost = 50 $\n gw_cost = 36 $\n total = cdn_cost + gw_cost"},
        {"op":"edge_create","fromRef":"cost","toRef":"cdn"}
    ]"#;

    const TRAFFIC_TEXT: &str = "dau = 200000\nsess = 3\nreq = 10 req\npeak = 3\navg_rps = dau × sess × req / 86400 s\npeak_rps = avg_rps × peak";

    /// id ноды из created-массива graph_apply по ref-имени.
    fn ref_id<'a>(created: &'a Value, ref_name: &str) -> &'a str {
        created
            .as_array()
            .expect("created — массив")
            .iter()
            .find(|entry| entry["ref"] == ref_name)
            .and_then(|entry| entry["node_id"].as_str())
            .unwrap_or_else(|| panic!("ref {ref_name} в created: {created}"))
    }

    /// Запись узла из отчёта analyze_bottlenecks по id.
    fn node_report(report: &Value, node_id: &str) -> Value {
        report["nodes"]
            .as_array()
            .expect("массив nodes")
            .iter()
            .find(|entry| entry["id"] == node_id)
            .cloned()
            .unwrap_or_else(|| panic!("нода {node_id} в отчёте: {report}"))
    }

    /// Сщbuild мини-эталона №1 через полный протокольный цикл.
    /// Возвращает (created, flow) ответа graph_apply.
    fn build_mini(transport: &mut Option<HeadlessSession>) -> (Value, Value) {
        let result = call(
            transport,
            "graph_apply",
            &format!(r#"{{"operations": {MINI_OPS}}}"#),
        );
        let structured = result["structuredContent"].clone();
        assert_eq!(structured["ok"], true, "ответ: {structured}");
        (structured["created"].clone(), structured["flow"].clone())
    }

    // --- Протокольный цикл (MW3): initialize → tools/list → tools/call ---

    /// Handshake: initialize с поддерживаемой версией — эхо версии
    /// (спек 2025-06-18), сервер отрекомендован как canvasdesk.
    #[test]
    fn initialize_echoes_supported_protocol() {
        let mut transport = Some(HeadlessSession::new());
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"headless-test"}}}"#;
        let HandleOutcome::Reply(reply) = handle_line(line, &mut transport) else {
            panic!("initialize должен ответить");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("JSON");
        assert_eq!(parsed["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(parsed["result"]["serverInfo"]["name"], "canvasdesk");
        assert!(parsed["result"]["capabilities"]["tools"].is_object());
    }

    /// tools/list отдаёт каталог моста — каждый инструмент диспетчера
    /// объявлен (39: 26 базовых + analyze_bottlenecks + 9 whatif_* +
    /// schemes_list/schemes_apply + lineage).
    #[test]
    fn tools_list_advertises_full_catalog() {
        let mut transport = Some(HeadlessSession::new());
        let line = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
        let HandleOutcome::Reply(reply) = handle_line(line, &mut transport) else {
            panic!("tools/list должен ответить");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("JSON");
        let tools = parsed["result"]["tools"].as_array().expect("массив tools");
        assert_eq!(
            tools.len(),
            39,
            "26 (FR-032/FR-033) + analyze_bottlenecks (FR-016) + 9 whatif_* (FR-017, CP6) + 3 (PRD-0008 Q5 + FR-048 X2)"
        );
    }

    /// tools/call проходит полный цикл: конверт → сессия → диспетчер →
    /// разворот конверта → text-контент + structuredContent (FR-034).
    #[test]
    fn tools_call_canvas_info_roundtrip() {
        let mut transport = Some(HeadlessSession::new());
        let result = call(&mut transport, "canvas_info", "{}");
        assert!(
            result["structuredContent"].is_object(),
            "canvas_info — объект: {result}"
        );
        assert!(!result["content"].as_array().expect("content").is_empty());
        assert!(
            result["content"][0]["type"] == "text",
            "text-контент для старых хостов: {result}"
        );
    }

    // --- Эталоны (гейты CP1/CP3/CP5 через протокол) ---

    /// CP3-гейт: один graph_apply собирает мини-эталон №1 — flow в ответе
    /// сходится с oracle ADR-0006 ±1 % (те же числа, что в canvas-scene).
    #[test]
    fn graph_apply_mini_reference_flow_oracle() {
        let mut transport = Some(HeadlessSession::new());
        let (created, flow) = build_mini(&mut transport);

        let traffic = ref_id(&created, "traffic");
        let cdn = ref_id(&created, "cdn");
        let gw = ref_id(&created, "gw");
        let cost = ref_id(&created, "cost");

        assert_close(
            flow[traffic]["value"].as_f64().expect("peak_rps"),
            208.3333,
            "peak_rps ноды «Нагрузка»",
        );
        assert_eq!(flow[traffic]["unit"], "req/s");
        assert_close(
            flow[cdn]["value"].as_f64().expect("cdn W"),
            0.0342857,
            "CDN W (Erlang-C, ρ 0.417)",
        );
        assert_close(
            flow[cdn]["outputs"]["origin_rps"]["value"]
                .as_f64()
                .expect("origin_rps"),
            20.8333,
            "CDN.origin_rps (проливание rps = 208.33)",
        );
        assert_close(
            flow[gw]["value"].as_f64().expect("gw W"),
            0.0032,
            "Gateway W (fromOutput=origin_rps)",
        );
        assert_close(
            flow[cost]["value"].as_f64().expect("cost"),
            86.0,
            "смета мини-эталона",
        );

        // Состояние сессии живёт между вызовами: модель не пересоздаётся
        let scene = transport.as_ref().expect("сессия").scene();
        assert_eq!(scene.canvas.nodes.len(), 4, "4 ноды мини-эталона");
    }

    /// CP5-гейт (ρ-лестница FR-016): базовая линия здорова (CDN ρ 0.417),
    /// DAU×2 → Warn (0.833), DAU×5.35 → Overload (2.23, ветка C ADR-0006).
    /// Весь сценарий — tools/call через протокол (node_update_text — путь
    /// с полным пересчётом; ленивость футера CR-012 не мешает отчёту).
    #[test]
    fn analyze_bottlenecks_rho_gate_cp5() {
        let mut transport = Some(HeadlessSession::new());
        let (created, _) = build_mini(&mut transport);
        let traffic = ref_id(&created, "traffic").to_owned();
        let cdn = ref_id(&created, "cdn").to_owned();

        // --- Базовая линия: здоровый ландшафт ---
        let report = call(&mut transport, "analyze_bottlenecks", "{}")["structuredContent"].clone();
        let nodes = report["nodes"].as_array().expect("массив");
        assert_eq!(nodes.len(), 2, "cdn + gw: {report}");
        assert_eq!(report["thresholds"]["warn_util"], 0.7);
        assert_eq!(report["thresholds"]["critical_util"], 0.9);
        let cdn_report = node_report(&report, &cdn);
        assert_eq!(
            cdn_report["severity"], "none",
            "ρ 0.417 < 0.7: {cdn_report}"
        );
        assert_close(
            cdn_report["utilization"].as_f64().expect("ρ cdn"),
            0.4167,
            "CDN ρ (named-выход utilization)",
        );
        assert_eq!(cdn_report["badge"], "42% · W: 34 ms", "бейдж: {cdn_report}");

        // --- DAU×2 (400k): ρ 0.833 → Warn ---
        // Аргументы собирает serde_json (переносы строк в тексте —
        // валидные \n внутри JSON-строки, сырые литералы в format! дали
        // бы control-character в конверте)
        let text_x2 = TRAFFIC_TEXT.replacen("200000", "400000", 1);
        let args_x2 = json!({"id": traffic, "text": text_x2}).to_string();
        call(&mut transport, "node_update_text", &args_x2);
        let report = call(&mut transport, "analyze_bottlenecks", "{}")["structuredContent"].clone();
        let cdn_report = node_report(&report, &cdn);
        assert_eq!(
            cdn_report["severity"], "warn",
            "ρ 0.833 ∈ [0.7, 0.9): {cdn_report}"
        );
        assert_close(
            cdn_report["utilization"].as_f64().expect("ρ ×2"),
            0.8333,
            "CDN ρ после ×2",
        );
        assert_eq!(
            cdn_report["badge"], "83% · W: 120 ms",
            "бейдж Warn: {cdn_report}"
        );

        // --- DAU×5.35 (1.07M, ветка C): ρ 2.23 → Overload ---
        let text_x535 = TRAFFIC_TEXT.replacen("200000", "1070000", 1);
        let args_x535 = json!({"id": traffic, "text": text_x535}).to_string();
        call(&mut transport, "node_update_text", &args_x535);
        let report = call(&mut transport, "analyze_bottlenecks", "{}")["structuredContent"].clone();
        let cdn_report = node_report(&report, &cdn);
        assert_eq!(
            cdn_report["severity"], "overload",
            "ρ 2.23 ≥ 1: {cdn_report}"
        );
        assert_close(
            cdn_report["utilization"].as_f64().expect("ρ ×5.35"),
            2.2292,
            "CDN ρ = 2.23 (ветка C)",
        );
        assert_eq!(cdn_report["badge"], "OVERLOAD 223%", "бейдж: {cdn_report}");
    }

    // --- Негативные ветки протокола ---

    /// Неизвестный инструмент — MCP-идиома: isError внутри result
    /// (НЕ JSON-RPC error), сообщение диспетчера проходит насквозь.
    #[test]
    fn unknown_tool_reports_iserror() {
        let mut transport = Some(HeadlessSession::new());
        let line = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"no_such_tool","arguments":{}}}"#;
        let result = call_raw(&mut transport, line);
        assert_eq!(
            result.get("isError").and_then(Value::as_bool),
            Some(true),
            "isError: {result}"
        );
        let text = result["content"][0]["text"].as_str().expect("текст ошибки");
        assert!(!text.is_empty(), "сообщение об ошибке не пустое");
    }

    /// Ошибка внутри инструмента (диспетчер Err) — тоже isError
    /// (конверт build_call_error разворачивается мостом без двойной
    /// упаковки — FR-034).
    #[test]
    fn tool_internal_error_is_iserror() {
        let mut transport = Some(HeadlessSession::new());
        let result = call_raw(
            &mut transport,
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"node_get","arguments":{"id":"no-such-node"}}}"#,
        );
        assert_eq!(
            result.get("isError").and_then(Value::as_bool),
            Some(true),
            "node_get несуществующей ноды — isError: {result}"
        );
    }

    /// Неизвестный метод — JSON-RPC error −32601 (не касается сессии).
    #[test]
    fn unknown_method_is_32601() {
        let mut transport = Some(HeadlessSession::new());
        let line = r#"{"jsonrpc":"2.0","id":9,"method":"workspace/symbol","params":{}}"#;
        let HandleOutcome::Reply(reply) = handle_line(line, &mut transport) else {
            panic!("неизвестный метод должен ответить ошибкой");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("JSON");
        assert_eq!(parsed["error"]["code"], -32601);
    }

    /// Битый JSON — parse error −32700 (handle_input, id null).
    #[test]
    fn broken_json_is_32700() {
        let mut transport = Some(HeadlessSession::new());
        let outcomes = handle_input("not json at all", &mut transport);
        assert_eq!(outcomes.len(), 1);
        let HandleOutcome::Reply(reply) = &outcomes[0] else {
            panic!("битый JSON должен ответить");
        };
        let parsed: Value = serde_json::from_str(reply).expect("JSON");
        assert_eq!(parsed["error"]["code"], -32700);
        assert_eq!(parsed["id"], Value::Null);
    }

    /// Batch (спек 2025-03-26): массив из двух запросов — два ответа
    /// по порядку; сессия обрабатывает каждый через полный цикл.
    #[test]
    fn batch_roundtrip() {
        let mut transport = Some(HeadlessSession::new());
        let input = r#"[
            {"jsonrpc":"2.0","id":11,"method":"ping"},
            {"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"canvas_info","arguments":{}}}
        ]"#;
        let outcomes = handle_input(input, &mut transport);
        assert_eq!(outcomes.len(), 2, "по ответу на элемент");
        let HandleOutcome::Reply(first) = &outcomes[0] else {
            panic!("ping должен ответить");
        };
        let parsed: Value = serde_json::from_str(first).expect("JSON");
        assert_eq!(parsed["id"], 11);
        assert_eq!(parsed["result"], json!({}));
        let HandleOutcome::Reply(second) = &outcomes[1] else {
            panic!("tools/call должен ответить");
        };
        let parsed: Value = serde_json::from_str(second).expect("JSON");
        assert_eq!(parsed["id"], 12);
        assert!(parsed["result"]["structuredContent"].is_object());
    }

    /// Notification — тишина: уведомления не порождают ответа ни на
    /// уровне моста, ни на уровне сессии (семантика on_mcp_wake).
    #[test]
    fn notification_silence() {
        let mut transport = Some(HeadlessSession::new());
        assert_eq!(
            handle_line(
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
                &mut transport
            ),
            HandleOutcome::Silent
        );
        // На уровне сессии notification тоже молчит: recv после send — None
        transport
            .as_mut()
            .expect("сессия")
            .send_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .expect("send");
        assert_eq!(
            transport.as_mut().expect("сессия").recv_line(),
            None,
            "notification не порождает ответа сессии"
        );
    }

    /// Сессия переживает последовательность вызовов (состояние модели
    /// накапливается): graph_apply → node_create_note → nodes_list.
    #[test]
    fn session_state_persists_across_calls() {
        let mut transport = Some(HeadlessSession::new());
        build_mini(&mut transport);
        // nodes_list возвращает массив — structuredContent только для
        // объектов (FR-034), массив приходит в text-контенте как JSON
        let before_result = call(&mut transport, "nodes_list", "{}");
        let before_text = before_result["content"][0]["text"].as_str().expect("text");
        let before: Value = serde_json::from_str(before_text).expect("nodes_list — JSON");
        let before_count = before.as_array().map(Vec::len).unwrap_or(0);
        assert_eq!(before_count, 4, "мини-эталон собран: {before}");
        call(
            &mut transport,
            "node_create_note",
            r#"{"x": 0, "y": 800, "text": "заметка драйвера"}"#,
        );
        let after_result = call(&mut transport, "nodes_list", "{}");
        let after_text = after_result["content"][0]["text"].as_str().expect("text");
        let after: Value = serde_json::from_str(after_text).expect("nodes_list — JSON");
        let after_count = after.as_array().map(Vec::len).unwrap_or(0);
        assert_eq!(
            after_count,
            before_count + 1,
            "модель накапливает состояние"
        );
    }
}
