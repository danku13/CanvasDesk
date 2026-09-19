//! M8/W4 (прошивка, wasm-port §6.1, трек B): запуск App в браузере —
//! web-набор сервисов + `spawn_app` (winit web). Зеркало нативной
//! обёртки main.rs по составу (§3.1): тот же `App`, свой платформенный
//! слой — дублирования UI-логики нет.
//!
//! Состав web-сервисов (карта замен §3.2, волна 1):
//! - storage: `MemStorage` — сцена в памяти (FS Access/OPFS — W6);
//! - thumbs: `NoopThumbs` (WebImageThumbnailProvider — W10);
//! - watcher: `NoopWatch` — событий ФС нет, перечитывание по жесту
//!   «Перезагрузить» (§3.2, §4.2);
//! - search: `MemSearch` — индекс в памяти, ответы через тот же
//!   `SearchEvent`-протокол T14 (натив — FTS5 worker);
//! - clipboard: `NoopClipboard` (navigator.clipboard — W5+);
//! - widget_state: `None` (localStorage — W11);
//! - renderer: `SpawnLocalRendererLaunch` — async-init через
//!   `spawn_local` (§3.4), результат в слот, побудка кадром.
//!
//! Платформенные различия против натива (осознанные, план §6.1):
//! - нет single-instance/exit-листенера/десктоп-монитора/shell-шины/
//!   MCP-pipe (натив-Windows обвязка — не для web);
//! - нет `init_widgets` (реестр материализует пакеты через std::fs —
//!   решение OPFS/память в W11) и тик-потока виджетов (`std::thread`
//!   недоступен; gloo-interval — W11);
//! - конфиг — дефолт (localStorage `Settings::from_str` — W6).

use std::path::PathBuf;
use std::sync::Arc;

use canvas_app::app::{measured_result_reserve_height, App, AppEvent};
use canvas_core::{CanvasStorage, MemStorage, Settings};
use canvas_scene::SceneState;
use winit::event_loop::EventLoop;

use crate::renderer_launch::SpawnLocalRendererLaunch;

/// Запуск браузерной сборки: сцена → event loop → App с web-набором
/// сервисов → `spawn_app`. Вызывается из `#[wasm_bindgen(start)]` после
/// инициализации каркаса (panic-hook + tracing-консоль, `boot`).
pub fn spawn_desk() -> anyhow::Result<()> {
    // CR-012 (правка 2, FR-037/ADR-0012): уровень 2 refit высоты — точное
    // измерение шейпингом canvas-render; тот же хук, что в нативном main
    canvas_scene::install_measured_reserve(measured_result_reserve_height);
    // Сцена в памяти: стартовый канвас (seed); путь — логическое имя для
    // автосейва (W6 подставит FS Access/OPFS и реальные имена)
    let storage: Arc<dyn CanvasStorage> = Arc::new(MemStorage::new());
    let scene = SceneState::load_or_seed_with_storage(PathBuf::from("default.canvas"), storage);
    let event_loop = EventLoop::<AppEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    // Drag-drop (T9): winit web не даёт DroppedFile (план §7) —
    // заглушка-отправитель; DOM-листенеры — W6 (§4.2)
    let drag_sender: Arc<dyn Fn(canvas_core::dragdrop::DragEvent) + Send + Sync> =
        Arc::new(|_event: canvas_core::dragdrop::DragEvent| {});
    // Виджеты (M5): live-хост — волна 2 (§5); тик-таймер — W11
    let widget_sender: canvas_widgets::WidgetEventSender =
        Arc::new(|_event: canvas_widgets::WidgetEvent| {});
    // Поиск (T14): MemSearch — ответы будят цикл через proxy (паттерн
    // сервисов W3; тот же AppEvent::Search, что у нативного worker'а)
    let search_service = canvas_core::MemSearch::new(Arc::new(move |event| {
        let _ = proxy.send_event(AppEvent::Search(event));
    }));
    let app = App::new(
        scene,
        // W10: WebImageThumbnailProvider (createImageBitmap → атлас)
        Box::new(canvas_core::NoopThumbs),
        // W6: конфиг из localStorage (Settings::from_str, §3.2)
        Settings::default(),
        None,
        // W6: каталог кэша (OPFS); сейчас — нет кэша
        None,
        drag_sender,
        widget_sender,
        Box::new(canvas_core::NoopWatch),
        Box::new(search_service),
        // W5+: navigator.clipboard за трейтом (§3.2)
        Box::new(canvas_core::NoopClipboard),
        // W11: widget_state в localStorage
        None,
        false,
        // W4 (§3.4): async-init Renderer — spawn_local + слот доставки
        Box::new(SpawnLocalRendererLaunch),
    );
    // winit web: цикл не блокирует поток — spawn_app ставит обработчики
    // (rAF/ResizeObserver) и возвращает управление браузеру; дальше web-код
    // живёт в событиях. Нативная компиляция (rlib-тесты каркаса): App и
    // цикл только создаются и дропаются — запуск требует JS-рунтайм.
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(app);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (app, event_loop);
    Ok(())
}

#[cfg(test)]
mod tests {
    /// spawn_desk нельзя звать нативно (EventLoop требует оконную систему/
    /// JS-рунтайм) — компиляционный контракт обвязки покрывают типы:
    /// стратегия рендера (renderer_launch), App::new на заглушках
    /// (canvas-app, 157 тестов) и слот (canvas-render). Полная цепочка —
    /// браузерный дым приёмки W4.
    #[test]
    fn spawn_desk_signature_is_result() {
        // Компиляционный контракт: обвязка — свободная функция с
        // единым Result-типом ошибки (стартовая точка start())
        let _spawn: fn() -> anyhow::Result<()> = super::spawn_desk;
    }
}
