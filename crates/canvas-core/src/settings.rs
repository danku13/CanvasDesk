//! Настройки приложения (пост-T7): `config.toml`, загружаемый при старте.
//!
//! Путь файла резолвит приложение (canvas-shell); здесь — только serde-схема
//! и чистые load/save. Расширение схемы — новые поля с `#[serde(default)]`:
//! старые конфиги не ломаются.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Угол экрана для летающей кнопки настроек.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    TopLeft,
    #[default]
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Corner {
    /// Следующий угол по циклу (переключение строкой в панели настроек).
    pub fn next(self) -> Self {
        match self {
            Corner::TopLeft => Corner::TopRight,
            Corner::TopRight => Corner::BottomRight,
            Corner::BottomRight => Corner::BottomLeft,
            Corner::BottomLeft => Corner::TopLeft,
        }
    }

    /// Подпись угла в панели настроек.
    pub fn label(self) -> &'static str {
        match self {
            Corner::TopLeft => "верхний левый",
            Corner::TopRight => "верхний правый",
            Corner::BottomLeft => "нижний левый",
            Corner::BottomRight => "нижний правый",
        }
    }
}

/// Тема интерфейса: тёмная (по умолчанию) или светлая (панель настроек).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Тёмная (базовая, SPEC T1: фон #1e1e22).
    #[default]
    Dark,
    /// Светлая.
    Light,
}

impl Theme {
    /// Переключение темы (строка панели настроек).
    pub fn next(self) -> Self {
        match self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        }
    }

    /// Подпись темы в панели настроек.
    pub fn label(self) -> &'static str {
        match self {
            Theme::Dark => "тёмная",
            Theme::Light => "светлая",
        }
    }
}

/// Язык интерфейса (FR-040): русский (дефолт — старые конфиги без поля
/// грузятся как `ru`) или английский. Смена — dropdown в разделе
/// «Внешний вид» модалки настроек (FR-039), применяется на лету;
/// названия языков в переключателе — на языке самого языка («русский»,
/// «English»), не через перевод этого enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    /// Русский (дефолт).
    #[default]
    Ru,
    /// Английский.
    En,
}

impl Language {
    /// Следующий язык по циклу (два значения — переключение замкнуто;
    /// используется тестами эквивалентности dropdown).
    pub fn next(self) -> Self {
        match self {
            Language::Ru => Language::En,
            Language::En => Language::Ru,
        }
    }

    /// Название языка в его собственной локали (конвенция Obsidian/VS Code:
    /// язык в переключателе подписывается собой, перевод не нужен).
    pub fn native_label(self) -> &'static str {
        match self {
            Language::Ru => "русский",
            Language::En => "English",
        }
    }
}

/// Вид сетки канваса: линии или точки (панель настроек).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GridStyle {
    /// Линии (классическая сетка).
    #[default]
    Lines,
    /// Точки в узлах мелкой сетки.
    Dots,
}

impl GridStyle {
    /// Переключение вида (строка панели настроек).
    pub fn next(self) -> Self {
        match self {
            GridStyle::Lines => GridStyle::Dots,
            GridStyle::Dots => GridStyle::Lines,
        }
    }

    /// Подпись вида в панели настроек.
    pub fn label(self) -> &'static str {
        match self {
            GridStyle::Lines => "линии",
            GridStyle::Dots => "точки",
        }
    }
}

/// FR-038 (п.4, advanced): точка привязки grid-снапа при перетаскивании.
/// В панели настроек НЕ показывается — только config.toml (владелец v2:
/// «advanced»; альтернативные точки привязки меняют только grid-притяжение,
/// направляющие соседей всегда по краям/центрам — п.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SnapAnchor {
    /// По краям bbox — семантика snap-движка как есть (ближайший край к
    /// линии, п.1-3).
    #[default]
    BoundingBox,
    /// Левый-верхний угол bbox — к пересечению линий сетки.
    Corner,
    /// Центр bbox — к пересечению линий сетки.
    Center,
}

/// Плотность сетки: шаг линий/точек относительно базового (20/100 world-px).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GridDensity {
    /// Частая: шаги 10/50 world-px.
    Dense,
    /// Средняя: шаги 20/100 world-px (базовая, SPEC T2).
    #[default]
    Medium,
    /// Редкая: шаги 40/200 world-px.
    Sparse,
}

impl GridDensity {
    /// Шаги (мелкий, крупный) в world-пикселях.
    pub fn steps(self) -> (f32, f32) {
        match self {
            GridDensity::Dense => (10.0, 50.0),
            GridDensity::Medium => (20.0, 100.0),
            GridDensity::Sparse => (40.0, 200.0),
        }
    }

    /// Переключение плотности по циклу (строка панели настроек).
    pub fn next(self) -> Self {
        match self {
            GridDensity::Dense => GridDensity::Medium,
            GridDensity::Medium => GridDensity::Sparse,
            GridDensity::Sparse => GridDensity::Dense,
        }
    }

    /// Подпись плотности в панели настроек.
    pub fn label(self) -> &'static str {
        match self {
            GridDensity::Dense => "частая",
            GridDensity::Medium => "средняя",
            GridDensity::Sparse => "редкая",
        }
    }
}

/// Настройки приложения. Дефолты — через `Default`, десериализация
/// подставляет их для отсутствующих полей (`#[serde(default)]` на struct).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Угол летающей кнопки настроек.
    pub button_corner: Corner,
    /// Рисовать сетку канваса.
    pub grid_visible: bool,
    /// Вид сетки: линии или точки.
    pub grid_style: GridStyle,
    /// Плотность сетки (шаг линий/точек).
    pub grid_density: GridDensity,
    /// Тема интерфейса.
    pub theme: Theme,
    /// Связи огибают посторонние ноды (роутинг полилинией).
    pub edges_avoid_nodes: bool,
    /// HUD (fps/p95, F3) включён сразу при старте.
    pub hud_on_start: bool,
    /// Режим фокуса связей (T23, brainstorm-focus): hover/выделение ноды
    /// подсвечивает её связи и соседей, остальное притемняется.
    /// Старые конфиги без поля грузятся как false (serde default).
    pub focus_mode: bool,
    /// Зона захвата портов ноды для старта drag связи (CR-003): в экранных
    /// px. Больше зона — не нужно целиться при протягивании связей. Дефолт —
    /// первый пресет; значения клампятся в `[PORT_ZONE_MIN, PORT_ZONE_MAX]`.
    pub port_zone_px: f32,
    /// Палитра шаблонов развёрнута постоянным левым доком (FR-025; false —
    /// свёрнута в вертикальную полосу категорий с hover-flyout — ревизия
    /// FR-025 2026-09-16, дефолт). Старые конфиги без поля грузятся
    /// свёрнутыми (serde default).
    pub template_palette_open: bool,
    /// FR-025 (построчные точки выхода): у каждой формульной строки Numi-
    /// листа — свой выходной порт на правом краю ноды; drag от него создаёт
    /// value-ребро со значением именно этой строки (`Edge::from_line`).
    /// Дефолт — выкл: поведение в точности прежнее (порты сторон, значение
    /// ноды целиком). Рендер/hit-тест/drag читают флаг на кадре.
    pub line_ports: bool,
    /// FR-028: онбординг-тур пройден до конца («Готово» на последнем шаге) —
    /// авто-показ при старте выключен навсегда; ручной вход из меню «?»
    /// остаётся. Старые конфиги без поля грузятся как false (serde default).
    pub onboarding_done: bool,
    /// FR-028: сколько раз подряд тур отложен «Пропустить»/Esc — авто-показ
    /// молчит после `ONBOARDING_MAX_DEFERS`. Ручной запуск из меню «?»
    /// счётчик не трогает. Значения клампятся в `[0, 3]` при загрузке
    /// (паттерн `port_zone_px` — ручные правки не роняют приложение).
    pub onboarding_defers: u8,
    /// FR-016 (индикаторы узких мест, CP5): оверлей анализа включён —
    /// рамка/бейджи по ρ и W из посчитанного потока. Дефолт — выкл;
    /// авто-включается один раз за запуск при первом появлении риска
    /// (Warn и выше) с тостом. Тогл: Ctrl+B, пункт меню канваса, панель
    /// настроек. Рендер читает флаг на кадре (как `line_ports`).
    pub bottleneck_overlay: bool,
    /// FR-040: язык интерфейса (`ru`/`en`). Старые конфиги без поля
    /// грузятся как `ru` (serde default на struct) — прежнее поведение.
    pub language: Language,
    /// FR-038 (п.5): мастер-тумблер магнитной раскладки. false — никакого
    /// снапа/предпросмотра/collision; остальные snap-настройки НЕ сбрасываются
    /// (обратно включил — всё вернулось). Snap-движок и рендер направляющих
    /// читают флаг на кадре. Старые конфиги без поля грузятся как true
    /// (serde default) — магнитная раскладка включена по умолчанию.
    pub snap_enabled: bool,
    /// FR-038 (п.1/19): притягивать к фоновой сетке на отпускании drag
    /// (во время драга — ghost-предпросмотр snapped-позиции, п.2).
    pub snap_to_grid: bool,
    /// FR-038 (п.6/19): направляющие соседей — края/центры/середины,
    /// majority-выбор оси, равные интервалы (п.6-13).
    pub snap_to_guides: bool,
    /// FR-038 (п.15/19): collision-avoidance — опциональный режим: движение
    /// при drag останавливается на границе зазора [`COLLISION_GAP`] вокруг
    /// чужих нод. Дефолт — выкл (прохождение сквозь — прежнее поведение).
    pub snap_collision: bool,
    /// FR-038 (п.6/19): допуск совпадения для направляющих и сетки,
    /// ЭКРАННЫЕ px (стартовый ориентир v2 «несколько пикселей»; внутри
    /// движка переводится в world делением на зум). Пресеты + кламп —
    /// по образцу `port_zone_px` (CR-003).
    pub snap_tolerance_px: f32,
    /// FR-038 (п.3): порог sub-сетки — при зуме СТРОГО выше линии полушага
    /// появляются (и снап мелкий). Дефолт 1.5 = «150%» из v2. Валидация пары
    /// с `snap_grid_coarse_zoom` — [`validated_grid_zoom_thresholds`].
    pub snap_grid_sub_zoom: f32,
    /// FR-038 (п.3): порог coarse-сетки — при зуме СТРОГО ниже линии
    /// укрупняются до major-шага. Дефолт 0.5 = «50%» из v2.
    pub snap_grid_coarse_zoom: f32,
    /// FR-038 (п.4, advanced): точка привязки grid-снапа — в основном UI
    /// не показывается (только config.toml).
    pub snap_anchor: SnapAnchor,
    /// FR-042 (F-13): агрегация связей — пучок рёбер одной пары рисуется
    /// одной линией с бейджем ×N, клик открывает main stage. false —
    /// прежний рендер связей без агрегации (регресс-щит, инвариант 5:
    /// количество/параметры инстансов идентичны прежнему поведению).
    /// Рендер/ввод читают флаг на кадре (как `line_ports`). Старые конфиги
    /// без поля грузятся как true (serde default) — агрегация включена.
    pub edge_aggregation: bool,
}

/// FR-028: лимит откладываний онбординга — после третьего «Пропустить» подряд
/// авто-показ замолкает (ручной вход из меню «?» живёт вечно).
pub const ONBOARDING_MAX_DEFERS: u8 = 3;

/// Пресеты зоны портов для строки панели настроек (CR-003): клик циклит.
pub const PORT_ZONE_PRESETS: [f32; 5] = [10.0, 14.0, 20.0, 28.0, 40.0];
/// Минимальная зона портов (экранные px, CR-003).
pub const PORT_ZONE_MIN: f32 = 10.0;
/// Максимальная зона портов (экранные px, CR-003).
pub const PORT_ZONE_MAX: f32 = 40.0;

/// Следующий пресет зоны портов по циклу (CR-003, панель настроек).
/// Значение вне пресетов округляется к ближайшему меньшему пресету.
pub fn next_port_zone(value: f32) -> f32 {
    let current = PORT_ZONE_PRESETS
        .iter()
        .rposition(|preset| *preset <= value)
        .unwrap_or(0);
    PORT_ZONE_PRESETS[(current + 1) % PORT_ZONE_PRESETS.len()]
}

/// Кламп значения зоны портов в допустимые границы (CR-003): защита от
/// ручной правки config.toml (0/отрицательные/гигантские значения).
pub fn clamp_port_zone(value: f32) -> f32 {
    value.clamp(PORT_ZONE_MIN, PORT_ZONE_MAX)
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            button_corner: Corner::TopRight,
            grid_visible: true,
            grid_style: GridStyle::Lines,
            grid_density: GridDensity::Medium,
            theme: Theme::Dark,
            edges_avoid_nodes: true,
            hud_on_start: false,
            focus_mode: false,
            port_zone_px: PORT_ZONE_PRESETS[0],
            // Ревизия FR-025 (2026-09-16): палитра примарно свёрнута.
            template_palette_open: false,
            // FR-025 (построчные точки выхода): по умолчанию выключено.
            line_ports: false,
            onboarding_done: false,
            onboarding_defers: 0,
            // FR-016: оверлей узких мест по умолчанию выключен.
            bottleneck_overlay: false,
            // FR-040: интерфейс по умолчанию — русский.
            language: Language::Ru,
            // FR-038: магнитная раскладка включена (п.5), тумблеры сетки/
            // направляющих — вкл (п.1/6), collision — выкл (п.15 опционален);
            // допуск и пороги — стартовые ориентиры v2.
            snap_enabled: true,
            snap_to_grid: true,
            snap_to_guides: true,
            snap_collision: false,
            snap_tolerance_px: SNAP_TOLERANCE_PRESETS[1],
            snap_grid_sub_zoom: SNAP_SUB_ZOOM_DEFAULT,
            snap_grid_coarse_zoom: SNAP_COARSE_ZOOM_DEFAULT,
            snap_anchor: SnapAnchor::BoundingBox,
            // FR-042 (F-13): агрегация связей включена по умолчанию.
            edge_aggregation: true,
        }
    }
}

/// FR-028: кламп счётчика откладываний онбординга в `[0, MAX]` — ручная
/// правка config.toml (99/200) не ломает таблицу решений показа тура.
pub fn clamp_onboarding_defers(value: u8) -> u8 {
    value.min(ONBOARDING_MAX_DEFERS)
}

// --- FR-038: магнитная раскладка (snap) — константы и пресеты -------------

/// FR-038 (п.15): минимальный зазор collision-avoidance вокруг чужих нод,
/// world px (0.0 — режим выключен). Стартовый ориентир v2 — настройкой не
/// является (п.15 «опционально» — только тумблер `snap_collision`).
pub const COLLISION_GAP: f32 = 8.0;

/// Дефолт порога sub-сетки (п.3 v2: «150%»); зеркало
/// `canvas_render::guides::DEFAULT_SUB_ZOOM` (зависимости core→render нет —
/// значение дублируется, паритет ловит тест ниже).
pub const SNAP_SUB_ZOOM_DEFAULT: f32 = 1.5;
/// Дефолт порога coarse-сетки (п.3 v2: «50%»); зеркало
/// `canvas_render::guides::DEFAULT_COARSE_ZOOM`.
pub const SNAP_COARSE_ZOOM_DEFAULT: f32 = 0.5;

/// Пресеты допуска направляющих (FR-038, панель настроек): клик циклит.
/// ЭКРАННЫЕ px — как `PORT_ZONE_PRESETS` (CR-003).
pub const SNAP_TOLERANCE_PRESETS: [f32; 5] = [4.0, 6.0, 8.0, 12.0, 16.0];
/// Минимальный допуск направляющих (экранные px, FR-038).
pub const SNAP_TOLERANCE_MIN: f32 = 2.0;
/// Максимальный допуск направляющих (экранные px, FR-038).
pub const SNAP_TOLERANCE_MAX: f32 = 32.0;

/// Пресеты порога sub-сетки (FR-038). Диапазоны пресетов sub и coarse НЕ
/// пересекаются (минимум sub 1.25 > максимум coarse 1.0) — любая пара
/// пресетов валидна по правилу «sub > coarse» (п.3).
pub const SNAP_SUB_ZOOM_PRESETS: [f32; 4] = [1.25, 1.5, 2.0, 3.0];
/// Границы клампа порога sub-сетки (ручные правки config.toml).
pub const SNAP_SUB_ZOOM_MIN: f32 = 1.0;
/// Границы клампа порога sub-сетки.
pub const SNAP_SUB_ZOOM_MAX: f32 = 6.0;

/// Пресеты порога coarse-сетки (FR-038) — см. [`SNAP_SUB_ZOOM_PRESETS`].
pub const SNAP_COARSE_ZOOM_PRESETS: [f32; 4] = [0.25, 0.5, 0.75, 1.0];
/// Границы клампа порога coarse-сетки (ручные правки config.toml).
pub const SNAP_COARSE_ZOOM_MIN: f32 = 0.1;
/// Границы клампа порога coarse-сетки.
pub const SNAP_COARSE_ZOOM_MAX: f32 = 1.0;

/// Следующий пресет допуска направляющих по циклу (FR-038, панель настроек).
/// Значение вне пресетов округляется к ближайшему меньшему — паттерн
/// [`next_port_zone`].
pub fn next_snap_tolerance(value: f32) -> f32 {
    let current = SNAP_TOLERANCE_PRESETS
        .iter()
        .rposition(|preset| *preset <= value)
        .unwrap_or(0);
    SNAP_TOLERANCE_PRESETS[(current + 1) % SNAP_TOLERANCE_PRESETS.len()]
}

/// Кламп допуска направляющих в допустимые границы (FR-038): защита от
/// ручной правки config.toml — паттерн [`clamp_port_zone`].
pub fn clamp_snap_tolerance(value: f32) -> f32 {
    value.clamp(SNAP_TOLERANCE_MIN, SNAP_TOLERANCE_MAX)
}

/// Следующий пресет порога sub-сетки по циклу (FR-038, панель настроек).
pub fn next_snap_sub_zoom(value: f32) -> f32 {
    let current = SNAP_SUB_ZOOM_PRESETS
        .iter()
        .rposition(|preset| *preset <= value)
        .unwrap_or(0);
    SNAP_SUB_ZOOM_PRESETS[(current + 1) % SNAP_SUB_ZOOM_PRESETS.len()]
}

/// Кламп порога sub-сетки в допустимые границы (FR-038).
pub fn clamp_snap_sub_zoom(value: f32) -> f32 {
    value.clamp(SNAP_SUB_ZOOM_MIN, SNAP_SUB_ZOOM_MAX)
}

/// Следующий пресет порога coarse-сетки по циклу (FR-038, панель настроек).
pub fn next_snap_coarse_zoom(value: f32) -> f32 {
    let current = SNAP_COARSE_ZOOM_PRESETS
        .iter()
        .rposition(|preset| *preset <= value)
        .unwrap_or(0);
    SNAP_COARSE_ZOOM_PRESETS[(current + 1) % SNAP_COARSE_ZOOM_PRESETS.len()]
}

/// Кламп порога coarse-сетки в допустимые границы (FR-038).
pub fn clamp_snap_coarse_zoom(value: f32) -> f32 {
    value.clamp(SNAP_COARSE_ZOOM_MIN, SNAP_COARSE_ZOOM_MAX)
}

/// Валидация пары порогов zoom-адаптивной сетки (FR-038, п.3): суб-порог
/// обязан быть СТРОГО больше coarse-порога, оба — конечные положительные;
/// иначе пара заменяется дефолтами v2 (150%/50%). Ручная правка config.toml
/// (sub = 0.2, coarse = 4.0, NaN) не ломает ни снап-движок, ни рендер сетки.
pub fn validated_grid_zoom_thresholds(sub: f32, coarse: f32) -> (f32, f32) {
    let valid = sub > coarse && sub.is_finite() && coarse.is_finite() && sub > 0.0 && coarse > 0.0;
    if valid {
        (sub, coarse)
    } else {
        (SNAP_SUB_ZOOM_DEFAULT, SNAP_COARSE_ZOOM_DEFAULT)
    }
}

impl Settings {
    /// Загрузить настройки; отсутствующий или битый файл — дефолты
    /// (ошибка разбора возвращается для лога, приложение не падает).
    /// Числовые поля клампятся/валидуются ([`Self::normalize`]) — ручные
    /// правки не роняют UX (CR-003, FR-028, FR-038).
    pub fn load(path: &Path) -> (Self, Option<String>) {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return (Self::default(), None);
            }
            Err(err) => {
                return (Self::default(), Some(format!("чтение конфига: {err}")));
            }
        };
        match toml::from_str::<Self>(&text) {
            Ok(mut settings) => {
                settings.normalize();
                (settings, None)
            }
            Err(err) => (
                Self::default(),
                Some(format!("разбор конфига (используются дефолты): {err}")),
            ),
        }
    }

    /// Сохранить настройки (создаёт родительские каталоги).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        std::fs::write(path, text)
    }

    /// Клампы/валидация числовых полей после разбора конфига: ручные правки
    /// config.toml не роняют UX (CR-003 — зона портов, FR-028 — откладывания
    /// онбординга, FR-038 — допуск и пара порогов сетки).
    fn normalize(&mut self) {
        self.port_zone_px = clamp_port_zone(self.port_zone_px);
        self.onboarding_defers = clamp_onboarding_defers(self.onboarding_defers);
        self.snap_tolerance_px = clamp_snap_tolerance(self.snap_tolerance_px);
        let (sub, coarse) =
            validated_grid_zoom_thresholds(self.snap_grid_sub_zoom, self.snap_grid_coarse_zoom);
        self.snap_grid_sub_zoom = sub;
        self.snap_grid_coarse_zoom = coarse;
    }

    /// Разбор из строки (тесты; логика общая с load, включая клампы
    /// [`Self::normalize`]).
    #[cfg(test)]
    fn load_toml_str(text: &str) -> (Self, Option<String>) {
        match toml::from_str::<Self>(text) {
            Ok(mut settings) => {
                settings.normalize();
                (settings, None)
            }
            Err(err) => (Self::default(), Some(err.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip: сохранённый конфиг читается обратно без потерь.
    #[test]
    fn toml_round_trip() {
        let settings = Settings {
            button_corner: Corner::BottomLeft,
            grid_visible: false,
            grid_style: GridStyle::Dots,
            grid_density: GridDensity::Sparse,
            theme: Theme::Light,
            edges_avoid_nodes: false,
            hud_on_start: true,
            focus_mode: true,
            port_zone_px: 28.0,
            template_palette_open: false,
            line_ports: true,
            onboarding_done: true,
            onboarding_defers: 2,
            bottleneck_overlay: true,
            language: Language::En,
            snap_enabled: false,
            snap_to_grid: false,
            snap_to_guides: true,
            snap_collision: true,
            snap_tolerance_px: 12.0,
            snap_grid_sub_zoom: 2.0,
            snap_grid_coarse_zoom: 0.25,
            snap_anchor: SnapAnchor::Center,
            edge_aggregation: true,
        };
        let dir = crate::test_scratch_root().join("canvasdesk-settings-test"); // FR-036: wasm-совместимая песочница
        let path = dir.join("config.toml");
        settings.save(&path).expect("сохранение");
        let (loaded, warn) = Settings::load(&path);
        assert_eq!(loaded, settings);
        assert!(warn.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// CR-003: зона портов — пресеты, цикл замкнут, кламп ручных значений.
    #[test]
    fn port_zone_cycle_and_clamp() {
        // Цикл по пресетам замкнут и без повторов на витке
        let mut zone = PORT_ZONE_PRESETS[0];
        let first = zone;
        let mut seen = vec![zone];
        for _ in 0..PORT_ZONE_PRESETS.len() - 1 {
            zone = next_port_zone(zone);
            assert!(!seen.contains(&zone), "повтор в цикле: {zone}");
            seen.push(zone);
        }
        assert_eq!(next_port_zone(zone), first);
        // Вне пресетов — ближайший меньший, затем следующий по циклу
        assert_eq!(next_port_zone(25.0), PORT_ZONE_PRESETS[3]);
        // Кламп: нижняя/верхняя границы, отрицательные и гигантские
        assert_eq!(clamp_port_zone(3.0), PORT_ZONE_MIN);
        assert_eq!(clamp_port_zone(-1.0), PORT_ZONE_MIN);
        assert_eq!(clamp_port_zone(999.0), PORT_ZONE_MAX);
        assert_eq!(clamp_port_zone(22.0), 22.0);
        // Дефолт — первый пресет
        assert_eq!(Settings::default().port_zone_px, PORT_ZONE_PRESETS[0]);
    }

    /// CR-003: кламп зоны портов при загрузке конфига — ручная правка
    /// config.toml не даёт нулевую/гигантскую зону.
    #[test]
    fn port_zone_clamped_on_load() {
        let (settings, warn) = Settings::load_toml_str("port_zone_px = 5.0\n");
        assert_eq!(settings.port_zone_px, PORT_ZONE_MIN);
        assert!(warn.is_none(), "кламп молчалив — значение валидно числово");
        let (settings, _) = Settings::load_toml_str("port_zone_px = 500.0\n");
        assert_eq!(settings.port_zone_px, PORT_ZONE_MAX);
        // Отсутствие поля — дефолт
        let (settings, _) = Settings::load_toml_str("grid_visible = false\n");
        assert_eq!(settings.port_zone_px, PORT_ZONE_PRESETS[0]);
    }

    /// Отсутствующий/битый файл — дефолты, без паники; битый — с предупреждением.
    #[test]
    fn broken_or_missing_gives_defaults() {
        let dir = crate::test_scratch_root().join("canvasdesk-settings-broken"); // FR-036
        let _ = std::fs::create_dir_all(&dir);
        let missing = dir.join("nope.toml");
        let (settings, warn) = Settings::load(&missing);
        assert_eq!(settings, Settings::default());
        assert!(warn.is_none(), "отсутствие файла — не ошибка");

        let broken = dir.join("broken.toml");
        std::fs::write(&broken, "button_corner = [это не toml").expect("запись");
        let (settings, warn) = Settings::load(&broken);
        assert_eq!(settings, Settings::default());
        assert!(warn.is_some(), "битый файл — дефолты + предупреждение");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Частичный конфиг: отсутствующие поля — дефолтами (расширяемость схемы).
    #[test]
    fn partial_config_uses_defaults() {
        let (settings, warn) = Settings::load_toml_str("grid_visible = false\n");
        assert!(!settings.grid_visible);
        assert_eq!(settings.button_corner, Corner::TopRight);
        assert!(
            !settings.template_palette_open,
            "ревизия FR-025: дефолт — свёрнутая палитра"
        );
        assert_eq!(settings.grid_style, GridStyle::Lines);
        assert_eq!(settings.grid_density, GridDensity::Medium);
        assert_eq!(settings.theme, Theme::Dark);
        assert!(settings.edges_avoid_nodes, "дефолт — огибать ноды");
        assert!(!settings.hud_on_start);
        assert!(warn.is_none());
    }

    /// FR-028: онбординг-поля — старый конфиг без них грузится дефолтами
    /// (`onboarding_done = false`, `onboarding_defers = 0` — тур показать);
    /// ручная правка `onboarding_defers = 99` клампится к лимиту 3.
    #[test]
    fn onboarding_fields_defaults_and_clamp() {
        let (settings, _) = Settings::load_toml_str("grid_visible = false\n");
        assert!(!settings.onboarding_done, "старый конфиг — тур не пройден");
        assert_eq!(
            settings.onboarding_defers, 0,
            "старый конфиг — без откладываний"
        );
        let (settings, _) = Settings::load_toml_str("onboarding_defers = 99\n");
        assert_eq!(settings.onboarding_defers, ONBOARDING_MAX_DEFERS);
        // Значения в диапазоне не трогаются
        let (settings, _) = Settings::load_toml_str("onboarding_defers = 2\n");
        assert_eq!(settings.onboarding_defers, 2);
        // Дефолты
        let defaults = Settings::default();
        assert!(!defaults.onboarding_done);
        assert_eq!(defaults.onboarding_defers, 0);
    }

    /// FR-040: язык — дефолт `ru` для старых конфигов без поля, чтение
    /// `language = "en"`, round-trip выбора, цикл из двух замкнут.
    #[test]
    fn language_defaults_and_round_trip() {
        // Старый конфиг без поля — русский, без предупреждения
        let (settings, warn) = Settings::load_toml_str("grid_visible = false\n");
        assert_eq!(settings.language, Language::Ru, "дефолт — русский");
        assert!(warn.is_none());
        // Поле читается
        let (settings, warn) = Settings::load_toml_str("language = \"en\"\n");
        assert_eq!(settings.language, Language::En);
        assert!(warn.is_none());
        // Цикл замкнут из двух
        assert_eq!(Language::Ru.next(), Language::En);
        assert_eq!(Language::En.next(), Language::Ru);
        // Названия в собственной локали — непустые
        assert_eq!(Language::Ru.native_label(), "русский");
        assert_eq!(Language::En.native_label(), "English");
        // Round-trip: сохранённый выбор читается обратно
        let dir = crate::test_scratch_root().join("canvasdesk-language-test");
        let path = dir.join("config.toml");
        let settings = Settings {
            language: Language::En,
            ..Settings::default()
        };
        settings.save(&path).expect("сохранение");
        let (loaded, warn) = Settings::load(&path);
        assert_eq!(loaded.language, Language::En);
        assert!(warn.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Тема: переключение замкнуто, подписи непустые, дефолт — тёмная.
    #[test]
    fn theme_cycle_and_labels() {
        assert_eq!(Theme::Dark.next(), Theme::Light);
        assert_eq!(Theme::Light.next(), Theme::Dark);
        assert_eq!(Settings::default().theme, Theme::Dark);
        for theme in [Theme::Dark, Theme::Light] {
            assert!(!theme.label().is_empty());
        }
    }

    /// Вид сетки: переключение замкнуто, подписи непустые.
    #[test]
    fn grid_style_cycle_and_labels() {
        assert_eq!(GridStyle::Lines.next(), GridStyle::Dots);
        assert_eq!(GridStyle::Dots.next(), GridStyle::Lines);
        for style in [GridStyle::Lines, GridStyle::Dots] {
            assert!(!style.label().is_empty());
        }
    }

    /// Плотность сетки: шаги мелкой/крупной линий, замкнутый цикл, подписи.
    #[test]
    fn grid_density_steps_cycle() {
        // Базовая (SPEC T2) — средняя
        assert_eq!(GridDensity::Medium.steps(), (20.0, 100.0));
        assert_eq!(GridDensity::Dense.steps(), (10.0, 50.0));
        assert_eq!(GridDensity::Sparse.steps(), (40.0, 200.0));
        // Цикл из 3 без повторов
        let start = GridDensity::Dense;
        let mut density = start;
        let mut seen = vec![density];
        for _ in 0..2 {
            density = density.next();
            assert!(!seen.contains(&density));
            seen.push(density);
        }
        assert_eq!(density.next(), start);
        for density in seen {
            assert!(!density.label().is_empty());
        }
    }

    /// Цикл углов замкнут: 4 переключения возвращают в исходный.
    #[test]
    fn corner_cycle_is_closed() {
        let start = Corner::TopLeft;
        let mut corner = start;
        let mut seen = vec![corner];
        for _ in 0..3 {
            corner = corner.next();
            assert!(
                !seen.contains(&corner),
                "цикл не должен повторяться раньше 4"
            );
            seen.push(corner);
        }
        assert_eq!(corner.next(), start);
        for corner in seen {
            assert!(!corner.label().is_empty());
        }
    }

    /// Имена полей в TOML — snake_case (стабильный формат файла).
    #[test]
    fn field_names_snake_case() {
        let text = toml::to_string_pretty(&Settings {
            button_corner: Corner::BottomRight,
            ..Settings::default()
        })
        .expect("сериализация");
        assert!(text.contains("button_corner = \"bottom_right\""), "{text}");
        assert!(text.contains("grid_visible"), "{text}");
        assert!(text.contains("grid_style"), "{text}");
        assert!(text.contains("grid_density"), "{text}");
        assert!(text.contains("theme"), "{text}");
        assert!(text.contains("edges_avoid_nodes"), "{text}");
        assert!(text.contains("hud_on_start"), "{text}");
        assert!(text.contains("port_zone_px"), "{text}");
        assert!(text.contains("line_ports"), "{text}");
        assert!(text.contains("onboarding_done"), "{text}");
        assert!(text.contains("onboarding_defers"), "{text}");
        assert!(text.contains("bottleneck_overlay"), "{text}");
        assert!(text.contains("language"), "{text}");
        // FR-038: поля магнитной раскладки — snake_case
        assert!(text.contains("snap_enabled"), "{text}");
        assert!(text.contains("snap_to_grid"), "{text}");
        assert!(text.contains("snap_to_guides"), "{text}");
        assert!(text.contains("snap_collision"), "{text}");
        assert!(text.contains("snap_tolerance_px"), "{text}");
        assert!(text.contains("snap_grid_sub_zoom"), "{text}");
        assert!(text.contains("snap_grid_coarse_zoom"), "{text}");
        assert!(text.contains("snap_anchor = \"bounding_box\""), "{text}");
    }

    /// FR-038: snap-поля — старый конфиг без них грузится дефолтами v2:
    /// мастер и оба тумблера включены, collision выключен, допуск — второй
    /// пресет (6 px), пороги 150%/50%, якорь — bbox (п.1/5/6/15/19).
    #[test]
    fn snap_fields_defaults_on_old_config() {
        let (settings, warn) = Settings::load_toml_str("grid_visible = false\n");
        assert!(settings.snap_enabled, "мастер-тумблер — вкл");
        assert!(settings.snap_to_grid, "привязка к сетке — вкл");
        assert!(settings.snap_to_guides, "направляющие — вкл");
        assert!(!settings.snap_collision, "collision — выкл (опция п.15)");
        assert_eq!(settings.snap_tolerance_px, SNAP_TOLERANCE_PRESETS[1]);
        assert_eq!(settings.snap_grid_sub_zoom, 1.5);
        assert_eq!(settings.snap_grid_coarse_zoom, 0.5);
        assert_eq!(settings.snap_anchor, SnapAnchor::BoundingBox);
        assert!(warn.is_none());
        // Флаги читаются из конфига
        let (settings, warn) = Settings::load_toml_str(
            "snap_enabled = false\nsnap_collision = true\nsnap_anchor = \"corner\"\n",
        );
        assert!(!settings.snap_enabled);
        assert!(settings.snap_collision);
        assert_eq!(settings.snap_anchor, SnapAnchor::Corner);
        assert!(warn.is_none());
    }

    /// FR-038: допуск направляющих — пресеты, цикл замкнут, кламп ручных
    /// значений и кламп при загрузке (паттерн port_zone, CR-003).
    #[test]
    fn snap_tolerance_cycle_clamp_and_load() {
        // Цикл по пресетам замкнут без повторов на витке
        let mut value = SNAP_TOLERANCE_PRESETS[0];
        let first = value;
        let mut seen = vec![value];
        for _ in 0..SNAP_TOLERANCE_PRESETS.len() - 1 {
            value = next_snap_tolerance(value);
            assert!(!seen.contains(&value), "повтор в цикле: {value}");
            seen.push(value);
        }
        assert_eq!(next_snap_tolerance(value), first);
        // Вне пресетов — ближайший меньший, затем следующий
        assert_eq!(next_snap_tolerance(10.0), SNAP_TOLERANCE_PRESETS[3]);
        // Кламп: границы и экстремумы
        assert_eq!(clamp_snap_tolerance(1.5), SNAP_TOLERANCE_MIN);
        assert_eq!(clamp_snap_tolerance(-5.0), SNAP_TOLERANCE_MIN);
        assert_eq!(clamp_snap_tolerance(99.0), SNAP_TOLERANCE_MAX);
        assert_eq!(clamp_snap_tolerance(7.0), 7.0);
        // Дефолт — второй пресет (стартовый ориентир v2 «несколько пикселей»)
        assert_eq!(Settings::default().snap_tolerance_px, 6.0);
        // Кламп при загрузке
        let (settings, warn) = Settings::load_toml_str("snap_tolerance_px = 999.0\n");
        assert_eq!(settings.snap_tolerance_px, SNAP_TOLERANCE_MAX);
        assert!(warn.is_none(), "кламп молчалив — значение валидно числово");
    }

    /// FR-038: пороги zoom-адаптивной сетки — пресеты (диапазоны sub и
    /// coarse не пересекаются — любая пара пресетов валидна), клампы и
    /// валидация пары «sub > coarse, иначе дефолты» на загрузке.
    #[test]
    fn snap_zoom_thresholds_presets_and_validation() {
        // Дефолты — ориентиры v2 150%/50% (паритет с рендером сетки ловит
        // тест measure_layout_consts_match_render в canvas-app: core→render
        // зависимости по архитектуре нет — ADR-0012)
        assert_eq!(Settings::default().snap_grid_sub_zoom, 1.5);
        assert_eq!(Settings::default().snap_grid_coarse_zoom, 0.5);
        // Любая пара пресетов валидна: минимальный sub > максимального coarse
        for sub in SNAP_SUB_ZOOM_PRESETS {
            for coarse in SNAP_COARSE_ZOOM_PRESETS {
                let (s, c) = validated_grid_zoom_thresholds(sub, coarse);
                assert_eq!((s, c), (sub, coarse), "пара пресетов испорчена");
                assert!(s > c);
            }
        }
        // Циклы замкнуты
        let mut sub = SNAP_SUB_ZOOM_PRESETS[0];
        for _ in 0..SNAP_SUB_ZOOM_PRESETS.len() - 1 {
            sub = next_snap_sub_zoom(sub);
        }
        assert_eq!(next_snap_sub_zoom(sub), SNAP_SUB_ZOOM_PRESETS[0]);
        let mut coarse = SNAP_COARSE_ZOOM_PRESETS[0];
        for _ in 0..SNAP_COARSE_ZOOM_PRESETS.len() - 1 {
            coarse = next_snap_coarse_zoom(coarse);
        }
        assert_eq!(next_snap_coarse_zoom(coarse), SNAP_COARSE_ZOOM_PRESETS[0]);
        // Клампы
        assert_eq!(clamp_snap_sub_zoom(0.5), SNAP_SUB_ZOOM_MIN);
        assert_eq!(clamp_snap_sub_zoom(50.0), SNAP_SUB_ZOOM_MAX);
        assert_eq!(clamp_snap_coarse_zoom(0.01), SNAP_COARSE_ZOOM_MIN);
        assert_eq!(clamp_snap_coarse_zoom(9.0), SNAP_COARSE_ZOOM_MAX);
        // Валидация: инверсия/равенство/не-конечность — дефолты
        assert_eq!(validated_grid_zoom_thresholds(0.5, 1.5), (1.5, 0.5));
        assert_eq!(validated_grid_zoom_thresholds(1.0, 1.0), (1.5, 0.5));
        assert_eq!(validated_grid_zoom_thresholds(f32::NAN, 0.5), (1.5, 0.5));
        assert_eq!(validated_grid_zoom_thresholds(2.0, -1.0), (1.5, 0.5));
        // Валидные значения проходят без изменений
        assert_eq!(validated_grid_zoom_thresholds(2.5, 0.75), (2.5, 0.75));
        // Кламп+валидация на загрузке: перепутанная пара заменяется дефолтом
        let (settings, warn) =
            Settings::load_toml_str("snap_grid_sub_zoom = 0.2\nsnap_grid_coarse_zoom = 4.0\n");
        assert_eq!(settings.snap_grid_sub_zoom, SNAP_SUB_ZOOM_DEFAULT);
        assert_eq!(settings.snap_grid_coarse_zoom, SNAP_COARSE_ZOOM_DEFAULT);
        assert!(warn.is_none());
    }

    /// FR-025: флаг построчных точек выхода — дефолт false (старые конфиги
    /// без поля — прежнее поведение).
    #[test]
    fn line_ports_defaults_off_and_round_trips() {
        let (settings, warn) = Settings::load_toml_str("grid_visible = false\n");
        assert!(!settings.line_ports, "дефолт — выкл");
        assert!(warn.is_none());
        let (settings, warn) = Settings::load_toml_str("line_ports = true\n");
        assert!(settings.line_ports, "флаг читается из конфига");
        assert!(warn.is_none());
    }

    /// FR-016 (CP5): флаг оверлея узких мест — дефолт false (старые конфиги
    /// без поля — прежнее поведение).
    #[test]
    fn bottleneck_overlay_defaults_off_and_round_trips() {
        let (settings, warn) = Settings::load_toml_str("grid_visible = false\n");
        assert!(!settings.bottleneck_overlay, "дефолт — выкл");
        assert!(warn.is_none());
        let (settings, warn) = Settings::load_toml_str("bottleneck_overlay = true\n");
        assert!(settings.bottleneck_overlay, "флаг читается из конфига");
        assert!(warn.is_none());
    }
}
