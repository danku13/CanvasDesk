//! Мост текстового ввода DOM → App (wasm-аудит 2026-09-25).
//!
//! Проблема: winit-web слушает ТОЛЬКО `keydown`/`keyup`
//! (`web_sys::canvas::on_keyboard_press`). Chromium ввод кириллицы
//! (Playwright `keyboard.type()` и реальная IME/раскладка без латинского
//! key-маппинга) доставляет через `insertText` БЕЗ keydown — winit-событий
//! не возникает вовсе, и кириллица не вводилась ни в поиск, ни в редактор
//! заметки (латиница шла keydown'ом и работала — отсюда «половинчатость»
//! бага на скриншотах 12b/34).
//!
//! Решение: слушать `beforeinput` на document; `inputType == "insertText"`
//! с непустым `data` → `AppEvent::ImeCommit(data)` → общий маршрут
//! `App::insert_committed_text` (редактор → поиск → explain).
//!
//! Дедупликация: обычное нажатие латинской клавиши порождает И keydown
//! (ввод уже сделал App), И `beforeinput`-insertText — повторный ввод
//! задвоил бы символ. Перехватчик keydown (capture, до winit) запоминает
//! момент последнего одиночного символа; `beforeinput` в пределах
//! [`DEDUP_WINDOW_MS`] после него глотается. IME-композиция и CDP
//! insertText keydown'а не имеют — проходят.
//!
//! FR-095 (мобильный web): виртуальная клавиатура. Канвас не editable —
//! мобильный браузер не поднимает клавиатуру (на десктопе клавиатура
//! физическая, проблемы нет). Решение — скрытый `<input>`-шим:
//! - создаётся из Rust при [`install`] (1×1, opacity 0, fixed, за
//!   пределами видимости, `pointer-events: none` — клики мимо него;
//!   фокусируется только программно);
//! - фокус управляется состоянием App: [`set_text_input_active`] зовётся
//!   TourAwareApp после каждого события цикла ([`App::text_input_active`])
//!   → true — `focus({preventScroll: true})`, false — `blur()`. Android
//!   поднимает клавиатуру на программный фокус; iOS Safari требует жеста —
//!   поэтому ДОПОЛНИТЕЛЬНО capture-фаза `pointerdown`/`touchstart` на
//!   канвасе (см. ниже) фокусирует шим СИНХРОННО в обработчике жеста;
//! - пока шим в фокусе, winit (слушающий keydown на канвасе) клавиатуру
//!   не видит: события keydown/keyup таргетятся в `<input>`, а не в
//!   потомков канваса. Форвардер (document, capture) пересоздаёт
//!   `KeyboardEvent` с теми же полями и dispatch'ит на канвас — winit
//!   получает события как обычно (синтетическое событие с `bubbles: true`
//!   всплывает и до дедуп-листенера, поэтому дедуп не ломается);
//! - гейт [`crate::touch_web::coarse_pointer`]: шим активируется только на
//!   coarse-указателях (телефон/планшет) — десктопная web-сборка ведёт себя
//!   бит-в-бит как прежде (шим никогда не получает фокус).
//!
//! Замечание о «ненастоящем» вводе: шиму ничего не делегируется — его
//! значение никогда не читается, накопленный текст очищается при blur.
//! Ввод идёт прежними маршрутами: keydown (форвардер) → winit → App и
//! beforeinput → ImeCommit (дедуп защищает от задвоения).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
// std::time::Instant на wasm32-unknown-unknown паникует — проектный alias
// (canvas_core::time::Instant = web_time::Instant, performance.now()) —
// урок первого прогона моста: паника «time not implemented» валила rAF.
use canvas_core::time::Instant;

use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use winit::event_loop::EventLoopProxy;

use canvas_app::app::AppEvent;

/// Окно дедупликации «keydown уже доставил этот символ», мс.
const DEDUP_WINDOW_MS: u128 = 120;

thread_local! {
    /// Скрытый input-шим (FR-095). None — шим ещё не создан (установка
    /// не удалась) либо создавался до этого вызова процесса.
    static SHIM: RefCell<Option<web_sys::HtmlInputElement>> = const { RefCell::new(None) };
    /// Последнее применённое состояние текстового режима (для переходов).
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
}

/// FR-095: применить состояние текстового ввода App («должна ли быть
/// поднята виртуальная клавиатура»). Вызывается TourAwareApp после каждого
/// события цикла; переход false→true фокусирует шим (программный фокус —
/// Android поднимает клавиатуру; iOS — см. capture-фокус по жесту ниже),
/// true→false — blur (клавиатура прячется). Гейт coarse-указателя: на
/// десктопе состояние игнорируется (шим не фокусируется вовсе —
/// поведение прежнее).
pub(crate) fn set_text_input_active(active: bool) {
    // FR-095: гейт coarse-указателя — на десктопе шим не фокусируется
    // (поведение прежнее). Источник флага — matchMedia в touch_platform.
    if !canvas_core::web_bridge::pointer_coarse() {
        return;
    }
    if ACTIVE.with(|cell| cell.replace(active)) == active {
        return;
    }
    SHIM.with(|cell| {
        let borrow = cell.borrow();
        let Some(shim) = borrow.as_ref() else {
            return;
        };
        if active {
            focus_prevent_scroll(shim);
        } else {
            // Накопленный текст сбрасываем: невидимый шим не должен
            // нести в себе композицию/значение между сессиями ввода.
            shim.set_value("");
            let _ = shim.blur();
        }
    });
}

/// `element.focus({ preventScroll: true })` (web-sys не экспортирует
/// FocusOptions — вызов отражением). Ошибка молча: без options фокус
/// всё равно быстрее следующего кадра, а скролл-прыжок гасится тем,
/// что шим фиксирован и страница не скроллится (dvh-вёрстка CR-014).
fn focus_prevent_scroll(element: &web_sys::HtmlInputElement) {
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&options, &"preventScroll".into(), &true.into());
    // focus(options) — только отражением: web-sys не экспортирует
    // FocusOptions. Ошибка молча: без options фокус всё равно быстрее
    // следующего кадра, а скролл-прыжок гасится тем, что шим фиксирован
    // и страница не скроллится (dvh-вёрстка CR-014).
    let focus_fn = js_sys::Reflect::get(element, &"focus".into())
        .ok()
        .and_then(|focus| focus.dyn_into::<js_sys::Function>().ok());
    match focus_fn {
        Some(focus) => {
            let _ = focus.call1(element, &options);
        }
        None => {
            let _ = element.focus();
        }
    }
}

/// Канвас приложения (единственный `<canvas>` в документе — его создаёт
/// winit и вставляет renderer_launch). Разрешение на каждый вызов: шим
/// устанавливается до вставки канваса в DOM, кэшировать нечего.
pub(crate) fn canvas_target(document: &web_sys::Document) -> Option<web_sys::Element> {
    document.query_selector("canvas").ok().flatten()
}

/// Установить DOM-листенеры `beforeinput`/`keydown` на document (вызывается
/// после построения event loop — текст шлётся в proxy). FR-095: там же
/// создаётся скрытый input-шим и ставятся его листенеры (форвардер
/// клавиатуры, capture-фокус по жесту, выход из текстового режима).
pub(crate) fn install(proxy: EventLoopProxy<AppEvent>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    // FR-095: клавиатура меняет видимый вьюпорт — App пангует камеру
    // редактора (VisualViewport { bottom_inset }). Прокси клонируется до
    // переезда в beforeinput-замыкание ниже.
    install_visual_viewport(&window, proxy.clone());
    let document = window.document();
    let target = window.unchecked_ref::<web_sys::EventTarget>();

    // FR-100: последний известный набор модификаторов — из DOM
    // keydown/keyup (capture на document — раньше listener'а winit на
    // канвасе, и раньше форвардера шима ниже) в App
    // (AppEvent::KeyboardModifiers). Компенсация дефектов winit-web:
    // (а) KeyboardInput уходит в App РАНЬШЕ ModifiersChanged — первая
    // Ctrl-комбинация после смены фокуса видела старый набор и печатала
    // символ; (б) blur сбрасывает набор winit в пустой. Здесь набор
    // доставляется ПЕРЕД winit-батчем того же нажатия, а на blur НЕ
    // сбрасывается — «залипание» гасится следующим keydown/keyup.
    // Только trusted-события: синтетика форвардера (isTrusted=false)
    // повторяет флаги оригинала — дубль не нужен.
    if let Some(document) = document.as_ref() {
        let modifiers_proxy = proxy.clone();
        let modifiers: Closure<dyn FnMut(web_sys::KeyboardEvent)> =
            Closure::new(move |event: web_sys::KeyboardEvent| {
                if !event.is_trusted() {
                    return;
                }
                let _ = modifiers_proxy.send_event(AppEvent::KeyboardModifiers {
                    control: event.ctrl_key(),
                    shift: event.shift_key(),
                    alt: event.alt_key(),
                    meta: event.meta_key(),
                });
            });
        let cb = modifiers.as_ref().unchecked_ref();
        for type_ in ["keydown", "keyup"] {
            let _ = document.add_event_listener_with_callback_and_bool(type_, cb, true);
        }
        // Слушатель живёт весь процесс страницы — утечка осознана (как у
        // остальных install* canvas-web).
        std::mem::forget(modifiers);
    }

    // Момент последнего одиночного символьного keydown (для дедупликации).
    let last_symbol_keydown: Rc<RefCell<Option<Instant>>> = Rc::new(RefCell::new(None));
    {
        let last = Rc::clone(&last_symbol_keydown);
        let keydown = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
            move |event: web_sys::KeyboardEvent| {
                // Одиночный текстовый символ без композиции — его уже обработает
                // winit (и приложение) как Key::Character.
                let is_symbol = event.key().chars().count() == 1 && !event.is_composing();
                if is_symbol {
                    *last.borrow_mut() = Some(Instant::now());
                }
            },
        );
        let _ =
            target.add_event_listener_with_callback("keydown", keydown.as_ref().unchecked_ref());
        // Слушатель живёт весь процесс страницы — утечка осознана (как у
        // остальных install* canvas-web).
        std::mem::forget(keydown);
    }

    let beforeinput = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        // web-sys этой версии не экспортирует InputEvent — читаем
        // `inputType`/`data` отражением (спека Input Event Level 2).
        let get = |key: &str| js_sys::Reflect::get(event.as_ref(), &key.into()).ok();
        let input_type = get("inputType")
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        // Целевые пути: обычный посимвольный ввод и финал IME-композиции.
        if input_type != "insertText" && input_type != "insertCompositionText" {
            return;
        }
        let data = get("data").and_then(|v| v.as_string()).unwrap_or_default();
        if data.is_empty() {
            return;
        }
        // Дедупликация: keydown этого символа уже ушёл в winit/App.
        if let Some(t) = *last_symbol_keydown.borrow() {
            if t.elapsed().as_millis() <= DEDUP_WINDOW_MS {
                return;
            }
        }
        let _ = proxy.send_event(AppEvent::ImeCommit(data));
    });
    if let Some(document) = document {
        let _ = document
            .add_event_listener_with_callback("beforeinput", beforeinput.as_ref().unchecked_ref());
        std::mem::forget(beforeinput);
        install_shim(&window, &document);
    }
}

/// FR-095: клавиатура меняет видимый вьюпорт (visualViewport) — низ
/// страницы перекрывается. App получает
/// `AppEvent::VisualViewport { bottom_inset }` и пангует камеру
/// редактора, чтобы курсор остался видим (`on_visual_viewport`;
/// дедуп по полю `visual_viewport_inset` — сдвиг только при изменении).
/// Инсет = innerHeight − (vv.height + vv.offsetTop), кламп ≥ 0
/// (клавиатура снизу даёт положительный инсет; полоска браузера учтена
/// в offsetTop). Источники событий — resize и scroll вьюпорта (клавиатура
/// поднимается/опускается, страница подскролливается сфокусированным шимом).
fn install_visual_viewport(window: &web_sys::Window, proxy: EventLoopProxy<AppEvent>) {
    let Some(vv) = window.visual_viewport() else {
        tracing::warn!(target: "canvas_web", "visualViewport недоступен — сдвиг камеры редактора над клавиатурой отключён");
        return;
    };
    let on_change: Closure<dyn FnMut()> = Closure::new(move || {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(vv) = window.visual_viewport() else {
            return;
        };
        let inner_h = window
            .inner_height()
            .ok()
            .and_then(|value| value.as_f64())
            .unwrap_or(0.0);
        let bottom_inset = (inner_h - (vv.height() + vv.offset_top())).max(0.0) as f32;
        let _ = proxy.send_event(AppEvent::VisualViewport { bottom_inset });
    });
    let callback = on_change.as_ref().unchecked_ref::<js_sys::Function>();
    let _ = vv.add_event_listener_with_callback("resize", callback);
    let _ = vv.add_event_listener_with_callback("scroll", callback);
    // Листенеры живёт весь процесс страницы — утечка осознана (как у
    // остальных install* canvas-web).
    std::mem::forget(on_change);
}

/// FR-095: создание скрытого input-шима и его листенеров.
///
/// Атрибуты (виртуальная клавиатура должна вести себя как обычный ввод
/// текста, но элемент не должен быть виден/доступен указателю):
/// - `1×1, opacity 0, fixed, left/top 0` — вне визуала и вьюпорта не
///   покидает (preventScroll-приёмка не двигает страницу);
/// - `pointer-events: none` — пользователь по нему не попадает;
/// - `autocapitalize/autocomplete off, spellcheck false` — мобильные
///   подсказки/автозамены не искажают ввод (App-редактор делает свой
///   регистр/проверки);
/// - `enterkeyhint="enter"` — на мобильной клавиатуре ⏎ (редактор заметки
///   многострочный; Enter форвардится и в поиск — «перейти»).
fn install_shim(_window: &web_sys::Window, document: &web_sys::Document) {
    let element = match document.create_element("input") {
        Ok(element) => element,
        Err(err) => {
            tracing::warn!(target: "canvas_web", ?err, "input-шим не создан — виртуальная клавиатура недоступна");
            return;
        }
    };
    let input: web_sys::HtmlInputElement = match element.dyn_into() {
        Ok(input) => input,
        Err(_) => {
            tracing::warn!(target: "canvas_web", "input-шим не приведён к HtmlInputElement");
            return;
        }
    };
    let _ = input.set_attribute("autocapitalize", "off");
    let _ = input.set_attribute("autocomplete", "off");
    let _ = input.set_attribute("spellcheck", "false");
    let _ = input.set_attribute("enterkeyhint", "enter");
    let style = "position:fixed;left:0;top:0;width:1px;height:1px;opacity:0;\
                 border:none;padding:0;margin:0;z-index:-1;pointer-events:none;";
    let _ = input.set_attribute("style", style);
    if let Some(body) = document.body() {
        if let Err(err) = body.append_child(&input) {
            tracing::warn!(target: "canvas_web", ?err, "input-шим не вставлен в DOM");
            return;
        }
    } else {
        tracing::warn!(target: "canvas_web", "body недоступен — input-шим не вставлен");
        return;
    }
    SHIM.with(|cell| *cell.borrow_mut() = Some(input.clone()));
    tracing::debug!(target: "canvas_web", "скрытый input-шим установлен (FR-095)");

    // Форвардер клавиатуры: пока шим в фокусе, keydown/keyup таргетятся в
    // него и до канваса (winit) не доходят. Пересоздаём событие с теми же
    // полями и dispatch'им на канвас; bubbles:true — синтетика всплывает до
    // дедуп-листенера window (задвоения insertText нет). Ограничение
    // «target — шим» исключает двойную доставку, когда фокус на канвасе
    // (родные события уже доходят до winit сами).
    let forward: Closure<dyn FnMut(web_sys::KeyboardEvent)> =
        Closure::new(move |event: web_sys::KeyboardEvent| {
            let is_shim = SHIM.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .is_some_and(|shim| event.target().is_some_and(|t| t == *shim.as_ref()))
            });
            if !is_shim {
                return;
            }
            // FR-100: chord'ы (Ctrl/Meta/Alt) гасят default-действие браузера
            // (подход шима Ctrl+P, index.html): иначе браузер параллельно с
            // App исполняет акселератор — Ctrl+A делал select-all шима.
            // preventDefault не мешает dispatch_event ниже — синтетика
            // доходит до winit. Обычные клавиши не гасим: шим копит текст
            // (значение никогда не читается, но input-семантика нужна IME).
            if event.ctrl_key() || event.meta_key() || event.alt_key() {
                event.prevent_default();
            }
            let Some(document) = web_sys::window().and_then(|window| window.document()) else {
                return;
            };
            let Some(canvas) = canvas_target(&document) else {
                return;
            };
            let init = web_sys::KeyboardEventInit::new();
            init.set_key(&event.key());
            init.set_code(&event.code());
            init.set_location(event.location());
            init.set_ctrl_key(event.ctrl_key());
            init.set_alt_key(event.alt_key());
            init.set_shift_key(event.shift_key());
            init.set_meta_key(event.meta_key());
            init.set_repeat(event.repeat());
            init.set_is_composing(event.is_composing());
            init.set_bubbles(true);
            init.set_cancelable(true);
            let synthetic =
                web_sys::KeyboardEvent::new_with_keyboard_event_init_dict(&event.type_(), &init)
                    .expect("KeyboardEvent синтезируется из валидного KeyboardEvent");
            // Синхронная доставка: winit-листенер канваса получит событие в
            // этом же тике (до rAF-обработки очереди — как родные keydown).
            let _ = canvas.dispatch_event(&synthetic);
        });
    let _ = document.add_event_listener_with_callback_and_bool(
        "keydown",
        forward.as_ref().unchecked_ref(),
        true,
    );
    let _ = document.add_event_listener_with_callback_and_bool(
        "keyup",
        forward.as_ref().unchecked_ref(),
        true,
    );
    std::mem::forget(forward);

    // Capture-фокус по жесту (iOS Safari: программный focus() вне
    // обработчика жеста клавиатуру не поднимает). pointerdown/touchstart на
    // канвасе в фазе захвата — РАНЬШЕ листенеров winit: если текстовый режим
    // уже активен, шим фокусируется синхронно, клавиатура остаётся/поднимается.
    // Тап, только ОТКРЫВАЮЩИЙ редактор, фокус получает от
    // set_text_input_active (программный путь — Android); iOS-пользователь
    // в этом случае добирает клавиатуру следующим тапом (известное
    // ограничение, зафиксировано в FR-095).
    let gesture_focus: Closure<dyn FnMut(web_sys::Event)> =
        Closure::new(move |event: web_sys::Event| {
            if !ACTIVE.with(|cell| cell.get()) {
                return;
            }
            let on_canvas = web_sys::window()
                .and_then(|window| window.document())
                .is_some_and(|document| {
                    canvas_target(&document)
                        .is_some_and(|canvas| event.target().is_some_and(|t| t == *canvas.as_ref()))
                });
            if !on_canvas {
                return;
            }
            SHIM.with(|cell| {
                if let Some(shim) = cell.borrow().as_ref() {
                    focus_prevent_scroll(shim);
                }
            });
        });
    let _ = document.add_event_listener_with_callback_and_bool(
        "pointerdown",
        gesture_focus.as_ref().unchecked_ref(),
        true,
    );
    let _ = document.add_event_listener_with_callback_and_bool(
        "touchstart",
        gesture_focus.as_ref().unchecked_ref(),
        true,
    );
    std::mem::forget(gesture_focus);
}
