//! M8/W6 (wasm-port §4.2): DOM-панель хранилища web-сборки (зеркало
//! файловых жестов нативной обвязки: «Открыть…»/недавние). Канвас —
//! GPU-UI winit, файловые действия браузера живут в DOM: пикер
//! (`showOpenFilePicker`), менеджер канвасов, экспорт (download-blob) —
//! всё требует `window`/жеста, поэтому кнопки.
//!
//! Кнопки — статичная разметка index.html (W12 дорисует стиль); Rust
//! вешает листенеры и держит подпись «Канвасы: <имя>» в синкре с
//! web_state (set_recent_label из каждой точки открытия).
//!
//! FR-106 (мультиканвас C3, issue #7): кнопка «Недавние» (reopen
//! последнего) становится входом в менеджер канвасов — до двухзонного
//! чипа активного канваса №21c (волна C4 заменит вход; решение №37b:
//! «Недавние» уходит, его работу забирает менеджер). Клик шлёт
//! `AppEvent::CanvasManagerOpen` — оверлей открывается в App.

use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use winit::event_loop::EventLoopProxy;

use canvas_app::app::AppEvent;

use crate::ime::canvas_target;

/// Привязать кнопки панели к web-действиям (вызывается после event loop).
pub(crate) fn install(proxy: EventLoopProxy<AppEvent>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let recent_proxy = proxy.clone();
    bind(&document, "btn-open", move || {
        let proxy = proxy.clone();
        wasm_bindgen_futures::spawn_local(async move {
            crate::fs_access::open_from_disk(proxy).await;
        });
    });
    // FR-106 (C3, issue #7): «Канвасы» — вход в менеджер канвасов (до чипа
    // №21c волны C4). Reopen последнего недавнего уходит вместе с кнопкой
    // «Недавние» (решение №37b): менеджер покрывает сценарий выбором из
    // списка + Enter (disk-хэндлы — «Открыть с диска…» остаётся рядом).
    bind(&document, "btn-recent", move || {
        let _ = recent_proxy.send_event(AppEvent::CanvasManagerOpen);
    });
    bind(&document, "btn-export", || {
        wasm_bindgen_futures::spawn_local(async move {
            crate::export::export_active().await;
        });
    });
    // FR-076: «Экспорт HTML» — артефакт защиты (GAP-01): офлайн-файл со
    // снимком канваса, значениями и what-if таблицей
    bind(&document, "btn-export-html", || {
        wasm_bindgen_futures::spawn_local(async move {
            crate::export::export_html_active().await;
        });
    });
    tracing::info!(target: "canvas_web", "DOM-панель хранилища подключена");
}

/// Повесить обработчик клика на кнопку по id (отсутствие кнопки — warn:
/// битая разметка не должна валить запуск). FR-100: после действия кнопки
/// фокус возвращается канвасу — winit-web слушает keydown ТОЛЬКО на нём,
/// фокус, оставшийся на кнопке, «убивает» клавиатуру до следующего клика
/// по канвасу (UR-001-02: после клика по тулбару Ctrl+A печатал «a»).
fn bind(document: &web_sys::Document, id: &str, mut handler: impl FnMut() + 'static) {
    let element = match document.get_element_by_id(id) {
        Some(element) => element,
        None => {
            tracing::warn!(target: "canvas_web", id, "кнопка панели не найдена в разметке");
            return;
        }
    };
    let button: web_sys::HtmlElement = match element.dyn_into() {
        Ok(button) => button,
        Err(_) => {
            tracing::warn!(target: "canvas_web", id, "элемент панели не кнопка");
            return;
        }
    };
    let return_focus: web_sys::Document = document.clone();
    let closure = Closure::wrap(Box::new(move || {
        handler();
        // Синхронно в обработчике клика (жест): фокус обратно канвасу.
        if let Some(canvas) = canvas_target(&return_focus) {
            if let Some(html) = canvas.dyn_ref::<web_sys::HtmlElement>() {
                let _ = html.focus();
            }
        }
    }) as Box<dyn FnMut()>);
    let _ = button.add_event_listener_with_callback(
        "click",
        closure.as_ref().unchecked_ref::<js_sys::Function>(),
    );
    closure.forget(); // singleton-панель: живёт до выгрузки страницы
}

/// Обновить подпись кнопки «Канвасы» под имя активного канваса
/// (синхронизация DOM ↔ web_state; ошибки молча — декоративный элемент).
/// CR-014: CSS панели эллипсирует длинное имя (`max-width` + `overflow:
/// hidden`), поэтому ПОЛНОЕ имя дублируется в `title` кнопки — тултип
/// показывает его при наведении, эллипсис ничего не прячет безвозвратно.
pub(crate) fn set_recent_label(name: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    if let Some(element) = document.get_element_by_id("btn-recent") {
        element
            .unchecked_ref::<web_sys::Node>()
            .set_text_content(Some(name));
        if let Err(err) = element.set_attribute("title", &format!("Канвасы: {name}")) {
            tracing::warn!(target: "canvas_web", ?err, "тултип кнопки «Канвасы» не обновлён");
        }
    }
}

/// UR-003: сдвинуть DOM-хром из-под углов GPU-панелей во всю высоту.
/// `left` — открыт левый док палитры (Ctrl+P): `#author-bar` уезжает
/// вправо от дока; `right` — открыта агент-панель (Ctrl+I): `#w6-toolbar`
/// уезжает влево от панели. Канвас — GPU-UI без z-index, DOM-бары
/// (z-index: 10) всегда выше: прежде они перекрывали шапки панелей
/// (у агент-панели — вместе с кнопкой ✕). Синхронизация — классы на
/// `body` (`cd-panel-left`/`cd-panel-right`); CSS в index.html — сдвиги
/// и transition. Идемпотентно: сет-методы classList не дублируют классы.
/// Вызывается после каждого события цикла (TourAwareApp, паттерн FR-095).
pub(crate) fn set_panel_overlap(left: bool, right: bool) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(body) = document.body() else {
        return;
    };
    let class_list = body.class_list();
    let sync = |class: &str, on: bool| {
        if on {
            class_list.add_1(class).ok();
        } else {
            class_list.remove_1(class).ok();
        }
    };
    sync("cd-panel-left", left);
    sync("cd-panel-right", right);
}
