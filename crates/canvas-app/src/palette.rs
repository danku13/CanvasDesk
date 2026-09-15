//! Палитра выделения (FR-009/FR-010) — плавающий тулбар над выделенным
//! объектом: нодой/нодами (CR-001) или связью.
//!
//! Заменяет текстовые контекстные меню ноды и связи (уточнение владельца:
//! «FR-009/FR-010 нужно превратить в палитру»). Свойства:
//!
//! - **Screen-space**: все координаты — логические px от угла окна; размер
//!   константен при любом зуме (уточнение владельца: «контекстное меню не
//!   должно масштабироваться с canvas»).
//! - **Группы настроек**: кнопки-триггеры в баре; по наведению на ТРИГГЕР
//!   раскрывается выпадающий перечень кнопок (`PaletteHover` — hover-intent
//!   по практикам фронта: открытие только от кнопки, задержка открытия,
//!   отсрочка закрытия, пин по клику). Пустая область, ГДЕ появилась бы
//!   выпадашка, ничего не открывает — как в Radix/Bootstrap dropdown.
//!   Пока выпадашка открыта, её колонка удерживает открытие (переход
//!   курсора кнопка → колонка не закрывает).
//! - **Иконки вместо текста там, где наглядно**: тип линии связи —
//!   сплошная/пунктир/точки, толщина, свотчи цвета, схемы раскладки
//!   (дерево →/↓, радиально) — иконки; редкие действия — текст.
//!
//! Иконки — векторные композиции квадов (`icon_quads`) через тот же
//! SDF-пайплайн карточек: без SVG-растеризатора и текстур. Геометрия —
//! чистые функции (тесты без GPU/окна).

use std::time::{Duration, Instant};

use canvas_core::{Canvas, EdgeLineStyle, EdgeThickness, FlowKind, NodeKind};

use canvas_render::ThemeColors;

use crate::ui::{point_in_rect, NodeSetting};
use crate::{preset_color, CardInstance, Vec2};

// --- Размеры (логические px, screen-space) ---

/// Сторона кнопки группы.
pub const PAL_BUTTON: f32 = 30.0;
/// Высота подписи под кнопкой группы.
pub const PAL_CAPTION_H: f32 = 12.0;
/// Внутренний отступ бара.
pub const PAL_BAR_PAD: f32 = 6.0;
/// Зазор между кнопками групп.
pub const PAL_GAP: f32 = 5.0;
/// Высота строки выпадающего перечня.
pub const PAL_ROW_H: f32 = 26.0;
/// Ширина колонки выпадающего перечня.
pub const PAL_ROW_W: f32 = 176.0;
/// Внутренний отступ колонки выпадашки.
pub const PAL_DROP_PAD: f32 = 5.0;
/// Зазор между баром и колонкой.
pub const PAL_DROP_GAP: f32 = 3.0;
/// Зазор от якоря (низ выделения) до бара.
pub const PAL_ANCHOR_GAP: f32 = 10.0;
/// Минимальный отступ бара/колонки от краёв окна.
pub const PAL_MARGIN: f32 = 8.0;
/// Сторона квадрата иконки в строке выпадашки.
pub const PAL_ICON: f32 = 18.0;

/// Цель палитры: выделенные ноды (primary + мультивыделение CR-001)
/// или выделенная связь.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteTarget {
    Nodes {
        primary: usize,
        selected: Vec<usize>,
    },
    Edge(usize),
}

/// Действие кнопки палитры.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteAction {
    /// FR-009: настройка/действие ноды (диспетчер `apply_node_setting`).
    Node {
        node_index: usize,
        setting: NodeSetting,
    },
    /// Цвет ноды: пресет "1".."6" или None — сброс. Мультивыделение
    /// применяется ко всем `targets`.
    NodeColor {
        targets: Vec<usize>,
        preset: Option<&'static str>,
    },
    /// FR-009: обернуть ноду в группу (bbox = нода + padding).
    NodeGroup(usize),
    /// FR-010: авто-раскладка связанных карточек от ноды-семени.
    Layout {
        seed: usize,
        mode: canvas_core::LayoutMode,
    },
    /// Стиль линии связи.
    EdgeStyle {
        edge_index: usize,
        style: EdgeLineStyle,
    },
    /// Толщина связи.
    EdgeThickness {
        edge_index: usize,
        thickness: EdgeThickness,
    },
    /// Цвет связи: пресет или сброс.
    EdgeColor {
        edge_index: usize,
        preset: Option<&'static str>,
    },
    /// FR-014: тип потока связи — value (переносит значение) или control
    /// (визуальная связь). Переключение — undo-шаг + пересчёт потока.
    EdgeFlowKind { edge_index: usize, kind: FlowKind },
}

/// Векторная иконка кнопки: композиция квадов (`icon_quads`) и/или
/// текстовый глиф (`icon_text`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteIcon {
    /// Свотч цвета (пресет "1".."6") или контурный квадрат (сброс).
    Swatch(Option<&'static str>),
    /// Линия связи: сплошная / пунктир / точки.
    LineSolid,
    LineDashed,
    LineDotted,
    /// Толщина связи: тонкая / обычная / толстая полоса.
    Thin,
    Medium,
    Thick,
    /// FR-010: раскладка деревом горизонтально / вертикально / радиально.
    TreeHorizontal,
    TreeVertical,
    Radial,
    /// FR-011: добавить дочернюю (+ квадрат снизу), сиблинга (+ квадрат
    /// справа), свернуть (−) / развернуть (+) ветку.
    AddChild,
    AddSibling,
    Collapse,
    Expand,
    /// Действия: дублировать (две карточки), папка, рамка группы.
    Duplicate,
    Folder,
    GroupBox,
    /// Кнопка группы «Действия»: три ползунка.
    Sliders,
    /// Текстовый глиф «Aa» (переименовать) — только `icon_text`.
    Rename,
    /// Текстовый глиф «×» (очистить текст) — только `icon_text`.
    Clear,
    /// FR-014: стрелка потока (группа «Поток» палитры связи).
    Flow,
}

/// Кнопка выпадающего перечня.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteEntry {
    pub action: PaletteAction,
    /// Подпись (рядом с иконкой; без иконки — вся строка).
    pub label: String,
    /// Иконка; None — строка только с текстом.
    pub icon: Option<PaletteIcon>,
    /// Текущее значение (подсветка строки — где мы сейчас).
    pub current: bool,
}

/// Группа кнопок: кнопка в баре + выпадающий перечень.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteGroup {
    /// Подпись под кнопкой группы.
    pub label: String,
    /// Иконка кнопки группы.
    pub icon: PaletteIcon,
    pub entries: Vec<PaletteEntry>,
}

/// Геометрия группы в баре (логические px).
#[derive(Debug, Clone, PartialEq)]
pub struct GroupLayout {
    /// Кнопка группы [x, y, w, h].
    pub button: [f32; 4],
    /// Origin подписи группы (центрируется по ширине кнопки).
    pub caption: [f32; 2],
    /// Rect колонки выпадашки (геометрия есть всегда; рисуется по open).
    pub dropdown: [f32; 4],
    /// Rect'ы строк выпадашки (параллельно entries).
    pub rows: Vec<[f32; 4]>,
}

/// Геометрия палитры на кадр.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteLayout {
    /// Rect бара [x, y, w, h].
    pub bar: [f32; 4],
    pub groups: Vec<GroupLayout>,
}

/// Результат hit-test'а палитры.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteHit {
    /// Клик по строке выпадашки группы.
    Entry { group: usize, entry: usize },
    /// Клик по кнопке-триггеру группы (пин-переключение раскрытия —
    /// WAI-ARIA menu button: клик открывает/закрывает наряду с hover).
    Trigger(usize),
    /// Клик по бару вне кнопок (глотается — не проходит в канвас).
    Bar,
}

// --- Состав групп по цели ---

/// Пресет цвета как 'static-строка (для иконок Swatch): цвет ноды/связи —
/// один из пресетов "1".."6"; значение вне пресетов — None (контурный свотч).
fn static_preset(color: Option<&str>) -> Option<&'static str> {
    color.and_then(|c| ["1", "2", "3", "4", "5", "6"].into_iter().find(|p| *p == c))
}

/// Группы палитры для цели. Ноды: [Цвет][Раскладка][Действия][Ветвление?];
/// мультивыделение — [Цвет][Раскладка]; связь — [Стиль][Толщина][Цвет].
pub fn palette_groups(canvas: &Canvas, target: &PaletteTarget) -> Vec<PaletteGroup> {
    match target {
        PaletteTarget::Edge(edge_index) => edge_groups(canvas, *edge_index),
        PaletteTarget::Nodes { primary, selected } => node_groups(canvas, *primary, selected),
    }
}

/// Группы для выделенных нод.
fn node_groups(canvas: &Canvas, primary: usize, selected: &[usize]) -> Vec<PaletteGroup> {
    let mut groups = Vec::new();
    groups.push(color_group(canvas, primary, selected));
    // FR-010: раскладка связанных — семя primary, из палитры под выделением
    groups.push(PaletteGroup {
        label: "Раскладка".to_owned(),
        icon: PaletteIcon::TreeHorizontal,
        entries: vec![
            layout_entry(primary, canvas_core::LayoutMode::TreeHorizontal, "Дерево →"),
            layout_entry(primary, canvas_core::LayoutMode::TreeVertical, "Дерево ↓"),
            layout_entry(primary, canvas_core::LayoutMode::Radial, "Радиально"),
        ],
    });
    // Мультивыделение: настройки конкретной ноды не имеют смысла
    if selected.len() <= 1 {
        if let Some(node) = canvas.nodes.get(primary) {
            groups.push(actions_group(primary, node.kind()));
            if node.kind() == NodeKind::Text {
                groups.push(branch_group(primary, node.collapsed == Some(true)));
            }
        }
    }
    groups
}

/// Группа «Цвет»: свотчи пресетов + сброс; мультивыделение — всем выделенным.
fn color_group(canvas: &Canvas, primary: usize, selected: &[usize]) -> PaletteGroup {
    let current = static_preset(
        canvas
            .nodes
            .get(primary)
            .and_then(|node| node.color.as_deref()),
    );
    let mut targets: Vec<usize> = selected.to_vec();
    if !targets.contains(&primary) {
        targets.push(primary);
    }
    let mut entries: Vec<PaletteEntry> = ["1", "2", "3", "4", "5", "6"]
        .into_iter()
        .map(|preset| PaletteEntry {
            action: PaletteAction::NodeColor {
                targets: targets.clone(),
                preset: Some(preset),
            },
            label: format!("Цвет {preset}"),
            icon: Some(PaletteIcon::Swatch(Some(preset))),
            current: current == Some(preset),
        })
        .collect();
    entries.push(PaletteEntry {
        action: PaletteAction::NodeColor {
            targets,
            preset: None,
        },
        label: "Без цвета".to_owned(),
        icon: Some(PaletteIcon::Swatch(None)),
        current: current.is_none(),
    });
    PaletteGroup {
        label: "Цвет".to_owned(),
        icon: PaletteIcon::Swatch(current),
        entries,
    }
}

fn layout_entry(seed: usize, mode: canvas_core::LayoutMode, label: &str) -> PaletteEntry {
    PaletteEntry {
        action: PaletteAction::Layout { seed, mode },
        label: label.to_owned(),
        icon: Some(match mode {
            canvas_core::LayoutMode::TreeHorizontal => PaletteIcon::TreeHorizontal,
            canvas_core::LayoutMode::TreeVertical => PaletteIcon::TreeVertical,
            canvas_core::LayoutMode::Radial => PaletteIcon::Radial,
        }),
        current: false,
    }
}

/// Группа «Действия» (FR-009): общие + типовые. Иконки — где наглядно,
/// редкие действия — текст.
fn actions_group(index: usize, kind: NodeKind) -> PaletteGroup {
    let entry = |setting: NodeSetting, label: &str, icon: Option<PaletteIcon>| PaletteEntry {
        action: PaletteAction::Node {
            node_index: index,
            setting,
        },
        label: label.to_owned(),
        icon,
        current: false,
    };
    let mut entries = vec![
        entry(
            NodeSetting::Rename,
            "Переименовать",
            Some(PaletteIcon::Rename),
        ),
        entry(
            NodeSetting::Duplicate,
            "Дублировать",
            Some(PaletteIcon::Duplicate),
        ),
        PaletteEntry {
            action: PaletteAction::NodeGroup(index),
            label: "Сгруппировать".to_owned(),
            icon: Some(PaletteIcon::GroupBox),
            current: false,
        },
    ];
    match kind {
        NodeKind::File => {
            entries.push(entry(
                NodeSetting::OpenFile,
                "Открыть файл",
                Some(PaletteIcon::Folder),
            ));
            entries.push(entry(
                NodeSetting::OpenFolder,
                "Открыть папку с файлом",
                Some(PaletteIcon::Folder),
            ));
            entries.push(entry(NodeSetting::CopyPath, "Скопировать путь", None));
        }
        NodeKind::Link => {
            entries.push(entry(NodeSetting::CopyPath, "Скопировать ссылку", None));
        }
        NodeKind::Text => {
            entries.push(entry(
                NodeSetting::ClearText,
                "Очистить текст",
                Some(PaletteIcon::Clear),
            ));
        }
        NodeKind::Group => {
            entries.push(entry(NodeSetting::Ungroup, "Разгруппировать", None));
        }
        NodeKind::Widget => {
            entries.push(entry(
                NodeSetting::WidgetReload,
                "Перезагрузить виджет",
                None,
            ));
            entries.push(entry(
                NodeSetting::WidgetPermissions,
                "Разрешения виджета…",
                None,
            ));
        }
        NodeKind::Unknown => {}
    }
    PaletteGroup {
        label: "Действия".to_owned(),
        icon: PaletteIcon::Sliders,
        entries,
    }
}

/// Группа «Ветвление» (FR-011, text-ноды): дочерняя, сиблинг, сворачивание.
fn branch_group(index: usize, collapsed: bool) -> PaletteGroup {
    let entry = |setting: NodeSetting, label: &str, icon: PaletteIcon| PaletteEntry {
        action: PaletteAction::Node {
            node_index: index,
            setting,
        },
        label: label.to_owned(),
        icon: Some(icon),
        current: false,
    };
    PaletteGroup {
        label: "Ветвление".to_owned(),
        icon: PaletteIcon::AddChild,
        entries: vec![
            entry(
                NodeSetting::AddChild,
                "Добавить дочернюю",
                PaletteIcon::AddChild,
            ),
            entry(
                NodeSetting::AddSibling,
                "Добавить сиблинга",
                PaletteIcon::AddSibling,
            ),
            if collapsed {
                entry(
                    NodeSetting::ExpandBranch,
                    "Развернуть ветку",
                    PaletteIcon::Expand,
                )
            } else {
                entry(
                    NodeSetting::CollapseBranch,
                    "Свернуть ветку",
                    PaletteIcon::Collapse,
                )
            },
        ],
    }
}

/// Группы для выделенной связи: [Стиль][Толщина][Цвет] с иконками.
fn edge_groups(canvas: &Canvas, edge_index: usize) -> Vec<PaletteGroup> {
    let Some(edge) = canvas.edges.get(edge_index) else {
        return Vec::new();
    };
    let style_entry = |style: EdgeLineStyle, label: &str, icon: PaletteIcon| PaletteEntry {
        action: PaletteAction::EdgeStyle { edge_index, style },
        label: label.to_owned(),
        icon: Some(icon),
        current: edge.style.unwrap_or(EdgeLineStyle::Solid) == style,
    };
    let thickness_entry = |thickness: EdgeThickness, label: &str, icon: PaletteIcon| PaletteEntry {
        action: PaletteAction::EdgeThickness {
            edge_index,
            thickness,
        },
        label: label.to_owned(),
        icon: Some(icon),
        current: edge.thickness.unwrap_or(EdgeThickness::Medium) == thickness,
    };
    let current = static_preset(edge.color.as_deref());
    let mut color_entries: Vec<PaletteEntry> = ["1", "2", "3", "4", "5", "6"]
        .into_iter()
        .map(|preset| PaletteEntry {
            action: PaletteAction::EdgeColor {
                edge_index,
                preset: Some(preset),
            },
            label: format!("Цвет {preset}"),
            icon: Some(PaletteIcon::Swatch(Some(preset))),
            current: current == Some(preset),
        })
        .collect();
    color_entries.push(PaletteEntry {
        action: PaletteAction::EdgeColor {
            edge_index,
            preset: None,
        },
        label: "Без цвета".to_owned(),
        icon: Some(PaletteIcon::Swatch(None)),
        current: current.is_none(),
    });
    vec![
        PaletteGroup {
            label: "Стиль".to_owned(),
            icon: PaletteIcon::LineSolid,
            entries: vec![
                style_entry(EdgeLineStyle::Solid, "Сплошная", PaletteIcon::LineSolid),
                style_entry(EdgeLineStyle::Dashed, "Пунктир", PaletteIcon::LineDashed),
                style_entry(EdgeLineStyle::Dotted, "Точки", PaletteIcon::LineDotted),
            ],
        },
        PaletteGroup {
            label: "Толщина".to_owned(),
            icon: PaletteIcon::Medium,
            entries: vec![
                thickness_entry(EdgeThickness::Thin, "Тонкая", PaletteIcon::Thin),
                thickness_entry(EdgeThickness::Medium, "Обычная", PaletteIcon::Medium),
                thickness_entry(EdgeThickness::Thick, "Толстая", PaletteIcon::Thick),
            ],
        },
        PaletteGroup {
            label: "Цвет".to_owned(),
            icon: PaletteIcon::Swatch(current),
            entries: color_entries,
        },
        // FR-014: тип потока — значение идёт в формулы downstream ($in/$N)
        // как value-ребро; control — визуальная связь. Пометка «(фолбэк при
        // цикле)» не нужна: цикл блокируется на создании/тогле.
        PaletteGroup {
            label: "Поток".to_owned(),
            icon: PaletteIcon::Flow,
            entries: vec![
                PaletteEntry {
                    action: PaletteAction::EdgeFlowKind {
                        edge_index,
                        kind: FlowKind::Value,
                    },
                    label: "Значение".to_owned(),
                    icon: Some(PaletteIcon::Flow),
                    current: edge.flow_kind() == FlowKind::Value,
                },
                PaletteEntry {
                    action: PaletteAction::EdgeFlowKind {
                        edge_index,
                        kind: FlowKind::Control,
                    },
                    label: "Контрольная".to_owned(),
                    icon: None,
                    current: edge.flow_kind() == FlowKind::Control,
                },
            ],
        },
    ]
}

// --- Геометрия ---

/// Размер бара по числу групп: [ширина, высота].
pub fn palette_bar_size(groups: &[PaletteGroup]) -> [f32; 2] {
    let width = PAL_BAR_PAD * 2.0
        + groups.len() as f32 * PAL_BUTTON
        + groups.len().saturating_sub(1) as f32 * PAL_GAP;
    let height = PAL_BAR_PAD * 2.0 + PAL_BUTTON + PAL_CAPTION_H;
    [width, height]
}

/// Origin бара под якорем (screen-точка под выделением): центр по x,
/// ниже якоря; кламп к краям окна.
pub fn palette_origin(anchor: [f32; 2], bar: [f32; 2], viewport: [f32; 2]) -> [f32; 2] {
    let max_x = (viewport[0] - bar[0] - PAL_MARGIN).max(PAL_MARGIN);
    let max_y = (viewport[1] - bar[1] - PAL_MARGIN).max(PAL_MARGIN);
    [
        (anchor[0] - bar[0] / 2.0).clamp(PAL_MARGIN, max_x),
        (anchor[1] + PAL_ANCHOR_GAP).clamp(PAL_MARGIN, max_y),
    ]
}

/// Геометрия палитры: кнопки групп, подписи, колонки выпадашек (геометрия
/// есть у всех — видимость определяет hover). Колонка, не помещающаяся
/// под баром, раскрывается над ним.
pub fn palette_layout(
    origin: [f32; 2],
    groups: &[PaletteGroup],
    viewport: [f32; 2],
) -> PaletteLayout {
    let bar = palette_bar_size(groups);
    let bar_bottom = origin[1] + bar[1];
    let layout_groups = groups
        .iter()
        .enumerate()
        .map(|(i, group)| {
            let bx = origin[0] + PAL_BAR_PAD + i as f32 * (PAL_BUTTON + PAL_GAP);
            let by = origin[1] + PAL_BAR_PAD;
            let button = [bx, by, PAL_BUTTON, PAL_BUTTON];
            let caption = [bx, by + PAL_BUTTON + 1.0];
            let drop_h = PAL_DROP_PAD * 2.0 + group.entries.len() as f32 * PAL_ROW_H;
            let max_drop_x = (viewport[0] - PAL_ROW_W - PAL_MARGIN).max(PAL_MARGIN);
            let dx = (bx + PAL_BUTTON / 2.0 - PAL_ROW_W / 2.0).clamp(PAL_MARGIN, max_drop_x);
            let dy_below = bar_bottom + PAL_DROP_GAP;
            let dy = if dy_below + drop_h > viewport[1] - PAL_MARGIN {
                // Снизу не помещается — раскрываем над баром
                (origin[1] - drop_h - PAL_DROP_GAP).max(PAL_MARGIN)
            } else {
                dy_below
            };
            let rows = group
                .entries
                .iter()
                .enumerate()
                .map(|(k, _)| {
                    [
                        dx + PAL_DROP_PAD,
                        dy + PAL_DROP_PAD + k as f32 * PAL_ROW_H,
                        PAL_ROW_W - PAL_DROP_PAD * 2.0,
                        PAL_ROW_H,
                    ]
                })
                .collect();
            GroupLayout {
                button,
                caption,
                dropdown: [dx, dy, PAL_ROW_W, drop_h],
                rows,
            }
        })
        .collect();
    PaletteLayout {
        bar: [origin[0], origin[1], bar[0], bar[1]],
        groups: layout_groups,
    }
}

/// Кнопка-триггер группы под курсором. ТОЛЬКО кнопки в баре: раскрытие
/// срабатывает от наведения на кнопку, а не от пустой области колонки
/// (баг-репорт владельца: «выпадающие меню активируются наведением на
/// область, где должно появляться выпадающее меню»). Колонка раскрытой
/// группы учитывается отдельно — в `PaletteHover::update`.
pub fn palette_trigger_at(lay: &PaletteLayout, point: Vec2) -> Option<usize> {
    lay.groups
        .iter()
        .enumerate()
        .find(|(_, g)| point_in_rect(g.button, point))
        .map(|(i, _)| i)
}

/// Задержка открытия по наведению (hover-intent, NN/g «задержка и
/// триггерная зона»): фильтрует случайные раскрытия при проводе курсора
/// через бар к канвасу.
pub const PALETTE_OPEN_DELAY_MS: u64 = 150;
/// Отсрочка закрытия после ухода курсора с открытой зоны (grace period,
/// как mouseleave-задержка в Bootstrap-дропдаунах): переживает щели
/// между баром и колонкой и микросходы с триггера.
pub const PALETTE_CLOSE_DELAY_MS: u64 = 300;

/// Состояние hover-раскрытия групп палитры (FR-009/FR-010).
///
/// Практики фронтенда для выпадающих меню, реализованные здесь:
///
/// 1. **Открытие только от триггера** (Radix UI DropdownMenu:
///    `onPointerEnter` на триггере) — пустая область под баром, где
///    колонка ПОЯВИТСЯ, ничего не открывает (баг-репорт владельца).
/// 2. **Hover-intent задержка открытия** (NN/g: задержка перед показом
///    hover-контента; Bootstrap `.dropdown:hover` + transition-delay) —
///    [`PALETTE_OPEN_DELAY_MS`].
/// 3. **Удержание открытой зоны**: пока раскрыто, курсор на колонке,
///    баре или триггере держит открытие; уход закрывает с отсрочкой
///    [`PALETTE_CLOSE_DELAY_MS`] (grace period, Amazon safe-triangle
///    решает то же — движение по диагонали не роняет меню).
/// 4. **Переключение групп** с задержкой открытия (а не мгновенно):
///    провод через соседние кнопки не мигает промежуточными колонками.
/// 5. **Пин по клику** (WAI-ARIA menu button pattern): клик по триггеру
///    открывает/закрывает раскрытие — стабильность для точного наведения.
/// 6. **Stale-guard**: смена цели/геометрии сбрасывает состояние —
///    индекс раскрытой группы не переживает смену выделения.
#[derive(Debug, Clone)]
pub struct PaletteHover {
    /// Раскрытая группа (колонка рисуется и кликабельна только у неё).
    pub open: Option<usize>,
    /// (группа, момент входа курсора на её кнопку) — накопление
    /// hover-intent для задержки открытия.
    trigger_since: Option<(usize, Instant)>,
    /// Момент ухода курсора с открытой зоны — отсрочка закрытия.
    left_since: Option<Instant>,
}

impl Default for PaletteHover {
    fn default() -> Self {
        Self::new()
    }
}

impl PaletteHover {
    pub fn new() -> Self {
        Self {
            open: None,
            trigger_since: None,
            left_since: None,
        }
    }

    /// Идут кадры ожидания (hover-intent открытие / отсрочка закрытия) —
    /// `about_to_wait` держит цикл перерисовки, иначе задержки не сработают
    /// при неподвижном курсоре (рендер — по request_redraw, без VSync-цикла).
    pub fn pending(&self) -> bool {
        self.trigger_since.is_some() || self.left_since.is_some()
    }

    /// Кадровое обновление по курсору (вызывается на перерисовке и на
    /// клике). Возвращает раскрытую группу.
    pub fn update(&mut self, lay: &PaletteLayout, cursor: Vec2) -> Option<usize> {
        self.update_at(lay, cursor, Instant::now())
    }

    /// То же с явным «сейчас» — для тестов без реального времени.
    pub fn update_at(&mut self, lay: &PaletteLayout, cursor: Vec2, now: Instant) -> Option<usize> {
        // Stale-guard: цель сменилась — индекс может указывать на чужую
        // группу (состав групп у нод и связей разный)
        if let Some(g) = self.open {
            if lay.groups.get(g).is_none() {
                self.reset();
                return None;
            }
        }
        let trigger = palette_trigger_at(lay, cursor);
        let in_bar = point_in_rect(lay.bar, cursor);
        let in_open_column = self
            .open
            .and_then(|g| lay.groups.get(g))
            .is_some_and(|g| point_in_rect(g.dropdown, cursor));
        let open_delay = Duration::from_millis(PALETTE_OPEN_DELAY_MS);
        let close_delay = Duration::from_millis(PALETTE_CLOSE_DELAY_MS);
        match trigger {
            Some(g) if self.open == Some(g) => {
                // Стабильно: курсор на триггере раскрытой группы
                self.trigger_since = None;
                self.left_since = None;
            }
            Some(g) => {
                // Курсор на кнопке другой/закрытой группы — копим hover-intent
                // (переключение тоже с задержкой: провод через бар не мигает)
                self.left_since = None;
                match self.trigger_since {
                    Some((tg, since)) if tg == g => {
                        if now.duration_since(since) >= open_delay {
                            self.open = Some(g);
                            self.trigger_since = None;
                        }
                    }
                    _ => self.trigger_since = Some((g, now)),
                }
            }
            None if in_open_column || in_bar => {
                // Внутри палитры, но не на кнопке: колонка раскрытой группы,
                // подписи, паддинг бара — открытие держится (переход
                // кнопка → колонка через зазор не закрывает)
                self.trigger_since = None;
                self.left_since = None;
            }
            None => {
                // Вне палитры — отсрочка закрытия (диагональные сходы,
                // зазор между баром и колонкой)
                self.trigger_since = None;
                if self.open.is_some() {
                    match self.left_since {
                        None => self.left_since = Some(now),
                        Some(since) => {
                            if now.duration_since(since) >= close_delay {
                                self.reset();
                            }
                        }
                    }
                }
            }
        }
        self.open
    }

    /// Клик по кнопке-триггеру: пин-переключение раскрытия (WAI-ARIA
    /// menu button). Клик открывает без задержки — намерение явное.
    pub fn toggle_trigger(&mut self, group: usize) {
        if self.open == Some(group) {
            self.reset();
        } else {
            self.open = Some(group);
            self.trigger_since = None;
            self.left_since = None;
        }
    }

    /// Сброс: действие выполнено, цель сменилась, геометрия исчезла.
    pub fn reset(&mut self) {
        self.open = None;
        self.trigger_since = None;
        self.left_since = None;
    }
}

/// Hit-test палитры: строки ТОЛЬКО раскрытой группы (колонки групп
/// перекрываются по x — кликабельна лишь раскрытая), затем кнопки-триггеры
/// (пин-переключение), затем бар. Колонка ЗАКРЫТОЙ группы не ловит — клик
/// уходит в канвас (у колонок нет «призрачной» зоны).
pub fn palette_hit(lay: &PaletteLayout, point: Vec2, open: Option<usize>) -> Option<PaletteHit> {
    if let Some(gi) = open {
        if let Some(group) = lay.groups.get(gi) {
            for (ei, row) in group.rows.iter().enumerate() {
                if point_in_rect(*row, point) {
                    return Some(PaletteHit::Entry {
                        group: gi,
                        entry: ei,
                    });
                }
            }
        }
    }
    if let Some(g) = palette_trigger_at(lay, point) {
        return Some(PaletteHit::Trigger(g));
    }
    point_in_rect(lay.bar, point).then_some(PaletteHit::Bar)
}

// --- Иконки ---

/// glyphon `Color` (пак (a<<24)|(r<<16)|(g<<8)|b) → линейный rgba для квадов.
pub fn color_to_rgba(color: crate::Color) -> [f32; 4] {
    [
        color.r() as f32 / 255.0,
        color.g() as f32 / 255.0,
        color.b() as f32 / 255.0,
        color.a() as f32 / 255.0,
    ]
}

/// Текстовый глиф иконки (для не-квадовых): «Aa» — переименовать,
/// «×» — очистить, «→» — поток значений (FR-014). None — иконка только квадами.
pub fn icon_text(icon: PaletteIcon) -> Option<&'static str> {
    match icon {
        PaletteIcon::Rename => Some("Aa"),
        PaletteIcon::Clear => Some("×"),
        PaletteIcon::Flow => Some("→"),
        _ => None,
    }
}

/// Добавить квад иконки: `fill` (Some — заливка) и/или `border`
/// (Some — контур через border.a). params: [radius, 0, 0, без тени].
fn icon_quad(
    quads: &mut Vec<CardInstance>,
    pos: [f32; 2],
    size: [f32; 2],
    fill: Option<[f32; 4]>,
    border: Option<[f32; 4]>,
    radius: f32,
) {
    quads.push(CardInstance {
        pos,
        size,
        fill: fill.unwrap_or([0.0; 4]),
        border: border.unwrap_or([0.0; 4]),
        params: [radius, 0.0, 0.0, 1.0],
    });
}

/// Иконка как композиция квадов внутри rect (логические px). `tint` —
/// цвет штрихов (theme.icon). Чистая функция — тестируется на число квадов
/// и попадание в границы rect.
pub fn icon_quads(
    icon: PaletteIcon,
    rect: [f32; 4],
    tint: [f32; 4],
    theme: &ThemeColors,
) -> Vec<CardInstance> {
    let [x, y, w, h] = rect;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let mut quads: Vec<CardInstance> = Vec::new();
    // Хелперы — Fn-замыкания с &mut Vec аргументом (без захвата quads)
    let solid = |q: &mut Vec<CardInstance>, pos: [f32; 2], size: [f32; 2], radius: f32| {
        icon_quad(q, pos, size, Some(tint), None, radius);
    };
    let outline = |q: &mut Vec<CardInstance>, pos: [f32; 2], size: [f32; 2], radius: f32| {
        icon_quad(q, pos, size, None, Some(tint), radius);
    };
    match icon {
        PaletteIcon::Swatch(preset) => match preset.and_then(|p| preset_color(p, theme)) {
            Some(fill) => icon_quad(
                &mut quads,
                [x + 3.0, y + 3.0],
                [w - 6.0, h - 6.0],
                Some(fill),
                None,
                3.0,
            ),
            // «Без цвета»: контурный квадрат
            None => outline(&mut quads, [x + 3.0, y + 3.0], [w - 6.0, h - 6.0], 3.0),
        },
        PaletteIcon::LineSolid => solid(&mut quads, [x + 3.0, cy - 1.5], [w - 6.0, 3.0], 1.5),
        PaletteIcon::LineDashed => {
            let seg = (w - 12.0) / 3.0;
            for k in 0..3u8 {
                solid(
                    &mut quads,
                    [x + 3.0 + k as f32 * (seg + 3.0), cy - 1.5],
                    [seg, 3.0],
                    1.5,
                );
            }
        }
        PaletteIcon::LineDotted => {
            let step = (w - 11.0) / 3.0;
            for k in 0..4u8 {
                solid(
                    &mut quads,
                    [x + 4.0 + k as f32 * step, cy - 1.5],
                    [3.0, 3.0],
                    1.5,
                );
            }
        }
        PaletteIcon::Thin => solid(&mut quads, [x + 4.0, cy - 1.0], [w - 8.0, 2.0], 1.0),
        PaletteIcon::Medium => solid(&mut quads, [x + 4.0, cy - 2.0], [w - 8.0, 4.0], 2.0),
        PaletteIcon::Thick => solid(&mut quads, [x + 4.0, cy - 3.5], [w - 8.0, 7.0], 3.0),
        PaletteIcon::TreeHorizontal => {
            // Корень слева, два ребёнка справа; колено из осевых отрезков
            let (bx, tx, ty, by) = (x + 2.0, x + w - 10.0, y + 2.0, y + h - 10.0);
            solid(&mut quads, [bx, cy - 4.0], [8.0, 8.0], 2.0);
            solid(&mut quads, [tx, ty], [8.0, 8.0], 2.0);
            solid(&mut quads, [tx, by], [8.0, 8.0], 2.0);
            let spine = cx - 1.0;
            // Ствол от корня до спины
            solid(
                &mut quads,
                [bx + 8.0, cy - 1.0],
                [(spine - bx - 8.0).max(1.0), 2.0],
                0.0,
            );
            // Спина между детьми
            solid(
                &mut quads,
                [spine, ty + 8.0],
                [2.0, (by - ty).max(1.0)],
                0.0,
            );
            // Ветки от спины к детям (центры боков квадратов)
            solid(
                &mut quads,
                [spine + 2.0, ty + 3.0],
                [(tx - spine - 2.0).max(1.0), 2.0],
                0.0,
            );
            solid(
                &mut quads,
                [spine + 2.0, by + 3.0],
                [(tx - spine - 2.0).max(1.0), 2.0],
                0.0,
            );
        }
        PaletteIcon::TreeVertical => {
            // Корень сверху, два ребёнка снизу (поворот TreeHorizontal)
            let (by, lx, rx) = (y + h - 10.0, x + 2.0, x + w - 10.0);
            solid(&mut quads, [cx - 4.0, y + 2.0], [8.0, 8.0], 2.0);
            solid(&mut quads, [lx, by], [8.0, 8.0], 2.0);
            solid(&mut quads, [rx, by], [8.0, 8.0], 2.0);
            let spine = cy - 1.0;
            // Ствол вниз от корня
            solid(
                &mut quads,
                [cx - 1.0, y + 10.0],
                [2.0, (spine - y - 10.0).max(1.0)],
                0.0,
            );
            // Спина между детьми
            solid(
                &mut quads,
                [lx + 8.0, spine],
                [(rx - lx).max(1.0), 2.0],
                0.0,
            );
            // Ветки вниз к детям
            solid(
                &mut quads,
                [lx + 3.0, spine + 2.0],
                [2.0, (by - spine - 2.0).max(1.0)],
                0.0,
            );
            solid(
                &mut quads,
                [rx + 3.0, spine + 2.0],
                [2.0, (by - spine - 2.0).max(1.0)],
                0.0,
            );
        }
        PaletteIcon::Radial => {
            // Кольцо (контур круга) + центр + 4 точки по осям
            outline(
                &mut quads,
                [x + 1.0, y + 1.0],
                [w - 2.0, h - 2.0],
                (w - 2.0) / 2.0,
            );
            solid(&mut quads, [cx - 2.5, cy - 2.5], [5.0, 5.0], 2.5);
            solid(&mut quads, [cx - 2.0, y + 2.0], [4.0, 4.0], 2.0);
            solid(&mut quads, [cx - 2.0, y + h - 6.0], [4.0, 4.0], 2.0);
            solid(&mut quads, [x + 2.0, cy - 2.0], [4.0, 4.0], 2.0);
            solid(&mut quads, [x + w - 6.0, cy - 2.0], [4.0, 4.0], 2.0);
        }
        PaletteIcon::AddChild => {
            // Плюс + квадрат-ребёнок снизу
            solid(&mut quads, [cx - 6.0, cy - 6.0], [12.0, 2.0], 1.0);
            solid(&mut quads, [cx - 1.0, cy - 8.0], [2.0, 12.0], 1.0);
            solid(&mut quads, [cx - 3.0, y + h - 7.0], [6.0, 6.0], 1.5);
        }
        PaletteIcon::AddSibling => {
            // Плюс + квадрат-сиблинг справа
            solid(&mut quads, [cx - 7.0, cy - 1.0], [12.0, 2.0], 1.0);
            solid(&mut quads, [cx - 1.0, cy - 6.0], [2.0, 12.0], 1.0);
            solid(&mut quads, [x + w - 7.0, cy - 3.0], [6.0, 6.0], 1.5);
        }
        PaletteIcon::Collapse => solid(&mut quads, [cx - 6.0, cy - 1.0], [12.0, 2.0], 1.0),
        PaletteIcon::Expand => {
            solid(&mut quads, [cx - 6.0, cy - 1.0], [12.0, 2.0], 1.0);
            solid(&mut quads, [cx - 1.0, cy - 6.0], [2.0, 12.0], 1.0);
        }
        PaletteIcon::Duplicate => {
            // Задняя карточка контуром, передняя — заливкой
            outline(&mut quads, [x + 2.0, y + 3.0], [w - 7.0, h - 7.0], 2.0);
            solid(&mut quads, [x + 5.0, y + 2.0], [w - 7.0, h - 7.0], 2.0);
        }
        PaletteIcon::Folder => {
            // Язычок папки + корпус
            solid(&mut quads, [x + 3.0, y + 4.0], [7.0, 3.0], 1.0);
            solid(&mut quads, [x + 3.0, y + 7.0], [w - 6.0, h - 11.0], 1.5);
        }
        PaletteIcon::GroupBox => {
            // Рамка группы + квадратик-нода внутри
            outline(&mut quads, [x + 2.0, y + 2.0], [w - 4.0, h - 4.0], 2.0);
            solid(&mut quads, [cx - 3.0, cy - 3.0], [6.0, 6.0], 1.5);
        }
        PaletteIcon::Sliders => {
            // Три ползунка: линии + квадратики-каретки на разных позициях
            for (k, knob) in [(0.0f32, 0.25f32), (1.0, 0.6), (2.0, 0.4)] {
                let ly = y + 3.0 + k * 5.5;
                solid(&mut quads, [x + 3.0, ly], [w - 6.0, 1.6], 0.8);
                solid(
                    &mut quads,
                    [x + 3.0 + knob * (w - 10.0), ly - 1.7],
                    [3.4, 5.0],
                    1.0,
                );
            }
        }
        // Текстовые глифы — квадов нет (рисуются ScreenText'ом)
        PaletteIcon::Rename | PaletteIcon::Clear | PaletteIcon::Flow => {}
    }
    quads
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Canvas, Edge, Node};

    fn text_scene() -> Canvas {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "a", 100.0, 100.0));
        canvas
    }

    fn file_scene() -> Canvas {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("f", "C:/x.png", 0.0, 0.0, 100.0, 100.0));
        canvas
    }

    /// Геометрия палитры для одной текстовой ноды в большом вьюпорте.
    fn laid_out_palette() -> (Canvas, PaletteLayout) {
        let canvas = text_scene();
        let groups = palette_groups(
            &canvas,
            &PaletteTarget::Nodes {
                primary: 0,
                selected: vec![0],
            },
        );
        let viewport = [1200.0, 800.0];
        let origin = palette_origin([600.0, 300.0], palette_bar_size(&groups), viewport);
        (canvas, palette_layout(origin, &groups, viewport))
    }

    /// Состав групп для text-ноды: Цвет, Раскладка, Действия, Ветвление —
    /// в этом порядке; у цветовой группы 7 записей (6 пресетов + сброс).
    #[test]
    fn text_node_groups_composition() {
        let canvas = text_scene();
        let groups = palette_groups(
            &canvas,
            &PaletteTarget::Nodes {
                primary: 0,
                selected: vec![0],
            },
        );
        let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, vec!["Цвет", "Раскладка", "Действия", "Ветвление"]);
        assert_eq!(groups[0].entries.len(), 7);
        // Раскладка: 3 схемы FR-010
        assert_eq!(groups[1].entries.len(), 3);
        assert!(matches!(
            groups[1].entries[0].action,
            PaletteAction::Layout {
                mode: canvas_core::LayoutMode::TreeHorizontal,
                ..
            }
        ));
        // Ветвление: дочерняя, сиблинг, свернуть
        assert_eq!(groups[3].entries.len(), 3);
        assert!(matches!(
            groups[3].entries[2].action,
            PaletteAction::Node {
                setting: NodeSetting::CollapseBranch,
                ..
            }
        ));
    }

    /// Файловая нода: нет группы «Ветвление», типовые пункты — открыть
    /// файл/папку, скопировать путь.
    #[test]
    fn file_node_groups_composition() {
        let canvas = file_scene();
        let groups = palette_groups(
            &canvas,
            &PaletteTarget::Nodes {
                primary: 0,
                selected: vec![0],
            },
        );
        let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, vec!["Цвет", "Раскладка", "Действия"]);
        let actions: Vec<&str> = groups[2].entries.iter().map(|e| e.label.as_str()).collect();
        assert!(actions.contains(&"Открыть файл"));
        assert!(actions.contains(&"Открыть папку с файлом"));
        assert!(actions.contains(&"Скопировать путь"));
        assert!(!actions.contains(&"Свернуть ветку"));
    }

    /// Мультивыделение: только Цвет (ко всем выделенным) и Раскладка.
    #[test]
    fn multi_selection_groups() {
        let mut canvas = text_scene();
        canvas.nodes.push(Node::text("b", "b", 400.0, 100.0));
        let groups = palette_groups(
            &canvas,
            &PaletteTarget::Nodes {
                primary: 0,
                selected: vec![0, 1],
            },
        );
        let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, vec!["Цвет", "Раскладка"]);
        // Цвет применяется ко всем выделенным
        match &groups[0].entries[0].action {
            PaletteAction::NodeColor { targets, .. } => assert_eq!(targets, &vec![0, 1]),
            other => panic!("ожидался NodeColor: {other:?}"),
        }
    }

    /// Группы связи: Стиль/Толщина/Цвет/Поток (FR-014); текущие значения помечены.
    #[test]
    fn edge_groups_mark_current() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "a", 0.0, 0.0));
        canvas.nodes.push(Node::text("b", "b", 300.0, 0.0));
        let mut edge = Edge::new("e1", "a", None, "b", None);
        edge.style = Some(EdgeLineStyle::Dashed);
        edge.thickness = Some(EdgeThickness::Thick);
        edge.color = Some("3".into());
        canvas.edges.push(edge);
        let groups = palette_groups(&canvas, &PaletteTarget::Edge(0));
        let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, vec!["Стиль", "Толщина", "Цвет", "Поток"]);
        let style_current: Vec<bool> = groups[0].entries.iter().map(|e| e.current).collect();
        assert_eq!(style_current, vec![false, true, false], "пунктир текущий");
        let thick_current: Vec<bool> = groups[1].entries.iter().map(|e| e.current).collect();
        assert_eq!(thick_current, vec![false, false, true], "толстая текущая");
        let color_current: Vec<bool> = groups[2].entries.iter().map(|e| e.current).collect();
        assert_eq!(
            color_current,
            vec![false, false, true, false, false, false, false]
        );
    }

    /// Геометрия бара: размер по числу групп, origin под якорем по центру,
    /// кламп к краям окна.
    #[test]
    fn bar_size_and_origin() {
        let groups = vec![PaletteGroup {
            label: "A".into(),
            icon: PaletteIcon::Sliders,
            entries: vec![PaletteEntry {
                action: PaletteAction::NodeGroup(0),
                label: "x".into(),
                icon: None,
                current: false,
            }],
        }];
        let [w, h] = palette_bar_size(&groups);
        assert_eq!(
            w,
            PAL_BAR_PAD * 2.0 + PAL_BUTTON,
            "одна группа — без зазоров"
        );
        assert_eq!(h, PAL_BAR_PAD * 2.0 + PAL_BUTTON + PAL_CAPTION_H);

        let viewport = [800.0, 600.0];
        let origin = palette_origin([400.0, 100.0], [w, h], viewport);
        assert!((origin[0] + w / 2.0 - 400.0).abs() < 1e-4, "центр по x");
        assert_eq!(origin[1], 100.0 + PAL_ANCHOR_GAP, "ниже якоря");
        // Клампы: якорь у правого края — бар внутри окна
        let origin = palette_origin([900.0, 100.0], [w, h], viewport);
        assert_eq!(origin[0], viewport[0] - w - PAL_MARGIN);
        // Якорь у нижнего края — бар поднят
        let origin = palette_origin([400.0, 620.0], [w, h], viewport);
        assert_eq!(origin[1], viewport[1] - h - PAL_MARGIN);
    }

    /// Layout: кнопки групп в баре, колонки выпадашек под баром; строки
    /// внутри колонки; hit-test: триггеры, строки раскрытой группы, бар.
    #[test]
    fn layout_hit_and_triggers() {
        let canvas = text_scene();
        let groups = palette_groups(
            &canvas,
            &PaletteTarget::Nodes {
                primary: 0,
                selected: vec![0],
            },
        );
        let viewport = [1200.0, 800.0];
        let origin = palette_origin([600.0, 300.0], palette_bar_size(&groups), viewport);
        let lay = palette_layout(origin, &groups, viewport);
        assert_eq!(lay.groups.len(), groups.len());
        // Кнопка 2-й группы правее 1-й на BUTTON + GAP
        assert!(
            (lay.groups[1].button[0] - lay.groups[0].button[0] - PAL_BUTTON - PAL_GAP).abs() < 1e-4
        );
        // Колонка под баром
        assert!(lay.groups[0].dropdown[1] > lay.bar[1] + lay.bar[3]);
        for row in &lay.groups[0].rows {
            assert!(row[0] >= lay.groups[0].dropdown[0]);
            assert!(row[1] >= lay.groups[0].dropdown[1]);
        }
        // Триггер — ТОЛЬКО кнопка; вне палитры — None
        let center = [
            lay.groups[0].button[0] + PAL_BUTTON / 2.0,
            lay.groups[0].button[1] + PAL_BUTTON / 2.0,
        ];
        assert_eq!(palette_trigger_at(&lay, center), Some(0));
        assert_eq!(palette_trigger_at(&lay, [1000.0, 700.0]), None);
        // БАГ-РЕГРЕССИЯ владельца: пустая область колонки ЗАКРЫТОЙ группы
        // (где выпадашка ПОЯВИЛАСЬ БЫ) не открывает и не ловит клик
        let row = lay.groups[0].rows[0];
        let ghost = [row[0] + 2.0, row[1] + 2.0];
        assert_eq!(palette_trigger_at(&lay, ghost), None);
        assert_eq!(palette_hit(&lay, ghost, None), None);
        // Hit-test: строка выпадашки РАСКРЫТОЙ группы 0
        assert_eq!(
            palette_hit(&lay, ghost, Some(0)),
            Some(PaletteHit::Entry { group: 0, entry: 0 })
        );
        // Чужая раскрытая группа строк не ловит (колонки перекрываются по x)
        assert_eq!(palette_hit(&lay, ghost, Some(1)), None);
        // Клик по кнопке группы — Trigger (пин-переключение)
        assert_eq!(
            palette_hit(&lay, center, Some(0)),
            Some(PaletteHit::Trigger(0))
        );
        // Клик по бару (вне кнопок) — Bar
        assert_eq!(
            palette_hit(&lay, [origin[0] + 2.0, origin[1] + 2.0], Some(0)),
            Some(PaletteHit::Bar)
        );
        // Мимо палитры — None
        assert_eq!(palette_hit(&lay, [1100.0, 700.0], Some(0)), None);
    }

    /// Hover-автомат: открытие только от триггера, с hover-intent задержкой;
    /// пустая область колонки закрытой группы НЕ открывает (баг-репорт).
    #[test]
    fn hover_opens_only_from_trigger_with_delay() {
        let (_, lay) = laid_out_palette();
        let mut hover = PaletteHover::new();
        let t0 = Instant::now();
        let btn = [
            lay.groups[0].button[0] + PAL_BUTTON / 2.0,
            lay.groups[0].button[1] + PAL_BUTTON / 2.0,
        ];
        // Первый кадр на триггере — ещё не открыто
        assert_eq!(hover.update_at(&lay, btn, t0), None);
        assert!(hover.pending(), "ждём открытия — кадры держатся");
        // До истечения задержки — закрыто
        assert_eq!(
            hover.update_at(&lay, btn, t0 + Duration::from_millis(100)),
            None
        );
        // После PALETTE_OPEN_DELAY — раскрыто
        assert_eq!(
            hover.update_at(&lay, btn, t0 + Duration::from_millis(200)),
            Some(0)
        );
        assert!(!hover.pending(), "стабильное состояние — кадры не гоняем");
        // БАГ-РЕГРЕССИЯ владельца: пустая область колонки (где выпадашка
        // ПОЯВИЛАСЬ БЫ) при ЗАКРЫТОЙ группе не открывает НИЧЕГО — ни сразу,
        // ни спустя время, превышающее hover-intent задержку
        let ghost_row = lay.groups[1].rows[0];
        let ghost = [ghost_row[0] + 2.0, ghost_row[1] + 2.0];
        assert_eq!(palette_trigger_at(&lay, ghost), None);
        let mut fresh = PaletteHover::new();
        assert_eq!(fresh.update_at(&lay, ghost, t0), None);
        assert_eq!(
            fresh.update_at(&lay, ghost, t0 + Duration::from_secs(5)),
            None,
            "пустая область колонки не открывает выпадашку"
        );
        // Колонки перекрываются по x (ROW_W > шаг кнопок): пока открыта
        // группа 0, курсор в зоне её колонки удерживает ИМЕННО её —
        // переключения на группу 1 от «пустой» области не происходит
        hover.open = Some(0);
        assert_eq!(
            hover.update_at(&lay, ghost, t0 + Duration::from_millis(250)),
            Some(0),
            "зона удержания открытой группы не переключает группу"
        );
    }

    /// Удержание открытой зоны: колонка и бар держат открытие, уход —
    /// закрытие с отсрочкой PALETTE_CLOSE_DELAY.
    #[test]
    fn hover_stays_in_open_zone_and_closes_with_delay() {
        let (_, lay) = laid_out_palette();
        let mut hover = PaletteHover::new();
        hover.open = Some(0);
        let t0 = Instant::now();
        // Курсор в колонке раскрытой группы — держится сколько угодно
        let row = lay.groups[0].rows[0];
        let in_col = [row[0] + 2.0, row[1] + 2.0];
        assert_eq!(hover.update_at(&lay, in_col, t0), Some(0));
        assert_eq!(
            hover.update_at(&lay, in_col, t0 + Duration::from_secs(10)),
            Some(0)
        );
        // Ушёл с палитры — открытие переживает отсрочку
        let far = [1100.0, 700.0];
        assert_eq!(
            hover.update_at(&lay, far, t0 + Duration::from_secs(10)),
            Some(0)
        );
        assert!(hover.pending());
        assert_eq!(
            hover.update_at(
                &lay,
                far,
                t0 + Duration::from_secs(10) + Duration::from_millis(200)
            ),
            Some(0)
        );
        // ...и закрывается после PALETTE_CLOSE_DELAY
        assert_eq!(
            hover.update_at(
                &lay,
                far,
                t0 + Duration::from_secs(10) + Duration::from_millis(350)
            ),
            None
        );
        assert!(!hover.pending());
    }

    /// Переключение групп — с той же hover-intent задержкой (провод через
    /// соседние кнопки не мигает промежуточными колонками).
    #[test]
    fn hover_switches_groups_with_delay() {
        let (_, lay) = laid_out_palette();
        assert!(lay.groups.len() >= 2, "нужны минимум две группы");
        let mut hover = PaletteHover::new();
        hover.open = Some(0);
        let t0 = Instant::now();
        let btn1 = [
            lay.groups[1].button[0] + PAL_BUTTON / 2.0,
            lay.groups[1].button[1] + PAL_BUTTON / 2.0,
        ];
        // Сразу после входа на соседний триггер — ещё открыта прежняя
        assert_eq!(hover.update_at(&lay, btn1, t0), Some(0));
        // После задержки — переключение
        assert_eq!(
            hover.update_at(&lay, btn1, t0 + Duration::from_millis(200)),
            Some(1)
        );
    }

    /// Пин по клику (WAI-ARIA menu button): toggle_trigger открывает без
    /// задержки и закрывает повторным кликом; сброс по reset.
    #[test]
    fn toggle_trigger_pins_and_resets() {
        let (_, lay) = laid_out_palette();
        let mut hover = PaletteHover::new();
        let t0 = Instant::now();
        let btn = [
            lay.groups[0].button[0] + PAL_BUTTON / 2.0,
            lay.groups[0].button[1] + PAL_BUTTON / 2.0,
        ];
        // Клик — мгновенное открытие (без hover-задержки)
        hover.toggle_trigger(0);
        assert_eq!(hover.update_at(&lay, btn, t0), Some(0));
        // Повторный клик — закрытие
        hover.toggle_trigger(0);
        assert_eq!(hover.open, None);
        // Stale-guard: индекс вне диапазона геометрии — сброс
        hover.toggle_trigger(9);
        assert_eq!(hover.open, Some(9));
        assert_eq!(
            hover.update_at(&lay, btn, t0),
            None,
            "вне диапазона — сброс"
        );
    }

    /// Колонка, не помещающаяся под баром, раскрывается над ним.
    #[test]
    fn dropdown_flips_above_when_clipped() {
        let canvas = text_scene();
        let groups = palette_groups(
            &canvas,
            &PaletteTarget::Nodes {
                primary: 0,
                selected: vec![0],
            },
        );
        let viewport = [800.0, 400.0];
        // Бар у нижнего края
        let origin = palette_origin([400.0, 390.0], palette_bar_size(&groups), viewport);
        let lay = palette_layout(origin, &groups, viewport);
        let drop_h = lay.groups[0].dropdown[3];
        assert!(
            lay.groups[0].dropdown[1] + drop_h <= viewport[1] - PAL_MARGIN + 0.01,
            "колонка внутри окна: {:?}",
            lay.groups[0].dropdown
        );
        assert!(
            lay.groups[0].dropdown[1] < origin[1],
            "колонка раскрыта НАД баром"
        );
    }

    /// Иконки: квадовые — в границах rect; текстовые — без квадов.
    #[test]
    fn icons_bounds_and_text_glyphs() {
        let tint = [0.6, 0.66, 0.75, 1.0];
        let rect = [10.0, 10.0, 18.0, 18.0];
        let quad_icons = [
            PaletteIcon::LineSolid,
            PaletteIcon::LineDashed,
            PaletteIcon::LineDotted,
            PaletteIcon::Thin,
            PaletteIcon::Medium,
            PaletteIcon::Thick,
            PaletteIcon::TreeHorizontal,
            PaletteIcon::TreeVertical,
            PaletteIcon::Radial,
            PaletteIcon::AddChild,
            PaletteIcon::AddSibling,
            PaletteIcon::Collapse,
            PaletteIcon::Expand,
            PaletteIcon::Duplicate,
            PaletteIcon::Folder,
            PaletteIcon::GroupBox,
            PaletteIcon::Sliders,
            PaletteIcon::Swatch(Some("1")),
            PaletteIcon::Swatch(None),
        ];
        for icon in quad_icons {
            let quads = icon_quads(icon, rect, tint, &ThemeColors::dark());
            assert!(!quads.is_empty(), "{icon:?} — есть квады");
            for q in &quads {
                assert!(q.pos[0] >= rect[0] - 0.01 && q.pos[1] >= rect[1] - 0.01);
                assert!(
                    q.pos[0] + q.size[0] <= rect[0] + rect[2] + 0.01
                        && q.pos[1] + q.size[1] <= rect[1] + rect[3] + 0.01,
                    "{icon:?}: {:?} в границах",
                    q.pos
                );
            }
        }
        // Текстовые глифы
        assert_eq!(icon_text(PaletteIcon::Rename), Some("Aa"));
        assert_eq!(icon_text(PaletteIcon::Clear), Some("×"));
        assert!(icon_quads(PaletteIcon::Rename, rect, tint, &ThemeColors::dark()).is_empty());
        assert!(icon_quads(PaletteIcon::Clear, rect, tint, &ThemeColors::dark()).is_empty());
        assert_eq!(icon_text(PaletteIcon::LineSolid), None);
    }

    /// color_to_rgba: распаковка u32-пака glyphon Color.
    #[test]
    fn color_unpack() {
        let rgba = color_to_rgba(crate::Color::rgb(0xff, 0x80, 0x00));
        assert!((rgba[0] - 1.0).abs() < 1e-4);
        assert!((rgba[1] - 128.0 / 255.0).abs() < 1e-4);
        assert!((rgba[2] - 0.0).abs() < 1e-4);
        assert!((rgba[3] - 1.0).abs() < 1e-4, "альфа непрозрачна");
    }
}
