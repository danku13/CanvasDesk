//! FR-106 (мультиканвас C3, issue #7): оверлей менеджера канвасов —
//! состояние без I/O + чистые функции раскладки/hit-зон.
//!
//! Паттерн модуля — `scheme_gallery_ui` (FR-049) / `storage_ui` (FR-105):
//! раскладка — чистые функции от вьюпорта и состояния (один источник
//! геометрии для отрисовки, ввода и реестра поверхностей — draw == hit);
//! отрисовка и ввод — `app.rs`/`app/overlays.rs`/`app/input.rs`. Геометрия —
//! токены `canvas_core::tokens` + константы кита (`kit::LIST_ROW_H`,
//! `BUTTON_HEIGHT`); цвета в отрисовке — только слоты `KitPalette`.
//!
//! Клавиатура (№9): Esc — закрыть; ↑/↓ — выбор (скролл следует за ним);
//! Enter — открыть выбранный; F2 — ренейм выбранного; Esc в ренейме —
//! отмена ренейма (НЕ закрытие оверлея); печатаемые символы — в буфер
//! фильтра (или ренейма, если он активен). default.canvas — обычная
//! строка без спец-логики (№24a).
//!
//! Строка хранилища (№51a) видна ВСЕГДА внизу оверлея: OPFS-режим —
//! «Хранилище: браузерное [Переехать на диск…]» (кнопки нет в
//! Firefox/Safari — не обещаем диск, `fs_access::available()`), режим
//! папки — «Хранилище: папка на диске» без кнопки.

use canvas_core::tokens;
use canvas_core::workspace::{group_entries, sorted_entries, CanvasEntry, SortMode};
use canvas_ui::kit;

/// Ширина панели менеджера (лог. px): имя + бейдж источника + дата —
/// 640 влезает в минимальный G4-вьюпорт (800) с полями.
pub const PANEL_W: f32 = 640.0;
/// Высота шапки (титул + «✕»).
pub const HEADER_H: f32 = 40.0;
/// Высота строки поиска (кит `TEXT_FIELD_HEIGHT`).
pub const SEARCH_H: f32 = kit::TEXT_FIELD_HEIGHT;
/// Полный шаг строки списка (строка + зазор `LIST_ROW_GAP`).
pub const ROW_STEP: f32 = kit::LIST_ROW_H + kit::LIST_ROW_GAP;
/// Окно списка (без скролла до 9 канвасов; больше — колесо).
pub const VISIBLE_ROWS: usize = 9;
/// Поле панели (spacing-scale).
pub const PANEL_PAD: f32 = tokens::SPACING_LG;
/// Ширина колонки даты изменения (дд.мм.гггг, caption).
pub const TS_W: f32 = 66.0;
/// Ширина бейджа источника записи (браузерное/папка/диск, caption).
pub const BADGE_W: f32 = 86.0;
/// Высота строки хранилища внизу панели (№51a).
pub const STORAGE_ROW_H: f32 = kit::BUTTON_HEIGHT + tokens::SPACING_SM;
/// Порог двойного клика (мс) — выбор имени/строки (ренейм №9 / открыть).
pub const DOUBLE_CLICK_MS: u128 = 400;

// ============================================================================
// Режим строки хранилища (№51a) — из ActiveKind/StartMode web-слоя
// ============================================================================

/// Режим строки хранилища внизу менеджера (№51a): из web_state (приезжает
/// событием `AppEvent::StorageMode` вместе с листингом).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageRowMode {
    /// Браузерное OPFS-хранилище; `fs_available = false` (Firefox/Safari) —
    /// кнопки «Переехать на диск…» нет (не обещаем диск, которого нет).
    Browser { fs_available: bool },
    /// Granted-папка — кнопки переезда нет (уже на диске).
    Folder,
    /// FR-108 (C5, №20): натив — файлы на диске (источник строк — недавние
    /// из config.toml). Кнопки переезда нет: файлами владеет ОС.
    Files,
}

/// Показывать ли кнопку «Переехать на диск…» (№51a): только OPFS-режим в
/// браузере с FS Access.
pub fn storage_move_available(mode: StorageRowMode) -> bool {
    matches!(mode, StorageRowMode::Browser { fs_available: true })
}

// ============================================================================
// Строка списка: заголовок группы репо (№43a) или канвас
// ============================================================================

/// Строка окна списка менеджера.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagerRow {
    /// Заголовок группы-зеркала репозитория (`repos/<имя>`, №43a).
    Group(String),
    /// Канвас — индекс в `CanvasManagerState::entries`.
    Entry(usize),
}

/// Отфильтрованный, отсортированный и сгруппированный список строк (№43a):
/// корень плоско, затем группы `repos/<имя>` в алфавите с заголовками.
/// Фильтр — подстрока по `display_name` (регистронезависимо, №3).
pub fn rows(entries: &[CanvasEntry], filter: &str, sort: SortMode) -> Vec<ManagerRow> {
    let needle = filter.trim().to_lowercase();
    let filtered: Vec<CanvasEntry> = entries
        .iter()
        .filter(|entry| needle.is_empty() || entry.display().to_lowercase().contains(&needle))
        .cloned()
        .collect();
    // Позиции оригинала: имя файла — канонический идентификатор записи
    // (регистронезависимая уникальность — инвариант листинга, C0).
    let index_of: std::collections::HashMap<&str, usize> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.name.as_str(), index))
        .collect();
    let mut out = Vec::new();
    let sorted = sorted_entries(&filtered, sort);
    let groups = group_entries(&sorted);
    let push = |out: &mut Vec<ManagerRow>, entry: &CanvasEntry| {
        if let Some(&index) = index_of.get(entry.name.as_str()) {
            out.push(ManagerRow::Entry(index));
        }
    };
    for entry in &groups.root {
        push(&mut out, entry);
    }
    for (repo, group_entries) in &groups.repos {
        out.push(ManagerRow::Group(repo.clone()));
        for entry in group_entries {
            push(&mut out, entry);
        }
    }
    out
}

/// Сдвиг выбора по списку строк (↑/↓): заголовки групп пропускаются,
/// кламп к первой/последней канвас-строке. `selected` — индекс в `rows()`.
pub fn move_selection(rows: &[ManagerRow], selected: usize, delta: i32) -> usize {
    let is_entry = |index: usize| matches!(rows.get(index), Some(ManagerRow::Entry(_)));
    let Some(start) = (0..rows.len()).find(|&i| is_entry(i)) else {
        return 0; // список пуст — любое значение безопасно (клампится выше)
    };
    // Точка старта: текущая позиция, если это канвас-строка, иначе первая.
    let mut current = if is_entry(selected) { selected } else { start };
    let step = delta.signum();
    let mut left = delta.saturating_abs();
    while left > 0 {
        let next = (current as i64 + step as i64).clamp(0, rows.len() as i64 - 1) as usize;
        if next == current {
            break; // упёрлись в край
        }
        current = next;
        if is_entry(current) {
            left -= 1;
        }
    }
    current
}

// ============================================================================
// Дата изменения строки (чистое форматирование unix ms → дд.мм.гггг)
// ============================================================================

/// Дата `дд.мм.гггг` из unix ms (UTC): без внешних зависимостей — алгоритм
/// days→civil (Говард Хиннант, общественный домен). Битые значения (0/
/// переполнение) клампятся в корректные даты — строки не паникуют.
pub fn format_ts(ts: u64) -> String {
    let days = (ts / 86_400_000) as i64;
    // days→civil: сдвиг эпохи 1970-01-01 → 0000-03-01 (алгоритм Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{d:02}.{m:02}.{y:04}")
}

// ============================================================================
// Состояние менеджера (без I/O)
// ============================================================================

/// Состояние оверлея менеджера канвасов (план §3.2; паттерн
/// `SchemeGalleryState`). `selected`/`scroll_top` — ординалы в
/// [`rows`]-списке (заголовки групп входят в нумерацию окна видимости,
/// но не выбираются — см. [`move_selection`]).
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasManagerState {
    /// Оверлей открыт.
    pub open: bool,
    /// Буфер поиска по подстроке имени (№3/№9; печатаемые символы).
    pub filter: String,
    /// Ординал выбранной строки (в `rows()`-списке).
    pub selected: usize,
    /// Верх окна видимости списка (в `rows()`-списке).
    pub scroll_top: usize,
    /// Листинг канвасов от web-слоя (`AppEvent::CanvasList`).
    pub entries: Vec<CanvasEntry>,
    /// Инлайн-ренейм (№9): индекс в `entries` + буфер редактирования.
    pub editing: Option<(usize, String)>,
    /// Режим сортировки (тогл в UI, №43a).
    pub sort: SortMode,
    /// Режим строки хранилища (№51a; `AppEvent::StorageMode`).
    pub storage: StorageRowMode,
    /// FR-108 (C5, №20): тонкий нативный слой — источник строк менеджера
    /// НЕ workspace-хранилище web-слоя, а недавние файлы из config.toml
    /// (`Settings::recent`). Ставится платформой сборки (`cfg!` в
    /// `App::new`): web — false (конвейер FR-104/105/106), натив — true.
    /// Компиляция обоих режимов — чистые тесты раскладки гоняют оба.
    pub native: bool,
}

impl Default for CanvasManagerState {
    fn default() -> Self {
        Self {
            open: false,
            filter: String::new(),
            selected: 0,
            scroll_top: 0,
            entries: Vec::new(),
            editing: None,
            sort: SortMode::ModifiedDesc,
            storage: StorageRowMode::Browser { fs_available: true },
            native: false,
        }
    }
}

impl CanvasManagerState {
    /// Открыть оверлей (сброс переходного состояния; листинг запросит
    /// вызывающий — конвейер `request_canvas_list`).
    pub fn open(&mut self) {
        self.open = true;
        self.filter.clear();
        self.selected = 0;
        self.scroll_top = 0;
        self.editing = None;
    }

    /// Закрыть оверлей (буфер ренейма теряется — отмена).
    pub fn close(&mut self) {
        self.open = false;
        self.filter.clear();
        self.selected = 0;
        self.scroll_top = 0;
        self.editing = None;
    }

    /// Свежий листинг от web-слоя: ренейм отменяется (индексы устарели),
    /// выбор/скролл клампятся к новому списку.
    pub fn set_entries(&mut self, entries: Vec<CanvasEntry>) {
        self.entries = entries;
        self.editing = None;
        self.clamp();
    }

    /// Кламп выбора/скролла к текущему `rows()`-списку.
    pub fn clamp(&mut self) {
        let len = self.rows().len();
        self.selected = self.selected.min(len.saturating_sub(1));
        self.scroll_top = self.scroll_top.min(self.max_scroll_top());
    }

    /// Текущие строки окна списка (фильтр + сортировка + группы).
    pub fn rows(&self) -> Vec<ManagerRow> {
        rows(&self.entries, &self.filter, self.sort)
    }

    /// Индекс выбранного канваса в `entries` (None — заголовок/пусто).
    pub fn selected_entry(&self) -> Option<usize> {
        match self.rows().get(self.selected) {
            Some(ManagerRow::Entry(index)) => Some(*index),
            _ => None,
        }
    }

    /// Выбрать строку канваса (индекс в `entries`); скролл следует за
    /// выбором.
    pub fn select_entry(&mut self, entry: usize) {
        if let Some(ord) = self
            .rows()
            .iter()
            .position(|row| matches!(row, ManagerRow::Entry(i) if *i == entry))
        {
            self.selected = ord;
            self.scroll_top =
                scroll_to_reveal(self.scroll_top, self.selected, self.max_scroll_top());
        }
    }

    fn max_scroll_top(&self) -> usize {
        self.rows().len().saturating_sub(VISIBLE_ROWS)
    }

    // --- инлайн-ренейм (№9) ---------------------------------------------------

    /// Начать ренейм канваса: буфер = текущее отображаемое имя.
    pub fn begin_rename(&mut self, entry: usize) {
        if let Some(existing) = self.entries.get(entry) {
            self.editing = Some((entry, existing.display().to_owned()));
        }
    }

    /// Символ в буфер ренейма.
    pub fn edit_insert(&mut self, ch: char) {
        if let Some((_, buffer)) = self.editing.as_mut() {
            if buffer.chars().count() < canvas_core::workspace::MAX_NAME_LEN {
                buffer.push(ch);
            }
        }
    }

    /// Backspace в буфере ренейма.
    pub fn edit_backspace(&mut self) {
        if let Some((_, buffer)) = self.editing.as_mut() {
            buffer.pop();
        }
    }

    /// Отменить ренейм (Esc — оверлей жив).
    pub fn cancel_edit(&mut self) {
        self.editing = None;
    }

    /// Подтвердить ренейм: забрать (индекс, буфер) — валидация/коллизии —
    /// вызывающий (нужны i18n и полный листинг).
    pub fn take_edit(&mut self) -> Option<(usize, String)> {
        self.editing.take()
    }

    /// Буфер ренейма (рендер строки; None — ренейма нет).
    pub fn edit_buffer(&self) -> Option<&str> {
        self.editing.as_ref().map(|(_, buffer)| buffer.as_str())
    }
}

// ============================================================================
// Скролл окна списка (образец — scheme_gallery/wheel_scroll_top)
// ============================================================================

/// Скролл вслед за клавиатурным выбором: выбранная строка обязана попасть в
/// окно `VISIBLE_ROWS` (сверху, если ушла выше; прижаться к низу, если
/// ниже). Уже видимая — без изменений.
pub fn scroll_to_reveal(scroll_top: usize, selected: usize, max_top: usize) -> usize {
    let bottom = scroll_top + VISIBLE_ROWS;
    if selected < scroll_top {
        selected.min(max_top)
    } else if selected >= bottom {
        (selected + 1 - VISIBLE_ROWS).min(max_top)
    } else {
        scroll_top
    }
}

/// Верх окна после колес-скролла (шаг — строки): кламп `0..=max_top`,
/// хвост списка достижим, пустого окна нет.
pub fn wheel_scroll_top(current: usize, delta_rows: f32, rows_len: usize) -> usize {
    let max_top = rows_len.saturating_sub(VISIBLE_ROWS);
    let next = (current as f32 + delta_rows).round();
    if next <= 0.0 {
        0
    } else if next >= max_top as f32 {
        max_top
    } else {
        next as usize
    }
}

// ============================================================================
// Раскладка оверлея (чистая функция; кламп к вьюпорту)
// ============================================================================

/// Измеренные потребителем ширины (один источник геометрии — кнопки кита
/// `kit::button_size`; сюда передаются готовые числа, раскладка не мерит).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ManagerWidths {
    /// Чип-тогл сортировки (canvas.manager.sort_*).
    pub sort_chip: f32,
    /// «Создать» (canvas.manager.create_empty — короткая подпись кнопки).
    pub create: f32,
    /// «Из шаблона…».
    pub template: f32,
    /// «Импорт файла…».
    pub import: f32,
    /// «Дублировать».
    pub duplicate: f32,
    /// «Переименовать».
    pub rename: f32,
    /// «Удалить».
    pub delete: f32,
    /// «Экспорт».
    pub export: f32,
    /// Подпись строки хранилища (canvas.storage.*).
    pub storage_label: f32,
    /// «Переехать на диск…» (0 — кнопки нет: папка/Firefox).
    pub storage_move: f32,
    /// CTA пустого состояния «Создать канвас».
    pub empty_create: f32,
    /// Вторичное пустого состояния «Открыть файл с диска…».
    pub empty_disk: f32,
}

/// Строка окна списка (hit-зоны: вся строка — выбор/открытие двойным
/// кликом; зона имени — ренейм двойным кликом, №9).
#[derive(Debug, Clone, PartialEq)]
pub struct ManagerRowLayout {
    /// Вся строка.
    pub row: [f32; 4],
    /// Зона имени (двойной клик — ренейм, НЕ открытие).
    pub name: [f32; 4],
    /// Индекс в `entries` (None — заголовок группы).
    pub entry: Option<usize>,
    /// Имя группы-репозитория (только заголовки, №43a).
    pub group: Option<String>,
    /// Выбрана (клавиатура/клик).
    pub is_selected: bool,
    /// Активный канвас.
    pub is_active: bool,
    /// Заголовок группы репо (рендер — подпись + разделитель).
    pub is_header: bool,
    /// Строка в режиме инлайн-ренейма.
    pub editing: bool,
}

/// Раскладка оверлея менеджера.
#[derive(Debug, Clone, PartialEq)]
pub struct ManagerLayout {
    /// Панель.
    pub panel: [f32; 4],
    /// Титул (canvas.manager.title).
    pub title: [f32; 4],
    /// «✕».
    pub close: [f32; 4],
    /// Поле поиска (canvas.manager.search).
    pub search: [f32; 4],
    /// Чип-тогл сортировки.
    pub sort_chip: [f32; 4],
    /// Кнопки создания (№7): пустой / из шаблона / импорт.
    pub create: [f32; 4],
    pub template: [f32; 4],
    pub import: [f32; 4],
    /// Строки окна видимости (`first_row..+`).
    pub rows: Vec<ManagerRowLayout>,
    /// Ординал первой видимой строки (в `rows()`-списке).
    pub first_row: usize,
    /// Кнопки выбранной строки: дублировать/переименовать/удалить/экспорт.
    pub duplicate: [f32; 4],
    pub rename: [f32; 4],
    pub delete: [f32; 4],
    pub export: [f32; 4],
    /// Строка хранилища (№51a): подпись слева, кнопка справи (0×0 — нет).
    pub storage_label: [f32; 4],
    pub storage_move: [f32; 4],
    /// Пустое состояние (№23a): карточка CTA вместо списка.
    pub empty: Option<EmptyCardLayout>,
    /// Есть ли строки (false → пустое состояние, даже если фильтр пуст).
    pub has_entries: bool,
}

/// Карточка пустого состояния (№23a): заголовок + подсказка + CTA.
#[derive(Debug, Clone, PartialEq)]
pub struct EmptyCardLayout {
    pub title: [f32; 4],
    pub hint: [f32; 4],
    /// CTA «Создать канвас» (primary).
    pub create: [f32; 4],
    /// «Открыть файл с диска…» (secondary).
    pub open_disk: [f32; 4],
}

/// Раскладка менеджера (чистая; `active` — имя файла активного канваса
/// для подсветки строки). Ширины кнопок измеряет потребитель
/// (`kit::button_size`), здесь — только геометрия.
pub fn manager_layout(
    viewport: [f32; 2],
    state: &CanvasManagerState,
    active: &str,
    widths: &ManagerWidths,
) -> ManagerLayout {
    let rows_all = state.rows();
    let has_entries = !state.entries.is_empty();
    let empty_shown = !has_entries;
    let gap = tokens::SPACING_SM;
    // Вертикальный стек (курсор сверху): пад → шапка → поиск → кнопки
    // создания → список → кнопки строки → строка хранилища → пад.
    // Пустое состояние (№23a): вместо списка и кнопок строки — карточка
    // CTA той же высоты, что окно списка из двух строк.
    // FR-108 (C5, native): кнопок выбранной строки нет (тонкий слой —
    // файлами владеет ОС) — ряд не занимает высоту.
    let top_chrome = PANEL_PAD + HEADER_H + gap + SEARCH_H + gap + kit::BUTTON_HEIGHT + gap;
    let bottom_chrome = if empty_shown {
        EMPTY_CARD_H + gap + STORAGE_ROW_H + PANEL_PAD
    } else if state.native {
        gap + STORAGE_ROW_H + PANEL_PAD
    } else {
        gap + kit::BUTTON_HEIGHT + gap + STORAGE_ROW_H + PANEL_PAD
    };
    // Панель клампится к вьюпорту (инвариант 320×240): список резиновый,
    // окно — не больше VISIBLE_ROWS строк.
    let max_h = (viewport[1] - tokens::SPACING_XL * 2.0).max(160.0);
    let list_avail = (max_h - top_chrome - bottom_chrome).max(ROW_STEP);
    let shown_rows = if empty_shown {
        0
    } else {
        (((list_avail / ROW_STEP).floor() as usize).clamp(1, VISIBLE_ROWS))
            .min(rows_all.len().saturating_sub(state.scroll_top))
    };
    let height = top_chrome + shown_rows as f32 * ROW_STEP + bottom_chrome;
    let width = PANEL_W.min(viewport[0]);
    let x = ((viewport[0] - width) * 0.5).max(0.0);
    let y = ((viewport[1] - height) * 0.5).max(0.0);
    let panel = [x, y, width, height];
    let inner_x = x + PANEL_PAD;
    let inner_w = width - PANEL_PAD * 2.0;
    let right = inner_x + inner_w;

    // Шапка (титул + «✕» справа).
    let title = [
        inner_x,
        y + PANEL_PAD + (HEADER_H - kit::BANNER_LABEL_LINE_H) * 0.5,
        inner_w - HEADER_H,
        kit::BANNER_LABEL_LINE_H,
    ];
    let close = [
        x + width - HEADER_H,
        y + PANEL_PAD + (HEADER_H - kit::ICON_BUTTON_SIZE) * 0.5,
        kit::ICON_BUTTON_SIZE,
        kit::ICON_BUTTON_SIZE,
    ];

    // Строка поиска + чип сортировки справа.
    let search_y = y + PANEL_PAD + HEADER_H + gap;
    let sort_chip = [
        (right - widths.sort_chip).max(inner_x),
        search_y + (SEARCH_H - kit::CHIP_HEIGHT) * 0.5,
        widths.sort_chip,
        kit::CHIP_HEIGHT,
    ];
    let search = [
        inner_x,
        search_y,
        (sort_chip[0] - tokens::SPACING_SM - inner_x).max(tokens::SPACING_MD),
        SEARCH_H,
    ];

    // Кнопки создания (№7): слева направо. FR-108 (native): «Из шаблона…»
    // нет (тонкий слой), «Импорт файла…» заменён на «Открыть…» (слот
    // `import` — тот же hit-id, платформенная ветка в клике).
    let actions_y = search_y + SEARCH_H + gap;
    let create = [inner_x, actions_y, widths.create, kit::BUTTON_HEIGHT];
    let template = if state.native {
        [0.0; 4]
    } else {
        [
            create[0] + create[2] + tokens::SPACING_SM,
            actions_y,
            widths.template,
            kit::BUTTON_HEIGHT,
        ]
    };
    let import = [
        if state.native {
            create[0] + create[2] + tokens::SPACING_SM
        } else {
            template[0] + template[2] + tokens::SPACING_SM
        },
        actions_y,
        widths.import,
        kit::BUTTON_HEIGHT,
    ];

    // Список (или карточка пустого состояния №23a).
    let list_y = actions_y + kit::BUTTON_HEIGHT + gap;
    let mut rows_layout = Vec::with_capacity(shown_rows);
    let first_row = state.scroll_top;
    let editing_entry = state.editing.as_ref().map(|(index, _)| *index);
    for (offset, row) in rows_all.iter().enumerate().skip(first_row).take(shown_rows) {
        let ry = list_y + (offset - first_row) as f32 * ROW_STEP;
        let (entry_index, group, is_header) = match row {
            ManagerRow::Group(name) => (None, Some(name.clone()), true),
            ManagerRow::Entry(index) => (Some(*index), None, false),
        };
        let is_selected = offset == state.selected && !is_header;
        let is_active = entry_index
            .and_then(|index| state.entries.get(index))
            .is_some_and(|e| e.name.eq_ignore_ascii_case(active));
        let row_rect = [inner_x, ry, inner_w, kit::LIST_ROW_H];
        // Зона имени: всё, что левее бейджа+даты (заголовок группы — вся
        // строка); хит-зона стартует с края строки — двойной клик по левой
        // части строки попадает в ренейм, а не в открытие.
        let name_w = if is_header {
            inner_w
        } else {
            (inner_w - BADGE_W - TS_W - tokens::SPACING_MD * 2.0).max(0.0)
        };
        let name = [inner_x, ry, name_w, kit::LIST_ROW_H];
        rows_layout.push(ManagerRowLayout {
            row: row_rect,
            name,
            entry: entry_index,
            group,
            is_selected,
            is_active,
            is_header,
            editing: editing_entry.is_some_and(|index| Some(index) == entry_index),
        });
    }

    // Кнопки выбранной строки / карточка пустого состояния.
    let after_list_y = list_y + shown_rows as f32 * ROW_STEP + gap;
    let empty = if empty_shown {
        Some(empty_card_layout(inner_x, list_y, inner_w, widths))
    } else {
        None
    };
    // FR-108 (native): ряд кнопок строки скрыт — ренейм/удаление/дубликат/
    // экспорт — зона ОС и будущих волн (сознательное ограничение C5,
    // FR-108 §Ограничения).
    let (duplicate, rename, delete, export_btn) = if empty_shown || state.native {
        ([0.0; 4], [0.0; 4], [0.0; 4], [0.0; 4])
    } else {
        // Справа налево: Экспорт, Удалить, Переименовать, Дублировать.
        let by = after_list_y;
        let export_r = [right - widths.export, by, widths.export, kit::BUTTON_HEIGHT];
        let delete = [
            export_r[0] - tokens::SPACING_SM - widths.delete,
            by,
            widths.delete,
            kit::BUTTON_HEIGHT,
        ];
        let rename = [
            delete[0] - tokens::SPACING_SM - widths.rename,
            by,
            widths.rename,
            kit::BUTTON_HEIGHT,
        ];
        let duplicate = [
            rename[0] - tokens::SPACING_SM - widths.duplicate,
            by,
            widths.duplicate,
            kit::BUTTON_HEIGHT,
        ];
        (duplicate, rename, delete, export_r)
    };

    // Строка хранилища (№51a): всегда внизу панели (кнопки строки — над ней).
    // FR-108 (native): ряд кнопок строки пуст — строка хранилища идёт сразу
    // за списком (bottom_chrome без BUTTON_HEIGHT).
    let storage_y = if empty_shown {
        list_y + EMPTY_CARD_H + gap
    } else if state.native {
        after_list_y
    } else {
        after_list_y + kit::BUTTON_HEIGHT + gap
    };
    let show_move = storage_move_available(state.storage) && widths.storage_move > 0.0;
    let storage_move = if show_move {
        [
            right - widths.storage_move,
            storage_y + (STORAGE_ROW_H - kit::BUTTON_HEIGHT) * 0.5,
            widths.storage_move,
            kit::BUTTON_HEIGHT,
        ]
    } else {
        [0.0; 4]
    };
    let storage_label = [
        inner_x,
        storage_y + (STORAGE_ROW_H - kit::BANNER_LABEL_LINE_H) * 0.5,
        if show_move {
            (storage_move[0] - tokens::SPACING_SM - inner_x).max(0.0)
        } else {
            inner_w
        }
        .min(widths.storage_label),
        kit::BANNER_LABEL_LINE_H,
    ];

    ManagerLayout {
        panel,
        title,
        close,
        search,
        sort_chip,
        create,
        template,
        import,
        rows: rows_layout,
        first_row,
        duplicate,
        rename,
        delete,
        export: export_btn,
        storage_label,
        storage_move,
        empty,
        has_entries,
    }
}

/// Высота карточки пустого состояния (№23a): заголовок + подсказка + кнопки.
const EMPTY_CARD_H: f32 = 150.0;

/// Карточка пустого состояния (№23a): по центру списка, CTA + вторичная.
fn empty_card_layout(x: f32, y: f32, w: f32, widths: &ManagerWidths) -> EmptyCardLayout {
    let title = [x, y + tokens::SPACING_LG, w, kit::BANNER_LABEL_LINE_H];
    let hint = [
        x,
        y + tokens::SPACING_LG + kit::BANNER_LABEL_LINE_H + tokens::SPACING_SM,
        w,
        kit::BANNER_LABEL_LINE_H,
    ];
    // Кнопки — по центру, рядом (primary слева).
    let both = widths.empty_create + tokens::SPACING_SM + widths.empty_disk;
    let bx = x + (w - both).max(0.0) * 0.5;
    let by = y + EMPTY_CARD_H - kit::BUTTON_HEIGHT - tokens::SPACING_LG;
    EmptyCardLayout {
        title,
        hint,
        create: [bx, by, widths.empty_create, kit::BUTTON_HEIGHT],
        open_disk: [
            bx + widths.empty_create + tokens::SPACING_SM,
            by,
            widths.empty_disk,
            kit::BUTTON_HEIGHT,
        ],
    }
}

/// Строка под точкой (hit-тест списка): индекс в `entries` (заголовок
/// группы — None, не интерактивен).
pub fn row_at(lay: &ManagerLayout, point: [f32; 2]) -> Option<Option<usize>> {
    for row in &lay.rows {
        if point_in_rect(row.row, point) {
            return Some(row.entry);
        }
    }
    None
}

/// Зона имени под точкой (двойной клик — ренейм №9): индекс в `entries`.
pub fn name_at(lay: &ManagerLayout, point: [f32; 2]) -> Option<usize> {
    for row in &lay.rows {
        if row.entry.is_some() && point_in_rect(row.name, point) {
            return row.entry;
        }
    }
    None
}

/// Точка в xywh-прямоугольнике (паритет `scheme_gallery_ui::point_in_rect`).
pub fn point_in_rect(rect: [f32; 4], point: [f32; 2]) -> bool {
    point[0] >= rect[0]
        && point[0] <= rect[0] + rect[2]
        && point[1] >= rect[1]
        && point[1] <= rect[1] + rect[3]
}

// ============================================================================
// Нативные тесты (чистая раскладка/состояние)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_core::workspace::EntryKind;

    fn entry(name: &str, ts: u64) -> CanvasEntry {
        CanvasEntry {
            name: name.to_owned(),
            ts,
            kind: EntryKind::Opfs,
            repo: None,
        }
    }

    fn repo_entry(name: &str, ts: u64, repo: &str) -> CanvasEntry {
        CanvasEntry {
            name: name.to_owned(),
            ts,
            kind: EntryKind::Folder,
            repo: Some(repo.to_owned()),
        }
    }

    // --- фильтр/сортировка/группы (№3/№43a) -----------------------------------

    #[test]
    fn rows_filter_by_display_name_substring_case_insensitive() {
        let entries = vec![
            entry("Проект.canvas", 1),
            entry("проект-2.canvas", 2),
            entry("Отчёт.canvas", 3),
        ];
        let matched = rows(&entries, "ПРОЕКТ", SortMode::Name);
        let names: Vec<_> = matched
            .iter()
            .filter_map(|r| match r {
                ManagerRow::Entry(i) => Some(entries[*i].name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(names, ["Проект.canvas", "проект-2.canvas"]);
        // Пустой фильтр — всё; фильтр без совпадений — пусто
        assert_eq!(rows(&entries, "", SortMode::Name).len(), 3);
        assert!(rows(&entries, "нет-такого", SortMode::Name).is_empty());
        // Фильтр не матчит по расширению файла, только по отображаемому имени
        assert_eq!(rows(&entries, "canvas", SortMode::Name).len(), 0);
    }

    #[test]
    fn rows_root_flat_then_repo_groups_with_headers() {
        let entries = vec![
            entry("мой.canvas", 1),
            repo_entry("svc.canvas", 2, "infra"),
            entry("default.canvas", 3),
            repo_entry("billing.canvas", 4, "mono"),
            repo_entry("core.canvas", 5, "infra"),
        ];
        // Имя (латиница < кириллицы): billing, core, default, мой, svc.
        assert_eq!(
            rows(&entries, "", SortMode::Name),
            vec![
                ManagerRow::Entry(2), // default.canvas
                ManagerRow::Entry(0), // мой.canvas
                ManagerRow::Group("infra".into()),
                ManagerRow::Entry(4), // core < svc
                ManagerRow::Entry(1),
                ManagerRow::Group("mono".into()),
                ManagerRow::Entry(3), // billing.canvas
            ],
            "корень плоско, группы в алфавите, внутри — сортировка"
        );
        // По дате: свежие сверху и в корне, и внутри групп
        assert_eq!(
            rows(&entries, "", SortMode::ModifiedDesc),
            vec![
                ManagerRow::Entry(2), // default (3)
                ManagerRow::Entry(0), // мой (1)
                ManagerRow::Group("infra".into()),
                ManagerRow::Entry(4), // core (5) свежее svc (2)
                ManagerRow::Entry(1),
                ManagerRow::Group("mono".into()),
                ManagerRow::Entry(3),
            ],
            "ModifiedDesc: default(3) → мой(1); infra: core(5) → svc(2)"
        );
    }

    #[test]
    fn filter_applies_to_groups_too() {
        let entries = vec![
            repo_entry("svc.canvas", 1, "infra"),
            repo_entry("bill.canvas", 2, "mono"),
        ];
        // Фильтр по имени канваса: остаётся строка + её заголовок группы
        let rows = rows(&entries, "bill", SortMode::Name);
        assert_eq!(
            rows,
            vec![ManagerRow::Group("mono".into()), ManagerRow::Entry(1)]
        );
    }

    // --- выбор (стрелки пропускают заголовки) ----------------------------------

    #[test]
    fn move_selection_skips_headers_and_clamps() {
        let rows = vec![
            ManagerRow::Entry(0),
            ManagerRow::Group("infra".into()),
            ManagerRow::Entry(2),
            ManagerRow::Entry(3),
        ];
        assert_eq!(move_selection(&rows, 0, 1), 2, "заголовок пропущен");
        assert_eq!(move_selection(&rows, 2, -1), 0);
        assert_eq!(move_selection(&rows, 0, -1), 0, "кламп вверх");
        assert_eq!(move_selection(&rows, 3, 1), 3, "кламп вниз");
        assert_eq!(move_selection(&rows, 2, 5), 3, "большой шаг клампится");
        // Выбор на заголовке — стрелки начинают с первой канвас-строки
        assert_eq!(move_selection(&rows, 1, 1), 2);
        // Пустой список — безопасный ноль
        assert_eq!(move_selection(&[], 0, 1), 0);
    }

    // --- скролл ------------------------------------------------------------------

    #[test]
    fn scroll_to_reveal_follows_selection() {
        let max_top = 20;
        assert_eq!(scroll_to_reveal(5, 3, max_top), 3, "выше окна — к нему");
        assert_eq!(scroll_to_reveal(0, 9, max_top), 1, "0+9=9 ≥ окно 0..9");
        assert_eq!(scroll_to_reveal(5, 5, max_top), 5, "видимая — как есть");
        assert_eq!(scroll_to_reveal(5, 13, max_top), 5, "13 в окне 5..14");
        assert_eq!(scroll_to_reveal(5, 14, max_top), 6, "14+1-9");
        assert_eq!(scroll_to_reveal(0, 100, max_top), 20, "кламп max_top");
        // Короткий список (окно длиннее) — без паники usize
        assert_eq!(scroll_to_reveal(0, 2, 0), 0);
    }

    #[test]
    fn wheel_scroll_clamps() {
        assert_eq!(wheel_scroll_top(0, 3.0, 20), 3);
        assert_eq!(wheel_scroll_top(4, -2.0, 20), 2);
        assert_eq!(wheel_scroll_top(3, 100.0, 20), 11, "кламп вниз (20-9)");
        assert_eq!(wheel_scroll_top(3, -100.0, 20), 0, "кламп вверх");
        assert_eq!(
            wheel_scroll_top(0, 5.0, 3),
            0,
            "короткий список — без скролла"
        );
        // Мелкие тачпад-шаги накапливаются (округление суммы)
        assert_eq!(wheel_scroll_top(0, 0.4, 20), 0);
        assert_eq!(wheel_scroll_top(0, 0.6, 20), 1);
    }

    // --- состояние ------------------------------------------------------------------

    #[test]
    fn state_open_close_and_set_entries_clamps() {
        let mut state = CanvasManagerState::default();
        state.open();
        assert!(state.open);
        state.filter = "запрос".into();
        state.set_entries(vec![entry("a.canvas", 1), entry("b.canvas", 2)]);
        assert_eq!(state.entries.len(), 2);
        state.filter.clear(); // фильтр «запрос» гасит список — кламп по полному
        state.selected = 5;
        state.scroll_top = 9;
        state.clamp();
        assert_eq!(state.selected, 1, "выбор клампится к списку");
        assert_eq!(state.scroll_top, 0, "скролл клампится (окно вмещает всё)");
        state.close();
        assert!(!state.open);
        assert!(state.filter.is_empty(), "фильтр сброшен");
        // Свежий листинг отменяет ренейм (индексы устарели)
        state.begin_rename(0);
        assert!(state.editing.is_some());
        state.set_entries(vec![entry("c.canvas", 1)]);
        assert!(state.editing.is_none(), "ренейм отменён сменой листинга");
    }

    #[test]
    fn state_selected_entry_resolves_rows() {
        let mut state = CanvasManagerState::default();
        state.set_entries(vec![
            entry("a.canvas", 2),
            repo_entry("svc.canvas", 1, "infra"),
            entry("b.canvas", 3),
        ]);
        state.sort = SortMode::Name;
        // rows (Name): a, b, [infra], svc → selected 0 = a
        state.selected = 0;
        assert_eq!(state.selected_entry(), Some(0));
        // selected на заголовке → None
        state.selected = 2;
        assert_eq!(
            state.selected_entry(),
            None,
            "заголовок группы не выбирается"
        );
        // select_entry ведёт скролл за выбором
        state.select_entry(1); // svc
        assert_eq!(state.selected_entry(), Some(1));
        assert_eq!(state.scroll_top, 0);
    }

    // --- инлайн-ренейм (№9) ----------------------------------------------------------

    #[test]
    fn rename_buffer_editing_ops() {
        let mut state = CanvasManagerState::default();
        state.set_entries(vec![entry("Идея.canvas", 1)]);
        state.begin_rename(0);
        assert_eq!(state.edit_buffer(), Some("Идея"));
        // Ввод: символы и backspace
        state.edit_insert('!');
        state.edit_insert(' ');
        state.edit_insert('2');
        assert_eq!(state.edit_buffer(), Some("Идея! 2"));
        state.edit_backspace();
        assert_eq!(state.edit_buffer(), Some("Идея! "));
        // Лимит длины — буфер не растёт сверх MAX_NAME_LEN
        state.begin_rename(0);
        for ch in "x".repeat(canvas_core::workspace::MAX_NAME_LEN).chars() {
            state.edit_insert(ch);
        }
        state.edit_insert('y');
        assert_eq!(
            state.edit_buffer().map(|buffer| buffer.chars().count()),
            Some(canvas_core::workspace::MAX_NAME_LEN),
            "лимит имени — буфер клампится"
        );
        // Отмена — буфер теряется
        state.cancel_edit();
        assert_eq!(state.edit_buffer(), None);
        // Коммит забирает (индекс, буфер)
        state.begin_rename(0);
        let taken = state.take_edit();
        assert_eq!(taken, Some((0, "Идея".into())));
        assert_eq!(state.edit_buffer(), None, "коммит гасит редактирование");
        // Нет такой записи — no-op
        state.begin_rename(9);
        assert_eq!(state.edit_buffer(), None);
    }

    // --- раскладка ---------------------------------------------------------------------

    fn widths() -> ManagerWidths {
        ManagerWidths {
            sort_chip: 120.0,
            create: 90.0,
            template: 110.0,
            import: 110.0,
            duplicate: 110.0,
            rename: 120.0,
            delete: 80.0,
            export: 80.0,
            storage_label: 200.0,
            storage_move: 140.0,
            empty_create: 130.0,
            empty_disk: 190.0,
        }
    }

    #[test]
    fn layout_places_chrome_rows_and_storage_row() {
        let mut state = CanvasManagerState::default();
        state.set_entries(
            (0..12)
                .map(|i| entry(&format!("c{i:02}.canvas"), i))
                .collect(),
        );
        state.sort = SortMode::Name; // c00 сверху — проверка активного
        let lay = manager_layout([1280.0, 800.0], &state, "c00.canvas", &widths());
        // Панель по центру, в пределах вьюпорта
        assert!((lay.panel[0] + lay.panel[2] / 2.0 - 640.0).abs() < 0.5);
        assert!(lay.panel[1] >= 0.0 && lay.panel[1] + lay.panel[3] <= 800.0);
        assert!(lay.has_entries);
        // Окно видимости ≤ VISIBLE_ROWS, первая строка — с scroll_top
        assert_eq!(lay.rows.len(), VISIBLE_ROWS);
        assert_eq!(lay.first_row, 0);
        // Строка хранилища всегда видна (№51a) — внутри панели, выше низа
        assert!(lay.storage_label[1] + lay.storage_label[3] <= lay.panel[1] + lay.panel[3]);
        assert!(
            lay.storage_move[2] > 0.0,
            "OPFS+FS → кнопка «Переехать…» есть"
        );
        // Кнопки строки: правый край «Экспорт» у правому полю панели
        assert!(
            (lay.export[0] + lay.export[2] - (lay.panel[0] + lay.panel[2] - PANEL_PAD)).abs() < 0.5
        );
        assert!(lay.duplicate[0] > lay.panel[0], "ряд кнопок внутри панели");
        // Активная строка подсвечена
        assert!(lay.rows[0].is_active, "c00.canvas — активный");
        assert!(!lay.rows[1].is_active);
    }

    #[test]
    fn layout_scrolls_window_and_hit_zones() {
        let mut state = CanvasManagerState::default();
        state.set_entries(
            (0..15)
                .map(|i| entry(&format!("c{i:02}.canvas"), i))
                .collect(),
        );
        state.sort = SortMode::Name; // c00…c14 — детерминированные ординалы
        state.scroll_top = 6;
        let lay = manager_layout([1280.0, 800.0], &state, "", &widths());
        assert_eq!(lay.first_row, 6);
        assert_eq!(lay.rows.len(), VISIBLE_ROWS, "хвост достижим");
        // hit: строка под точкой — индекс записи; заголовок не интерактивен
        let point = [lay.rows[0].row[0] + 4.0, lay.rows[0].row[1] + 4.0];
        assert_eq!(row_at(&lay, point), Some(Some(6)));
        // Зона имени — левая часть строки (ренейм двойным кликом №9)
        assert_eq!(name_at(&lay, point), Some(6));
        // Дата справа — уже не зона имени
        let date_pt = [
            lay.rows[0].row[0] + lay.rows[0].row[2] - 8.0,
            lay.rows[0].row[1] + 4.0,
        ];
        assert_eq!(name_at(&lay, date_pt), None, "бейдж/дата — не имя");
        // Мимо строк — None
        assert_eq!(row_at(&lay, [640.0, 5.0]), None);
    }

    #[test]
    fn layout_empty_state_card() {
        let mut state = CanvasManagerState::default();
        state.set_entries(vec![]);
        let lay = manager_layout([1280.0, 800.0], &state, "", &widths());
        assert!(!lay.has_entries);
        let empty = lay.empty.expect("пустое состояние №23a");
        assert!(empty.create[2] > 0.0, "CTA «Создать канвас»");
        assert!(empty.open_disk[2] > 0.0, "вторичное «Открыть с диска…»");
        // Кнопки внутри панели, не пересекаются
        assert!(empty.create[0] + empty.create[2] <= empty.open_disk[0]);
        assert!(
            empty.open_disk[0] + empty.open_disk[2] <= lay.panel[0] + lay.panel[2],
            "в пределах панели"
        );
        // Строка хранилища остаётся видимой и в пустом состоянии (№51a)
        assert!(lay.storage_label[2] > 0.0);
        // Кнопки строки выбранного — свёрнуты (выбирать нечего)
        assert_eq!(lay.duplicate, [0.0; 4]);
    }

    #[test]
    fn layout_storage_row_by_mode() {
        let mut state = CanvasManagerState::default();
        state.set_entries(vec![entry("a.canvas", 1)]);
        // OPFS + FS Access (Chromium) — кнопка «Переехать на диск…»
        state.storage = StorageRowMode::Browser { fs_available: true };
        let lay = manager_layout([1280.0, 800.0], &state, "", &widths());
        assert!(lay.storage_move[2] > 0.0);
        // OPFS без FS Access (Firefox/Safari) — только текст (№51a честность)
        state.storage = StorageRowMode::Browser {
            fs_available: false,
        };
        let lay = manager_layout([1280.0, 800.0], &state, "", &widths());
        assert_eq!(lay.storage_move, [0.0; 4], "кнопки нет — диск не обещаем");
        assert!(lay.storage_label[2] > 0.0, "подпись хранилища остаётся");
        // Папка — кнопки нет (уже на диске)
        state.storage = StorageRowMode::Folder;
        let lay = manager_layout([1280.0, 800.0], &state, "", &widths());
        assert_eq!(lay.storage_move, [0.0; 4]);
        assert_eq!(storage_move_available(StorageRowMode::Folder), false);
        assert_eq!(
            storage_move_available(StorageRowMode::Browser {
                fs_available: false
            }),
            false
        );
        assert_eq!(
            storage_move_available(StorageRowMode::Browser { fs_available: true }),
            true
        );
    }

    #[test]
    fn layout_clamps_to_minimal_viewport() {
        let mut state = CanvasManagerState::default();
        state.set_entries((0..12).map(|i| entry(&format!("c{i}.canvas"), i)).collect());
        state.sort = SortMode::Name;
        let lay = manager_layout([800.0, 560.0], &state, "", &widths());
        assert!(lay.panel[0] >= 0.0 && lay.panel[1] >= 0.0);
        assert!(lay.panel[0] + lay.panel[2] <= 800.0);
        assert!(
            lay.panel[1] + lay.panel[3] <= 560.0,
            "кламп высоты к вьюпорту (строки скроллятся)"
        );
        assert!(lay.rows.len() < VISIBLE_ROWS, "окно ужалось под высоту");
        // Строка хранилища не вылезает за панель
        assert!(lay.storage_label[1] + STORAGE_ROW_H <= lay.panel[1] + lay.panel[3] + 0.5);
    }

    // --- FR-108 (C5): тонкий нативный слой ----------------------------------------------

    /// Натив: «Из шаблона…» и ряд кнопок строки скрыты (тонкий слой),
    /// «Открыть…» (слот import) встаёт сразу за «Создать», строка
    /// хранилища — режим Files без кнопки переезда.
    #[test]
    fn layout_native_thin_layer() {
        let mut state = CanvasManagerState::default();
        // ts убывают (f0 — свежайший): дефолтная сортировка ModifiedDesc
        // держит f0 первой строкой — активный файл подсвечен.
        state.set_entries(
            (0..3)
                .map(|i| entry(&format!("f{i}.canvas"), (3 - i) as u64))
                .collect(),
        );
        state.native = true;
        state.storage = StorageRowMode::Files;
        let lay = manager_layout([1280.0, 800.0], &state, "f0.canvas", &widths());
        assert_eq!(lay.template, [0.0; 4], "натив: шаблона нет (тонкий слой)");
        assert!(
            lay.import[0] >= lay.create[0] + lay.create[2],
            "«Открыть…» сразу за «Создать»"
        );
        assert!(lay.import[2] > 0.0, "слот import занят «Открыть…»");
        // Ряд кнопок выбранной строки скрыт (ренейм/удаление/дубликат/экспорт)
        assert_eq!(lay.duplicate, [0.0; 4]);
        assert_eq!(lay.rename, [0.0; 4]);
        assert_eq!(lay.delete, [0.0; 4]);
        assert_eq!(lay.export, [0.0; 4]);
        // Файлы на диске — кнопки «Переехать…» нет (файлами владеет ОС)
        assert_eq!(lay.storage_move, [0.0; 4]);
        assert!(!storage_move_available(StorageRowMode::Files));
        assert!(
            lay.storage_label[2] > 0.0,
            "подпись хранилища (Files) видна"
        );
        // Панель в вьюпорту, список/поиск работают как на web (реюз C3)
        assert!(lay.has_entries);
        assert!(lay.panel[1] + lay.panel[3] <= 800.0);
        assert_eq!(lay.rows.len(), 3);
        assert!(lay.rows[0].is_active, "активный файл подсвечен");
        // Нативная панель короче web-режима (нет ряда кнопок строки)
        let mut web_state = CanvasManagerState::default();
        web_state.set_entries(state.entries.clone());
        web_state.sort = state.sort;
        let web_lay = manager_layout([1280.0, 800.0], &web_state, "f0.canvas", &widths());
        assert!(
            lay.panel[3] < web_lay.panel[3],
            "натив без ряда кнопок строки ниже"
        );
    }

    /// Натив: пустое состояние (№23a) живо — CTA «Создать» и «Открыть файл
    /// с диска…» ведут в нативные диалоги (№34a), карточка та же.
    #[test]
    fn layout_native_empty_state() {
        let mut state = CanvasManagerState::default();
        state.set_entries(vec![]);
        state.native = true;
        let lay = manager_layout([1280.0, 800.0], &state, "", &widths());
        let empty = lay.empty.expect("пустое состояние живо и на нативе");
        assert!(empty.create[2] > 0.0, "CTA «Создать канвас»");
        assert!(empty.open_disk[2] > 0.0, "вторичное «Открыть файл…»");
        assert!(!lay.has_entries);
    }

    // --- дата (format_ts) ---------------------------------------------------------------

    #[test]
    fn format_ts_known_dates() {
        assert_eq!(format_ts(0), "01.01.1970", "эпоха");
        // 2026-10-09 00:00:00 UTC = 1791513600 s
        assert_eq!(format_ts(1_791_513_600_000), "09.10.2026");
        // 2000-02-29 (високосный) = 951782400 s
        assert_eq!(format_ts(951_782_400_000), "29.02.2000");
        // 1970-12-31 = 31535 s? нет: 31.12.1970 = 365 дней = 31536000 s
        assert_eq!(
            format_ts(31_536_000_000),
            "01.01.1971",
            "365 дней — новый год"
        );
        assert_eq!(format_ts(31_449_600_000), "31.12.1970", "364 дня — канун");
        // 01.03.1970 (после февраля) = 59 дней = 5097600 s
        assert_eq!(format_ts(5_097_600_000), "01.03.1970");
    }
}
