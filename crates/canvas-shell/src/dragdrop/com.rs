//! COM-реализация IDropTarget (T9): OleInitialize на потоке event loop,
//! RegisterDragDrop, извлечение CF_HDROP/CF_UNICODETEXT из IDataObject.
//!
//! Реализация по плану `docs/plans/T9-drag-drop.md` §5 (шаг 5). Сырые данные
//! (байты CF_HDROP / строка CF_UNICODETEXT) уходят в приложение как есть —
//! парсинг и раскладку делает `canvas_app::ui` (там тестируется на любой ОС).
//! Паники в COM-методах нет: ошибки чтения данных дают DROPEFFECT_NONE.
//!
//! Тонкость windows-rs 0.62: `#[implement]` генерирует тип-обёртку
//! `DropTargetHandler_Impl` (IUnknownImpl + vtbl), трейты интерфейсов
//! реализуются на ней, а поля — на исходном `DropTargetHandler` (обёртка
//! Deref'ится к нему).

use std::sync::{Arc, Mutex};

use windows::core::implement;
use windows::Win32::Foundation::{HWND, POINT, POINTL};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::Com::{
    IDataObject, DVASPECT_CONTENT, FORMATETC, STGMEDIUM, TYMED_HGLOBAL,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{
    IDropTarget, IDropTarget_Impl, OleInitialize, RegisterDragDrop, ReleaseStgMedium,
    RevokeDragDrop, CF_HDROP, CF_UNICODETEXT, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;

use super::{DragData, DragEvent};

/// Состояние drop-target'а: окно, отправитель событий и кэш данных DragEnter
/// (Explorer отдаёт DATA уже в Enter — DragOver их не даёт; Drop перечитывает).
#[implement(IDropTarget)]
struct DropTargetHandler {
    hwnd: HWND,
    sender: Arc<dyn Fn(DragEvent) + Send + Sync>,
    cached: Mutex<Option<DragData>>,
}

impl DropTargetHandler {
    /// Экранные координаты (POINTL из IDropTarget) -> клиентские физические
    /// px окна; в f32 после конвертации.
    fn client_pt(&self, pt: &POINTL) -> (f32, f32) {
        let mut point = POINT { x: pt.x, y: pt.y };
        // SAFETY: hwnd валиден, пока жив App (DropWatcher живёт не дольше);
        // point — локальный POD, ScreenToClient только пишет в него.
        // Ложный результат (окно уже уничтожено — гонка завершения) оставляет
        // экранные координаты: событие уходит, но приложение его переживёт.
        let _ = unsafe { ScreenToClient(self.hwnd, &mut point) };
        (point.x as f32, point.y as f32)
    }

    /// Доступ к кэшу данных drag. Отравление mutex'а возможно только при
    /// панике в методах — её нет; на всякий случай забираем даже отравленный.
    fn cache(&self) -> std::sync::MutexGuard<'_, Option<DragData>> {
        self.cached.lock().unwrap_or_else(|err| err.into_inner())
    }
}

/// Трейт IDropTarget на сгенерированной обёртке (см. модульный комментарий):
/// `self` Deref'ится к `DropTargetHandler`, методы — чистые, без паники.
impl IDropTarget_Impl for DropTargetHandler_Impl {
    fn DragEnter(
        &self,
        pdataobj: windows::core::Ref<IDataObject>,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        // Данные снимаем сразу: Enter — единственный момент, когда источник
        // гарантированно отдаёт IDataObject; дальше кэш живёт до Drop
        let data = pdataobj.as_ref().map_or(DragData::None, extract);
        let has_data = data != DragData::None;
        let client_pt = self.client_pt(pt);
        *self.cache() = Some(data.clone());
        (self.sender)(DragEvent::Enter { data, client_pt });
        set_effect(pdweffect, has_data);
        Ok(())
    }

    fn DragOver(
        &self,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        let client_pt = self.client_pt(pt);
        (self.sender)(DragEvent::Over { client_pt });
        // Данных в Over нет — эффект по наличию кэша из Enter
        let has_data = self.cache().is_some();
        set_effect(pdweffect, has_data);
        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        *self.cache() = None;
        (self.sender)(DragEvent::Leave);
        Ok(())
    }

    fn Drop(
        &self,
        pdataobj: windows::core::Ref<IDataObject>,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        // План T9 §5: на Drop данные перечитываем с IDataObject заново —
        // кэшу Enter не доверяем (источник мог обновить содержимое)
        let data = pdataobj.as_ref().map_or(DragData::None, extract);
        let has_data = data != DragData::None;
        *self.cache() = None;
        let client_pt = self.client_pt(pt);
        (self.sender)(DragEvent::Drop { data, client_pt });
        set_effect(pdweffect, has_data);
        Ok(())
    }
}

/// Эффект курсора для OLE: COPY при поддерживаемых данных, иначе NONE.
/// Нулевой указатель (у OLE его нет, но подстрахуемся) — просто пропускаем.
fn set_effect(pdweffect: *mut DROPEFFECT, has_data: bool) {
    if pdweffect.is_null() {
        return;
    }
    // SAFETY: OLE всегда передаёт валидный указатель на DWORD-эффект
    // (in/out параметр IDropTarget); null проверен выше.
    unsafe {
        *pdweffect = if has_data {
            DROPEFFECT_COPY
        } else {
            DROPEFFECT_NONE
        };
    }
}

/// Снять данные с IDataObject: CF_HDROP (файлы/папки Explorer) — сырые байты
/// целиком (заголовок DROPFILES + список, парсит `canvas_app::ui`);
/// иначе CF_UNICODETEXT — текст до первого 0u16; иначе `DragData::None`
/// (поддерживаемых форматов нет -> эффект DROPEFFECT_NONE).
fn extract(data_obj: &IDataObject) -> DragData {
    let fmt = |cf_format: u16| FORMATETC {
        cfFormat: cf_format,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };
    // CF_HDROP — приоритетный формат (файлы и папки, включая множественный
    // выбор). SAFETY: стандартный GetData; при успехе STGMEDIUM переходит
    // нам и освобождается ReleaseStgMedium сразу после копирования.
    if let Ok(mut medium) = unsafe { data_obj.GetData(&fmt(CF_HDROP.0)) } {
        let bytes = hglobal_bytes(&medium);
        // SAFETY: успешный GetData передаёт владение STGMEDIUM — обязательный
        // ReleaseStgMedium после копирования байтов (иначе утечка HGLOBAL).
        unsafe {
            ReleaseStgMedium(&mut medium);
        }
        if let Some(bytes) = bytes {
            return DragData::HdropBytes(bytes);
        }
        // CF_HDROP есть, но блок не читается — не падаем: эффект будет NONE
        return DragData::None;
    }
    // Нет CF_HDROP — пробуем текст (URL/строка из браузера и т.п.).
    // SAFETY: то же, что и выше для CF_HDROP.
    if let Ok(mut medium) = unsafe { data_obj.GetData(&fmt(CF_UNICODETEXT.0)) } {
        let bytes = hglobal_bytes(&medium);
        // SAFETY: см. выше — ReleaseStgMedium обязателен при успехе GetData.
        unsafe {
            ReleaseStgMedium(&mut medium);
        }
        if let Some(bytes) = bytes {
            // UTF-16 до первого 0u16 (CF_UNICODETEXT null-terminated);
            // from_utf16_lossy — битые суррогаты не роняют приложение.
            // Хвостовой нечётный байт (remainder) игнорируется.
            let units: Vec<u16> = bytes
                // chunks_exact (а не as_chunks) — MSRV 1.80 рабочей области
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .take_while(|&unit| unit != 0)
                .collect();
            return DragData::Text(String::from_utf16_lossy(&units));
        }
    }
    DragData::None
}

/// Скопировать байты из HGLOBAL-носителя STGMEDIUM (запрашивали TYMED_HGLOBAL).
/// None — GlobalLock не дал указатель (пустой/битый блок): данные недоступны,
/// но не паникуем — эффект будет DROPEFFECT_NONE.
fn hglobal_bytes(medium: &STGMEDIUM) -> Option<Vec<u8>> {
    // SAFETY: union-чтение hGlobal корректно при tymed == TYMED_HGLOBAL —
    // формат запрошен именно так в FORMATETC выше.
    let hglobal = unsafe { medium.u.hGlobal };
    // SAFETY: GlobalLock на HGLOBAL из GetData; указатель валиден до
    // парного GlobalUnlock; null проверяем сразу.
    let ptr = unsafe { GlobalLock(hglobal) };
    if ptr.is_null() {
        return None;
    }
    let size = unsafe { GlobalSize(hglobal) };
    // SAFETY: ptr и size от одного HGLOBAL — размер аллокации, читаем её
    // целиком копией, чтобы не зависеть от lifetime глобальной памяти.
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, size) }.to_vec();
    // SAFETY: GlobalLock требует парного GlobalUnlock — ptr получен выше.
    let _ = unsafe { GlobalUnlock(hglobal) };
    Some(bytes)
}

/// Владелец зарегистрированного drop-target: `Drop` отзывает регистрацию
/// (RevokeDragDrop) и отпускает COM-ссылку — объект DropTargetHandler
/// освободится, когда OLE отпустит свои ссылки.
pub struct DropWatcher {
    hwnd: HWND,
    /// COM-ссылка живёт только ради владения: отпускает DropTargetHandler,
    /// когда OLE перестаёт держать свои (потому и не читается).
    _target: IDropTarget,
}

impl Drop for DropWatcher {
    fn drop(&mut self) {
        // SAFETY: окно могло уже уничтожиться — winit сам зовёт RevokeDragDrop
        // в WM_DESTROY, двойной вызов безвреден; результат сознательно
        // игнорируем (снятие с уже уничтоженного окна не ошибка для нас).
        let _ = unsafe { RevokeDragDrop(self.hwnd) };
    }
}

/// Зарегистрировать IDropTarget на окне `hwnd` и доставлять события через
/// `sender`. Требует STA (OleInitialize) на потоке event loop — вызывается
/// в `resumed()`; ошибка наружу -> приложение работает без drag-drop
/// (graceful degradation, план T9 §7).
///
/// HWND приходит сырым `isize`: winit 0.30 публично отдаёт его только через
/// raw-window-handle (`Win32WindowHandle::hwnd.get()`), а свою зависимость
/// `windows` в canvas-app не тащим (AGENTS — Win32 только в canvas-shell);
/// конвертация в типизированный HWND — здесь.
pub fn install(
    hwnd: isize,
    sender: Arc<dyn Fn(DragEvent) + Send + Sync>,
) -> windows::core::Result<DropWatcher> {
    // Win32 HWND — указатель без внутренней структуры: конвертация тривиальна
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    // SAFETY: OleInitialize обязателен до RegisterDragDrop (STA потока event
    // loop; winit с with_drag_and_drop(false) его не делает). Парный
    // OleUninitialize не зовём: поток event loop живёт до конца процесса,
    // а вызов после чужой деинициализации опаснее отсутствия.
    // S_OK и S_FALSE (уже инициализирован) оба маппятся в Ok.
    unsafe { OleInitialize(None) }?;
    let handler = DropTargetHandler {
        hwnd,
        sender,
        cached: Mutex::new(None),
    };
    // ComObject забирает handler в кучу; интерфейс держит объект живым,
    // пока жив target (в DropWatcher) и ссылки OLE
    let target: IDropTarget = handler.into();
    // SAFETY: hwnd валиден (окно только что создано в resumed), target —
    // валидный COM-объект; RegisterDragDrop AddRef'ит его себе.
    unsafe { RegisterDragDrop(hwnd, &target) }?;
    Ok(DropWatcher {
        hwnd,
        _target: target,
    })
}
