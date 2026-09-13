//! Типы контракта host'а виджетов (кроссплатформенные, T20-E/F):
//! запросы live-отображения и кадр применения. Реализация host'а —
//! `host/mod.rs` под cfg(windows); чистые типы здесь, чтобы менеджер
//! приложения и тесты компилировались на любой ОС.

use crate::layout::PhysRect;
use crate::manifest::WidgetManifest;
use crate::ThemeInfo;
use crate::WidgetProps;
use std::path::PathBuf;
use std::sync::Arc;

/// Отправитель событий приложения (EventLoopProxy-обёртка): host будит
/// цикл из колбэков WebView2 (все приходят на UI-поток, но прокси —
/// единственный легальный путь событий в event loop).
pub type WidgetEventSender = Arc<dyn Fn(crate::WidgetEvent) + Send + Sync>;

/// Запрос на live-отображение виджета (менеджер строит из LOD-решения).
#[derive(Debug, Clone)]
pub struct LiveRequest {
    pub node_id: String,
    /// Папка пакета (для виртуального origin).
    pub package_dir: PathBuf,
    pub manifest: WidgetManifest,
    pub props: WidgetProps,
    pub theme: ThemeInfo,
    /// Прямоугольник контента в ФИЗИЧЕСКИХ px клиентской области окна.
    pub rect: PhysRect,
    /// Радиус скругления в физ. px (CORNER_RADIUS × zoom × scale).
    pub corner: i32,
    /// Зум канваса (для ZoomFactor).
    pub zoom: f32,
}

/// Кадр применения (менеджер → хост), порядок: destroy → final_capture →
/// hide → live → refresh.
#[derive(Debug, Default)]
pub struct FrameApplication {
    /// Создать/обновить/показать live-инстансы.
    pub live: Vec<LiveRequest>,
    /// Скрыть и приостановить (final_capture уже отработал).
    pub hide: Vec<String>,
    /// Снять финальный снапшот перед скрытием.
    pub final_capture: Vec<String>,
    /// Уничтожить инстансы (выход из пула/битая нода/удаление).
    pub destroy: Vec<String>,
    /// Освежить снапшот suspended-виджета (resume → capture → suspend).
    pub refresh: Vec<String>,
}

/// Ошибка host'а.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("среда WebView2 не создана (рантайм недоступен?)")]
    NoEnvironment,
    #[error("инстанс виджета `{0}` не найден")]
    InstanceNotFound(String),
    #[error("Win32/COM: {0}")]
    #[cfg(windows)]
    Win(#[from] windows::core::Error),
    #[error("{0}")]
    Other(String),
}
