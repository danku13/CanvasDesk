//! UI-модель панели поиска (T14, TASKS/SPEC §5.2): однострочное поле ввода с
//! кареткой, список результатов, геометрия панели, in-memory substring-поиск
//! по заметкам и именам нод.
//!
//! Модуль чистый (без wgpu/winit) — покрывается юнит-тестами. Рендер панели
//! выполняет приложение через `FrameOverlay` (screen_instances: CardInstance
//! с fill/border/скруглением; ScreenText для текста) — координаты берутся из
//! `layout`. egui в стеке нет и не заводится (план T14 §3).
//!
//! Ввод НЕ использует `EditingSession` (это многострочный редактор заметок):
//! поле поиска — однострочное, своя лёгкая модель `SearchInput`.

use std::collections::HashSet;

/// Ширина панели, логические px (клампится к окну − 2×PANEL_SIDE_MARGIN).
pub const PANEL_WIDTH: f32 = 460.0;
/// Боковой отступ панели от краёв окна, логические px.
pub const PANEL_SIDE_MARGIN: f32 = 12.0;
/// Отступ панели от верхнего края окна, логические px.
pub const PANEL_TOP_MARGIN: f32 = 12.0;
/// Высота поля ввода, логические px.
pub const INPUT_HEIGHT: f32 = 36.0;
/// Высота строки результата, логические px.
pub const ROW_HEIGHT: f32 = 28.0;
/// Максимум видимых строк результата (далее — прокрутка).
pub const MAX_VISIBLE_ROWS: usize = 8;
/// Внутренний отступ содержимого панели, логические px.
pub const PANEL_PADDING: f32 = 8.0;

/// Разделитель слова для Ctrl+Backspace — простая эвристика: пробельный символ
/// или ASCII-знак препинания. Буквы (включая кириллицу), цифры и прочие
/// многобайтные символы (эмодзи-подобные) считаются символами слова.
fn is_word_separator(c: char) -> bool {
    c.is_whitespace() || c.is_ascii_punctuation()
}

/// Однострочное поле ввода поиска: строка + каретка (байтовый индекс,
/// всегда на границе UTF-8-символов).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchInput {
    query: String,
    cursor: usize,
}

impl SearchInput {
    /// Пустое поле.
    pub fn new() -> Self {
        Self::default()
    }

    /// Текущий запрос.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Позиция каретки в байтах (на границе символов).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Символ слева от байтового индекса `idx` (граница символов):
    /// `(байтовый индекс символа, символ)`; `None`, если `idx == 0`.
    fn char_before(&self, idx: usize) -> Option<(usize, char)> {
        self.query[..idx].char_indices().next_back()
    }

    /// Заменить весь текст (Ctrl+F повторное открытие: прошлый запрос выделен —
    /// ввод замещает), каретка в конец.
    pub fn set_query(&mut self, text: &str) {
        self.query.clear();
        self.query.push_str(text);
        self.cursor = self.query.len();
    }

    /// Вставить текст в позицию каретки (символы печати). Каретка остаётся на
    /// границе символов (вставляется строка целиком).
    pub fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // Каретка всегда на границе символов — insert_str не паникует.
        self.query.insert_str(self.cursor, text);
        self.cursor += text.len();
    }

    /// Backspace: удалить символ слева (многобайтный — целиком); с `word`
    /// (Ctrl+Backspace) — слово слева. Возвращает true, если текст изменился.
    ///
    /// Word-backspace (простая эвристика, план T14 §3): от каретки влево
    /// удаляется цепочка символов слова, затем цепочка разделителей (пробелы +
    /// знаки препинания) до начала предыдущего слова — «привет мир|» →
    /// «привет|», «file.txt|» → «file|», «hello   |» → «hello|».
    pub fn backspace(&mut self, word: bool) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let start = if word {
            let mut idx = self.cursor;
            // Фаза 1: символы слова (до разделителя).
            while let Some((_, c)) = self.char_before(idx) {
                if is_word_separator(c) {
                    break;
                }
                idx -= c.len_utf8();
            }
            // Фаза 2: разделители до начала предыдущего слова.
            while let Some((_, c)) = self.char_before(idx) {
                if !is_word_separator(c) {
                    break;
                }
                idx -= c.len_utf8();
            }
            idx
        } else {
            match self.char_before(self.cursor) {
                Some((_, c)) => self.cursor - c.len_utf8(),
                // Недостижимо: cursor > 0 и на границе символов.
                None => return false,
            }
        };
        if start >= self.cursor {
            return false;
        }
        self.query.replace_range(start..self.cursor, "");
        self.cursor = start;
        true
    }

    /// Delete: удалить символ справа от каретки (многобайтный — целиком).
    /// Возвращает true, если текст изменился.
    pub fn delete(&mut self) -> bool {
        let len = self.query[self.cursor..]
            .chars()
            .next()
            .map_or(0, char::len_utf8);
        if len == 0 {
            return false;
        }
        self.query.replace_range(self.cursor..self.cursor + len, "");
        true
    }

    /// Движение каретки: Home.
    pub fn move_to_start(&mut self) {
        self.cursor = 0;
    }

    /// Движение каретки: End.
    pub fn move_to_end(&mut self) {
        self.cursor = self.query.len();
    }

    /// Движение каретки: влево на один символ (по границам UTF-8).
    pub fn move_left(&mut self) {
        if let Some((_, c)) = self.char_before(self.cursor) {
            self.cursor -= c.len_utf8();
        }
    }

    /// Движение каретки: вправо на один символ (по границам UTF-8).
    pub fn move_right(&mut self) {
        if let Some(c) = self.query[self.cursor..].chars().next() {
            self.cursor += c.len_utf8();
        }
    }
}

/// Строка результата поиска (уже сматчена в ноду приложения).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRow {
    /// Заголовок: имя файла / первые символы текста заметки.
    pub title: String,
    /// Подзаголовок: хвост пути или «заметка».
    pub subtitle: String,
}

/// Действие панели, возвращаемое при обработке клавиш (Enter/Esc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelAction {
    /// Прыжок к строке с индексом.
    Jump(usize),
    /// Закрыть панель.
    Close,
}

/// Состояние панели поиска.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchPanel {
    /// Панель открыта (видима, получает клавиатуру).
    pub open: bool,
    /// Поле ввода.
    pub input: SearchInput,
    /// Строки результатов (соответствие нодам — в приложении).
    pub rows: Vec<SearchRow>,
    /// Выбранная строка (Enter прыгает к ней).
    pub selected: Option<usize>,
    /// Верхняя видимая строка (прокрутка).
    pub scroll_top: usize,
}

impl SearchPanel {
    /// Открыть панель (текст прошлой выборки сохраняется — ввод замещает).
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Закрыть панель (rows сохраняются для цикла F3).
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Панель открыта?
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Установить результаты (после Query/scan_scene): сброс выбора на первую
    /// строку, скролл вверх.
    pub fn set_results(&mut self, rows: Vec<SearchRow>) {
        self.selected = if rows.is_empty() { None } else { Some(0) };
        self.scroll_top = 0;
        self.rows = rows;
    }

    /// Сдвиг выбора на `delta` строк с зацикливанием (F3/Shift+F3, Up/Down):
    /// вниз с последней строки — на первую, вверх с первой — на последнюю.
    /// Пустой список — no-op.
    pub fn move_selection(&mut self, delta: i32) {
        if self.rows.is_empty() {
            return;
        }
        // Длина списка и индексы много меньше i64 — переполнений нет.
        let len = self.rows.len() as i64;
        let current = self.selected.map_or(0, |s| s as i64);
        let wrapped = (current + delta as i64).rem_euclid(len);
        self.selected = Some(wrapped as usize);
    }

    /// Прокрутить к выбранной строке, если она вне видимого окна
    /// [scroll_top, scroll_top + MAX_VISIBLE_ROWS).
    pub fn ensure_selection_visible(&mut self) {
        if self.rows.is_empty() {
            self.scroll_top = 0;
            return;
        }
        let sel = self.selected.unwrap_or(0);
        if sel < self.scroll_top {
            // Выше окна — прокрутка к выбранной строке.
            self.scroll_top = sel;
        } else if sel >= self.scroll_top.saturating_add(MAX_VISIBLE_ROWS) {
            // Ниже окна — выбранная строка становится последней видимой.
            // Ветка гарантирует sel >= MAX_VISIBLE_ROWS — вычитание не
            // уходит в минус.
            self.scroll_top = sel - (MAX_VISIBLE_ROWS - 1);
        }
        // Прокрутка не дальше конца списка.
        self.scroll_top = self.scroll_top.min(self.rows.len());
    }

    /// Enter: прыжок к выбранной строке (нет выбора — к первой);
    /// пустой список — `None`.
    pub fn confirm(&self) -> Option<PanelAction> {
        if self.rows.is_empty() {
            None
        } else {
            Some(PanelAction::Jump(self.selected.unwrap_or(0)))
        }
    }

    /// Esc: закрыть панель.
    pub fn cancel(&self) -> PanelAction {
        PanelAction::Close
    }
}

/// Геометрия панели в логических px (для отрисовки через FrameOverlay).
#[derive(Debug, Clone, PartialEq)]
pub struct PanelLayout {
    /// Прямоугольник панели: `[x0, y0, x1, y1]`.
    pub panel_rect: [f32; 4],
    /// Прямоугольник поля ввода (без рамки-заголовка).
    pub input_rect: [f32; 4],
    /// Прямоугольники видимых строк (индекс = позиция в rows, начиная со
    /// scroll_top; длина ≤ MAX_VISIBLE_ROWS).
    pub row_rects: Vec<[f32; 4]>,
}

/// Геометрия панели: топ-центр, ширина PANEL_WIDTH (кламп к окну), высота =
/// поле + видимые строки (0 строк — только поле), скролл от panel.scroll_top.
///
/// Вырожденное окно (ширина/высота ≤ 0 — свёрнутое окно, ширина меньше двух
/// боковых отступов) схлопывает панель в точку — без паники. Высота окна
/// панель не ограничивает (геометрия топ-центра).
pub fn layout(window_w: f32, window_h: f32, panel: &SearchPanel) -> PanelLayout {
    let window_w = window_w.max(0.0);
    let width = (window_w - 2.0 * PANEL_SIDE_MARGIN).clamp(0.0, PANEL_WIDTH);
    if width <= 0.0 || window_h <= 0.0 {
        // Схлопывание в точку (x — центр вырожденного окна).
        let cx = window_w / 2.0;
        let point = [cx, PANEL_TOP_MARGIN, cx, PANEL_TOP_MARGIN];
        return PanelLayout {
            panel_rect: point,
            input_rect: point,
            row_rects: Vec::new(),
        };
    }

    let x0 = (window_w - width) / 2.0;
    let x1 = x0 + width;
    let y0 = PANEL_TOP_MARGIN;

    // Окно прокрутки клампится к длине списка (scroll_top задаётся извне).
    let scroll_top = panel.scroll_top.min(panel.rows.len());
    let visible = (panel.rows.len() - scroll_top).min(MAX_VISIBLE_ROWS);

    let inner_l = x0 + PANEL_PADDING;
    // Крошечная ширина (< 2 отступов) — вырожденные, но не вывернутые rect'ы.
    let inner_r = (x1 - PANEL_PADDING).max(inner_l);
    let input_top = y0 + PANEL_PADDING;
    let input_rect = [inner_l, input_top, inner_r, input_top + INPUT_HEIGHT];

    let mut row_rects = Vec::with_capacity(visible);
    let y1 = if visible == 0 {
        // Только поле ввода.
        input_top + INPUT_HEIGHT + PANEL_PADDING
    } else {
        let rows_top = input_top + INPUT_HEIGHT + PANEL_PADDING;
        for i in 0..visible {
            let y = rows_top + i as f32 * ROW_HEIGHT;
            row_rects.push([inner_l, y, inner_r, y + ROW_HEIGHT]);
        }
        rows_top + visible as f32 * ROW_HEIGHT + PANEL_PADDING
    };

    PanelLayout {
        panel_rect: [x0, y0, x1, y1],
        input_rect,
        row_rects,
    }
}

/// Запись сцены для in-memory поиска: нода + заголовок + текст (заметка или
/// пустая строка для файлов).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneEntry<'a> {
    /// Индекс ноды в canvas.nodes.
    pub node: usize,
    /// Имя файла или текст заметки (заголовок).
    pub title: &'a str,
    /// Текст заметки (для text-нод) — пусто для файлов.
    pub text: &'a str,
}

/// In-memory substring-поиск (регистронезависимый) по заголовкам и текстам:
/// находит заметки и ноды, которых нет в FTS-индексе (файлы без текста,
/// ещё не проиндексированные). Возвращает индексы нод (порядок входа,
/// дубликаты нод — один раз). Пустой запрос — пустой результат.
pub fn scan_scene(query: &str, entries: &[SceneEntry<'_>]) -> Vec<usize> {
    if query.is_empty() {
        return Vec::new();
    }
    let needle = query.to_lowercase();
    let mut seen: HashSet<usize> = HashSet::new();
    let mut result = Vec::new();
    for entry in entries {
        if seen.contains(&entry.node) {
            continue; // нода уже в результате — не добавляем дважды
        }
        let title = entry.title.to_lowercase();
        let text = entry.text.to_lowercase();
        if title.contains(needle.as_str()) || text.contains(needle.as_str()) {
            seen.insert(entry.node);
            result.push(entry.node);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-4;

    /// `n` строк-заглушек.
    fn rows(n: usize) -> Vec<SearchRow> {
        (0..n)
            .map(|i| SearchRow {
                title: format!("строка {i}"),
                subtitle: String::new(),
            })
            .collect()
    }

    /// Открытая панель с `n` результатами (выбор на первой, скролл сверху).
    fn panel_with_rows(n: usize) -> SearchPanel {
        let mut panel = SearchPanel::default();
        panel.open();
        panel.set_results(rows(n));
        panel
    }

    /// Открытая панель с заданным выбором и прокруткой.
    fn panel_configured(rows_n: usize, selected: Option<usize>, scroll_top: usize) -> SearchPanel {
        SearchPanel {
            open: true,
            input: SearchInput::default(),
            rows: rows(rows_n),
            selected,
            scroll_top,
        }
    }

    // ---------- SearchInput ----------

    /// Вставка текста двигает каретку (латиница + кириллица); вставка в
    /// середину строки.
    #[test]
    fn input_insert_and_cursor() {
        let mut input = SearchInput::new();
        input.insert_str("аб");
        assert_eq!(input.query(), "аб");
        assert_eq!(input.cursor(), 4);
        input.insert_str("в");
        assert_eq!(input.query(), "абв");
        assert_eq!(input.cursor(), 6);
        // Пустая вставка — no-op
        input.insert_str("");
        assert_eq!(input.query(), "абв");

        // Вставка в середину
        let mut input = SearchInput::new();
        input.set_query("abc");
        input.move_left();
        input.insert_str("X");
        assert_eq!(input.query(), "abXc");
        assert_eq!(input.cursor(), 3);
    }

    /// Backspace удаляет символ слева (кириллица, 2 байта), на пустой строке
    /// и в начале строки — no-op.
    #[test]
    fn input_backspace_cyrillic() {
        let mut input = SearchInput::new();
        input.set_query("абв");
        assert!(input.backspace(false));
        assert_eq!(input.query(), "аб");
        assert_eq!(input.cursor(), 4);
        assert!(input.backspace(false));
        assert!(input.backspace(false));
        assert_eq!(input.query(), "");
        assert_eq!(input.cursor(), 0);
        // Пустая строка — no-op без паники
        assert!(!input.backspace(false));

        // Каретка в начале непустой строки — no-op
        let mut input = SearchInput::new();
        input.set_query("абв");
        input.move_to_start();
        assert!(!input.backspace(false));
        assert_eq!(input.query(), "абв");
    }

    /// Backspace не рвёт многобайтный символ: «𝕏» (4 байта) удаляется целиком.
    #[test]
    fn input_backspace_multibyte_whole_char() {
        let mut input = SearchInput::new();
        input.set_query("𝕏");
        assert_eq!(input.cursor(), 4);
        assert!(input.backspace(false));
        assert_eq!(input.query(), "");
        assert_eq!(input.cursor(), 0);

        // Символы вокруг многобайтного: «a𝕏b» — удаляем b, затем 𝕏 целиком
        let mut input = SearchInput::new();
        input.set_query("a𝕏b");
        assert_eq!(input.cursor(), 6);
        input.backspace(false);
        assert_eq!(input.query(), "a𝕏");
        assert_eq!(input.cursor(), 5);
        input.backspace(false);
        assert_eq!(input.query(), "a");
        assert_eq!(input.cursor(), 1);
    }

    /// Ctrl+Backspace: слово + разделители до предыдущего слова (эвристика:
    /// разделитель = пробел или ASCII-знак препинания; кириллица и
    /// многобайтные символы — символы слова).
    #[test]
    fn input_word_backspace() {
        // латиница
        let mut input = SearchInput::new();
        input.set_query("hello world");
        assert!(input.backspace(true));
        assert_eq!(input.query(), "hello");
        assert_eq!(input.cursor(), 5);

        // кириллица
        let mut input = SearchInput::new();
        input.set_query("привет мир");
        input.backspace(true);
        assert_eq!(input.query(), "привет");

        // слово + цифры, затем пробел
        let mut input = SearchInput::new();
        input.set_query("смета 2026");
        input.backspace(true);
        assert_eq!(input.query(), "смета");

        // многобайтный «словесный» символ — тоже слово
        let mut input = SearchInput::new();
        input.set_query("мир 𝕏");
        input.backspace(true);
        assert_eq!(input.query(), "мир");

        // каретка в начале — no-op
        let mut input = SearchInput::new();
        input.set_query("hello");
        input.move_to_start();
        assert!(!input.backspace(true));
        assert_eq!(input.query(), "hello");

        // каретка после пробелов: фаза «слова» пуста, удаляются пробелы
        let mut input = SearchInput::new();
        input.set_query("hello   ");
        input.backspace(true);
        assert_eq!(input.query(), "hello");

        // знак препинания — разделитель: «file.txt|» -> «file|»
        let mut input = SearchInput::new();
        input.set_query("file.txt");
        input.backspace(true);
        assert_eq!(input.query(), "file");

        // одно слово без разделителей — удаляется целиком
        let mut input = SearchInput::new();
        input.set_query("абвг");
        input.backspace(true);
        assert_eq!(input.query(), "");
        assert_eq!(input.cursor(), 0);
    }

    /// Delete удаляет символ справа (многобайтный — целиком); в конце строки
    /// и на пустой — no-op. (set_query ставит каретку в конец — перед delete
    /// каретка переводится в начало.)
    #[test]
    fn input_delete() {
        let mut input = SearchInput::new();
        input.set_query("abc");
        input.move_to_start(); // каретка в начало
        assert!(input.delete());
        assert_eq!(input.query(), "bc");
        assert_eq!(input.cursor(), 0); // каретка на месте

        input.move_to_end();
        assert!(!input.delete());
        assert_eq!(input.query(), "bc");

        // многобайтный символ справа удаляется целиком
        let mut input = SearchInput::new();
        input.set_query("𝕏b");
        input.move_to_start();
        input.delete();
        assert_eq!(input.query(), "b");

        let mut input = SearchInput::new();
        input.set_query("a𝕏"); // каретка 5
        input.move_left(); // перед 𝕏
        assert!(input.delete());
        assert_eq!(input.query(), "a");

        // пустая строка — no-op без паники
        let mut input = SearchInput::new();
        assert!(!input.delete());
    }

    /// Стрелки/Home/End ходят по границам символов: кириллица (2 байта) и
    /// «𝕏» (4 байта); за края — no-op.
    #[test]
    fn input_arrows_home_end() {
        let mut input = SearchInput::new();
        input.set_query("абвг"); // 4 символа, 8 байт
        assert_eq!(input.cursor(), 8);
        input.move_left();
        assert_eq!(input.cursor(), 6);
        input.move_left();
        assert_eq!(input.cursor(), 4);
        input.move_right();
        assert_eq!(input.cursor(), 6);
        input.move_to_start();
        assert_eq!(input.cursor(), 0);
        input.move_left(); // на начале — no-op
        assert_eq!(input.cursor(), 0);
        input.move_to_end();
        assert_eq!(input.cursor(), 8);
        input.move_right(); // в конце — no-op
        assert_eq!(input.cursor(), 8);

        // 4-байтные символы: шаг каретки = 4
        let mut input = SearchInput::new();
        input.set_query("𝕏𝕏");
        assert_eq!(input.cursor(), 8);
        input.move_left();
        assert_eq!(input.cursor(), 4);
        input.move_left();
        input.move_left();
        assert_eq!(input.cursor(), 0);
    }

    /// set_query замещает текст, каретка — в конец (в т.ч. многобайтный).
    #[test]
    fn input_set_query() {
        let mut input = SearchInput::new();
        input.set_query("смета");
        assert_eq!(input.query(), "смета");
        assert_eq!(input.cursor(), 10);
        input.set_query("𝕏");
        assert_eq!(input.cursor(), 4);
        // повторная установка замещает текст
        input.set_query("a");
        assert_eq!(input.query(), "a");
        assert_eq!(input.cursor(), 1);
        input.set_query("");
        assert_eq!(input.query(), "");
        assert_eq!(input.cursor(), 0);
    }

    /// Все операции на пустом поле — no-op без паники.
    #[test]
    fn input_empty_operations_are_noop() {
        let mut input = SearchInput::new();
        input.insert_str("");
        assert!(!input.backspace(false));
        assert!(!input.backspace(true));
        assert!(!input.delete());
        input.move_left();
        input.move_right();
        input.move_to_start();
        input.move_to_end();
        input.set_query("");
        assert_eq!(input.query(), "");
        assert_eq!(input.cursor(), 0);
    }

    // ---------- SearchPanel ----------

    /// open/close переключают видимость; rows и выбор переживают закрытие
    /// (цикл F3 после Esc).
    #[test]
    fn panel_open_close_preserves_rows() {
        let mut panel = SearchPanel::default();
        assert!(!panel.is_open());
        panel.open();
        assert!(panel.is_open());
        panel.set_results(rows(2));
        panel.move_selection(1);
        panel.close();
        assert!(!panel.is_open());
        assert_eq!(panel.rows.len(), 2);
        assert_eq!(panel.selected, Some(1));
        panel.open();
        assert!(panel.is_open());
        assert_eq!(panel.rows.len(), 2); // данные пережили закрытие
    }

    /// set_results: непустой список — выбор на 0 и скролл вверх; пустой —
    /// выбора нет.
    #[test]
    fn panel_set_results_resets_selection() {
        let mut panel = panel_configured(5, Some(4), 3);
        panel.set_results(rows(3));
        assert_eq!(panel.selected, Some(0));
        assert_eq!(panel.scroll_top, 0);
        // пустой список
        panel.set_results(Vec::new());
        assert!(panel.rows.is_empty());
        assert_eq!(panel.selected, None);
        assert_eq!(panel.scroll_top, 0);
    }

    /// move_selection зацикливается: вниз с последней — на первую, вверх с
    /// первой — на последнюю; большой шаг сворачивается по модулю длины;
    /// пустой список — no-op.
    #[test]
    fn panel_move_selection_wraps() {
        let mut panel = panel_with_rows(3);
        assert_eq!(panel.selected, Some(0));
        panel.move_selection(1);
        assert_eq!(panel.selected, Some(1));
        panel.move_selection(1);
        assert_eq!(panel.selected, Some(2));
        // вниз с последней — на первую (зацикливание)
        panel.move_selection(1);
        assert_eq!(panel.selected, Some(0));
        // вверх с первой — на последнюю
        panel.move_selection(-1);
        assert_eq!(panel.selected, Some(2));
        // большой шаг: (2 + 5) mod 3 = 1
        panel.move_selection(5);
        assert_eq!(panel.selected, Some(1));
        // большой шаг назад: (1 - 5) mod 3 = 2
        panel.move_selection(-5);
        assert_eq!(panel.selected, Some(2));

        // пустой список — no-op без паники
        let mut empty = SearchPanel::default();
        empty.set_results(Vec::new());
        empty.move_selection(3);
        empty.move_selection(-2);
        assert_eq!(empty.selected, None);
    }

    /// ensure_selection_visible: выбор за нижней границей окна — окно
    /// подгоняется (выбранная строка последняя видимая); выше окна —
    /// прокрутка к ней; видимая — не трогается; пустой список — no-op.
    #[test]
    fn panel_ensure_selection_visible() {
        // 10 строк, выбор 9 — вне окна [0, 8)
        let mut panel = panel_configured(10, Some(9), 0);
        panel.ensure_selection_visible();
        assert_eq!(panel.scroll_top, 2); // окно [2, 10)
        let sel = panel.selected.unwrap();
        assert!(sel >= panel.scroll_top && sel < panel.scroll_top + MAX_VISIBLE_ROWS);

        // выбор выше окна
        let mut panel = panel_configured(10, Some(3), 5);
        panel.ensure_selection_visible();
        assert_eq!(panel.scroll_top, 3);

        // уже видимая — прокрутка не меняется
        let mut panel = panel_configured(10, Some(5), 0);
        panel.ensure_selection_visible();
        assert_eq!(panel.scroll_top, 0);

        // пустой список — no-op без паники
        let mut empty = SearchPanel::default();
        empty.ensure_selection_visible();
        assert_eq!(empty.scroll_top, 0);
    }

    /// confirm: пустой список — None; иначе Jump(выбранная), без выбора —
    /// Jump(0). cancel — всегда Close.
    #[test]
    fn panel_confirm_and_cancel() {
        let mut panel = SearchPanel::default();
        // пустой список — Enter ничего не прыгает
        assert_eq!(panel.confirm(), None);
        assert_eq!(panel.cancel(), PanelAction::Close);

        panel.set_results(rows(3));
        assert_eq!(panel.confirm(), Some(PanelAction::Jump(0)));
        panel.move_selection(2);
        assert_eq!(panel.confirm(), Some(PanelAction::Jump(2)));

        // выбор сброшен внешне — прыжок к первой
        let no_selection = panel_configured(3, None, 0);
        assert_eq!(no_selection.confirm(), Some(PanelAction::Jump(0)));
        assert_eq!(no_selection.cancel(), PanelAction::Close);
    }

    // ---------- layout ----------

    /// Окно 1280×720, 3 строки: панель топ-центр (ширина 460, x-центр = 640),
    /// поле ввода внутри с отступом, строки ниже поля высотой 28.
    #[test]
    fn layout_centered_with_three_rows() {
        let panel = panel_with_rows(3);
        let lay = layout(1280.0, 720.0, &panel);
        let pr = lay.panel_rect;
        // ширина и центрирование
        assert!((pr[2] - pr[0] - PANEL_WIDTH).abs() < EPS);
        assert!(((pr[0] + pr[2]) / 2.0 - 640.0).abs() < EPS);
        assert!((pr[1] - PANEL_TOP_MARGIN).abs() < EPS);
        // точные координаты (все значения представимы в f32 точно)
        assert_eq!(pr, [410.0, 12.0, 870.0, 156.0]);

        // поле ввода внутри панели с отступом
        let ir = lay.input_rect;
        assert_eq!(ir, [418.0, 20.0, 862.0, 56.0]);
        assert!((ir[1] - (pr[1] + PANEL_PADDING)).abs() < EPS);
        assert!((pr[2] - PANEL_PADDING - ir[2]).abs() < EPS);
        assert!((ir[3] - ir[1] - INPUT_HEIGHT).abs() < EPS);

        // три строки
        assert_eq!(lay.row_rects.len(), 3);
        let first = lay.row_rects[0];
        assert!((first[1] - (ir[3] + PANEL_PADDING)).abs() < EPS); // ниже поля
        assert!((first[3] - first[1] - ROW_HEIGHT).abs() < EPS);
        assert!((first[0] - ir[0]).abs() < EPS);
        assert!((first[2] - ir[2]).abs() < EPS);
        // строки идут подряд
        assert!((lay.row_rects[1][1] - first[3]).abs() < EPS);
        assert_eq!(lay.row_rects[0], [418.0, 64.0, 862.0, 92.0]);
        assert_eq!(lay.row_rects[2], [418.0, 120.0, 862.0, 148.0]);
        // панель заканчивается отступом ниже последней строки
        let last = lay.row_rects[2];
        assert!((pr[3] - (last[3] + PANEL_PADDING)).abs() < EPS);
    }

    /// 12 строк — максимум 8 видимых прямоугольников, каждый высотой
    /// ROW_HEIGHT.
    #[test]
    fn layout_caps_rows_at_eight() {
        let panel = panel_with_rows(12);
        let lay = layout(1280.0, 720.0, &panel);
        assert_eq!(lay.row_rects.len(), MAX_VISIBLE_ROWS);
        for r in &lay.row_rects {
            assert!((r[3] - r[1] - ROW_HEIGHT).abs() < EPS);
        }
    }

    /// Узкое окно 300×600: ширина клампится к 300 − 2×12 = 276; широкое окно —
    /// панель полной ширины PANEL_WIDTH.
    #[test]
    fn layout_narrow_window_clamps_width() {
        let panel = panel_with_rows(3);
        let lay = layout(300.0, 600.0, &panel);
        let pr = lay.panel_rect;
        assert!((pr[2] - pr[0] - 276.0).abs() < EPS);
        assert!(((pr[0] + pr[2]) / 2.0 - 150.0).abs() < EPS);
        assert_eq!(lay.row_rects.len(), 3);

        // широкое окно — полная ширина
        let lay = layout(2000.0, 1000.0, &panel);
        assert!((lay.panel_rect[2] - lay.panel_rect[0] - PANEL_WIDTH).abs() < EPS);
    }

    /// scroll_top задаёт окно строк: 10 строк со scroll_top=5 — прямоугольники
    /// строк 5..10; scroll_top за пределами списка клампится (строк нет).
    #[test]
    fn layout_respects_scroll_window() {
        let mut panel = panel_with_rows(10);
        panel.scroll_top = 5;
        let lay = layout(1280.0, 720.0, &panel);
        // окно прокрутки: строки 5..10 — 5 видимых
        assert_eq!(lay.row_rects.len(), 5);
        let rows_top = PANEL_TOP_MARGIN + PANEL_PADDING + INPUT_HEIGHT + PANEL_PADDING;
        for (i, r) in lay.row_rects.iter().enumerate() {
            assert!((r[1] - (rows_top + i as f32 * ROW_HEIGHT)).abs() < EPS);
            assert!((r[3] - r[1] - ROW_HEIGHT).abs() < EPS);
        }

        // scroll_top больше длины списка — кламп, видимых строк нет
        panel.scroll_top = 20;
        let lay = layout(1280.0, 720.0, &panel);
        assert!(lay.row_rects.is_empty());
    }

    /// Вырожденное окно (ширина/высота ≤ 0, ширина меньше двух боковых
    /// отступов) — панель схлопывается в точку, без паники; крошечная ширина
    /// не выворачивает внутренние rect'ы.
    #[test]
    fn layout_degenerate_window_collapses() {
        let panel = panel_with_rows(3);
        for (w, h) in [
            (0.0, 600.0),
            (-40.0, 600.0),
            (1280.0, 0.0),
            (1280.0, -5.0),
            (24.0, 600.0),
        ] {
            let lay = layout(w, h, &panel);
            let pr = lay.panel_rect;
            assert!((pr[2] - pr[0]).abs() < EPS, "нулевая ширина при ({w}, {h})");
            assert!(lay.row_rects.is_empty(), "нет строк при ({w}, {h})");
        }

        // крошечное окно (30 px): ширина 6, внутренние rect'ы не вывернуты
        let lay = layout(30.0, 600.0, &panel);
        assert!((lay.panel_rect[2] - lay.panel_rect[0] - 6.0).abs() < EPS);
        assert!(lay.input_rect[2] >= lay.input_rect[0]);
        assert_eq!(lay.row_rects.len(), 3);
    }

    /// 0 строк: панель — только поле ввода (высота = отступ + поле + отступ).
    #[test]
    fn layout_without_rows_is_input_only() {
        let mut panel = SearchPanel::default();
        panel.open();
        panel.set_results(Vec::new());
        let lay = layout(1280.0, 720.0, &panel);
        assert!(lay.row_rects.is_empty());
        let pr = lay.panel_rect;
        let expected_h = PANEL_TOP_MARGIN + PANEL_PADDING + INPUT_HEIGHT + PANEL_PADDING;
        assert!((pr[3] - expected_h).abs() < EPS);
        assert!((pr[3] - 64.0).abs() < EPS); // 12 + 8 + 36 + 8
    }

    // ---------- scan_scene ----------

    /// Латиница: регистр запроса и заголовка не важен.
    #[test]
    fn scan_latin_case_insensitive() {
        let entries = [SceneEntry {
            node: 0,
            title: "Report.TXT",
            text: "",
        }];
        assert_eq!(scan_scene("report", &entries), vec![0usize]);
        assert_eq!(scan_scene("REPORT", &entries), vec![0usize]);
        assert_eq!(scan_scene("rep", &entries), vec![0usize]);
        assert_eq!(scan_scene("missing", &entries), Vec::<usize>::new());
    }

    /// Кириллица: регистронезависимый substring (критерий приёмки T14 —
    /// «смета» находит «Смета_2026.xlsx»).
    #[test]
    fn scan_cyrillic_case_insensitive() {
        let entries = [SceneEntry {
            node: 3,
            title: "Смета_2026.xlsx",
            text: "",
        }];
        assert_eq!(scan_scene("смета", &entries), vec![3usize]);
        assert_eq!(scan_scene("СМЕТА", &entries), vec![3usize]);
        assert_eq!(scan_scene("2026", &entries), vec![3usize]);
        assert_eq!(scan_scene("план", &entries), Vec::<usize>::new());
    }

    /// Матч по text (заметка), не только по title.
    #[test]
    fn scan_matches_text_not_only_title() {
        let entries = [
            SceneEntry {
                node: 0,
                title: "Заметка про бюджет",
                text: "здесь упоминается смета 2026",
            },
            SceneEntry {
                node: 1,
                title: "План",
                text: "черновик",
            },
        ];
        // матч только по text
        assert_eq!(scan_scene("упоминается", &entries), vec![0usize]);
        // матч по title
        assert_eq!(scan_scene("план", &entries), vec![1usize]);
        // матч по text второй записи
        assert_eq!(scan_scene("черновик", &entries), vec![1usize]);
        assert_eq!(scan_scene("нет такого", &entries), Vec::<usize>::new());
    }

    /// Пустой запрос — пустой результат; пустой список записей — пусто.
    #[test]
    fn scan_empty_query_or_entries() {
        let entries = [SceneEntry {
            node: 0,
            title: "смета",
            text: "",
        }];
        assert_eq!(scan_scene("", &entries), Vec::<usize>::new());
        assert!(scan_scene("смета", &[]).is_empty());
    }

    /// Дубликат ноды (две записи с одним node) — в результате один раз.
    #[test]
    fn scan_deduplicates_nodes() {
        let entries = [
            SceneEntry {
                node: 2,
                title: "смета",
                text: "",
            },
            SceneEntry {
                node: 2,
                title: "другой заголовок",
                text: "тоже про смету",
            },
            SceneEntry {
                node: 5,
                title: "смета_2026",
                text: "",
            },
        ];
        assert_eq!(scan_scene("смет", &entries), vec![2usize, 5usize]);
    }

    /// Порядок результата — порядок первых вхождений во входном списке.
    #[test]
    fn scan_preserves_entry_order() {
        let entries = [
            SceneEntry {
                node: 9,
                title: "смета",
                text: "",
            },
            SceneEntry {
                node: 4,
                title: "прочее",
                text: "",
            },
            SceneEntry {
                node: 7,
                title: "СМЕТА",
                text: "",
            },
        ];
        assert_eq!(scan_scene("смета", &entries), vec![9usize, 7usize]);
    }
}
