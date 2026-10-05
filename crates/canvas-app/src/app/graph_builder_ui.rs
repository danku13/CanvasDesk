#![allow(dead_code)] // FR-LLM-D: hit-test + state used by future Stream D integration (worker wiring)
//! FR-LLM-D / PRD-0010 F-3 — Graph builder dialog: модальный диалог генерации
//! графа из текста через LLM (.byok → .canvas JSON).
//!
//! Диалог: textarea (текст пользователя) + mode selector (3 radio-cards:
//! Mindmap / Outline / Summary) + cost estimate + Generate/Cancel buttons
//! + preview ghost-nodes на канвасе (reuse [`AgentPreview`]).
//!
//! **Не вызывает LLM напрямую.** UI + state management — здесь; сетевой
//! вызов (`GraphBuilder::build`) пойдёт через worker, точка интеграции —
//! [`App::graph_builder_generate`] (`// FR-LLM-D-TODO`).
//!
//! **Mock generate:** [`App::graph_builder_generate`] строит preview из
//! текста по эвристикам (как `agent_build_preview`), не вызывая LLM.
//!
//! FR-055: рендер через `KitDraw` + kit-компоненты (`modal_style`,
//! `button_style`, `chip_style`). Никаких сырых `CardInstance` — все
//! квады/тексты идут через адаптер `KitDraw`.

// FR-LLM-D: маркер для поиска (grep) — все новые файлы/добавления помечены
// `// FR-LLM-D:` в комментариях.
// FR-LLM-D-TODO: маркер для мест, где нужна интеграция с LLM-воркером.

use super::*;
// FR-LLM-D: UiRect — геометрия kit-компонентов.
use canvas_ui::geometry::UiRect;

// FR-LLM-D: re-use AgentPreview / AgentPreviewNode из agent_panel.rs (одна
// semantics — ghost-ноды для accept/reject).
pub use super::agent_panel::{AgentPreview, AgentPreviewNode};

/// FR-LLM-D / PRD-0010 F-3: режим генерации графа (обёртка над
/// `canvas_graph_builder::GraphBuilderMode` для UI state).
pub use canvas_graph_builder::GraphBuilderMode;

/// Ширина диалога (как прототип #aisetov — 640px, но генератор компактнее).
const DIALOG_W: f32 = 560.0;
/// Высота диалога (textarea ~120px + mode cards 3×54px + buttons).
const DIALOG_H: f32 = 460.0;
/// Внутренний отступ.
const PAD: f32 = 16.0;
/// Высота textarea (многострочный input).
const TEXTAREA_H: f32 = 120.0;
/// Высота radio-card режима.
const MODE_CARD_H: f32 = 54.0;
/// Зазор между radio-cards.
const MODE_GAP: f32 = 6.0;
/// Высота кнопок Generate / Cancel.
const BTN_H: f32 = canvas_ui::kit::BUTTON_HEIGHT;
/// Ширина кнопок.
const BTN_W: f32 = 130.0;

/// FR-LLM-D / PRD-0010 F-3: состояние диалога генерации графа.
#[derive(Debug, Clone, Default)]
pub struct GraphBuilderState {
    /// Диалог открыт.
    pub open: bool,
    /// Текст пользователя (textarea).
    pub text: String,
    /// Выбранный режим (mindmap / outline / summary).
    pub mode: GraphBuilderMode,
    /// Preview ghost-нод (после Generate, до Accept/Reject).
    pub preview: Option<AgentPreview>,
    /// Cost estimate для отображения (Q4 — перед Generate).
    pub cost_estimate: Option<f32>,
    /// Идёт LLM-запрос (блокирует кнопки).
    pub busy: bool,
    /// Каретка в textarea.
    pub caret: usize,
}

impl App {
    /// FR-LLM-D / PRD-0010 F-3: render graph builder dialog overlay.
    /// Возвращает (quads, texts) для screen_bands. Диалог скрыт если
    /// `!state.open`.
    pub(super) fn graph_builder_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut quads = Vec::new();
        let mut texts = Vec::new();
        if !self.graph_builder.open {
            return (quads, texts);
        }
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (quads, texts);
        }
        let palette = self.effective_palette();
        let kit_palette = palette.kit_palette();
        // FR-LLM-D: KitDraw-адаптер.
        let mut d = crate::kit_ui::KitDraw::new();
        d.set_icon_set(self.icon_set_active());

        // === Затемнение канваса (как ai_onboarding_overlay) ===============
        d.rect(
            UiRect::new(0.0, 0.0, viewport[0], viewport[1]),
            palette.stage_dim,
            [0.0; 4],
            0.0,
        );

        // === Карточка диалога — kit::modal_style (FR-055) ================
        let dialog_w = DIALOG_W.min(viewport[0] - 40.0);
        let dialog_h = DIALOG_H.min(viewport[1] - 40.0);
        let dialog_x = (viewport[0] - dialog_w) / 2.0;
        let dialog_y = (viewport[1] - dialog_h) / 2.0;
        let modal_style = canvas_ui::kit::modal_style(&kit_palette);
        let dialog_rect = UiRect::new(dialog_x, dialog_y, dialog_w, dialog_h);
        d.rect(
            dialog_rect,
            modal_style.fill,
            modal_style.border,
            modal_style.radius,
        );

        // === Заголовок «Генерация графа» =================================
        let title_y = dialog_y + PAD;
        d.label_left(
            UiRect::new(dialog_x + PAD, title_y, dialog_w - PAD * 2.0, 20.0),
            "Генерация графа",
            kit_palette.text_title,
            16.0,
        );
        // Подзаголовок (режим + стоимость).
        let sub_y = title_y + 22.0;
        let mode_label = self.graph_builder.mode.label();
        let cost_str = match self.graph_builder.cost_estimate {
            Some(c) => format!("· оценка ~${:.2}", c),
            None => "· оценка ~$0.02".to_owned(),
        };
        d.label_left(
            UiRect::new(dialog_x + PAD, sub_y, dialog_w - PAD * 2.0, 14.0),
            &format!("Режим: {} {}", mode_label, cost_str),
            kit_palette.text_muted,
            11.0,
        );

        // === Textarea (многострочный input) ==============================
        let ta_y = sub_y + 20.0;
        let ta_w = dialog_w - PAD * 2.0;
        let ta_rect = UiRect::new(dialog_x + PAD, ta_y, ta_w, TEXTAREA_H);
        let ta_style = canvas_ui::kit::control_style_of(
            kit_palette.panel_fill,
            kit_palette.panel_border,
            kit_palette.text,
            canvas_core::tokens::RADIUS_PANEL,
        );
        d.control(ta_rect, &ta_style);
        let ta_text = if self.graph_builder.text.is_empty() {
            "Вставьте текст для генерации графа (1-3 абзаца). Mindmap — дерево идей, \
             Outline — список разделов, Summary — сводка ключевых тезисов."
                .to_owned()
        } else {
            self.graph_builder.text.clone()
        };
        let ta_color = if self.graph_builder.text.is_empty() {
            kit_palette.text_muted
        } else {
            kit_palette.text
        };
        d.label_left(
            UiRect::new(
                ta_rect.x + 8.0,
                ta_rect.y + 6.0,
                ta_rect.w - 16.0,
                ta_rect.h - 12.0,
            ),
            &ta_text,
            ta_color,
            12.0,
        );

        // === Mode selector — 3 radio-cards (вертикально) =================
        let modes_y = ta_y + TEXTAREA_H + 12.0;
        let modes = [
            (
                GraphBuilderMode::Mindmap,
                "Mindmap",
                "Дерево идей от центральной темы",
            ),
            (
                GraphBuilderMode::Outline,
                "Outline",
                "Плоский список разделов с под-нодами",
            ),
            (
                GraphBuilderMode::Summary,
                "Summary",
                "Сводка текста в 3-5 ключевых нодах",
            ),
        ];
        for (i, (mode, label, desc)) in modes.iter().enumerate() {
            let mc_y = modes_y + i as f32 * (MODE_CARD_H + MODE_GAP);
            let mc_rect = UiRect::new(dialog_x + PAD, mc_y, ta_w, MODE_CARD_H);
            let selected = self.graph_builder.mode == *mode;
            let hovered = point_in_rect([mc_rect.x, mc_mc_y(mc_y), ta_w, MODE_CARD_H], self.cursor);
            // J2 (Task J / FR-UI-RADIO): migrate style computation to
            // `kit::radio_card` — card_fill/card_border/indicator_fill/
            // label_color/desc_color now come from the kit's (selected,
            // state) → palette slots mapping (1:1 with the previous manual
            // match — I-1: zero visual jump). Geometry (indicator 16×16
            // glyph at y=8, label.y=6, desc.y=24) preserved as-is — kit's
            // canonical indicator 12×12 at y=center/label.y=8/desc.y=30
            // differs from existing layout; full geometry migration would
            // cause ~6px visual shift of label/desc rows + indicator resize
            // 16→12 — separate wave of UI-geometry canonicalization
            // (analogous to Agent G's close-button TODOs).
            let state = if hovered {
                canvas_ui::kit::KitState::Hovered
            } else {
                canvas_ui::kit::KitState::Normal
            };
            let (_rc_layout, rc_style) = canvas_ui::kit::radio_card(
                mc_rect,
                100.0,
                Some(ta_w - 36.0),
                selected,
                state,
                &kit_palette,
            );
            let mc_style = canvas_ui::kit::control_style_of(
                rc_style.card_fill,
                rc_style.card_border,
                rc_style.label_color,
                rc_style.radius,
            );
            d.control(mc_rect, &mc_style);
            // Radio dot ◉ / ○ — glyph fallback (kit's paint_radio_card draws
            // a filled circle 12×12 at canonical centered position; existing
            // uses glyph "◉"/"○" in 14px font at y=8 — preserved 1:1).
            let dot_glyph = if selected { "◉" } else { "○" };
            d.label_left(
                UiRect::new(mc_rect.x + 8.0, mc_rect.y + 8.0, 16.0, 16.0),
                dot_glyph,
                rc_style.indicator_fill,
                14.0,
            );
            // Label (bold) — position preserved.
            d.label_left(
                UiRect::new(mc_rect.x + 28.0, mc_rect.y + 6.0, 100.0, 16.0),
                label,
                rc_style.label_color,
                12.0,
            );
            // Description (приглушённый) — position preserved.
            d.label_left(
                UiRect::new(mc_rect.x + 28.0, mc_rect.y + 24.0, ta_w - 36.0, 14.0),
                desc,
                rc_style.desc_color,
                11.0,
            );
        }

        // === Cost estimate (под mode cards) ==============================
        let est_y = modes_y + 3.0 * (MODE_CARD_H + MODE_GAP) + 4.0;
        let est_str = format!(
            "Запрос ~${:.2} · день ${:.2} / ${:.2}",
            self.graph_builder.cost_estimate.unwrap_or(0.02),
            self.ai_cost_day,
            self.settings.llm.cost_limit_daily
        );
        d.label_left(
            UiRect::new(dialog_x + PAD, est_y, dialog_w - PAD * 2.0, 14.0),
            &est_str,
            kit_palette.text_muted,
            10.0,
        );

        // === Кнопки Cancel / Generate — kit::footer_buttons_measured ======
        // J2 (Task J / FR-UI-FOOTER): migrate footer button rect computation
        // to `kit::footer_buttons_measured`. Slot is the bottom button row
        // ( BTN_H tall, dialog.w - PAD wide — the right inset PAD = 16 is
        // preserved by insetting slot.w). Buttons [Cancel (130), Generate
        // (130)] right-aligned, gap = `GAP_CONTROLS = SPACING_SM = 8`
        // (matches existing `gen_x - 8.0 - BTN_W`). Positions are 1:1 with
        // the previous manual `gen_x = dialog.right - PAD - BTN_W` /
        // `cancel_x = gen_x - 8 - BTN_W` formula — I-1: zero visual jump.
        let btn_y = dialog_y + dialog_h - PAD - BTN_H;
        let footer_slot = UiRect::new(dialog_x, btn_y, dialog_w - PAD, BTN_H);
        let mut footer_m = canvas_ui::measure::TextMeasurer::new();
        let footer_widths = [BTN_W, BTN_W];
        let footer_btns =
            canvas_ui::kit::footer_buttons_measured(footer_slot, &footer_widths, &mut footer_m);
        // kit returns left-to-right: [0] = Cancel (left), [1] = Generate (right).
        let cancel_rect = footer_btns[0].0;
        let gen_rect = footer_btns[1].0;
        // Cancel — Secondary.
        let cancel_hovered = point_in_rect(
            [cancel_rect.x, cancel_rect.y, cancel_rect.w, cancel_rect.h],
            self.cursor,
        );
        let cancel_style = canvas_ui::kit::button_style(
            canvas_ui::kit::ButtonVariant::Secondary,
            if cancel_hovered {
                canvas_ui::kit::KitState::Hovered
            } else {
                canvas_ui::kit::KitState::Normal
            },
            &kit_palette,
        );
        d.control(cancel_rect, &cancel_style);
        d.label_center(cancel_rect, "Отмена", cancel_style.text, 13.0);
        // Generate — Primary (disabled если busy).
        let gen_hovered = point_in_rect(
            [gen_rect.x, gen_rect.y, gen_rect.w, gen_rect.h],
            self.cursor,
        );
        let gen_state = if self.graph_builder.busy {
            canvas_ui::kit::KitState::Disabled
        } else if gen_hovered {
            canvas_ui::kit::KitState::Hovered
        } else {
            canvas_ui::kit::KitState::Normal
        };
        let gen_style = canvas_ui::kit::button_style(
            canvas_ui::kit::ButtonVariant::Primary,
            gen_state,
            &kit_palette,
        );
        let gen_label = if self.graph_builder.busy {
            "Генерация…"
        } else {
            "Сгенерировать"
        };
        d.control(gen_rect, &gen_style);
        d.label_center(gen_rect, gen_label, gen_style.text, 13.0);

        // === Preview (если есть — внутри диалога, compact list) =========
        if let Some(preview) = &self.graph_builder.preview {
            let prev_y = btn_y - 60.0;
            let prev_rect = UiRect::new(dialog_x + PAD, prev_y, dialog_w - PAD * 2.0, 52.0);
            let prev_style = canvas_ui::kit::control_style_of(
                [
                    kit_palette.accent[0],
                    kit_palette.accent[1],
                    kit_palette.accent[2],
                    0.08,
                ],
                [
                    kit_palette.accent[0],
                    kit_palette.accent[1],
                    kit_palette.accent[2],
                    0.40,
                ],
                kit_palette.text,
                canvas_core::tokens::RADIUS_PANEL,
            );
            d.control(prev_rect, &prev_style);
            let prev_label = format!(
                "Preview: {} нод, {} рёбер. Accept — применить, Reject — отменить.",
                preview.nodes.len(),
                preview.edges.len()
            );
            d.label_left(
                UiRect::new(
                    prev_rect.x + 8.0,
                    prev_rect.y + 6.0,
                    prev_rect.w - 16.0,
                    16.0,
                ),
                &prev_label,
                kit_palette.accent,
                11.0,
            );
            // Accept / Reject кнопки (мини).
            let acc_w = 70.0;
            let acc_x = prev_rect.x + prev_rect.w - 8.0 - acc_w * 2.0 - 4.0;
            let rej_x = acc_x + acc_w + 4.0;
            let acc_hovered = point_in_rect([acc_x, prev_rect.y + 24.0, acc_w, 20.0], self.cursor);
            let rej_hovered = point_in_rect([rej_x, prev_rect.y + 24.0, acc_w, 20.0], self.cursor);
            let acc_style = canvas_ui::kit::button_style(
                canvas_ui::kit::ButtonVariant::Primary,
                if acc_hovered {
                    canvas_ui::kit::KitState::Hovered
                } else {
                    canvas_ui::kit::KitState::Normal
                },
                &kit_palette,
            );
            let acc_rect = UiRect::new(acc_x, prev_rect.y + 24.0, acc_w, 20.0);
            d.control(acc_rect, &acc_style);
            d.label_center(acc_rect, "Accept", acc_style.text, 10.0);
            let rej_style = canvas_ui::kit::button_style(
                canvas_ui::kit::ButtonVariant::Secondary,
                if rej_hovered {
                    canvas_ui::kit::KitState::Hovered
                } else {
                    canvas_ui::kit::KitState::Normal
                },
                &kit_palette,
            );
            let rej_rect = UiRect::new(rej_x, prev_rect.y + 24.0, acc_w, 20.0);
            d.control(rej_rect, &rej_style);
            d.label_center(rej_rect, "Reject", rej_style.text, 10.0);
        }

        // FR-LLM-D: дрейн KitDraw → возвращаемые Vec'и.
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

    /// FR-LLM-D / PRD-0010 F-3: rect диалога генератора (для hit-тестов).
    pub(super) fn graph_builder_rect(&self) -> Option<[f32; 4]> {
        if !self.graph_builder.open {
            return None;
        }
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return None;
        }
        let dialog_w = DIALOG_W.min(viewport[0] - 40.0);
        let dialog_h = DIALOG_H.min(viewport[1] - 40.0);
        Some([
            (viewport[0] - dialog_w) / 2.0,
            (viewport[1] - dialog_h) / 2.0,
            dialog_w,
            dialog_h,
        ])
    }

    /// FR-LLM-D / PRD-0010 F-3: hit-test диалога генератора графа.
    pub(crate) fn graph_builder_hit(&self, point: [f32; 2]) -> Option<GraphBuilderHit> {
        let [dx, dy, dw, dh] = self.graph_builder_rect()?;
        // Клик мимо диалога (по затемнению) — закрыть.
        if !point_in_rect([dx, dy, dw, dh], point) {
            return Some(GraphBuilderHit::Backdrop);
        }
        // Cancel / Generate кнопки — kit::footer_buttons_measured (J2:
        // geometry mirrors the overlay render path; same slot/widths).
        let btn_y = dy + dh - PAD - BTN_H;
        let footer_slot = UiRect::new(dx, btn_y, dw - PAD, BTN_H);
        let mut footer_m = canvas_ui::measure::TextMeasurer::new();
        let footer_widths = [BTN_W, BTN_W];
        let footer_btns =
            canvas_ui::kit::footer_buttons_measured(footer_slot, &footer_widths, &mut footer_m);
        let cancel_rect = footer_btns[0].0;
        let gen_rect = footer_btns[1].0;
        if point_in_rect(
            [cancel_rect.x, cancel_rect.y, cancel_rect.w, cancel_rect.h],
            point,
        ) {
            return Some(GraphBuilderHit::Cancel);
        }
        if point_in_rect([gen_rect.x, gen_rect.y, gen_rect.w, gen_rect.h], point) {
            return Some(GraphBuilderHit::Generate);
        }
        // Textarea (для фокуса).
        let ta_y = dy + PAD + 22.0 + 20.0;
        let ta_w = dw - PAD * 2.0;
        if point_in_rect([dx + PAD, ta_y, ta_w, TEXTAREA_H], point) {
            return Some(GraphBuilderHit::Textarea);
        }
        // Mode cards (3 radio).
        let modes_y = ta_y + TEXTAREA_H + 12.0;
        for i in 0..3 {
            let mc_y = modes_y + i as f32 * (MODE_CARD_H + MODE_GAP);
            if point_in_rect([dx + PAD, mc_y, ta_w, MODE_CARD_H], point) {
                return Some(GraphBuilderHit::Mode(i));
            }
        }
        // Accept/Reject (если есть preview).
        if let Some(preview) = &self.graph_builder.preview {
            let _ = preview;
            let prev_y = btn_y - 60.0;
            let prev_w = dw - PAD * 2.0;
            let acc_w = 70.0;
            let acc_x = dx + PAD + prev_w - 8.0 - acc_w * 2.0 - 4.0;
            let rej_x = acc_x + acc_w + 4.0;
            if point_in_rect([acc_x, prev_y + 24.0, acc_w, 20.0], point) {
                return Some(GraphBuilderHit::Accept);
            }
            if point_in_rect([rej_x, prev_y + 24.0, acc_w, 20.0], point) {
                return Some(GraphBuilderHit::Reject);
            }
        }
        Some(GraphBuilderHit::NoOp)
    }

    /// FR-LLM-D / PRD-0010 F-3.4: генерация графа (mock).
    ///
    /// Шаги:
    /// 1. Проверка AI paused / off / cost limit.
    /// 2. Cost estimate (для отображения).
    /// 3. Mock: построить preview из текста (без LLM-вызова).
    ///
    /// FR-LLM-D-TODO: реальный LLM-вызов через `GraphBuilder::build()`:
    ///   let input = GraphBuilderInput::new(&state.text, state.mode);
    ///   let output = builder.build(&input).await?;
    ///   state.preview = Some(to_agent_preview(&output));
    pub(super) fn graph_builder_generate(&mut self) {
        if self.graph_builder.busy {
            return;
        }
        // 1. Защитные проверки.
        if self.ai_paused {
            self.show_toast("AI на паузе — нажмите ▶ в статусной панели");
            return;
        }
        if self.settings.llm.provider_graph == canvas_llm::LlmProviderId::Off {
            self.show_toast("Graph Builder: Off — выберите провайдера в Настройках AI");
            return;
        }
        if self.ai_cost_day >= self.settings.llm.cost_limit_daily {
            self.show_toast(format!(
                "Дневной лимит исчерпан (${:.2} / ${:.2})",
                self.ai_cost_day, self.settings.llm.cost_limit_daily
            ));
            return;
        }
        // FR-LLM-OAUTH-APP / PRD-0010 F-5.9: построение реального провайдера
        // через фабрику (`llm_factory`) до генерации — валидация конфига
        // (пустой BYOK-ключ / невыполненный OAuth-вход ChatGPT). При
        // недоступности — тост (механика панели) и запрос не уходит; для
        // ChatGptOAuth с заданным BYOK-ключом сообщение обещает fallback на
        // BYOK (реальный вызов строит провайдер повторно с `Byok` — точка
        // интеграции ниже). Mock-флоу без фичи l1-llm не меняется.
        #[cfg(feature = "l1-llm")]
        if let Some(message) = crate::llm_factory::provider_unavailable_message(
            self.settings.llm.provider_graph,
            &self.settings.llm.model_graph,
            &self.settings.llm,
            &self.oauth_assets(),
        ) {
            self.show_toast(message);
            return;
        }
        // 2. Cost estimate.
        self.graph_builder.cost_estimate = Some(0.04);
        self.graph_builder.busy = true;

        // 3. Mock preview (без LLM).
        let preview = self.graph_builder_build_preview(&self.graph_builder.text);
        self.graph_builder.preview = Some(preview);

        // FR-LLM-D-TODO: реальный LLM-вызов:
        //   let provider = ...;  // из settings.llm.provider_graph
        //   let privacy = self.settings.llm.data_residency.privacy_mode();
        //   let builder = canvas_graph_builder::GraphBuilder::new(provider, privacy);
        //   let input = canvas_graph_builder::GraphBuilderInput::new(
        //       &self.graph_builder.text,
        //       self.graph_builder.mode,
        //   );
        //   let output = pollster::block_on(builder.build(&input))?;
        //   let canvas = canvas_graph_builder::GraphBuilder::<P>::to_canvas(&output);
        //   // применить canvas через graph_apply, обновить cost.

        self.graph_builder.busy = false;
        // ChatGPT rate counter.
        if self.settings.llm.provider_graph == canvas_llm::LlmProviderId::ChatGptOAuth {
            self.ai_chatgpt_rate_used = self.ai_chatgpt_rate_used.saturating_add(1);
        }
        self.request_redraw();
    }

    /// FR-LLM-D / PRD-0010 F-3: построить preview из текста (mock — для UI).
    ///
    /// Различает те же паттерны что `agent_build_preview`:
    /// - "CAC" + "LTV" → 2 ноды + 1 ребро.
    /// - "воронк"/"funnel" → 3 ноды + 2 ребра.
    /// - Иначе → 2 generic-ноды + 1 ребро.
    fn graph_builder_build_preview(&self, text: &str) -> AgentPreview {
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
                (vec!["Тема", "Идея 1"], vec![(0, 1, "связь")])
            };
        // Layout: цепочка слева-направо от центра вьюпорта (без выделения).
        let center = self.camera.position();
        let base_x = center[0] - titles.len() as f32 * 155.0;
        let base_y = center[1] - 60.0;
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

    /// FR-LLM-D / PRD-0010 F-3: принять preview графа.
    pub(super) fn graph_builder_accept(&mut self) {
        let Some(preview) = self.graph_builder.preview.take() else {
            return;
        };
        // FR-LLM-D-TODO: реальный graph_apply с undo-шагом.
        let n = preview.nodes.len();
        let m = preview.edges.len();
        self.show_toast(format!(
            "Граф сгенерирован: {}×{} · применён одним undo-шагом",
            n, m
        ));
        self.graph_builder.cost_estimate = None;
        self.graph_builder.open = false;
        self.request_redraw();
    }

    /// FR-LLM-D / PRD-0010 F-3: отклонить preview графа.
    pub(super) fn graph_builder_reject(&mut self) {
        self.graph_builder.preview = None;
        self.graph_builder.cost_estimate = None;
        self.request_redraw();
    }
}

/// FR-LLM-D: helper для получения y-координаты mode-card (вынесен чтобы
/// не дублировать формулу в hit-test и render).
fn mc_mc_y(mc_y: f32) -> f32 {
    mc_y
}

/// FR-LLM-D / PRD-0010 F-3: hit-test диалога генератора графа — элемент под
/// курсором.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphBuilderHit {
    /// Клик мимо диалога (по затемнению) — закрыть.
    Backdrop,
    /// Клик внутри диалога, но мимо активных элементов — глотаем.
    NoOp,
    /// Textarea — фокус для печати.
    Textarea,
    /// Mode card №i (0..3).
    Mode(usize),
    /// Кнопка «Отмена» — закрыть диалог.
    Cancel,
    /// Кнопка «Сгенерировать» — запустить генерацию.
    Generate,
    /// Кнопка Accept (после генерации).
    Accept,
    /// Кнопка Reject.
    Reject,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Состояние по умолчанию — закрыто, без текста, Mindmap режим.
    #[test]
    fn state_default_closed_mindmap() {
        let s = GraphBuilderState::default();
        assert!(!s.open);
        assert!(s.text.is_empty());
        assert_eq!(s.mode, GraphBuilderMode::Mindmap);
        assert!(s.preview.is_none());
        assert!(s.cost_estimate.is_none());
        assert!(!s.busy);
    }

    /// Размеры диалога.
    #[test]
    fn dialog_sizes() {
        assert_eq!(DIALOG_W, 560.0);
        assert_eq!(DIALOG_H, 460.0);
        assert_eq!(PAD, 16.0);
        assert_eq!(TEXTAREA_H, 120.0);
        assert_eq!(MODE_CARD_H, 54.0);
    }

    /// GraphBuilderMode — 3 режима.
    #[test]
    fn mode_three_variants() {
        let modes = [
            GraphBuilderMode::Mindmap,
            GraphBuilderMode::Outline,
            GraphBuilderMode::Summary,
        ];
        assert_eq!(modes.len(), 3);
        // Все различные.
        assert_ne!(modes[0], modes[1]);
        assert_ne!(modes[1], modes[2]);
        assert_ne!(modes[0], modes[2]);
    }

    /// Mode labels non-empty.
    #[test]
    fn mode_labels_non_empty() {
        assert!(!GraphBuilderMode::Mindmap.label().is_empty());
        assert!(!GraphBuilderMode::Outline.label().is_empty());
        assert!(!GraphBuilderMode::Summary.label().is_empty());
    }

    /// Mode descriptions non-empty.
    #[test]
    fn mode_descriptions_non_empty() {
        assert!(!GraphBuilderMode::Mindmap.description().is_empty());
        assert!(!GraphBuilderMode::Outline.description().is_empty());
        assert!(!GraphBuilderMode::Summary.description().is_empty());
    }

    /// GraphBuilderHit variants distinct.
    #[test]
    fn hit_variants_distinct() {
        assert_ne!(GraphBuilderHit::Backdrop, GraphBuilderHit::NoOp);
        assert_ne!(GraphBuilderHit::Generate, GraphBuilderHit::Cancel);
        assert_ne!(GraphBuilderHit::Mode(0), GraphBuilderHit::Mode(1));
        assert_ne!(GraphBuilderHit::Accept, GraphBuilderHit::Reject);
    }

    /// mc_mc_y — identity helper.
    #[test]
    fn mc_mc_y_identity() {
        assert_eq!(mc_mc_y(42.0), 42.0);
        assert_eq!(mc_mc_y(0.0), 0.0);
    }
}
