//! M8/W5 (wasm-port §3.2, «Аргументы CLI» → web): URL-параметры вместо
//! `std::env::args`. Приёмка W5 требует `?stress=5000` (60 fps) —
//! минимальный набор строки `CliArgs`; W6 добавил `?canvas=` — имя канваса
//! в OPFS (план §4.2).
//!
//! Парсер — **чистая функция** над строкой запроса (процентов-декодирование
//! не нужно: значения — числа и простые токены), поэтому тестируется
//! нативно в обычных `#[test]` (гейты каркаса) без JS-рунтайма. web-часть —
//! только взять `location.search` (`spawn_desk`).
//!
//! Неизвестные параметры игнорируются (резерв W12: `?theme=`); битые
//! значения — None + warn в лог (деградация, не паника — правило обёртки:
//! страница обязана открыться при любом URL).

/// Разобранные URL-параметры запуска (зеркало части `CliArgs` — `parse_args`
/// в canvas-app; desktop/path на web не существуют: `--desktop` —
/// Windows-only, файл-путь приходит из хранилища — W6).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WebParams {
    /// `?stress=N` — нагрузочная сцена из N нод вместо seed-канваса (T5).
    pub stress: Option<usize>,
    /// `?stress-widgets=N` — добавить N виджет-нод (нагрузка M5, T20-F).
    pub stress_widgets: Option<usize>,
    /// `?log=debug` — уровень консольного лога (W7: диагностика на web,
    /// дефолт INFO). Неизвестное значение — None (тихо, INFO).
    pub log_level: Option<LogLevel>,
    /// `?canvas=имя` — имя канваса в OPFS вместо `default.canvas` (W6,
    /// план §4.2): «открыть именованный канвас по ссылке». Значение
    /// проходит санитизацию ([`sanitize_canvas_name`]): битое/опасное
    /// имя — None (тихий старт с недавним/дефолтным). String вместо
    /// Copy-полей — derive сужен до Clone.
    pub canvas: Option<String>,
    /// `?template=<id>` (FR-049): авто-вставка встроенной схемы после
    /// инициализации App. Неизвестный id — мягкий отказ (тост), URL не
    /// валидируется здесь: реестр схем проверяет при применении.
    pub template: Option<String>,
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

/// Минимальный процентов-декод (URL-кодировка браузера: `location.search`
/// отдаёт кириллицу как `%D0%B8...`). Неизвестные `%`-последовательности
/// остаются как есть (декодер не должен ломать честные `%` в имени).
fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        // %XX — только с двумя валидными hex-цифрами; срезы байтовые
        // (строчные срезы паниковали бы на много-байтовых символах)
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or(""),
                16,
            ) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Извлечь значение строкового параметра (первое вхождение; с
/// процентов-декодом — браузер кодирует кириллицу в location.search).
fn string_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        if name == key && !value.is_empty() {
            return Some(percent_decode(value));
        }
    }
    None
}

/// Санитизация имени канваса из URL/файла (W6): OPFS — плоская ФС без
/// каталогов, но имя попадает в логи, IndexedDB и `download`-атрибут —
/// пропускаем только безопасные имена файлов. Правила: непустое, без
/// разделителей пути (`/`, `\`) и `..`, без управляющих символов, ≤ 80
/// символов; суффикс `.canvas` добавляется при отсутствии (нативная
/// конвенция SPEC §5.1). Буквы/цифры/пробел/`._-` + кириллица/Unicode —
/// разрешены (`char::is_alphanumeric`); остальные символы («?", "#",
/// "&" и т.п.) резались бы URL-парсером или ломали ключи хранилищ —
/// отклоняем всё имя (не вырезаем: пользователь должен видеть отказ).
pub fn sanitize_canvas_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 80 {
        return None;
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed == ".." {
        return None;
    }
    if trimmed.chars().any(char::is_control) {
        return None;
    }
    // Точка не первая и не последняя («.hidden», «name.» — лишние сюрпризы)
    if trimmed.starts_with('.') || trimmed.ends_with('.') {
        return None;
    }
    let safe = trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '_' | '-' | '.'));
    if !safe {
        return None;
    }
    if trimmed.to_ascii_lowercase().ends_with(".canvas") {
        Some(trimmed.to_string())
    } else {
        Some(format!("{trimmed}.canvas"))
    }
}

/// Санитизация имени произвольного файла (M8/W10, DOM-drop): те же правила
/// безопасности, что у [`sanitize_canvas_name`], но расширение сохраняется
/// как есть и суффикс не дописывается (картинки идут в OPFS `files/` для
/// превью WebImageThumbnailProvider, план §4.3).
pub fn sanitize_file_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 80 {
        return None;
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed == ".." {
        return None;
    }
    if trimmed.chars().any(char::is_control) {
        return None;
    }
    if trimmed.starts_with('.') || trimmed.ends_with('.') {
        return None;
    }
    let safe = trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '_' | '-' | '.'));
    if !safe {
        return None;
    }
    Some(trimmed.to_string())
}

/// Кандидаты имени при коллизии (M8/W10): «имя.ext», «имя-1.ext»,
/// «имя-2.ext», … (без расширения — суффикс в конец; ведущая точка не
/// стем: «.gitignore» → «.gitignore-1»). Первый незанятый выбирает
/// вызывающий: проверка наличия на web асинхронная (get_file_handle →
/// NotFoundError = свободно), поэтому чистая функция отдаёт список.
pub fn file_name_candidates(name: &str, max: usize) -> Vec<String> {
    // Стем/расширение: последняя точка (расширение — не пусто и не всё имя)
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, Some(ext)),
        _ => (name, None),
    };
    let mut out = Vec::with_capacity(max);
    out.push(name.to_owned());
    for n in 1..max as u32 {
        out.push(match ext {
            Some(ext) => format!("{stem}-{n}.{ext}"),
            None => format!("{stem}-{n}"),
        });
    }
    out
}

/// Разобрать строку запроса (`location.search`, с ведущим `?` или без).
/// Неизвестные ключи игнорируются; битое число — Err с текстом (caller
/// пишет warn и продолжает без параметра). `log` — мягкий параметр:
/// неузнанное значение молча даёт INFO, ошибки не порождает (URL с
/// опечаткой в уровне лога не должен ронять `?stress`). `canvas` — тоже
/// мягкий: опасное имя — None (тихий старт с недавним/дефолтным канвасом).
pub fn parse_query(query: &str) -> Result<WebParams, String> {
    let query = query.strip_prefix('?').unwrap_or(query);
    if query.is_empty() {
        return Ok(WebParams::default());
    }
    let stress = numeric_param(query, "stress")?;
    let stress_widgets = numeric_param(query, "stress-widgets")?;
    let log_level = string_param(query, "log").and_then(|value| LogLevel::parse(&value));
    let canvas = string_param(query, "canvas").and_then(|value| sanitize_canvas_name(&value));
    let template = string_param(query, "template");
    Ok(WebParams {
        stress,
        stress_widgets,
        log_level,
        canvas,
        template,
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
                log_level: None,
                canvas: None,
                template: None
            }
        );
    }

    /// FR-049 (US-5): `?template=<id>` — id схемы для авто-вставки;
    /// пустое значение — None (мягкий старт без схемы).
    #[test]
    fn template_param_parses() {
        let params = parse_query("?template=com.canvasdesk.scheme.intro-calculations")
            .expect("валидный запрос");
        assert_eq!(
            params.template.as_deref(),
            Some("com.canvasdesk.scheme.intro-calculations")
        );
        let empty = parse_query("?template=").expect("валидный запрос");
        assert_eq!(empty.template, None, "пустое значение — None");
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
                log_level: None,
                canvas: None,
                template: None
            }
        );
    }

    /// Оба параметра сразу + неизвестные ключи между ними — игнор
    /// (W6: `canvas=x` больше не неизвестный — парсится; `theme` по-прежнему
    /// игнорируется).
    #[test]
    fn multiple_params_and_unknown_keys() {
        let params = parse_query("?stress=100&canvas=x&stress-widgets=10&theme=dark")
            .expect("валидный запрос");
        assert_eq!(
            params,
            WebParams {
                stress: Some(100),
                stress_widgets: Some(10),
                log_level: None,
                canvas: Some("x.canvas".to_string()),
                template: None
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

    /// W6: `?canvas=` — имя канваса в OPFS; суффикс .canvas добавляется.
    #[test]
    fn canvas_param_parses_and_appends_suffix() {
        let params = parse_query("?canvas=проект-альфа").expect("валидный запрос");
        assert_eq!(params.canvas.as_deref(), Some("проект-альфа.canvas"));
        // Явный суффикс сохраняется как есть (регистр не трогаем)
        let params = parse_query("?canvas=Notes.CANVAS").expect("валидный запрос");
        assert_eq!(params.canvas.as_deref(), Some("Notes.CANVAS"));
    }

    /// W6: опасные имена отклоняются мягко (None) — URL-инъекции в
    /// плоскую ФС/логи не проходят, страница открывается без параметра.
    #[test]
    fn canvas_param_sanitizes() {
        // Тривиальные отказы
        assert_eq!(super::sanitize_canvas_name(""), None);
        assert_eq!(super::sanitize_canvas_name("   "), None);
        assert_eq!(super::sanitize_canvas_name(".."), None);
        assert_eq!(super::sanitize_canvas_name("a/b"), None);
        assert_eq!(super::sanitize_canvas_name("a\\\\b"), None);
        assert_eq!(super::sanitize_canvas_name(".hidden"), None);
        assert_eq!(super::sanitize_canvas_name("name."), None);
        assert_eq!(super::sanitize_canvas_name("смета?x"), None);
        assert_eq!(super::sanitize_canvas_name(&"x".repeat(81)), None);
        // Управляющий символ внутри
        assert_eq!(super::sanitize_canvas_name("a\nb"), None);
        // Верхняя граница длины проходит (80 символов + суффикс)
        assert!(super::sanitize_canvas_name(&"x".repeat(80)).is_some());
        // Битый canvas не роняет соседний валидный stress
        let params = parse_query("?stress=5&canvas=a%2Fb").expect("валидный запрос");
        assert_eq!(params.stress, Some(5));
        assert_eq!(params.canvas, None);
    }

    /// W6: процентов-декод — браузер кодирует кириллицу в location.search
    /// (`?canvas=w6-%D0%B8%D0%BC%D1%8F`), декод вернёт «w6-имя».
    #[test]
    fn canvas_param_percent_decoded() {
        let params = parse_query("?canvas=w6-%D0%B8%D0%BC%D1%8F").expect("валидный запрос");
        assert_eq!(params.canvas.as_deref(), Some("w6-имя.canvas"));
        // Некорректный % остаётся как есть (потом отвергнет санитизация)
        let params = parse_query("?canvas=a%zz").expect("валидный запрос");
        assert_eq!(
            params.canvas, None,
            "«%zz» не декодируется — санитизация отвергла"
        );
    }

    /// W6: canvas комбинируется с остальными параметрами дыма.
    #[test]
    fn canvas_combines_with_stress_and_log() {
        let params = parse_query("?canvas=демо&stress=100&log=debug").expect("валидный запрос");
        assert_eq!(params.canvas.as_deref(), Some("демо.canvas"));
        assert_eq!(params.stress, Some(100));
        assert_eq!(params.log_level, Some(LogLevel::Debug));
    }

    /// W10: имя произвольного файла — расширение сохраняется, суффикс
    /// не дописывается; опасные имена отклоняются целиком.
    #[test]
    fn file_name_sanitize_keeps_extension() {
        use super::sanitize_file_name;
        assert_eq!(
            sanitize_file_name("фото лето.png").as_deref(),
            Some("фото лето.png")
        );
        assert_eq!(sanitize_file_name(" a.PNG ").as_deref(), Some("a.PNG"));
        // Опасные — None (как у канвасов)
        assert_eq!(sanitize_file_name("../etc/passwd"), None);
        assert_eq!(sanitize_file_name("a/b.png"), None);
        assert_eq!(sanitize_file_name(".hidden"), None);
        assert_eq!(sanitize_file_name("name."), None);
        assert_eq!(sanitize_file_name("a?b.png"), None);
        assert_eq!(sanitize_file_name(""), None);
    }

    /// W10: коллизии имён при приёме в OPFS — суффикс -N, расширение
    /// сохраняется; ведущая точка — часть имени (не стем).
    #[test]
    fn file_name_candidates_avoid_collisions() {
        use super::file_name_candidates;
        let c = file_name_candidates("pic.png", 3);
        assert_eq!(c, vec!["pic.png", "pic-1.png", "pic-2.png"]);
        let c = file_name_candidates("notes", 2);
        assert_eq!(c, vec!["notes", "notes-1"]);
        let c = file_name_candidates(".gitignore", 2);
        assert_eq!(c, vec![".gitignore", ".gitignore-1"]);
    }
}
