//! M8/W4 (wasm-port §3.4): платформенная стратегия инициализации Renderer.
//!
//! `Renderer::new` — async (wgpu-адаптер/устройство). На нативе главный
//! поток можно блокировать (`pollster::block_on` — сегодняшнее поведение);
//! в браузере блокировка главного потока означает дедлок всей страницы
//! (futура wgpu-web разрешается только оборотом JS event loop — план
//! §7, риск «pollster-блокировка потока на wasm»). Поэтому запуск
//! инициализации — инъектированная стратегия (паттерн W3-сервисов,
//! wasm-port §3.1: платформенные способности — за трейтами, реализации —
//! в платформенных крейтах, выбор — в точке сборки бинарника):
//!
//! - натив — `BlockOnRendererLaunch` (pollster; идентично коду до W4);
//! - web — `canvas_web::SpawnLocalRendererLaunch`: `spawn_local` + слот
//!   доставки результата + побудка кадра через `request_redraw`;
//! - тесты — `NoopRendererLaunch` (Pending с пустым слотом, инертен).
//!
//! Слот — `Rc<RefCell<…>>`: живёт только на главном потоке (web-футура
//! `spawn_local` исполняется там же; натив слот не использует вовсе),
//! пересечений потоков нет. App хранит слот до первого
//! `RedrawRequested` после готовности GPU (кадры до готовности
//! пропускаются — renderer `None`, деградация R14).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use winit::window::Window;

use crate::Renderer;

/// Результат запуска инициализации Renderer.
///
/// `Ready` боксирует Renderer (clippy large_enum_variant: натив-путь
/// живёт одно мгновение — от launch до install_renderer, цена бокса
/// несущественна; `Pending`/`Failed` — лёгкие варианты).
pub enum RendererLaunch {
    /// Renderer готов синхронно (натив: `pollster::block_on`).
    Ready(Box<Renderer>),
    /// Инициализация стартовала асинхронно (web: `spawn_local`): футура
    /// положит результат в слот и разбудит event loop; приложение забирает
    /// его первым `RedrawRequested` после готовности GPU.
    Pending(RendererSlot),
    /// Ошибка инициализации (семантика `Err` у `Renderer::new`).
    Failed(anyhow::Error),
}

/// Слот доставки результата async-инициализации (одноразовый): web-футура
/// кладёт `Ok(Renderer)`/`Err`, кадр-цикл приложения забирает.
#[derive(Clone)]
pub struct RendererSlot(Rc<RefCell<Option<anyhow::Result<Renderer>>>>);

impl RendererSlot {
    /// Новый пустой слот.
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(None)))
    }

    /// Положить результат (вызывается web-футурой на главном потоке).
    pub fn put(&self, result: anyhow::Result<Renderer>) {
        *self.0.borrow_mut() = Some(result);
    }

    /// Забрать результат, если готов (вызывается кадр-циклом приложения;
    /// повторный вызов после изъятия — `None`).
    pub fn take(&self) -> Option<anyhow::Result<Renderer>> {
        self.0.borrow_mut().take()
    }
}

impl Default for RendererSlot {
    fn default() -> Self {
        Self::new()
    }
}

/// Стратегия запуска async-инициализации Renderer. Инъектируется в
/// `App::new` (паттерн W3-сервисов): платформенный слой выбирает
/// реализацию в точке сборки бинарника.
pub trait RendererLauncher {
    /// Запустить инициализацию Renderer для окна. Синхронно готовый
    /// результат — `Ready`/`Failed`; асинхронный запуск — `Pending`+слот.
    fn launch(&self, window: Arc<Window>, prefer_dx12: bool) -> RendererLaunch;
}

/// Нативная стратегия: `pollster::block_on` — поведение идентично коду
/// до W4 (блокирующая инициализация, один раз при старте, SPEC §6.3).
pub struct BlockOnRendererLaunch;

impl RendererLauncher for BlockOnRendererLaunch {
    fn launch(&self, window: Arc<Window>, prefer_dx12: bool) -> RendererLaunch {
        match pollster::block_on(Renderer::new(window, prefer_dx12)) {
            Ok(renderer) => RendererLaunch::Ready(Box::new(renderer)),
            Err(err) => RendererLaunch::Failed(err),
        }
    }
}

/// Тестовая заглушка (паттерн `Noop*` из canvas-core/providers): всегда
/// `Pending` с пустым слотом — событие «инициализация пошла» без GPU.
pub struct NoopRendererLaunch;

impl RendererLauncher for NoopRendererLaunch {
    fn launch(&self, _window: Arc<Window>, _prefer_dx12: bool) -> RendererLaunch {
        RendererLaunch::Pending(RendererSlot::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Слот одноразовый: put → take → take(None) — семантика доставки
    /// ровно одного результата async-инициализации.
    #[test]
    fn slot_delivers_exactly_once() {
        let slot = RendererSlot::new();
        assert!(slot.take().is_none(), "новый слот пуст");
        slot.put(Err(anyhow::anyhow!("тест-ошибка")));
        assert!(slot.take().expect("результат положен").is_err());
        assert!(slot.take().is_none(), "повторный take — пусто");
    }

    /// Клон слота разделяет ячейку: футура кладёт в клон, приложение
    /// забирает из оригинала (web-схема доставки).
    #[test]
    fn slot_clone_shares_cell() {
        let slot = RendererSlot::new();
        let producer = slot.clone();
        producer.put(Err(anyhow::anyhow!("из клона")));
        assert!(slot.take().is_some());
    }

    /// Контракт варианта Pending — несёт пустой слот (App в redraw
    /// забирает его до отрисовки; полная цепочка — браузерный дым
    /// canvas-web, launch требует живого event loop).
    #[test]
    fn pending_variant_carries_empty_slot() {
        let outcome = RendererLaunch::Pending(RendererSlot::new());
        match outcome {
            RendererLaunch::Pending(slot) => assert!(slot.take().is_none()),
            _ => panic!("ожидали вариант Pending"),
        }
    }
}
