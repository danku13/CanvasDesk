//! WebView2-хост виджетов (T20-E, план M5 §4.1/§4.4/§4.5). Windows-only.
//!
//! Контракт: все методы вызываются ТОЛЬКО из потока event loop (UI-поток
//! приложения) — WebView2-колбэки приходят в накачку сообщений того же
//! потока. Жизненным циклом управляет менеджер (canvas-app): он строит
//! `FrameApplication` из LOD-решений, хост лишь исполняет.
//!
//! Архитектура airspace: для каждого live-инстанса создаётся дочерний HWND
//! (WS_CHILD) поверх области контента ноды; контроллер WebView2 родится в
//! нём. Это даёт контроль позиции (SetWindowPos), скругления (SetWindowRgn)
//! и изоляции ввода (клик по контенту — виджету, хром ноды — канвасу).
//!
//! SAFETY-дисциплина: заимствования `RefCell` не держатся поверх вызовов,
//! способных синхронно вызвать колбэк (CreateCoreWebView2Controller может
//! завершиться синхронно при горячем окружении).

use crate::bridge::{HostToWidget, Reply};
pub use crate::host_types::{FrameApplication, HostError, LiveRequest, WidgetEventSender};
use crate::layout::{zoom_factor, PhysRect};
use crate::permissions::Permissions;
use crate::snapshot::decode_png_rgba;
use crate::WidgetEvent;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{
    AcceleratorKeyPressedEventHandler, CapturePreviewCompletedHandler,
    CoreWebView2EnvironmentOptions, CreateCoreWebView2ControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler, NavigationStartingEventHandler,
    NewWindowRequestedEventHandler, ProcessFailedEventHandler, TrySuspendCompletedHandler,
    WebMessageReceivedEventHandler, WebResourceRequestedEventHandler,
};

use windows::core::{w, Interface, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{HGLOBAL, HINSTANCE, HWND, RECT};
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
use windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;
use windows::Win32::System::Com::{
    CoInitializeEx, IStream, COINIT_APARTMENTTHREADED, STREAM_SEEK_SET,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{SetFocus, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, SetWindowPos, ShowWindow,
    SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE, WINDOW_STYLE, WNDCLASSW,
    WS_CHILD,
};

/// Один live-инстанс: дочерний HWND + контроллер WebView2.
struct Instance {
    child: HWND,
    controller: ICoreWebView2Controller,
    webview: ICoreWebView2,
    /// ICoreWebView2_3 (TrySuspend/Resume/VirtualHost) — кэш cast'а.
    webview3: ICoreWebView2_3,
    bounds: PhysRect,
    corner: i32,
    zoom_factor: f32,
    visible: bool,
    suspended: bool,
}

/// Внутреннее состояние (UI-поток).
struct HostState {
    parent: HWND,
    sender: WidgetEventSender,
    env: Option<ICoreWebView2Environment>,
    env_requested: bool,
    env_failed: bool,
    user_data_folder: PathBuf,
    /// Запросы, ждущие среду.
    queued: Vec<LiveRequest>,
    instances: HashMap<String, Instance>,
    /// Контроллеры в процессе создания.
    pending: HashSet<String>,
    /// Захваты снапшотов в полёте (не более одного на инстанс).
    captures: HashSet<String>,
    failed: HashSet<String>,
}

/// Публичный хост (клонируем через Rc — приложение владеет одним).
pub struct WidgetHost {
    state: Rc<RefCell<HostState>>,
}

impl WidgetHost {
    /// Создание хоста. `parent_hwnd` — HWND окна канваса (из
    /// raw-window-handle, `NonZeroIsize::get()`), COM инициализируется как
    /// STA (безопасно при повторной инициализации — S_FALSE).
    pub fn new(parent_hwnd: isize, sender: WidgetEventSender) -> Result<Self, HostError> {
        // SAFETY: CoInitializeEx на UI-потоке; S_FALSE (уже инициализировано)
        // — успех (HRESULT ok() это учитывает), ошибку логируем, но продолжаем
        if let Err(e) = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok() {
            tracing::warn!(error = %e, "CoInitializeEx STA: уже инициализировано или отказ");
        }
        register_widget_window_class();
        Ok(Self {
            state: Rc::new(RefCell::new(HostState {
                // SAFETY: HWND от raw-window-handle — валидный указатель окна
                parent: HWND(parent_hwnd as *mut core::ffi::c_void),
                sender,
                env: None,
                env_requested: false,
                env_failed: false,
                user_data_folder: PathBuf::new(),
                queued: Vec::new(),
                instances: HashMap::new(),
                pending: HashSet::new(),
                captures: HashSet::new(),
                failed: HashSet::new(),
            })),
        })
    }

    /// Корень данных WebView2 (user-data-folder, один на приложение —
    /// SPEC §7.6). Вызывается до первого `apply`.
    pub fn set_user_data_folder(&self, folder: &Path) {
        self.state.borrow_mut().user_data_folder = folder.to_owned();
    }

    /// Асинхронное создание среды (идемпотентно). По готовности —
    /// `WidgetEvent::EnvironmentReady` и разгрузка очереди live-запросов.
    pub fn ensure_environment(&self) {
        let (requested, has_env, failed) = {
            let s = self.state.borrow();
            (s.env_requested, s.env.is_some(), s.env_failed)
        };
        if requested || has_env || failed {
            return;
        }
        let user_data = self.state.borrow().user_data_folder.clone();
        if user_data.as_os_str().is_empty() {
            return;
        }
        if let Err(e) = std::fs::create_dir_all(&user_data) {
            tracing::warn!(?user_data, error = %e, "user-data-folder не создан");
        }
        self.state.borrow_mut().env_requested = true;

        let weak = Rc::downgrade(&self.state);
        let options: ICoreWebView2EnvironmentOptions =
            CoreWebView2EnvironmentOptions::default().into();
        let user_data_w: HSTRING = HSTRING::from(user_data.as_os_str());
        // SAFETY: хендлер живёт до завершения асинхронного вызова (COM
        // держит ссылку), замыкание на UI-потоке
        let handler = CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
            move |error_code, environment| {
                error_code?;
                let env = environment.ok_or_else(|| {
                    windows::core::Error::from_hresult(windows::Win32::Foundation::E_POINTER)
                })?;
                let Some(state) = weak.upgrade() else {
                    return Ok(());
                };
                let queued: Vec<LiveRequest> = {
                    let mut s = state.borrow_mut();
                    s.env = Some(env);
                    std::mem::take(&mut s.queued)
                };
                (state.borrow().sender)(WidgetEvent::EnvironmentReady { ok: true });
                // Очередь live-запросов, копившаяся до среды
                for req in queued {
                    WidgetHost {
                        state: state.clone(),
                    }
                    .create_or_update(req);
                }
                Ok(())
            },
        ));
        // SAFETY: создание среды; колбэк может прийти синхронно — заимствований нет
        unsafe {
            CreateCoreWebView2EnvironmentWithOptions(
                PCWSTR::null(),
                PCWSTR::from_raw(user_data_w.as_ptr()),
                &options,
                &handler,
            )
        }
        .map_err(|e| {
            tracing::error!(error = %e, "создание среды WebView2 не запущено");
            self.state.borrow_mut().env_failed = true;
            (self.state.borrow().sender)(WidgetEvent::EnvironmentReady { ok: false });
            e
        })
        .ok();
        // Ошибка ЗАПУСКА (не завершения) сразу гасит попытку; ошибка
        // завершения придёт EnvironmentReady{ok:false} из колбэка
    }

    /// Применить кадр (менеджер вызывает в RedrawRequested до render).
    pub fn apply(&self, frame: &FrameApplication) {
        for node_id in &frame.destroy {
            self.destroy_instance(node_id);
        }
        for node_id in &frame.final_capture {
            self.capture(node_id);
        }
        for node_id in &frame.hide {
            self.hide_instance(node_id);
        }
        for req in frame.live.clone() {
            self.create_or_update(req);
        }
        for node_id in &frame.refresh {
            self.refresh_snapshot(node_id);
        }
    }

    /// Прямоугольник live-инстанса обновился без полного live-цикла
    /// (drag/resize ноды) — фактически покрывается apply(live), оставлено
    /// для ясности API.
    pub fn update_geometry(&self, node_id: &str, rect: PhysRect, corner: i32, zoom: f32) {
        let mut state = self.state.borrow_mut();
        if let Some(inst) = state.instances.get_mut(node_id) {
            update_instance_geometry(inst, rect, corner, zoom);
        }
    }

    /// Спрятать все (окно свернулось/скрылось) — дёшево, без снапшотов.
    pub fn hide_all(&self) {
        let mut state = self.state.borrow_mut();
        let ids: Vec<String> = state.instances.keys().cloned().collect();
        for id in ids {
            state.hide_instance(&id);
        }
    }

    /// Скрыть и приостановить инстанс.
    fn hide_instance(&self, node_id: &str) {
        let mut state = self.state.borrow_mut();
        state.hide_instance(node_id);
    }

    /// Уничтожить инстанс (Close контроллера + DestroyWindow).
    fn destroy_instance(&self, node_id: &str) {
        let mut state = self.state.borrow_mut();
        state.destroy_instance(node_id);
    }

    /// Отправить сообщение виджету (host → widget).
    pub fn post_message(&self, node_id: &str, message: &HostToWidget) {
        let webview = {
            let state = self.state.borrow();
            match state.instances.get(node_id) {
                Some(inst) => inst.webview.clone(),
                None => return,
            }
        };
        let json = HSTRING::from(message.to_json());
        // SAFETY: PostWebMessageAsJson — передача строки виджету
        unsafe { webview.PostWebMessageAsJson(PCWSTR::from_raw(json.as_ptr())) }
            .map_err(|e| tracing::warn!(node_id, error = %e, "postMessage не доставлен"))
            .ok();
    }

    /// Ответ на запрос виджета (readDir/stateGet/stateSet).
    pub fn reply(&self, node_id: &str, reply: &Reply) {
        self.post_raw(node_id, &reply.to_json());
    }

    /// Отправка сырого JSON виджету (общая ветка post/reply).
    fn post_raw(&self, node_id: &str, json: &str) {
        let webview = {
            let state = self.state.borrow();
            match state.instances.get(node_id) {
                Some(inst) => inst.webview.clone(),
                None => return,
            }
        };
        let wide = HSTRING::from(json);
        // SAFETY: см. post_message
        unsafe { webview.PostWebMessageAsJson(PCWSTR::from_raw(wide.as_ptr())) }
            .map_err(|e| tracing::warn!(node_id, error = %e, "reply не доставлен"))
            .ok();
    }

    /// Снять снапшот (CapturePreview → PNG → RGBA → SnapshotReady).
    /// Не более одного захвата на инстанс одновременно.
    pub fn capture(&self, node_id: &str) {
        let webview = {
            let mut state = self.state.borrow_mut();
            if state.captures.contains(node_id) {
                return;
            }
            let Some(webview) = state.instances.get(node_id).map(|i| i.webview.clone()) else {
                return;
            };
            state.captures.insert(node_id.to_owned());
            webview
        };
        self.capture_on(node_id, webview);
    }

    /// Освежить suspended-снапшот: Resume → capture → (в колбэке) TrySuspend.
    fn refresh_snapshot(&self, node_id: &str) {
        let webview3 = {
            let state = self.state.borrow();
            match state.instances.get(node_id) {
                Some(inst) => {
                    if !inst.suspended {
                        return; // живой — снапшот не нужен
                    }
                    inst.webview3.clone()
                }
                None => return,
            }
        };
        // SAFETY: Resume безопасен всегда
        if let Err(e) = unsafe { webview3.Resume() } {
            tracing::warn!(node_id, error = %e, "Resume для refresh не удался");
        }
        self.capture(node_id);
    }

    fn capture_on(&self, node_id: &str, webview: ICoreWebView2) {
        // Поток в памяти: PNG пишется WebView2, читаем в колбэке
        // SAFETY: поток освобождается сам (fdeleteonrelease=true)
        let stream = match unsafe { CreateStreamOnHGlobal(HGLOBAL::default(), true) } {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(node_id, error = %e, "IStream не создан");
                self.state.borrow_mut().captures.remove(node_id);
                return;
            }
        };
        let weak = Rc::downgrade(&self.state);
        let node = node_id.to_owned();
        let stream_copy = stream.clone();
        // SAFETY: хендлер держится WebView2 до завершения захвата
        let handler = CapturePreviewCompletedHandler::create(Box::new(move |error_code| {
            error_code?;
            let bytes = read_stream(&stream_copy)?;
            let Some(state) = weak.upgrade() else {
                return Ok(());
            };
            {
                let mut s = state.borrow_mut();
                s.captures.remove(&node);
            }
            match decode_png_rgba(&bytes) {
                Ok(snapshot) => {
                    (state.borrow().sender)(WidgetEvent::SnapshotReady {
                        node_id: node.clone(),
                        snapshot,
                    });
                    // Инстанс уже скрыт — возвращаем в suspend
                    let re_suspend = {
                        let s = state.borrow();
                        s.instances
                            .get(&node)
                            .is_some_and(|i| i.suspended && !i.visible)
                    };
                    if re_suspend {
                        if let Some(inst) = state.borrow().instances.get(&node) {
                            // SAFETY: TrySuspend допустим только на скрытом окне — так и есть
                            let handler =
                                TrySuspendCompletedHandler::create(Box::new(|_, _| Ok(())));
                            let _ = unsafe { inst.webview3.TrySuspend(&handler) };
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(node = %node, error = %e, "снапшот не декодирован");
                }
            }
            Ok(())
        }));
        // SAFETY: асинхронный захват; PNG-формат
        unsafe {
            webview.CapturePreview(
                COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                &stream,
                &handler,
            )
        }
        .map_err(|e| {
            tracing::warn!(node_id, error = %e, "CapturePreview не запущен");
            self.state.borrow_mut().captures.remove(node_id);
        })
        .ok();
    }

    /// Фокус канвасу (Esc внутри виджета — SPEC §8).
    pub fn return_focus(&self) {
        let parent = self.state.borrow().parent;
        // SAFETY: SetFocus на родительское окно того же потока
        unsafe { SetFocus(Some(parent)) }
            .map_err(|e| tracing::warn!(error = %e, "SetFocus не удался"))
            .ok();
    }

    /// Создать (асинхронно) или обновить live-инстанс.
    fn create_or_update(&self, req: LiveRequest) {
        {
            let mut state = self.state.borrow_mut();
            if state.failed.contains(&req.node_id) || state.pending.contains(&req.node_id) {
                return;
            }
            if let Some(inst) = state.instances.get_mut(&req.node_id) {
                update_instance_geometry(inst, req.rect, req.corner, req.zoom);
                if !inst.visible {
                    show_instance(inst);
                }
                return;
            }
        }
        let state = self.state.borrow();
        let Some(env) = state.env.clone() else {
            drop(state);
            let mut s = self.state.borrow_mut();
            s.queued.retain(|r| r.node_id != req.node_id);
            s.queued.push(req);
            self.ensure_environment();
            return;
        };
        drop(state);
        self.spawn_controller(env, req);
    }

    /// Асинхронное создание контроллера в дочернем HWND.
    fn spawn_controller(&self, env: ICoreWebView2Environment, req: LiveRequest) {
        {
            let mut s = self.state.borrow_mut();
            s.pending.insert(req.node_id.clone());
        }
        // Дочерний HWND над областью контента (airspace, план M5 §4.11)
        // SAFETY: класс зарегистрирован; WS_CHILD — не активируется сам
        let child = match unsafe {
            CreateWindowExW(
                Default::default(),
                w!("CanvasDeskWidgetHost"),
                PCWSTR::null(),
                WINDOW_STYLE(WS_CHILD.0),
                req.rect.x,
                req.rect.y,
                req.rect.w.max(1),
                req.rect.h.max(1),
                Some(self.state.borrow().parent),
                None,
                GetModuleHandleW(None).ok().map(|m| HINSTANCE(m.0)),
                None,
            )
        } {
            Ok(hwnd) => hwnd,
            Err(e) => {
                tracing::error!(node_id = %req.node_id, error = %e, "child HWND не создан");
                let mut s = self.state.borrow_mut();
                s.pending.remove(&req.node_id);
                s.failed.insert(req.node_id.clone());
                (s.sender.clone())(WidgetEvent::ControllerReady {
                    node_id: req.node_id.clone(),
                    ok: false,
                });
                return;
            }
        };

        let weak = Rc::downgrade(&self.state);
        let node_id = req.node_id.clone();
        // Клон для ветки ошибки ЗА замыканием (замыкание забирает node_id)
        let node_id_err = req.node_id.clone();
        let virtual_host = req.manifest.virtual_host();
        let sender = self.state.borrow().sender.clone();

        // SAFETY: хендлер держится WebView2 до завершения создания
        let handler = CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
            move |error_code, controller| {
                error_code?;
                let Some(controller) = controller else {
                    return Err(windows::core::Error::from_hresult(
                        windows::Win32::Foundation::E_POINTER,
                    ));
                };
                let Some(state) = weak.upgrade() else {
                    return Ok(());
                };
                match build_instance(&state, &req, child, controller) {
                    Ok(()) => {
                        tracing::info!(node_id = %node_id, host = %virtual_host, "виджет live-контроллер готов");
                        sender(WidgetEvent::ControllerReady {
                            node_id: node_id.clone(),
                            ok: true,
                        });
                        Ok(())
                    }
                    Err(e) => {
                        tracing::error!(node_id = %node_id, error = %e, "инстанс виджета не собран");
                        sender(WidgetEvent::ControllerReady {
                            node_id: node_id.clone(),
                            ok: false,
                        });
                        // Чистим за собой
                        let mut s = state.borrow_mut();
                        s.pending.remove(&node_id);
                        s.failed.insert(node_id.clone());
                        // SAFETY: окно уже без контроллера
                        let _ = unsafe { DestroyWindow(child) };
                        Ok(())
                    }
                }
            },
        ));
        // SAFETY: асинхронное создание контроллера в child HWND; колбэк может
        // прийти синхронно — заимствований на стеке нет
        unsafe { env.CreateCoreWebView2Controller(child, &handler) }
            .map_err(|e| {
                tracing::error!(node_id = %node_id_err, error = %e, "CreateController не запущен");
                let mut s = self.state.borrow_mut();
                s.pending.remove(&node_id_err);
                s.failed.insert(node_id_err.clone());
                (s.sender.clone())(WidgetEvent::ControllerReady {
                    node_id: node_id_err,
                    ok: false,
                });
                // SAFETY: окно создано — освобождаем
                let _ = unsafe { DestroyWindow(child) };
            })
            .ok();
    }

    // -- Диагностика ------------------------------------------------------

    pub fn has_instance(&self, node_id: &str) -> bool {
        self.state.borrow().instances.contains_key(node_id)
    }

    pub fn is_live(&self, node_id: &str) -> bool {
        self.state
            .borrow()
            .instances
            .get(node_id)
            .is_some_and(|i| i.visible)
    }

    pub fn instance_count(&self) -> usize {
        self.state.borrow().instances.len()
    }

    pub fn environment_ready(&self) -> bool {
        self.state.borrow().env.is_some()
    }
}

impl HostState {
    fn hide_instance(&mut self, node_id: &str) {
        let Some(inst) = self.instances.get_mut(node_id) else {
            return;
        };
        if inst.visible {
            // SAFETY: скрытие окна-ребёнка; TrySuspend требует IsVisible=false
            let _ = unsafe { ShowWindow(inst.child, SW_HIDE) };
            let _ = unsafe { inst.controller.SetIsVisible(false) };
            let handler = TrySuspendCompletedHandler::create(Box::new(|_, _| Ok(())));
            let _ = unsafe { inst.webview3.TrySuspend(&handler) };
            inst.visible = false;
            inst.suspended = true;
        }
    }

    fn destroy_instance(&mut self, node_id: &str) {
        if let Some(inst) = self.instances.remove(node_id) {
            // SAFETY: Close освобождает WebView2; затем окно
            let _ = unsafe { inst.controller.Close() };
            let _ = unsafe { DestroyWindow(inst.child) };
            self.pending.remove(node_id);
            self.captures.remove(node_id);
        }
    }
}

/// Геометрия инстанса: позиция child-окна + bounds контроллера + зум.
fn update_instance_geometry(inst: &mut Instance, rect: PhysRect, corner: i32, zoom: f32) {
    let zf = zoom_factor(zoom);
    if inst.bounds != rect {
        // SAFETY: асинхронный перенос без активации и z-изменений
        unsafe {
            SetWindowPos(
                inst.child,
                None,
                rect.x,
                rect.y,
                rect.w.max(1),
                rect.h.max(1),
                SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE,
            )
        }
        .unwrap_or_else(|e| tracing::warn!(error = %e, "SetWindowPos виджета"));
        // SAFETY: bounds контроллера — относительно child (0,0,w,h)
        let _ = unsafe {
            inst.controller.SetBounds(RECT {
                left: 0,
                top: 0,
                right: rect.w.max(1),
                bottom: rect.h.max(1),
            })
        };
        if corner > 0 && corner != inst.corner {
            // SAFETY: регион после успеха принадлежит системе
            let rgn = unsafe {
                CreateRoundRectRgn(0, 0, rect.w.max(1) + 1, rect.h.max(1) + 1, corner, corner)
            };
            unsafe { SetWindowRgn(inst.child, Some(rgn), true) };
        }
        inst.corner = corner;
        inst.bounds = rect;
    }
    if (inst.zoom_factor - zf).abs() > f32::EPSILON {
        // SAFETY: масштаб контента = зум канваса (план П3)
        let _ = unsafe { inst.controller.SetZoomFactor(zf as f64) };
        inst.zoom_factor = zf;
    }
    // WebView2 нужно уведомлять о переносе родителя (актуально после
    // перемещения окна приложения целиком)
    // SAFETY: дешёвый no-op в большинстве кадров
    let _ = unsafe { inst.controller.NotifyParentWindowPositionChanged() };
}

/// Показать скрытый инстанс (возобновить).
fn show_instance(inst: &mut Instance) {
    if inst.suspended {
        // SAFETY: Resume безопасен всегда
        let _ = unsafe { inst.webview3.Resume() };
        inst.suspended = false;
    }
    // SAFETY: показ без активации (фокус не крадём)
    let _ = unsafe { ShowWindow(inst.child, SW_SHOWNOACTIVATE) };
    let _ = unsafe { inst.controller.SetIsVisible(true) };
    inst.visible = true;
}

/// Сборка инстанса из готовного контроллера: события, origin, навигация.
/// Всё производное (permissions/virtual_host/origin) выводится из запроса.
fn build_instance(
    state: &Rc<RefCell<HostState>>,
    req: &LiveRequest,
    child: HWND,
    controller: ICoreWebView2Controller,
) -> windows::core::Result<()> {
    let node_id = req.node_id.as_str();
    let manifest = &req.manifest;
    let permissions = Permissions::new(manifest.permissions.clone());
    let virtual_host = manifest.virtual_host();
    let origin_prefix = format!("https://{virtual_host}/");
    let package_dir = req.package_dir.as_path();
    let grants_network = permissions.grants_network();
    let (rect, corner, zoom) = (req.rect, req.corner, req.zoom);
    // SAFETY: доступ к CoreWebView2 контроллера
    let webview = unsafe { controller.CoreWebView2() }?;
    // SAFETY: cast к ICoreWebView2_3 (TrySuspend/Resume/VirtualHostMapping)
    let webview3: ICoreWebView2_3 = webview.cast()?;

    let folder_w: HSTRING = HSTRING::from(package_dir.as_os_str());
    // SAFETY: виртуальный origin пакета (SPEC §7.6): навигация только сюда
    unsafe {
        webview3.SetVirtualHostNameToFolderMapping(
            PCWSTR::from_raw(HSTRING::from(virtual_host).as_ptr()),
            PCWSTR::from_raw(folder_w.as_ptr()),
            COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_ALLOW,
        )
    }?;
    let _ = origin_prefix;

    // --- События ------------------------------------------------------------

    // Мост: widget → host (валидация схемы — в parse_widget_message,
    // enforcement permissions — в менеджере приложения)
    let weak = Rc::downgrade(state);
    let node = node_id.to_owned();
    // SAFETY: хендлер регистрируется и живёт вместе с webview
    let handler = WebMessageReceivedEventHandler::create(Box::new(move |_wv, args| {
        let Some(args) = args else { return Ok(()) };
        let mut raw = PWSTR::null();
        // SAFETY: out-параметр PWSTR освобождаем ниже (CoTaskMemFree)
        unsafe { args.WebMessageAsJson(&mut raw) }?;
        let text = pwstr_to_string(raw);
        if let Some(state) = weak.upgrade() {
            let sender = state.borrow().sender.clone();
            match crate::bridge::parse_widget_message(&text) {
                Ok(parsed) => {
                    let (call, id) = match parsed {
                        crate::bridge::Parsed::Notification(call) => (call, None),
                        // readDir/stateGet/stateSet с id — передаём наверх:
                        // reply уйдёт через WidgetHost::reply с тем же id
                        crate::bridge::Parsed::Request { call, id } => (call, Some(id)),
                    };
                    sender(WidgetEvent::Message {
                        node_id: node.clone(),
                        message: call,
                        id,
                    });
                }
                Err(e) => {
                    // AGENTS: невалидное сообщение = drop + warn, не паника
                    tracing::warn!(node_id = %node, error = %e, "сообщение виджета отброшено");
                }
            }
        }
        Ok(())
    }));
    let mut token = 0i64;
    // SAFETY: регистрация обработчика
    unsafe { webview.add_WebMessageReceived(&handler, &mut token)? };

    // Навигация вне пакета запрещена (SPEC §7.6)
    let allowed = origin_prefix.to_owned();
    let node = node_id.to_owned();
    // SAFETY: см. выше
    let handler = NavigationStartingEventHandler::create(Box::new(move |_wv, args| {
        let Some(args) = args else { return Ok(()) };
        let mut raw = PWSTR::null();
        // SAFETY: out-параметр
        unsafe { args.Uri(&mut raw) }?;
        let uri = pwstr_to_string(raw);
        if !uri.starts_with(&allowed) {
            tracing::warn!(node_id = %node, %uri, "навигация вне пакета отменена");
            // SAFETY: отмена навигации
            unsafe { args.SetCancel(true) }?;
        }
        Ok(())
    }));
    // SAFETY: регистрация
    unsafe { webview.add_NavigationStarting(&handler, &mut token)? };

    // Новые окна запрещены
    // SAFETY: см. выше
    let handler = NewWindowRequestedEventHandler::create(Box::new(move |_wv, args| {
        if let Some(args) = args {
            // SAFETY: гасим popup
            unsafe { args.SetHandled(true) }?;
        }
        Ok(())
    }));
    // SAFETY: регистрация
    unsafe { webview.add_NewWindowRequested(&handler, &mut token)? };

    // Крэш процесса виджета: лог + уведомление (менеджер пересоздаст)
    let weak = Rc::downgrade(state);
    let node = node_id.to_owned();
    // SAFETY: см. выше
    let handler = ProcessFailedEventHandler::create(Box::new(move |_wv, args| {
        if args.is_some() {
            tracing::warn!(node_id = %node, "процесс виджета упал (ProcessFailed)");
        }
        if let Some(state) = weak.upgrade() {
            let sender = state.borrow().sender.clone();
            sender(WidgetEvent::ControllerReady {
                node_id: node.clone(),
                ok: false,
            });
        }
        Ok(())
    }));
    // SAFETY: регистрация
    unsafe { webview.add_ProcessFailed(&handler, &mut token)? };

    // Сетевой фильтр: всё, что не origin пакета — только с permission network
    let env = { state.borrow().env.clone() };
    let allowed = origin_prefix.to_owned();
    let node = node_id.to_owned();
    // SAFETY: фильтр на ВСЕ контексты ресурсов
    unsafe {
        webview.AddWebResourceRequestedFilter(w!("*"), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL)?
    };
    let handler = WebResourceRequestedEventHandler::create(Box::new(move |_wv, args| {
        let Some(args) = args else { return Ok(()) };
        // SAFETY: чтение запроса
        let request = unsafe { args.Request() }?;
        let mut raw = PWSTR::null();
        // SAFETY: out-параметр
        unsafe { request.Uri(&mut raw) }?;
        let uri = pwstr_to_string(raw);
        if uri.starts_with(&allowed) {
            return Ok(()); // виртуальный origin — обслуживается из папки пакета
        }
        if grants_network {
            return Ok(()); // разрешено манифестом
        }
        // Отказ: пустой 403-ответ
        let empty = HSTRING::from("");
        if let Some(env) = env.clone() {
            // SAFETY: синтетический ответ-запрет (SPEC §7.6: network opt-in)
            if let Ok(response) = unsafe {
                env.CreateWebResourceResponse(
                    None,
                    403,
                    PCWSTR::from_raw(empty.as_ptr()),
                    PCWSTR::from_raw(empty.as_ptr()),
                )
            } {
                // SAFETY: подмена ответа
                let _ = unsafe { args.SetResponse(&response) };
            }
        }
        tracing::warn!(node_id = %node, %uri, "внешняя сеть без permission заблокирована");
        Ok(())
    }));
    // SAFETY: регистрация
    unsafe { webview.add_WebResourceRequested(&handler, &mut token)? };

    // Esc возвращает фокус канвасу (SPEC §8)
    let weak = Rc::downgrade(state);
    // SAFETY: см. выше
    let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_ctrl, args| {
        let Some(args) = args else { return Ok(()) };
        let mut vk = 0u32;
        // SAFETY: виртуальный код клавиши
        unsafe { args.VirtualKey(&mut vk) }?;
        // Реагируем на нажатие (keydown), не на отпуск
        let mut kind = COREWEBVIEW2_KEY_EVENT_KIND(0);
        // SAFETY: тип события клавиатуры (out-параметр)
        unsafe { args.KeyEventKind(&mut kind) }?;
        if vk == u32::from(VK_ESCAPE.0) && kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN {
            if let Some(state) = weak.upgrade() {
                let parent = state.borrow().parent;
                // SAFETY: фокус канвасу
                let _ = unsafe { SetFocus(Some(parent)) };
            }
            // SAFETY: глотаем Esc — он адресован хосту, не виджету
            unsafe { args.SetHandled(true) }?;
        }
        Ok(())
    }));
    // SAFETY: регистрация на контроллере
    unsafe { controller.add_AcceleratorKeyPressed(&handler, &mut token)? };

    // --- Навигация и геометрия ----------------------------------------------

    let url = HSTRING::from(format!(
        "{origin_prefix}{}",
        manifest.entry.replace('\\', "/")
    ));
    // SAFETY: переход на виртуальный origin пакета
    unsafe { webview.Navigate(PCWSTR::from_raw(url.as_ptr()))? };

    let mut inst = Instance {
        child,
        controller,
        webview,
        webview3,
        bounds: PhysRect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        },
        corner: 0,
        zoom_factor: 0.0,
        visible: false,
        suspended: false,
    };
    update_instance_geometry(&mut inst, rect, corner, zoom);
    show_instance(&mut inst);
    {
        let mut s = state.borrow_mut();
        s.pending.remove(node_id);
        s.failed.remove(node_id);
        s.instances.insert(node_id.to_owned(), inst);
    }
    Ok(())
}

/// Чтение IStream в байты (Seek 0 → чанками по 8 КБ, кламп 4 МБ).
fn read_stream(stream: &IStream) -> windows::core::Result<Vec<u8>> {
    let mut out = Vec::new();
    // SAFETY: перемотка в начало
    unsafe { stream.Seek(0, STREAM_SEEK_SET, None)? };
    let mut chunk = [0u8; 8192];
    loop {
        let mut read = 0u32;
        // SAFETY: буфер валиден, размер передан корректно
        let hr = unsafe {
            stream.Read(
                chunk.as_mut_ptr() as *mut core::ffi::c_void,
                chunk.len() as u32,
                Some(&mut read),
            )
        };
        hr.ok()?;
        if read == 0 {
            break;
        }
        out.extend_from_slice(&chunk[..read as usize]);
        if out.len() > 4 * 1024 * 1024 {
            tracing::warn!("снапшот >4 МБ — обрезан");
            break;
        }
    }
    Ok(out)
}

/// PWSTR из out-параметра в String с освобождением CoTaskMemFree.
/// SAFETY-контракт: вызывающий гарантирует, что raw либо null, либо выделен
/// CoTaskMemAlloc (get-свойства WebView2).
fn pwstr_to_string(raw: PWSTR) -> String {
    if raw.is_null() {
        return String::new();
    }
    // SAFETY: строка валидна до освобождения
    let s = unsafe { raw.to_string().unwrap_or_default() };
    // SAFETY: освобождение по контракту выделения
    unsafe {
        windows::Win32::System::Com::CoTaskMemFree(Some(raw.as_ptr() as *const core::ffi::c_void))
    };
    s
}

/// Регистрация класса child-окон (однократно на процесс).
fn register_widget_window_class() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static REGISTERED: AtomicBool = AtomicBool::new(false);
    if REGISTERED.load(Ordering::Relaxed) {
        return;
    }
    let class = WNDCLASSW {
        // SAFETY: обёртка с корректной ABI (extern "system")
        lpfnWndProc: Some(widget_wnd_proc),
        hInstance: unsafe { GetModuleHandleW(None) }
            .map(|m| HINSTANCE(m.0))
            .unwrap_or_default(),
        lpszClassName: w!("CanvasDeskWidgetHost"),
        ..Default::default()
    };
    // SAFETY: регистрация idempotent (ERROR_CLASS_ALREADY_EXISTS — ок)
    let atom = unsafe { RegisterClassW(&class) };
    if atom == 0 {
        tracing::warn!("класс CanvasDeskWidgetHost уже зарегистрирован или отказ");
    }
    REGISTERED.store(true, Ordering::Relaxed);
}

/// Snapshot-размер по запросу (для менеджера; в хосте — только декод).
pub fn snapshot_size_for(rect: PhysRect) -> (u32, u32) {
    let clamp = crate::layout::SNAPSHOT_MAX_SIDE;
    let scale = (clamp as f32 / rect.w.max(rect.h).max(1) as f32).min(1.0);
    (
        (rect.w.max(1) as f32 * scale).round() as u32,
        (rect.h.max(1) as f32 * scale).round() as u32,
    )
}

/// WNDPROC child-окон: чистый DefWindowProc с корректной ABI.
unsafe extern "system" fn widget_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    // SAFETY: проброс в системную процедуру
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_size_clamps() {
        let big = PhysRect {
            x: 0,
            y: 0,
            w: 1024,
            h: 512,
        };
        assert_eq!(snapshot_size_for(big), (512, 256));
        let small = PhysRect {
            x: 0,
            y: 0,
            w: 304,
            h: 164,
        };
        assert_eq!(snapshot_size_for(small), (304, 164));
        let degenerate = PhysRect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        };
        assert_eq!(snapshot_size_for(degenerate).0, 1);
    }
}
