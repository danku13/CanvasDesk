//! Табличная модель тела ноды (D-2 FR-061, Н-3 пре-PRD PRD-0004) —
//! декларативное описание строк данных + проход A двухпроходной раскладки
//! (замер направляющих, лестница деградации бейджей §3.4).
//!
//! Архитектурное правило FR-061 (приказ владельца): потребители (`text.rs`)
//! дают ДАННЫЕ (текст ноды, результаты `expr`, проливания, авто-строки),
//! каркас (`row_grid`) считает ГЕОМЕТРИЮ (направляющие, режим бейджей);
//! исполнение (буферы/квады) — `text.rs`/`renderer.rs`. Разбор рода строки
//! — ТОЛЬКО через [`canvas_core::expr::line_kind`], грамматика не
//! дублируется (риск №2 анализа §8).
//!
//! Слои: домен (части значения) — `canvas-core`; замер (TextMeasurer,
//! RowGuides) — `canvas-ui` (`measure_row_cells`, D-3, контракты этапа A);
//! модель строки и проход A — здесь; рендер — `text.rs` (этап B).
//!
//! Инварианты: текст ноды — единственный источник истины (I-3); хром
//! таблицы не меняет Y-ряд (I-1); детерминизм прохода A (повторный кадр —
//! идентичный результат, D-11).

use canvas_core::expr::{line_kind, NumiLineKind};
use canvas_core::Language;
use canvas_ui::measure::TextMeasurer;
use canvas_ui::row_guides::{measure_row_cells, RowGuides};

use crate::SpillView;

/// Вес замера ячеек таблицы — паритет `mono_attrs()` рендера (Weight 400).
/// Приёмка T9 FR-061: cosmic-text ищет лицо семейства только среди лиц
/// ТОЧНОГО веса запроса (`get_font_matches` → `font_weight_diff == 0`);
/// у Noto Sans Mono лица 400/700, и запрос MEDIUM промахивался мимо
/// семейства целиком — замер уходил в системный шрифт того же веса
/// («rps» 19.2 px против рендера 21.6 px) → направляющие заужены →
/// юниты/числа налезали на ячейки. Замер ячеек — только NORMAL.
pub(crate) const MEASURE_WEIGHT: cosmic_text::Weight = cosmic_text::Weight::NORMAL;

/// Зазор между ячейками «значение»/«юнит»/«бейдж» (world-px) — параметр
/// [`pass_a`]; токен D-14 [`canvas_core::tokens::TABLE_GUIDE_GAP`] (анализ
/// §3.1: единая система отсчёта).
pub(crate) const GUIDE_GAP: f32 = canvas_core::tokens::TABLE_GUIDE_GAP;
/// Минимальная дорожка лидера (world-px): короче — лидер не рисуется.
pub(crate) const LEADER_MIN: f32 = canvas_core::tokens::TABLE_LEADER_MIN;
/// Зазор лидера до ячейки значения и от конца левого текста (world-px).
pub(crate) const LEADER_PAD: f32 = canvas_core::tokens::TABLE_LEADER_PAD;

/// Род строки данных — селектор хрома зоны (фон/начертание). Y-ряд не
/// меняет (I-1): высота строки задаётся блоком тела, не родом.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowKind {
    /// Авто-строка приёмника (префикс «Переменные · входящие значения»,
    /// Р-4) — источник значения upstream, наклонное начертание.
    Auto,
    /// Строка-присваивание параметра (`name = литерал`) с результатом.
    Param,
    /// Формульная строка (выражение) с результатом.
    Calc,
    /// D-7/D-9 (этап C): заголовок блока-ведомости «▸ расчёт · N строк»
    /// (Н-2) — Σ узлового итога на направляющей чисел, как в прототипе.
    Total,
    /// FR-061 хвосты (D-7, runtime v1): превью-строка СВЁРНУТОЙ ведомости
    /// «параметры · P · формулы · K» (слева) + «Σ первое-значение» на
    /// направляющей (прототип §3.5, .preview-row).
    Preview,
}

/// Бейдж каскада Р-1 в бейдж-колонке (D-6, анализ §3.1): примечания полосы D
/// в колонку НЕ входят. Иконный режим деградации — [`BadgeMode::Icon`].
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RowBadge {
    /// Проливание: подпись источника «← Объект» (+ «· выход»).
    Spill { label: String },
    /// What-if: дельта сценария «+12 пп» / «+100 rps» (янтарный).
    Delta(String),
    /// Ошибка строки: «!» (полный текст — тултип, механика FR-013 пр.4).
    Error,
}

impl RowBadge {
    /// Текст бейджа в текстовом режиме.
    pub(crate) fn text(&self) -> &str {
        match self {
            RowBadge::Spill { label } => label,
            RowBadge::Delta(delta) => delta,
            RowBadge::Error => "!",
        }
    }

    /// Глиф бейджа в иконном режиме деградации (ASCII-безопасный, моно).
    pub(crate) fn icon(&self) -> &'static str {
        match self {
            RowBadge::Spill { .. } => "<",
            RowBadge::Delta(_) => "d",
            RowBadge::Error => "!",
        }
    }
}

/// Декларативное описание одной строки данных ноды (D-2): ячейки
/// имя/значение/юнит/бейдж. Левая часть (имя + «=» + формула) рисуется
/// блоком тела (существующий буфер, I-1); ячейки справа — по направляющим.
/// Поля `name`/`formula` — источник истины для теста инварианта ширины T2
/// и kit-Row этапа E.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RowCells {
    pub kind: RowKind,
    /// Индекс строки текста ноды (Param/Calc — привязка блока/порта);
    /// `None` — авто-строка префикса (порты/якоря не даёт).
    pub source_line: Option<usize>,
    /// Ячейка «имя»: путь авто-строки / имя параметра (Calc — пусто).
    pub name: String,
    /// Формульная часть строки (RHS после «=»; T2/этап E).
    pub formula: String,
    /// Ячейка «значение»: число (части D-1) или полный текст, если части
    /// недоступны (проливание — строка источника, what-if «стало»).
    pub value: String,
    /// Ячейка «юнит» (пусто — значение целиком в ячейке значения).
    pub unit: String,
    /// Значение из upstream (проливание/авто-строка) — наклонное начертание
    /// Р-2 (визуальное отличие пришедшего по связи).
    pub upstream: bool,
    /// Приглушённое значение (unmapped «—», Р-3).
    pub dim_value: bool,
    /// Бейдж каскада (None — колонка пуста).
    pub badge: Option<RowBadge>,
    /// Полный текст ошибки (тултип «!»).
    pub error_message: Option<String>,
}

/// Режим бейдж-колонки — ступень лестницы деградации §3.4: сначала
/// жертвуем текстом бейджей (→ иконки), затем колонкой целиком; числа
/// и юниты не деградируют никогда.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BadgeMode {
    /// Текстовые бейджи (полная таблица).
    Text,
    /// Иконные бейджи (узкие ноды, L3).
    Icon,
    /// Колонка скрыта (совсем тесно; формулы остаются целиком — ellipsis
    /// формулы отложен в этап C/D, здесь колонка просто уходит).
    None,
}

/// Результат прохода A: направляющие ноды + режим бейджей (детерминированы
/// входами — D-11 кладёт их отпечаток в ключ кэша).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TablePass {
    pub guides: Option<RowGuides>,
    pub badge_mode: BadgeMode,
    /// FR-061 коммит 3 (последняя ступень лестницы §3.4 + Q8): пер-строчный
    /// план усечения формул узкой ноды — алиасы идентификаторов, затем
    /// хвостовой ellipsis; полная формула — в тултип строки. Выровнен по
    /// `rows`; `None` — строка без изменений. Детерминирован входами (D-11).
    pub ellipsis: Vec<Option<RowEllipsis>>,
}

/// FR-061 коммит 3: усечённое отображение формулы (лестница §3.4, ступень
/// «совсем тесно»): `display` — текст строки для рендера (замещает сегмент
/// блока тела; имя/числа/юниты не трогаются — всегда читаемы), `full` —
/// полная формула для тултипа строки. Полное имя переменной — в строке
/// параметра (решение Q8).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RowEllipsis {
    pub display: String,
    pub full: String,
}

/// Q8 FR-061: порог алиаса идентификатора формулы (символов, mono).
/// Длиннее — «авто-обрезка имени»: первые [`ALIAS_IDENT_KEEP`] симв. + «…».
pub(crate) const ALIAS_IDENT_MAX: usize = 16;
/// Q8: сколько символов идентификатора остаётся при алиасе.
const ALIAS_IDENT_KEEP: usize = 13;

/// Q8 FR-061 («авто-обрезка имени», решение владельца): алиасы длинных
/// идентификаторов формулы. Идентификатор = токен из букв/цифр/«_»,
/// начинающийся с буквы или «_» (юникод — кириллица входит); длиннее
/// [`ALIAS_IDENT_MAX`] — обрезается до [`ALIAS_IDENT_KEEP`] + «…».
/// Числовые литералы не трогаются (лестница §3.4: числа не деградируют).
/// Грамматика не дублируется — чисто визуальная классификация символов
/// (прецедент O-5 `formula_rich_runs`). Полное имя остаётся в строке
/// параметра, полная формула — в тултипе расчётной строки.
pub(crate) fn alias_idents(formula: &str) -> String {
    let mut out = String::with_capacity(formula.len());
    let mut token = String::new();
    for c in formula.chars() {
        if c.is_alphanumeric() || c == '_' {
            token.push(c);
        } else {
            flush_ident(&mut token, &mut out);
            out.push(c);
        }
    }
    flush_ident(&mut token, &mut out);
    out
}

/// Хвост [`alias_idents`]: токен — идентификатор длиннее порога → алиас.
fn flush_ident(token: &mut String, out: &mut String) {
    if token.is_empty() {
        return;
    }
    let is_ident = token
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
        && token.chars().all(|c| c.is_alphanumeric() || c == '_');
    if is_ident && token.chars().count() > ALIAS_IDENT_MAX {
        let kept: String = token.chars().take(ALIAS_IDENT_KEEP).collect();
        out.push_str(&kept);
        out.push('…');
    } else {
        out.push_str(token);
    }
    token.clear();
}

/// Отпечаток прохода A для ключа свежести кэша (D-11): ширины направляющих
/// (0.1 px) и режим бейджей. Этап B: направляющие — чистая функция входов
/// ключа (текст/ширина/зум/исходы), отпечаток не нужен; ЭТАП C активирует
/// его, когда режим блока/кламп/порог T станут runtime-состоянием (D-7).
#[allow(dead_code)]
pub(crate) fn pass_key(pass: &TablePass) -> String {
    match &pass.guides {
        Some(g) => format!(
            "G:{:.1},{:.1},{:.1}:{:?}",
            g.value_w, g.unit_w, g.badge_w, pass.badge_mode
        ),
        None => String::new(),
    }
}

/// D-7 (этап C): число расчётных строк (Expression-исходы) — N заголовка
/// блока-ведомости; параметры/авто-строки не в счёт.
pub(crate) fn calc_row_count(rows: &[RowCells]) -> usize {
    rows.iter().filter(|row| row.kind == RowKind::Calc).count()
}

/// D-7 (этап C): режим тела Н-3 — сквозной лист (Н-1) при
/// `data_count <= T`, блок-ведомость (Н-2) при большем. Число строк
/// данных = параметры + расчёт (авто-строки не в счёт — диагностика);
/// заголовок «расчёта» имеет смысл только при наличии расчётных строк.
pub(crate) fn block_mode(rows: &[RowCells], threshold: usize) -> bool {
    let data_count = rows
        .iter()
        .filter(|row| matches!(row.kind, RowKind::Param | RowKind::Calc))
        .count();
    data_count > threshold && calc_row_count(rows) > 0
}

/// D-7 (этап C): текст заголовка блока-ведомости «▸ расчёт · N строк» —
/// единая точка сборки для рендера и тестов; русская плюрализация
/// (1 строка / 2–4 строки / 5+ строк). D-14 (этап D): обёртка над
/// [`block_header_text_lang`] с языком по умолчанию (RU) — обратная
/// совместимость тестов/вызовов.
pub(crate) fn block_header_text(calc_count: usize) -> String {
    block_header_text_lang(calc_count, Language::Ru, true)
}

/// D-14 (этап D): локализованный заголовок блока-ведомости. RU —
/// плюрализация «строка/строки/строк»; EN — «line/lines». Высота строки
/// от языка не зависит (одна строка — I-2 не страдает).
/// FR-061 хвосты (D-7 runtime v1): шеврон состояния — «▾» развёрнут
/// (дефолт продукта), «▸» свёрнут (прототип .blk-hdr, cursor:pointer).
pub(crate) fn block_header_text_lang(
    calc_count: usize,
    language: Language,
    expanded: bool,
) -> String {
    let chevron = if expanded { "▾" } else { "▸" };
    match language {
        Language::En => {
            let noun = if calc_count == 1 { "line" } else { "lines" };
            format!("{chevron} calc · {calc_count} {noun}")
        }
        Language::Ru => {
            let noun = match (calc_count % 10, calc_count % 100) {
                (1, 11) => "строк",
                (1, _) => "строка",
                (2..=4, 11..=14) => "строк",
                (2..=4, _) => "строки",
                _ => "строк",
            };
            format!("{chevron} расчёт · {calc_count} {noun}")
        }
    }
}

/// FR-061 хвосты (D-7 runtime v1): локализованный текст превью-строки
/// свёрнутой ведомости — «параметры · P · формулы · K» (прототип
/// .preview-row). RU — плюрализация «строка/строки/строк», EN —
/// «line/lines».
pub(crate) fn block_preview_text_lang(
    param_count: usize,
    calc_count: usize,
    language: Language,
) -> String {
    match language {
        Language::En => {
            let noun = if calc_count == 1 { "line" } else { "lines" };
            format!("params · {param_count} · formulas · {calc_count} {noun}")
        }
        Language::Ru => {
            let noun = match (calc_count % 10, calc_count % 100) {
                (1, 11) => "строк",
                (1, _) => "строка",
                (2..=4, 11..=14) => "строк",
                (2..=4, _) => "строки",
                _ => "строк",
            };
            format!("параметры · {param_count} · формулы · {calc_count} {noun}")
        }
    }
}

/// FR-061 хвосты (D-8 runtime v1, «Раскрыть+авто»): аффорданс экспандера
/// описания — свёрнуто и обрезано («⋯ целиком ▾», клик раскрывает).
pub(crate) fn desc_expand_text_lang(language: Language) -> String {
    match language {
        Language::En => "⋯ show all ▾".to_owned(),
        Language::Ru => "⋯ целиком ▾".to_owned(),
    }
}

/// FR-061 хвосты (D-8 runtime v1): аффорданс экспандера описания —
/// раскрыто («▴ свернуть»; автосворачивание — клик вне ноды/начало правки).
pub(crate) fn desc_collapse_text_lang(language: Language) -> String {
    match language {
        Language::En => "▴ collapse".to_owned(),
        Language::Ru => "▴ свернуть".to_owned(),
    }
}

/// «было → стало (+Δ)» → («стало», «Δ») — структурный разбор полного
/// дельта-формата [`canvas_core::expr::whatif_full_delta`] для бейдж-колонки
/// этапа B (D-1: структурные части. Приложение передаёт строку — рендер
/// разбираёт её детерминированно; контракт формата залочен тестом на
/// корпусе `whatif_full_delta`). `None` — формат не распознан (fallback:
/// строка целиком в ячейке значения).
pub(crate) fn split_full_delta(s: &str) -> Option<(String, String)> {
    let (_base, rest) = s.split_once(" → ")?;
    let (new, delta) = rest.rsplit_once(" (")?;
    if !delta.ends_with(')') || new.is_empty() {
        return None;
    }
    Some((new.to_owned(), delta[..delta.len() - 1].to_owned()))
}

/// RHS строки-присваивания (после первого «=» вне литералов) — формульная
/// часть для [`RowCells::formula`] (T2/этап E). Экранированный `\=` —
/// литерал, не оператор; `==` — сравнение, не присваивание (семантика
/// детектора рода строки).
pub(crate) fn assignment_rhs(line: &str) -> String {
    let bytes = line.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'=' {
            let escaped = i > 0 && bytes[i - 1] == b'\\';
            let is_eq_eq = i + 1 < bytes.len() && bytes[i + 1] == b'=';
            if !escaped && !is_eq_eq {
                return line[i + 1..].trim().to_owned();
            }
        }
    }
    String::new()
}

/// Подпись бейджа проливания: «← Источник» (+ «· выход»).
fn spill_label(view: &SpillView) -> String {
    match &view.from_output {
        Some(output) => format!("← {} · {}", view.from_label, output),
        None => format!("← {}", view.from_label),
    }
}

/// Собрать табличные строки ноды (D-2) из источников истины: текст ноды
/// (виртуальный при what-if — 1:1 к базовому), построчные результаты
/// `expr` (база), дельты what-if (FR-017), проливания параметров (FR-029),
/// авто-строки приёмника (FR-050 Р-4). Проза/заголовки/списки строками
/// данных не становятся. Проливание/what-if показываются ТОЛЬКО на строках
/// с результатом (прежнее правило FR-013 пр.2: цикл по outcomes).
pub(crate) fn build_rows(
    body_text: &str,
    line_outcomes: Option<&[Option<canvas_core::expr::ExprOutcome>]>,
    whatif_deltas: &[(usize, String)],
    spill_views: &[SpillView],
    auto_rows: &[canvas_core::flow::AutoRow],
) -> Vec<RowCells> {
    // Авто-строки — префикс таблицы (Р-4): части D-1 без повторного парсинга.
    let mut rows: Vec<RowCells> = auto_rows
        .iter()
        .map(|row| {
            let parts = row.display_parts();
            let unmapped = row.value.is_none();
            RowCells {
                kind: RowKind::Auto,
                source_line: None,
                name: parts.path,
                formula: String::new(),
                value: parts.num,
                unit: parts.unit,
                upstream: true,
                dim_value: unmapped,
                badge: None,
                error_message: None,
            }
        })
        .collect();

    let Some(outcomes) = line_outcomes else {
        return rows;
    };
    let spill_by_line: std::collections::HashMap<usize, &SpillView> = spill_views
        .iter()
        .filter_map(|view| view.line.map(|line| (line, view)))
        .collect();
    let whatif_by_line: std::collections::HashMap<usize, &String> = whatif_deltas
        .iter()
        .map(|(line, delta)| (*line, delta))
        .collect();

    let lines: Vec<&str> = body_text.split('\n').collect();
    for (i, outcome) in outcomes.iter().enumerate() {
        let Some(outcome) = outcome else { continue };
        // Род строки — детектор движка (грамматика не дублируется).
        let Some(raw) = lines.get(i) else { continue };
        let (kind, name) = match line_kind(raw) {
            NumiLineKind::Assignment { name } => (RowKind::Param, Some(name)),
            // Строка с исходом без присваивания — расчётная. line_kind
            // смотрит с ПУСТЫМ окружением: переменные листа он называет
            // прозой («a * 2» при `a` выше), но движок ноды с окружением
            // дал результат — исход важнее детектора (дубля грамматики
            // нет: имя здесь не нужно, результат уже вычислен движком).
            _ => (RowKind::Calc, None),
        };
        // Формульная часть: присваивание — RHS, выражение — вся строка.
        let formula = match kind {
            RowKind::Param => assignment_rhs(raw),
            _ => raw.trim().to_owned(),
        };
        let mut row = match outcome {
            canvas_core::expr::ExprOutcome::Ok(value) => {
                let (num, unit) = value.display_parts();
                RowCells {
                    kind,
                    source_line: Some(i),
                    name: name.unwrap_or_default(),
                    formula,
                    value: num,
                    unit,
                    upstream: false,
                    dim_value: false,
                    badge: None,
                    error_message: None,
                }
            }
            canvas_core::expr::ExprOutcome::Err(message) => RowCells {
                kind,
                source_line: Some(i),
                name: name.unwrap_or_default(),
                formula,
                value: String::new(),
                unit: String::new(),
                upstream: false,
                dim_value: false,
                badge: Some(RowBadge::Error),
                error_message: Some(message.clone()),
            },
        };
        // FR-017/FR-029: приоритет отображения — what-if дельта, затем
        // проливание, затем локальный результат (прежнее правило FR-013).
        let whatif_delta: Option<&String> = whatif_by_line.get(&i).copied();
        if let Some(delta) = whatif_delta {
            match split_full_delta(delta) {
                Some((new, delta)) => {
                    row.value = new;
                    row.unit = String::new();
                    row.badge = Some(RowBadge::Delta(delta));
                }
                None => {
                    row.value = delta.clone();
                    row.unit = String::new();
                }
            }
        } else if let Some(view) = spill_by_line.get(&i) {
            if let Some(value) = &view.value {
                // Пролитое значение — строка источника (уже отформатирована
                // «число юнит»): ячейка значения целиком, юнит-колонка пуста
                // (без обратного парсинга — риск дубля грамматики §8).
                row.value = value.clone();
                row.unit = String::new();
                row.upstream = true;
                row.badge = Some(RowBadge::Spill {
                    label: spill_label(view),
                });
            }
        }
        rows.push(row);
    }
    rows
}

/// Естественные ширины ячеек строки (проход A, §3.2): значение/юнит —
/// [`measure_row_cells`] (TextMeasurer F-6, контракт этапа A), бейдж —
/// [`TextMeasurer::width_of`] по тексту/иконке режима.
fn cell_widths(
    measurer: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    row: &RowCells,
    badge_mode: BadgeMode,
    family: &str,
    size: f32,
) -> canvas_ui::row_guides::RowCellWidths {
    let cells = measure_row_cells(
        measurer,
        fs,
        &row.value,
        &row.unit,
        0.0,
        family,
        size,
        MEASURE_WEIGHT,
    );
    let badge_w = match (&row.badge, badge_mode) {
        (None, _) => 0.0,
        (Some(badge), BadgeMode::Text) => {
            measurer.width_of_weighted(fs, badge.text(), family, size, MEASURE_WEIGHT)
        }
        (Some(badge), BadgeMode::Icon) => {
            measurer.width_of_weighted(fs, badge.icon(), family, size, MEASURE_WEIGHT)
        }
        (Some(_), BadgeMode::None) => 0.0,
    };
    canvas_ui::row_guides::RowCellWidths {
        value_w: cells.value_w,
        unit_w: cells.unit_w,
        badge_w,
    }
}

/// Проход A (§3.2 + лестница §3.4): замер ячеек по всем строкам →
/// направляющие ноды ([`RowGuides::measure`] + [`RowGuides::with_right_edge`]);
/// детект переполнения — точная арифметика ширин против `body_width`,
/// деградация бейджей Text → Icon → None (числа/юниты не деградируют);
/// после исчерпания колонки бейджей — усечение формул (коммит 3: алиасы
/// Q8 + хвостовой ellipsis, [`plan_row_ellipsis`]).
/// Детерминизм: одинаковые входы → идентичный [`TablePass`] (D-11).
/// `left_max` — правый край самого широкого левого текста (имя+формула,
/// world-px; конец лидера не правее `left_max + [`LEADER_PAD`]`).
/// `floor` — нижняя ступень лестницы (повторный проход после перешейпа
/// с усечённым отображением не поднимается обратно — план стабилен).
/// `prior` — уже применённый план усечения (выравнен по `rows`); строки
/// с планом не пере-планируются (алиас/ellipsis фиксированы за билд).
/// `size` — кегль ЯЧЕЕК (результаты, mono 12); `left_size` — кегль ЛЕВОГО
/// текста строк (тело, mono 14 — приёмка T9: план усечения обязан мерить
/// левый текст его фактическим кеглем, иначе бюджет дорожки завышен
/// на (left_size/size − 1) и усечённая строка всё равно наезжает).
#[allow(clippy::too_many_arguments)]
pub(crate) fn pass_a(
    measurer: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    rows: &[RowCells],
    left_max: f32,
    body_width: f32,
    family: &str,
    size: f32,
    left_size: f32,
    floor: BadgeMode,
    prior: &[Option<RowEllipsis>],
) -> TablePass {
    debug_assert!(
        prior.is_empty() || prior.len() == rows.len(),
        "prior выровнен по rows (или пуст)"
    );
    let mut mode = floor;
    loop {
        let widths: Vec<canvas_ui::row_guides::RowCellWidths> = rows
            .iter()
            .map(|row| cell_widths(measurer, fs, row, mode, family, size))
            .collect();
        let guides = RowGuides::measure(&widths).map(|g| g.with_right_edge(body_width, GUIDE_GAP));
        let fits = match &guides {
            // Нет строк данных — таблицы нет, режим не важен.
            None => true,
            Some(g) => {
                // T2: левый текст + лидер не наезжают на ячейку значения.
                let leader_start = left_max + LEADER_PAD;
                leader_start + LEADER_MIN <= g.value_x
            }
        };
        if fits {
            return TablePass {
                guides,
                badge_mode: mode,
                ellipsis: prior.to_vec(),
            };
        }
        match mode {
            BadgeMode::Text => mode = BadgeMode::Icon,
            BadgeMode::Icon => mode = BadgeMode::None,
            // Совсем тесно: колонка скрыта; если и после этого лидер не
            // помещается — последняя ступень §3.4: усечение формул
            // (алиасы Q8 → хвостовой ellipsis; полное — в тултип).
            BadgeMode::None => {
                let widths: Vec<canvas_ui::row_guides::RowCellWidths> = rows
                    .iter()
                    .map(|row| cell_widths(measurer, fs, row, mode, family, size))
                    .collect();
                let guides =
                    RowGuides::measure(&widths).map(|g| g.with_right_edge(body_width, GUIDE_GAP));
                let ellipsis = match &guides {
                    None => prior.to_vec(),
                    Some(g) if left_max + LEADER_PAD + LEADER_MIN <= g.value_x => prior.to_vec(),
                    Some(g) => {
                        // Доступная дорожка левого текста: до минимума лидера.
                        let available = g.value_x - LEADER_PAD - LEADER_MIN;
                        rows.iter()
                            .enumerate()
                            .map(|(i, row)| {
                                prior.get(i).cloned().flatten().or_else(|| {
                                    plan_row_ellipsis(
                                        measurer, fs, row, available, family, left_size,
                                    )
                                })
                            })
                            .collect()
                    }
                };
                return TablePass {
                    guides,
                    badge_mode: mode,
                    ellipsis,
                };
            }
        }
    }
}

/// FR-061 коммит 3: план усечения ОДНОЙ строки (последняя ступень §3.4).
/// Расчётные строки (Calc) — усечение формулы целиком: имя параметра/путь
/// авто-строки не деградируют («имя+числа+юниты всегда читаемы»).
/// Приёмка T9 FR-061: присваивания с ВЫРАЖЕНИЕМ в RHS (`load = a / b`) —
/// тоже формульные строки (прототип: calcRow = [имя][=][формула]); их RHS
/// деградирует с ЗАЩИЩЁННЫМ префиксом «имя =» (само имя не трогается),
/// иначе левый текст наезжает на ячейки (нечем спасать — Param ранее не
/// планировался вовсе). Литеральные RHS — числа: не деградируют никогда
/// ([`is_literal_rhs`]). Порядок: алиасы идентификаторов (Q8) → хвостовой
/// ellipsis ([`TextMeasurer::ellipsis_weighted`], детерминированный
/// бинарный поиск).
fn plan_row_ellipsis(
    measurer: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    row: &RowCells,
    available: f32,
    family: &str,
    size: f32,
) -> Option<RowEllipsis> {
    if available <= 0.0 || row.formula.is_empty() {
        return None;
    }
    match row.kind {
        RowKind::Calc => plan_text_ellipsis(
            measurer,
            fs,
            &row.formula,
            String::new(),
            available,
            family,
            size,
        ),
        RowKind::Param if !is_literal_rhs(row) => {
            // Префикс «имя =» защищён; деградирует только RHS-формула.
            let prefix = format!("{} = ", row.name);
            plan_text_ellipsis(measurer, fs, &row.formula, prefix, available, family, size)
        }
        // Параметры с литеральным RHS (числа) и авто-строки не усекаются.
        _ => None,
    }
}

/// Усечь «prefix + formula» в дорожку `available`: алиасы идентификаторов
/// → хвостовой ellipsis; префикс (например «имя = ») не усекается.
/// `full` тултипа — prefix + полная формула. `None` — влезает целиком.
fn plan_text_ellipsis(
    measurer: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    formula: &str,
    prefix: String,
    available: f32,
    family: &str,
    size: f32,
) -> Option<RowEllipsis> {
    let full = format!("{prefix}{formula}");
    if measurer.width_of_weighted(fs, &full, family, size, MEASURE_WEIGHT) <= available {
        return None; // влезает целиком — усечение не нужно
    }
    let aliased = alias_idents(formula);
    let base = format!("{prefix}{aliased}");
    let display =
        if measurer.width_of_weighted(fs, &base, family, size, MEASURE_WEIGHT) <= available {
            base
        } else {
            let track = (available
                - measurer.width_of_weighted(fs, &prefix, family, size, MEASURE_WEIGHT))
            .max(0.0);
            let rhs = measurer.ellipsis_weighted(fs, &aliased, family, size, track, MEASURE_WEIGHT);
            if rhs.is_empty() {
                return None; // даже «…» не влезает — строку не трогаем (тултип-зона не строится)
            }
            format!("{prefix}{rhs}")
        };
    if display.is_empty() || display == full {
        return None;
    }
    Some(RowEllipsis { display, full })
}

/// Литеральный RHS присваивания — значение ячейки повторяет формулу
/// дословно (с точностью до пробелов: «800 rps» = value+unit). У таких
/// строк RHS — числа/юнит: не деградирует никогда (§3.4), а в левой части
/// литерал УБИРАЕТСЯ strip-переопределением ([`param_strip_overrides`]) —
/// значение показывается один раз, в ячейке (прототип: paramRow без
/// формулы). Выражение в RHS («a + b») литералом не считается.
pub(crate) fn is_literal_rhs(row: &RowCells) -> bool {
    if row.kind != RowKind::Param || row.value.is_empty() || row.formula.is_empty() {
        return false;
    }
    let value_unit = if row.unit.is_empty() {
        row.value.clone()
    } else {
        format!("{} {}", row.value, row.unit)
    };
    row.formula.split_whitespace().collect::<Vec<_>>().join(" ")
        == value_unit.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Strip-переопределения левой части Param-строк с литеральным RHS
/// (приёмка T9 FR-061, дублирование текста): сегмент тела `servers = 3`
/// замещается «servers =» — литерал показывается ТОЛЬКО в ячейке значения
/// (прототип: paramRow = [имя][=][лидер]|[значение]). Возвращает список
/// (source_line, текст); применяется как overrides `body_items` (тот же
/// канал, что усечение формул — строки не пересекаются: Param/Calc).
/// Пролитые/what-if строки не трогаются (там левая часть — подпись
/// источника, литерального дубля нет).
pub(crate) fn param_strip_overrides(rows: &[RowCells]) -> Vec<(usize, String)> {
    rows.iter()
        .filter(|row| {
            row.source_line.is_some() && row.badge.is_none() && !row.upstream && is_literal_rhs(row)
        })
        .map(|row| {
            (
                row.source_line.unwrap_or_default(),
                format!("{} =", row.name),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Детерминированный FontSystem: только вшитый рендером моно-шрифт
    /// (метрики тестов = метрикам рендера, практика row_guides.rs).
    fn font_system() -> cosmic_text::FontSystem {
        let mut fs = cosmic_text::FontSystem::new();
        const FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSansMono-Regular.ttf");
        fs.db_mut().load_font_data(FONT.to_vec());
        fs
    }

    const FAMILY: &str = "Noto Sans Mono";
    const SIZE: f32 = 12.0;

    /// Исходы реальным движком (`eval_lines`) — контракт D-2 «разбор строк
    /// консистентен с парсером» проверяется на настоящих исходах.
    fn outcomes(source: &str) -> Vec<Option<canvas_core::expr::ExprOutcome>> {
        canvas_core::expr::eval_lines(source)
    }

    /// Значение с юнитом из реального выражения (для авто-строк).
    fn value_of(source: &str) -> canvas_core::expr::Value {
        match canvas_core::expr::eval_lines(source)[0].as_ref().unwrap() {
            canvas_core::expr::ExprOutcome::Ok(value) => value.clone(),
            other => panic!("ожидался Ok, получен {other:?}"),
        }
    }

    /// D-2: присваивание с результатом → Param-строка с частями D-1;
    /// формульная строка → Calc; проза/строки без исхода — не строки данных.
    #[test]
    fn build_rows_splits_params_and_calcs() {
        let text = "Веб-сервис\nrps = 800 rps\n800 rps / 12 ms\nзаметка без кода";
        let rows = build_rows(text, Some(&outcomes(text)), &[], &[], &[]);
        assert_eq!(rows.len(), 2, "только строки с исходом");
        assert_eq!(rows[0].kind, RowKind::Param);
        assert_eq!(rows[0].name, "rps");
        assert_eq!(rows[0].value, "800");
        assert_eq!(rows[0].unit, "rps");
        assert_eq!(rows[0].source_line, Some(1));
        assert_eq!(rows[0].formula, "800 rps");
        assert_eq!(rows[1].kind, RowKind::Calc);
        assert_eq!(rows[1].formula, "800 rps / 12 ms");
        assert!(rows[1].name.is_empty(), "у выражения имени нет");
        assert!(!rows[1].value.is_empty());
    }

    /// D-2: авто-строки — префикс с частями D-1; unmapped — «—» приглушено.
    #[test]
    fn build_rows_prefixes_auto_rows() {
        let auto = canvas_core::flow::AutoRow {
            node_id: "n1".into(),
            edge_id: "e1".into(),
            slot: 0,
            path: "Профиль.peak_rps".into(),
            field: "peak_rps".into(),
            value: Some(value_of("1200 rps")),
        };
        let rows = build_rows("", None, &[], &[], &[auto]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, RowKind::Auto);
        assert_eq!(rows[0].name, "Профиль.peak_rps");
        assert_eq!(rows[0].value, "1200");
        assert_eq!(rows[0].unit, "rps");
        assert!(rows[0].upstream);
        assert!(!rows[0].dim_value);
        assert!(rows[0].source_line.is_none());
        // Unmapped — «—» в ячейке числа (Р-3), приглушено
        let unmapped = canvas_core::flow::AutoRow {
            node_id: "n1".into(),
            edge_id: "e2".into(),
            slot: 1,
            path: "Профиль.missing".into(),
            field: "missing".into(),
            value: None,
        };
        let rows = build_rows("", None, &[], &[], &[unmapped]);
        assert_eq!(rows[0].value, "—");
        assert!(rows[0].dim_value);
    }

    /// D-6: what-if — ячейка значения = «стало», бейдж = Δ; проливание —
    /// значение источника + бейдж «← источник»; приоритет what-if выше.
    #[test]
    fn build_rows_maps_whatif_and_spill_badges() {
        let text = "rps = 800";
        let outcomes = outcomes(text);
        // What-if: полный дельта-формат whatif_full_delta (контракт canvas-core)
        let whatif = vec![(0, "800 rps → 900 rps (+100 rps)".to_owned())];
        let rows = build_rows(text, Some(&outcomes), &whatif, &[], &[]);
        assert_eq!(rows[0].value, "900 rps");
        assert_eq!(rows[0].badge, Some(RowBadge::Delta("+100 rps".to_owned())));
        // Проливание: значение источника + подпись «← Объект · выход»
        let spill = SpillView {
            param: "rps".into(),
            line: Some(0),
            from_label: "Профиль".into(),
            from_output: Some("peak".into()),
            value: Some("1200 rps".into()),
            path: "Профиль.peak_rps".into(),
            local: Some("800 rps".into()),
        };
        let rows = build_rows(
            text,
            Some(&outcomes),
            &[],
            std::slice::from_ref(&spill),
            &[],
        );
        assert_eq!(rows[0].value, "1200 rps");
        assert!(rows[0].upstream);
        assert_eq!(
            rows[0].badge,
            Some(RowBadge::Spill {
                label: "← Профиль · peak".to_owned()
            })
        );
        // What-if поверх проливания
        let rows = build_rows(
            text,
            Some(&outcomes),
            &whatif,
            std::slice::from_ref(&spill),
            &[],
        );
        assert_eq!(rows[0].value, "900 rps");
    }

    /// D-2: ошибка строки — бейдж «!» + полный текст для тултипа.
    #[test]
    fn build_rows_maps_error_badge() {
        let outcomes = vec![Some(canvas_core::expr::ExprOutcome::Err(
            "неизвестно x".into(),
        ))];
        let rows = build_rows("x = 1", Some(&outcomes), &[], &[], &[]);
        assert_eq!(rows[0].badge, Some(RowBadge::Error));
        assert_eq!(rows[0].error_message.as_deref(), Some("неизвестно x"));
        assert!(rows[0].value.is_empty());
    }

    /// D-7: план блока — порог T (Q2, дефолт 4): 4 строки данных — лист,
    /// 5+ с расчётными — блок-ведомость; авто-строки в счёт не идут.
    #[test]
    fn block_mode_respects_threshold() {
        let text = "a = 1\nb = 2\nc = 3\nd = 4\n5\n6 ms";
        let rows = build_rows(text, Some(&outcomes(text)), &[], &[], &[]);
        // 5 данных (4 Param + 1 Calc? нет: все — присваивания)
        let params_only = build_rows(
            "a = 1\nb = 2\nc = 3\nd = 4\ne = 5",
            Some(&outcomes("a = 1\nb = 2\nc = 3\nd = 4\ne = 5")),
            &[],
            &[],
            &[],
        );
        assert_eq!(params_only.len(), 5);
        assert!(
            !block_mode(&params_only, canvas_core::NODE_BODY_BLOCK_THRESHOLD),
            "5 присваиваний без расчёта — лист (заголовок неуместен)"
        );
        assert!(
            !block_mode(&rows[..4], canvas_core::NODE_BODY_BLOCK_THRESHOLD),
            "4 строки — на пороге, лист (Q2: добавил 5-ю — блок появился)"
        );
        // Смешанный корпус: 4 параметра + расчётные строки
        let mixed_text = "a = 1\nb = 2\nc = 3\nd = 4\na * 2\nb * 2";
        let mixed = build_rows(mixed_text, Some(&outcomes(mixed_text)), &[], &[], &[]);
        assert!(
            block_mode(&mixed, canvas_core::NODE_BODY_BLOCK_THRESHOLD),
            "6 строк данных с расчётом — блок"
        );
        assert_eq!(calc_row_count(&mixed), 2);
        assert_eq!(block_header_text(2), "▾ расчёт · 2 строки");
        assert_eq!(block_header_text(5), "▾ расчёт · 5 строк");
        assert_eq!(block_header_text(1), "▾ расчёт · 1 строка");
        assert_eq!(block_header_text(12), "▾ расчёт · 12 строк");
        // D-14: EN-локализация — line/lines (высота строки та же — I-2).
        assert_eq!(
            block_header_text_lang(1, Language::En, true),
            "▾ calc · 1 line"
        );
        assert_eq!(
            block_header_text_lang(5, Language::En, true),
            "▾ calc · 5 lines"
        );
        // FR-061 хвосты (D-7 runtime v1): свёрнутый блок — шеврон «▸».
        assert_eq!(
            block_header_text_lang(5, Language::Ru, false),
            "▸ расчёт · 5 строк"
        );
        assert_eq!(
            block_header_text_lang(5, Language::En, false),
            "▸ calc · 5 lines"
        );
        // Хвосты: превью-строка свёрнутой ведомости (прототип .preview-row).
        assert_eq!(
            block_preview_text_lang(3, 5, Language::Ru),
            "параметры · 3 · формулы · 5 строк"
        );
        assert_eq!(
            block_preview_text_lang(1, 1, Language::Ru),
            "параметры · 1 · формулы · 1 строка"
        );
        assert_eq!(
            block_preview_text_lang(2, 5, Language::En),
            "params · 2 · formulas · 5 lines"
        );
        // Хвосты (D-8 runtime v1): аффордансы экспандера описания.
        assert_eq!(desc_expand_text_lang(Language::Ru), "⋯ целиком ▾");
        assert_eq!(desc_expand_text_lang(Language::En), "⋯ show all ▾");
        assert_eq!(desc_collapse_text_lang(Language::Ru), "▴ свернуть");
        assert_eq!(desc_collapse_text_lang(Language::En), "▴ collapse");
    }

    /// Разбор полного дельта-формата: корпус whatif_full_delta — «стало» и
    /// «Δ» без потери спец-веток («пп», знак, скаляр).
    #[test]
    fn split_full_delta_parses_contract_corpus() {
        let cases = [
            ("800 rps → 900 rps (+100 rps)", ("900 rps", "+100 rps")),
            ("42 % → 55 % (+13 пп)", ("55 %", "+13 пп")),
            ("20 → 18.5 (-1.5)", ("18.5", "-1.5")),
        ];
        for (input, (new, delta)) in cases {
            assert_eq!(split_full_delta(input), Some((new.into(), delta.into())));
        }
        // Не-дельта формат — None (fallback в ячейку значения)
        assert_eq!(split_full_delta("800 rps"), None);
        assert_eq!(split_full_delta("было → стало"), None);
    }

    /// RHS присваивания: экранированный `\=` и сравнение `==` не режутся.
    #[test]
    fn assignment_rhs_respects_escapes() {
        assert_eq!(assignment_rhs("rps = 800"), "800");
        assert_eq!(assignment_rhs("flag = a == b"), "a == b");
        assert_eq!(assignment_rhs("lit = 2 \\= 2"), "2 \\= 2");
        assert_eq!(assignment_rhs("no-op"), "");
    }

    /// T3-расширение: проход A — направляющие от max по строкам, право-край
    /// от body_width; детерминизм (повторный вызов — идентичный результат).
    #[test]
    fn pass_a_measures_guides_deterministically() {
        let text = "rps = 800 rps\nlatency = 800 rps / 12";
        let rows = build_rows(text, Some(&outcomes(text)), &[], &[], &[]);
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        let pass1 = pass_a(
            &mut m,
            &mut fs,
            &rows,
            60.0,
            300.0,
            FAMILY,
            SIZE,
            SIZE,
            BadgeMode::Text,
            &[],
        );
        let pass2 = pass_a(
            &mut m,
            &mut fs,
            &rows,
            60.0,
            300.0,
            FAMILY,
            SIZE,
            SIZE,
            BadgeMode::Text,
            &[],
        );
        assert_eq!(pass1, pass2, "детерминизм прохода A");
        let g = pass1.guides.unwrap();
        assert_eq!(g.badge_w, 0.0, "бейджей нет — колонка нулевая");
        assert!(g.unit_right() <= 300.0, "юнит внутри тела");
        assert!(g.value_right() < g.unit_right(), "числа левее юнитов");
        assert_eq!(pass1.badge_mode, BadgeMode::Text);
        // Широкое значение раздвинуло направляющую чисел
        let wide_text = "rps = 138912345 rps\nlatency = 800 rps / 12";
        let wide_rows = build_rows(wide_text, Some(&outcomes(wide_text)), &[], &[], &[]);
        let wide = pass_a(
            &mut m,
            &mut fs,
            &wide_rows,
            60.0,
            300.0,
            FAMILY,
            SIZE,
            SIZE,
            BadgeMode::Text,
            &[],
        );
        assert!(wide.guides.unwrap().value_w > g.value_w);
    }

    /// Лестница деградации §3.4: узкое тело выталкивает бейджи в иконки,
    /// затем скрывает колонку; числа/юниты не деградируют (направляющие живы).
    #[test]
    fn pass_a_degrades_badges_first() {
        let text = "rps = 800 rps";
        let outcomes = outcomes(text);
        let spill = SpillView {
            param: "rps".into(),
            line: Some(0),
            from_label: "Профиль нагрузки".into(),
            from_output: None,
            value: Some("1200 rps".into()),
            path: "Профиль.peak_rps".into(),
            local: Some("800 rps".into()),
        };
        let rows = build_rows(text, Some(&outcomes), &[], &[spill], &[]);
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        // Широкое тело — текстовые бейджи
        let wide = pass_a(
            &mut m,
            &mut fs,
            &rows,
            60.0,
            520.0,
            FAMILY,
            SIZE,
            SIZE,
            BadgeMode::Text,
            &[],
        );
        assert_eq!(wide.badge_mode, BadgeMode::Text);
        assert!(wide.guides.unwrap().badge_w > 0.0);
        // Узкое тело — иконки: колонка жива, но узкая
        let narrow = pass_a(
            &mut m,
            &mut fs,
            &rows,
            60.0,
            190.0,
            FAMILY,
            SIZE,
            SIZE,
            BadgeMode::Text,
            &[],
        );
        assert_eq!(narrow.badge_mode, BadgeMode::Icon);
        // Совсем узкое — колонка скрыта, направляющие значений живы
        let tiny = pass_a(
            &mut m,
            &mut fs,
            &rows,
            60.0,
            120.0,
            FAMILY,
            SIZE,
            SIZE,
            BadgeMode::Text,
            &[],
        );
        assert_eq!(tiny.badge_mode, BadgeMode::None);
        let g = tiny.guides.unwrap();
        assert!(g.value_right() <= 120.0);
    }

    /// T2-инвариант (§7): на корпусе tcp-lb (имя 19 симв. + формула 18 +
    /// бейдж-источник) каждая строка помещается: левый текст + лидер не
    /// наезжают на направляющую чисел, ячейки внутри body_width.
    #[test]
    fn pass_a_t2_width_invariant_on_tcp_lb_corpus() {
        let text =
            "входящий_трафик = 1200\nобработанный = входящий_трафик * 0.98\nзадержка = 12.5 ms";
        let outcomes = outcomes(text);
        let spill = SpillView {
            param: "входящий_трафик".into(),
            line: Some(0),
            from_label: "Профиль нагрузки".into(),
            from_output: None,
            value: Some("1200 rps".into()),
            path: "Профиль.peak_rps".into(),
            local: Some("1200 rps".into()),
        };
        let rows = build_rows(text, Some(&outcomes), &[], &[spill], &[]);
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        for body_width in [300.0, 384.0, 520.0] {
            let pass = pass_a(
                &mut m,
                &mut fs,
                &rows,
                140.0,
                body_width,
                FAMILY,
                SIZE,
                SIZE,
                BadgeMode::Text,
                &[],
            );
            let g = pass.guides.unwrap();
            assert!(
                g.value_x >= 140.0 + LEADER_PAD + LEADER_MIN,
                "ширина {body_width}: лидер не наезжает на значение"
            );
            assert!(g.value_right() <= body_width, "значение внутри тела");
        }
        // Деградация детерминирована: 384 → одна и та же ступень
        let p1 = pass_a(
            &mut m,
            &mut fs,
            &rows,
            140.0,
            384.0,
            FAMILY,
            SIZE,
            SIZE,
            BadgeMode::Text,
            &[],
        );
        let p2 = pass_a(
            &mut m,
            &mut fs,
            &rows,
            140.0,
            384.0,
            FAMILY,
            SIZE,
            SIZE,
            BadgeMode::Text,
            &[],
        );
        assert_eq!(p1, p2);
    }

    /// D-11: отпечаток прохода различает направляющие и режимы — стейл
    /// после смены режима невозможен.
    #[test]
    fn pass_key_reflects_guides_and_mode() {
        let empty = TablePass {
            guides: None,
            badge_mode: BadgeMode::Text,
            ellipsis: Vec::new(),
        };
        assert_eq!(pass_key(&empty), "", "нет таблицы — нет отпечатка");
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        let text = "rps = 800 rps";
        let rows = build_rows(text, Some(&outcomes(text)), &[], &[], &[]);
        let pass = pass_a(
            &mut m,
            &mut fs,
            &rows,
            60.0,
            300.0,
            FAMILY,
            SIZE,
            SIZE,
            BadgeMode::Text,
            &[],
        );
        let key = pass_key(&pass);
        assert!(key.starts_with("G:"), "отпечаток направляющих");
        assert!(key.contains("Text"));
    }

    /// Приёмка T9 FR-061: литеральный RHS (значение ячейки повторяет
    /// формулу) — детект; выражение — не литерал; strip-переопределения —
    /// только для литеральных Param без бейджа/upstream.
    #[test]
    fn literal_rhs_detection_and_strip_overrides() {
        let text = "servers = 3\nload = servers * 2\nrps = 800 rps";
        let rows = build_rows(text, Some(&outcomes(text)), &[], &[], &[]);
        assert!(is_literal_rhs(&rows[0]), "«servers = 3» — литерал");
        assert!(!is_literal_rhs(&rows[1]), "«servers * 2» — выражение");
        assert!(is_literal_rhs(&rows[2]), "«800 rps» — литерал с юнитом");
        let strip = param_strip_overrides(&rows);
        assert_eq!(
            strip,
            vec![
                (0usize, "servers =".to_owned()),
                (2usize, "rps =".to_owned())
            ],
            "load (выражение) не strip'ается"
        );
        // Пролитая строка: бейдж есть, значение upstream — не strip'ается.
        let spill = SpillView {
            param: "servers".into(),
            line: Some(0),
            from_label: "Профиль".into(),
            from_output: None,
            value: Some("50 rps".into()),
            path: "Профиль.peak".into(),
            local: Some("3".into()),
        };
        let rows = build_rows(text, Some(&outcomes(text)), &[], &[spill], &[]);
        assert!(
            param_strip_overrides(&rows)
                .iter()
                .all(|(line, _)| *line != 0),
            "пролитая строка без strip (левая часть — подпись источника)"
        );
    }

    /// Приёмка T9 FR-061: Param с выражением в RHS усекается с ЗАЩИЩЁННЫМ
    /// префиксом «имя =» (лестница §3.4 дошла до последней ступени);
    /// литеральный Param (числа) не усекается никогда.
    #[test]
    fn pass_a_truncates_param_expression_rhs_with_protected_prefix() {
        let text = "load = connection_per_second_value / server_rate_value";
        let rows = build_rows(text, Some(&outcomes(text)), &[], &[], &[]);
        assert!(!is_literal_rhs(&rows[0]));
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        // left_max — ширина полного левого текста строки (заведомо шире
        // тела 120 px): лестница деградации доходит до последней ступени.
        let left_max = m.width_of_weighted(
            &mut fs,
            "load = connection_per_second_value / server_rate_value",
            FAMILY,
            SIZE,
            MEASURE_WEIGHT,
        );
        let pass = pass_a(
            &mut m,
            &mut fs,
            &rows,
            left_max,
            120.0,
            FAMILY,
            SIZE,
            SIZE,
            BadgeMode::Text,
            &[],
        );
        let plan = pass.ellipsis[0]
            .as_ref()
            .expect("узкое тело: план усечения для Param-выражения");
        assert!(
            plan.display.starts_with("load = "),
            "префикс «имя =» защищён"
        );
        assert!(plan.display.ends_with('…'), "хвост — многоточие");
        assert_eq!(
            plan.full, "load = connection_per_second_value / server_rate_value",
            "тултип — полная строка"
        );
        // Усечённое отображение реально влезает в доступную дорожку.
        let g = pass.guides.expect("строки данных есть");
        let available = g.value_x - LEADER_PAD - LEADER_MIN;
        let display_w = m.width_of_weighted(&mut fs, &plan.display, FAMILY, SIZE, MEASURE_WEIGHT);
        assert!(
            display_w <= available + 1.0,
            "display ({display_w}) влезает в дорожку ({available})"
        );
        // Литеральный Param — числа не деградируют.
        let lit_text = "servers = 3";
        let lit = build_rows(lit_text, Some(&outcomes(lit_text)), &[], &[], &[]);
        let pass2 = pass_a(
            &mut m,
            &mut fs,
            &lit,
            0.0,
            60.0,
            FAMILY,
            SIZE,
            SIZE,
            BadgeMode::Text,
            &[],
        );
        assert!(
            pass2.ellipsis.first().is_none_or(|e| e.is_none()),
            "литеральный RHS не усекается (числа не деградируют)"
        );
    }

    /// Q8 FR-061: алиасы — длинные идентификаторы (кириллица/латиница/_)
    /// обрезаются до KEEP+«…», числа и короткие имена не трогаются.
    #[test]
    fn alias_idents_truncates_long_idents_only() {
        let long = "конверсия_в_платящего";
        assert_eq!(long.chars().count(), 21, "длиннее порога");
        let aliased = alias_idents(&format!("{long} * 2"));
        assert_eq!(
            aliased,
            format!(
                "{}… * 2",
                long.chars().take(ALIAS_IDENT_KEEP).collect::<String>()
            )
        );
        // Числовой литерал не деградирует (лестница §3.4)
        assert_eq!(
            alias_idents("12345678901234567890 * 2"),
            "12345678901234567890 * 2"
        );
        // Короткие идентификаторы не трогаются
        assert_eq!(alias_idents("rps * 2 + _x1"), "rps * 2 + _x1");
        // Смешанный токен с ведущей цифрой — не идентификатор
        assert_eq!(alias_idents("2nd_order_term"), "2nd_order_term");
        // Порог: ровно MAX — не алиасится, MAX+1 — алиасится
        let max_ident = "a".repeat(ALIAS_IDENT_MAX);
        assert_eq!(alias_idents(&max_ident), max_ident);
        let over_ident = format!("{}x", "a".repeat(ALIAS_IDENT_MAX));
        assert_eq!(
            alias_idents(&over_ident),
            format!("{}…", "a".repeat(ALIAS_IDENT_KEEP))
        );
    }

    /// Коммит 3 (лестница §3.4, последняя ступень): на узком теле после
    /// скрытия колонки бейджей длинная формула расчётной строки получает
    /// план усечения (display короче, full — оригинал); параметр и числа —
    /// без изменений; план стабилен при повторном проходе (floor + prior).
    #[test]
    fn pass_a_plans_formula_ellipsis_on_narrow_body() {
        let long_formula = "конверсия_в_платящего * входящий_трафик_портов * 2";
        // Известные переменные — иначе движок молчит (проза) и строки нет.
        let text =
            format!("конверсия_в_платящего = 0.05\nвходящий_трафик_портов = 100\n{long_formula}");
        let rows = build_rows(&text, Some(&outcomes(&text)), &[], &[], &[]);
        let calc = rows
            .iter()
            .find(|r| r.kind == RowKind::Calc)
            .expect("расчётная строка с исходом");
        assert_eq!(calc.formula.trim(), long_formula, "формула — вся строка");
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        // Совсем узкое тело: бейджей нет, левый текст заведомо шире дорожки
        let pass = pass_a(
            &mut m,
            &mut fs,
            &rows,
            260.0,
            160.0,
            FAMILY,
            SIZE,
            SIZE,
            BadgeMode::Text,
            &[],
        );
        assert_eq!(pass.badge_mode, BadgeMode::None);
        let plan: Vec<&RowEllipsis> = pass.ellipsis.iter().flatten().collect();
        assert!(!plan.is_empty(), "длинная формула получила план");
        for e in &plan {
            assert_eq!(e.full, long_formula, "тултип — полная формула");
            assert_ne!(e.display, e.full, "отображение усечено");
            assert!(
                e.display.ends_with('…') || e.display.contains('…'),
                "алиас/ellipsis"
            );
            let w = m.width_of(&mut fs, &e.display, FAMILY, SIZE);
            let g = pass.guides.unwrap();
            assert!(
                w <= g.value_x - LEADER_PAD - LEADER_MIN + 1.0,
                "усечённое отображение влезает в дорожку"
            );
        }
        // Параметр (имя) не деградирует: у Param-строк плана нет
        assert!(
            pass.ellipsis
                .iter()
                .zip(&rows)
                .all(|(e, r)| r.kind != RowKind::Param || e.is_none()),
            "план только для Calc"
        );
        // Повторный проход с floor+prior: план и режим стабильны (перешейп)
        let repass = pass_a(
            &mut m,
            &mut fs,
            &rows,
            80.0, // left_max после усечения — меньше
            160.0,
            FAMILY,
            SIZE,
            SIZE,
            pass.badge_mode,
            &pass.ellipsis,
        );
        assert_eq!(repass.badge_mode, pass.badge_mode, "пол лестницы");
        assert_eq!(repass.ellipsis, pass.ellipsis, "план стабилен");
        assert_eq!(repass.guides, pass.guides, "направляющие те же");
        // Широкое тело — плана нет
        let wide = pass_a(
            &mut m,
            &mut fs,
            &rows,
            60.0,
            520.0,
            FAMILY,
            SIZE,
            SIZE,
            BadgeMode::Text,
            &[],
        );
        assert!(wide.ellipsis.iter().all(|e| e.is_none()));
    }
}
