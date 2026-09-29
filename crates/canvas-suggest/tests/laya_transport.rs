//! Интеграционные тесты L1-транспорта (feature `l1-laya`): клиент против
//! фейкового `/v1/systemone` на std::net — без sidecar, без GPU, офлайн.
//!
//! Покрытие: форма payload (порт client.py), парсинг ответа, retry ×1,
//! таймаут → Transport-ошибка, smoke-гейт, деградация fusion(lex, ∅) = lex.

#![cfg(feature = "l1-laya")]

use canvas_suggest::fusion::fuse;
use canvas_suggest::laya::client::{parse_choice_answer, LayaClient, LayaError};
use canvas_suggest::laya::{run_smoke, SmokeFixture};
use canvas_suggest::types::OptionDesc;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Поведение фейкового сервера на каждое соединение.
enum Mode {
    /// Корректный JSON-ответ.
    Ok(&'static str),
    /// Первое соединение оборвать без ответа, дальше — Ok.
    CloseFirst(&'static str),
    /// Задержаться перед ответом (тест таймаута).
    Slow(&'static str),
}

struct FakeServer {
    addr: String,
    hits: Arc<AtomicUsize>,
    bodies: Arc<Mutex<Vec<String>>>,
}

fn spawn_server(mode: Mode) -> FakeServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr").to_string();
    let hits = Arc::new(AtomicUsize::new(0));
    let bodies = Arc::new(Mutex::new(Vec::new()));
    {
        let hits = Arc::clone(&hits);
        let bodies = Arc::clone(&bodies);
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                let Ok(mut stream) = conn else { break };
                let n = hits.fetch_add(1, Ordering::SeqCst);
                let (delay, respond) = match &mode {
                    Mode::Ok(resp) => (Duration::ZERO, *resp),
                    Mode::CloseFirst(resp) => {
                        if n == 0 {
                            (Duration::ZERO, "")
                        } else {
                            (Duration::ZERO, *resp)
                        }
                    }
                    Mode::Slow(resp) => (Duration::from_millis(600), *resp),
                };
                if delay > Duration::ZERO {
                    std::thread::sleep(delay);
                }
                if respond.is_empty() {
                    // оборвать соединение без HTTP-ответа
                    drop(shutdown_raw(&mut stream));
                    continue;
                }
                if let Some(req) = read_request(&mut stream) {
                    bodies.lock().unwrap().push(req);
                }
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    respond.len(),
                    respond
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
            }
        });
    }
    FakeServer { addr, hits, bodies }
}

fn shutdown_raw(stream: &mut TcpStream) -> std::io::Result<()> {
    stream.shutdown(std::net::Shutdown::Both)
}

/// Прочитать HTTP-запрос (заголовки + тело по Content-Length).
fn read_request(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    // читаем до конца заголовков
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if let Some(pos) = find_header_end(&buf) {
                    let headers = String::from_utf8_lossy(&buf[..pos]).to_uppercase();
                    let cl = headers
                        .lines()
                        .find_map(|l| l.strip_prefix("CONTENT-LENGTH:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if buf.len() >= pos + 4 + cl {
                        return Some(
                            String::from_utf8_lossy(&buf[pos + 4..pos + 4 + cl]).to_string(),
                        );
                    }
                }
            }
            Err(_) => return None,
        }
    }
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

const RESP: &str = r#"{"answers":{"main":{"probabilities":{"ue-gross-margin":0.51,"ue-cac":0.30,"none":0.19},"answer_confidence":0.51}},"usage":{"input_tokens":604,"output_tokens":0}}"#;

fn options() -> Vec<OptionDesc> {
    vec![
        OptionDesc::new("ue-cac", "CAC (стоимость привлечения клиента)"),
        OptionDesc::new("ue-gross-margin", "Маржа (margin): маржинальность"),
    ]
}

#[test]
fn client_choice_parses_and_payload_matches_harness() {
    let srv = spawn_server(Mode::Ok(RESP));
    let client = LayaClient::new(format!("http://{}", srv.addr), "multilingual", 2000);
    let ans = client
        .choice("[canvas] 8 nodes; cats: unit-economics(4)", &options())
        .expect("choice ok");
    assert_eq!(ans.confidence, 0.51);
    assert_eq!(ans.probs[0], ("ue-gross-margin".to_string(), 0.51));
    // payload: явная модель + criteria + state.document (порт client.py)
    let bodies = srv.bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1);
    let v: serde_json::Value = serde_json::from_str(&bodies[0]).unwrap();
    assert_eq!(v["model"], "multilingual");
    assert_eq!(
        v["state"]["document"],
        "[canvas] 8 nodes; cats: unit-economics(4)"
    );
    assert_eq!(v["questions"]["main"]["type"], "choice");
    assert!(v["questions"]["main"]["criteria"]["ue-cac"].is_string());
}

#[test]
fn transport_error_retried_once_then_ok() {
    let srv = spawn_server(Mode::CloseFirst(RESP));
    let client = LayaClient::new(format!("http://{}", srv.addr), "multilingual", 2000);
    let ans = client.choice("ctx", &options()).expect("retry спасает");
    assert_eq!(ans.probs[0].0, "ue-gross-margin");
    assert_eq!(srv.hits.load(Ordering::SeqCst), 2, "ровно две попытки");
}

#[test]
fn timeout_yields_transport_error() {
    let srv = spawn_server(Mode::Slow(RESP));
    let client = LayaClient::new(format!("http://{}", srv.addr), "multilingual", 150);
    let err = client.choice("ctx", &options()).expect_err("таймаут");
    assert!(matches!(err, LayaError::Transport(_)), "{err}");
}

#[test]
fn connection_refused_is_transport_error() {
    // порт почти наверняка свободен: listener не поднят
    let client = LayaClient::new("http://127.0.0.1:1", "multilingual", 300);
    let err = client
        .choice("ctx", &options())
        .expect_err("отказ соединения");
    assert!(matches!(err, LayaError::Transport(_)), "{err}");
}

#[test]
fn smoke_passes_on_reference_top1() {
    let srv = spawn_server(Mode::Ok(RESP));
    let client = LayaClient::new(format!("http://{}", srv.addr), "multilingual", 2000);
    let fx = SmokeFixture {
        document: "smoke".into(),
        options: options(),
        expect_top1: "ue-gross-margin".into(),
    };
    assert!(run_smoke(&client, &fx).is_ok());
}

#[test]
fn smoke_detects_drift() {
    let srv = spawn_server(Mode::Ok(RESP));
    let client = LayaClient::new(format!("http://{}", srv.addr), "multilingual", 2000);
    let fx = SmokeFixture {
        document: "smoke".into(),
        options: options(),
        expect_top1: "ue-cac".into(), // дрейф: модель стала отвечать иначе
    };
    let err = run_smoke(&client, &fx).expect_err("дрейф ловится");
    assert!(err.contains("smoke-дрейф"), "{err}");
}

/// Продукт-инвариант деградации (FR-079 §3): нет mm → fusion вырождается
/// в lex (порядок сохраняется, масштаб α не меняет ранж).
#[test]
fn degradation_fusion_without_mm_equals_lex() {
    let lex = vec![
        ("a".to_string(), 3.0),
        ("b".to_string(), 1.5),
        ("c".to_string(), 0.25),
    ];
    let fused = fuse(&lex, &[], 0.85);
    let ids: Vec<&str> = fused.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["a", "b", "c"]);
    // и порядок совпадает с чистым lex-ранжированием
    let mut sorted_lex = lex.clone();
    sorted_lex.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap().then_with(|| x.0.cmp(&y.0)));
    let lex_ids: Vec<&str> = sorted_lex.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(ids, lex_ids);
}

/// Парсинг ответа с отсутствующим answer_confidence (толерантность).
#[test]
fn parse_answer_without_confidence() {
    let a = parse_choice_answer(r#"{"answers":{"main":{"probabilities":{"x":1.0}}}}"#)
        .expect("без confidence тоже парсится");
    assert_eq!(a.confidence, 0.0);
    assert_eq!(a.probs.len(), 1);
}
