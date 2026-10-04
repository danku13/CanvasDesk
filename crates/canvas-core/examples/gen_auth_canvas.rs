//! Генератор .canvas с шаблонной нодой auth-service (для браузерного репро).
//! Запуск: cargo run -p canvas-core --example gen_auth_canvas -- out.canvas
use canvas_core::templates::{instantiate, TemplateRegistry};

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: gen_auth_canvas <out.canvas>");
    let registry = TemplateRegistry::builtin();
    let manifest = registry
        .find("com.canvasdesk.auth-service")
        .expect("auth-service в builtin-реестре");
    let mut canvas = canvas_core::Canvas::default();
    let node = instantiate(
        manifest,
        &Default::default(),
        "auth1".to_string(),
        -240.0,
        -300.0,
    )
    .expect("инстанс auth-service");
    canvas.nodes.push(node);
    let json = canvas.to_json().expect("serialize");
    std::fs::write(&out, json).expect("write");
    println!("written {}", out);
}
