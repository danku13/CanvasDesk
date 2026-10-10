//! FR-104 (мультиканвас C1, №17a): URL-синк активного канваса —
//! `history.replaceState` с `?canvas=<имя>` при каждой смене
//! (`web_state::set_active`, единая точка открытия на web). Дисковый
//! канвас (FS Access) параметр УБИРАЕТ: `?canvas=` по контракту W6 —
//! имя в OPFS, ссылка на дисковый файл так не открывается.
//!
//! `replaceState` (не `pushState`): история не обрастает шагами
//! переключения канвасов, кнопка «назад» остаётся про навигацию страницы.
//! Остальные параметры (`?log=`, `?stress=`, …) сохраняются как есть.
//!
//! Чистая часть ([`replace_canvas_param`]) тестируется нативно; wasm-часть —
//! тонкая обёртка над `window.history()` (паттерн `clean_url_after_callback`
//! в llm_web, FR-096).

#[cfg(target_arch = "wasm32")] // sync_active (зеркалит тип хранилища)
use crate::web_state::ActiveKind;

/// Заменить параметр `canvas` в строке запроса (`?a=1&canvas=x&b=2`):
/// прежние вхождения `canvas` удаляются, новое дописывается в конец;
/// `None` — параметр убирается. Остальные пары сохраняются в исходном
/// виде и порядке (значения НЕ перекодируются — проход насквозь).
/// Возвращает строку С ведущим `?` либо пустую (параметров не осталось).
pub fn replace_canvas_param(search: &str, value: Option<&str>) -> String {
    let body = search.strip_prefix('?').unwrap_or(search);
    let mut pairs: Vec<String> = body
        .split('&')
        .filter(|pair| {
            // Пустые пары («?&», «a=1&&b=2») не переносим; canvas заменим.
            !pair.is_empty() && pair.split_once('=').map_or(true, |(k, _)| k != "canvas")
        })
        .map(str::to_owned)
        .collect();
    if let Some(value) = value {
        pairs.push(format!("canvas={}", encode_component(value)));
    }
    if pairs.is_empty() {
        String::new()
    } else {
        format!("?{}", pairs.join("&"))
    }
}

/// Минимальный процентов-кодировщик значения (набор `encodeURIComponent`:
/// не кодируются ASCII буквы/цифры и `-_.!~*'()`). Имена канвасов
/// санитизированы (буквы/цифры/пробел/`._-`), но пробел и кириллица
/// обязаны ехать в `%XX` — иначе `location.search` их не вернёт.
fn encode_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

// ============================================================================
// Wasm-часть: history.replaceState
// ============================================================================

/// Синк URL при смене активного канваса: OPFS — `?canvas=<имя>`, диск —
/// убрать параметр (дисковый файл по ?canvas= не открывается).
#[cfg(target_arch = "wasm32")]
pub(crate) fn sync_active(kind: ActiveKind, name: &str) {
    let value = (kind == ActiveKind::Opfs).then_some(name);
    sync_canvas_param(value);
}

/// Записать `?canvas=` в текущий URL (без перезагрузки/шага истории).
/// Идемпотентно: неизменившаяся строка запроса не трогает history.
#[cfg(target_arch = "wasm32")]
fn sync_canvas_param(value: Option<&str>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let search = window.location().search().unwrap_or_default();
    let next = replace_canvas_param(&search, value);
    if next == search {
        return;
    }
    let path = window.location().pathname().unwrap_or_default();
    let url = format!("{path}{next}");
    if let Err(err) = window.history().and_then(|history| {
        history
            .replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&url))
            .map_err(|err| err)
    }) {
        tracing::warn!(target: "canvas_web", ?err, "URL-синк ?canvas= не удался");
    }
}

// ============================================================================
// Нативные тесты: чистая часть replaceState-строки
// ============================================================================

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// №17a: установка параметра — с нуля, замена, сохранение соседей.
    #[test]
    fn canvas_param_set_and_replace() {
        // С нуля (стартовый выбор канваса)
        assert_eq!(
            replace_canvas_param("", Some("default.canvas")),
            "?canvas=default.canvas"
        );
        // Замена существующего, соседи сохранены в исходном порядке
        assert_eq!(
            replace_canvas_param("?canvas=old.canvas&log=debug", Some("new.canvas")),
            "?log=debug&canvas=new.canvas"
        );
        // Значение без параметров вокруг
        assert_eq!(
            replace_canvas_param("?canvas=old.canvas", Some("new.canvas")),
            "?canvas=new.canvas"
        );
        // Повторная установка того же значения — та же строка (wasm-часть
        // на этом пропускает replaceState)
        assert_eq!(
            replace_canvas_param("?canvas=x.canvas", Some("x.canvas")),
            "?canvas=x.canvas"
        );
    }

    /// №17a: удаление параметра (дисковый канвас / стресс-деградация).
    #[test]
    fn canvas_param_removed() {
        assert_eq!(replace_canvas_param("?canvas=x.canvas", None), "");
        assert_eq!(
            replace_canvas_param("?stress=5&canvas=x.canvas&log=debug", None),
            "?stress=5&log=debug",
            "соседи сохранены, canvas убран"
        );
        assert_eq!(replace_canvas_param("", None), "");
    }

    /// Кириллица и пробел кодируются (`%XX` UTF-8), безопасные символы —
    /// как есть: `location.search` возвращает именно такую форму.
    #[test]
    fn canvas_param_percent_encodes_value() {
        assert_eq!(
            replace_canvas_param("", Some("мой проект.canvas")),
            // м=%D0%BC о=%D0%BE й=%D0%B9, пробел=%20
            "?canvas=%D0%BC%D0%BE%D0%B9%20%D0%BF%D1%80%D0%BE%D0%B5%D0%BA%D1%82.canvas"
        );
        assert_eq!(
            replace_canvas_param("", Some("a-b_1.2")),
            "?canvas=a-b_1.2",
            "безопасные символы не кодируются"
        );
    }

    /// Битые/пустые пары вокруг не ломают синк (паттерн parse_query).
    #[test]
    fn canvas_param_tolerates_empty_pairs() {
        assert_eq!(
            replace_canvas_param("?&canvas=x.canvas&&log=debug", Some("y.canvas")),
            "?log=debug&canvas=y.canvas",
            "пустые пары не переносятся"
        );
        // «голый» ключ без «=» — не canvas, сохраняется
        assert_eq!(replace_canvas_param("?flag&canvas=x.canvas", None), "?flag");
    }
}
