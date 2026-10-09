#![allow(dead_code)] // FR-LLM-D: hit-test + state used by future Stream D integration (worker wiring)
//! FR-LLM-D / PRD-0010 F-4 — Agent panel: чат-UI tool-calling через LLM.
//!
//! Правая боковая панель (388px, full-height, `Ctrl+I` для toggle). Юзер
//! печатает запрос («создай CAC и свяжи с LTV»), LLM вызывает MCP-инструменты
//! (`node_create_note`, `edge_create`, `graph_validate`, `graph_apply`),
//! результат показывается preview ghost-нодами → Accept/Reject (Q3).
//!
//! **Q3 (selection-aware):** `selected: Vec<usize>` — пусто = весь канвас.
//! Валидация whitelist: агент может СОЗДАВАТЬ новые ноды ИЛИ МОДИФИЦИРОВАТЬ
//! только ВЫДЕЛЕННЫЕ. Непросленные мутации — REJECT (см. [`ValidationResult`]).
//!
//! **Не вызывает LLM напрямую.** UI + state management + validation — здесь;
//! сетевой вызов (`LlmProvider::tool_calling`) пойдёт через worker (как
//! `suggest_worker`), точка интеграции — [`App::agent_send`] (`// FR-LLM-D-TODO`).
//!
//! **Mock preview:** [`App::agent_build_preview`] строит preview из текста
//! пользователя по эвристикам прототипа (`aiAgentBuildPreview`):
//! - "CAC" + "LTV" → 2 ноды + 1 ребро (CAC ↔ LTV).
//! - "воронк"/"funnel" → 3 ноды + 2 ребра (Visits → Regs → Purchases).
//! - Иначе → 2 generic-ноды + 1 ребро.
//!
//! FR-055: рендер через `KitDraw` + kit-компоненты (`panel_style`,
//! `chip_style`, `button_style`, `icon_button_style`). Никаких сырых
//! `CardInstance` — все квады/тексты идут через адаптер `KitDraw`.

// FR-LLM-D: маркер для поиска (grep) — все новые файлы/добавления помечены
// `// FR-LLM-D:` в комментариях.
// FR-LLM-D-TODO: маркер для мест, где нужна интеграция с LLM-воркером.

use super::*;
// FR-LLM-D: UiRect — геометрия kit-компонентов (panel_style/chip_style
// принимают UiRect, не сырой [f32; 4]).
use canvas_ui::geometry::{UiRect, UiVec2};
// LAY-W4: layout-примитивы кита — Column-скелет панели + Row-строки
// (design/rules/11-layouts.md LAY2/LAY4/LAY10; аудит-2026-10 §3.2).
use canvas_ui::geometry::EdgeInsets;
use canvas_ui::layout::{pad, pilot_backend, Child, Column, CrossAlign, MainAlign, Row};

/// Ширина панели (прототип строки 484: 388px, max 94vw — клампим в рендере).
pub const AGENT_PANEL_W: f32 = 388.0;
/// Минимальная ширина вьюпорта для показа панели. LAY-W1 (LAY8.2):
/// решение о показе — HideBelow в декларации реестра
/// (`ui_registry::AGENT_PANEL_DEGRADATION`, 600×240); константа —
/// именованный параметр политики, тела draw/hit консультируются с
/// реестром (`ui_registry::agent_panel_visible`), не сравнивают сами.
pub const AGENT_PANEL_MIN_VIEWPORT_W: f32 = 600.0;
/// Высота шапки (bolt + title + close). LAY-W7 (аудит layouts-2026-10 §5):
/// псевдоним `tokens::PANEL_HEADER_H_L` (44) — значение уже совпадало;
/// выразим намерение через канонический токен шкалы S3.
const HEAD_H: f32 = canvas_core::tokens::PANEL_HEADER_H_L;
/// Высота контекстной строки (3 chips). LAY-W7: значение вне шкалы S3 как
/// самостоятельная компонента — фиксируем как S3-производную: контейнер
/// чипов `CHIP_HEIGHT`(24) с вертикальным падом ≈ 32 (полоса центрирует
/// 24-px чипы в 32-px строке). Аналог `QUICK_H` для контекстной строки.
const CTX_H: f32 = 32.0;
/// Высота строки cost estimate.
const EST_H: f32 = 22.0;
/// Высота input row (textarea + send button). LAY-W7: 44 = `MIN_TOUCH_TARGET`
/// (LAY8.3 — тач-зона для input-области; поле внутри = `INPUT_FIELD_H`=30,
/// `kit::TEXT_FIELD_HEIGHT`). Контейнер строки, не само поле — значение
/// вне шкалы S3 как самостоятельная компонента, фиксируем как touch-target-
/// derivative (44 = канонический минимум LAY8.3).
const INPUT_H: f32 = 44.0;
/// Высота quick-actions (3 preset buttons). LAY-W7: контейнер чипов
/// `CHIP_HEIGHT`(24) в 36-px полосе (центрирование 24-px пилюль в 36-px
/// строке). Значение вне шкалы S3 как самостоятельная компонента —
/// фиксируем как S3-производную (chip-row container).
const QUICK_H: f32 = 36.0;
/// Внутренний отступ панели — токен SPACING_LG (design/rules 03, S1:
/// паддинг контейнеров — MD/LG).
const PAD: f32 = canvas_core::tokens::SPACING_LG;
/// Зазор между элементами в стеке — токен SPACING_SM.
const GAP: f32 = canvas_core::tokens::SPACING_SM;
/// Размер иконочных кнопок (✕/send) — константа кита (design/rules 03, S3:
/// «высоты — константы кита, не параметры вызова»).
const ICON_BTN: f32 = canvas_ui::kit::ICON_BUTTON_SIZE;
/// Высота поля ввода — константа кита TEXT_FIELD_HEIGHT (S3).
const INPUT_FIELD_H: f32 = canvas_ui::kit::TEXT_FIELD_HEIGHT;
/// Высота чипов/пилюль — константа кита CHIP_HEIGHT (S3; была 18 — вне шкалы).
const CHIP_H: f32 = canvas_ui::kit::CHIP_HEIGHT;
/// Горизонтальный пад текста чипа — 2·SPACING_SM.
const CHIP_PAD_H: f32 = canvas_core::tokens::SPACING_SM;
/// Высота кнопок preview (Accept / Reject) — внутри лога сообщений.
const PREVIEW_BTN_H: f32 = 26.0;
/// Радиус чипов quick-actions (pills).
const PILL_RADIUS: f32 = canvas_core::tokens::RADIUS_PILL;
/// Кегль текста поля ввода (тот же, что у чипов/лога — 11/12 шкала).
const INPUT_FONT: f32 = 12.0;

/// FR-LLM-D / PRD-0010 F-4: состояние агент-панели.
#[derive(Debug, Clone, Default)]
pub struct AgentState {
    /// Панель открыта (toggle `Ctrl+I`).
    pub open: bool,
    /// Журнал сообщений (user / bot / error / preview-with-actions).
    pub messages: Vec<AgentMessage>,
    /// Поле ввода (draft) — kit `TextFieldModel` (FR-058): текст/каретка/
    /// селекция — в СИМВОЛАХ (design/rules 00, П6: «каретка поля считается
    /// в символах, не в байтах» — прежний `caret: usize` был в байтах и
    /// ломался на кириллице).
    pub field: canvas_ui::kit::TextFieldModel,
    /// Поле в фокусе (UR-005): рамка accent + каретка (design/rules 04, A4:
    /// «рамка фокуса — слот accent, паттерн TextField витрины»). Прежде стиль
    /// поля был захардкожен Normal — поле всегда выглядело неактивным.
    pub input_focused: bool,
    /// Идёт LLM-запрос (блокирует input + send).
    pub busy: bool,
    /// Preview ghost-нод (Q3: confirm перед apply). `None` — preview нет.
    pub preview: Option<AgentPreview>,
    /// Cost estimate для текущего запроса (Q4 — для отображения в панели).
    pub cost_estimate: Option<f32>,
}

impl AgentState {
    /// Текст поля (для отправки/рендера).
    pub fn input_text(&self) -> &str {
        &self.field.text
    }

    /// Очистить поле (после отправки); каретка/селекция сбрасываются.
    pub fn clear_input(&mut self) {
        self.field = canvas_ui::kit::TextFieldModel::default();
    }
}

/// FR-LLM-D / PRD-0010 F-4: сообщение в журнале агент-панели.
#[derive(Debug, Clone)]
pub enum AgentMessage {
    /// Сообщение пользователя.
    User(String),
    /// Ответ бота (text + kind + tool_calls для отображения).
    Bot {
        text: String,
        kind: AgentMsgKind,
        tool_calls: Vec<ToolCallDisplay>,
    },
}

/// FR-LLM-D: вид сообщения бота (нормальный / ошибка / успех).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentMsgKind {
    #[default]
    Normal,
    Error,
    Success,
}

/// FR-LLM-D: отображаемый вызов инструмента в логе (имя + краткие аргументы
/// + статус: pending / success / error).
#[derive(Debug, Clone)]
pub struct ToolCallDisplay {
    pub tool_name: String,
    pub args_summary: String,
    pub status: ToolCallStatus,
}

/// FR-LLM-D: статус вызова инструмента (для лога).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToolCallStatus {
    #[default]
    Pending,
    Success,
    Error,
}

/// FR-LLM-D / PRD-0010 F-4: preview результат агента (ghost-ноды).
#[derive(Debug, Clone, Default)]
pub struct AgentPreview {
    /// Сгенерированные ноды (для отображения в логе и применения через
    /// `graph_apply`).
    pub nodes: Vec<AgentPreviewNode>,
    /// Рёбра: (from_idx, to_idx, label) — индексы в `nodes`.
    pub edges: Vec<(usize, usize, String)>,
}

/// FR-LLM-D: одна preview-нода (заголовок + позиция для ghost-overlay).
#[derive(Debug, Clone)]
pub struct AgentPreviewNode {
    pub title: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// FR-LLM-D / PRD-0010 F-4.8: контекст запроса (Q3 — selection-aware).
#[derive(Debug, Clone)]
pub enum AgentContext {
    /// Выделенных нод нет — контекст весь канвас.
    EntireCanvas,
    /// Выделенные ноды (агент может модифицировать ТОЛЬКО их + создавать
    /// новые; непросленные — REJECT).
    SelectedNodes(Vec<usize>),
}

/// FR-LLM-D / PRD-0010 F-4.9: результат валидации tool_calls (Q3 whitelist).
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Все ли вызовы валидны (создание новых / модификация selected).
    /// `true` по умолчанию (нет отклонённых вызовов → валидно); `false`
    /// когда `rejected` непустой (см. [`Self::default`] ниже — ручная
    /// реализация, т.к. `bool::default()` = `false`, что неверно для
    /// семантики «нет отклонений → валидно»).
    pub valid: bool,
    /// Отклонённые вызовы: (tool_name, причина). `valid=false` → непусто.
    pub rejected: Vec<(String, String)>,
}

impl Default for ValidationResult {
    /// Дефолт: `valid = true`, `rejected = []` (нет вызовов — нет проблем).
    /// Вручную: `#[derive(Default)]` дал бы `valid = false` (неверно).
    fn default() -> Self {
        Self {
            valid: true,
            rejected: Vec::new(),
        }
    }
}

/// UR-005: единая геометрия интерактивных rect'ов панели — один источник
/// для draw (`agent_panel_overlay`) и hit (`agent_panel_hit`). Урок CR-033:
/// расхождение конвенций draw/hit не переживает рефакторинги — дублирующая
/// геометрия запрещена (design/rules 00, П5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct AgentPanelLayout {
    /// Фон панели.
    pub panel: UiRect,
    /// Кнопка закрытия ✕ (ICON_BUTTON_SIZE, центр шапки).
    pub close: UiRect,
    /// Поле ввода (TEXT_FIELD_HEIGHT, центр 44-пиксельной полосы).
    pub input: UiRect,
    /// Кнопка отправки (ICON_BUTTON_SIZE, центр полосы ввода).
    pub send: UiRect,
    /// Quick-action пилюли (CHIP_HEIGHT).
    pub quick: [UiRect; 3],
}

impl AgentPanelLayout {
    /// Раскладка по rect'у панели (правый край, full-height).
    ///
    /// LAY-W4 (design/layouts-audit-2026-10.md §3.2 → §5): каркас панели —
    /// Column-скелет из пяти полос (шапка / контекст / лог-grow / input /
    /// quick) по каркасу LAY10 «Панель» (шапка → тело → футер); строки
    /// внутри полос — Row («тулбар»: ✕ прижат `MainAlign::End`,
    /// quick-пилюли — равные доли). Структура [`AgentPanelLayout`] и
    /// семантика полей НЕ изменились: геометрия по-прежнему одна на draw и
    /// hit (урок CR-033, LAY1.2). Золотые тесты `agent_panel_layout_golden_*`
    /// пинят прежнюю геометрию — нулевой сдвиг (допуск < 0.01 px).
    ///
    /// Соответствие нормативу:
    /// - LAY2: скелет выражен примитивами `Column`/`Row`/`pad`/
    ///   `Child::fixed`/`Child::flexible` — прежняя ручная арифметика
    ///   (`py + ph - QUICK_H - INPUT_H`, `(pw - PAD*2 - GAP*2)/3`) убрана;
    /// - LAY4 («лог-grow»): Column поддерживает flex-доли по высоте
    ///   (`Child.grow`, FR-062 F-14), поэтому лог сообщений — единственный
    ///   grow-заполнитель скелета (LAY4.1): `Child::flexible(.., 1.0)`
    ///   получает остаток высоты слота после фиксированных полос и зазоров —
    ///   дословный эквивалент прежнего расчёта нижних полос от нижнего края;
    ///   в строке ввода grow-заполнитель — поле, кнопка фиксирована;
    ///   quick-пилюли — три осмысленные равные доли 1:1:1;
    /// - LAY7: зазоры строк — `GAP` (SPACING_SM), пад полос — `PAD`
    ///   (SPACING_LG); `gap: 0.0` скелета — нейтральный «ритм без зазора»:
    ///   полосы примыкают (прежний ритм панели), воздух даёт центрирование
    ///   контента внутри полос.
    pub fn build(panel: UiRect) -> Self {
        // Выбор backend'а — осознанный (LAY9.1, ADR-0014 «потребитель
        // выбирает backend осознанно»): NativeBackend (`pilot_backend`)
        // считает в чистой f32-арифметике — побитовый паритет с прежней
        // ручной раскладкой. Дефолтный FlexLayoutEngine дополнительно
        // приводит локации к целой px-сетке (зеркало taffy round_layout в
        // `finish_line`) — сдвиг полос до 0.5 px, несовместимый с нулевым
        // сдвигом LAY-W4 (golden-тесты). Семантики Fit/grow у обоих
        // backend'ов совпадают (пинено юнит-тестами canvas-ui/layout.rs).
        let backend = pilot_backend();

        // Скелет: пять полос от слота панели. Ширина полос — вся панель;
        // внутренние поля полос — горизонтальный `pad` ниже (вертикальных
        // полей у панели нет — прежняя геометрия).
        let bands = Column {
            gap: 0.0,
            ..Column::default()
        }
        .lay_out_with(
            backend,
            panel,
            &[
                Child::fixed(panel.w, HEAD_H),      // шапка (bolt/title/close)
                Child::fixed(panel.w, CTX_H),       // контекстная строка (3 chips)
                Child::flexible(panel.w, 0.0, 1.0), // лог сообщений — grow (LAY4.1)
                Child::fixed(panel.w, INPUT_H),     // полоса ввода (поле + send)
                Child::fixed(panel.w, QUICK_H),     // полоса quick-actions
            ],
        );
        // bands[1] (контекст) и bands[2] (лог) — каркас для отрисовки
        // overlay; интерактивных rect'ов не дают и в структуру не входят.
        let head_band = bands[0];
        let input_band = bands[3];
        let quick_band = bands[4];

        // Пад полосы по горизонтали (LAY7: SPACING_LG — пад контейнера).
        let pad_h = |band: UiRect| {
            pad(
                band,
                EdgeInsets {
                    left: PAD,
                    right: PAD,
                    ..EdgeInsets::default()
                },
            )
        };

        // ✕ — ICON_BUTTON_SIZE: правый край шапки с падом PAD (Row{End} —
        // LAY10 «тулбар», прижатие без «распорки»), вертикально по центру
        // полосы (CrossAlign::Center).
        let close = Row {
            gap: 0.0,
            main: MainAlign::End,
            cross: CrossAlign::Center,
            ..Row::default()
        }
        .lay_out_with(
            backend,
            pad_h(head_band),
            &[Child::fixed(ICON_BTN, ICON_BTN)],
        )[0];

        // Полоса ввода: поле — TEXT_FIELD_HEIGHT, единственный grow-ребёнок
        // строки (остаток ширины после кнопки и зазора — LAY4.1); send —
        // ICON_BUTTON_SIZE; зазор GAP, оба по центру 44-пиксельной полосы.
        let input_row = Row {
            gap: GAP,
            cross: CrossAlign::Center,
            ..Row::default()
        }
        .lay_out_with(
            backend,
            pad_h(input_band),
            &[
                Child::flexible(0.0, INPUT_FIELD_H, 1.0), // поле ввода
                Child::fixed(ICON_BTN, ICON_BTN),         // кнопка отправки
            ],
        );
        let input = input_row[0];
        let send = input_row[1];

        // Quick-actions: три пилюли CHIP_HEIGHT равными долями (Row с
        // grow 1:1:1 — осмысленные доли LAY4), центр 36-пиксельной полосы.
        let quick_rects = Row {
            gap: GAP,
            cross: CrossAlign::Center,
            ..Row::default()
        }
        .lay_out_with(
            backend,
            pad_h(quick_band),
            &[
                Child::flexible(0.0, CHIP_H, 1.0),
                Child::flexible(0.0, CHIP_H, 1.0),
                Child::flexible(0.0, CHIP_H, 1.0),
            ],
        );
        let quick = [quick_rects[0], quick_rects[1], quick_rects[2]];

        Self {
            panel,
            close,
            input,
            send,
            quick,
        }
    }
}

/// LAY-W8: конвертация `UiRect` → `[x, y, w, h]` для передачи в
/// `touch_targets::touch_hit_xywh` (контейнер — панель).
fn to_xywh(r: UiRect) -> [f32; 4] {
    [r.x, r.y, r.w, r.h]
}

impl App {
    /// FR-LLM-D / PRD-0010 F-4: render agent panel overlay.
    /// Возвращает (quads, texts, icons) для screen_bands/icon_instances.
    /// LAY-W1 (LAY8.2): панель скрыта если `!state.open` ИЛИ срабатывает
    /// HideBelow реестра (`ui_registry::agent_panel_visible`) — ниже
    /// минимума 600×240 панель не рисуется ЦЕЛИКОМ (LAY8 п.4).
    pub(super) fn agent_panel_overlay(
        &self,
    ) -> (
        Vec<CardInstance>,
        Vec<OwnedScreenText>,
        Vec<canvas_render::IconInstance>,
    ) {
        let mut quads = Vec::new();
        let mut texts = Vec::new();
        let mut icons_out = Vec::new();
        if !self.agent_panel.open {
            return (quads, texts, icons_out);
        }
        let viewport = self.viewport_logical();
        // LAY-W1 (LAY8.2): показ решает политика HideBelow реестра
        // (AGENT_PANEL_DEGRADATION, 600×240) — ниже минимума панель не
        // рисуется ЦЕЛИКОМ (LAY8 п.4: промежуточных ступеней нет).
        if !ui_registry::agent_panel_visible(viewport) {
            return (quads, texts, icons_out);
        }
        let palette = self.effective_palette();
        let kit_palette = palette.kit_palette();
        // FR-LLM-D: KitDraw-адаптер (как ai_status_panel.rs/overlays.rs).
        // Все квады и тексты собираются в `d`, в конце дрейним в Vec'и.
        let mut d = crate::kit_ui::KitDraw::new();
        // FR-ICONS: активный набор (None = Glyph fallback).
        d.set_icon_set(self.icon_set_active());

        // Позиция панели: правый край, full-height. Ширина — min(388, 94% vw).
        let panel_w = AGENT_PANEL_W.min(viewport[0] * 0.94);
        let panel_x = viewport[0] - panel_w;
        let panel_y = 0.0;
        let panel_h = viewport[1];
        // UR-005: единая раскладка draw==hit (константы кита: ICON_BUTTON_SIZE /
        // TEXT_FIELD_HEIGHT / CHIP_HEIGHT — design/rules 03, S3).
        let lay = AgentPanelLayout::build(UiRect::new(panel_x, panel_y, panel_w, panel_h));
        let panel = [panel_x, panel_y, panel_w, panel_h];

        // CR-015/UR-005: TextMeasurer+FontSystem — один на кадр, до всех
        // текстовых секций (чипы, empty-state wrap, поле ввода, bubbles).
        let mut measurer = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        let sans_family = canvas_render::text::SANS_FAMILY; // «Noto Sans Display»

        // === Фон панели — kit::panel_style (FR-055: слот panel_*) ==========
        let panel_style = canvas_ui::kit::panel_style(&kit_palette);
        d.rect(
            UiRect::new(panel[0], panel[1], panel[2], panel[3]),
            panel_style.fill,
            panel_style.border,
            0.0, // без радиуса — full-height боковая панель
        );
        // Тонкая разделительная линия слева (визуальный «бортик»).
        d.rect(
            UiRect::new(panel[0], panel[1], 1.0, panel[3]),
            kit_palette.panel_border,
            [0.0; 4],
            0.0,
        );

        // === Шапка: bolt ⚡ + title + sub + close ✕ =======================
        let head_y = panel_y + 8.0;
        // FR-ICONS/UR-005: болт — SVG-иконка `zap` (глиф ⚡ отсутствовал в
        // сабсете — тофу в wasm; теперь есть и в атласе, и в сабсете).
        let bolt_rect = UiRect::new(panel_x + PAD, head_y + 2.0, 20.0, 20.0);
        d.icon(bolt_rect, "zap", "⚡", kit_palette.accent, 14.0);
        // Заголовок «AI Агент».
        d.label_left(
            UiRect::new(panel_x + PAD + 22.0, head_y + 1.0, 180.0, 14.0),
            "AI Агент",
            kit_palette.text_title,
            13.0,
        );
        // Подзаголовок (одна строка, приглушённый).
        d.label_left(
            UiRect::new(
                panel_x + PAD + 22.0,
                head_y + 16.0,
                panel_w - PAD * 2.0 - 22.0,
                12.0,
            ),
            "tool-calling · 42 MCP-инструмента · PRD-0010 F-4",
            kit_palette.text_muted,
            9.0,
        );
        // Кнопка закрытия ✕ (правый край шапки, ICON_BUTTON_SIZE).
        let close_rect = lay.close;
        let close_hovered = point_in_rect(
            [close_rect.x, close_rect.y, close_rect.w, close_rect.h],
            self.cursor,
        );
        let close_state = if close_hovered {
            canvas_ui::kit::KitState::Hovered
        } else {
            canvas_ui::kit::KitState::Normal
        };
        let close_style = canvas_ui::kit::icon_button_style(close_state, &kit_palette);
        d.control(close_rect, &close_style);
        // FR-ICONS/UR-005: ✕ — SVG-иконка `close` (глиф — фолбэк).
        d.icon(close_rect, "close", "✕", close_style.text, 13.0);

        // === Контекстная строка: 3 chips (Context / Provider / Rate) =====
        let ctx_y = panel_y + HEAD_H;
        let ctx = self.agent_selection_context();
        let ctx_label = match &ctx {
            AgentContext::EntireCanvas => "Context: entire canvas".to_owned(),
            AgentContext::SelectedNodes(ids) => {
                if ids.len() == 1 {
                    if let Some(&i) = ids.first() {
                        let title = self
                            .scene
                            .canvas
                            .nodes
                            .get(i)
                            .map(|n| {
                                n.label
                                    .clone()
                                    .or_else(|| n.title().map(|s| s.to_owned()))
                                    .unwrap_or_else(|| format!("node #{}", i))
                            })
                            .unwrap_or_else(|| "?".into());
                        format!("Selected: 1 — {}", title)
                    } else {
                        "Selected: 1".to_owned()
                    }
                } else {
                    format!("Selected: {} nodes", ids.len())
                }
            }
        };
        // UR-005 (П6): ширины чипов — по фактическому замеру шрифта, не
        // фиксированные 100px (литерал давал клип «BYOK (свой ключ)»).
        // Провайдер — короткая подпись (FR-LLM-FIX-2: полные подписи —
        // только в настройках, в чипе они раздували панель).
        let prov_label = self.ai_status_prov_label(self.settings.llm.provider_agent);
        let rate_label = format!(
            "ChatGPT: {}/80",
            self.ai_chatgpt_rate_used.saturating_sub(0)
        );
        let rate_visible =
            self.settings.llm.provider_agent == canvas_llm::LlmProviderId::ChatGptOAuth;
        let chip_y = ctx_y + (CTX_H - CHIP_H) * 0.5;
        let chip_gap = canvas_core::tokens::SPACING_S; // 6 — зазор чипов в баре (S1)
        let chip_font = 10.0;
        let mut chip_text_w =
            |s: &str| measurer.width_of(&mut fs, s, sans_family, chip_font) + CHIP_PAD_H * 2.0;
        // Rate chip (первый справа, если виден), затем provider; context —
        // остаток ширины с ellipsis (молчаливая обрезка запрещена — П6).
        let rate_w = if rate_visible {
            chip_text_w(&rate_label)
        } else {
            0.0
        };
        let prov_w = chip_text_w(&prov_label).max(52.0);
        let content_right = panel_x + panel_w - PAD;
        let rate_x = content_right - rate_w;
        let prov_x = rate_x - chip_gap - prov_w;
        let ctx_x = panel_x + PAD;
        let ctx_w = (prov_x - chip_gap - ctx_x).max(60.0);
        // Context chip (Selected → Selected-слот, иначе Normal).
        let ctx_state = if matches!(ctx, AgentContext::SelectedNodes(_)) {
            canvas_ui::kit::KitState::Selected
        } else {
            canvas_ui::kit::KitState::Normal
        };
        let ctx_chip_style = canvas_ui::kit::chip_style(ctx_state, &kit_palette);
        let ctx_rect = UiRect::new(ctx_x, chip_y, ctx_w, CHIP_H);
        d.control(ctx_rect, &ctx_chip_style);
        let ctx_shown = measurer.ellipsis(
            &mut fs,
            &ctx_label,
            sans_family,
            chip_font,
            ctx_w - CHIP_PAD_H * 2.0,
        );
        d.label_left(
            UiRect::new(ctx_x + CHIP_PAD_H, chip_y, ctx_w - CHIP_PAD_H * 2.0, CHIP_H),
            &ctx_shown,
            ctx_chip_style.text,
            chip_font,
        );
        // Provider chip (короткая подпись, замеренная ширина).
        let prov_chip_style =
            canvas_ui::kit::chip_style(canvas_ui::kit::KitState::Normal, &kit_palette);
        let prov_rect = UiRect::new(prov_x, chip_y, prov_w, CHIP_H);
        d.control(prov_rect, &prov_chip_style);
        d.label_center(prov_rect, &prov_label, prov_chip_style.text, chip_font);
        // Rate chip (только для ChatGPT OAuth).
        if rate_visible {
            let rate_chip_style =
                canvas_ui::kit::chip_style(canvas_ui::kit::KitState::Normal, &kit_palette);
            let rate_rect = UiRect::new(rate_x, chip_y, rate_w, CHIP_H);
            d.control(rate_rect, &rate_chip_style);
            d.label_center(rate_rect, &rate_label, rate_chip_style.text, chip_font);
        }

        // === Журнал сообщений (скроллится; здесь — простая простыня) ======
        let log_y = ctx_y + CTX_H;
        let log_h = panel_h - HEAD_H - CTX_H - EST_H - INPUT_H - QUICK_H - GAP;
        let log_rect = UiRect::new(panel_x + PAD, log_y, panel_w - PAD * 2.0, log_h);
        // Фон журнала (слегка тоньше основной панели — визуальное разделение).
        d.rect(
            log_rect,
            [
                kit_palette.panel_fill[0],
                kit_palette.panel_fill[1],
                kit_palette.panel_fill[2],
                kit_palette.panel_fill[3] * 0.5,
            ],
            [0.0; 4],
            canvas_core::tokens::RADIUS_PANEL,
        );
        // Empty state — подсказка пользователю что делать.
        let mut msg_y = log_y + 8.0;
        // CR-015 fix (Task Q1+Q2): TextMeasurer для реального шейпинга текста
        // bubble (вместо эвристик `text.len() as f32 * 6.0` / `text.len() /
        // 32.0` / `text.len() / 48` — те ломаются на Cyrillic/emoji: byte_count
        // ≠ glyph_count). Measurer+FontSystem создан ОДИН раз выше (UR-005:
        // чипы/empty-state/полё ввода/bubbles — один пул на кадр). Семейство/
        // кегль — те же, что у `d.label_left` рендера bubble (FR-053: метрики
        // раскладки = метрики рендера). Контракт «вложенный лок FontSystem
        // запрещён» (app.rs:209): `agent_panel_overlay` не вызывается внутри
        // другого FontSystem-лока (handler.rs —顶层 render path, без
        // шейпинг-локов рядом).
        let bubble_family = sans_family; // «Noto Sans Display»
        let bubble_size = 11.0; // bubble text font_size (matches d.label_left)
        if self.agent_panel.messages.is_empty() {
            // UR-005: подсказка переносится по реальной ширине (TextMeasurer).
            // Прежде — одна строка `label_left` с молчаливой обрезкой справа
            // (скриншот владельца: «…агент вызовет M» — хвост текста терялся).
            let hint = "Панель агента пуста. Опишите задачу текстом — агент \
                        вызовет MCP-инструменты (node_create, graph_validate, \
                        graph_apply…), покажет preview ghost-нодами на канвасе \
                        и спросит Accept/Reject. Деструктивные операции — только \
                        с подтверждением.";
            let hint_area_w = log_rect.w - 16.0;
            let hint_lines = measurer.wrap(&mut fs, hint, bubble_family, bubble_size, hint_area_w);
            let line_h = 15.0; // bubble_size 11 · 1.35 → 15 (ритм лога)
            let mut line_y = msg_y;
            for line in &hint_lines {
                d.label_left(
                    UiRect::new(log_rect.x + 8.0, line_y, hint_area_w, line_h),
                    line,
                    kit_palette.text_muted,
                    bubble_size,
                );
                line_y += line_h;
            }
        } else {
            // Каждое сообщение — user (справа) / bot (слева) / error.
            for msg in &self.agent_panel.messages {
                match msg {
                    AgentMessage::User(text) => {
                        // User bubble — выравнивание справа, акцентный фон.
                        // CR-015 fix (Task Q1): real glyph shaping via
                        // TextMeasurer (was `text.len() as f32 * 6.0` and
                        // `(text.len() as f32 / 32.0).ceil()` heuristics —
                        // break on Cyrillic/emoji: byte_count ≠ glyph_count,
                        // multi-byte UTF-8 over-estimates width and line
                        // count). Семейство/кегль — те же, что у `d.label_left`
                        // bubble text (FR-053: метрики раскладки = метрики
                        // рендера, cosmic-text шейпинг).
                        let max_bubble_w = log_rect.w - 16.0;
                        let text_w = measurer.width_of(&mut fs, text, bubble_family, bubble_size);
                        // +16 = 2·SPACING_SM (внутренний пад bubble).
                        let bubble_w = max_bubble_w.min(text_w + 16.0);
                        let bubble_x = log_rect.x + log_rect.w - 8.0 - bubble_w;
                        // Wrap по реальной ширине строки внутри bubble
                        // (bubble_w − 2·pad = bubble_w − 16). 16.0 = line-h
                        // для кегля 11 (font_size · SCREEN_LINE_FACTOR 1.3 ≈
                        // 14.3 → 14, но исторический user-bubble line-h был 16;
                        // preserved чтобы не менять вертикальный ритм
                        // user-bubble'ов, не использующих kit).
                        let user_lines = measurer.wrap(
                            &mut fs,
                            text,
                            bubble_family,
                            bubble_size,
                            bubble_w - 16.0,
                        );
                        let bubble_h = 28.0_f32.max(user_lines.len() as f32 * 16.0 + 12.0);
                        let user_style = canvas_ui::kit::control_style_of(
                            [
                                kit_palette.accent[0],
                                kit_palette.accent[1],
                                kit_palette.accent[2],
                                0.12,
                            ],
                            [
                                kit_palette.accent[0],
                                kit_palette.accent[1],
                                kit_palette.accent[2],
                                0.30,
                            ],
                            kit_palette.text,
                            canvas_core::tokens::RADIUS_PANEL,
                        );
                        let bubble_rect = UiRect::new(bubble_x, msg_y, bubble_w, bubble_h);
                        d.control(bubble_rect, &user_style);
                        d.label_left(
                            UiRect::new(
                                bubble_x + 8.0,
                                msg_y + 4.0,
                                bubble_w - 16.0,
                                bubble_h - 8.0,
                            ),
                            text,
                            user_style.text,
                            11.0,
                        );
                        msg_y += bubble_h + 6.0;
                    }
                    AgentMessage::Bot {
                        text,
                        kind,
                        tool_calls,
                    } => {
                        // Bot bubble — слева, нейтральный фон.
                        let bubble_w = log_rect.w - 16.0;
                        // CR-015 fix (Task Q2): real n_lines via
                        // TextMeasurer.wrap (was `(text.len() / 48).max(1)`
                        // heuristic — byte_count / 48 ≈ glyph-agnostic line
                        // count; breaks on Cyrillic/emoji: multi-byte UTF-8
                        // over-counts lines for short unicode text). Wrap to
                        // kit's text_area.w (= bubble_w − 2·SPACING_SM).
                        let max_text_w = bubble_w - 16.0; // = kit text_area.w
                        let bot_lines =
                            measurer.wrap(&mut fs, text, bubble_family, bubble_size, max_text_w);
                        let n_lines = bot_lines.len().max(1);
                        // Accept/Reject buttons если есть preview.
                        let has_preview =
                            self.agent_panel.preview.is_some() && *kind == AgentMsgKind::Normal;
                        // Q2 (Task Q): bubble_h считается из kit-канонических
                        // метрик (CHAT_HEADER_H/CHAT_LINE_H/CHAT_TOOL_CALL_H +
                        // SPACING_S/SPACING_SM), не из магических 24/14. Было:
                        // `24 + n_lines*14 + tool_calls*14 + (preview? 26+8)`.
                        // Стало: pad_half + header + text + (gap+tool_calls)?
                        // + (gap+preview_btn)? + pad_half. Визуальный delta
                        // без preview: -4..+2 px (tighter bubble — меньше
                        // пустого пространства); с preview: -6 px.
                        let pad_half = canvas_core::tokens::SPACING_SM * 0.5; // 4
                        let header_h = canvas_ui::kit::CHAT_HEADER_H; // 12
                        let text_h = (n_lines as f32) * canvas_ui::kit::CHAT_LINE_H; // n_lines·14
                        let mut bubble_h = pad_half + header_h + text_h;
                        if !tool_calls.is_empty() {
                            bubble_h += canvas_core::tokens::SPACING_S
                                + (tool_calls.len() as f32) * canvas_ui::kit::CHAT_TOOL_CALL_H;
                        }
                        if has_preview {
                            bubble_h += canvas_core::tokens::SPACING_S + PREVIEW_BTN_H;
                        }
                        bubble_h += pad_half; // bottom pad
                                              // Q2 (Task Q): migrate bubble geometry to kit::chat_bubble
                                              // layout (was style-only — `_cb_layout` discarded,
                                              // geometry hand-rolled). Kit extended с `header: bool`
                                              // param + `ChatBubbleLayout.header_area: Option<UiRect>`
                                              // (crates/canvas-ui/src/component/chat_bubble.rs) —
                                              // header_area занимает верх bubble (подпись отправителя
                                              // «AI Агент»), text_area начинается ниже header'а.
                                              // Теперь ИСПОЛЬЗУЕМ layout: `cb_layout.rect` для фона,
                                              // `cb_layout.header_area` для «AI Агент» header,
                                              // `cb_layout.text_area` для text body,
                                              // `cb_layout.tool_call_rows[i]` для tool_call строк.
                        let kind_kit = match kind {
                            AgentMsgKind::Normal => canvas_ui::kit::ChatBubbleKind::Normal,
                            AgentMsgKind::Error => canvas_ui::kit::ChatBubbleKind::Error,
                            AgentMsgKind::Success => canvas_ui::kit::ChatBubbleKind::Success,
                        };
                        let bubble_slot = UiRect::new(log_rect.x + 8.0, msg_y, bubble_w, bubble_h);
                        let (cb_layout, cb_style) = canvas_ui::kit::chat_bubble(
                            bubble_slot,
                            n_lines,
                            tool_calls.len(),
                            kind_kit,
                            &kit_palette,
                            true, // header = true («AI Агент» подпись над text)
                        );
                        let bot_style = canvas_ui::kit::control_style_of(
                            cb_style.fill,
                            cb_style.border,
                            cb_style.text_color,
                            cb_style.radius,
                        );
                        // Bubble background — kit layout.rect (весь слот).
                        d.control(cb_layout.rect, &bot_style);
                        // «AI Агент» header — kit layout.header_area.
                        if let Some(header_area) = &cb_layout.header_area {
                            d.label_left(*header_area, "AI Агент", kit_palette.text_muted, 9.0);
                        }
                        // Text body — kit layout.text_area.
                        d.label_left(cb_layout.text_area, text, cb_style.text_color, 11.0);
                        // tool_calls — моноширинные строки, по kit
                        // layout.tool_call_rows[i] rect'ам.
                        for (i, tc) in tool_calls.iter().enumerate() {
                            let status_glyph = match tc.status {
                                ToolCallStatus::Pending => "…",
                                ToolCallStatus::Success => "✓",
                                ToolCallStatus::Error => "✗",
                            };
                            let success_color: [f32; 4] = kit_palette.control_success;
                            let danger_color = kit_palette.control_danger;
                            let status_color = match tc.status {
                                ToolCallStatus::Success => success_color,
                                ToolCallStatus::Error => danger_color,
                                ToolCallStatus::Pending => kit_palette.text_muted,
                            };
                            let tc_rect = cb_layout.tool_call_rows[i];
                            d.label_left(
                                tc_rect,
                                &format!("{} {}  {}", status_glyph, tc.tool_name, tc.args_summary),
                                kit_palette.text_muted,
                                10.0,
                            );
                            // Status glyph — отдельным цветом (поверх серого).
                            d.label_left(
                                UiRect::new(tc_rect.x, tc_rect.y, 14.0, 14.0),
                                status_glyph,
                                status_color,
                                10.0,
                            );
                        }
                        // Accept/Reject кнопки — если есть preview. Позиция:
                        // ниже последнего tool_call_row (или text_area, если
                        // tool_calls нет) с зазором SPACING_S (kit canonical).
                        if has_preview {
                            let buttons_y = if let Some(last_tc) = cb_layout.tool_call_rows.last() {
                                last_tc.bottom() + canvas_core::tokens::SPACING_S
                            } else {
                                cb_layout.text_area.bottom() + canvas_core::tokens::SPACING_S
                            };
                            let btn_w = (cb_layout.rect.w - 8.0) / 2.0;
                            let accept_rect = UiRect::new(
                                cb_layout.rect.x + canvas_core::tokens::SPACING_SM,
                                buttons_y,
                                btn_w,
                                PREVIEW_BTN_H,
                            );
                            let reject_rect = UiRect::new(
                                cb_layout.rect.x + canvas_core::tokens::SPACING_SM + btn_w + 8.0,
                                buttons_y,
                                btn_w,
                                PREVIEW_BTN_H,
                            );
                            // Accept — Success-вариант button_style.
                            let accept_style = canvas_ui::kit::button_style(
                                canvas_ui::kit::ButtonVariant::Primary,
                                if point_in_rect(
                                    [accept_rect.x, accept_rect.y, accept_rect.w, accept_rect.h],
                                    self.cursor,
                                ) {
                                    canvas_ui::kit::KitState::Hovered
                                } else {
                                    canvas_ui::kit::KitState::Normal
                                },
                                &kit_palette,
                            );
                            d.control(accept_rect, &accept_style);
                            d.label_center(accept_rect, "Accept", accept_style.text, 11.0);
                            // Reject — Secondary-вариант.
                            let reject_style = canvas_ui::kit::button_style(
                                canvas_ui::kit::ButtonVariant::Secondary,
                                if point_in_rect(
                                    [reject_rect.x, reject_rect.y, reject_rect.w, reject_rect.h],
                                    self.cursor,
                                ) {
                                    canvas_ui::kit::KitState::Hovered
                                } else {
                                    canvas_ui::kit::KitState::Normal
                                },
                                &kit_palette,
                            );
                            d.control(reject_rect, &reject_style);
                            d.label_center(reject_rect, "Reject", reject_style.text, 11.0);
                        }
                        // Advance msg_y past bubble + gap (SPACING_SM = 8 px —
                        // исторический зазор между сообщениями, preserved).
                        msg_y = cb_layout.rect.bottom() + canvas_core::tokens::SPACING_SM;
                    }
                }
                // Защита от переполнения журнала (последние N сообщений).
                if msg_y > log_y + log_h {
                    break;
                }
            }
        }

        // === Cost estimate (одна строка моноширинно) =====================
        let est_y = panel_y + panel_h - QUICK_H - INPUT_H - EST_H;
        let est_str = match self.agent_panel.cost_estimate {
            Some(c) => format!(
                "Запрос ~${:.2} · день ${:.2} / ${:.2}",
                c, self.ai_cost_day, self.settings.llm.cost_limit_daily
            ),
            None => format!(
                "Запрос ~$0.02 · день ${:.2} / ${:.2}",
                self.ai_cost_day, self.settings.llm.cost_limit_daily
            ),
        };
        d.label_left(
            UiRect::new(panel_x + PAD, est_y, panel_w - PAD * 2.0, EST_H),
            &est_str,
            kit_palette.text_muted,
            10.0,
        );

        // === Input area (textarea + send button) =========================
        // UR-005: раскладка — из единого AgentPanelLayout (draw == hit);
        // высоты — константы кита (TEXT_FIELD_HEIGHT 30 / ICON_BUTTON_SIZE 26).
        let input_rect = lay.input;
        let send_rect = lay.send;
        // Фокус (design/rules 04, A4: рамка фокуса — слот accent, паттерн
        // TextField витрины). busy → Disabled (текст приглушён, каретки нет).
        let input_focused = self.agent_panel.input_focused && !self.agent_panel.busy;
        // Контейнер поля — сырой CardInstance (как пилот TextField в
        // overlays.rs): params.y = 1 — фокус-кольцо 3px accent; Normal —
        // слоты control_fill/control_border (ST2), радиус RADIUS_CHIP.
        let (input_fill, input_border) = if input_focused {
            (kit_palette.control_fill, kit_palette.accent)
        } else {
            (kit_palette.control_fill, kit_palette.control_border)
        };
        d.quads.push(crate::app::band_rect_quad_pub(
            [input_rect.x, input_rect.y, input_rect.w, input_rect.h],
            input_fill,
            input_border,
            canvas_core::tokens::RADIUS_CHIP,
        ));
        // Геометрия текста — kit::text_field (FR-055/058): ellipsis,
        // text_area, caret_x по замеру префикса тем же кеглем (П6).
        let placeholder = "Опишите задачу: «создай CAC и свяжи с LTV»";
        let field_min = UiVec2::new(
            canvas_ui::kit::TEXT_FIELD_MIN_W,
            canvas_ui::kit::TEXT_FIELD_HEIGHT,
        );
        let field_max = UiVec2::new(input_rect.w, canvas_ui::kit::TEXT_FIELD_HEIGHT);
        let field_layout = canvas_ui::kit::text_field(
            input_rect,
            field_min,
            field_max,
            &self.agent_panel.field,
            placeholder,
            input_focused,
            if self.agent_panel.busy {
                canvas_ui::kit::KitState::Disabled
            } else {
                canvas_ui::kit::KitState::Normal
            },
            &kit_palette,
            &mut measurer,
            &mut fs,
            sans_family,
            INPUT_FONT,
        );
        let text_area = field_layout.text_area;
        // UR-005: выделение — подложка accent α0.25 под текстом (фон до текста;
        // границы — замер префиксов тем же кеглем, что и рендер).
        if input_focused {
            if let Some((a, b)) = self.agent_panel.field.sel {
                let chars: Vec<char> = self.agent_panel.field.text.chars().collect();
                let (start, end) = (a.min(b), a.max(b));
                if start != end {
                    let sel_prefix: String = chars[..start].iter().collect();
                    let sel_full: String = chars[..end].iter().collect();
                    let x0 = text_area.x
                        + measurer
                            .width_of(&mut fs, &sel_prefix, sans_family, INPUT_FONT)
                            .min(text_area.w);
                    let x1 = text_area.x
                        + measurer
                            .width_of(&mut fs, &sel_full, sans_family, INPUT_FONT)
                            .min(text_area.w);
                    if x1 > x0 {
                        d.rect(
                            UiRect::new(x0, text_area.y + 2.0, x1 - x0, text_area.h - 4.0),
                            [
                                kit_palette.accent[0],
                                kit_palette.accent[1],
                                kit_palette.accent[2],
                                0.25,
                            ],
                            [0.0; 4],
                            0.0,
                        );
                    }
                }
            }
        }
        // Текст поля: пустое — placeholder приглушённым (kit text_field даёт
        // ellipsis-версию в text_shown); цвет disabled при busy (ST2).
        let is_placeholder = self.agent_panel.field.text.is_empty();
        let input_color = if self.agent_panel.busy {
            kit_palette.disabled_text
        } else if is_placeholder {
            kit_palette.text_muted
        } else {
            kit_palette.text
        };
        d.label_left(text_area, &field_layout.text_shown, input_color, INPUT_FONT);
        // Каретка — 1.5px слот accent (паттерн пилота TextField; kit даёт
        // caret_x = -1 когда не в фокусе).
        if input_focused && field_layout.caret_x >= 0.0 {
            d.quads.push(crate::app::band_rect_quad_pub(
                [
                    field_layout.caret_x,
                    text_area.y + 2.0,
                    1.5,
                    text_area.h - 4.0,
                ],
                kit_palette.accent,
                [0.0; 4],
                0.0,
            ));
        }
        // Send button ➤ (ICON_BUTTON_SIZE; иконка — SVG `send`, глиф — фолбэк).
        let send_hovered = point_in_rect(
            [send_rect.x, send_rect.y, send_rect.w, send_rect.h],
            self.cursor,
        );
        let send_state = if self.agent_panel.busy {
            canvas_ui::kit::KitState::Disabled
        } else if send_hovered {
            canvas_ui::kit::KitState::Hovered
        } else {
            canvas_ui::kit::KitState::Normal
        };
        let send_style = canvas_ui::kit::button_style(
            canvas_ui::kit::ButtonVariant::Primary,
            send_state,
            &kit_palette,
        );
        d.control(send_rect, &send_style);
        d.icon(send_rect, "send", "➤", send_style.text, 14.0);

        // === Quick actions (3 preset pill-кнопки) ========================
        let quick_labels: [&str; 3] = ["CAC ↔ LTV", "Воронка из 3 нод", "Проверка графа"];
        for (i, label) in quick_labels.iter().enumerate() {
            let qrect = lay.quick[i];
            let qhovered = point_in_rect([qrect.x, qrect.y, qrect.w, qrect.h], self.cursor);
            let qstate = if qhovered {
                canvas_ui::kit::KitState::Hovered
            } else {
                canvas_ui::kit::KitState::Normal
            };
            // Pill: chip_style с радиусом RADIUS_PILL (максимальный радиус).
            let qstyle = canvas_ui::kit::chip_style(qstate, &kit_palette);
            d.rect(qrect, qstyle.fill, qstyle.border, PILL_RADIUS);
            d.label_center(qrect, label, qstyle.text, 10.0);
        }

        // FR-LLM-D: дрейн KitDraw → возвращаемые Vec'и (как ai_status_panel.rs).
        // UR-005: +icons — прежде иконки KitDraw терялись (дрейнился только
        // search_overlay) — SVG-иконки панели не доходили до рендера.
        quads = d.quads;
        texts = d
            .texts
            .into_iter()
            .map(|t| OwnedScreenText {
                text: t.text,
                origin: t.origin,
                width: t.width,
                font_size: t.font_size,
                color: t.color,
                align: t.align,
            })
            .collect();
        icons_out = d.icons;
        (quads, texts, icons_out)
    }

    /// FR-LLM-D / PRD-0010 F-4: rect панели агента (для hit-тестов и
    /// клавиатурного фокуса). `None` — панель скрыта: закрыта (`!open`) или
    /// HideBelow реестра (`ui_registry::agent_panel_visible`, LAY-W1) —
    /// панель не участвует в хитах (LAY8 п.3–4).
    pub(super) fn agent_panel_rect(&self) -> Option<[f32; 4]> {
        if !self.agent_panel.open {
            return None;
        }
        let viewport = self.viewport_logical();
        if !ui_registry::agent_panel_visible(viewport) {
            return None;
        }
        let panel_w = AGENT_PANEL_W.min(viewport[0] * 0.94);
        Some([viewport[0] - panel_w, 0.0, panel_w, viewport[1]])
    }

    /// FR-LLM-D / PRD-0010 F-4.8: контекст запроса (Q3 — selection-aware).
    /// Пустое выделение → весь канвас; иначе — список индексов выделенных нод.
    pub(super) fn agent_selection_context(&self) -> AgentContext {
        let primary = match self.selected {
            Some(Selection::Node(i)) => Some(i),
            _ => self.selected_nodes.first().copied(),
        };
        if self.selected_nodes.is_empty() && primary.is_none() {
            AgentContext::EntireCanvas
        } else {
            let mut ids: Vec<usize> = self.selected_nodes.clone();
            if let Some(p) = primary {
                if !ids.contains(&p) {
                    ids.push(p);
                }
            }
            AgentContext::SelectedNodes(ids)
        }
    }

    /// FR-LLM-D / PRD-0010 F-4.9: валидация tool_calls против selection
    /// whitelist (Q3). Разрешено: создавать новые, модифицировать selected.
    /// REJECT: modify non-selected.
    ///
    /// Это статический анализ tool_calls ДО применения. Реальная защита —
    /// `graph_apply` на серверной стороне (MCP) тоже проверяет.
    pub(super) fn agent_validate_tool_calls(
        &self,
        calls: &[canvas_llm::ToolCall],
        selected: &[usize],
    ) -> ValidationResult {
        let selected_ids: std::collections::HashSet<String> = selected
            .iter()
            .filter_map(|&i| self.scene.canvas.nodes.get(i).map(|n| n.id.clone()))
            .collect();
        let mut rejected = Vec::new();
        for call in calls {
            // REJECT-паттерны: node_edit / node_delete / node_move /
            // node_resize / node_set_color / edge_delete с id НЕ из selected.
            let modifying_id = call.name.as_str();
            let target_id: Option<&str> = match modifying_id {
                "node_edit" | "node_delete" | "node_move" | "node_resize" | "node_set_color"
                | "edge_delete" => call
                    .arguments
                    .as_object()
                    .and_then(|o| o.iter().find(|(k, _)| k == "id"))
                    .and_then(|(_, v)| v.as_string()),
                _ => None,
            };
            if let Some(id) = target_id {
                if !selected_ids.contains(id) {
                    rejected.push((
                        call.name.clone(),
                        format!(
                            "попытка модифицировать непросленную ноду/ребро «{}» (Q3 whitelist)",
                            id
                        ),
                    ));
                }
            }
        }
        ValidationResult {
            valid: rejected.is_empty(),
            rejected,
        }
    }

    /// FR-LLM-D / PRD-0010 F-4.11: построить preview из tool_calls (ghost-ноды).
    ///
    /// MOCK (без LLM): строит preview из текста пользователя по эвристикам
    /// прототипа `aiAgentBuildPreview`:
    /// - "CAC" + "LTV" → 2 ноды (CAC, LTV) + 1 ребро (LTV/CAC).
    /// - "воронк"/"funnel" → 3 ноды (Visits, Regs, Purchases) + 2 ребра.
    /// - Иначе → 2 generic-ноды + 1 ребро.
    ///
    /// FR-010 v2 layout: цепочка слева-направо от якоря (выделенной ноды
    /// или центра вьюпорта).
    pub(super) fn agent_build_preview(&self, text: &str) -> AgentPreview {
        let t = text.to_lowercase();
        let (titles, edges): (Vec<&str>, Vec<(usize, usize, &str)>) =
            if t.contains("cac") && t.contains("ltv") {
                (vec!["CAC", "LTV"], vec![(0, 1, "LTV/CAC")])
            } else if t.contains("воронк") || t.contains("funnel") {
                (
                    vec!["Визиты", "Регистрации", "Покупки"],
                    vec![(0, 1, "visits"), (1, 2, "regs")],
                )
            } else {
                (vec!["Новая нода", "Связанная нода"], vec![(0, 1, "value")])
            };
        // Layout: цепочка слева-направо от якоря (выделенная нода или центр
        // вьюпорта). Шаг — 320px (как прототип).
        let anchor = self.selected.and_then(|s| match s {
            Selection::Node(i) => self.scene.canvas.nodes.get(i),
            _ => None,
        });
        let (base_x, base_y) = if let Some(a) = anchor {
            (a.x + a.width + 90.0, a.y - 10.0)
        } else {
            // Центр вьюпорта в world-координатах = camera.position().
            let center = self.camera.position();
            (center[0] - titles.len() as f32 * 155.0, center[1] - 60.0)
        };
        let nodes = titles
            .iter()
            .enumerate()
            .map(|(i, title)| AgentPreviewNode {
                title: title.to_string(),
                x: base_x + i as f32 * 320.0,
                y: base_y,
                width: 240.0,
                height: 120.0,
            })
            .collect();
        let edges = edges
            .into_iter()
            .map(|(f, t, l)| (f, t, l.to_string()))
            .collect();
        AgentPreview { nodes, edges }
    }

    /// W2 п.2: preview из реальных tool_calls модели. Разбираются:
    /// - `graph_apply` (батч FR-033): операции `node_create_note` /
    ///   `node_create_file` / `template_instantiate` → ноды, `edge_create`
    ///   (`fromRef`/`toRef`, алиасы `from`/`to`) → рёбра по ref-адресам батча;
    /// - прямые вызовы `node_create_note` / `node_create_file` / `edge_create`
    ///   (модель может вызвать инструмент вне батча — при `tool_choice=auto`
    ///   доступны все 42 MCP-инструмента).
    ///
    /// Имена ops — по схеме инструмента `graph_apply` (tools_list) и
    /// валидатору сцены (`canvas-scene/src/mcp.rs`: «неизвестная операция»
    /// для всего, кроме `node_create_note/node_create_file/…`). Прежние
    /// `n_note`/`n_file` в промпте/парсере были дрейфом от схемы: модель,
    /// следующая JSON-схеме, давала пустой preview — «0 нод, 0 связей»
    /// (репродукция 2026-10-09). `n_note`/`n_file` оставлены алиасами.
    /// Ноды без x/y (модель не задала позицию) — цепочка от якоря (как
    /// [`Self::agent_build_preview`]). Рёбра по неразрешённым ref — пропускаются.
    pub(super) fn agent_preview_from_calls(&self, calls: &[canvas_llm::ToolCall]) -> AgentPreview {
        // Anchor-геометрия — как mock-preview.
        let anchor = self.selected.and_then(|s| match s {
            Selection::Node(i) => self.scene.canvas.nodes.get(i),
            _ => None,
        });
        let (base_x, base_y) = if let Some(a) = anchor {
            (a.x + a.width + 90.0, a.y - 10.0)
        } else {
            let center = self.camera.position();
            (center[0] - 155.0, center[1] - 60.0)
        };
        preview_from_calls(calls, base_x, base_y)
    }

    /// W2 п.2: preview из результата Graph Builder (GeneratedNode/Edge →
    /// ghost-ноды той же механики Accept/Reject).
    #[cfg(feature = "l1-llm")]
    pub(super) fn agent_preview_from_graph_output(
        &self,
        output: &canvas_graph_builder::GraphBuilderOutput,
    ) -> AgentPreview {
        let anchor = self.selected.and_then(|s| match s {
            Selection::Node(i) => self.scene.canvas.nodes.get(i),
            _ => None,
        });
        let (base_x, base_y) = if let Some(a) = anchor {
            (a.x + a.width + 90.0, a.y - 10.0)
        } else {
            let center = self.camera.position();
            (
                center[0] - output.nodes.len() as f32 * 155.0,
                center[1] - 60.0,
            )
        };
        let nodes: Vec<AgentPreviewNode> = output
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| AgentPreviewNode {
                title: n.label.clone(),
                x: base_x + i as f32 * 320.0,
                y: base_y,
                width: 240.0,
                height: 120.0,
            })
            .collect();
        let edges: Vec<(usize, usize, String)> = output
            .edges
            .iter()
            .filter_map(|e| {
                let from = output.nodes.iter().position(|n| n.id == e.from)?;
                let to = output.nodes.iter().position(|n| n.id == e.to)?;
                Some((from, to, e.label.clone().unwrap_or_default()))
            })
            .collect();
        AgentPreview { nodes, edges }
    }

    /// W2 п.2: применить preview через `mcp_dispatch("n", …)` — батч
    /// операций FR-033 («всё или ничего», один undo-шаг). Возвращает
    /// (создано нод, создано связей) или человекочитаемую ошибку.
    pub(super) fn agent_apply_preview_ops(
        &mut self,
        preview: &AgentPreview,
    ) -> Result<(usize, usize), String> {
        let mut operations: Vec<serde_json::Value> = Vec::new();
        let refs: Vec<String> = (0..preview.nodes.len()).map(|i| format!("n{i}")).collect();
        for (i, node) in preview.nodes.iter().enumerate() {
            operations.push(serde_json::json!({
                "op": "n_note",
                "ref": refs[i],
                "x": node.x,
                "y": node.y,
                "width": node.width,
                "height": node.height,
                "text": node.title,
            }));
        }
        for (from, to, label) in &preview.edges {
            let Some(f) = refs.get(*from) else { continue };
            let Some(t) = refs.get(*to) else { continue };
            let mut op = serde_json::json!({
                "op": "edge_create",
                "fromRef": f,
                "toRef": t,
            });
            if !label.is_empty() {
                op["label"] = serde_json::Value::String(label.clone());
            }
            operations.push(op);
        }
        if operations.is_empty() {
            return Err("нет операций для применения".into());
        }
        let result = canvas_scene::mcp_dispatch(
            &mut self.scene,
            &self.templates,
            "n",
            &serde_json::json!({ "operations": operations }),
        );
        match result {
            Ok(resp) => {
                let ok = resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                if ok {
                    Ok((preview.nodes.len(), preview.edges.len()))
                } else {
                    Err(resp
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("graph_apply: операция отклонена")
                        .to_string())
                }
            }
            Err(e) => Err(e),
        }
    }

    /// FR-LLM-D / PRD-0010 F-4: принять preview → применить через
    /// `mcp_dispatch("n")` одним undo-шагом (FR-033: батч «всё или
    /// ничего»). W2: реальный graph_apply (было — mock-сообщение).
    pub(super) fn agent_accept_preview(&mut self) {
        let Some(preview) = self.agent_panel.preview.take() else {
            return;
        };
        match self.agent_apply_preview_ops(&preview) {
            Ok((applied_nodes, applied_edges)) => {
                self.agent_panel.messages.push(AgentMessage::Bot {
                    text: format!(
                        "✓ Применено: {} {}·{} {}· graph_apply — один undo-шаг (F-4.5). \
                         Деструктивные ops по-прежнему требуют confirm.",
                        applied_nodes,
                        plural_ru(applied_nodes, "нода", "ноды", "нод"),
                        applied_edges,
                        plural_ru(applied_edges, "связь", "связи", "связей")
                    ),
                    kind: AgentMsgKind::Success,
                    tool_calls: Vec::new(),
                });
            }
            Err(e) => {
                // Батч откатен сценой (FR-033) — канвас прежний.
                self.agent_panel.messages.push(AgentMessage::Bot {
                    text: format!("✕ graph_apply отклонён: {e}. Канвас не изменён."),
                    kind: AgentMsgKind::Error,
                    tool_calls: Vec::new(),
                });
            }
        }
        self.agent_panel.cost_estimate = None;
        self.request_redraw();
    }

    /// FR-LLM-D / PRD-0010 F-4: отклонить preview.
    pub(super) fn agent_reject_preview(&mut self) {
        self.agent_panel.preview = None;
        self.agent_panel.cost_estimate = None;
        self.agent_panel.messages.push(AgentMessage::Bot {
            text: "Превью отклонено — ghost-ноды убраны, канвас не изменён. \
                   Переформулируйте запрос или уточните контекст."
                .to_owned(),
            kind: AgentMsgKind::Normal,
            tool_calls: Vec::new(),
        });
        self.request_redraw();
    }

    /// FR-LLM-D / PRD-0010 F-4: отправить пользовательский запрос агенту.
    ///
    /// Шаги:
    /// 1. Проверка AI paused / off / cost limit (защита от лишних запросов).
    /// 2. Добавить user-сообщение в журнал.
    /// 3. Контекст (selected или весь канвас) → redact если Cloud.
    /// 4. Cost estimate (для отображения).
    /// 5. Mock: построить preview из текста + добавить bot-сообщение.
    ///
    /// FR-LLM-D-TODO: реальный LLM-вызов через worker (`tool_calling`),
    /// multi-turn с `tool_results`, валидация через `agent_validate_tool_calls`.
    pub(super) fn agent_send(&mut self, text: &str) {
        let txt = text.trim();
        if txt.is_empty() || self.agent_panel.busy {
            return;
        }
        // 1. Защитные проверки (как прототип `agSend`).
        if self.ai_paused {
            self.agent_panel.messages.push(AgentMessage::Bot {
                text: "AI на паузе (⏸ в статусной панели). Нажмите ▶, чтобы возобновить."
                    .to_owned(),
                kind: AgentMsgKind::Error,
                tool_calls: Vec::new(),
            });
            self.request_redraw();
            return;
        }
        let llm = &self.settings.llm;
        if llm.provider_agent == canvas_llm::LlmProviderId::Off {
            self.agent_panel.messages.push(AgentMessage::Bot {
                text: "Провайдер Agent Panel: Off. Выберите провайдера в \
                       «Настройки AI» (9-й таб)."
                    .to_owned(),
                kind: AgentMsgKind::Error,
                tool_calls: Vec::new(),
            });
            self.request_redraw();
            return;
        }
        if self.ai_cost_day >= llm.cost_limit_daily {
            self.agent_panel.messages.push(AgentMessage::Bot {
                text: format!(
                    "⚠ Rate limit: дневной лимит cost исчерпан (${:.2} / ${:.2}). \
                     Запрос отклонён — увеличьте лимит в «Настройки AI».",
                    self.ai_cost_day, llm.cost_limit_daily
                ),
                kind: AgentMsgKind::Error,
                tool_calls: Vec::new(),
            });
            self.request_redraw();
            return;
        }

        // FR-LLM-OAUTH-APP / PRD-0010 F-5.9: построение реального провайдера
        // через фабрику (`llm_factory`) до отправки — валидация конфига
        // (пустой BYOK-ключ / невыполненный OAuth-вход ChatGPT). При
        // недоступности — сообщение в журнал и запрос не уходит; для
        // ChatGptOAuth с заданным BYOK-ключом сообщение обещает fallback на
        // BYOK (реальный вызов строит провайдер повторно с `Byok` — точка
        // интеграции ниже). Mock-флоу без фичи l1-llm не меняется.
        #[cfg(feature = "l1-llm")]
        if let Some(message) = crate::llm_factory::provider_unavailable_message(
            llm.provider_agent,
            &llm.model_agent,
            llm,
            &self.oauth_assets(),
        ) {
            self.agent_panel.messages.push(AgentMessage::Bot {
                text: message,
                kind: AgentMsgKind::Error,
                tool_calls: Vec::new(),
            });
            self.request_redraw();
            return;
        }

        // 2. User-сообщение в журнал.
        self.agent_panel
            .messages
            .push(AgentMessage::User(txt.to_owned()));
        self.agent_panel.clear_input();
        self.agent_panel.busy = true;

        // 3. Контекст (для отображения и будущего redact).
        let ctx = self.agent_selection_context();

        // W2 п.2: реальный LLM-вызов через executor (натив — worker-поток;
        // wasm — шов spawn_local, W3). Провайдер строит фабрика (валидация
        // выше); ошибка джобы → graceful fallback на mock-флоу ниже (F-5.9:
        // панель остаётся работоспособной без сети и после волны W3).
        #[cfg(feature = "l1-llm")]
        if let Some(provider) = crate::llm_factory::build_feature_provider(
            llm.provider_agent,
            &llm.model_agent,
            llm,
            &self.oauth_assets(),
        ) {
            // Контекст канваса (Q3 selection-aware) + инструменты MCP.
            let ctx_hint = match &ctx {
                AgentContext::EntireCanvas => "user selected: nothing (entire canvas)".to_owned(),
                AgentContext::SelectedNodes(ids) => format!("user selected node indexes: {ids:?}"),
            };
            // W2-фикс (репродукция 2026-10-09): промпт дрейфовал от схемы —
            // звал ops «n_note»/«n_file», а схема graph_apply и валидатор
            // сцены требуют node_create_note/node_create_file (модель,
            // следующая схеме, давала пустой preview). Канонические имена +
            // пример батча + запрет прямых вызовов/текстовых ответов.
            let system = "You are the CanvasDesk canvas agent. Build the user's scheme by \
                          calling the `graph_apply` tool — ALWAYS via one graph_apply call, \
                          never by calling node_create_note/edge_create as separate tools, \
                          never with a plain-text answer. `operations` is an array of ops \
                          with EXACT names: node_create_note {ref, x, y, text, width?, \
                          height?} | node_create_file {ref, x, y, path} | \
                          template_instantiate {ref, template, params?, x, y} | \
                          edge_create {fromRef, toRef, kind? \"value\"|\"control\"}. \
                          A `ref` names a node created earlier IN THE SAME batch; edges \
                          address those refs. Example: {\"operations\":[{\"op\":\
                          \"node_create_note\",\"ref\":\"a\",\"x\":0,\"y\":0,\"text\":\
                          \"Traffic = 10k users\"},{\"op\":\"node_create_note\",\"ref\":\
                          \"b\",\"x\":320,\"y\":0,\"text\":\"CAC = $50\"},{\"op\":\
                          \"edge_create\",\"fromRef\":\"a\",\"toRef\":\"b\"}]} Place nodes \
                          on a grid (step 320 by x, 200 by y). Keep the plan small and \
                          useful."
                .to_owned();
            let messages = vec![
                canvas_llm::Message::system(&format!("{system}\n{ctx_hint}")),
                canvas_llm::Message::user(txt),
            ];
            let tools = mcp_tools_as_tooldefs();
            self.llm_executor
                .spawn_agent(crate::llm_executor::AgentJob {
                    provider,
                    messages,
                    tools,
                    opts: canvas_llm::ToolCallingOpts::default(),
                });
            self.request_redraw();
            return;
        }
        let ctx_str = match &ctx {
            AgentContext::EntireCanvas => "весь канвас".to_owned(),
            AgentContext::SelectedNodes(ids) => {
                if ids.len() == 1 {
                    if let Some(&i) = ids.first() {
                        let title = self
                            .scene
                            .canvas
                            .nodes
                            .get(i)
                            .map(|n| {
                                n.label
                                    .clone()
                                    .or_else(|| n.title().map(|s| s.to_owned()))
                                    .unwrap_or_else(|| format!("node #{}", i))
                            })
                            .unwrap_or_else(|| "?".into());
                        format!("выбранная нода «{}»", title)
                    } else {
                        "1 выбранная нода".to_owned()
                    }
                } else {
                    format!("{} выбранных нод", ids.len())
                }
            }
        };

        // 4. Cost estimate (mock — ~$0.02 как в прототипе).
        self.agent_panel.cost_estimate = Some(0.02);

        // 5. Bot-сообщение «Планирую…» с tool_calls (mock как прототип).
        let tool_calls = vec![
            ToolCallDisplay {
                tool_name: "graph_read".to_owned(),
                args_summary: format!("{{context: {}}}", ctx_str),
                status: ToolCallStatus::Success,
            },
            ToolCallDisplay {
                tool_name: "node_create".to_owned(),
                args_summary: format!(
                    "×{}",
                    if txt.to_lowercase().contains("воронк")
                        || txt.to_lowercase().contains("funnel")
                    {
                        3
                    } else {
                        2
                    }
                ),
                status: ToolCallStatus::Success,
            },
            ToolCallDisplay {
                tool_name: "graph_validate".to_owned(),
                args_summary: "→ ok · циклов нет · юниты сходятся".to_owned(),
                status: ToolCallStatus::Success,
            },
        ];
        self.agent_panel.messages.push(AgentMessage::Bot {
            text: format!("Планирую… контекст: {} · оценка ~${:.2}", ctx_str, 0.02),
            kind: AgentMsgKind::Normal,
            tool_calls,
        });

        // 6. Mock: построить preview из текста + добавить финальное сообщение.
        let preview = self.agent_build_preview(txt);
        let n = preview.nodes.len();
        let m = preview.edges.len();
        self.agent_panel.preview = Some(preview);
        self.agent_panel.messages.push(AgentMessage::Bot {
            text: format!(
                "Предлагаю связку: {} {}, {} {} (последовательность graph_apply, \
                 один undo-шаг). Ghost-превью — на канвасе. Фактический расход: $0.021.",
                n,
                plural_ru(n, "нода", "ноды", "нод"),
                m,
                plural_ru(m, "связь", "связи", "связей")
            ),
            kind: AgentMsgKind::Normal,
            tool_calls: Vec::new(),
        });

        // FR-LLM-D-TODO: реальный LLM-вызов:
        //   let messages = [Message::system(...), Message::user(txt)];
        //   let tools = canvas_mcp::tools_list() → ToolDef[];
        //   let calls = provider.tool_calling(&messages, &tools, &opts).await?;
        //   let validation = self.agent_validate_tool_calls(&calls, &selected);
        //   if !validation.valid { self.agent_panel.messages.push(error); return; }
        //   let preview = self.agent_build_preview_from_calls(&calls);
        //   self.agent_panel.preview = Some(preview);
        //   cost += actual_cost(...); self.ai_cost_session += cost; self.ai_cost_day += cost;

        self.agent_panel.busy = false;
        // 7. ChatGPT rate counter (mock — как прототип).
        if llm.provider_agent == canvas_llm::LlmProviderId::ChatGptOAuth {
            self.ai_chatgpt_rate_used = self.ai_chatgpt_rate_used.saturating_add(1);
        }
        self.request_redraw();
    }

    /// FR-LLM-D / PRD-0010 F-4: hit-test агент-панели — определить элемент
    /// под курсором (для обработчика ввода). `None` — клик мимо панели.
    /// UR-005: геометрия — из единого [`AgentPanelLayout`] (draw == hit).
    ///
    /// LAY-W8 (FR-097): на coarse-указателе hit-зоны ✕/send/quick-пилюль/
    /// Accept-Reject дотягиваются до 44 лог. px центрированно, с клампом в
    /// `panel` — расширенная зона не выходит за панель и не перекрывает
    /// канвас. На точном указателе — rect без изменений (десктоп прежний).
    pub(crate) fn agent_panel_hit(&self, point: [f32; 2]) -> Option<AgentPanelHit> {
        use crate::touch_targets::touch_hit_xywh;
        let panel = self.agent_panel_rect()?;
        if !point_in_rect(panel, point) {
            return None;
        }
        let lay = AgentPanelLayout::build(UiRect::new(panel[0], panel[1], panel[2], panel[3]));
        // FR-097: тач-цель ≥ 44 лог. px (кламп в панель)
        // Кнопка закрытия ✕.
        if point_in_rect(touch_hit_xywh(to_xywh(lay.close), panel), point) {
            return Some(AgentPanelHit::Close);
        }
        // Input area — TEXT_FIELD_HEIGHT (30 px): на coarse дотягиваем до 44.
        if point_in_rect(touch_hit_xywh(to_xywh(lay.input), panel), point) {
            return Some(AgentPanelHit::Input);
        }
        // Send button.
        if point_in_rect(touch_hit_xywh(to_xywh(lay.send), panel), point) {
            return Some(AgentPanelHit::Send);
        }
        // Quick actions (3 preset).
        for (i, q) in lay.quick.iter().enumerate() {
            if point_in_rect(touch_hit_xywh(to_xywh(*q), panel), point) {
                return Some(AgentPanelHit::QuickAction(i));
            }
        }
        // Accept/Reject кнопки (если есть preview в последнем сообщении).
        let log_y = panel[1] + HEAD_H + CTX_H;
        let log_h = panel[3] - HEAD_H - CTX_H - EST_H - INPUT_H - QUICK_H - GAP;
        if self.agent_panel.preview.is_some() {
            // Грубая проверка — кнопки где-то в нижней половине лога.
            // FR-LLM-D-TODO: точная геометрия — через макет сообщений.
            let btn_y = log_y + log_h - PREVIEW_BTN_H - 16.0;
            let btn_w = (panel[2] - PAD * 2.0 - 8.0) / 2.0;
            let accept_rect = [panel[0] + PAD + 16.0, btn_y, btn_w, PREVIEW_BTN_H];
            let reject_rect = [
                panel[0] + PAD + 16.0 + btn_w + 8.0,
                btn_y,
                btn_w,
                PREVIEW_BTN_H,
            ];
            // FR-097: тач-цель Accept/Reject (PREVIEW_BTN_H=26) ≥ 44 (кламп
            // в панель)
            if point_in_rect(touch_hit_xywh(accept_rect, panel), point) {
                return Some(AgentPanelHit::Accept);
            }
            if point_in_rect(touch_hit_xywh(reject_rect, panel), point) {
                return Some(AgentPanelHit::Reject);
            }
        }
        // Клик мимо активных элементов (тело панели) — глотаем ввод.
        Some(AgentPanelHit::NoOp)
    }

    /// W2 п.1: диспетчер клика по агент-панели (вызов из обработчика ввода
    /// ПЕРЕД canvas-pick — панель транзиентна, как ai-status-panel).
    /// `true` — клик поглощён панелью.
    pub(super) fn agent_panel_click(&mut self, point: [f32; 2]) -> bool {
        let Some(hit) = self.agent_panel_hit(point) else {
            // UR-005: клик мимо панели — поле теряет фокус (панель остаётся
            // открытой, канвас получает свои хоткеи; возврат фокуса — клик
            // по Input).
            if self.agent_panel.input_focused {
                self.agent_panel.input_focused = false;
                self.request_redraw();
            }
            return false;
        };
        match hit {
            AgentPanelHit::Close => {
                self.agent_panel.open = false;
                self.agent_panel.input_focused = false;
                self.request_redraw();
            }
            AgentPanelHit::Input => {
                // UR-005: клик по полю — фокус (рамка accent + каретка;
                // клавиатура маршрутизируется веткой agent_panel в on_key).
                self.agent_panel.input_focused = true;
                // Волна «input-адекватность» (design/rules/09-input.md IN5):
                // клик по полю — каретка по месту клика (замер kit::text_field
                // тем же кеглем, что рендер — INPUT_FONT 12.0).
                if let Some(rect) = self.agent_panel_rect() {
                    let lay = AgentPanelLayout::build(canvas_ui::geometry::UiRect::new(
                        rect[0], rect[1], rect[2], rect[3],
                    ));
                    let input_rect = lay.input;
                    let slot = canvas_ui::geometry::UiRect::new(
                        input_rect.x,
                        input_rect.y,
                        input_rect.w,
                        input_rect.h,
                    );
                    let min = canvas_ui::geometry::UiVec2::new(
                        canvas_ui::kit::TEXT_FIELD_MIN_W,
                        canvas_ui::kit::TEXT_FIELD_HEIGHT,
                    );
                    let max = canvas_ui::geometry::UiVec2::new(
                        input_rect.w,
                        canvas_ui::kit::TEXT_FIELD_HEIGHT,
                    );
                    let kit_palette = self.effective_palette().kit_palette();
                    let mut m = canvas_ui::measure::TextMeasurer::new();
                    let mut fs = canvas_render::text::measure_font_system();
                    let lay = canvas_ui::kit::text_field(
                        slot,
                        min,
                        max,
                        &self.agent_panel.field,
                        "",
                        true,
                        canvas_ui::kit::KitState::Normal,
                        &kit_palette,
                        &mut m,
                        &mut fs,
                        canvas_render::text::SANS_FAMILY,
                        INPUT_FONT,
                    );
                    let idx = canvas_ui::kit::caret_index_at_x(
                        &self.agent_panel.field,
                        lay.text_area,
                        lay.scroll_x,
                        point[0],
                        &mut m,
                        &mut fs,
                        canvas_render::text::SANS_FAMILY,
                        INPUT_FONT,
                    );
                    self.agent_panel.field.caret = idx;
                    self.agent_panel.field.sel = None;
                }
                self.request_redraw();
            }
            AgentPanelHit::Send => {
                let text = self.agent_panel.input_text().to_owned();
                self.agent_send(&text);
            }
            AgentPanelHit::QuickAction(i) => {
                let prompts = ["CAC ↔ LTV", "Воронка из 3 нод", "Проверка графа"];
                if let Some(p) = prompts.get(i) {
                    self.agent_send(p);
                }
            }
            AgentPanelHit::Accept => self.agent_accept_preview(),
            AgentPanelHit::Reject => self.agent_reject_preview(),
            AgentPanelHit::NoOp => {}
        }
        true
    }
}

/// FR-LLM-D: русская плюрализация (как прототип `pluralRu`).
/// Возвращает один из вариантов по числу: 1 — `one`, 2-4 — `few`,
/// 5-0/11-14 — `many`. Входные строки пробрасываются по lifetime `'a` —
/// вызывающий передаёт строковые литералы (`&'static str`), lifetimes
/// согласованы.
pub(crate) fn plural_ru<'a>(n: usize, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let n10 = n % 10;
    let n100 = n % 100;
    if n10 == 1 && n100 != 11 {
        one
    } else if (2..=4).contains(&n10) && !(12..=14).contains(&n100) {
        few
    } else {
        many
    }
}

/// FR-LLM-D: расширение `JsonVal` — доступ к object-pairs (для валидации
/// tool_calls). `JsonVal` уже имеет `as_object` через pattern-match, но
/// нам нужен便捷-метод — добавим trait-extension ниже.
trait JsonValExt {
    fn as_object(&self) -> Option<&[(String, canvas_llm::JsonVal)]>;
    fn as_string(&self) -> Option<&str>;
}

impl JsonValExt for canvas_llm::JsonVal {
    fn as_object(&self) -> Option<&[(String, canvas_llm::JsonVal)]> {
        match self {
            canvas_llm::JsonVal::Object(pairs) => Some(pairs),
            _ => None,
        }
    }
    fn as_string(&self) -> Option<&str> {
        match self {
            canvas_llm::JsonVal::String(s) => Some(s),
            _ => None,
        }
    }
}

/// FR-LLM-D / PRD-0010 F-4: hit-test агент-панели — элемент под курсором.
/// Возвращается `App::agent_panel_hit` для обработчика ввода.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentPanelHit {
    /// Клик мимо активных элементов (тело панели) — ввод глотается.
    NoOp,
    /// Кнопка ✕ — закрыть панель.
    Close,
    /// Кнопка ➤ — отправить запрос.
    Send,
    /// Input area — клик фокусирует input (для печати).
    Input,
    /// Accept — применить preview.
    Accept,
    /// Reject — отклонить preview.
    Reject,
    /// Quick action №i (0..3) — preset-запрос.
    QuickAction(usize),
}

/// W2 п.2: MCP-инструменты (`canvas_mcp::tools_list()`, tools/list JSON)
/// → `Vec<ToolDef>` для `LlmProvider::tool_calling`. Схема — JsonVal
/// (from_serde), имена/описания — строки реестра.
pub(super) fn mcp_tools_as_tooldefs() -> Vec<canvas_llm::ToolDef> {
    let list = canvas_mcp::tools_list();
    let tools = list.get("tools").and_then(|v| v.as_array());
    tools
        .map(|arr| {
            arr.iter()
                .filter_map(|t| {
                    let name = t.get("name")?.as_str()?.to_string();
                    let description = t
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let input_schema = t
                        .get("inputSchema")
                        .cloned()
                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                    Some(canvas_llm::ToolDef {
                        name,
                        description,
                        input_schema: canvas_llm::JsonVal::from_serde(&input_schema),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// W2-фикс (репродукция 2026-10-09 «0 нод, 0 связей»): чистое ядро парсера
/// tool_calls → ghost-превью (без `&self` — unit-тесты без App). Имена ops —
/// канонические по схеме `graph_apply` (`node_create_note`/`node_create_file`
/// /`template_instantiate`/`edge_create`); `n_note`/`n_file` — legacy-алиасы
/// (дрейф промпта от схемы был корневой причиной пустого превью). Кроме
/// батча разбираются прямые вызовы инструментов создания (вне graph_apply).
pub(crate) fn preview_from_calls(
    calls: &[canvas_llm::ToolCall],
    base_x: f32,
    base_y: f32,
) -> AgentPreview {
    let mut nodes: Vec<AgentPreviewNode> = Vec::new();
    let mut refs: Vec<String> = Vec::new(); // ref → индекс в nodes
    let mut pending_edges: Vec<(String, String)> = Vec::new();

    // Аргументы (батч-операция ИЛИ прямой вызов) → ghost-нода + ref-имя.
    // Общие поля: x/y/width/height/ref; заголовок — title (явный, FR-072) /
    // первая строка text / path / имя шаблона; без позиции — цепочка от якоря.
    let mut push_node = |args: &serde_json::Value, refs: &mut Vec<String>| {
        let i = nodes.len();
        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| {
                args.get("text")
                    .and_then(|v| v.as_str())
                    .map(|t| t.lines().next().unwrap_or(t).to_string())
            })
            .or_else(|| args.get("path").and_then(|v| v.as_str()).map(String::from))
            .or_else(|| {
                args.get("template")
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| "Новая нода".into());
        let auto_x = base_x + i as f32 * 320.0;
        nodes.push(AgentPreviewNode {
            title,
            x: args
                .get("x")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32)
                .unwrap_or(auto_x),
            y: args
                .get("y")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32)
                .unwrap_or(base_y),
            width: args
                .get("width")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32)
                .unwrap_or(240.0),
            height: args
                .get("height")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32)
                .unwrap_or(120.0),
        });
        match args.get("ref").and_then(|v| v.as_str()) {
            Some(r) => refs.push(r.to_string()),
            None => refs.push(format!("__idx{i}")),
        }
    };
    // Аргументы ребра → (from, to): канонические fromRef/toRef + алиасы
    // from/to (схема edge_create их допускает; прямые вызовы — from/to).
    let edge_ends = |args: &serde_json::Value| -> Option<(String, String)> {
        let from = args
            .get("fromRef")
            .and_then(|v| v.as_str())
            .or_else(|| args.get("from").and_then(|v| v.as_str()))?;
        let to = args
            .get("toRef")
            .and_then(|v| v.as_str())
            .or_else(|| args.get("to").and_then(|v| v.as_str()))?;
        Some((from.to_string(), to.to_string()))
    };

    for call in calls {
        let args = call.arguments.to_serde();
        if matches!(call.name.as_str(), "graph_apply" | "n") {
            // Батч FR-033: массив operations (схема graph_apply).
            let ops = args
                .get("operations")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for op in &ops {
                match op.get("op").and_then(|v| v.as_str()).unwrap_or("") {
                    "node_create_note" | "n_note" => push_node(op, &mut refs),
                    "node_create_file" | "n_file" => push_node(op, &mut refs),
                    "template_instantiate" => push_node(op, &mut refs),
                    "edge_create" => {
                        // Адресация резолвится ПОСЛЕ прохода нод (ref может
                        // ссылаться на более ранний op).
                        if let Some((f, t)) = edge_ends(op) {
                            pending_edges.push((f, t));
                        }
                    }
                    _ => {} // edge_delete/param_set/node_move/group_create — превью не строят
                }
            }
        } else {
            // Прямой вызов инструмента (вне батча).
            match call.name.as_str() {
                "node_create_note" | "node_create_file" => push_node(&args, &mut refs),
                "edge_create" => {
                    if let Some((f, t)) = edge_ends(&args) {
                        pending_edges.push((f, t));
                    }
                }
                _ => {} // читающие/прочие инструменты — не операции превью
            }
        }
    }

    // Резолв рёбер по ref → индекс; неразрешенные (id существующих нод
    // канваса, опечатки модели) пропускаются.
    let resolve = |r: &String| -> Option<usize> { refs.iter().position(|x| x == r) };
    let mut edges: Vec<(usize, usize, String)> = Vec::new();
    for (f, t) in pending_edges.drain(..) {
        if let (Some(fi), Some(ti)) = (resolve(&f), resolve(&t)) {
            edges.push((fi, ti, String::new()));
        }
    }
    AgentPreview { nodes, edges }
}

/// W2-фикс (репродукция 2026-10-09): итоговое сообщение панели по результату
/// агент-запроса. Чистая функция (unit-тесты). Раньше пустой результат
/// отображался как «Готово: 0 нод, 0 связей» — ложный успех; теперь различаются:
/// успех (есть превью) / модель не вызвала инструменты / вызвала только
/// непревьюируемые (читающие) инструменты.
pub(crate) fn agent_result_message(
    calls: &[canvas_llm::ToolCall],
    preview: &AgentPreview,
) -> (String, AgentMsgKind) {
    let n = preview.nodes.len();
    let m = preview.edges.len();
    if n > 0 || m > 0 {
        return (
            format!(
                "Готово: {} {}, {} {} — ghost-превью на канвасе. \
                 Accept применит операции одним undo-шагом (FR-033).",
                n,
                plural_ru(n, "нода", "ноды", "нод"),
                m,
                plural_ru(m, "связь", "связи", "связей")
            ),
            AgentMsgKind::Normal,
        );
    }
    if calls.is_empty() {
        return (
            "Модель не вызвала инструменты (пустой tool_calls — обычно это \
             текстовый ответ мимо tool-calling). Переформулируйте запрос \
             конкретнее — какие ноды создать и как их связать, — либо \
             выберите модель с tool-calling в «Настройки AI» (9-й таб)."
                .to_owned(),
            AgentMsgKind::Error,
        );
    }
    // Вызовы есть, превью пустое: создающих операций нет (только читающие
    // инструменты либо рёбра по неразрешённым адресам).
    let mut counts: Vec<(String, u32)> = Vec::new();
    for call in calls {
        match counts.iter_mut().find(|(name, _)| name == &call.name) {
            Some((_, cnt)) => *cnt += 1,
            None => counts.push((call.name.clone(), 1)),
        }
    }
    let summary = counts
        .iter()
        .map(|(name, cnt)| {
            if *cnt > 1 {
                format!("{name}×{cnt}")
            } else {
                name.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    (
        format!(
            "Модель вызвала только {summary} — операций создания нод/связей \
             нет, превью строить не из чего. Попросите явно: «создай ноды … \
             и свяжи их»."
        ),
        AgentMsgKind::Error,
    )
}

/// W2-фикс (репродукция 2026-10-09): подсказка к auth-ошибкам (401/403 —
/// истёкший/неверный ключ, «API key expired»). Сырое тело API ничего не
/// говорит о починке — добавляем путь к «Настройки AI». `None` — не auth.
pub(crate) fn auth_hint(reason: &str) -> Option<String> {
    let lower = reason.to_lowercase();
    let is_auth = lower.contains("llm auth")
        || lower.contains("http 401")
        || lower.contains("http 403")
        || lower.contains("api key expired")
        || lower.contains("invalid api key");
    if is_auth {
        Some(format!(
            "{reason}\n→ Похоже, API-ключ отклонён провайдером. Обновите его: \
             «Настройки AI» (9-й таб) → строка API-ключ → вставьте актуальный \
             → «Проверить» (зелёный бейдж «ключ валиден»)."
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LAY-W7 (аудит layouts-2026-10 §5): `HEAD_H` — псевдоним
    /// `tokens::PANEL_HEADER_H_L` (44, large-вариант шапки панели S3).
    /// `INPUT_H`(44)/`CTX_H`(32)/`QUICK_H`(36) — S3-производные контейнеры
    /// (тач-зона и chip-row полосы); инвариант арифметики зафиксирован.
    #[test]
    fn lay_w7_heights_are_canonical_s3() {
        assert_eq!(HEAD_H, canvas_core::tokens::PANEL_HEADER_H_L);
        // S3-производные контейнеры — инвариант значений:
        assert_eq!(INPUT_H, 44.0); // = MIN_TOUCH_TARGET (LAY8.3)
        assert_eq!(CTX_H, 32.0); // chip-row container
        assert_eq!(QUICK_H, 36.0); // chip-row container
    }

    /// LAY-W8 (FR-097): на coarse hit-зоны ✕/send (ICON_BUTTON_SIZE 26) и
    /// quick-пилюль (CHIP_HEIGHT 24) дотягиваются до 44 центрированно, с
    /// клампом в панель. Тест верифицирует МАТЕМАТИКУ, используемую в
    /// `agent_panel_hit` (без `App`/GPU): на coarse клик 5 px вне кнопки
    /// попадает в расширенную зону, на precise — мимо.
    #[test]
    fn lay_w8_panel_layout_hit_zones_expand_on_coarse() {
        let panel = [1100.0, 0.0, 388.0, 900.0];
        let lay = AgentPanelLayout::build(UiRect::new(panel[0], panel[1], panel[2], panel[3]));
        use crate::touch_targets::touch_hit_xywh;

        // ✕ — правый верхний угол. Клик 5 px левее ✕, в шапке (панель).
        let close_xywh = to_xywh(lay.close);
        let left_of_close = [close_xywh[0] - 5.0, close_xywh[1] + close_xywh[3] / 2.0];
        // Точка внутри панели.
        assert!(point_in_rect(panel, left_of_close));

        let was = canvas_core::web_bridge::pointer_coarse();
        canvas_core::web_bridge::set_pointer_coarse(true);
        assert!(
            point_in_rect(touch_hit_xywh(close_xywh, panel), left_of_close),
            "coarse: клик 5 px левее ✕ попадает в расширенную зону"
        );
        canvas_core::web_bridge::set_pointer_coarse(false);
        assert_eq!(touch_hit_xywh(close_xywh, panel), close_xywh);
        assert!(
            !point_in_rect(touch_hit_xywh(close_xywh, panel), left_of_close),
            "precise: клик 5 px левее ✕ проходит мимо (rect без изменений)"
        );

        // Quick-пилюля: клик 5 px ниже пилюли, в полосе QUICK_H=36.
        // Пилюля CHIP_H=24 в 36-полосе → под пилюлей 6 px (центр 36/24=6).
        // Расширение +10 px → точка 5 px ниже пилюли попадает в зону.
        // Симметричный тест в середине панели (вне QUICK_H у нижнего края —
        // кламп панели может ограничить расширение; используем макет с
        // большой высотой, чтобы кламп не мешал).
        let panel_tall = [1100.0, 0.0, 388.0, 2000.0];
        let lay_tall = AgentPanelLayout::build(UiRect::new(
            panel_tall[0],
            panel_tall[1],
            panel_tall[2],
            panel_tall[3],
        ));
        let q = to_xywh(lay_tall.quick[0]);
        let below_q = [q[0] + q[2] / 2.0, q[1] + q[3] + 5.0];
        assert!(point_in_rect(panel_tall, below_q));
        canvas_core::web_bridge::set_pointer_coarse(true);
        assert!(
            point_in_rect(touch_hit_xywh(q, panel_tall), below_q),
            "coarse: клик 5 px ниже quick-пилюли попадает в расширенную зону"
        );
        // Расширение по высоте произошло (height > исходной CHIP_H).
        let expanded = touch_hit_xywh(q, panel_tall);
        assert!(
            expanded[3] > q[3],
            "coarse: расширение по высоте (>{}), expanded={expanded:?}",
            q[3]
        );
        canvas_core::web_bridge::set_pointer_coarse(false);
        assert!(
            !point_in_rect(touch_hit_xywh(q, panel_tall), below_q),
            "precise: клик 5 px ниже quick-пилюли проходит мимо"
        );
        canvas_core::web_bridge::set_pointer_coarse(was);
    }

    /// Панель скрыта если `!state.open`.
    #[test]
    fn closed_state_no_rect() {
        let state = AgentState::default();
        assert!(!state.open);
    }

    /// Открытая панель — `open: true`.
    #[test]
    fn open_state_flag() {
        let state = AgentState {
            open: true,
            ..AgentState::default()
        };
        assert!(state.open);
    }

    /// Плюрализация RU — 1/2/5 нод.
    #[test]
    fn plural_ru_noda_nody_nod() {
        assert_eq!(plural_ru(1, "нода", "ноды", "нод"), "нода");
        assert_eq!(plural_ru(2, "нода", "ноды", "нод"), "ноды");
        assert_eq!(plural_ru(3, "нода", "ноды", "нод"), "ноды");
        assert_eq!(plural_ru(4, "нода", "ноды", "нод"), "ноды");
        assert_eq!(plural_ru(5, "нода", "ноды", "нод"), "нод");
        assert_eq!(plural_ru(11, "нода", "ноды", "нод"), "нод");
        assert_eq!(plural_ru(21, "нода", "ноды", "нод"), "нода");
        assert_eq!(plural_ru(22, "нода", "ноды", "нод"), "ноды");
        assert_eq!(plural_ru(25, "нода", "ноды", "нод"), "нод");
    }

    /// Плюрализация RU — связь/связи/связей.
    #[test]
    fn plural_ru_svyaz() {
        assert_eq!(plural_ru(1, "связь", "связи", "связей"), "связь");
        assert_eq!(plural_ru(2, "связь", "связи", "связей"), "связи");
        assert_eq!(plural_ru(5, "связь", "связи", "связей"), "связей");
    }

    /// Ширина панели — 388px (как прототип строки 484).
    #[test]
    fn panel_width_388() {
        assert_eq!(AGENT_PANEL_W, 388.0);
    }

    /// Минимальный вьюпорт — 600px (панель прячется на узких окнах).
    #[test]
    fn min_viewport_w_is_600() {
        assert_eq!(AGENT_PANEL_MIN_VIEWPORT_W, 600.0);
    }

    /// FR-LLM-D: AgentState default — закрытая панель, без сообщений, не занят.
    /// UR-005: поле — kit TextFieldModel (пусто, caret 0, без селекции),
    /// фокуса нет (панель закрыта).
    #[test]
    fn agent_state_default() {
        let s = AgentState::default();
        assert!(!s.open);
        assert!(s.messages.is_empty());
        assert!(s.input_text().is_empty());
        assert_eq!(s.field.caret, 0);
        assert!(s.field.sel.is_none());
        assert!(!s.input_focused);
        assert!(!s.busy);
        assert!(s.preview.is_none());
        assert!(s.cost_estimate.is_none());
    }

    /// UR-005: clear_input сбрасывает текст/каретку/селекцию (после отправки).
    #[test]
    fn agent_state_clear_input_resets_field() {
        let mut s = AgentState::default();
        s.field.set_text("привет мир".to_owned());
        s.field.move_caret(3, false);
        s.field.move_caret(-2, true); // селекция
        assert!(!s.input_text().is_empty());
        assert!(s.field.sel.is_some());
        s.clear_input();
        assert!(s.input_text().is_empty());
        assert_eq!(s.field.caret, 0);
        assert!(s.field.sel.is_none());
    }

    /// UR-005: каретка/селекция поля — в СИМВОЛАХ, не в байтах
    /// (design/rules 00 П6; прежний caret был в байтах и ломался на кириллице).
    #[test]
    fn agent_field_caret_counts_chars_not_bytes() {
        let mut s = AgentState::default();
        // «воронка» — 7 символов = 14 байт UTF-8.
        s.field.set_text("воронка".to_owned());
        assert_eq!(s.field.text.len(), 14);
        assert_eq!(s.field.caret, 7);
        s.field.move_caret(-3, false);
        assert_eq!(s.field.caret, 4);
        // Вставка в середину кириллицы.
        s.field.insert("X");
        assert_eq!(s.field.text, "вороXнка");
        assert_eq!(s.field.caret, 5);
    }

    /// UR-005: выделение — shift-стрелки расширяют, вставка/удаление
    /// замещают выделенный диапазон (kit TextFieldModel FR-058).
    #[test]
    fn agent_field_selection_extend_and_replace() {
        let mut s = AgentState::default();
        s.field.set_text("абвгд".to_owned());
        s.field.move_caret(-2, true); // Shift+Left ×1
                                      // sel = (anchor, head): anchor — исходная каретка (5), head — новая (3).
        assert_eq!(s.field.sel, Some((5, 3)));
        s.field.move_caret(-1, true); // ещё Shift+Left → head 2
        assert_eq!(s.field.sel, Some((5, 2)));
        // Вставка замещает выделенный диапазон [2..5) = «вгд».
        s.field.insert("Z");
        assert_eq!(s.field.text, "абZ");
        assert!(s.field.sel.is_none());
        // Backspace без выделения удаляет символ перед кареткой;
        // с выделением — весь диапазон.
        s.field.set_text("пробелы и выделение".to_owned());
        s.field.select_all();
        s.field.backspace();
        assert!(s.field.text.is_empty());
    }

    /// UR-005: Space вставляет пробел (регрессия: winit отдаёт Space как
    /// Named(NamedKey::Space) — ветка Character его не ловила, пробелы
    /// не вводились). Вставка идёт через model.insert — в позицию каретки.
    #[test]
    fn agent_field_space_insert() {
        let mut s = AgentState::default();
        s.field.set_text("создай".to_owned());
        s.field.insert(" ");
        s.field.insert("CAC");
        assert_eq!(s.input_text(), "создай CAC");
    }

    /// FR-LLM-D: AgentPreview default — пустой.
    #[test]
    fn agent_preview_default_empty() {
        let p = AgentPreview::default();
        assert!(p.nodes.is_empty());
        assert!(p.edges.is_empty());
    }

    /// FR-LLM-D: ValidationResult default — валидный, без отклонений.
    #[test]
    fn validation_result_default_valid() {
        let v = ValidationResult::default();
        assert!(v.valid);
        assert!(v.rejected.is_empty());
    }

    /// FR-LLM-D: AgentMsgKind default — Normal.
    #[test]
    fn agent_msg_kind_default_normal() {
        assert_eq!(AgentMsgKind::default(), AgentMsgKind::Normal);
    }

    /// FR-LLM-D: ToolCallStatus default — Pending.
    #[test]
    fn tool_call_status_default_pending() {
        assert_eq!(ToolCallStatus::default(), ToolCallStatus::Pending);
    }

    /// FR-LLM-D: JsonValExt as_object / as_string.
    #[test]
    fn json_val_ext_object_and_string() {
        use canvas_llm::JsonVal;
        let obj = JsonVal::Object(vec![
            ("id".into(), JsonVal::String("n1".into())),
            ("x".into(), JsonVal::Number(10.0)),
        ]);
        let pairs = obj.as_object().unwrap();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].0, "id");
        let s = pairs[0].1.as_string().unwrap();
        assert_eq!(s, "n1");
        // Не-объект → None.
        assert!(JsonVal::Number(5.0).as_object().is_none());
        // Не-строка → None.
        assert!(JsonVal::Number(5.0).as_string().is_none());
    }

    /// FR-LLM-D: AgentPanelHit variants distinct.
    #[test]
    fn agent_panel_hit_variants_distinct() {
        assert_ne!(AgentPanelHit::Close, AgentPanelHit::Send);
        assert_ne!(AgentPanelHit::Accept, AgentPanelHit::Reject);
        assert_ne!(AgentPanelHit::QuickAction(0), AgentPanelHit::QuickAction(1));
        assert_ne!(AgentPanelHit::NoOp, AgentPanelHit::Input);
    }

    /// FR-LLM-D: AgentContext — EntireCanvas vs SelectedNodes.
    #[test]
    fn agent_context_variants() {
        let entire = AgentContext::EntireCanvas;
        let selected = AgentContext::SelectedNodes(vec![0, 1, 2]);
        match entire {
            AgentContext::EntireCanvas => {}
            AgentContext::SelectedNodes(_) => panic!("expected EntireCanvas"),
        }
        match selected {
            AgentContext::SelectedNodes(ids) => assert_eq!(ids, vec![0, 1, 2]),
            AgentContext::EntireCanvas => panic!("expected SelectedNodes"),
        }
    }

    /// FR-LLM-D: PILL_RADIUS — из токенов (FR-055 единый источник радиусов).
    #[test]
    fn pill_radius_uses_token() {
        assert_eq!(PILL_RADIUS, canvas_core::tokens::RADIUS_PILL);
    }

    /// W2 п.2: MCP tools/list → ToolDef[] (имя `n` присутствует, схема не
    /// пуста) — конвертация без сети.
    #[test]
    fn mcp_tools_as_tooldefs_has_graph_apply() {
        let defs = mcp_tools_as_tooldefs();
        assert!(!defs.is_empty());
        assert!(defs.iter().any(|t| t.name == "graph_apply"));
        assert!(defs.iter().all(|t| !t.description.is_empty()));
        // Схема батча — объект с properties.operations.
        let n = defs.iter().find(|t| t.name == "graph_apply").unwrap();
        let schema = n.input_schema.to_serde();
        assert!(schema.get("properties").is_some());
    }

    /// Помощник: ToolCall из JSON-аргументов.
    fn tc(name: &str, args: serde_json::Value) -> canvas_llm::ToolCall {
        canvas_llm::ToolCall {
            id: format!("call_{name}"),
            name: name.to_owned(),
            arguments: canvas_llm::JsonVal::from_serde(&args),
        }
    }

    /// W2 п.2: preview из tool_calls — legacy-алиасы n_note/edge_create
    /// (совместимость со старым промптом).
    #[test]
    fn preview_from_calls_parses_batch_ops() {
        let calls = [tc(
            "n",
            serde_json::json!({
                "operations": [
                    { "op": "n_note", "ref": "a", "x": 10.0, "y": 20.0, "text": "CAC" },
                    { "op": "n_note", "ref": "b", "y": 20.0, "text": "LTV" },
                    { "op": "edge_create", "fromRef": "a", "toRef": "b" }
                ]
            }),
        )];
        let preview = preview_from_calls(&calls, 0.0, 0.0);
        assert_eq!(preview.nodes.len(), 2);
        assert_eq!(preview.nodes[0].title, "CAC");
        assert_eq!(preview.nodes[0].x, 10.0);
        // Нода без x — авто-позиция цепочкой (шаг 320).
        assert_eq!(preview.nodes[1].x, 320.0);
        assert_eq!(preview.edges, vec![(0, 1, String::new())]);
    }

    /// РЕГРЕСС «0 нод, 0 связей» (репродукция 2026-10-09): схема graph_apply
    /// (tools_list) и валидатор сцены требуют имена node_create_note — модель,
    /// следующая схеме, раньше давала пустой preview. Канонические имена
    /// обязаны парситься.
    #[test]
    fn preview_parses_schema_op_names() {
        let calls = [tc(
            "graph_apply",
            serde_json::json!({
                "operations": [
                    { "op": "node_create_note", "ref": "tam", "x": 0.0, "y": 0.0, "text": "TAM = 5M" },
                    { "op": "node_create_note", "ref": "sam", "x": 320.0, "y": 0.0, "text": "SAM = 1M" },
                    { "op": "node_create_note", "ref": "som", "x": 640.0, "y": 0.0, "text": "SOM = 50k" },
                    { "op": "edge_create", "fromRef": "tam", "toRef": "sam" },
                    { "op": "edge_create", "fromRef": "sam", "toRef": "som" }
                ]
            }),
        )];
        let preview = preview_from_calls(&calls, 0.0, 0.0);
        assert_eq!(preview.nodes.len(), 3, "канонические имена ops дают ноды");
        assert_eq!(preview.edges.len(), 2, "рёбра по ref-ам батча резолвятся");
        assert_eq!(preview.nodes[0].title, "TAM = 5M");
        assert_eq!(preview.edges[0], (0, 1, String::new()));
        assert_eq!(preview.edges[1], (1, 2, String::new()));
    }

    /// W2-фикс: прямые вызовы инструментов создания (вне graph_apply) —
    /// при tool_choice=auto модель может вызвать node_create_note напрямую;
    /// раньше такой вызов молча давал пустой preview.
    #[test]
    fn preview_parses_direct_tool_calls() {
        let calls = [
            tc(
                "node_create_note",
                serde_json::json!({ "ref": "a", "x": 5.0, "y": 6.0, "text": "Визиты" }),
            ),
            tc(
                "node_create_note",
                serde_json::json!({ "ref": "b", "x": 325.0, "y": 6.0, "text": "Регистрации", "title": "Regs" }),
            ),
            tc("edge_create", serde_json::json!({ "from": "a", "to": "b" })),
        ];
        let preview = preview_from_calls(&calls, 0.0, 0.0);
        assert_eq!(preview.nodes.len(), 2, "прямые node_create_note → ноды");
        assert_eq!(preview.nodes[1].title, "Regs", "title — явный заголовок");
        assert_eq!(preview.edges, vec![(0, 1, String::new())], "from/to алиасы");
    }

    /// W2-фикс: template_instantiate в батче → preview-нода (раньше — молча
    /// пропускалась); неразрешённые ref рёбер — пропускаются без паники.
    #[test]
    fn preview_parses_template_and_drops_unresolved_edges() {
        let calls = [tc(
            "graph_apply",
            serde_json::json!({
                "operations": [
                    { "op": "template_instantiate", "ref": "t1", "template": "ue-cac", "x": 0.0, "y": 0.0 },
                    { "op": "edge_create", "fromRef": "t1", "toRef": "missing" }
                ]
            }),
        )];
        let preview = preview_from_calls(&calls, 0.0, 0.0);
        assert_eq!(preview.nodes.len(), 1);
        assert_eq!(preview.nodes[0].title, "ue-cac");
        assert!(preview.edges.is_empty(), "неразрешённый ref отброшен");
    }

    /// W2-фикс: итоговое сообщение — успех при непустом preview.
    #[test]
    fn agent_result_message_success_with_preview() {
        let preview = AgentPreview {
            nodes: vec![
                AgentPreviewNode {
                    title: "A".into(),
                    x: 0.0,
                    y: 0.0,
                    width: 240.0,
                    height: 120.0,
                },
                AgentPreviewNode {
                    title: "B".into(),
                    x: 320.0,
                    y: 0.0,
                    width: 240.0,
                    height: 120.0,
                },
            ],
            edges: vec![(0, 1, String::new())],
        };
        let calls = [tc("graph_apply", serde_json::json!({}))];
        let (text, kind) = agent_result_message(&calls, &preview);
        assert_eq!(kind, AgentMsgKind::Normal);
        assert!(text.contains("Готово: 2 ноды, 1 связь"), "text = {text}");
    }

    /// РЕГРЕСС «Готово: 0 нод, 0 связей» (репродукция 2026-10-09): пустой
    /// tool_calls — НЕ успех, а честная ошибка с подсказкой.
    #[test]
    fn agent_result_message_empty_calls_is_error() {
        let empty = AgentPreview::default();
        let (text, kind) = agent_result_message(&[], &empty);
        assert_eq!(kind, AgentMsgKind::Error);
        assert!(text.contains("не вызвала инструменты"), "text = {text}");
        assert!(!text.contains("Готово"), "ложный успех запрещён");
    }

    /// W2-фикс: только читающие инструменты — ошибка со сводкой имён.
    #[test]
    fn agent_result_message_readonly_calls_lists_tools() {
        let calls = [
            tc("nodes_list", serde_json::json!({})),
            tc("nodes_list", serde_json::json!({})),
            tc("graph_validate", serde_json::json!({})),
        ];
        let (text, kind) = agent_result_message(&calls, &AgentPreview::default());
        assert_eq!(kind, AgentMsgKind::Error);
        assert!(text.contains("nodes_list×2"), "text = {text}");
        assert!(text.contains("graph_validate"), "text = {text}");
    }

    /// W2-фикс: auth-подсказка — 401/API key expired получают путь к
    /// «Настройки AI»; прочие ошибки — без подсказки (None).
    #[test]
    fn auth_hint_matches_401_and_expired_key() {
        let expired =
            "llm auth: HTTP 401: {\"error\":{\"message\":\"API key expired.\",\"code\":401}}";
        let hinted = auth_hint(expired).expect("401 → подсказка");
        assert!(hinted.contains(expired), "исходный текст сохранён");
        assert!(hinted.contains("Настройки AI"), "подсказка про настройки");
        assert!(auth_hint("llm transport: timeout").is_none());
        assert!(auth_hint("llm protocol: нет tool_calls").is_none());
        assert!(auth_hint("llm rate limit").is_none());
    }

    /// LAY-W4: допуск золотого теста геометрии (px). Задача требует
    /// «нулевой сдвиг»: бит-в-бит либо плавающая погрешность < 0.01 px —
    /// гейт вдвое строже (фактический дрейф рефакторинга — 0: все rect'ы
    /// бит-в-бит равны прежним, замерено пробой на 6 панелях).
    const GOLDEN_EPS: f32 = 0.005;

    /// Помощник золотого теста: каждое поле rect'а — против значения,
    /// снятого с прежней (ручной) арифметики `AgentPanelLayout::build`.
    fn assert_rect_golden(what: &str, actual: UiRect, x: f32, y: f32, w: f32, h: f32) {
        let d = [actual.x - x, actual.y - y, actual.w - w, actual.h - h];
        assert!(
            d[0].abs() <= GOLDEN_EPS
                && d[1].abs() <= GOLDEN_EPS
                && d[2].abs() <= GOLDEN_EPS
                && d[3].abs() <= GOLDEN_EPS,
            "{what}: ожидалось ({x}, {y}, {w}, {h}), получено {actual:?}, \
             дрейф (Δx, Δy, Δw, Δh) = ({:?})",
            d
        );
    }

    /// LAY-W4, золотая геометрия (draw == hit, урок CR-033): каноническая
    /// панель 388×800 (вьюпорт 1280×800). Значения сняты со СТАРОЙ
    /// реализации build() до рефакторинга на Column/Row-скелет — рефакторинг
    /// не имеет права сдвинуть ни один rect (LAY1.2: вторая геометрия для
    /// хитов запрещена; golden пинит единственную).
    #[test]
    fn agent_panel_layout_golden_canonical_388x800() {
        let lay = AgentPanelLayout::build(UiRect::new(0.0, 0.0, 388.0, 800.0));
        // Слот проходит насквозь без изменений.
        assert_eq!(lay.panel, UiRect::new(0.0, 0.0, 388.0, 800.0));
        // ✕: правый край с падом (0+388−12−26), центр шапки 44 → y = 9.
        assert_rect_golden("close", lay.close, 350.0, 9.0, 26.0, 26.0);
        // Полоса ввода: y = 800−36−44+7 = 727; w = 388−2·12−26−8 = 330.
        assert_rect_golden("input", lay.input, 12.0, 727.0, 330.0, 30.0);
        assert_rect_golden("send", lay.send, 350.0, 729.0, 26.0, 26.0);
        // quick: (388−2·12−2·8)/3 = 116; y = 800−36+6 = 770.
        assert_rect_golden("quick[0]", lay.quick[0], 12.0, 770.0, 116.0, 24.0);
        assert_rect_golden("quick[1]", lay.quick[1], 136.0, 770.0, 116.0, 24.0);
        assert_rect_golden("quick[2]", lay.quick[2], 260.0, 770.0, 116.0, 24.0);
    }

    /// LAY-W4, золотая геометрия: узкое окно ниже брейкпоинта 600
    /// (вьюпорт 400×560 → ширина панели клампится 0.94·vw = 376).
    #[test]
    fn agent_panel_layout_golden_narrow_376x560() {
        let lay = AgentPanelLayout::build(UiRect::new(24.0, 0.0, 376.0, 560.0));
        assert_eq!(lay.panel, UiRect::new(24.0, 0.0, 376.0, 560.0));
        assert_rect_golden("close", lay.close, 362.0, 9.0, 26.0, 26.0);
        assert_rect_golden("input", lay.input, 36.0, 487.0, 318.0, 30.0);
        assert_rect_golden("send", lay.send, 362.0, 489.0, 26.0, 26.0);
        assert_rect_golden("quick[0]", lay.quick[0], 36.0, 530.0, 112.0, 24.0);
        assert_rect_golden("quick[1]", lay.quick[1], 156.0, 530.0, 112.0, 24.0);
        assert_rect_golden("quick[2]", lay.quick[2], 276.0, 530.0, 112.0, 24.0);
    }

    /// LAY-W4, золотая геометрия: широкое окно 1920×1080 (панель у правого
    /// края — координаты ~1900 пинят арифметику на больших величинах).
    #[test]
    fn agent_panel_layout_golden_wide_1920x1080() {
        let lay = AgentPanelLayout::build(UiRect::new(1532.0, 0.0, 388.0, 1080.0));
        assert_eq!(lay.panel, UiRect::new(1532.0, 0.0, 388.0, 1080.0));
        assert_rect_golden("close", lay.close, 1882.0, 9.0, 26.0, 26.0);
        assert_rect_golden("input", lay.input, 1544.0, 1007.0, 330.0, 30.0);
        assert_rect_golden("send", lay.send, 1882.0, 1009.0, 26.0, 26.0);
        assert_rect_golden("quick[0]", lay.quick[0], 1544.0, 1050.0, 116.0, 24.0);
        assert_rect_golden("quick[1]", lay.quick[1], 1668.0, 1050.0, 116.0, 24.0);
        assert_rect_golden("quick[2]", lay.quick[2], 1792.0, 1050.0, 116.0, 24.0);
    }

    /// LAY-W4, золотая геометрия: дробные координаты/высота (нестепенные
    /// двойкой float'ы — пин погрешности f32: (319.6−40)/3 = 93.200005,
    /// высота 737.5 → полосы 664.5/707.5). Погрешность рефакторинга здесь
    /// максимальна и обязана остаться в пределах GOLDEN_EPS.
    #[test]
    fn agent_panel_layout_golden_fractional_319x737() {
        let lay = AgentPanelLayout::build(UiRect::new(20.4, 0.0, 319.6, 737.5));
        assert_eq!(lay.panel, UiRect::new(20.4, 0.0, 319.6, 737.5));
        assert_rect_golden("close", lay.close, 302.0, 9.0, 26.0, 26.0);
        assert_rect_golden("input", lay.input, 32.4, 664.5, 261.6, 30.0);
        assert_rect_golden("send", lay.send, 302.0, 666.5, 26.0, 26.0);
        assert_rect_golden("quick[0]", lay.quick[0], 32.4, 707.5, 93.200005, 24.0);
        assert_rect_golden("quick[1]", lay.quick[1], 133.6, 707.5, 93.200005, 24.0);
        assert_rect_golden("quick[2]", lay.quick[2], 234.80002, 707.5, 93.200005, 24.0);
    }
}
