//! canvas-app — нативный бинарь `canvasdesk`: тонкая обёртка запуска.
//!
//! M8/W2 (wasm-port §3.1): `App` и обработчики событий вынесены в
//! библиотечную часть (`canvas_app::app`); здесь осталась только нативная
//! инициализация — разбор CLI, трейсинг, сервисы shell (тамбнейлы, вотчер,
//! поиск, виджеты), event loop и `run_app`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use canvas_app::app::{
    add_stress_widgets, measured_result_reserve_height, parse_args, stress_canvas, App, AppEvent,
};
use canvas_core::{resolve_node_path, watched_dirs, Settings, ThumbnailProvider};
use canvas_scene::SceneState;
use canvas_shell::{SearchCommand, SearchService, ThumbService, WatchService};
use winit::event_loop::{EventLoop, EventLoopProxy};

fn main() -> anyhow::Result<()> {
    // FR-008: подкоманда `mcp` — режим MCP-посредника (stdio ↔ pipe) того
    // же бинарника: один exe на весь стек. Перехват ДО инициализации
    // трейсинга: stdout в mcp-режиме занят протоколом (ADR-0010/FR-035 —
    // stdout только JSON-RPC; run_stdio молчалив, диагностика — в stderr).
    // Файл с именем «mcp» открывается как ./mcp (путь с префиксом).
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(String::as_str) == Some("mcp") {
        return canvas_mcp::run_stdio(&argv[1..]);
    }
    // По умолчанию info, но без спама внутренних крейтов wgpu; переопределяется через RUST_LOG
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("info,wgpu_hal=warn,wgpu_core=warn")
    });
    // FR-035/ADR-0010 (чистота stdout): контракт «stdout — протокол/продукт,
    // stderr — диагностика». tracing_subscriber::fmt() по умолчанию писал в
    // stdout с ANSI — автоспавненный из моста GUI клал цветные логи прямо в
    // JSON-RPC-канал («Invalid JSON \x1b[2m…» у hermes). Логи — в stderr;
    // ANSI — только на живом терминале (машиночитаемые хвосты stderr без
    // escape-последовательностей).
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
        .init();
    // Версия сборки первой строкой лога: version (Cargo.toml) + git-коммит +
    // флаг «грязной» рабочей копии + профиль — по логу видно, какую именно
    // сборку запустили (env от build.rs; без git — «unknown»)
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        commit = option_env!("CANVASDESK_GIT_COMMIT").unwrap_or("unknown"),
        dirty = option_env!("CANVASDESK_GIT_DIRTY").unwrap_or("?"),
        profile = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        "CanvasDesk запускается"
    );
    // T17 (R5 + краш-сейф): sentinel от прошлой аварийной сессии →
    // форс-восстановление иконок ДО всего остального, независимо от
    // режима запуска (TASKS T17: «kill -9 → следующий запуск
    // восстанавливает»; kill обходит Drop-страховку guard'а)
    #[cfg(windows)]
    if canvas_shell::desktop::icons::crash_recovery() {
        tracing::info!("иконки десктопа восстановлены после аварийной сессии (sentinel)");
    }
    let args = parse_args(&std::env::args().skip(1).collect::<Vec<_>>())?;
    // T15-relaunch: single-instance handoff ДО загрузки сцены/конфига —
    // повторный запуск (в т.ч. перезапуск на --desktop из меню канваса)
    // сигналит работающему инстансу штатный выход и ждёт смерти предыдущего
    // владельца мьютекса. Пока ждём — сцена не читается: старый инстанс
    // успеет сохранить dirty-сцену без гонки записи/чтения default.canvas.
    #[cfg(windows)]
    let _instance_guard = {
        use canvas_shell::desktop::single_instance;
        if single_instance::signal_exit() {
            tracing::info!("работающий инстанс получил сигнал завершения — ждём его выхода");
        }
        match single_instance::InstanceGuard::acquire(single_instance::SINGLE_INSTANCE_WAIT_MS) {
            Some(guard) => Some(guard),
            None => {
                tracing::warn!(
                    wait_ms = single_instance::SINGLE_INSTANCE_WAIT_MS,
                    "предыдущий инстанс не завершился вовремя — запускаемся вторым (деградация R14)"
                );
                None
            }
        }
    };
    // Настройки приложения (~/.canvasdesk/config.toml); битый/отсутствующий
    // файл — дефолты + warn, приложение не падает
    let config_path = canvas_shell::default_config_path();
    let (settings, config_warn) = match &config_path {
        Some(path) => Settings::load(path),
        None => (Settings::default(), None),
    };
    if let Some(warn) = config_warn {
        tracing::warn!(%warn, "конфиг не применён, дефолты");
    }
    // CR-012 (правка 2, FR-037/ADR-0012): уровень 2 refit высоты — точное
    // измерение шейпингом (canvas-render) инжектируется в canvas-scene ДО
    // загрузки сцены: стартовые высоты нод сразу точные (как до выноса).
    // Headless/wasm работает на консервативной оценке уровня 1 (growth-only).
    canvas_scene::install_measured_reserve(measured_result_reserve_height);
    let scene = match args.stress {
        Some(n) => {
            tracing::info!(nodes = n, path = %args.path.display(), "нагрузочный режим --stress");
            let canvas = stress_canvas(n);
            if let Err(err) = canvas.save(&args.path) {
                tracing::warn!(%err, "не удалось сохранить стресс-сцену");
            }
            SceneState::new(canvas, args.path)
        }
        None => SceneState::load_or_seed(args.path),
    };
    // M5: --stress-widgets N — детерминированная сетка виджет-нод (часы)
    let mut scene = scene;
    if let Some(k) = args.stress_widgets {
        let added = add_stress_widgets(&mut scene.canvas, k);
        tracing::info!(widgets = added, "нагрузочные виджеты добавлены");
        scene.mark_dirty();
    }
    let event_loop = EventLoop::<AppEvent>::with_user_event().build()?;
    // Пул тамбнейлов (T6): провайдер Windows + SQLite-кэш; worker'ы будят
    // event loop через proxy — иначе при ControlFlow::Wait результаты
    // лежали бы в канале до следующего ввода
    let proxy: EventLoopProxy<AppEvent> = event_loop.create_proxy();
    // Exit-листенер single-instance (T15-relaunch): новый запуск (в т.ч.
    // перезапуск на --desktop из меню канваса) сигналит событие — поток будит
    // event loop через AppEvent::InstanceExit, приложение штатно сохраняется
    // и выходит, освобождая мьютекс для нового инстанса. Провал — warn:
    // повторные запуски не закроют этот инстанс сигналом (деградация R14).
    #[cfg(windows)]
    {
        let proxy = proxy.clone();
        if let Err(err) = canvas_shell::desktop::single_instance::spawn_exit_listener(move || {
            let _ = proxy.send_event(AppEvent::InstanceExit);
        }) {
            tracing::warn!(%err, "exit-листенер не запущен — повторный запуск не закроет этот инстанс");
        }
    }
    // Отправитель drag-событий в event loop (T9): тот же паттерн, что и
    // ThumbService-вокер — IDropTarget (shell) шлёт AppEvent::Drag через proxy
    let drag_sender: Arc<dyn Fn(canvas_shell::dragdrop::DragEvent) + Send + Sync> = {
        let proxy = proxy.clone();
        Arc::new(move |event| {
            let _ = proxy.send_event(AppEvent::Drag(event));
        })
    };
    // M5 (T20-F): события host'а виджетов (WidgetEvent) — тем же паттерном;
    // прокси берём ЗДЕСЬ, у EventLoop: у ActiveEventLoop, доступного в
    // resumed(), нет create_proxy (winit 0.30) — локальный Linux-чек этого
    // не видит, виндовую компиляцию ловит только CI (урок a9488ae)
    let widget_sender: canvas_widgets::WidgetEventSender = {
        let proxy = proxy.clone();
        Arc::new(move |event| {
            let _ = proxy.send_event(AppEvent::Widget(event));
        })
    };
    // Файловый вотчер (T10): агрегатор shell шлёт батчи FileEvent через proxy;
    // первичный набор директорий — сразу после загрузки сцены, дальше —
    // sync_watch_dirs по событиям модели (дроп/удаление/rename)
    let file_sender: canvas_shell::FileEventSender = {
        let proxy = proxy.clone();
        Arc::new(move |events| {
            let _ = proxy.send_event(AppEvent::FileEvents(events));
        })
    };
    let mut watcher = WatchService::new(file_sender);
    watcher.sync_dirs(&watched_dirs(&scene.canvas, &scene.canvas_dir()));
    #[cfg(windows)]
    let provider: Arc<dyn ThumbnailProvider + Send + Sync> =
        Arc::new(canvas_shell::ShellThumbnailProvider);
    #[cfg(not(windows))]
    let provider: Arc<dyn ThumbnailProvider + Send + Sync> =
        Arc::new(canvas_shell::NoopThumbnailProvider);
    let cache = canvas_shell::default_cache_dir().and_then(|dir| {
        match canvas_shell::ThumbCache::open(&dir) {
            Ok(cache) => Some(cache),
            Err(err) => {
                tracing::warn!(%err, "тамбнейл-кэш недоступен, работаем без него");
                None
            }
        }
    });
    let thumbs = ThumbService::new(provider, cache, {
        let proxy = proxy.clone();
        Some(Arc::new(move || {
            let _ = proxy.send_event(AppEvent::ThumbsReady);
        }))
    });
    // Поисковый индекс (T14): worker-поток FTS5 в общем cache.db; ответы —
    // AppEvent::Search через proxy (паттерн ThumbService/Watcher). Ошибка
    // открытия БД — деградация: warn внутри, пустые результаты (SPEC §5.3)
    let search_responder: canvas_shell::SearchResponder = {
        let proxy = proxy.clone();
        Arc::new(move |event| {
            let _ = proxy.send_event(AppEvent::Search(event));
        })
    };
    let search_cache_dir = canvas_shell::default_cache_dir().unwrap_or_else(|| PathBuf::from("."));
    let search_service = SearchService::spawn(search_cache_dir, search_responder);
    // Первичная индексация file-нод загруженного канваса (T14): полный
    // пересбор таблицы, лишние записи удаляются (ReplaceAll)
    {
        let canvas_dir = scene.canvas_dir();
        let entries: Vec<canvas_shell::IndexEntry> = scene
            .canvas
            .nodes
            .iter()
            .filter_map(|node| {
                let file = node.file.as_ref()?;
                Some(canvas_shell::IndexEntry {
                    path: resolve_node_path(file, &canvas_dir),
                    display_name: Path::new(file)
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| file.clone()),
                })
            })
            .collect();
        search_service.command(SearchCommand::ReplaceAll { entries });
    }
    event_loop.run_app(&mut {
        let mut app = App::new(
            scene,
            thumbs,
            settings,
            config_path,
            drag_sender,
            widget_sender,
            watcher,
            search_service,
            args.desktop,
        );
        // M5 (T20-F): реестр виджетов (материализация встроенных + скан)
        app.init_widgets();
        // M5: тик-поток host'а (1 c) — будит цикл для refresh-снапшотов
        // (LOD-расписание считает менеджер по времени, тик — только побудка;
        // паттерн — сервисы T15/T16, sender через EventLoopProxy)
        {
            let proxy = proxy.clone();
            std::thread::Builder::new()
                .name("widget-tick".to_owned())
                .spawn(move || loop {
                    std::thread::sleep(Duration::from_secs(1));
                    let _ = proxy.send_event(AppEvent::Widget(canvas_widgets::WidgetEvent::Tick));
                })
                .ok();
        }
        // Shell-монитор десктопа (T15): поток WinEventHook + DPI-поллинг;
        // слежка (Watch) устанавливается в resumed() после attach.
        // Спавним при --desktop до attach — событие WorkerWDestroyed может
        // прийти раньше, чем приложение дойдёт до recovery-логики
        #[cfg(windows)]
        if args.desktop {
            let proxy = proxy.clone();
            let responder: canvas_shell::desktop::monitor::DesktopResponder =
                Arc::new(move |event| {
                    let _ = proxy.send_event(AppEvent::Desktop(event));
                });
            app.set_desktop_monitor(
                canvas_shell::desktop::monitor::DesktopMonitorService::spawn(responder),
            );
        }
        // Шина системных событий (T16): message-only окно на отдельном
        // потоке; спавн БЕЗ привязки к --desktop — события сессии/сна/
        // shell-файлов полезны в любом режиме (план §8.7); провал — warn
        // внутри spawn + деградация (R14). Паттерн спавна — T15-монитор.
        #[cfg(windows)]
        {
            let proxy = proxy.clone();
            let responder: canvas_shell::shell_events::window::ShellResponder =
                Arc::new(move |event| {
                    let _ = proxy.send_event(AppEvent::Shell(event));
                });
            app.set_shell_events(
                canvas_shell::shell_events::window::ShellEventService::spawn(responder),
            );
            // Первичный набор SHChangeNotify-подписок — через единую точку
            // sync_watch_dirs (вотчер уже синхронизирован в main() выше —
            // дифф-синк идемпотентен; команды лягут в канал шины и дрени-
            // руются по WM_APP_WAKE после создания окна потоком)
            app.sync_watch_dirs();
        }
        // MCP named pipe (MCP-интеграция): worker-поток \\.\pipe\canvasdesk
        // принимает JSON-RPC от canvas-mcp-посредника; waker — тот же паттерн,
        // что ThumbService (worker будит event loop через proxy). Провал spawn —
        // warn внутри + None: MCP недоступен, приложение работает как обычно.
        #[cfg(windows)]
        {
            let proxy = proxy.clone();
            let mcp_waker: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
                let _ = proxy.send_event(AppEvent::McpWake);
            });
            app.set_mcp_server(canvas_shell::mcp_pipe::McpPipeServer::spawn(
                canvas_mcp::PIPE_NAME,
                mcp_waker,
            ));
        }
        // Не-Windows: режим десктопа недоступен — предупреждение и обычный
        // оконный режим (деградация, SPEC §9; ядро приложения то же)
        #[cfg(not(windows))]
        if args.desktop {
            tracing::warn!("--desktop поддерживается только на Windows — оконный режим");
        }
        app
    })?;
    Ok(())
}
