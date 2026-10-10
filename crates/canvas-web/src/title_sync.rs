//! FR-107 (мультиканвас C4, №28a): `document.title` вкладки — «Имя —
//! CanvasDesk». Обновляется при каждой смене активного канваса тем же
//! хуком, что URL-синк `?canvas=` из C1 ([`web_state::set_active`] →
//! локи + URL + title), в т.ч. при открытии, создании и ренейме активного
//! (ренейм активного переезжает на новое имя в `web_requests::run_canvas_op`).
//! Нет активного имени — просто «CanvasDesk».
//!
//! Чистая часть ([`document_title`]) тестируется нативно; wasm-часть —
//! тонкая обёртка над `Document::set_title` (паттерн `url_sync`, №17a).

/// Заголовок вкладки для активного канваса (№28a): `Some(имя файла)` →
/// «Имя — CanvasDesk» (имя — [`display_name`], без `.canvas`); `None`
/// или пустое имя — просто «CanvasDesk». Чистая функция (нативный тест).
pub fn document_title(file_name: Option<&str>) -> String {
    match file_name
        .map(canvas_core::workspace::display_name)
        .filter(|name| !name.is_empty())
    {
        Some(name) => format!("{name} — CanvasDesk"),
        None => "CanvasDesk".to_owned(),
    }
}

// ============================================================================
// Wasm-часть: document.title
// ============================================================================

/// Применить заголовок вкладки (идемпотентно; ошибки — в лог: заголовок —
/// декоративный хром, падать он не должен).
#[cfg(target_arch = "wasm32")]
pub(crate) fn sync(file_name: Option<&str>) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    document.set_title(&document_title(file_name));
}

// ============================================================================
// Нативные тесты: чистая часть заголовка
// ============================================================================

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::document_title;

    /// №28a: имя канваса — «Имя — CanvasDesk» (display_name без .canvas).
    #[test]
    fn title_with_canvas_name() {
        assert_eq!(
            document_title(Some("мой проект.canvas")),
            "мой проект — CanvasDesk"
        );
        assert_eq!(
            document_title(Some("default.canvas")),
            "default — CanvasDesk"
        );
        // Дисковый файл — тот же формат (имя = имя файла)
        assert_eq!(
            document_title(Some("отчёт-q4.canvas")),
            "отчёт-q4 — CanvasDesk"
        );
    }

    /// №28a: нет активного имени — просто «CanvasDesk».
    #[test]
    fn title_without_canvas_name() {
        assert_eq!(document_title(None), "CanvasDesk");
        assert_eq!(
            document_title(Some("")),
            "CanvasDesk",
            "пустое имя — без дефиса"
        );
    }

    /// №28a: имя без расширения и «голое» расширение не ломают формат.
    #[test]
    fn title_edge_names() {
        // Имя без .canvas — как есть (уникальные имена уже уникальны)
        assert_eq!(document_title(Some("шаблон")), "шаблон — CanvasDesk");
        // Файл из «голого» расширения — display_name пуст → дефолт
        assert_eq!(document_title(Some(".canvas")), "CanvasDesk");
    }
}
