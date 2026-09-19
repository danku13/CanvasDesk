//! M8/W5 (wasm-port §3.2, «Аргументы CLI» → web): URL-параметры вместо
//! `std::env::args`. Приёмка W5 требует `?stress=5000` (60 fps) —
//! минимальный набор строки `CliArgs`; полный `?canvas=`/recent — W6 (§4).
//!
//! Парсер — **чистая функция** над строкой запроса (процентов-декодирование
//! не нужно: значения — только числа), поэтому тестируется нативно в
//! обычных `#[test]` (гейты каркаса) без JS-рунтайма. web-часть — только
//! взять `location.search` (`spawn_desk`).
//!
//! Неизвестные параметры игнорируются (резерв W6: `?canvas=`, `?theme=`);
//! битые значения — None + warn в лог (деградация, не паника — правило
//! обёртки: страница обязана открыться при любом URL).

/// Разобранные URL-параметры запуска (зеркало части `CliArgs` — `parse_args`
/// в canvas-app; desktop/path на web не существуют: `--desktop` —
/// Windows-only, файл-путь приходит из хранилища — W6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WebParams {
    /// `?stress=N` — нагрузочная сцена из N нод вместо seed-канваса (T5).
    pub stress: Option<usize>,
    /// `?stress-widgets=N` — добавить N виджет-нод (нагрузка M5, T20-F).
    pub stress_widgets: Option<usize>,
    /// `?log=debug` — уровень консольного лога (W7: диагностика на web,
    /// дефолт INFO). Неизвестное значение — None (тихо, INFO).
    pub log_level: Option<LogLevel>,
}

/// Уровень лога из URL (срез `tracing_subscriber::filter::LevelFilter`:
/// canvas-web не тянет tracing-subscriber в сигнатуры url_params).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Trace,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// Разобрать имя уровня (регистронезависимо, как RUST_LOG); None —
    /// не уровень (тихий фолбэк на INFO: URL с опечаткой не повод
    /// отказывать странице в параметрах).
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "debug" => Some(Self::Debug),
            "trace" => Some(Self::Trace),
            "info" => Some(Self::Info),
            "warn" | "warning" => Some(Self::Warn),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

/// Извлечь значение числового параметра из пары `key=value`; нечисловое
/// значение — ошибка (`None` снаружи превращается в warn).
fn numeric_param(query: &str, key: &str) -> Result<Option<usize>, String> {
    for pair in query.split('&') {
        // Пустые пары от «?a=1&&b=2» / «?&» — пропустить
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        if name != key {
            continue;
        }
        // Число с подчёркиваниями/пробелами не принимаем: parse честно вернёт
        // ошибку — warn в лог понятнее тихого игнора
        return value
            .parse::<usize>()
            .map(Some)
            .map_err(|_| format!("{key}: не число: {value}"));
    }
    Ok(None)
}

/// Извлечь значение строкового параметра (первое вхождение; значения без
/// процентов-декодирования — принимаются только простые токены).
fn string_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        if name == key && !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

/// Разобрать строку запроса (`location.search`, с ведущим `?` или без).
/// Неизвестные ключи игнорируются; битое число — Err с текстом (caller
/// пишет warn и продолжает без параметра). `log` — мягкий параметр:
/// неузнанное значение молча даёт INFO, ошибки не порождает (URL с
/// опечаткой в уровне лога не должен ронять `?stress`).
pub fn parse_query(query: &str) -> Result<WebParams, String> {
    let query = query.strip_prefix('?').unwrap_or(query);
    if query.is_empty() {
        return Ok(WebParams::default());
    }
    let stress = numeric_param(query, "stress")?;
    let stress_widgets = numeric_param(query, "stress-widgets")?;
    let log_level = string_param(query, "log").and_then(|value| LogLevel::parse(&value));
    Ok(WebParams {
        stress,
        stress_widgets,
        log_level,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_query, LogLevel, WebParams};

    /// Приёмка W5: `?stress=5000` — нагрузочная сцена на 5000 нод.
    #[test]
    fn stress_param_parses() {
        let params = parse_query("?stress=5000").expect("валидный запрос");
        assert_eq!(
            params,
            WebParams {
                stress: Some(5000),
                stress_widgets: None,
                log_level: None
            }
        );
    }

    /// Ведущий `?` опционален (location.search его всегда даёт, но парсер
    /// не должен требовать).
    #[test]
    fn leading_question_mark_is_optional() {
        assert_eq!(
            parse_query("stress=7").expect("валидный запрос"),
            WebParams {
                stress: Some(7),
                stress_widgets: None,
                log_level: None
            }
        );
    }

    /// Оба параметра сразу + неизвестные ключи между ними — игнор.
    #[test]
    fn multiple_params_and_unknown_keys() {
        let params = parse_query("?stress=100&canvas=x&stress-widgets=10&theme=dark")
            .expect("валидный запрос");
        assert_eq!(
            params,
            WebParams {
                stress: Some(100),
                stress_widgets: Some(10),
                log_level: None
            }
        );
    }

    /// Пустой запрос — дефолт (обычный запуск без параметров).
    #[test]
    fn empty_query_is_default() {
        assert_eq!(
            parse_query("").expect("пустой запрос"),
            WebParams::default()
        );
        assert_eq!(
            parse_query("?").expect("пустой запрос"),
            WebParams::default()
        );
    }

    /// Битое число — Err (caller: warn + запуск без параметра).
    #[test]
    fn broken_number_is_error() {
        assert!(parse_query("?stress=abc").is_err());
        assert!(parse_query("?stress=1e3").is_err());
        assert!(parse_query("?stress=-5").is_err());
        // Пустое значение — тоже не число
        assert!(parse_query("?stress=").is_err());
    }

    /// Пустые пары и ключи без значения не роняют разбор.
    #[test]
    fn empty_pairs_are_skipped() {
        let params = parse_query("?&stress=3&&x&=1").expect("валидный запрос");
        assert_eq!(params.stress, Some(3));
    }

    /// W7: `?log=debug` — уровень консоли (диагностика на web).
    #[test]
    fn log_level_param_parses() {
        let params = parse_query("?log=debug").expect("валидный запрос");
        assert_eq!(params.log_level, Some(LogLevel::Debug));
        // Регистронезависимо (как RUST_LOG)
        let params = parse_query("?log=TRACE").expect("валидный запрос");
        assert_eq!(params.log_level, Some(LogLevel::Trace));
    }

    /// `log` комбинируется с остальными параметрами (дым W7: стресс+лог).
    #[test]
    fn log_level_combines_with_stress() {
        let params = parse_query("?stress=5000&log=debug").expect("валидный запрос");
        assert_eq!(params.stress, Some(5000));
        assert_eq!(params.log_level, Some(LogLevel::Debug));
    }

    /// Неизвестный уровень — тихий INFO (None), без ошибки: URL с опечаткой
    /// в `log` не должен ронять валидный `?stress` рядом.
    #[test]
    fn unknown_log_level_is_lenient() {
        let params = parse_query("?stress=7&log=вербозный").expect("валидный запрос");
        assert_eq!(params.stress, Some(7));
        assert_eq!(params.log_level, None);
    }
}
