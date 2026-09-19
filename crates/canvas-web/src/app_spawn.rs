//! M8/W4 (прошивка, wasm-port §6.1, трек B): запуск App в браузере —
//! web-набор сервисов + `spawn_app` (winit web). Зеркало нативной
//! обёртки main.rs по составу (§3.1): тот же `App`, свой платформенный
//! слой — дублирования UI-логики нет.
//!
//! Состав web-сервисов (карта замен §3.2; W6 — storage, W11 — виджеты, W10 — превью):
//! - storage: `OpfsStorage` (зеркало + фоновая запись в OPFS, §4) —
//!   дефолт и фолбэк; `?stress` и отказ OPFS — `MemStorage` (без
//!   сохранения); после «Открыть с диска» — `FsAccessStorage` (диск);
//! - thumbs: `WebImageThumbs` (W10) — createImageBitmap → OffscreenCanvas
//!   downscale → RGBA в существующий thumbs-атлас;
//! - watcher: `NoopWatch` — событий ФС нет, перечитывание по жесту
//!   «Перезагрузить» (§3.2, §4.2);
//! - search: `MemSearch` — индекс в памяти, ответы через тот же
//!   `SearchEvent`-протокол T14 (натив — FTS5 worker);
//! - clipboard: `WebClipboard` — navigator.clipboard + кэш (W5);
//! - config: TOML из localStorage (`canvasdesk.config`); запись обратно
//!   настроек — W12 (read-only в W6);
//! - widget_state: `WebWidgetState` — localStorage (W11);
//! - виджеты: реестр встроенных пакетов в памяти (`App::init_widgets`,
//!   W11), тик LOD — setInterval (W11); live-хост — волна 2 (§5);
//! - renderer: `SpawnLocalRendererLaunch` — async-init через
//!   `spawn_local` (§3.4), результат в слот, побудка кадром.
//!
//! Аргументы запуска (§3.2 «Аргументы CLI» → web): `?stress=N`,
//! `?stress-widgets=N`, `?canvas=` (W6 — имя канваса в OPFS), `?log=`
//! (W7).
//!
//! Платформенные различия против натива (осознанные, план §6.1):
//! - нет single-instance/exit-листенера/десктоп-монитора/shell-шины/
//!   MCP-pipe (натив-Windows обвязка — не для web);
//! - файловые жесты (открыть/экспорт/недавние/drop) — DOM-панель и
//!   DOM-листенеры (W6, §4.2), drag-превью T9 на web недоступно.

use std::path::PathBuf;
use std::sync::Arc;

use canvas_app::app::{
    add_stress_widgets, measured_result_reserve_height, stress_canvas, App, AppEvent,
};
use canvas_core::Settings;
#[cfg(not(target_arch = "wasm32"))]
use canvas_core::{CanvasStorage, MemStorage};
use canvas_scene::SceneState;
use winit::event_loop::EventLoop;

use crate::renderer_launch::SpawnLocalRendererLaunch;
use crate::url_params::WebParams;
#[cfg(target_arch = "wasm32")]
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

/// Запуск браузерной сборки: инициализация каркаса уже сделана (`boot`).
/// W6: хранилище инициализируется асинхронно (OPFS/недавние) ДО построения
/// App — построение сцены и App переехало в async-таск
/// ([`spawn_desk_web`]); натив (rlib-тесты) строит всё синхронно
/// ([`spawn_desk_native`]) — запуск требует оконной системы/JS.
pub fn spawn_desk() -> anyhow::Result<()> {
    // CR-012 (правка 2, FR-037/ADR-0012): уровень 2 refit высоты — точное
    // измерение шейпингом canvas-render; тот же хук, что в нативном main
    canvas_scene::install_measured_reserve(measured_result_reserve_height);
    let params = read_params();
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(err) = spawn_desk_web(params).await {
                tracing::error!(target: "canvas_web", %err, "не удалось запустить CanvasDesk (web)");
            }
        });
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = params;
        spawn_desk_native()
    }
}

/// W6 (web): async-путь — хранилище (OPFS init: недавние/`?canvas=`/сеяние)
/// → сцена → App → DOM-панель и drop-листенеры → `spawn_app`.
#[cfg(target_arch = "wasm32")]
async fn spawn_desk_web(params: WebParams) -> anyhow::Result<()> {
    let init = crate::opfs::init_scene(&params).await;
    // Общее OPFS-хранилище для DOM-drop/reopen — только если оно
    // действительно OPFS (stress/fallback-MemStorage копии не сохраняют)
    if let Some(opfs) = init.opfs {
        crate::web_state::set_opfs_storage(opfs);
    }
    let mut scene = match params.stress {
        // Нагрузочная сцена — всегда в памяти (не засорять OPFS/recent)
        Some(n) => {
            tracing::info!(target: "canvas_web", nodes = n, "нагрузочный режим ?stress");
            SceneState::with_storage(
                stress_canvas(n),
                PathBuf::from("stress.canvas"),
                init.storage,
            )
        }
        None => SceneState::with_storage(init.canvas, init.path, init.storage),
    };
    // M5 (T20-F): ?stress-widgets=N — детерминированная сетка виджет-нод
    if let Some(k) = params.stress_widgets {
        let added = add_stress_widgets(&mut scene.canvas, k);
        tracing::info!(target: "canvas_web", widgets = added, "нагрузочные виджеты добавлены");
        scene.mark_dirty();
    }
    let event_loop = EventLoop::<AppEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    // M5 (T20-F): события host'а виджетов (WidgetEvent) — тем же паттерном,
    // что у сервисов W3; W11: Tick из setInterval (widgets_web) тоже идёт
    // сюда и будит цикл как на нативе.
    // Drag-drop (T9): winit web не даёт DroppedFile (план §7) — заглушка-
    // отправитель; приём файлов — DOM-drop (W6) шлёт OpenScene напрямую.
    let widget_sender: canvas_widgets::WidgetEventSender = {
        let proxy = proxy.clone();
        Arc::new(move |event| {
            let _ = proxy.send_event(AppEvent::Widget(event));
        })
    };
    let drag_sender: Arc<dyn Fn(canvas_core::dragdrop::DragEvent) + Send + Sync> =
        Arc::new(|_event: canvas_core::dragdrop::DragEvent| {});
    // Поиск (T14): MemSearch — ответы будят цикл через proxy (паттерн
    // сервисов W3; тот же AppEvent::Search, что у нативного worker'а).
    // W7: DEBUG-оракулы дыма — «поисковый backend ответил hits=N».
    let search_proxy = proxy.clone();
    let search_service = canvas_core::MemSearch::new(Arc::new(move |event| {
        match &event {
            canvas_core::SearchEvent::Ready(hits) => {
                tracing::debug!(target: "canvas_web", hits = hits.len(), "поисковый backend ответил");
            }
            canvas_core::SearchEvent::Indexed(count) => {
                tracing::debug!(target: "canvas_web", count, "поисковый индекс обновлён");
            }
        }
        let _ = search_proxy.send_event(AppEvent::Search(event));
    }));
    // W6: конфиг — TOML из localStorage (read-only; запись обратно — W12)
    let settings = load_settings();
    let mut app = App::new(
        scene,
        // W10 (§3.2): превью картинок — createImageBitmap → атлас;
        // результаты будят цикл через AppEvent::ThumbsReady
        Box::new(crate::web_thumbs::WebImageThumbs::new(proxy.clone())),
        settings,
        None,
        // W6: каталог кэша; web без SQLite — None = реестр виджетов
        // в памяти (W11) и превью без дискового кэша (W10, декод дешёв)
        None,
        drag_sender,
        // W11: клон — sender ещё понадобится install_tick (тики setInterval)
        widget_sender.clone(),
        Box::new(canvas_core::NoopWatch),
        Box::new(search_service),
        // W5 (§3.2): navigator.clipboard за трейтом — Ctrl+C/X/V живут
        Box::new(WebClipboard::new()),
        // W11 (§3.2): widget_state — localStorage за трейтом core
        Some(Box::new(crate::widgets_web::web::WebWidgetState::new())),
        false,
        // W4 (§3.4): async-init Renderer — spawn_local + слот доставки
        Box::new(SpawnLocalRendererLaunch),
    );
    // W11 (§5): реестр виджетов — как на нативе (в памяти: встроенные
    // пакеты из include_dir; выбор режима — в App::new по каталогу кэша)
    app.init_widgets();
    // W11: тик LOD/refresh — setInterval 1 с (зеркало widget-tick-потока)
    crate::widgets_web::web::install_tick(widget_sender);
    // W6: DOM-панель хранилища (открыть/недавние/экспорт) + приём drop
    crate::toolbar::install(proxy.clone());
    crate::drop_files::install(proxy);
    // winit web: цикл не блокирует поток — spawn_app ставит обработчики
    // (rAF/ResizeObserver) и возвращает управление браузеру.
    use winit::platform::web::EventLoopExtWebSys;
    event_loop.spawn_app(app);
    Ok(())
}

/// W6 (web): конфиг из localStorage (`canvasdesk.config`, TOML — тот же
/// формат, что config.toml натива). Битый текст — дефолт + warn
/// (страница открывается всегда); клампы CR-003/FR-028 — как в `load`.
#[cfg(target_arch = "wasm32")]
fn load_settings() -> Settings {
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return Settings::default();
    };
    let Ok(text) = storage.get_item("canvasdesk.config") else {
        return Settings::default();
    };
    let Some(text) = text else {
        return Settings::default();
    };
    match toml::from_str::<Settings>(&text) {
        Ok(mut settings) => {
            settings.port_zone_px = canvas_core::clamp_port_zone(settings.port_zone_px);
            settings.onboarding_defers =
                canvas_core::clamp_onboarding_defers(settings.onboarding_defers);
            settings
        }
        Err(err) => {
            tracing::warn!(target: "canvas_web", %err, "разбор конфига localStorage (используются дефолты)");
            Settings::default()
        }
    }
}

/// Нативный путь rlib-тестов каркаса: синхронная сборка App на
/// заглушках (сцена — MemStorage, в памяти), запуск невозможен —
/// требуется оконная система. Компиляционный контракт обвязки.
#[cfg(not(target_arch = "wasm32"))]
fn spawn_desk_native() -> anyhow::Result<()> {
    let params = read_params();
    // Сцена в памяти: стартовый канвас (seed); web подставит OPFS (W6)
    let storage: Arc<dyn CanvasStorage> = Arc::new(MemStorage::new());
    let mut scene = match params.stress {
        Some(n) => {
            tracing::info!(target: "canvas_web", nodes = n, "нагрузочный режим ?stress");
            SceneState::with_storage(stress_canvas(n), PathBuf::from("stress.canvas"), storage)
        }
        None => SceneState::load_or_seed_with_storage(PathBuf::from("default.canvas"), storage),
    };
    if let Some(k) = params.stress_widgets {
        let added = add_stress_widgets(&mut scene.canvas, k);
        tracing::info!(target: "canvas_web", widgets = added, "нагрузочные виджеты добавлены");
        scene.mark_dirty();
    }
    let event_loop = EventLoop::<AppEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let drag_sender: Arc<dyn Fn(canvas_core::dragdrop::DragEvent) + Send + Sync> =
        Arc::new(|_event: canvas_core::dragdrop::DragEvent| {});
    let widget_sender: canvas_widgets::WidgetEventSender =
        Arc::new(|_event: canvas_widgets::WidgetEvent| {});
    let search_service = canvas_core::MemSearch::new(Arc::new(move |event| {
        let _ = proxy.send_event(AppEvent::Search(event));
    }));
    let app = App::new(
        scene,
        Box::new(canvas_core::NoopThumbs),
        Settings::default(),
        None,
        None,
        drag_sender,
        widget_sender,
        Box::new(canvas_core::NoopWatch),
        Box::new(search_service),
        Box::new(canvas_core::NoopClipboard),
        None,
        false,
        Box::new(SpawnLocalRendererLaunch),
    );
    // App и цикл только создаются и дропаются (JS-рунтайма нет)
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
        assert_eq!(params.canvas, None);
    }

    /// spawn_desk нельзя звать нативно (EventLoop требует оконную систему/
    /// JS-рунтайм) — компиляционный контракт обвязки покрывают типы:
    /// стратегия рендера (renderer_launch), App::new на заглушках
    /// (canvas-app, 157 тестов) и слот (canvas-render). Полная цепочка —
    /// браузерный дым приёмки W4/W5/W6.
    #[test]
    fn spawn_desk_signature_is_result() {
        // Компиляционный контракт: обвязка — свободная функция с
        // единым Result-типом ошибки (стартовая точка start())
        let _spawn: fn() -> anyhow::Result<()> = super::spawn_desk;
    }
}
