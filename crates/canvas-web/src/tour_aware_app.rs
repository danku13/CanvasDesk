//! FR-028 v2: wrapper `ApplicationHandler` вокруг `App`, который после
//! каждого event'а дёргает `App::drain_tour_signals` и прокидывает
//! каждый сигнал в `crate::tour_signal::emit(name)` — то есть в
//! `window.__canvasdeskTour.signal(name)` в JS-bus.
//!
//! Не модифицирует `App` (он остаётся platform-нейтральным); просто
//! делегирует все методы `ApplicationHandler<AppEvent>` во внутренний
//! App, а после каждого вызова дёргает drain.
//!
//! Нативные rlib-тесты этот модуль не используют (canvas-web в нативном
//! режиме только компилируется, не запускается — нет окна). Gated на
//! `#[cfg(target_arch = "wasm32")]` (как `js_glue`, `toolbar`, `ime`).
//!
//! Альтернатива (отвергнута): слить drain в сам `App::about_to_wait`.
//! Не подходит — `about_to_wait` вызывается реже, чем `window_event`
//! (только когда event loop простаивает); сигналы от мутаторов во
//! время event'ов (например, `create_note_at` в обработчике клика)
//! задерживались бы до следующего простоя — плохой UX для tour-гейтов.
//! Wrapper вызывает drain после *каждого* event'а — latency минимальна.

#![cfg(target_arch = "wasm32")]

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use canvas_app::app::{App, AppEvent};

/// Wrapper вокруг `App`, эмитящий tour-сигналы после каждого event'а.
pub(crate) struct TourAwareApp {
    inner: App,
}

impl TourAwareApp {
    pub(crate) fn new(app: App) -> Self {
        Self { inner: app }
    }

    /// Дренировать pending tour-сигналы и эмитить каждый в JS-bus.
    /// Идемпотентно: пустой drain → 0 эмиссий.
    ///
    /// FR-095: там же синхронизируется состояние текстового ввода
    /// ([`App::text_input_active`] → шим виртуальной клавиатуры): хук
    /// срабатывает после КАЖДОГО события цикла, поэтому переход
    /// редактор/поиск ↔ канвас доезжает до web-слоя без отдельных мостов.
    fn drain_and_emit(&mut self) {
        crate::ime::set_text_input_active(self.inner.text_input_active());
        // UR-003: синхронизация DOM-хрома с GPU-панелями во всю высоту —
        // body-классы сдвигают #author-bar/#w6-toolbar из-под углов
        // палитры (Ctrl+P) и агент-панели (Ctrl+I). Идемпотентно.
        let (panel_left, panel_right) = self.inner.html_panel_overlap();
        crate::toolbar::set_panel_overlap(panel_left, panel_right);
        let signals = self.inner.drain_tour_signals();
        for name in signals {
            crate::tour_signal::emit(&name);
        }
    }
}

impl ApplicationHandler<AppEvent> for TourAwareApp {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, start_cause: winit::event::StartCause) {
        self.inner.new_events(event_loop, start_cause);
        self.drain_and_emit();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        self.inner.window_event(event_loop, window_id, event);
        self.drain_and_emit();
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        // TourSignal — складываем в pending (это путь для программной
        // эмиссии через EventLoopProxy::send_event). После вызова
        // inner.user_event drain_and_emit прокиднёт в JS.
        self.inner.user_event(event_loop, event);
        self.drain_and_emit();
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        self.inner.device_event(event_loop, device_id, event);
        self.drain_and_emit();
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.resumed(event_loop);
        self.drain_and_emit();
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.suspended(event_loop);
        self.drain_and_emit();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.about_to_wait(event_loop);
        self.drain_and_emit();
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.exiting(event_loop);
        self.drain_and_emit();
    }

    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        self.inner.memory_warning(event_loop);
        self.drain_and_emit();
    }
}
