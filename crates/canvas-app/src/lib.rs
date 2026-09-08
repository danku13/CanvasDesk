//! canvas-app library — общая логика приложения.
//!
//! Переэкспортирует типы и функции, нужные интеграционным тестам без окна
//! winit, и содержит модуль `ui` — чистые функции интерфейса (геометрия
//! оверлеев, hit-тесты, палитра контекстного меню, детектор двойного
//! клика), общий для бинаря (`main.rs`) и тестов. Раньше эти функции
//! дублировались в `main.rs` копипастой — теперь источник один.

pub use canvas_core::{
    edge_at, nearest_side, port_at, port_point, Canvas, Corner, Edge, Node, NodeKind, Settings,
    Side, SpatialIndex,
};
pub use canvas_render::camera::Vec2;
pub use canvas_render::cards::{preset_color, CardInstance, HEADER_HEIGHT};
pub use canvas_render::edit::{
    edge_edit_area, map_key, session_area, EditTarget, EditingSession, KeyCommand, Marker,
    EDGE_EDIT_HEIGHT, EDGE_EDIT_WIDTH,
};
pub use canvas_render::text::{
    body_area, OverlayText, ScreenText, BODY_FONT_SIZE, BODY_LINE_HEIGHT, BODY_PADDING,
    BODY_TOP_GAP,
};
pub use canvas_render::{
    Camera, Color, FrameMeter, FrameOverlay, FrameStats, SceneView, Selection,
};
pub use winit::keyboard::{Key, ModifiersState, NamedKey};

/// Чистая UI-логика приложения: геометрия оверлеев (контекстное меню,
/// панель настроек), hit-тесты, генератор id заметок, детектор двойного
/// клика. Не зависит от окна и GPU — используется бинарём и тестами.
pub mod ui {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    /// Ширина контекстного меню в world-px (T7).
    pub const MENU_WIDTH: f32 = 170.0;
    /// Высота пункта меню в world-px.
    pub const MENU_ITEM_HEIGHT: f32 = 26.0;
    /// Внутренний отступ меню в world-px.
    pub const MENU_PADDING: f32 = 6.0;
    /// Сдвиг подписи пункта: слева место под образец цвета.
    pub const MENU_LABEL_X: f32 = 26.0;
    /// Пункты палитры (T7): пресеты "1".."6" + None — сброс цвета.
    pub const MENU_ITEMS: [Option<&str>; 7] = [
        Some("1"),
        Some("2"),
        Some("3"),
        Some("4"),
        Some("5"),
        Some("6"),
        None,
    ];
    /// Фон меню — тёмный, почти непрозрачный.
    pub const MENU_FILL: [f32; 4] = [0.11, 0.11, 0.13, 0.97];

    /// Минимальные размеры ноды (ручной resize, T7).
    pub const MIN_NODE_WIDTH: f32 = 160.0;
    pub const MIN_NODE_HEIGHT: f32 = 64.0;
    /// Потолок автороста ширины заметки под контент (T7).
    pub const MAX_NOTE_WIDTH: f32 = 600.0;
    /// Зона захвата в правом нижнем углу ноды для ручного resize (world-px, T7).
    pub const RESIZE_HANDLE: f32 = 16.0;

    // --- Drag-drop из Explorer (T9, план docs/plans/T9-drag-drop.md) ---

    /// Ширина карточки дропа в world-px (как seed-карточки файлов).
    pub const DROP_CARD_W: f32 = 320.0;
    /// Высота карточки дропа в world-px.
    pub const DROP_CARD_H: f32 = 220.0;
    /// Зазор сетки дропа (шаг = карточка + зазор, критерий T9).
    pub const DROP_GRID_GAP: f32 = 24.0;
    /// Колонок в ряду сетки дропа (перенос строки после 5 карточек).
    pub const DROP_GRID_COLS: usize = 5;
    /// Призраков на превью зоны дропа не больше (дёшево рисовать, план §5).
    pub const DROP_PREVIEW_MAX: usize = 50;

    /// Сторона летающей кнопки настроек (логические px).
    pub const SETTINGS_BUTTON: f32 = 36.0;
    /// Отступ кнопки и панели настроек от краёв окна (логические px).
    pub const SETTINGS_MARGIN: f32 = 12.0;
    /// Зазор между кнопкой и панелью настроек.
    pub const SETTINGS_GAP: f32 = 8.0;
    /// Ширина панели настроек.
    pub const PANEL_WIDTH: f32 = 300.0;
    /// Высота строки настройки.
    pub const PANEL_ROW_HEIGHT: f32 = 28.0;
    /// Высота заголовка панели.
    pub const PANEL_HEADER_HEIGHT: f32 = 30.0;
    /// Высота строки-подсказки внизу панели.
    pub const PANEL_HINT_HEIGHT: f32 = 24.0;
    /// Внутренний отступ панели.
    pub const PANEL_PADDING: f32 = 10.0;

    /// Строки панели настроек (порядок = порядок отображения).
    pub const SETTINGS_ROWS: [SettingsRow; 3] = [
        SettingsRow::ButtonCorner,
        SettingsRow::Grid,
        SettingsRow::HudOnStart,
    ];

    /// Строка-переключатель панели настроек.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SettingsRow {
        /// Угол летающей кнопки (цикл по 4 углам).
        ButtonCorner,
        /// Сетка канваса вкл/выкл.
        Grid,
        /// HUD (F3) включён при старте.
        HudOnStart,
    }

    impl SettingsRow {
        /// Подпись строки с текущим значением.
        pub fn label(self, settings: &Settings) -> String {
            let on_off = |v: bool| if v { "вкл" } else { "выкл" };
            match self {
                SettingsRow::ButtonCorner => {
                    format!("Угол кнопки: {}", settings.button_corner.label())
                }
                SettingsRow::Grid => format!("Сетка: {}", on_off(settings.grid_visible)),
                SettingsRow::HudOnStart => {
                    format!("HUD при запуске: {}", on_off(settings.hud_on_start))
                }
            }
        }
    }

    /// Точка в rect [x, y, w, h]? (логические px, границы включительны)
    pub fn point_in_rect(rect: [f32; 4], point: Vec2) -> bool {
        point[0] >= rect[0]
            && point[0] <= rect[0] + rect[2]
            && point[1] >= rect[1]
            && point[1] <= rect[1] + rect[3]
    }

    /// Rect летающей кнопки настроек в логических px от угла окна.
    pub fn button_rect(corner: Corner, viewport: Vec2) -> [f32; 4] {
        let x = match corner {
            Corner::TopLeft | Corner::BottomLeft => SETTINGS_MARGIN,
            _ => viewport[0] - SETTINGS_MARGIN - SETTINGS_BUTTON,
        };
        let y = match corner {
            Corner::TopLeft | Corner::TopRight => SETTINGS_MARGIN,
            _ => viewport[1] - SETTINGS_MARGIN - SETTINGS_BUTTON,
        };
        [x, y, SETTINGS_BUTTON, SETTINGS_BUTTON]
    }

    /// Высота панели настроек: паддинги + заголовок + строки + подсказка.
    pub fn panel_height() -> f32 {
        PANEL_PADDING * 2.0
            + PANEL_HEADER_HEIGHT
            + SETTINGS_ROWS.len() as f32 * PANEL_ROW_HEIGHT
            + PANEL_HINT_HEIGHT
    }

    /// Rect панели настроек: прижата к кнопке (с зазором), в том же углу.
    pub fn panel_rect(corner: Corner, viewport: Vec2) -> [f32; 4] {
        let height = panel_height();
        let x = match corner {
            Corner::TopLeft | Corner::BottomLeft => SETTINGS_MARGIN,
            _ => viewport[0] - SETTINGS_MARGIN - PANEL_WIDTH,
        };
        let y = match corner {
            Corner::TopLeft | Corner::TopRight => SETTINGS_MARGIN + SETTINGS_BUTTON + SETTINGS_GAP,
            _ => viewport[1] - SETTINGS_MARGIN - SETTINGS_BUTTON - SETTINGS_GAP - height,
        };
        [x, y, PANEL_WIDTH, height]
    }

    /// Hit-test строки панели: индекс в SETTINGS_ROWS или None
    /// (заголовок/подсказка/паддинги не кликабельны).
    pub fn panel_row_at(panel: [f32; 4], point: Vec2) -> Option<usize> {
        let rows_top = panel[1] + PANEL_PADDING + PANEL_HEADER_HEIGHT;
        if point[0] < panel[0]
            || point[0] > panel[0] + panel[2]
            || point[1] < rows_top
            || point[1] > rows_top + SETTINGS_ROWS.len() as f32 * PANEL_ROW_HEIGHT
        {
            return None;
        }
        let i = ((point[1] - rows_top) / PANEL_ROW_HEIGHT) as usize;
        (i < SETTINGS_ROWS.len()).then_some(i)
    }

    /// Точка в зоне resize (правый нижний угол ноды)? Чистая функция для тестов.
    pub fn in_resize_corner(node: &Node, point: Vec2) -> bool {
        let right = node.x + node.width;
        let bottom = node.y + node.height;
        point[0] >= right - RESIZE_HANDLE
            && point[0] <= right
            && point[1] >= bottom - RESIZE_HANDLE
            && point[1] <= bottom
    }

    /// Контекстное меню ноды (T7): палитра цветов в world-точке клика ПКМ.
    pub struct ContextMenu {
        pub node: usize,
        pub origin: Vec2,
    }

    /// Активный drag резиновой линии новой связи (T8): от порта ноды к курсору.
    pub struct EdgeDrag {
        pub from_node: String,
        pub from_side: Side,
    }

    /// Максимальный интервал между кликами двойного клика (winit его не даёт, T7).
    const DOUBLE_CLICK_INTERVAL: Duration = Duration::from_millis(500);
    /// Максимальный сдвиг курсора между кликами двойного клика (логические px).
    const DOUBLE_CLICK_DIST: f64 = 5.0;

    /// Детектор двойного клика (T7): интервал и сдвиг между нажатиями ЛКМ.
    pub struct DoubleClick {
        last: Option<(Instant, Vec2)>,
    }

    impl Default for DoubleClick {
        fn default() -> Self {
            Self::new()
        }
    }

    impl DoubleClick {
        pub fn new() -> Self {
            Self { last: None }
        }

        /// Зарегистрировать нажатие; true — это второй клик пары.
        pub fn register(&mut self, at: Instant, pos: Vec2) -> bool {
            let double = self.last.is_some_and(|(time, prev)| {
                at.duration_since(time) <= DOUBLE_CLICK_INTERVAL
                    && (pos[0] as f64 - prev[0] as f64).abs() <= DOUBLE_CLICK_DIST
                    && (pos[1] as f64 - prev[1] as f64).abs() <= DOUBLE_CLICK_DIST
            });
            self.last = Some((at, pos));
            double
        }
    }

    /// Первый свободный id вида `{prefix}-N` (T9): N от 1, занятые в канвасе
    /// пропускаются. Обобщение генератора id заметок на `file-N`/`note-N`
    /// (вызовы с "note" — заметки, с "file" — ноды дропа).
    pub fn next_free_id(canvas: &Canvas, prefix: &str) -> String {
        let mut n = 1u32;
        while canvas
            .nodes
            .iter()
            .any(|node| node.id == format!("{prefix}-{n}"))
        {
            n += 1;
        }
        format!("{prefix}-{n}")
    }

    // --- Парсинг CF_HDROP и раскладка дропа (T9) ---

    /// Разобрать содержимое CF_HDROP ЦЕЛИКОМ: DROPFILES-заголовок (20 байт:
    /// pFiles-офсет LE, pt, fNC, fWide) + UTF-16 null-terminated строки +
    /// DOUBLE null в конце. Чистая функция от байтов — тестируется синтетикой
    /// на любой ОС (снимает shell байты с IDataObject как есть).
    ///
    /// Толерантность: нечётный хвостовой байт игнорируется; без терминатора
    /// отдаём что накопили. ANSI-вариант (fWide=0) не поддерживаем — Explorer
    /// всегда кладёт UTF-16.
    pub fn parse_hdrop_bytes(bytes: &[u8]) -> Vec<PathBuf> {
        // DROPFILES-заголовок = 20 байт; меньше — битый формат
        if bytes.len() < 20 {
            return Vec::new();
        }
        let p_files = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        if p_files > bytes.len() {
            return Vec::new();
        }
        let f_wide = i32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) != 0;
        if !f_wide {
            return Vec::new();
        }
        let mut paths = Vec::new();
        let mut segment: Vec<u16> = Vec::new();
        // Пары u16; хвостовый нечётный байт (remainder) игнорируем
        for chunk in bytes[p_files..].chunks_exact(2) {
            let unit = u16::from_le_bytes([chunk[0], chunk[1]]);
            if unit == 0 {
                if segment.is_empty() {
                    // Пустой сегмент = терминатор списка (DOUBLE null)
                    return paths;
                }
                paths.push(PathBuf::from(String::from_utf16_lossy(&segment)));
                segment.clear();
            } else {
                segment.push(unit);
            }
        }
        // Терминатора не было — отдаём что накопили
        if !segment.is_empty() {
            paths.push(PathBuf::from(String::from_utf16_lossy(&segment)));
        }
        paths
    }

    /// Разворачивает пути дропа: каталог — его дети (глубина 1, подпапки-дети
    /// НЕ разворачиваются — сами станут нодами); симлинки пропускаются (не
    /// следуем — защита от циклов); обычные файлы — как есть. Детей каталога
    /// сортируем по имени (предсказуемость сетки), скрытые/системные — мимо.
    pub fn expand_drop_paths(paths: &[PathBuf]) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for path in paths {
            // symlink_metadata не следует по ссылке: симлинки видны сразу
            let Ok(meta) = std::fs::symlink_metadata(path) else {
                continue; // путь исчез/недоступен — пропускаем
            };
            let file_type = meta.file_type();
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                let Ok(entries) = std::fs::read_dir(path) else {
                    continue; // нечитаемый каталог — пропускаем целиком
                };
                let mut children: Vec<(std::ffi::OsString, PathBuf)> = Vec::new();
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    if !child_is_hidden(&name, &entry) {
                        children.push((name, entry.path()));
                    }
                }
                // Сортировка по имени: порядок сетки не зависит от выдачи FS
                children.sort_by(|a, b| a.0.cmp(&b.0));
                out.extend(children.into_iter().map(|(_, child)| child));
            } else {
                out.push(path.clone()); // обычный файл — порядок входа сохраняем
            }
        }
        out
    }

    /// Скрытый/системный ребёнок каталога? Unix — имя с ведущей точкой;
    /// Windows — FILE_ATTRIBUTE_HIDDEN (0x2) | FILE_ATTRIBUTE_SYSTEM (0x4).
    #[cfg(not(windows))]
    fn child_is_hidden(name: &std::ffi::OsStr, _entry: &std::fs::DirEntry) -> bool {
        name.to_string_lossy().starts_with('.')
    }

    /// Скрытый/системный ребёнок каталога (Windows): читаем атрибуты
    /// метаданных записи каталога, битые — считаем скрытыми (не показываем).
    #[cfg(windows)]
    fn child_is_hidden(_name: &std::ffi::OsStr, entry: &std::fs::DirEntry) -> bool {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
        entry
            .metadata()
            .map(|meta| {
                meta.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
            })
            .unwrap_or(true)
    }

    /// Позиции сетки дропа от origin (row-major): `count` карточек,
    /// перенос строки после DROP_GRID_COLS колонок, шаг = карточка + зазор.
    pub fn drop_grid(origin: Vec2, count: usize) -> Vec<Vec2> {
        (0..count)
            .map(|i| {
                let col = i % DROP_GRID_COLS;
                let row = i / DROP_GRID_COLS;
                [
                    origin[0] + col as f32 * (DROP_CARD_W + DROP_GRID_GAP),
                    origin[1] + row as f32 * (DROP_CARD_H + DROP_GRID_GAP),
                ]
            })
            .collect()
    }

    /// Вид текста из CF_UNICODETEXT: URL или обычный текст.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DropTextKind {
        /// Начинается (после trim) с `http://`/`https://` — без учёта регистра.
        Url,
        /// Всё остальное.
        Plain,
    }

    /// Классификация текста дропа: префикс `http://`/`https://` (case-
    /// insensitive, после trim) — Url, иначе Plain.
    pub fn drop_text_kind(text: &str) -> DropTextKind {
        let trimmed = text.trim();
        let head: String = trimmed
            .chars()
            .take("https://".len())
            .flat_map(char::to_lowercase)
            .collect();
        if head.starts_with("http://") || head == "https://" {
            DropTextKind::Url
        } else {
            DropTextKind::Plain
        }
    }

    /// Тип вставки из дропа (T9): файловая нода или заметка с текстом.
    #[derive(Debug, Clone, PartialEq)]
    pub enum DropInsertKind {
        /// Файловая нода (путь как дал Explorer, абсолютный).
        File(PathBuf),
        /// Текстовая нода: URL или произвольный текст (критерий T9 — URL
        /// становится заметкой с текстом ссылки; Plain-текст — бонус).
        Note(String),
    }

    /// Одна вставка дропа: id, тип и позиция в world-координатах.
    #[derive(Debug, Clone, PartialEq)]
    pub struct DropInsert {
        pub id: String,
        pub kind: DropInsertKind,
        pub pos: Vec2,
    }

    /// Спланировать вставку дропа (T9): сырые данные из shell -> готовые
    /// ноды с id и позициями. CF_HDROP разбирается и разворачивается
    /// (каталоги — глубина 1), позиции даёт сетка от origin; текст — единая
    /// заметка в origin. Канвас не мутируется — вставку делает приложение.
    pub fn plan_drop(
        canvas: &Canvas,
        data: &canvas_shell::dragdrop::DragData,
        origin: Vec2,
    ) -> Vec<DropInsert> {
        let occupied: HashSet<&str> = canvas.nodes.iter().map(|node| node.id.as_str()).collect();
        let mut issued: HashSet<String> = HashSet::new();
        match data {
            canvas_shell::dragdrop::DragData::HdropBytes(bytes) => {
                let paths = expand_drop_paths(&parse_hdrop_bytes(bytes));
                let positions = drop_grid(origin, paths.len());
                paths
                    .into_iter()
                    .zip(positions)
                    .map(|(path, pos)| DropInsert {
                        id: next_free_plan_id(&occupied, &mut issued, "file"),
                        kind: DropInsertKind::File(path),
                        pos,
                    })
                    .collect()
            }
            canvas_shell::dragdrop::DragData::Text(text) => {
                // И Url, и Plain -> единая заметка с полным текстом
                let kind = match drop_text_kind(text) {
                    DropTextKind::Url | DropTextKind::Plain => DropInsertKind::Note(text.clone()),
                };
                vec![DropInsert {
                    id: next_free_plan_id(&occupied, &mut issued, "note"),
                    kind,
                    pos: origin,
                }]
            }
            canvas_shell::dragdrop::DragData::None => Vec::new(),
        }
    }

    /// Следующий свободный `{prefix}-N` внутри плана: избегаем и занятых в
    /// канвасе, и уже выданных в этом плане (несколько файлов подряд).
    fn next_free_plan_id(
        occupied: &HashSet<&str>,
        issued: &mut HashSet<String>,
        prefix: &str,
    ) -> String {
        let mut n = 1u32;
        loop {
            let id = format!("{prefix}-{n}");
            if !occupied.contains(id.as_str()) && !issued.contains(&id) {
                issued.insert(id.clone());
                return id;
            }
            n += 1;
        }
    }

    /// Rect пункта меню в world-координатах: [x, y, w, h].
    pub fn menu_item_rect(origin: Vec2, i: usize) -> [f32; 4] {
        [
            origin[0] + MENU_PADDING,
            origin[1] + MENU_PADDING + i as f32 * MENU_ITEM_HEIGHT,
            MENU_WIDTH - MENU_PADDING * 2.0,
            MENU_ITEM_HEIGHT,
        ]
    }

    /// Полный rect меню: [x, y, w, h].
    pub fn menu_rect(origin: Vec2) -> [f32; 4] {
        [
            origin[0],
            origin[1],
            MENU_WIDTH,
            MENU_PADDING * 2.0 + MENU_ITEMS.len() as f32 * MENU_ITEM_HEIGHT,
        ]
    }

    /// Hit-test пункта меню по world-точке (T7).
    pub fn menu_item_at(origin: Vec2, point: Vec2) -> Option<usize> {
        let [x, y, w, h] = menu_rect(origin);
        if point[0] < x
            || point[0] > x + w
            || point[1] < y + MENU_PADDING
            || point[1] > y + h - MENU_PADDING
        {
            return None;
        }
        let i = ((point[1] - y - MENU_PADDING) / MENU_ITEM_HEIGHT) as usize;
        (i < MENU_ITEMS.len()).then_some(i)
    }

    /// Подпись пункта меню.
    pub fn menu_label(item: Option<&str>) -> String {
        match item {
            Some(preset) => format!("Цвет {preset}"),
            None => "Без цвета".to_owned(),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Детектор двойного клика (T7): пара кликов в интервале — double,
        /// далёкие по времени или позиции — нет.
        #[test]
        fn double_click_detection() {
            let t0 = Instant::now();
            let mut detector = DoubleClick::new();
            assert!(!detector.register(t0, [100.0, 100.0]), "первый клик");
            assert!(detector.register(t0 + Duration::from_millis(200), [102.0, 99.0]));
            // Третий клик сразу после — тоже double (считаем парами)
            assert!(detector.register(t0 + Duration::from_millis(300), [100.0, 100.0]));

            let mut detector = DoubleClick::new();
            assert!(!detector.register(t0, [0.0, 0.0]));
            // Интервал превышен
            assert!(!detector.register(t0 + Duration::from_millis(600), [0.0, 0.0]));

            let mut detector = DoubleClick::new();
            assert!(!detector.register(t0, [0.0, 0.0]));
            // Курсор ушёл дальше порога
            assert!(!detector.register(t0 + Duration::from_millis(100), [50.0, 0.0]));
        }

        /// Генератор id (T7/T9): первый свободный по префиксу.
        #[test]
        fn note_id_first_free() {
            let canvas = Canvas::default();
            assert_eq!(next_free_id(&canvas, "note"), "note-1");
            let mut canvas = Canvas::default();
            canvas.nodes.push(Node::text("note-1", "", 0.0, 0.0));
            assert_eq!(next_free_id(&canvas, "note"), "note-2");
            canvas.nodes.push(Node::text("note-2", "", 0.0, 0.0));
            assert_eq!(next_free_id(&canvas, "note"), "note-3");
        }

        // --- Drag-drop (T9) ---

        /// Синтетический CF_HDROP: DROPFILES-заголовок {pFiles=20, pt=0,
        /// fNC=0, fWide=1} + UTF-16 строки с \0 каждая + финальный \0.
        fn hdrop(paths: &[&str]) -> Vec<u8> {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&20u32.to_le_bytes()); // pFiles — офсет строк
            bytes.extend_from_slice(&0i32.to_le_bytes()); // pt.x
            bytes.extend_from_slice(&0i32.to_le_bytes()); // pt.y
            bytes.extend_from_slice(&0i32.to_le_bytes()); // fNC
            bytes.extend_from_slice(&1i32.to_le_bytes()); // fWide
            for path in paths {
                for unit in path.encode_utf16() {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
                bytes.extend_from_slice(&0u16.to_le_bytes());
            }
            bytes.extend_from_slice(&0u16.to_le_bytes()); // DOUBLE null — конец списка
            bytes
        }

        /// CF_HDROP: два пути, пустой список.
        #[test]
        fn parse_hdrop_paths_and_empty() {
            let paths = parse_hdrop_bytes(&hdrop(&["C:/a.txt", "C:/dir/b.jpg"]));
            assert_eq!(
                paths,
                vec![PathBuf::from("C:/a.txt"), PathBuf::from("C:/dir/b.jpg")]
            );
            assert!(parse_hdrop_bytes(&hdrop(&[])).is_empty());
        }

        /// Битый CF_HDROP: короткий буфер, pFiles больше длины, ANSI-вариант.
        #[test]
        fn parse_hdrop_malformed() {
            // len < 20 — вообще не DROPFILES
            assert!(parse_hdrop_bytes(&[0u8; 19]).is_empty());
            // pFiles указывает за конец буфера
            let mut bytes = hdrop(&["C:/a.txt"]);
            bytes[0..4].copy_from_slice(&100u32.to_le_bytes());
            assert!(parse_hdrop_bytes(&bytes).is_empty());
            // fWide = 0 — ANSI не поддерживаем (Explorer всегда UTF-16)
            let mut bytes = hdrop(&["C:/a.txt"]);
            bytes[16..20].copy_from_slice(&0i32.to_le_bytes());
            assert!(parse_hdrop_bytes(&bytes).is_empty());
        }

        /// Нет терминатора списка — отдаём что накопили; нечётный хвост — мимо.
        #[test]
        fn parse_hdrop_without_terminator() {
            // header + "a.txt\0" + "b.txt" (без \0 и без DOUBLE null) + мусорный байт
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&20u32.to_le_bytes());
            bytes.extend_from_slice(&[0u8; 12]);
            bytes.extend_from_slice(&1i32.to_le_bytes());
            for path in ["a.txt", "b.txt"] {
                for unit in path.encode_utf16() {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
                if path == "a.txt" {
                    bytes.extend_from_slice(&0u16.to_le_bytes());
                }
            }
            bytes.push(0xff); // нечётный хвостовой байт — игнорируется
            let paths = parse_hdrop_bytes(&bytes);
            assert_eq!(paths, vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")]);
        }

        /// Уникальный temp-каталог теста (без новых зависимостей).
        fn temp_dir(name: &str) -> PathBuf {
            let dir =
                std::env::temp_dir().join(format!("canvasdesk-t9-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("tempdir");
            dir
        }

        /// Каталог -> дети глубины 1 (подпапка сама нода), скрытый skip,
        /// сортировка по имени.
        #[test]
        fn expand_drop_folder_depth_one() {
            let dir = temp_dir("folder");
            std::fs::write(dir.join("b.txt"), b"1").unwrap();
            std::fs::write(dir.join("a.txt"), b"2").unwrap();
            std::fs::write(dir.join(".hidden"), b"3").unwrap();
            // На Windows «скрытый» — атрибут файла, а не точка в имени:
            // выставляем attrib +h (встроенная команда), иначе фильтр
            // FILE_ATTRIBUTE_HIDDEN не имеет что фильтровать (урок CI fa1ca0b).
            // На Unix достаточно имени с ведущей точкой.
            #[cfg(windows)]
            {
                let status = std::process::Command::new("attrib")
                    .arg("+h")
                    .arg(dir.join(".hidden").as_os_str())
                    .status()
                    .expect("запуск attrib");
                assert!(status.success(), "attrib +h не смог скрыть файл");
            }
            std::fs::create_dir_all(dir.join("sub")).unwrap();
            let paths = expand_drop_paths(std::slice::from_ref(&dir));
            assert_eq!(
                paths,
                vec![dir.join("a.txt"), dir.join("b.txt"), dir.join("sub")]
            );
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// Файл -> сам; порядок входа сохраняется (не сортируется).
        #[test]
        fn expand_drop_files_keep_order() {
            let dir = temp_dir("files");
            let z = dir.join("z.txt");
            let a = dir.join("a.txt");
            std::fs::write(&z, b"1").unwrap();
            std::fs::write(&a, b"2").unwrap();
            let paths = expand_drop_paths(&[z.clone(), a.clone()]);
            assert_eq!(paths, vec![z, a]);
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// Симлинк пропускается (не следуем — защита от циклов). Unix-only.
        #[cfg(unix)]
        #[test]
        fn expand_drop_symlink_skipped() {
            let dir = temp_dir("symlink");
            std::fs::write(dir.join("real.txt"), b"x").unwrap();
            std::os::unix::fs::symlink(dir.join("real.txt"), dir.join("link.txt")).unwrap();
            let paths = expand_drop_paths(&[dir.join("link.txt")]);
            assert!(paths.is_empty(), "симлинк должен быть пропущен: {paths:?}");
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// Сетка дропа: 0/1/5/6/11 позиций, шаг карточка+зазор, перенос
        /// строки после 5 колонок (критерий T9 — предсказуемость).
        #[test]
        fn drop_grid_layout() {
            assert!(drop_grid([0.0, 0.0], 0).is_empty());
            assert_eq!(drop_grid([10.0, 20.0], 1), vec![[10.0, 20.0]]);
            let five = drop_grid([0.0, 0.0], 5);
            assert_eq!(five.len(), 5);
            assert_eq!(five[1][0], DROP_CARD_W + DROP_GRID_GAP);
            assert_eq!(five[4][0], 4.0 * (DROP_CARD_W + DROP_GRID_GAP));
            // 6-я — вторая строка, с origin.x
            let six = drop_grid([7.0, 11.0], 6);
            assert_eq!(six[5], [7.0, 11.0 + DROP_CARD_H + DROP_GRID_GAP]);
            // 11-я — третья строка
            let eleven = drop_grid([0.0, 0.0], 11);
            assert_eq!(eleven[10][1], 2.0 * (DROP_CARD_H + DROP_GRID_GAP));
        }

        /// next_free_id: первый свободный по префиксу, чужие префиксы не мешают.
        #[test]
        fn next_free_id_prefixes() {
            let canvas = Canvas::default();
            assert_eq!(next_free_id(&canvas, "note"), "note-1");
            let mut canvas = Canvas::default();
            canvas.nodes.push(Node::text("note-1", "", 0.0, 0.0));
            assert_eq!(next_free_id(&canvas, "note"), "note-2");
            canvas
                .nodes
                .push(Node::file("file-1", "a", 0.0, 0.0, 1.0, 1.0));
            canvas
                .nodes
                .push(Node::file("file-2", "b", 0.0, 0.0, 1.0, 1.0));
            assert_eq!(next_free_id(&canvas, "file"), "file-3");
            // Чистый префикс — с 1
            assert_eq!(next_free_id(&canvas, "link"), "link-1");
        }

        /// Классификация текста: http/https без учёта регистра — Url.
        #[test]
        fn drop_text_kind_detection() {
            assert_eq!(drop_text_kind("http://example.com"), DropTextKind::Url);
            assert_eq!(drop_text_kind("https://example.com"), DropTextKind::Url);
            assert_eq!(drop_text_kind("  HTTPs://Example.COM "), DropTextKind::Url);
            assert_eq!(drop_text_kind("привет"), DropTextKind::Plain);
            assert_eq!(drop_text_kind(""), DropTextKind::Plain);
        }

        /// План дропа файлов: свободные id с учётом занятых, сетка от origin,
        /// канвас не мутируется. Пути — реальные temp-файлы: expand их
        /// проверяет на ФС (несуществующие пропускаются).
        #[test]
        fn plan_drop_files_grid_and_ids() {
            let dir = temp_dir("plan");
            std::fs::write(dir.join("a.png"), b"1").unwrap();
            std::fs::write(dir.join("b.png"), b"2").unwrap();
            std::fs::write(dir.join("c.png"), b"3").unwrap();
            let a = dir.join("a.png").to_string_lossy().into_owned();
            let b = dir.join("b.png").to_string_lossy().into_owned();
            let c = dir.join("c.png").to_string_lossy().into_owned();
            let mut canvas = Canvas::default();
            canvas
                .nodes
                .push(Node::file("file-1", "old.png", 0.0, 0.0, 10.0, 10.0));
            let plan = plan_drop(
                &canvas,
                &canvas_shell::dragdrop::DragData::HdropBytes(hdrop(&[&a, &b, &c])),
                [100.0, 200.0],
            );
            assert_eq!(plan.len(), 3);
            // file-1 занят в канвасе — нумерация со свободных
            assert_eq!(plan[0].id, "file-2");
            assert_eq!(plan[1].id, "file-3");
            assert_eq!(plan[2].id, "file-4");
            assert_eq!(plan[0].pos, [100.0, 200.0]);
            assert_eq!(plan[1].pos, [100.0 + DROP_CARD_W + DROP_GRID_GAP, 200.0]);
            assert_eq!(plan[0].kind, DropInsertKind::File(PathBuf::from(&a)));
            // Канвас не мутирован
            assert_eq!(canvas.nodes.len(), 1);
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// План дропа текста: одна заметка в origin с полным текстом.
        #[test]
        fn plan_drop_text_single_note() {
            let plan = plan_drop(
                &Canvas::default(),
                &canvas_shell::dragdrop::DragData::Text("https://example.com".into()),
                [5.0, 6.0],
            );
            assert_eq!(plan.len(), 1);
            assert_eq!(plan[0].id, "note-1");
            assert_eq!(plan[0].pos, [5.0, 6.0]);
            assert_eq!(
                plan[0].kind,
                DropInsertKind::Note("https://example.com".into())
            );
        }

        /// Нет поддерживаемых форматов — план пуст.
        #[test]
        fn plan_drop_none_is_empty() {
            assert!(plan_drop(
                &Canvas::default(),
                &canvas_shell::dragdrop::DragData::None,
                [0.0, 0.0]
            )
            .is_empty());
        }

        /// Hit-test меню (T7): пункты палитры, края, промахи.
        #[test]
        fn menu_hit_test() {
            let origin = [100.0, 50.0];
            // Первый пункт (цвет "1")
            assert_eq!(
                menu_item_at(origin, [110.0, 50.0 + MENU_PADDING + 3.0]),
                Some(0)
            );
            // Последний пункт (сброс цвета)
            let last_y = 50.0 + MENU_PADDING + 6.0 * MENU_ITEM_HEIGHT + 3.0;
            assert_eq!(menu_item_at(origin, [110.0, last_y]), Some(6));
            // Правее меню, выше, ниже — промах
            assert_eq!(menu_item_at(origin, [100.0 + MENU_WIDTH + 1.0, 60.0]), None);
            assert_eq!(menu_item_at(origin, [110.0, 49.0]), None);
            assert_eq!(
                menu_item_at(origin, [110.0, 50.0 + menu_rect(origin)[3] + 1.0]),
                None
            );
            // Вертикальный паддинг между рамкой и первым пунктом — промах
            assert_eq!(menu_item_at(origin, [110.0, 51.0]), None);
        }

        /// Зона resize (T7): правый нижний угол ноды, границы включительны.
        #[test]
        fn resize_corner_hit_zone() {
            let mut node = Node::text("n", "t", 100.0, 100.0);
            node.width = 260.0;
            node.height = 120.0;
            // Угол (360, 220): внутри зоны
            assert!(in_resize_corner(&node, [355.0, 215.0]));
            assert!(in_resize_corner(&node, [360.0, 220.0]));
            // Снаружи: левее/выше зоны, за пределами ноды
            assert!(!in_resize_corner(&node, [340.0, 215.0]));
            assert!(!in_resize_corner(&node, [355.0, 200.0]));
            assert!(!in_resize_corner(&node, [365.0, 220.0]));
            // Противоположный угол — не resize
            assert!(!in_resize_corner(&node, [105.0, 105.0]));
        }

        /// Кнопка настроек: rect в каждом из 4 углов viewport (панель настроек).
        #[test]
        fn settings_button_corners() {
            let viewport = [1600.0, 900.0];
            let tl = button_rect(Corner::TopLeft, viewport);
            assert_eq!(
                tl,
                [
                    SETTINGS_MARGIN,
                    SETTINGS_MARGIN,
                    SETTINGS_BUTTON,
                    SETTINGS_BUTTON
                ]
            );
            let tr = button_rect(Corner::TopRight, viewport);
            assert_eq!(tr[0], 1600.0 - SETTINGS_MARGIN - SETTINGS_BUTTON);
            assert_eq!(tr[1], SETTINGS_MARGIN);
            let br = button_rect(Corner::BottomRight, viewport);
            assert_eq!(br[0], 1600.0 - SETTINGS_MARGIN - SETTINGS_BUTTON);
            assert_eq!(br[1], 900.0 - SETTINGS_MARGIN - SETTINGS_BUTTON);
            let bl = button_rect(Corner::BottomLeft, viewport);
            assert_eq!(bl[0], SETTINGS_MARGIN);
            assert_eq!(bl[1], 900.0 - SETTINGS_MARGIN - SETTINGS_BUTTON);
            // Точка кнопки попадает в hit-test, соседняя — нет
            assert!(point_in_rect(tr, [tr[0] + 2.0, tr[1] + 2.0]));
            assert!(!point_in_rect(tr, [tr[0] - 1.0, tr[1] + 2.0]));
        }

        /// Панель настроек: прижата к углу кнопки, целиком в viewport.
        #[test]
        fn settings_panel_placement() {
            let viewport = [1600.0, 900.0];
            for corner in [
                Corner::TopLeft,
                Corner::TopRight,
                Corner::BottomLeft,
                Corner::BottomRight,
            ] {
                let panel = panel_rect(corner, viewport);
                assert!(
                    panel[0] >= 0.0 && panel[0] + panel[2] <= viewport[0],
                    "{corner:?}"
                );
                assert!(
                    panel[1] >= 0.0 && panel[1] + panel[3] <= viewport[1],
                    "{corner:?}"
                );
                let button = button_rect(corner, viewport);
                // Панель по горизонтали на той же стороне, что и кнопка
                let same_side = (panel[0] - button[0]).abs() < 1.0
                    || ((panel[0] + panel[2]) - (button[0] + button[2])).abs() < 1.0;
                assert!(same_side, "{corner:?}: панель не под кнопкой");
            }
        }

        /// Hit-test строк панели: строки кликабельны, заголовок/подсказка/паддинги — нет.
        #[test]
        fn settings_panel_row_hit_test() {
            let viewport = [1600.0, 900.0];
            let panel = panel_rect(Corner::TopRight, viewport);
            let rows_top = panel[1] + PANEL_PADDING + PANEL_HEADER_HEIGHT;
            // Первая и последняя строки
            assert_eq!(
                panel_row_at(panel, [panel[0] + 20.0, rows_top + 3.0]),
                Some(0)
            );
            let last = SETTINGS_ROWS.len() - 1;
            let last_y = rows_top + last as f32 * PANEL_ROW_HEIGHT + 3.0;
            assert_eq!(panel_row_at(panel, [panel[0] + 20.0, last_y]), Some(last));
            // Заголовок и подсказка не кликабельны
            assert_eq!(
                panel_row_at(panel, [panel[0] + 20.0, panel[1] + PANEL_PADDING + 3.0]),
                None
            );
            let hint_y = rows_top + SETTINGS_ROWS.len() as f32 * PANEL_ROW_HEIGHT + 3.0;
            assert_eq!(panel_row_at(panel, [panel[0] + 20.0, hint_y]), None);
            // Мимо панели
            assert_eq!(panel_row_at(panel, [panel[0] - 5.0, last_y]), None);
            assert_eq!(panel_row_at(panel, [panel[0] + 20.0, panel[1] - 5.0]), None);
        }
    }
}
