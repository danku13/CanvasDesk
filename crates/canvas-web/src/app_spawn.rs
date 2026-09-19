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
//! - clipboard: `WebClipboard` — navigator.clipboard + кэш (W5);
//! - widget_state: `None` (localStorage — W11);
//! - renderer: `SpawnLocalRendererLaunch` — async-init через
//!   `spawn_local` (§3.4), результат в слот, побудка кадром.
//!
//! Аргументы запуска (§3.2 «Аргументы CLI» → web, W5): URL-параметры
//! `?stress=N`/`?stress-widgets=N` — зеркало флагов `--stress`/`--stress-widgets`
//! нативной обёртки (приёмка W5: `?stress=5000` — 60 fps); `?canvas=` — W6.
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

use canvas_app::app::{
    add_stress_widgets, measured_result_reserve_height, stress_canvas, App, AppEvent,
};
use canvas_core::{CanvasStorage, MemStorage, Settings};
use canvas_scene::SceneState;
use winit::event_loop::EventLoop;

use crate::renderer_launch::SpawnLocalRendererLaunch;
use crate::url_params::WebParams;
use crate::web_clipboard::WebClipboard;

/// Чтение URL-параметров запуска (`location.search`). Выделено ради
/// нативных тестов: JS-часть — одна строка; парсинг — [`parse_query`]
/// (чистая функция, тесты в url_params). Вызывается дважды: из `boot`
/// (уровень лога — до инициализации трейсинга) и из `spawn_desk` —
/// чтение location дёшево, а сигнатура точки входа остаётся без параметров.
#[cfg(target_arch = "wasm32")]
pub(crate) fn read_params() -> WebParams {
    use crate::url_params::parse_query;
    let query = web_sys::window()
        .map(|window| window.location().search().unwrap_or_default())
        .unwrap_or_default();
    match parse_query(&query) {
        Ok(params) => params,
        Err(err) => {
            tracing::warn!(target: "canvas_web", %err, query = %query, "битый URL-параметр — запуск без него");
            WebParams::default()
        }
    }
}

/// Нативная заглушка (rlib-тесты каркаса): env-аргументов/URL нет — дефолт.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn read_params() -> WebParams {
    WebParams::default()
}

/// Запуск браузерной сборки: сцена → event loop → App с web-набором
/// сервисов → `spawn_app`. Вызывается из `#[wasm_bindgen(start)]` после
/// инициализации каркаса (panic-hook + tracing-консоль, `boot`).
pub fn spawn_desk() -> anyhow::Result<()> {
    // CR-012 (правка 2, FR-037/ADR-0012): уровень 2 refit высоты — точное
    // измерение шейпингом canvas-render; тот же хук, что в нативном main
    canvas_scene::install_measured_reserve(measured_result_reserve_height);
    // Аргументы запуска (§3.2 → web, W5): ?stress=N / ?stress-widgets=N —
    // зеркало parse_args нативной обёртки (path/desktop на web не существуют)
    let params = read_params();
    // Сцена в памяти: стартовый канвас (seed); путь — логическое имя для
    // автосейва (W6 подставит FS Access/OPFS и реальные имена)
    let storage: Arc<dyn CanvasStorage> = Arc::new(MemStorage::new());
    let mut scene = match params.stress {
        // Зеркало стресс-ветки main.rs: stress.canvas, чтобы автосейв не
        // затирал default.canvas (MemStorage — в памяти, но имя важно для
        // HUD/логов и будущей W6-материализации)
        Some(n) => {
            tracing::info!(target: "canvas_web", nodes = n, "нагрузочный режим ?stress");
            SceneState::with_storage(stress_canvas(n), PathBuf::from("stress.canvas"), storage)
        }
        None => SceneState::load_or_seed_with_storage(PathBuf::from("default.canvas"), storage),
    };
    // M5 (T20-F): ?stress-widgets=N — детерминированная сетка виджет-нод
    if let Some(k) = params.stress_widgets {
        let added = add_stress_widgets(&mut scene.canvas, k);
        tracing::info!(target: "canvas_web", widgets = added, "нагрузочные виджеты добавлены");
        scene.mark_dirty();
    }
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
    // сервисов W3; тот же AppEvent::Search, что у нативного worker'а).
    // W7: на web нет консоли кроме браузерной — ответ backend'а дублируем
    // в tracing (DEBUG; виден с ?log=debug) — дым приёмки ищет эти строки:
    // доказательство круга Query → MemSearch → SearchEvent → proxy.
    let search_service = canvas_core::MemSearch::new(Arc::new(move |event| {
        match &event {
            canvas_core::SearchEvent::Ready(hits) => {
                tracing::debug!(target: "canvas_web", hits = hits.len(), "поисковый backend ответил");
            }
            canvas_core::SearchEvent::Indexed(count) => {
                tracing::debug!(target: "canvas_web", count, "поисковый индекс обновлён");
            }
        }
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
        // W5 (§3.2): navigator.clipboard за трейтом — Ctrl+C/X/V живут
        Box::new(WebClipboard::new()),
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
    use super::read_params;

    /// Аргументы запуска на нативной заглушке — дефолт (no stress):
    /// компиляционный контракт обвязки сохраняется, запуск без параметров
    /// сеет обычный default.canvas.
    #[test]
    fn native_params_are_default() {
        let params = read_params();
        assert_eq!(params.stress, None);
        assert_eq!(params.stress_widgets, None);
    }

    /// spawn_desk нельзя звать нативно (EventLoop требует оконную систему/
    /// JS-рунтайм) — компиляционный контракт обвязки покрывают типы:
    /// стратегия рендера (renderer_launch), App::new на заглушках
    /// (canvas-app, 157 тестов) и слот (canvas-render). Полная цепочка —
    /// браузерный дым приёмки W4/W5.
    #[test]
    fn spawn_desk_signature_is_result() {
        // Компиляционный контракт: обвязка — свободная функция с
        // единым Result-типом ошибки (стартовая точка start())
        let _spawn: fn() -> anyhow::Result<()> = super::spawn_desk;
    }
}
