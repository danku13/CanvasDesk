//! M8/W6 (wasm-port §4.2): недавние канвасы — список имён в IndexedDB
//! (через JS-глю `window.__canvasdesk` в index.html; IDs-бойлерплейт —
//! компактнее в JS, Rust владеет логикой выбора). «F5 → reopen из недавних
//! без пикера»: старт выбирает верхнюю запись (recent_top) — OPFS не
//! требует разрешений; дисковый хэндл (FS Access) переоткрывается кнопкой
//! «Недавние» — requestPermission обязан жить внутри жеста пользователя.
//!
//! Хранилище: база `canvasdesk`, стор `recent`, ключ `name`, значение
//! `{ name, ts }` (ts — Date.now() в JS). Чистая часть ([`recent_top_of`])
//! тестируется нативно.

/// Выбрать «верхний» недавний: максимум ts; пустой список — None.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // потребители — init/reopen (wasm); натив: только тесты
pub(crate) fn recent_top_of(entries: &[(String, f64)]) -> Option<String> {
    entries
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(name, _)| name.clone())
}

/// Записать имя в недавние (fire-and-forget; ошибки — в лог внутри глю).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn record_recent(name: &str) {
    if crate::js_glue::call("recentPut", &[name.into()])
        .await
        .is_none()
    {
        tracing::debug!(target: "canvas_web", name, "JS-глю недавних недоступен (IndexedDB-запись пропущена)");
    }
}

/// Список недавних: (имя, ts). Ошибки/отсутствие глю — пустой список
/// (старт честно уходит в default.canvas).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn recent_list() -> Vec<(String, f64)> {
    let Some(Ok(value)) = crate::js_glue::call("recentList", &[]).await else {
        return Vec::new();
    };
    parse_recent(&value)
}

/// Верхняя запись недавних (или None).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn recent_top() -> Option<String> {
    recent_top_of(&recent_list().await)
}

/// Разбор ответа глю: Array<{name: string, ts: number}> → Vec<(String, f64)>.
/// Чужие/битые элементы молча пропускаются (IndexedDB-данные сторонних
/// версий не должны ронять старт).
#[cfg(target_arch = "wasm32")]
fn parse_recent(value: &wasm_bindgen::JsValue) -> Vec<(String, f64)> {
    use wasm_bindgen::JsCast;
    let Ok(array) = value.clone().dyn_into::<js_sys::Array>() else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|entry| {
            let object = entry.dyn_into::<js_sys::Object>().ok()?;
            let name = js_sys::Reflect::get(&object, &"name".into())
                .ok()?
                .as_string()?;
            let ts = js_sys::Reflect::get(&object, &"ts".into()).ok()?.as_f64()?;
            Some((name, ts))
        })
        .collect()
}

/// Хэндл дискового файла по имени (для reopen; None — нет записи/глю).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn disk_handle_of(name: &str) -> Option<web_sys::FileSystemFileHandle> {
    use wasm_bindgen::JsCast;
    let Some(Ok(value)) = crate::js_glue::call("handleGet", &[name.into()]).await else {
        return None;
    };
    value.dyn_into::<web_sys::FileSystemFileHandle>().ok()
}

/// Запомнить дисковый хэндл под именем канваса (fire-and-forget).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn store_disk_handle(name: &str, handle: &web_sys::FileSystemFileHandle) {
    let _ = crate::js_glue::call("handlePut", &[name.into(), handle.into()]).await;
}

// ============================================================================
// Чистые тесты (натив)
// ============================================================================

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::recent_top_of;

    /// Верхний недавний = максимум ts; пустой список — None.
    #[test]
    fn top_is_max_ts() {
        let entries = vec![
            ("a.canvas".to_string(), 100.0),
            ("c.canvas".to_string(), 300.0),
            ("b.canvas".to_string(), 200.0),
        ];
        assert_eq!(recent_top_of(&entries).as_deref(), Some("c.canvas"));
        assert_eq!(recent_top_of(&[]), None);
        // Один элемент — он и верхний
        let single = vec![("only.canvas".to_string(), 1.0)];
        assert_eq!(recent_top_of(&single).as_deref(), Some("only.canvas"));
        // Равные ts — стабильный максимум (последний из равных не важен,
        // важен детерминизм: max_by возвращает ПОСЛЕДНИЙ максимум)
        let tie = vec![("x.canvas".to_string(), 5.0), ("y.canvas".to_string(), 5.0)];
        let top = recent_top_of(&tie).expect("есть максимум");
        assert!(top == "x.canvas" || top == "y.canvas");
    }
}
