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
use canvas_ui::geometry::UiRect;

/// Ширина панели (прототип строки 484: 388px, max 94vw — клампим в рендере).
pub const AGENT_PANEL_W: f32 = 388.0;
/// Минимальная ширина вьюпорта для показа панели (на узких окнах панель
/// прячется — как `AI_STATUS_MIN_VIEWPORT_W`).
pub const AGENT_PANEL_MIN_VIEWPORT_W: f32 = 600.0;
/// Высота шапки (bolt + title + close).
const HEAD_H: f32 = 44.0;
/// Высота контекстной строки (3 chips).
const CTX_H: f32 = 32.0;
/// Высота строки cost estimate.
const EST_H: f32 = 22.0;
/// Высота input row (textarea + send button).
const INPUT_H: f32 = 44.0;
/// Высота quick-actions (3 preset buttons).
const QUICK_H: f32 = 36.0;
/// Внутренний отступ панели.
const PAD: f32 = 12.0;
/// Зазор между элементами в стеке.
const GAP: f32 = 8.0;
/// Размер кнопки закрытия (✕).
const CLOSE_BTN_SIZE: f32 = 22.0;
/// Высота кнопок preview (Accept / Reject) — внутри лога сообщений.
const PREVIEW_BTN_H: f32 = 26.0;
/// Ширина чипов контекста (provider / rate / context).
const CHIP_W: f32 = 100.0;
/// Радиус чипов quick-actions (pills).
const PILL_RADIUS: f32 = canvas_core::tokens::RADIUS_PILL;

/// FR-LLM-D / PRD-0010 F-4: состояние агент-панели.
#[derive(Debug, Clone, Default)]
pub struct AgentState {
    /// Панель открыта (toggle `Ctrl+I`).
    pub open: bool,
    /// Журнал сообщений (user / bot / error / preview-with-actions).
    pub messages: Vec<AgentMessage>,
    /// Текст в input area (draft).
    pub input: String,
    /// Идёт LLM-запрос (блокирует input + send).
    pub busy: bool,
    /// Preview ghost-нод (Q3: confirm перед apply). `None` — preview нет.
    pub preview: Option<AgentPreview>,
    /// Cost estimate для текущего запроса (Q4 — для отображения в панели).
    pub cost_estimate: Option<f32>,
    /// Каретка в input (для будущей inline-правки).
    pub caret: usize,
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

impl App {
    /// FR-LLM-D / PRD-0010 F-4: render agent panel overlay.
    /// Возвращает (quads, texts) для screen_bands. Панель скрыта если
    /// `!state.open` или вьюпорт слишком узкий.
    pub(super) fn agent_panel_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut quads = Vec::new();
        let mut texts = Vec::new();
        if !self.agent_panel.open {
            return (quads, texts);
        }
        let viewport = self.viewport_logical();
        if viewport[0] < AGENT_PANEL_MIN_VIEWPORT_W || viewport[1] <= 0.0 {
            return (quads, texts);
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
        let panel = [panel_x, panel_y, panel_w, panel_h];

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
        // FR-ICONS: ⚡ — glyph fallback (SVG-атлас может не иметь).
        d.label_left(
            UiRect::new(panel_x + PAD, head_y + 4.0, 16.0, 20.0),
            "⚡",
            kit_palette.accent,
            14.0,
        );
        // Заголовок «AI Агент».
        d.label_left(
            UiRect::new(panel_x + PAD + 18.0, head_y + 1.0, 180.0, 14.0),
            "AI Агент",
            kit_palette.text_title,
            13.0,
        );
        // Подзаголовок (одна строка, приглушённый).
        d.label_left(
            UiRect::new(
                panel_x + PAD + 18.0,
                head_y + 16.0,
                panel_w - PAD * 2.0 - 18.0,
                12.0,
            ),
            "tool-calling · 42 MCP-инструмента · PRD-0010 F-4",
            kit_palette.text_muted,
            9.0,
        );
        // Кнопка закрытия ✕ (правый край шапки).
        let close_x = panel_x + panel_w - PAD - CLOSE_BTN_SIZE;
        let close_hovered = point_in_rect(
            [close_x, head_y + 2.0, CLOSE_BTN_SIZE, CLOSE_BTN_SIZE],
            self.cursor,
        );
        let close_state = if close_hovered {
            canvas_ui::kit::KitState::Hovered
        } else {
            canvas_ui::kit::KitState::Normal
        };
        let close_style = canvas_ui::kit::icon_button_style(close_state, &kit_palette);
        let close_rect = UiRect::new(close_x, head_y + 2.0, CLOSE_BTN_SIZE, CLOSE_BTN_SIZE);
        d.control(close_rect, &close_style);
        d.label_center(close_rect, "✕", close_style.text, 13.0);

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
        // Context chip (ширина — остаток от provider/rate chips).
        let prov_label = crate::settings_ui::ai_provider_label(
            self.settings.language,
            self.settings.llm.provider_agent,
        );
        let rate_label = format!(
            "ChatGPT: {}/80",
            self.ai_chatgpt_rate_used.saturating_sub(0)
        );
        let rate_visible =
            self.settings.llm.provider_agent == canvas_llm::LlmProviderId::ChatGptOAuth;
        let chip_h = 18.0;
        let chip_y = ctx_y + 6.0;
        let chip_gap = 4.0;
        // Layout: [Context ...] [Provider 100px] [Rate 100px?]
        let rate_w = if rate_visible { CHIP_W } else { 0.0 };
        let prov_x = panel_x + panel_w - PAD - rate_w - chip_gap - CHIP_W;
        let rate_x = panel_x + panel_w - PAD - rate_w;
        let ctx_x = panel_x + PAD;
        let ctx_w = prov_x - chip_gap - ctx_x;
        // Context chip.
        let ctx_state = if matches!(ctx, AgentContext::SelectedNodes(_)) {
            canvas_ui::kit::KitState::Selected
        } else {
            canvas_ui::kit::KitState::Normal
        };
        let ctx_chip_style = canvas_ui::kit::chip_style(ctx_state, &kit_palette);
        let ctx_rect = UiRect::new(ctx_x, chip_y, ctx_w, chip_h);
        d.control(ctx_rect, &ctx_chip_style);
        d.label_center(ctx_rect, &ctx_label, ctx_chip_style.text, 10.0);
        // Provider chip.
        let prov_chip_style =
            canvas_ui::kit::chip_style(canvas_ui::kit::KitState::Normal, &kit_palette);
        let prov_rect = UiRect::new(prov_x, chip_y, CHIP_W, chip_h);
        d.control(prov_rect, &prov_chip_style);
        d.label_center(prov_rect, &prov_label, prov_chip_style.text, 10.0);
        // Rate chip (только для ChatGPT OAuth).
        if rate_visible {
            let rate_chip_style =
                canvas_ui::kit::chip_style(canvas_ui::kit::KitState::Normal, &kit_palette);
            let rate_rect = UiRect::new(rate_x, chip_y, CHIP_W, chip_h);
            d.control(rate_rect, &rate_chip_style);
            d.label_center(rate_rect, &rate_label, rate_chip_style.text, 10.0);
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
        if self.agent_panel.messages.is_empty() {
            d.label_left(
                UiRect::new(
                    log_rect.x + 8.0,
                    msg_y,
                    log_rect.w - 16.0,
                    log_rect.h - 16.0,
                ),
                "Панель агента пуста. Опишите задачу текстом — агент вызовет \
                 MCP-инструменты (node_create, graph_validate, graph_apply…), \
                 покажет preview ghost-нодами на канвасе и спросит Accept/Reject. \
                 Деструктивные операции — только с подтверждением.",
                kit_palette.text_muted,
                11.0,
            );
        } else {
            // Каждое сообщение — user (справа) / bot (слева) / error.
            for msg in &self.agent_panel.messages {
                match msg {
                    AgentMessage::User(text) => {
                        // User bubble — выравнивание справа, акцентный фон.
                        let bubble_w = (log_rect.w - 16.0).min(text.len() as f32 * 6.0 + 16.0);
                        let bubble_x = log_rect.x + log_rect.w - 8.0 - bubble_w;
                        let bubble_h =
                            28.0_f32.max((text.len() as f32 / 32.0).ceil() * 16.0 + 12.0);
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
                        let n_lines = (text.len() / 48).max(1);
                        let mut bubble_h = 24.0 + (n_lines as f32) * 14.0;
                        // tool_calls — каждая строка +14px.
                        bubble_h += tool_calls.len() as f32 * 14.0;
                        // Accept/Reject buttons если есть preview.
                        let has_preview =
                            self.agent_panel.preview.is_some() && *kind == AgentMsgKind::Normal;
                        if has_preview {
                            bubble_h += PREVIEW_BTN_H + 8.0;
                        }
                        // FR-LLM-D: семантические цвета — берём из kit_palette
                        // (control_danger/control_success — audit §3: зелёный
                        // раньше был инлайн-литералом `[0.30, 0.75, 0.55, 1.0]`,
                        // паритет с ai_status_panel).
                        let success_color: [f32; 4] = kit_palette.control_success;
                        let danger_color = kit_palette.control_danger;
                        let (bot_fill, bot_border, bot_text) = match kind {
                            AgentMsgKind::Normal => (
                                kit_palette.panel_fill,
                                kit_palette.panel_border,
                                kit_palette.text,
                            ),
                            AgentMsgKind::Error => (
                                [danger_color[0], danger_color[1], danger_color[2], 0.08],
                                [danger_color[0], danger_color[1], danger_color[2], 0.38],
                                danger_color,
                            ),
                            AgentMsgKind::Success => (
                                [success_color[0], success_color[1], success_color[2], 0.08],
                                [success_color[0], success_color[1], success_color[2], 0.38],
                                success_color,
                            ),
                        };
                        let bot_style = canvas_ui::kit::control_style_of(
                            bot_fill,
                            bot_border,
                            bot_text,
                            canvas_core::tokens::RADIUS_PANEL,
                        );
                        let bubble_rect = UiRect::new(log_rect.x + 8.0, msg_y, bubble_w, bubble_h);
                        d.control(bubble_rect, &bot_style);
                        d.label_left(
                            UiRect::new(
                                bubble_x_inner(bubble_rect),
                                msg_y + 4.0,
                                bubble_w - 16.0,
                                14.0,
                            ),
                            "AI Агент",
                            kit_palette.text_muted,
                            9.0,
                        );
                        d.label_left(
                            UiRect::new(
                                bubble_x_inner(bubble_rect),
                                msg_y + 16.0,
                                bubble_w - 16.0,
                                bubble_h - 20.0,
                            ),
                            text,
                            bot_text,
                            11.0,
                        );
                        msg_y += 16.0 + (n_lines as f32) * 14.0 + 4.0;
                        // tool_calls — моноширинные строки.
                        for tc in tool_calls {
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
                            d.label_left(
                                UiRect::new(
                                    bubble_x_inner(bubble_rect),
                                    msg_y,
                                    bubble_w - 16.0,
                                    14.0,
                                ),
                                &format!("{} {}  {}", status_glyph, tc.tool_name, tc.args_summary),
                                kit_palette.text_muted,
                                10.0,
                            );
                            // Status glyph — отдельным цветом (поверх серого).
                            d.label_left(
                                UiRect::new(bubble_x_inner(bubble_rect), msg_y, 14.0, 14.0),
                                status_glyph,
                                status_color,
                                10.0,
                            );
                            msg_y += 14.0;
                        }
                        // Accept/Reject кнопки — если есть preview.
                        if has_preview {
                            msg_y += 4.0;
                            let btn_w = (bubble_w - 8.0) / 2.0;
                            let accept_rect = UiRect::new(
                                bubble_x_inner(bubble_rect),
                                msg_y,
                                btn_w,
                                PREVIEW_BTN_H,
                            );
                            let reject_rect = UiRect::new(
                                bubble_x_inner(bubble_rect) + btn_w + 8.0,
                                msg_y,
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
                            msg_y += PREVIEW_BTN_H;
                        }
                        msg_y += 8.0;
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
        let input_y = panel_y + panel_h - QUICK_H - INPUT_H + 4.0;
        let send_w = 36.0;
        let input_w = panel_w - PAD * 2.0 - send_w - GAP;
        let input_rect = UiRect::new(panel_x + PAD, input_y, input_w, INPUT_H - 8.0);
        let input_state = if self.agent_panel.busy {
            canvas_ui::kit::KitState::Disabled
        } else {
            canvas_ui::kit::KitState::Normal
        };
        let input_style = canvas_ui::kit::control_style_of(
            kit_palette.panel_fill,
            kit_palette.panel_border,
            kit_palette.text,
            canvas_core::tokens::RADIUS_PANEL,
        );
        let _ = input_state; // FR-LLM-D-TODO: визуальное отличие disabled (через kit Disabled слот — будущая волна kit)
        d.control(input_rect, &input_style);
        let placeholder = "Опишите задачу: «создай CAC и свяжи с LTV»";
        let input_text = if self.agent_panel.input.is_empty() {
            placeholder.to_owned()
        } else {
            self.agent_panel.input.clone()
        };
        let input_color = if self.agent_panel.input.is_empty() {
            kit_palette.text_muted
        } else {
            kit_palette.text
        };
        d.label_left(
            UiRect::new(
                input_rect.x + 8.0,
                input_rect.y + 4.0,
                input_rect.w - 16.0,
                input_rect.h - 8.0,
            ),
            &input_text,
            input_color,
            12.0,
        );
        // Send button ➤.
        let send_rect = UiRect::new(
            panel_x + PAD + input_w + GAP,
            input_y,
            send_w,
            INPUT_H - 8.0,
        );
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
        d.label_center(send_rect, "➤", send_style.text, 14.0);

        // === Quick actions (3 preset pill-кнопки) ========================
        let quick_y = panel_y + panel_h - QUICK_H + 4.0;
        let quick_labels: [&str; 3] = ["CAC ↔ LTV", "Воронка из 3 нод", "Проверка графа"];
        let quick_w = (panel_w - PAD * 2.0 - GAP * 2.0) / 3.0;
        for (i, label) in quick_labels.iter().enumerate() {
            let qx = panel_x + PAD + i as f32 * (quick_w + GAP);
            let qhovered = point_in_rect([qx, quick_y, quick_w, 24.0], self.cursor);
            let qstate = if qhovered {
                canvas_ui::kit::KitState::Hovered
            } else {
                canvas_ui::kit::KitState::Normal
            };
            // Pill: chip_style с радиусом RADIUS_PILL (максимальный радиус).
            let qstyle = canvas_ui::kit::chip_style(qstate, &kit_palette);
            let qrect = UiRect::new(qx, quick_y, quick_w, 24.0);
            d.rect(qrect, qstyle.fill, qstyle.border, PILL_RADIUS);
            d.label_center(qrect, label, qstyle.text, 10.0);
        }

        // FR-LLM-D: дрейн KitDraw → возвращаемые Vec'и (как ai_status_panel.rs).
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
        (quads, texts)
    }

    /// FR-LLM-D / PRD-0010 F-4: rect панели агента (для hit-тестов и
    /// клавиатурного фокуса). `None` — панель скрыта.
    pub(super) fn agent_panel_rect(&self) -> Option<[f32; 4]> {
        if !self.agent_panel.open {
            return None;
        }
        let viewport = self.viewport_logical();
        if viewport[0] < AGENT_PANEL_MIN_VIEWPORT_W || viewport[1] <= 0.0 {
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

    /// FR-LLM-D / PRD-0010 F-4: принять preview → применить через graph_apply
    /// одним undo-шагом. MOCK: создаёт ноды напрямую в канвасе (без MCP).
    ///
    /// FR-LLM-D-TODO: реальная реализация — через `canvas_mcp::graph_apply`
    /// или `canvas_scene::mcp_dispatch` с операцией `node_create_note` +
    /// `edge_create`. Здесь — заглушка для UI-демо.
    pub(super) fn agent_accept_preview(&mut self) {
        let Some(preview) = self.agent_panel.preview.take() else {
            return;
        };
        let n = preview.nodes.len();
        let m = preview.edges.len();
        // FR-LLM-D-TODO: реальный graph_apply с undo-шагом.
        // Сейчас просто очистим preview и добавим success-сообщение.
        self.agent_panel.messages.push(AgentMessage::Bot {
            text: format!(
                "✓ Применено: {} {}·{} {}· graph_apply — один undo-шаг (F-4.5). \
                 Деструктивные ops по-прежнему требуют confirm.",
                n,
                plural_ru(n, "нода", "ноды", "нод"),
                m,
                plural_ru(m, "связь", "связи", "связей")
            ),
            kind: AgentMsgKind::Success,
            tool_calls: Vec::new(),
        });
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

        // 2. User-сообщение в журнал.
        self.agent_panel
            .messages
            .push(AgentMessage::User(txt.to_owned()));
        self.agent_panel.input.clear();
        self.agent_panel.caret = 0;
        self.agent_panel.busy = true;

        // 3. Контекст (для отображения и будущего redact).
        let ctx = self.agent_selection_context();
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
    pub(crate) fn agent_panel_hit(&self, point: [f32; 2]) -> Option<AgentPanelHit> {
        let panel = self.agent_panel_rect()?;
        if !point_in_rect(panel, point) {
            return None;
        }
        let [panel_x, panel_y, panel_w, panel_h] = panel;
        // Кнопка закрытия ✕ (правый край шапки).
        let head_y = panel_y + 8.0;
        let close_x = panel_x + panel_w - PAD - CLOSE_BTN_SIZE;
        if point_in_rect(
            [close_x, head_y + 2.0, CLOSE_BTN_SIZE, CLOSE_BTN_SIZE],
            point,
        ) {
            return Some(AgentPanelHit::Close);
        }
        // Input area.
        let input_y = panel_y + panel_h - QUICK_H - INPUT_H + 4.0;
        let send_w = 36.0;
        let input_w = panel_w - PAD * 2.0 - send_w - GAP;
        let input_rect = [panel_x + PAD, input_y, input_w, INPUT_H - 8.0];
        if point_in_rect(input_rect, point) {
            return Some(AgentPanelHit::Input);
        }
        // Send button.
        let send_rect = [
            panel_x + PAD + input_w + GAP,
            input_y,
            send_w,
            INPUT_H - 8.0,
        ];
        if point_in_rect(send_rect, point) {
            return Some(AgentPanelHit::Send);
        }
        // Quick actions (3 preset).
        let quick_y = panel_y + panel_h - QUICK_H + 4.0;
        let quick_w = (panel_w - PAD * 2.0 - GAP * 2.0) / 3.0;
        for i in 0..3 {
            let qx = panel_x + PAD + i as f32 * (quick_w + GAP);
            if point_in_rect([qx, quick_y, quick_w, 24.0], point) {
                return Some(AgentPanelHit::QuickAction(i));
            }
        }
        // Accept/Reject кнопки (если есть preview в последнем сообщении).
        let log_y = panel_y + HEAD_H + CTX_H;
        let log_h = panel_h - HEAD_H - CTX_H - EST_H - INPUT_H - QUICK_H - GAP;
        if self.agent_panel.preview.is_some() {
            // Грубая проверка — кнопки где-то в нижней половине лога.
            // FR-LLM-D-TODO: точная геометрия — через макет сообщений.
            let btn_y = log_y + log_h - PREVIEW_BTN_H - 16.0;
            let btn_w = (panel_w - PAD * 2.0 - 8.0) / 2.0;
            let accept_rect = [panel_x + PAD + 16.0, btn_y, btn_w, PREVIEW_BTN_H];
            let reject_rect = [
                panel_x + PAD + 16.0 + btn_w + 8.0,
                btn_y,
                btn_w,
                PREVIEW_BTN_H,
            ];
            if point_in_rect(accept_rect, point) {
                return Some(AgentPanelHit::Accept);
            }
            if point_in_rect(reject_rect, point) {
                return Some(AgentPanelHit::Reject);
            }
        }
        // Клик мимо активных элементов (тело панели) — глотаем ввод.
        Some(AgentPanelHit::NoOp)
    }
}

/// FR-LLM-D: вспомогательная функция — x-координата внутреннего отступа
/// bubble (8px от левого края bubble).
fn bubble_x_inner(bubble: UiRect) -> f32 {
    bubble.x + 8.0
}

/// FR-LLM-D: русская плюрализация (как прототип `pluralRu`).
/// Возвращает один из вариантов по числу: 1 — `one`, 2-4 — `few`,
/// 5-0/11-14 — `many`. Входные строки пробрасываются по lifetime `'a` —
/// вызывающий передаёт строковые литералы (`&'static str`), lifetimes
/// согласованы.
fn plural_ru<'a>(n: usize, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
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

#[cfg(test)]
mod tests {
    use super::*;

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
    #[test]
    fn agent_state_default() {
        let s = AgentState::default();
        assert!(!s.open);
        assert!(s.messages.is_empty());
        assert!(s.input.is_empty());
        assert!(!s.busy);
        assert!(s.preview.is_none());
        assert!(s.cost_estimate.is_none());
        assert_eq!(s.caret, 0);
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
}
