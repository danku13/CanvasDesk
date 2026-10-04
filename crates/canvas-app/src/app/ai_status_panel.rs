#![allow(dead_code)] // FR-LLM-FIX: hit-test + constants used by Stream C/D integration
//! FR-LLM-FIX / PRD-0010 F-7.9 (Q4): AI status panel — правый нижний угол,
//! над миникартой. Показывает активную модель+провайдера, тумблеры
//! Suggest/Graph/Agent, cost за сессию/день (с прогресс-баром дневного
//! лимита), кнопки ⏸ (пауза всех AI) и ⚙ (открыть таб «AI и модели»).
//!
//! **Расположение** (прототип `prototype-unified.html` строки 450-481,
//! 898-930): правый край, над миникартой. Ширина **302px** (как в
//! прототипе), высота — auto (≈ 92px без paused-лейбла, +18px с ним),
//! паддинг 9×11px, border-radius 10px.
//!
//! **Скрытие** (F-7.9): на узких окнах (viewport < 900px) панель не
//! рисуется — экономим место под канвас; при `LlmSettings::all_off()`
//! тоже скрыта (AI выключен → нечего показывать).
//!
//! FR-LLM-C / FR-055: рендер через `KitDraw` + kit-компоненты
//! (`kit::panel_style`, `kit::chip_style`, `kit::icon_button_style`).
//! Никаких сырых `CardInstance` — все квады/тексты идут через адаптер
//! `KitDraw` (как в `admin_ui.rs`/`overlays.rs`). Полоса — `UiLayer::Panels`.

use super::*;
// FR-LLM-C: UiRect — геометрия kit-компонентов (panel_style/chip_style
// принимают UiRect, не сырой [f32; 4]). Импорт локально — app.rs не
// использует UiRect (только UiPoint), несем через короткий алиас.
use canvas_ui::geometry::UiRect;

// FR-LLM-B: маркер для поиска (grep) — все новые файлы/добавления помечены
// `// FR-LLM-B:` в комментариях.
// FR-LLM-C: маркер Stream C — рефакторинг рендера панели на kit-компоненты.
// FR-LLM-FIX: маркер правок layout/per-feature-model (302px, тумблеры активны).

// FR-LLM-FIX: геометрия из прототипа F-7.9 (строки 450-481). Раньше панель
// была 220×90 (как миникарта) — элементы наезжали. Теперь 1:1 по CSS.

/// Ширина панели (прототип: `width:302px`).
pub const AI_STATUS_W: f32 = 302.0;
/// Высота панели без paused-лейбла: 9 + 24 + 7 + 16 + 7 + 14 + 5 + 4 + 9 ≈ 95.
/// Берём с округлением до целого (используется для позиционирования над
/// миникартой); реальная высота с paused-лейблом — `+ AI_STATUS_PAUSED_EXTRA`.
pub const AI_STATUS_H: f32 = 95.0;
/// Дополнительная высота когда показан paused-лейбл (5px margin + ~13px текст).
pub const AI_STATUS_PAUSED_EXTRA: f32 = 18.0;
/// Зазор над миникартой (прототип: 8px между панелью и миникартой).
pub const AI_STATUS_GAP_ABOVE_MINIMAP: f32 = 8.0;
/// Отступ от правого края = отступ миникарты (12px в прототипе `right:12px`).
pub const AI_STATUS_MARGIN: f32 = 12.0;
/// Минимальная ширина вьюпорта для показа панели (F-7.9: < 900px — скрыта).
pub const AI_STATUS_MIN_VIEWPORT_W: f32 = 900.0;
/// Размер миникарты (зеркало `canvas_render::minimap::MINIMAP_W/H`).
const MINIMAP_W: f32 = 220.0;
const MINIMAP_H: f32 = 140.0;
const MINIMAP_MARGIN: f32 = 16.0;

// FR-LLM-FIX: внутренняя геометрия — 1:1 по CSS прототипа.
/// Паддинг панели: 9px top/bottom, 11px left/right (`padding:9px 11px`).
const PAD_TOP: f32 = 9.0;
const PAD_BOTTOM: f32 = 9.0;
const PAD_X: f32 = 11.0;
/// Радиус панели — 10px (`border-radius:10px`).
const PANEL_RADIUS: f32 = canvas_core::tokens::RADIUS_PANEL;
/// Зазоры между строками (`margin-bottom:7px` у head, `margin-top:7px` у
/// costs, `margin-top:5px` у progress-bar).
const GAP_HEAD_FEATS: f32 = 7.0;
const GAP_FEATS_COST: f32 = 7.0;
const GAP_COST_BAR: f32 = 5.0;
const GAP_BAR_PAUSED: f32 = 5.0;

/// Размер цветной точки состояния (`width/height:8px; border-radius:50%`).
const DOT_SIZE: f32 = 8.0;
/// Размер кнопок ⏸/⚙ (`width/height:24px; border-radius:6px`).
const HEAD_BTN_SIZE: f32 = 24.0;
const HEAD_BTN_RADIUS: f32 = canvas_core::tokens::RADIUS_CHIP; // 6.0
/// Высота шапки (head row) — по tallest child (24px кнопки).
const HEAD_ROW_H: f32 = HEAD_BTN_SIZE;
/// Высота строки features (chip font 10.5px + padding 2.5*2 ≈ 15.5, ~16).
const FEAT_ROW_H: f32 = 16.0;
/// Высота строки costs (font 11px ≈ 14px line).
const COST_ROW_H: f32 = 14.0;
/// Высота прогресс-бара (`height:4px`).
const BAR_H: f32 = 4.0;
const BAR_RADIUS: f32 = 2.0;

// Feature chip метрики (прототип `.ais-feat`).
/// Font-size feature-чипа — 10.5px.
const FEAT_FONT: f32 = 10.5;
/// Паддинг feature-чипа — 2.5px 9px.
const FEAT_PAD_Y: f32 = 2.5;
const FEAT_PAD_X: f32 = 9.0;
/// Зазор между feature-чипами (`gap:5px`).
const FEAT_GAP: f32 = 5.0;
/// Радиус feature-чипа — pill (`border-radius:99px`). Используем RADIUS_PILL
/// из шкалы токенов (FR-055: единый источник радиусов) — визуально идентичен
/// 99px для чипов такой высоты, но не ломает хит-тест вёрстки.
const FEAT_RADIUS: f32 = canvas_core::tokens::RADIUS_PILL;

impl App {
    /// FR-LLM-FIX / PRD-0010 F-7.9 (Q4): статусная панель AI — квады + тексты.
    ///
    /// FR-LLM-C: рендер через `KitDraw` + kit-компоненты. Возвращает
    /// `(Vec<CardInstance>, Vec<OwnedScreenText>)`, дёргая `KitDraw::quads`
    /// и конвертируя `OwnedText` → `OwnedScreenText` (как `overlays.rs`).
    ///
    /// Возвращает пустые `Vec`, если:
    /// - вьюпорт слишком узкий (< 900px, F-7.9);
    /// - AI выключен (`LlmSettings::all_off()`);
    /// - вьюпорт нулевой (первый кадр / скрытое окно).
    ///
    /// Иначе — рендерит 4 строки (плюс опциональный paused-лейбл):
    /// 1. **head**: цветная точка + имя модели + подпись провайдера + ⏸ + ⚙.
    /// 2. **features**: 3 pill-чипа Suggest/Graph/Agent (✓ = включён).
    /// 3. **costs**: «Session: $X.XX · Day: $X.XX / $L.LL».
    /// 4. **progress**: горизонтальный прогресс-бар day/limit.
    /// 5. (если `ai_paused`) — янтарный лейбл «AI на паузе».
    pub(super) fn ai_status_panel(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut quads = Vec::new();
        let mut texts = Vec::new();
        let viewport = self.viewport_logical();
        if viewport[0] < AI_STATUS_MIN_VIEWPORT_W || viewport[1] <= 0.0 {
            return (quads, texts);
        }
        let llm = &self.settings.llm;
        if llm.all_off() {
            return (quads, texts);
        }
        let palette = self.effective_palette();
        let kit_palette = palette.kit_palette();
        // FR-LLM-C: KitDraw-адаптер (как overlays.rs/admin_ui.rs). Все квады
        // и тексты собираются в `d`, в конце дрейним в возвращаемые Vec'и.
        let mut d = crate::kit_ui::KitDraw::new();
        // FR-ICONS: активный набор (None = Glyph fallback; ⚙ может попасть
        // в SVG-атлас, ⏸/▶ идут глифом).
        d.set_icon_set(self.icon_set_active());

        // FR-LLM-FIX: реальная высота панели — auto (зависит от paused).
        let panel_h = if self.ai_paused {
            AI_STATUS_H + AI_STATUS_PAUSED_EXTRA
        } else {
            AI_STATUS_H
        };

        // Позиция панели: правый край, над миникартой (если миникарта есть)
        // или просто в правом нижнем углу.
        let has_minimap = self.minimap_rect().is_some();
        let panel_x = viewport[0] - AI_STATUS_W - AI_STATUS_MARGIN;
        let panel_y = if has_minimap {
            // Над миникартой (8px зазор).
            viewport[1] - MINIMAP_MARGIN - MINIMAP_H - AI_STATUS_GAP_ABOVE_MINIMAP - panel_h
        } else {
            // Миникарты нет — панель в правом нижнем углу с отступом.
            viewport[1] - AI_STATUS_MARGIN - panel_h
        };
        let panel = [panel_x, panel_y, AI_STATUS_W, panel_h];

        // === Фон панели — kit::panel_style (FR-055: слоты panel_*) ======
        let panel_style = canvas_ui::kit::panel_style(&kit_palette);
        d.rect(
            UiRect::new(panel[0], panel[1], panel[2], panel[3]),
            panel_style.fill,
            panel_style.border,
            panel_style.radius,
        );

        // FR-LLM-FIX: per-feature активная модель/провайдер. Приоритет
        // suggest > graph > agent (как в прототипе aiSetActive('suggest')).
        // Для BYOK берём model_suggest/graph/agent; для не-BYOK — фиксированную
        // модель провайдера (как aiProvModel() в прототипе).
        let (active_provider, active_model, active_label_key) = [
            (llm.provider_suggest, llm.model_suggest.as_str(), "Suggest"),
            (llm.provider_graph, llm.model_graph.as_str(), "Graph"),
            (llm.provider_agent, llm.model_agent.as_str(), "Agent"),
        ]
        .into_iter()
        .find(|(p, _, _)| *p != canvas_llm::LlmProviderId::Off)
        .unwrap_or((canvas_llm::LlmProviderId::Off, "", "AI"));
        let model_name = provider_fixed_model(active_provider, active_model);
        let model_name = if model_name.is_empty() {
            self.tr(keys::AI_STATUS_NO_MODEL).to_owned()
        } else {
            model_name.to_owned()
        };
        let provider_label =
            crate::settings_ui::ai_provider_label(self.settings.language, active_provider);

        // === Строка 1: head — точка + модель + провайдер + ⏸ + ⚙ ===========
        // head_y — верх строки; вертикально центрируем элементы внутри 24px.
        let content_x = panel[0] + PAD_X;
        let content_w = panel[2] - 2.0 * PAD_X;
        let head_y = panel[1] + PAD_TOP;
        // Цветная точка состояния: green = активно, amber = пауза.
        // Не kit-слот (индикатор состояния — семантика, а не тема); рисуем
        // через d.rect (FR-LLM-C: KitDraw-адаптер, не сырой CardInstance).
        let dot_color = if self.ai_paused {
            [0.95, 0.65, 0.30, 1.0] // янтарный — пауза
        } else {
            [0.30, 0.75, 0.55, 1.0] // зелёный — активно
        };
        // Точка — 8×8, вертикально по центру строки 24px (отступ 8 сверху).
        let dot_y = head_y + (HEAD_ROW_H - DOT_SIZE) * 0.5;
        d.rect(
            UiRect::new(content_x, dot_y, DOT_SIZE, DOT_SIZE),
            dot_color,
            [0.0; 4],
            DOT_SIZE / 2.0, // circle
        );
        // Имя модели (font 12.5px, `font-weight:600`). Подпись провайдера
        // — pill-чип правее; оставляем под модель ~ половину ширины строки
        // минус место под кнопки (24+6+24=54) и под провайдер-чип (~60).
        let model_max_w = content_w - DOT_SIZE - 6.0 - 60.0 - 6.0 - 54.0;
        let model_x = content_x + DOT_SIZE + 6.0;
        // Baseline-выравнивание: центрируем 12.5px-шрифт по 24px-строке.
        let model_y = head_y + (HEAD_ROW_H - 12.5) * 0.5;
        d.label_left(
            UiRect::new(model_x, model_y, model_max_w.max(40.0), 14.0),
            &model_name,
            kit_palette.text_title,
            12.5,
        );
        // Подпись провайдера (BYOK / ChatGPT / Ollama / Laya) — pill-чип
        // (border-radius:99px, padding:1px 7px, font 9.5px).
        let prov_chip_x = model_x + model_max_w.max(40.0) + 6.0;
        let prov_chip_h = 14.0;
        let prov_chip_y = head_y + (HEAD_ROW_H - prov_chip_h) * 0.5;
        // Ширина чипа — по подписи (грубо: len*5.5 + 14 padding); clamp.
        let prov_chip_w = (provider_label.len() as f32 * 5.5 + 14.0).clamp(48.0, 96.0);
        let prov_chip_rect = UiRect::new(prov_chip_x, prov_chip_y, prov_chip_w, prov_chip_h);
        let prov_chip_style =
            canvas_ui::kit::chip_style(canvas_ui::kit::KitState::Normal, &kit_palette);
        d.control(prov_chip_rect, &prov_chip_style);
        d.label_center(prov_chip_rect, &provider_label, prov_chip_style.text, 9.5);

        // FR-LLM-FIX: кнопки ⏸/⚙ — 24×24, border-radius 6px (RADIUS_CHIP).
        // Позиция: у правого края контента, с зазором 6px между ними.
        let gear_x = panel[0] + panel[2] - PAD_X - HEAD_BTN_SIZE;
        let pause_x = gear_x - 6.0 - HEAD_BTN_SIZE;
        let pause_glyph = if self.ai_paused { "▶" } else { "⏸" };
        let pause_hovered =
            point_in_rect([pause_x, head_y, HEAD_BTN_SIZE, HEAD_BTN_SIZE], self.cursor);
        let gear_hovered =
            point_in_rect([gear_x, head_y, HEAD_BTN_SIZE, HEAD_BTN_SIZE], self.cursor);
        // (btn_x, glyph, hovered, icon_name) — icon_name="" → глиф через
        // label_center; иначе — d.icon (SVG-атлас + glyph fallback).
        for (btn_x, glyph, hovered, icon_name) in [
            (pause_x, pause_glyph, pause_hovered, ""),
            (gear_x, "⚙", gear_hovered, "gear"),
        ] {
            let btn_rect = UiRect::new(btn_x, head_y, HEAD_BTN_SIZE, HEAD_BTN_SIZE);
            let btn_state = if hovered {
                canvas_ui::kit::KitState::Hovered
            } else {
                canvas_ui::kit::KitState::Normal
            };
            let btn_style = canvas_ui::kit::icon_button_style(btn_state, &kit_palette);
            // FR-LLM-FIX: радиус 6px — kit::icon_button_style уже использует
            // RADIUS_CHIP (6.0), совпадает с прототипом.
            d.control(btn_rect, &btn_style);
            if icon_name.is_empty() {
                // FR-ICONS: иконки нет в атласе — глиф шрифтом через label_center.
                d.label_center(btn_rect, glyph, btn_style.text, 12.0);
            } else {
                // FR-ICONS: иконка может быть в SVG-атласе; glyph — fallback.
                d.icon(btn_rect, icon_name, glyph, btn_style.text, 12.0);
            }
        }
        let _ = active_label_key; // маркер — для будущей подписи активности.

        // === Строка 2: features — 3 pill-чипа Suggest/Graph/Agent =========
        // FR-LLM-FIX: pill-форма (radius RADIUS_PILL), font 10.5px,
        // padding 2.5×9px, gap 5px, flex-wrap (на узких панелях чипы
        // переносятся на следующую строку — но ширины 302px хватает на 3).
        let feats_y = head_y + HEAD_ROW_H + GAP_HEAD_FEATS;
        let feats: [(AiStatusPanelHit, &str, bool, &'static str); 3] = [
            (
                AiStatusPanelHit::ToggleSuggest,
                "Suggest",
                self.ai_suggest_enabled,
                keys::AI_TOOLTIP_SUGGEST,
            ),
            (
                AiStatusPanelHit::ToggleGraph,
                "Graph",
                self.ai_graph_enabled,
                keys::AI_TOOLTIP_GRAPH,
            ),
            (
                AiStatusPanelHit::ToggleAgent,
                "Agent",
                self.ai_agent_enabled,
                keys::AI_TOOLTIP_AGENT,
            ),
        ];
        let mut chip_cursor_x = content_x;
        for (kind, label, enabled, _tip_key) in feats.iter() {
            // «on» = фича включена И не на паузе (как aiSyncFeats() прототипа).
            let on = *enabled && !self.ai_paused;
            // Ширина чипа — по тексту: padding 9*2 + len*5.5 (~10.5px font).
            let label_text = if on {
                format!("{} ✓", label)
            } else {
                label.to_string()
            };
            let chip_w = FEAT_PAD_X * 2.0 + label_text.len() as f32 * 5.5;
            let chip_h = FEAT_FONT + FEAT_PAD_Y * 2.0;
            let chip_rect = UiRect::new(chip_cursor_x, feats_y, chip_w, chip_h);
            let hovered = point_in_rect([chip_cursor_x, feats_y, chip_w, chip_h], self.cursor);
            // FR-LLM-C: on → Selected (слот selected_fill), hovered → Hovered,
            // off → Normal. chip_style — слот заливки/рамки/текста, radius=RADIUS_PILL.
            let chip_state = if on {
                canvas_ui::kit::KitState::Selected
            } else if hovered {
                canvas_ui::kit::KitState::Hovered
            } else {
                canvas_ui::kit::KitState::Normal
            };
            let chip_style = canvas_ui::kit::chip_style(chip_state, &kit_palette);
            // FR-LLM-FIX: pill-радиус переопределяем (chip_style даёт RADIUS_CHIP=6;
            // прототип — 99px). Рисуем вручную через d.rect с нужным радиусом.
            d.rect(chip_rect, chip_style.fill, chip_style.border, FEAT_RADIUS);
            d.label_center(chip_rect, &label_text, chip_style.text, FEAT_FONT);
            chip_cursor_x += chip_w + FEAT_GAP;
            let _ = kind; // hit-test использует свои координаты — `kind` здесь
                          // только для порядка перечисления (см. `ai_status_panel_hit`).
        }

        // === Строка 3: costs — «Session: $X.XX · Day: $X.XX / $L.LL» =====
        let costs_y = feats_y + FEAT_ROW_H + GAP_FEATS_COST;
        let session_str = format!("${:.2}", self.ai_cost_session);
        let day_str = format!("${:.2}", self.ai_cost_day);
        let limit_str = format!("${:.2}", llm.cost_limit_daily);
        let costs_text = self.trf(keys::AI_STATUS_DAY, &[("v", &day_str), ("lim", &limit_str)]);
        let session_text = self.trf(keys::AI_STATUS_SESSION, &[("v", &session_str)]);
        let combined = format!("{} · {}", session_text, costs_text);
        d.label_left(
            UiRect::new(content_x, costs_y, content_w, COST_ROW_H),
            &combined,
            kit_palette.text_muted,
            11.0,
        );

        // === Строка 4: progress bar — day cost / limit =====================
        let progress_y = costs_y + COST_ROW_H + GAP_COST_BAR;
        let progress_w = content_w;
        // Фон прогресс-бара (слот palette_border, прозрачный).
        d.rect(
            UiRect::new(content_x, progress_y, progress_w, BAR_H),
            [
                palette.palette_border[0],
                palette.palette_border[1],
                palette.palette_border[2],
                0.5,
            ],
            [0.0; 4],
            BAR_RADIUS,
        );
        // Заполнение: доля day/limit, кламп 0..1; > 0.8 — янтарный (warning).
        let pct = if llm.cost_limit_daily > 0.0 {
            (self.ai_cost_day / llm.cost_limit_daily).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        if pct > 0.0 {
            let fill_color = if pct >= 0.8 {
                [0.95, 0.65, 0.30, 1.0] // янтарный — близко к лимиту
            } else {
                kit_palette.accent
            };
            d.rect(
                UiRect::new(content_x, progress_y, (progress_w * pct).max(2.0), BAR_H),
                fill_color,
                [0.0; 4],
                BAR_RADIUS,
            );
        }

        // FR-LLM-FIX: paused-лейбл — показывается под progress-баром когда
        // ai_paused. Янтарный текст 10px (как `.ais-paused-lb` в прототипе).
        if self.ai_paused {
            let paused_y = progress_y + BAR_H + GAP_BAR_PAUSED;
            let amber = [242.0 / 255.0, 165.0 / 255.0, 76.0 / 255.0, 1.0];
            d.label_left(
                UiRect::new(content_x, paused_y, content_w, 13.0),
                self.tr(keys::AI_STATUS_PAUSED),
                amber,
                10.0,
            );
        }

        // FR-LLM-C: дрейн KitDraw → возвращаемые Vec'и (как overlays.rs).
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

    /// FR-LLM-FIX / PRD-0010 F-7.9: rect панели AI-статуса (для hit-тестов
    /// и клип-области рендера). Учитывает paused-state (панель выше на
    /// `AI_STATUS_PAUSED_EXTRA` когда ai_paused). Возвращает `None`, если
    /// панель скрыта (узкий вьюпорт / all_off).
    pub(super) fn ai_status_panel_rect(&self) -> Option<[f32; 4]> {
        let viewport = self.viewport_logical();
        if viewport[0] < AI_STATUS_MIN_VIEWPORT_W || viewport[1] <= 0.0 {
            return None;
        }
        if self.settings.llm.all_off() {
            return None;
        }
        let panel_h = if self.ai_paused {
            AI_STATUS_H + AI_STATUS_PAUSED_EXTRA
        } else {
            AI_STATUS_H
        };
        let has_minimap = self.minimap_rect().is_some();
        let panel_x = viewport[0] - AI_STATUS_W - AI_STATUS_MARGIN;
        let panel_y = if has_minimap {
            viewport[1] - MINIMAP_MARGIN - MINIMAP_H - AI_STATUS_GAP_ABOVE_MINIMAP - panel_h
        } else {
            viewport[1] - AI_STATUS_MARGIN - panel_h
        };
        Some([panel_x, panel_y, AI_STATUS_W, panel_h])
    }

    /// FR-LLM-FIX / PRD-0010 F-7.9: hit-test панели AI-статуса — определить,
    /// по какому элементу кликнул пользователь (для обработчика ввода).
    /// Возвращает `None`, если клик мимо панели.
    pub(crate) fn ai_status_panel_hit(&self, point: [f32; 2]) -> Option<AiStatusPanelHit> {
        let panel = self.ai_status_panel_rect()?;
        if !point_in_rect(panel, point) {
            return None;
        }
        let content_x = panel[0] + PAD_X;
        let head_y = panel[1] + PAD_TOP;

        // FR-LLM-FIX: кнопки ⏸/⚙ в шапке — 24×24, правый край контента.
        let gear_x = panel[0] + panel[2] - PAD_X - HEAD_BTN_SIZE;
        let pause_x = gear_x - 6.0 - HEAD_BTN_SIZE;
        if point_in_rect([pause_x, head_y, HEAD_BTN_SIZE, HEAD_BTN_SIZE], point) {
            return Some(AiStatusPanelHit::PauseToggle);
        }
        if point_in_rect([gear_x, head_y, HEAD_BTN_SIZE, HEAD_BTN_SIZE], point) {
            return Some(AiStatusPanelHit::OpenSettings);
        }

        // FR-LLM-FIX: чипы Suggest/Graph/Agent — pill-форма, ширина по
        // тексту + padding. Перебираем в том же порядке, что и в рендере.
        let feats_y = head_y + HEAD_ROW_H + GAP_HEAD_FEATS;
        let feats: [(&str, bool); 3] = [
            ("Suggest", self.ai_suggest_enabled),
            ("Graph", self.ai_graph_enabled),
            ("Agent", self.ai_agent_enabled),
        ];
        let mut chip_cursor_x = content_x;
        for (i, (label, enabled)) in feats.iter().enumerate() {
            let on = *enabled && !self.ai_paused;
            let label_text = if on {
                format!("{} ✓", label)
            } else {
                label.to_string()
            };
            let chip_w = FEAT_PAD_X * 2.0 + label_text.len() as f32 * 5.5;
            let chip_h = FEAT_FONT + FEAT_PAD_Y * 2.0;
            if point_in_rect([chip_cursor_x, feats_y, chip_w, chip_h], point) {
                return Some(match i {
                    0 => AiStatusPanelHit::ToggleSuggest,
                    1 => AiStatusPanelHit::ToggleGraph,
                    _ => AiStatusPanelHit::ToggleAgent,
                });
            }
            chip_cursor_x += chip_w + FEAT_GAP;
        }

        // Клик мимо кнопок — клик по телу панели (не проваливается под
        // канвас); возвращаем No-op, чтобы вызывающий «съел» ввод.
        Some(AiStatusPanelHit::NoOp)
    }
}

/// FR-LLM-FIX: модель провайдера для отображения в шапке панели. Для BYOK —
/// переданный `byok_model` (model_suggest/graph/agent); для не-BYOK —
/// фиксированное имя модели провайдера (как `aiProvModel()` прототипа).
/// Возвращает пустую строку для Off (вызывающий показывает fallback-текст).
fn provider_fixed_model(provider: canvas_llm::LlmProviderId, byok_model: &str) -> String {
    match provider {
        canvas_llm::LlmProviderId::Byok => byok_model.to_owned(),
        canvas_llm::LlmProviderId::ChatGptOAuth => "gpt-5.2".to_owned(),
        canvas_llm::LlmProviderId::Ollama => "llama3.1:8b".to_owned(),
        canvas_llm::LlmProviderId::Laya => "laya-1.13".to_owned(),
        canvas_llm::LlmProviderId::Off => String::new(),
    }
}

/// FR-LLM-B / PRD-0010 F-7.9: hit-test панели AI-статуса — элемент под
/// курсором. Возвращается `App::ai_status_panel_hit` для обработчика ввода.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AiStatusPanelHit {
    /// Клик мимо активных элементов (тело панели) — ввод глотается.
    NoOp,
    /// Кнопка ⏸/▶ — переключить паузу всех AI-функций.
    PauseToggle,
    /// Кнопка ⚙ — открыть настройки → таб «AI и модели».
    OpenSettings,
    /// Чип Suggest — toggle `ai_suggest_enabled`.
    ToggleSuggest,
    /// Чип Graph — toggle `ai_graph_enabled`.
    ToggleGraph,
    /// Чип Agent — toggle `ai_agent_enabled`.
    ToggleAgent,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Панель скрыта на узком вьюпорте (< 900px, F-7.9). Без `App`
    /// (нужен GPU/scene) — проверяем только константу.
    #[test]
    fn min_viewport_w_is_900() {
        assert_eq!(AI_STATUS_MIN_VIEWPORT_W, 900.0);
    }

    /// FR-LLM-FIX: размер панели — 302px ширина (как в прототипе F-7.9).
    /// Высота — auto (92px без paused-лейбла, +18px с ним).
    #[test]
    fn panel_size_matches_prototype() {
        assert_eq!(AI_STATUS_W, 302.0);
        assert!(AI_STATUS_H >= 90.0 && AI_STATUS_H <= 100.0);
    }

    /// Зазор над миникартой — 8px (по прототипу F-7.9).
    #[test]
    fn gap_above_minimap_is_8() {
        assert_eq!(AI_STATUS_GAP_ABOVE_MINIMAP, 8.0);
    }

    /// FR-LLM-FIX: padding панели — 9px top/bottom, 11px left/right
    /// (`padding:9px 11px` в прототипе).
    #[test]
    fn padding_matches_prototype() {
        assert_eq!(PAD_TOP, 9.0);
        assert_eq!(PAD_BOTTOM, 9.0);
        assert_eq!(PAD_X, 11.0);
    }

    /// FR-LLM-FIX: радиус панели — 10px (`border-radius:10px`).
    #[test]
    fn panel_radius_matches_prototype() {
        assert_eq!(PANEL_RADIUS, 10.0);
    }

    /// FR-LLM-FIX: размер кнопок ⏸/⚙ — 24×24, радиус 6px.
    #[test]
    fn head_buttons_match_prototype() {
        assert_eq!(HEAD_BTN_SIZE, 24.0);
        assert_eq!(HEAD_BTN_RADIUS, 6.0);
    }

    /// FR-LLM-FIX: точка состояния — 8×8 (circle).
    #[test]
    fn dot_size_matches_prototype() {
        assert_eq!(DOT_SIZE, 8.0);
    }

    /// FR-LLM-FIX: прогресс-бар — 4px высота, радиус 2px.
    #[test]
    fn progress_bar_matches_prototype() {
        assert_eq!(BAR_H, 4.0);
        assert_eq!(BAR_RADIUS, 2.0);
    }

    /// FR-LLM-FIX: feature-чипы — pill-форма (RADIUS_PILL), font 10.5px,
    /// padding 2.5×9px, gap 5px.
    #[test]
    fn feature_chips_match_prototype() {
        assert_eq!(FEAT_RADIUS, canvas_core::tokens::RADIUS_PILL);
        assert_eq!(FEAT_FONT, 10.5);
        assert_eq!(FEAT_PAD_Y, 2.5);
        assert_eq!(FEAT_PAD_X, 9.0);
        assert_eq!(FEAT_GAP, 5.0);
    }

    /// FR-LLM-FIX: provider_fixed_model — фиксированные модели для не-BYOK
    /// провайдеров (как `aiProvModel()` прототипа); BYOK — переданное имя.
    #[test]
    fn provider_fixed_model_matches_prototype() {
        assert_eq!(
            provider_fixed_model(canvas_llm::LlmProviderId::ChatGptOAuth, ""),
            "gpt-5.2"
        );
        assert_eq!(
            provider_fixed_model(canvas_llm::LlmProviderId::Ollama, ""),
            "llama3.1:8b"
        );
        assert_eq!(
            provider_fixed_model(canvas_llm::LlmProviderId::Laya, ""),
            "laya-1.13"
        );
        assert_eq!(
            provider_fixed_model(canvas_llm::LlmProviderId::Byok, "deepseek-v3.2"),
            "deepseek-v3.2"
        );
        assert_eq!(provider_fixed_model(canvas_llm::LlmProviderId::Off, ""), "");
    }

    /// FR-LLM-C: радиус чипов берётся из шкалы токенов (FR-055: единый
    /// источник радиусов). Pill-форма (RADIUS_PILL) для feature-чипов.
    #[test]
    fn feature_chip_radius_uses_token() {
        assert_eq!(FEAT_RADIUS, canvas_core::tokens::RADIUS_PILL);
    }
}
