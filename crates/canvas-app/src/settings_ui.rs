//! FR-026: панель настроек — группы настроек и выпадающие меню — чистая
//! модель (образец [`crate::hints_ui`]/`template_ui`): модель групп
//! ([`SETTINGS_GROUPS`]), род строки ([`row_kind`]), перечень значений
//! многозначной настройки ([`dropdown_options`]) с чистым применением
//! выбора ([`apply_dropdown_value`]), геометрия панели и меню с клампом
//! к окну ([`panel_layout`]/[`dropdown_layout`]) и hit-тесты.
//!
//! Рендер и ввод — приложение (`main.rs`): панель собирается по кадру из
//! квадов + screen-текстов, клик по тумблеру переключает значение, клик
//! по dropdown-строке открывает меню (выбор пункта применяет значение
//! через [`apply_dropdown_value`] + побочные эффекты рендера на стороне
//! `App`). Схема `config.toml` не меняется — это реорганизация UI.

use canvas_core::{Corner, GridDensity, GridStyle, Settings, PORT_ZONE_PRESETS};

use crate::ui::{
    panel_rect, point_in_rect, PANEL_HEADER_HEIGHT, PANEL_HINT_HEIGHT, PANEL_PADDING,
    PANEL_ROW_HEIGHT, PANEL_WIDTH,
};

/// Высота заголовка группы (капс-текст меньшим кеглем).
pub const GROUP_TITLE_HEIGHT: f32 = 22.0;
/// Межсекционный отступ между группами.
pub const GROUP_GAP: f32 = 8.0;
/// Высота пункта выпадающего меню.
pub const DROPDOWN_ROW_H: f32 = 26.0;
/// Внутренние поля выпадающего меню.
pub const DROPDOWN_MARGIN: f32 = 6.0;

// Панель настроек: строки (порядок = прежний плоский список, источник
// инварианта полноты групп). Тема вынесена в отдельную кнопку-переключатель
// рядом с кнопкой настроек — вне панели, как раньше.
//
// FR-025 (построчные точки выхода) добавит `LinePorts` в группу
// «Связи и порты» — расширение в двух местах (SETTINGS_ROWS + группа).

/// Строка-переключатель панели настроек.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsRow {
    /// Угол летающей кнопки (цикл по 4 углам).
    ButtonCorner,
    /// Сетка канваса вкл/выкл.
    Grid,
    /// Вид сетки: линии или точки.
    GridStyle,
    /// Плотность сетки (цикл по 3 вариантам).
    GridDensity,
    /// Связи огибают посторонние ноды.
    EdgesAvoid,
    /// Зона захвата портов для drag связи (CR-003): цикл по пресетам.
    PortZone,
    /// Режим фокуса связей (T23, brainstorm-focus) вкл/выкл.
    FocusMode,
    /// HUD (F3) включён при старте.
    HudOnStart,
}

/// Плоский список всех строк панели (порядок = порядок до FR-026).
/// Группировка — в [`SETTINGS_GROUPS`]; инвариант полноты (юнит-тест):
/// union строк групп == этот список без дублей.
pub const SETTINGS_ROWS: [SettingsRow; 8] = [
    SettingsRow::ButtonCorner,
    SettingsRow::Grid,
    SettingsRow::GridStyle,
    SettingsRow::GridDensity,
    SettingsRow::EdgesAvoid,
    SettingsRow::PortZone,
    SettingsRow::FocusMode,
    SettingsRow::HudOnStart,
];

/// Группа настроек панели (FR-026): заголовок-капс + строки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsGroup {
    /// Заголовок секции («КАНВАС», «СВЯЗИ И ПОРТЫ», «ПРИЛОЖЕНИЕ»).
    pub title: &'static str,
    /// Строки группы в порядке отображения.
    pub rows: &'static [SettingsRow],
}

/// Группы настроек (FR-026): логические секции вместо плоского списка.
/// Распределение v1: «Канвас» — сетка; «Связи и порты» — связи/порты/фокус
/// (сюда же войдёт `LinePorts` FR-025); «Приложение» — кнопка и HUD.
pub const SETTINGS_GROUPS: [SettingsGroup; 3] = [
    SettingsGroup {
        title: "Канвас",
        rows: &[
            SettingsRow::Grid,
            SettingsRow::GridStyle,
            SettingsRow::GridDensity,
        ],
    },
    SettingsGroup {
        title: "Связи и порты",
        rows: &[
            SettingsRow::EdgesAvoid,
            SettingsRow::PortZone,
            SettingsRow::FocusMode,
        ],
    },
    SettingsGroup {
        title: "Приложение",
        rows: &[SettingsRow::ButtonCorner, SettingsRow::HudOnStart],
    },
];

impl SettingsRow {
    /// Подпись строки с текущим значением (закрытый dropdown показывает
    /// текущее значение на самой строке — HIG «Pop-Up Buttons»).
    pub fn label(self, settings: &Settings) -> String {
        let on_off = |v: bool| if v { "вкл" } else { "выкл" };
        match self {
            SettingsRow::ButtonCorner => {
                format!("Угол кнопки: {}", settings.button_corner.label())
            }
            SettingsRow::Grid => format!("Сетка: {}", on_off(settings.grid_visible)),
            SettingsRow::GridStyle => format!("Вид сетки: {}", settings.grid_style.label()),
            SettingsRow::GridDensity => {
                format!("Плотность сетки: {}", settings.grid_density.label())
            }
            SettingsRow::EdgesAvoid => {
                format!("Связи огибают ноды: {}", on_off(settings.edges_avoid_nodes))
            }
            SettingsRow::PortZone => {
                format!("Зона портов: {} px", settings.port_zone_px as i32)
            }
            SettingsRow::FocusMode => {
                format!("Фокус на связях: {}", on_off(settings.focus_mode))
            }
            SettingsRow::HudOnStart => {
                format!("HUD при запуске: {}", on_off(settings.hud_on_start))
            }
        }
    }
}

/// Род строки панели: тумблер (клик переключает) или dropdown (клик
/// открывает меню значений). Инвариант (юнит-тест): `Toggle` — ровно для
/// `bool`-полей `Settings`, `Dropdown` — для остальных.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// Булева настройка: клик — переключить (цикл из двух значений —
    /// это тумблер, меню избыточно).
    Toggle,
    /// Многозначная настройка: клик — открыть выпадающее меню.
    Dropdown,
}

/// Род строки панели.
pub fn row_kind(row: SettingsRow) -> RowKind {
    match row {
        SettingsRow::ButtonCorner
        | SettingsRow::GridStyle
        | SettingsRow::GridDensity
        | SettingsRow::PortZone => RowKind::Dropdown,
        SettingsRow::Grid
        | SettingsRow::EdgesAvoid
        | SettingsRow::FocusMode
        | SettingsRow::HudOnStart => RowKind::Toggle,
    }
}

/// Полный перечень значений многозначной настройки с отметкой текущего
/// (`(значение, текущее)`). Порядок пунктов = порядку цикла `.next()` —
/// выбор пункта `i` эквивалентен соответствующему числу нажатий цикла
/// (инвариант, юнит-тест). Для тумблеров — пустой список (меню избыточно).
///
/// Зона портов: текущим считается пресет, от которого цикл
/// `next_port_zone` шагнул бы дальше (последний пресет ≤ значения) —
/// ручная правка `config.toml` между пресетами всё равно получает отметку.
pub fn dropdown_options(row: SettingsRow, settings: &Settings) -> Vec<(String, bool)> {
    match row {
        SettingsRow::ButtonCorner => [
            Corner::TopLeft,
            Corner::TopRight,
            Corner::BottomRight,
            Corner::BottomLeft,
        ]
        .into_iter()
        .map(|corner| (corner.label().to_owned(), settings.button_corner == corner))
        .collect(),
        SettingsRow::GridStyle => [GridStyle::Lines, GridStyle::Dots]
            .into_iter()
            .map(|style| (style.label().to_owned(), settings.grid_style == style))
            .collect(),
        SettingsRow::GridDensity => [GridDensity::Dense, GridDensity::Medium, GridDensity::Sparse]
            .into_iter()
            .map(|density| (density.label().to_owned(), settings.grid_density == density))
            .collect(),
        SettingsRow::PortZone => {
            let current = PORT_ZONE_PRESETS
                .iter()
                .rposition(|preset| *preset <= settings.port_zone_px)
                .unwrap_or(0);
            PORT_ZONE_PRESETS
                .iter()
                .enumerate()
                .map(|(i, preset)| (format!("{} px", *preset as i32), i == current))
                .collect()
        }
        SettingsRow::Grid
        | SettingsRow::EdgesAvoid
        | SettingsRow::FocusMode
        | SettingsRow::HudOnStart => Vec::new(),
    }
}

/// Применить выбор пункта dropdown к настройкам (чистая функция — только
/// значение; побочные эффекты рендера — на стороне `App`, сохранение
/// конфига — общий хвост вызывающего). Индекс вне диапазона — без изменений.
pub fn apply_dropdown_value(settings: &mut Settings, row: SettingsRow, index: usize) {
    match row {
        SettingsRow::ButtonCorner => {
            let corners = [
                Corner::TopLeft,
                Corner::TopRight,
                Corner::BottomRight,
                Corner::BottomLeft,
            ];
            if let Some(corner) = corners.get(index) {
                settings.button_corner = *corner;
            }
        }
        SettingsRow::GridStyle => {
            let styles = [GridStyle::Lines, GridStyle::Dots];
            if let Some(style) = styles.get(index) {
                settings.grid_style = *style;
            }
        }
        SettingsRow::GridDensity => {
            let densities = [GridDensity::Dense, GridDensity::Medium, GridDensity::Sparse];
            if let Some(density) = densities.get(index) {
                settings.grid_density = *density;
            }
        }
        SettingsRow::PortZone => {
            if let Some(preset) = PORT_ZONE_PRESETS.get(index) {
                settings.port_zone_px = *preset;
            }
        }
        SettingsRow::Grid
        | SettingsRow::EdgesAvoid
        | SettingsRow::FocusMode
        | SettingsRow::HudOnStart => {}
    }
}

/// Состояние выпадающего меню настроек (FR-026): какая строка открыта и
/// клавиатурное выделение пункта. Пункты вычисляются на кадр из
/// [`dropdown_options`] — состояние не может устареть. Хранится в `App`,
/// сбрасывается при закрытии панели/меню.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DropdownState {
    /// Открытое меню строки (`None` — все закрыты).
    pub open_row: Option<SettingsRow>,
    /// Клавиатурное выделение (индекс в списке пунктов).
    pub selected: usize,
}

impl DropdownState {
    /// Открыто ли какое-нибудь меню.
    pub fn is_open(&self) -> bool {
        self.open_row.is_some()
    }

    /// Закрыть меню и сбросить выделение.
    pub fn reset(&mut self) {
        self.open_row = None;
        self.selected = 0;
    }

    /// Открыть меню строки; выделение — на текущем значении (первая
    /// отметка), чтобы Enter сразу применял видимое состояние.
    pub fn open(&mut self, row: SettingsRow, settings: &Settings) {
        self.open_row = Some(row);
        self.selected = dropdown_options(row, settings)
            .iter()
            .position(|(_, current)| *current)
            .unwrap_or(0);
    }

    /// Сдвиг выделения с закольцовыванием; true — было изменение.
    pub fn move_selection(&mut self, delta: i32, count: usize) -> bool {
        if count == 0 {
            return false;
        }
        let len = count as i32;
        let next = (self.selected as i32 + delta).rem_euclid(len);
        if next == self.selected as i32 {
            return false;
        }
        self.selected = next as usize;
        true
    }
}

/// Элемент панели настроек: заголовок группы (не кликабелен) или строка
/// настройки (кликабельна).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelEntry {
    /// Заголовок секции.
    Header(&'static str),
    /// Строка настройки.
    Row(SettingsRow),
}

/// Геометрия панели настроек (FR-026): rect целиком + rect'ы элементов
/// (заголовки групп и строки) в порядке отображения. Высота панели =
/// прежняя формула + заголовки секций + межсекционные отступы.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelLayout {
    /// Rect панели `[x, y, w, h]` (логические px).
    pub rect: [f32; 4],
    /// Элементы по вертикали: заголовки групп и строки настроек.
    pub entries: Vec<(PanelEntry, [f32; 4])>,
}

impl PanelLayout {
    /// Rect строки настройки (`None` для заголовков и отсутствующих).
    pub fn row_rect(&self, row: SettingsRow) -> Option<[f32; 4]> {
        self.entries.iter().find_map(|(entry, rect)| match entry {
            PanelEntry::Row(r) if *r == row => Some(*rect),
            _ => None,
        })
    }
}

/// Высота панели настроек: паддинги + заголовок + группы (заголовок
/// секции + строки, межсекционные отступы) + подсказка.
pub fn panel_height() -> f32 {
    let entries: f32 = SETTINGS_GROUPS
        .iter()
        .map(|group| GROUP_TITLE_HEIGHT + group.rows.len() as f32 * PANEL_ROW_HEIGHT)
        .sum();
    let gaps = SETTINGS_GROUPS.len().saturating_sub(1) as f32 * GROUP_GAP;
    PANEL_PADDING * 2.0 + PANEL_HEADER_HEIGHT + entries + gaps + PANEL_HINT_HEIGHT
}

/// Геометрия панели с позициями заголовков групп и строк (позиция/ширина
/// панели — та же, что у `ui::panel_rect`; источник один).
pub fn panel_layout(corner: Corner, viewport: [f32; 2]) -> PanelLayout {
    let rect = panel_rect(corner, viewport);
    let mut entries = Vec::new();
    let mut y = rect[1] + PANEL_PADDING + PANEL_HEADER_HEIGHT;
    for (gi, group) in SETTINGS_GROUPS.iter().enumerate() {
        if gi > 0 {
            y += GROUP_GAP;
        }
        entries.push((
            PanelEntry::Header(group.title),
            [rect[0], y, PANEL_WIDTH, GROUP_TITLE_HEIGHT],
        ));
        y += GROUP_TITLE_HEIGHT;
        for &row in group.rows {
            entries.push((
                PanelEntry::Row(row),
                [rect[0], y, PANEL_WIDTH, PANEL_ROW_HEIGHT],
            ));
            y += PANEL_ROW_HEIGHT;
        }
    }
    PanelLayout { rect, entries }
}

/// Hit-test строки панели: какая строка настройки под точкой. Заголовки
/// групп, шапка, подсказка и паддинги — `None` (не кликабельны).
pub fn row_at(layout: &PanelLayout, point: [f32; 2]) -> Option<SettingsRow> {
    for (entry, rect) in &layout.entries {
        if let PanelEntry::Row(row) = entry {
            if point_in_rect(*rect, point) {
                return Some(*row);
            }
        }
    }
    None
}

/// Геометрия выпадающего меню `[x, y, w, h]` с клампом к окну: ниже
/// строки-якоря; не влезает снизу — выше строки. Ширина = ширине панели
/// (единая сетка, самый длинный пункт «верхний левый»/«редкая» влезает с
/// запасом); кламп по горизонтали и вертикали — паттерн
/// `hints_ui::popup_layout` (FR-021). `count == 0` — пустой rect.
pub fn dropdown_layout(anchor: [f32; 4], viewport: [f32; 2], count: usize) -> [f32; 4] {
    if count == 0 {
        return [0.0; 4];
    }
    let height = count as f32 * DROPDOWN_ROW_H + DROPDOWN_MARGIN * 2.0;
    let width = PANEL_WIDTH.min((viewport[0] - DROPDOWN_MARGIN * 2.0).max(0.0));
    let mut x = anchor[0];
    if x + width > viewport[0] - DROPDOWN_MARGIN {
        x = viewport[0] - DROPDOWN_MARGIN - width;
    }
    let x = x.max(DROPDOWN_MARGIN);
    // Ниже строки; не влезает снизу — выше (не перекрывая саму строку)
    let below = anchor[1] + anchor[3] + 2.0;
    let y = if below + height <= viewport[1] - DROPDOWN_MARGIN {
        below
    } else {
        (anchor[1] - height - 2.0).max(DROPDOWN_MARGIN)
    };
    [x, y, width, height]
}

/// Hit-test пункта выпадающего меню: индекс пункта под точкой или `None`
/// (вне меню, в полях-паддингах, за последним пунктом).
pub fn dropdown_item_at(menu: [f32; 4], count: usize, point: [f32; 2]) -> Option<usize> {
    if !point_in_rect(menu, point) {
        return None;
    }
    let rel = point[1] - menu[1] - DROPDOWN_MARGIN;
    if rel < 0.0 {
        return None;
    }
    let index = (rel / DROPDOWN_ROW_H) as usize;
    (index < count).then_some(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Инвариант полноты: union строк групп == SETTINGS_ROWS — ни одна
    /// настройка не потеряна и не задублирована; порядок групп стабилен.
    #[test]
    fn groups_cover_all_rows() {
        let mut seen = Vec::new();
        let mut titles = Vec::new();
        for group in &SETTINGS_GROUPS {
            assert!(!group.title.is_empty(), "заголовок группы пуст");
            assert!(!group.rows.is_empty(), "пустая группа: {}", group.title);
            titles.push(group.title);
            for &row in group.rows {
                assert!(!seen.contains(&row), "дубль строки в группах: {row:?}");
                seen.push(row);
            }
        }
        assert_eq!(titles, vec!["Канвас", "Связи и порты", "Приложение"]);
        for row in SETTINGS_ROWS {
            assert!(seen.contains(&row), "строка вне групп: {row:?}");
        }
        assert_eq!(seen.len(), SETTINGS_ROWS.len());
    }

    /// Инвариант булевых: `Toggle` — ровно для `bool`-полей `Settings`;
    /// многозначные — `Dropdown`.
    #[test]
    fn row_kind_partitions_bool_and_value_rows() {
        let defaults = Settings::default();
        for row in SETTINGS_ROWS {
            match row {
                SettingsRow::Grid => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.grid_visible;
                }
                SettingsRow::EdgesAvoid => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.edges_avoid_nodes;
                }
                SettingsRow::FocusMode => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.focus_mode;
                }
                SettingsRow::HudOnStart => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.hud_on_start;
                }
                SettingsRow::ButtonCorner
                | SettingsRow::GridStyle
                | SettingsRow::GridDensity
                | SettingsRow::PortZone => {
                    assert_eq!(row_kind(row), RowKind::Dropdown);
                }
            }
        }
    }

    /// Инвариант значений: у каждого dropdown полный перечень значений
    /// (4 угла / 2 вида / 3 плотности / 5 пресетов), текущее отмечено ровно
    /// один раз; у тумблеров список пуст.
    #[test]
    fn dropdown_options_counts_and_current() {
        let settings = Settings {
            button_corner: Corner::BottomLeft,
            grid_style: GridStyle::Dots,
            grid_density: GridDensity::Sparse,
            port_zone_px: 20.0,
            ..Settings::default()
        };

        let corners = dropdown_options(SettingsRow::ButtonCorner, &settings);
        assert_eq!(corners.len(), 4);
        assert_eq!(corners.iter().filter(|(_, cur)| *cur).count(), 1);
        assert_eq!(
            corners
                .iter()
                .find(|(_, cur)| *cur)
                .map(|(label, _)| label.as_str()),
            Some("нижний левый")
        );

        let styles = dropdown_options(SettingsRow::GridStyle, &settings);
        assert_eq!(styles.len(), 2);
        assert_eq!(
            styles
                .iter()
                .find(|(_, cur)| *cur)
                .map(|(label, _)| label.as_str()),
            Some("точки")
        );

        let densities = dropdown_options(SettingsRow::GridDensity, &settings);
        assert_eq!(densities.len(), 3);
        assert_eq!(
            densities
                .iter()
                .find(|(_, cur)| *cur)
                .map(|(label, _)| label.as_str()),
            Some("редкая")
        );

        let zones = dropdown_options(SettingsRow::PortZone, &settings);
        assert_eq!(zones.len(), PORT_ZONE_PRESETS.len());
        assert_eq!(
            zones
                .iter()
                .find(|(_, cur)| *cur)
                .map(|(label, _)| label.as_str()),
            Some("20 px")
        );
        // Значение между пресетами — отметка на последнем меньшем (цикл)
        let between = Settings {
            port_zone_px: 22.0,
            ..Settings::default()
        };
        let zones = dropdown_options(SettingsRow::PortZone, &between);
        assert_eq!(
            zones
                .iter()
                .find(|(_, cur)| *cur)
                .map(|(label, _)| label.as_str()),
            Some("20 px")
        );

        for row in [
            SettingsRow::Grid,
            SettingsRow::EdgesAvoid,
            SettingsRow::FocusMode,
            SettingsRow::HudOnStart,
        ] {
            assert!(
                dropdown_options(row, &settings).is_empty(),
                "{row:?}: тумблер без меню"
            );
        }
    }

    /// Инвариант эквивалентности: выбор пункта `i` == k нажатий цикла
    /// (значение и подпись совпадают с `.next()`/`next_port_zone`).
    #[test]
    fn dropdown_choice_matches_value_cycle() {
        /// (строка, цикл значений, число значений) — сценарий эквивалентности.
        type CycleCase = (SettingsRow, fn(&mut Settings, usize), usize);
        let cases: [CycleCase; 4] = [
            (
                SettingsRow::ButtonCorner,
                |s, k| {
                    for _ in 0..k {
                        s.button_corner = s.button_corner.next();
                    }
                },
                4,
            ),
            (
                SettingsRow::GridStyle,
                |s, k| {
                    for _ in 0..k {
                        s.grid_style = s.grid_style.next();
                    }
                },
                2,
            ),
            (
                SettingsRow::GridDensity,
                |s, k| {
                    for _ in 0..k {
                        s.grid_density = s.grid_density.next();
                    }
                },
                3,
            ),
            (
                SettingsRow::PortZone,
                |s, k| {
                    for _ in 0..k {
                        s.port_zone_px = canvas_core::next_port_zone(s.port_zone_px);
                    }
                },
                PORT_ZONE_PRESETS.len(),
            ),
        ];
        for (row, cycle, count) in cases {
            for start in 0..count {
                // Текущее значение = start нажатий цикла от дефолта; его
                // индекс в перечне — по отметке dropdown_options
                let mut base = Settings::default();
                cycle(&mut base, start);
                let current_index = dropdown_options(row, &base)
                    .iter()
                    .position(|(_, current)| *current)
                    .expect("текущее значение отмечено");
                for chosen in 0..count {
                    let mut expected = base.clone();
                    cycle(&mut expected, (chosen + count - current_index) % count);
                    let mut applied = base.clone();
                    apply_dropdown_value(&mut applied, row, chosen);
                    assert_eq!(
                        applied, expected,
                        "{row:?}: current={current_index}, chosen={chosen}"
                    );
                }
            }
        }
    }

    /// Выбор вне диапазона — без изменений; выбор текущего — значение то же.
    #[test]
    fn apply_dropdown_value_out_of_range_is_noop() {
        let mut settings = Settings::default();
        let before = settings.clone();
        apply_dropdown_value(&mut settings, SettingsRow::ButtonCorner, 4);
        apply_dropdown_value(&mut settings, SettingsRow::GridStyle, 99);
        apply_dropdown_value(&mut settings, SettingsRow::GridDensity, usize::MAX);
        apply_dropdown_value(&mut settings, SettingsRow::PortZone, 5);
        assert_eq!(settings, before);
    }

    /// `dropdown_layout`: кламп к окну на всех углах панели и узком окне;
    /// у нижнего края — выше строки (не перекрывая её); 0 пунктов — пусто.
    #[test]
    fn dropdown_layout_clamps_to_window() {
        assert_eq!(
            dropdown_layout([0.0, 0.0, 100.0, 28.0], [800.0, 600.0], 0),
            [0.0; 4]
        );
        let viewport = [1600.0, 900.0];
        for corner in [
            Corner::TopLeft,
            Corner::TopRight,
            Corner::BottomLeft,
            Corner::BottomRight,
        ] {
            let layout = panel_layout(corner, viewport);
            for row in SETTINGS_ROWS {
                let Some(anchor) = layout.row_rect(row) else {
                    continue;
                };
                let menu = dropdown_layout(anchor, viewport, 5);
                assert!(menu[0] >= DROPDOWN_MARGIN - 0.01, "{corner:?} {row:?}");
                assert!(
                    menu[0] + menu[2] <= viewport[0] - DROPDOWN_MARGIN + 0.01,
                    "{corner:?} {row:?}"
                );
                assert!(menu[1] >= DROPDOWN_MARGIN - 0.01, "{corner:?} {row:?}");
                assert!(
                    menu[1] + menu[3] <= viewport[1] - DROPDOWN_MARGIN + 0.01,
                    "{corner:?} {row:?}"
                );
                assert_eq!(menu[2], PANEL_WIDTH);
            }
        }
        // Узкое окно (320×240): меню клампится внутрь, ширина <= окна
        let narrow = [320.0, 240.0];
        let menu = dropdown_layout([8.0, 100.0, 300.0, 28.0], narrow, 5);
        assert!(
            menu[0] >= DROPDOWN_MARGIN - 0.01
                && menu[0] + menu[2] <= 320.0 - DROPDOWN_MARGIN + 0.01
        );
        assert!(
            menu[1] >= DROPDOWN_MARGIN - 0.01
                && menu[1] + menu[3] <= 240.0 - DROPDOWN_MARGIN + 0.01
        );
        assert!(menu[2] <= 320.0);
        // У нижнего края — меню выше строки, её верх виден
        let menu = dropdown_layout([50.0, 200.0, 300.0, 28.0], narrow, 5);
        assert!(menu[1] + menu[3] <= 200.0, "меню выше якорной строки");
        // Меню ниже строки, когда влезает: не перекрывает якорь
        let menu = dropdown_layout([50.0, 20.0, 300.0, 28.0], [800.0, 600.0], 3);
        assert!(menu[1] >= 20.0 + 28.0, "меню ниже якорной строки");
    }

    /// Hit-тесты: строки кликабельны по своим rect'ам, заголовки групп /
    /// шапка / подсказка / паддинги — None; `row_rect` согласован с `row_at`.
    #[test]
    fn row_hit_tests() {
        let viewport = [1600.0, 900.0];
        let layout = panel_layout(Corner::TopRight, viewport);
        // Первая строка = первая строка первой группы (Grid), последняя =
        // последняя строка последней группы (HudOnStart)
        let first = SETTINGS_GROUPS[0].rows[0];
        let last = SETTINGS_GROUPS[2].rows[1];
        let first_rect = layout.row_rect(first).expect("первая строка");
        let last_rect = layout.row_rect(last).expect("последняя строка");
        assert_eq!(
            row_at(&layout, [first_rect[0] + 20.0, first_rect[1] + 3.0]),
            Some(first)
        );
        assert_eq!(
            row_at(&layout, [last_rect[0] + 20.0, last_rect[1] + 3.0]),
            Some(last)
        );
        // Заголовок панели, заголовок группы, подсказка, мимо панели — None
        let header_y = layout.rect[1] + PANEL_PADDING + 3.0;
        assert_eq!(row_at(&layout, [layout.rect[0] + 20.0, header_y]), None);
        let group_rect = layout.entries[0].1;
        assert_eq!(
            row_at(&layout, [group_rect[0] + 20.0, group_rect[1] + 3.0]),
            None
        );
        let hint_y = layout.rect[1] + layout.rect[3] - PANEL_PADDING - PANEL_HINT_HEIGHT + 3.0;
        assert_eq!(row_at(&layout, [layout.rect[0] + 20.0, hint_y]), None);
        assert_eq!(
            row_at(&layout, [layout.rect[0] - 5.0, last_rect[1] + 3.0]),
            None
        );
        assert_eq!(
            row_at(&layout, [layout.rect[0] + 20.0, layout.rect[1] - 5.0]),
            None
        );
        // Согласованность высот: сумма элементов + подсказка/паддинги = панель
        let bottom = layout.entries.last().expect("элементы есть").1[1] + PANEL_ROW_HEIGHT;
        assert!(
            (bottom + PANEL_HINT_HEIGHT + PANEL_PADDING - (layout.rect[1] + layout.rect[3])).abs()
                < 0.01
        );
    }

    /// Порядок строк в layout = порядок групп (не прежний плоский список).
    #[test]
    fn panel_layout_rows_follow_groups() {
        let layout = panel_layout(Corner::TopLeft, [1600.0, 900.0]);
        let rows: Vec<SettingsRow> = layout
            .entries
            .iter()
            .filter_map(|(entry, _)| match entry {
                PanelEntry::Row(row) => Some(*row),
                PanelEntry::Header(_) => None,
            })
            .collect();
        let expected: Vec<SettingsRow> = SETTINGS_GROUPS
            .iter()
            .flat_map(|group| group.rows.iter().copied())
            .collect();
        assert_eq!(rows, expected);
        let headers: Vec<&str> = layout
            .entries
            .iter()
            .filter_map(|(entry, _)| match entry {
                PanelEntry::Header(title) => Some(*title),
                PanelEntry::Row(_) => None,
            })
            .collect();
        assert_eq!(headers.len(), SETTINGS_GROUPS.len());
    }

    /// Hit-тест пунктов меню: пункты 0/средний/последний, паддинги и мимо
    /// меню — None.
    #[test]
    fn dropdown_item_hit_tests() {
        let count = 5;
        let menu = dropdown_layout([100.0, 100.0, 300.0, 28.0], [1600.0, 900.0], count);
        let height = count as f32 * DROPDOWN_ROW_H + DROPDOWN_MARGIN * 2.0;
        assert!((menu[3] - height).abs() < 0.01);
        // Пункт 0 и последний
        assert_eq!(
            dropdown_item_at(
                menu,
                count,
                [menu[0] + 30.0, menu[1] + DROPDOWN_MARGIN + 3.0]
            ),
            Some(0)
        );
        assert_eq!(
            dropdown_item_at(
                menu,
                count,
                [
                    menu[0] + 30.0,
                    menu[1] + DROPDOWN_MARGIN + 4.0 * DROPDOWN_ROW_H + 3.0
                ]
            ),
            Some(4)
        );
        // Паддинг сверху и мимо меню — None
        assert_eq!(
            dropdown_item_at(menu, count, [menu[0] + 30.0, menu[1] + 1.0]),
            None
        );
        assert_eq!(
            dropdown_item_at(menu, count, [menu[0] + 30.0, menu[1] + menu[3] + 5.0]),
            None
        );
        assert_eq!(
            dropdown_item_at(menu, count, [menu[0] + 500.0, menu[1] + 20.0]),
            None
        );
    }

    /// Модель состояния меню: открытие ставит выделение на текущее значение,
    /// сдвиг закольцован, reset закрывает.
    #[test]
    fn dropdown_state_model() {
        let settings = Settings {
            button_corner: Corner::BottomRight,
            ..Settings::default()
        };
        let mut state = DropdownState::default();
        assert!(!state.is_open());
        state.open(SettingsRow::ButtonCorner, &settings);
        assert!(state.is_open());
        assert_eq!(state.open_row, Some(SettingsRow::ButtonCorner));
        // BottomRight — третий пункт цикла (TopLeft, TopRight, BottomRight, …)
        assert_eq!(state.selected, 2);
        assert!(state.move_selection(1, 4));
        assert_eq!(state.selected, 3);
        assert!(state.move_selection(1, 4), "закольцовывание вперёд");
        assert_eq!(state.selected, 0);
        assert!(state.move_selection(-1, 4), "закольцовывание назад");
        assert_eq!(state.selected, 3);
        state.reset();
        assert!(!state.is_open());
        assert_eq!(state.selected, 0);
        assert!(!state.move_selection(1, 0), "пустой список — сдвига нет");
    }
}
