#![allow(dead_code)] // FR-LLM-B: hit-test + constants used by Stream C/D integration
//! FR-LLM-B / PRD-0010 F-7.9 (Q4): AI status panel — правый нижний угол,
//! над миникартой. Показывает активную модель+провайдера, тумблеры
//! Suggest/Graph/Agent, cost за сессию/день (с прогресс-баром дневного
//! лимита), кнопки ⏸ (пауза всех AI) и ⚙ (открыть таб «AI и модели»).
//!
//! **Скрытие** (F-7.9): на узких окнах (viewport < 900px) панель не
//! рисуется — экономим место под канвас; при `LlmSettings::all_off()`
//! тоже скрыта (AI выключен → нечего показывать).
//!
//! **Расположение** (прототип строки 900-930): правый край, над миникартой
//! (миникарта — 220×140, 16px отступ; панель — 220×90, 8px зазор над
//! миникартой). Ширина = ширина миникарты (220), выравнивание по правому
//! краю.
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

/// Ширина панели = ширина миникарты (220px) — выравнивание по правому краю.
pub const AI_STATUS_W: f32 = 220.0;
/// Высота панели: 4 строки (head / features / costs / progress) ≈ 90px.
pub const AI_STATUS_H: f32 = 90.0;
/// Зазор над миникартой (прототип: 8px между панелью и миникартой).
pub const AI_STATUS_GAP_ABOVE_MINIMAP: f32 = 8.0;
/// Отступ от правого края = отступ миникарты (16px).
pub const AI_STATUS_MARGIN: f32 = 16.0;
/// Минимальная ширина вьюпорта для показа панели (F-7.9: < 900px — скрыта).
pub const AI_STATUS_MIN_VIEWPORT_W: f32 = 900.0;
/// Размер миникарты (зеркало `canvas_render::minimap::MINIMAP_W/H`).
const MINIMAP_W: f32 = 220.0;
const MINIMAP_H: f32 = 140.0;
const MINIMAP_MARGIN: f32 = 16.0;

/// Высота строк панели (head/features/costs/progress-bar).
const ROW_H: f32 = 22.0;
/// Размер иконок-кнопок ⏸/⚙ в шапке.
const HEAD_BTN_SIZE: f32 = 18.0;
/// Радиус маленьких кнопок-чипов (features / ⏸/⚙). Совпадает с
/// `RADIUS_CHIP` из canvas_core::tokens — раньше дублировали константу;
/// теперь берём напрямую из токенов (FR-055: шкала токенов — единый
/// источник радиусов).
const CHIP_RADIUS: f32 = canvas_core::tokens::RADIUS_CHIP;
/// Высота прогресс-бара дневного лимита.
const PROGRESS_H: f32 = 4.0;

impl App {
    /// FR-LLM-B / PRD-0010 F-7.9 (Q4): статусная панель AI — квады + тексты.
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
    /// Иначе — рендерит 4 строки:
    /// 1. **head**: цветная точка + имя модели + подпись провайдера + ⏸ + ⚙.
    /// 2. **features**: 3 чипа-тумблера Suggest/Graph/Agent (✓ = включён).
    /// 3. **costs**: «Session: $X.XX · Day: $X.XX / $L.LL».
    /// 4. **progress**: горизонтальный прогресс-бар day/limit.
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

        // Позиция панели: правый край, над миникартой (если миникарта есть)
        // или просто в правом нижнем углу.
        let has_minimap = self.minimap_rect().is_some();
        let panel_x = viewport[0] - AI_STATUS_W - AI_STATUS_MARGIN;
        let panel_y = if has_minimap {
            // Над миникартой (8px зазор).
            viewport[1] - MINIMAP_MARGIN - MINIMAP_H - AI_STATUS_GAP_ABOVE_MINIMAP - AI_STATUS_H
        } else {
            // Миникарты нет — панель в правом нижнем углу с отступом.
            viewport[1] - AI_STATUS_MARGIN - AI_STATUS_H
        };
        let panel = [panel_x, panel_y, AI_STATUS_W, AI_STATUS_H];

        // === Фон панели — kit::panel_style (FR-055: слоты panel_*) ======
        let panel_style = canvas_ui::kit::panel_style(&kit_palette);
        d.rect(
            UiRect::new(panel[0], panel[1], panel[2], panel[3]),
            panel_style.fill,
            panel_style.border,
            panel_style.radius,
        );

        // Активная модель + провайдер: приоритет suggest > graph > agent
        // (как в прототипе — `aiSetActive('suggest')` по умолчанию).
        let (active_provider, active_label_key) = [
            (llm.provider_suggest, "Suggest"),
            (llm.provider_graph, "Graph"),
            (llm.provider_agent, "Agent"),
        ]
        .into_iter()
        .find(|(p, _)| *p != canvas_llm::LlmProviderId::Off)
        .unwrap_or((canvas_llm::LlmProviderId::Off, "AI"));
        let model_name = if llm.model.is_empty() {
            // Модель не выбрана — показываем подпись провайдера.
            self.tr(keys::AI_STATUS_NO_MODEL).to_owned()
        } else {
            llm.model.clone()
        };
        let provider_label =
            crate::settings_ui::ai_provider_label(self.settings.language, active_provider);

        // === Строка 1: head — точка + модель + провайдер + ⏸ + ⚙ ===========
        let head_y = panel[1] + 6.0;
        // Цветная точка состояния: green = активно, yellow = пауза, gray = Off.
        // Не kit-слот (индикатор состояния — семантика, а не тема); рисуем
        // через d.rect (FR-LLM-C: KitDraw-адаптер, не сырой CardInstance).
        let dot_color = if self.ai_paused {
            [0.95, 0.65, 0.30, 1.0] // янтарный — пауза
        } else {
            [0.30, 0.75, 0.55, 1.0] // зелёный — активно
        };
        d.rect(
            UiRect::new(panel[0] + 10.0, head_y + 6.0, 6.0, 6.0),
            dot_color,
            [0.0; 4],
            3.0,
        );
        // Имя модели (max ~120px, чтобы не наезжать на кнопки).
        d.label_left(
            UiRect::new(panel[0] + 22.0, head_y + 1.0, 120.0, 14.0),
            &model_name,
            kit_palette.text_title,
            11.0,
        );
        // Подпись провайдера (BYOK / ChatGPT / Ollama / Laya) — kit-чип.
        let prov_chip_w = 60.0;
        let prov_chip_x = panel[0] + 22.0 + 120.0 + 4.0;
        let prov_chip_rect = UiRect::new(prov_chip_x, head_y + 2.0, prov_chip_w, 14.0);
        let prov_chip_style =
            canvas_ui::kit::chip_style(canvas_ui::kit::KitState::Normal, &kit_palette);
        d.control(prov_chip_rect, &prov_chip_style);
        d.label_center(prov_chip_rect, &provider_label, prov_chip_style.text, 10.0);

        // Кнопки ⏸/⚙ — kit::icon_button_style (Ghost-вариант). Позиция
        // остаётся как в прототипе (HEAD_BTN_SIZE=18); kit::icon_button_rect
        // не используем, т.к. он уозвращает квадрат ICON_BUTTON_SIZE=26
        // (FR-LLM-C: SAME layout — приоритет над унификацией размера).
        let gear_x = panel[0] + panel[2] - 10.0 - HEAD_BTN_SIZE;
        let pause_x = gear_x - 4.0 - HEAD_BTN_SIZE;
        let pause_glyph = if self.ai_paused { "▶" } else { "⏸" };
        let pause_hovered = point_in_rect(
            [pause_x, head_y + 2.0, HEAD_BTN_SIZE, HEAD_BTN_SIZE],
            self.cursor,
        );
        let gear_hovered = point_in_rect(
            [gear_x, head_y + 2.0, HEAD_BTN_SIZE, HEAD_BTN_SIZE],
            self.cursor,
        );
        // (btn_x, glyph, hovered, icon_name) — icon_name="" → глиф через
        // label_center; иначе — d.icon (SVG-атлас + glyph fallback).
        for (btn_x, glyph, hovered, icon_name) in [
            (pause_x, pause_glyph, pause_hovered, ""),
            (gear_x, "⚙", gear_hovered, "gear"),
        ] {
            let btn_rect = UiRect::new(btn_x, head_y + 2.0, HEAD_BTN_SIZE, HEAD_BTN_SIZE);
            let btn_state = if hovered {
                canvas_ui::kit::KitState::Hovered
            } else {
                canvas_ui::kit::KitState::Normal
            };
            let btn_style = canvas_ui::kit::icon_button_style(btn_state, &kit_palette);
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

        // === Строка 2: features — 3 чипа-тумблера Suggest/Graph/Agent =====
        let feats_y = panel[1] + ROW_H + 8.0;
        let feat_w = (panel[2] - 16.0 - 8.0) / 3.0; // 3 чипа с зазорами 4px
        let feat_gap = 4.0;
        let feats: [(canvas_llm::LlmProviderId, &str, &'static str); 3] = [
            (llm.provider_suggest, "Suggest", keys::AI_TOOLTIP_SUGGEST),
            (llm.provider_graph, "Graph", keys::AI_TOOLTIP_GRAPH),
            (llm.provider_agent, "Agent", keys::AI_TOOLTIP_AGENT),
        ];
        for (i, (prov, label, _tip_key)) in feats.iter().enumerate() {
            let chip_x = panel[0] + 8.0 + i as f32 * (feat_w + feat_gap);
            let chip_rect = UiRect::new(chip_x, feats_y, feat_w, 16.0);
            let on = *prov != canvas_llm::LlmProviderId::Off;
            let hovered = point_in_rect([chip_x, feats_y, feat_w, 16.0], self.cursor);
            // FR-LLM-C: on → Selected (слот selected_fill), hovered → Hovered,
            // off → Normal. chip_style — слот заливки/рамки/текста, radius=RADIUS_CHIP.
            let chip_state = if on {
                canvas_ui::kit::KitState::Selected
            } else if hovered {
                canvas_ui::kit::KitState::Hovered
            } else {
                canvas_ui::kit::KitState::Normal
            };
            let chip_style = canvas_ui::kit::chip_style(chip_state, &kit_palette);
            d.control(chip_rect, &chip_style);
            // Лейбл «Suggest ✓» / «Suggest» (✓ — признак включённого).
            let label_text = if on {
                format!("{} ✓", label)
            } else {
                label.to_string()
            };
            d.label_center(chip_rect, &label_text, chip_style.text, 10.0);
        }

        // === Строка 3: costs — «Session: $X.XX · Day: $X.XX / $L.LL» =====
        let costs_y = panel[1] + ROW_H * 2.0 + 10.0;
        let session_str = format!("${:.2}", self.ai_cost_session);
        let day_str = format!("${:.2}", self.ai_cost_day);
        let limit_str = format!("${:.2}", llm.cost_limit_daily);
        let costs_text = self.trf(keys::AI_STATUS_DAY, &[("v", &day_str), ("lim", &limit_str)]);
        let session_text = self.trf(keys::AI_STATUS_SESSION, &[("v", &session_str)]);
        d.label_left(
            UiRect::new(panel[0] + 10.0, costs_y, panel[2] - 20.0, 14.0),
            &format!("{} · {}", session_text, costs_text),
            kit_palette.text,
            10.0,
        );

        // === Строка 4: progress bar — day cost / limit =====================
        let progress_y = panel[1] + ROW_H * 3.0 + 12.0;
        let progress_w = panel[2] - 20.0;
        // Фон прогресс-бара (слот palette_border, прозрачный).
        d.rect(
            UiRect::new(panel[0] + 10.0, progress_y, progress_w, PROGRESS_H),
            [
                palette.palette_border[0],
                palette.palette_border[1],
                palette.palette_border[2],
                0.5,
            ],
            [0.0; 4],
            PROGRESS_H / 2.0,
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
                UiRect::new(
                    panel[0] + 10.0,
                    progress_y,
                    (progress_w * pct).max(2.0),
                    PROGRESS_H,
                ),
                fill_color,
                [0.0; 4],
                PROGRESS_H / 2.0,
            );
        }
        // Подпись «AI на паузе» под шапкой — если пауза активна.
        if self.ai_paused {
            // FR-LLM-C: янтарный #F2A54C → [f32; 4] (sRGBA, для KitDraw).
            let amber = [242.0 / 255.0, 165.0 / 255.0, 76.0 / 255.0, 1.0];
            d.label_left(
                UiRect::new(
                    panel[0] + 10.0,
                    panel[1] + ROW_H + 4.0,
                    panel[2] - 20.0,
                    14.0,
                ),
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

    /// FR-LLM-B / PRD-0010 F-7.9: rect панели AI-статуса (для hit-тестов).
    /// Возвращает `None`, если панель скрыта (узкий вьюпорт / all_off).
    pub(super) fn ai_status_panel_rect(&self) -> Option<[f32; 4]> {
        let viewport = self.viewport_logical();
        if viewport[0] < AI_STATUS_MIN_VIEWPORT_W || viewport[1] <= 0.0 {
            return None;
        }
        if self.settings.llm.all_off() {
            return None;
        }
        let has_minimap = self.minimap_rect().is_some();
        let panel_x = viewport[0] - AI_STATUS_W - AI_STATUS_MARGIN;
        let panel_y = if has_minimap {
            viewport[1] - MINIMAP_MARGIN - MINIMAP_H - AI_STATUS_GAP_ABOVE_MINIMAP - AI_STATUS_H
        } else {
            viewport[1] - AI_STATUS_MARGIN - AI_STATUS_H
        };
        Some([panel_x, panel_y, AI_STATUS_W, AI_STATUS_H])
    }

    /// FR-LLM-B / PRD-0010 F-7.9: hit-test панели AI-статуса — определить,
    /// по какому элементу кликнул пользователь (для обработчика ввода).
    /// Возвращает `None`, если клик мимо панели.
    pub(crate) fn ai_status_panel_hit(&self, point: [f32; 2]) -> Option<AiStatusPanelHit> {
        let panel = self.ai_status_panel_rect()?;
        if !point_in_rect(panel, point) {
            return None;
        }
        // Кнопки ⏸/⚙ в шапке (правый край, y = panel.y + 6..24).
        let head_y = panel[1] + 6.0;
        let gear_x = panel[0] + panel[2] - 10.0 - HEAD_BTN_SIZE;
        let pause_x = gear_x - 4.0 - HEAD_BTN_SIZE;
        if point_in_rect([pause_x, head_y + 2.0, HEAD_BTN_SIZE, HEAD_BTN_SIZE], point) {
            return Some(AiStatusPanelHit::PauseToggle);
        }
        if point_in_rect([gear_x, head_y + 2.0, HEAD_BTN_SIZE, HEAD_BTN_SIZE], point) {
            return Some(AiStatusPanelHit::OpenSettings);
        }
        // Чипы Suggest/Graph/Agent (строка 2).
        let feats_y = panel[1] + ROW_H + 8.0;
        let feat_w = (panel[2] - 16.0 - 8.0) / 3.0;
        let feat_gap = 4.0;
        for (i, kind) in [
            AiStatusPanelHit::ToggleSuggest,
            AiStatusPanelHit::ToggleGraph,
            AiStatusPanelHit::ToggleAgent,
        ]
        .iter()
        .enumerate()
        {
            let chip_x = panel[0] + 8.0 + i as f32 * (feat_w + feat_gap);
            if point_in_rect([chip_x, feats_y, feat_w, 16.0], point) {
                return Some(*kind);
            }
        }
        // Клик мимо кнопок — клик по телу панели (не проваливается под
        // канвас); возвращаем No-op, чтобы вызывающий «съел» ввод.
        Some(AiStatusPanelHit::NoOp)
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
    /// Чип Suggest — toggle провайдера suggest (Off ↔ последний активный).
    ToggleSuggest,
    /// Чип Graph — toggle провайдера graph.
    ToggleGraph,
    /// Чип Agent — toggle провайдера agent.
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

    /// Размер панели: 220×90 (как в прототипе F-7.9).
    #[test]
    fn panel_size_matches_prototype() {
        assert_eq!(AI_STATUS_W, 220.0);
        assert_eq!(AI_STATUS_H, 90.0);
    }

    /// Зазор над миникартой — 8px (по прототипу F-7.9).
    #[test]
    fn gap_above_minimap_is_8() {
        assert_eq!(AI_STATUS_GAP_ABOVE_MINIMAP, 8.0);
    }

    /// Ширина панели = ширина миникарты (выравнивание по правому краю).
    #[test]
    fn panel_width_equals_minimap_width() {
        assert_eq!(AI_STATUS_W, MINIMAP_W);
    }

    /// FR-LLM-C: радиус чипов берётся из шкалы токенов (FR-055: единый
    /// источник радиусов; раньше дублировали PANEL_RADIUS=8 / CHIP_RADIUS=4).
    /// Теперь CHIP_RADIUS = canvas_core::tokens::RADIUS_CHIP = 6.0.
    #[test]
    fn chip_radius_uses_token() {
        assert_eq!(CHIP_RADIUS, canvas_core::tokens::RADIUS_CHIP);
    }
}
