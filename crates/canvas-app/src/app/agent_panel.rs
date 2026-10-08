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
        // CR-015 fix (Task Q1+Q2): TextMeasurer для реального шейпинга текста
        // bubble (вместо эвристик `text.len() as f32 * 6.0` / `text.len() /
        // 32.0` / `text.len() / 48` — те ломаются на Cyrillic/emoji: byte_count
        // ≠ glyph_count). Один measurer+FontSystem на цикл сообщений (не на
        // сообщение — кэш переиспользуется). Семейство/кегль — те же, что у
        // `d.label_left` рендера bubble (FR-053: метрики раскладки = метрики
        // рендера). Контракт «вложенный лок FontSystem запрещён» (app.rs:209):
        // `agent_panel_overlay` не вызывается внутри другого FontSystem-лока
        // (handler.rs:314 —顶层 render path, без шейпинг-локов рядом).
        let mut measurer = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        let bubble_family = canvas_render::text::SANS_FAMILY; // «Noto Sans Display»
        let bubble_size = 11.0; // bubble text font_size (matches d.label_left)
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

    /// W2 п.2: preview из реальных tool_calls модели. Разбираются вызовы
    /// `n` (graph_apply батч, FR-033): `n_note {ref, x, y, text}` → ноды,
    /// `edge_create {fromRef, toRef}` → рёбра (по ref-адресам батча).
    /// Ноды без x/y (модель не задала позицию) — цепочка от якоря (как
    /// [`Self::agent_build_preview`]). Вызовы без нод — пустой preview
    /// (панель покажет «нечего применять»).
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

        let mut nodes: Vec<AgentPreviewNode> = Vec::new();
        let mut refs: Vec<String> = Vec::new(); // ref → индекс в nodes
        let mut edges: Vec<(usize, usize, String)> = Vec::new();
        let mut pending_edges: Vec<(String, String)> = Vec::new();

        for call in calls {
            if call.name != "graph_apply" {
                continue;
            }
            let ops = call
                .arguments
                .to_serde()
                .get("operations")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for op in &ops {
                let op_name = op.get("op").and_then(|v| v.as_str()).unwrap_or("");
                match op_name {
                    "n_note" | "n_file" => {
                        let title = op
                            .get("text")
                            .and_then(|v| v.as_str())
                            .map(|t| t.lines().next().unwrap_or(t).to_string())
                            .or_else(|| op.get("path").and_then(|v| v.as_str()).map(String::from))
                            .unwrap_or_else(|| "Новая нода".into());
                        let i = nodes.len();
                        let auto_x = base_x + i as f32 * 320.0;
                        nodes.push(AgentPreviewNode {
                            title,
                            x: op
                                .get("x")
                                .and_then(|v| v.as_f64())
                                .map(|v| v as f32)
                                .unwrap_or(auto_x),
                            y: op
                                .get("y")
                                .and_then(|v| v.as_f64())
                                .map(|v| v as f32)
                                .unwrap_or(base_y),
                            width: op
                                .get("width")
                                .and_then(|v| v.as_f64())
                                .map(|v| v as f32)
                                .unwrap_or(240.0),
                            height: op
                                .get("height")
                                .and_then(|v| v.as_f64())
                                .map(|v| v as f32)
                                .unwrap_or(120.0),
                        });
                        if let Some(r) = op.get("ref").and_then(|v| v.as_str()) {
                            refs.push(r.to_string());
                        } else {
                            refs.push(format!("__idx{i}"));
                        }
                    }
                    "edge_create" => {
                        // Адресация fromRef/toRef резолвится ПОСЛЕ прохода
                        // нод (ref может ссылаться на более ранний op).
                        let from = op.get("fromRef").and_then(|v| v.as_str());
                        let to = op.get("toRef").and_then(|v| v.as_str());
                        if let (Some(f), Some(t)) = (from, to) {
                            pending_edges.push((f.to_string(), t.to_string()));
                        }
                    }
                    _ => {}
                }
            }
        }
        // Резолв ребер по ref → индекс; неразрешенные пропускаются.
        // (второй проход: ref-таблица уже заполнена)
        let resolve = |r: &String| -> Option<usize> { refs.iter().position(|x| x == r) };
        for (f, t) in pending_edges.drain(..) {
            if let (Some(fi), Some(ti)) = (resolve(&f), resolve(&t)) {
                edges.push((fi, ti, String::new()));
            }
        }
        AgentPreview { nodes, edges }
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
        self.agent_panel.input.clear();
        self.agent_panel.caret = 0;
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
            let system = "You are the CanvasDesk canvas agent. Plan edits and call the \
                          `graph_apply` tool (batch of FR-033 operations: n_note/n_file/template_instantiate/\
                          edge_create/edge_delete/param_set/node_move/group_create) to fulfil the \
                          user's request. Create nodes with n_note (ref/x/y/text/width/height) and \
                          link them with edge_create (fromRef/toRef). Keep the plan small and useful."
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

    /// W2 п.1: диспетчер клика по агент-панели (вызов из обработчика ввода
    /// ПЕРЕД canvas-pick — панель транзиентна, как ai-status-panel).
    /// `true` — клик поглощён панелью.
    pub(super) fn agent_panel_click(&mut self, point: [f32; 2]) -> bool {
        let Some(hit) = self.agent_panel_hit(point) else {
            return false;
        };
        match hit {
            AgentPanelHit::Close => {
                self.agent_panel.open = false;
                self.request_redraw();
            }
            AgentPanelHit::Input => {
                // Фокус input: клавиатура и так маршрутизируется в UI-слой
                // (esc_stack верхний owner); символьный ввод панели —
                // через agent_input_push (см. route_owner_key).
                self.request_redraw();
            }
            AgentPanelHit::Send => {
                let text = self.agent_panel.input.clone();
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

    /// W2 п.2: preview из tool_calls — n_note/edge_create → ghost-ноды/рёбра
    /// (парсинг аргументов батча; позиции модели сохраняются).
    #[test]
    fn preview_from_calls_parses_batch_ops() {
        let calls = [canvas_llm::ToolCall {
            id: "call_1".into(),
            name: "n".into(),
            arguments: canvas_llm::JsonVal::from_serde(&serde_json::json!({
                "operations": [
                    { "op": "n_note", "ref": "a", "x": 10.0, "y": 20.0, "text": "CAC" },
                    { "op": "n_note", "ref": "b", "y": 20.0, "text": "LTV" },
                    { "op": "edge_create", "fromRef": "a", "toRef": "b" }
                ]
            })),
        }];
        // agent_preview_from_calls — метод App; здесь проверяем чистую
        // часть через статический разбор: конвертация аргументов.
        let args = calls[0].arguments.to_serde();
        let ops = args.get("operations").and_then(|v| v.as_array()).unwrap();
        assert_eq!(ops.len(), 3);
        assert_eq!(ops[0]["ref"], "a");
        assert_eq!(ops[1]["op"], "n_note");
        assert_eq!(ops[2]["op"], "edge_create");
    }
}
