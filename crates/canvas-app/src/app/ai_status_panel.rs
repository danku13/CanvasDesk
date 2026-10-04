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
//
// FR-LLM-FIX-2 (фикс «вёрстка всё ещё кривая», скриншот владельца):
// головная строка переведена на flex-семантику CSS прототипа с ЗАМЕРОМ
// шрифта (TextMeasurer) вместо эвристики `len*5.5`:
// - `.ais-head { display:flex; gap:6px }` — порядок dot | model | prov |
//   spacer | ⏸ | ⚙, а НЕ «модель фиксированной ширины 146px» (чип
//   «BYOK (свой ключ)» уезжал в середину строки и наезжал на ⏸/⚙);
// - `.ais-prov { flex:none; padding:1px 7px }` — чип контент-ширины по
//   замеру (кириллица в 9.5px шире эвристики → текст вылезал за чип);
// - `.ais-model { overflow:hidden; text-overflow:ellipsis }` — модель
//   сжимается первой и обрезается «…» по фактической ширине;
// - короткая подпись провайдера (как `aiProvLabel()` прототипа:
//   «BYOK», а не «BYOK (свой ключ)» — полные подписи остаются в настройках);
// - feature-чипы и их клик-зоны считаются ОДНОЙ функцией замера (раньше
//   hit-test расходился с отрисованным кадром).

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

// FR-LLM-FIX-2: геометрия головной строки — flex-параметры CSS прототипа.
/// Зазор flex-строки шапки (`gap:6px` в `.ais-head`).
const HEAD_GAP: f32 = 6.0;
/// Горизонтальный паддинг чипа провайдера (`padding:1px 7px` → 7px слева/справа).
const PROV_CHIP_PAD_X: f32 = 7.0;
/// Высота чипа провайдера (`padding:1px` + font 9.5 ≈ 14px).
const PROV_CHIP_H: f32 = 14.0;
/// Клампы ширины чипа провайдера: минимум — «Off», максимум — защита от
/// длинных локалей (при превышении чип обрезается через ellipsis).
const PROV_CHIP_MIN_W: f32 = 36.0;
const PROV_CHIP_MAX_W: f32 = 120.0;

/// FR-LLM-FIX-2: результат flex-раскладки головной строки (координаты
/// относительно панели; y — уже с `head_y`).
struct HeadRow {
    /// Область текста модели — от точки+gap до чипа провайдера; текст
    /// кладётся с ellipsis по фактической ширине (`text-overflow:ellipsis`).
    model_area: UiRect,
    /// Чип провайдера — контент-ширина по замеру (`flex:none`).
    prov_chip: UiRect,
    /// x кнопки ⏸/▶ (24×24 от `head_y`).
    pause_x: f32,
    /// x кнопки ⚙.
    gear_x: f32,
}

/// FR-LLM-FIX-2: flex-раскладка головной строки ПО ЗАМЕРУ шрифта — единая
/// для рендера и hit-теста (клик-зоны идентичны кадру отрисовки).
///
/// Порядок и семантика — 1:1 с CSS прототипа (`.ais-head`): точка-индикатор
/// → модель (сжимается, ellipsis) → чип провайдера (контент-ширина) →
/// распорка → ⏸ → ⚙; `gap:6px` между соседями. Кнопки прибиты к правому
/// краю контента (как `.ais-sp { flex:1 }` + `.ais-btn { flex:none }`).
///
/// Оба текста обрезаются по фактическим областям (модель — всегда; чип —
/// только при клампе `PROV_CHIP_MAX_W`, защита от длинных локалей).
///
/// Замер — `TextMeasurer` под guard'ом `measure_font_system` в коротком
/// скоупе вызывающего (контракт «вложенный лок FontSystem запрещён»). Возвращает
/// `(раскладка, текст модели, текст чипа)` — обрезанные варианты для отрисовки.
fn head_row_layout(
    m: &mut canvas_ui::measure::TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    content_x: f32,
    content_w: f32,
    head_y: f32,
    model_text: &str,
    prov_text: &str,
) -> (HeadRow, String, String) {
    // Кнопки ⏸/⚙ — правый край контента (как раньше: 24 + gap + 24).
    let gear_x = content_x + content_w - HEAD_BTN_SIZE;
    let pause_x = gear_x - HEAD_GAP - HEAD_BTN_SIZE;
    // Чип провайдера: контент-ширина (замер + padding 7×2), клампы.
    let prov_text_w = crate::kit_ui::measured_width(m, fs, prov_text, 9.5);
    let prov_chip_w = (prov_text_w + PROV_CHIP_PAD_X * 2.0).clamp(PROV_CHIP_MIN_W, PROV_CHIP_MAX_W);
    let prov_chip = UiRect::new(
        pause_x - HEAD_GAP - prov_chip_w,
        head_y + (HEAD_ROW_H - PROV_CHIP_H) * 0.5,
        prov_chip_w,
        PROV_CHIP_H,
    );
    // Текст чипа: длинные локали обрезаем «…» до ширины чипа минус паддинг
    // (кириллица в 9.5px шире эвристики — текст не должен вылезать за чип).
    let prov_fitted = if prov_text_w + PROV_CHIP_PAD_X * 2.0 > PROV_CHIP_MAX_W {
        m.ellipsis(
            fs,
            prov_text,
            crate::kit_ui::FONT_FAMILY,
            9.5,
            (prov_chip_w - PROV_CHIP_PAD_X * 2.0).max(0.0),
        )
    } else {
        prov_text.to_owned()
    };
    // Модель: от точки (dot 8px + gap) до чипа (gap); ellipsis по факту.
    let model_x = content_x + DOT_SIZE + HEAD_GAP;
    let model_avail = (prov_chip.x - HEAD_GAP - model_x).max(0.0);
    let model_fitted = m.ellipsis(
        fs,
        model_text,
        crate::kit_ui::FONT_FAMILY,
        12.5,
        model_avail,
    );
    let model_area = UiRect::new(
        model_x,
        head_y + (HEAD_ROW_H - 12.5) * 0.5,
        model_avail,
        14.0,
    );
    (
        HeadRow {
            model_area,
            prov_chip,
            pause_x,
            gear_x,
        },
        model_fitted,
        prov_fitted,
    )
}

/// FR-LLM-FIX-2: подписи и ширины feature-чипов (Suggest/Graph/Agent) по
/// замеру шрифта — ЕДИНЫЙ источник для рендера и hit-теста. Раньше ширина
/// была `len*5.5` (эвристика) — клик-зона расходилась с отрисованным кадром.
/// Активный чип подписывается « ✓» (как `aiSyncFeats()` прототипа); на паузе
/// все чипы рисуются выключенными.
fn feat_chips_measured(
    m: &mut canvas_ui::measure::TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    suggest_on: bool,
    graph_on: bool,
    agent_on: bool,
    paused: bool,
) -> [(String, f32); 3] {
    let defs = [
        ("Suggest", suggest_on),
        ("Graph", graph_on),
        ("Agent", agent_on),
    ];
    let mut out: [(String, f32); 3] = std::array::from_fn(|_| (String::new(), 0.0f32));
    for (i, (label, enabled)) in defs.iter().enumerate() {
        let on = *enabled && !paused;
        let text = if on {
            format!("{} ✓", label)
        } else {
            (*label).to_string()
        };
        let w = FEAT_PAD_X * 2.0 + crate::kit_ui::measured_width(m, fs, &text, FEAT_FONT);
        out[i] = (text, w);
    }
    out
}

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
        // FR-LLM-FIX: пустое имя модели — «не выбрана» для активного
        // провайдера (ChatGPT/Ollama без подключения, BYOK без выбора
        // модели из /v1/models), «AI выключен» — только когда все Off.
        let model_name = if model_name.is_empty() {
            if active_provider == canvas_llm::LlmProviderId::Off {
                self.tr(keys::AI_STATUS_NO_MODEL).to_owned()
            } else {
                // FR-LLM-FIX: активный провайдер без модели — «не выбрана».
                self.tr(keys::AI_STATUS_MODEL_NOT_CHOSEN).to_owned()
            }
        } else {
            model_name.to_owned()
        };
        // FR-LLM-FIX-2: короткая подпись провайдера для чипа статусной
        // панели — как `aiProvLabel()` прототипа («BYOK», «ChatGPT OAuth»…).
        // Полные подписи («BYOK (свой ключ)») остаются в настройках.
        let provider_label = self.ai_status_prov_label(active_provider);

        // === Строка 1: head — точка + модель + провайдер + ⏸ + ⚙ ===========
        // head_y — верх строки; вертикально центрируем элементы внутри 24px.
        let content_x = panel[0] + PAD_X;
        let content_w = panel[2] - 2.0 * PAD_X;
        let head_y = panel[1] + PAD_TOP;
        // Цветная точка состояния: green = активно, amber = пауза. Слоты
        // `control_success`/`control_warning` kit-палитры (audit §3 — были
        // инлайн-литералами; рисуем через d.rect, FR-LLM-C: KitDraw-адаптер,
        // не сырой CardInstance).
        let dot_color = if self.ai_paused {
            kit_palette.control_warning // янтарный — пауза
        } else {
            kit_palette.control_success // зелёный — активно
        };
        // Точка — 8×8, вертикально по центру строки 24px (отступ 8 сверху).
        let dot_y = head_y + (HEAD_ROW_H - DOT_SIZE) * 0.5;
        d.rect(
            UiRect::new(content_x, dot_y, DOT_SIZE, DOT_SIZE),
            dot_color,
            [0.0; 4],
            DOT_SIZE / 2.0, // circle
        );
        // FR-LLM-FIX-2: flex-раскладка шапки по замеру шрифта (короткий
        // скоуп — контракт «вложенный лок FontSystem запрещён»; замер —
        // тем же семейством/весом, что и отрисовка painter'а).
        let mut m = crate::kit_ui::new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let (head, model_fitted, prov_fitted) = head_row_layout(
            &mut m,
            &mut fs,
            content_x,
            content_w,
            head_y,
            &model_name,
            &provider_label,
        );
        // Имя модели (font 12.5px, `font-weight:600` в прототипе) —
        // сжимается первой, ellipsis по фактической ширине области.
        d.label_left(head.model_area, &model_fitted, kit_palette.text_title, 12.5);
        // Подпись провайдера — pill-чип контент-ширины
        // (border-radius:99px, padding:1px 7px, font 9.5px), flex:none.
        let prov_chip_rect = head.prov_chip;
        let prov_chip_style =
            canvas_ui::kit::chip_style(canvas_ui::kit::KitState::Normal, &kit_palette);
        d.control(prov_chip_rect, &prov_chip_style);
        d.label_center(prov_chip_rect, &prov_fitted, prov_chip_style.text, 9.5);

        // FR-LLM-FIX: кнопки ⏸/⚙ — 24×24, border-radius 6px (RADIUS_CHIP).
        // Позиция — из раскладки (правый край контента, зазор 6px).
        let gear_x = head.gear_x;
        let pause_x = head.pause_x;
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
        // FR-LLM-FIX-2: подписи+ширины чипов — тот же замер, что и в
        // hit-тесте (единая функция `feat_chips_measured`).
        let feats = feat_chips_measured(
            &mut m,
            &mut fs,
            self.ai_suggest_enabled,
            self.ai_graph_enabled,
            self.ai_agent_enabled,
            self.ai_paused,
        );
        let feats_on = [
            self.ai_suggest_enabled,
            self.ai_graph_enabled,
            self.ai_agent_enabled,
        ];
        let mut chip_cursor_x = content_x;
        for (i, (label_text, chip_w)) in feats.iter().enumerate() {
            // «on» = фича включена И не на паузе (как aiSyncFeats() прототипа).
            let on = feats_on[i] && !self.ai_paused;
            let chip_h = FEAT_FONT + FEAT_PAD_Y * 2.0;
            let chip_rect = UiRect::new(chip_cursor_x, feats_y, *chip_w, chip_h);
            let hovered = point_in_rect([chip_cursor_x, feats_y, *chip_w, chip_h], self.cursor);
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
            d.label_center(chip_rect, label_text, chip_style.text, FEAT_FONT);
            chip_cursor_x += *chip_w + FEAT_GAP;
            // hit-тест — та же геометрия (см. `ai_status_panel_hit`);
            // порядок чипов = порядок ToggleSuggest/Graph/Agent в hit-enum.
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
                kit_palette.control_warning // янтарный — близко к лимиту
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
        // ai_paused. Янтарный текст 10px (как `.ais-paused-lb` в прототипе),
        // через слот `control_warning` (audit §3 — был инлайн-литералом
        // `[242/255, 165/255, 76/255, 1.0]` = каноническое amber-значение
        // слота).
        if self.ai_paused {
            let paused_y = progress_y + BAR_H + GAP_BAR_PAUSED;
            let amber = kit_palette.control_warning;
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

    /// FR-LLM-FIX-2: короткая подпись провайдера для чипа статусной панели —
    /// паритет с `aiProvLabel()` прототипа ({byok:'BYOK', chatgpt:'ChatGPT
    /// OAuth', ollama:'Ollama', laya:'Laya (local)', off:'Off'}). Полные
    /// подписи настроек («BYOK (свой ключ)», «ChatGPT (вход)») — только в
    /// 9-м табе; в шапке панели они раздували чип и наезжали на кнопки.
    fn ai_status_prov_label(&self, p: canvas_llm::LlmProviderId) -> String {
        let key = match p {
            canvas_llm::LlmProviderId::Off => keys::AI_PROV_SHORT_OFF,
            canvas_llm::LlmProviderId::Laya => keys::AI_PROV_SHORT_LAYA,
            canvas_llm::LlmProviderId::Ollama => keys::AI_PROV_SHORT_OLLAMA,
            canvas_llm::LlmProviderId::Byok => keys::AI_PROV_SHORT_BYOK,
            canvas_llm::LlmProviderId::ChatGptOAuth => keys::AI_PROV_SHORT_CHATGPT,
        };
        self.tr(key).to_owned()
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

        // FR-LLM-FIX: кнопки ⏸/⚙ в шапке — 24×24, правый край контента
        // (та же математика, что в `head_row_layout`).
        let gear_x = panel[0] + panel[2] - PAD_X - HEAD_BTN_SIZE;
        let pause_x = gear_x - HEAD_GAP - HEAD_BTN_SIZE;
        if point_in_rect([pause_x, head_y, HEAD_BTN_SIZE, HEAD_BTN_SIZE], point) {
            return Some(AiStatusPanelHit::PauseToggle);
        }
        if point_in_rect([gear_x, head_y, HEAD_BTN_SIZE, HEAD_BTN_SIZE], point) {
            return Some(AiStatusPanelHit::OpenSettings);
        }

        // FR-LLM-FIX-2: чипы Suggest/Graph/Agent — тот же замер шрифта, что
        // и в рендере (`feat_chips_measured`): клик-зона = отрисованный кадр.
        let feats_y = head_y + HEAD_ROW_H + GAP_HEAD_FEATS;
        let mut m = crate::kit_ui::new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let feats = feat_chips_measured(
            &mut m,
            &mut fs,
            self.ai_suggest_enabled,
            self.ai_graph_enabled,
            self.ai_agent_enabled,
            self.ai_paused,
        );
        let mut chip_cursor_x = content_x;
        for (i, (_label_text, chip_w)) in feats.iter().enumerate() {
            let chip_h = FEAT_FONT + FEAT_PAD_Y * 2.0;
            if point_in_rect([chip_cursor_x, feats_y, *chip_w, chip_h], point) {
                return Some(match i {
                    0 => AiStatusPanelHit::ToggleSuggest,
                    1 => AiStatusPanelHit::ToggleGraph,
                    _ => AiStatusPanelHit::ToggleAgent,
                });
            }
            chip_cursor_x += *chip_w + FEAT_GAP;
        }

        // Клик мимо кнопок — клик по телу панели (не проваливается под
        // канвас); возвращаем No-op, чтобы вызывающий «съел» ввод.
        Some(AiStatusPanelHit::NoOp)
    }
}

/// FR-LLM-FIX: модель провайдера для отображения в шапке панели. Для BYOK —
/// переданный `byok_model` (model_suggest/graph/agent); для не-BYOK —
/// фиксированное имя модели провайдера (как `aiProvModel()` прототипа).
/// Возвращает пустую строку для Off и для не-BYOK провайдеров без хардкода
/// (ChatGPT/Ollama — список моделей подгружается из `/v1/models` после
/// OAuth/подключения; Laya — single-model sidecar «laya-1.13»).
fn provider_fixed_model(provider: canvas_llm::LlmProviderId, byok_model: &str) -> String {
    match provider {
        canvas_llm::LlmProviderId::Byok => byok_model.to_owned(),
        // FR-LLM-FIX: хардкоды «gpt-5.2» и «llama3.1:8b» удалены — модель
        // подгружается из `/v1/models` после OAuth/подключения. До подключения
        // — пустая строка (статусная панель показывает «не выбрана»).
        canvas_llm::LlmProviderId::ChatGptOAuth => String::new(),
        canvas_llm::LlmProviderId::Ollama => String::new(),
        // Laya — single-model sidecar; «laya-1.13» не хардкод, а идентификатор
        // единственной модели этого провайдера.
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
        // const-блок: константный диапазон — ловится на компиляции
        // (clippy::assertions_on_constants, CI 2026-10-04).
        const { assert!(AI_STATUS_H >= 90.0 && AI_STATUS_H <= 100.0) }
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

    /// FR-LLM-FIX: provider_fixed_model — модель провайдера для шапки панели.
    /// Раньше возвращала хардкоды «gpt-5.2»/«llama3.1:8b» для ChatGPT/Ollama;
    /// теперь возвращает пустую строку (модель подгружается из /v1/models).
    /// Laya — single-model sidecar «laya-1.13»; BYOK — переданное имя; Off — пусто.
    #[test]
    fn provider_fixed_model_matches_prototype() {
        // FR-LLM-FIX: хардкоды gpt-5.2/llama3.1:8b удалены.
        assert_eq!(
            provider_fixed_model(canvas_llm::LlmProviderId::ChatGptOAuth, ""),
            ""
        );
        assert_eq!(
            provider_fixed_model(canvas_llm::LlmProviderId::Ollama, ""),
            ""
        );
        // Laya — single-model sidecar; «laya-1.13» не хардкод, а
        // идентификатор единственной модели этого провайдера.
        assert_eq!(
            provider_fixed_model(canvas_llm::LlmProviderId::Laya, ""),
            "laya-1.13"
        );
        // BYOK — переданное имя модели (model_suggest/graph/agent).
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

    /// FR-LLM-FIX-2: flex-раскладка шапки — элементы НЕ пересекаются на
    /// самых широких комбинациях (кириллица + длинная модель + длинный чип).
    /// Регрессия на скриншот владельца: чип «BYOK (свой ключ)» (ширина по
    /// эвристике len*5.5) наезжал на ⏸/⚙, модель стояла фиксированной
    /// шириной в середине строки. Замер — реальным шрифтом кита.
    #[test]
    fn head_row_no_overlap_with_long_labels() {
        let mut m = crate::kit_ui::new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let content_w = AI_STATUS_W - 2.0 * PAD_X; // 280
        let cases = [
            ("не выбрана", "BYOK"),
            ("glm-5.3-flash", "ChatGPT OAuth"),
            // Длинная модель — обязана сжаться ellipsis'ом, не вытеснить чип.
            ("nemotron-super:free — $0", "ChatGPT OAuth"),
            ("laya-1.13", "Laya (local)"),
            ("qwen2.5-72b", "Self-hosted"),
        ];
        for (model, prov) in cases {
            let (head, fitted, prov_fitted) =
                head_row_layout(&mut m, &mut fs, 0.0, content_w, 0.0, model, prov);
            // Кнопки ⏸/⚙ — внутри контента.
            assert!(head.gear_x + HEAD_BTN_SIZE <= content_w + 0.01, "gear out");
            assert!(head.pause_x > head.gear_x - HEAD_GAP - HEAD_BTN_SIZE - 0.01);
            // Чип левее кнопок (gap), модель левее чипа (gap) — нет наездов.
            assert!(
                head.prov_chip.x + head.prov_chip.w <= head.pause_x + 0.01,
                "prov chip {prov} overlaps pause btn"
            );
            assert!(
                head.model_area.x + head.model_area.w <= head.prov_chip.x + 0.01,
                "model area overlaps prov chip ({model} / {prov})"
            );
            // Обрезанный текст модели помещается в свою область.
            let fitted_w = m.width_of(&mut fs, &fitted, crate::kit_ui::FONT_FAMILY, 12.5);
            assert!(
                fitted_w <= head.model_area.w + 1.0,
                "model '{fitted}' wider than area"
            );
            // ОТРИСОВАННЫЙ текст чипа (с ellipsis при клампе) помещается В чип
            // — кириллица не вылезает за границы чипа.
            let prov_fitted_w = m.width_of(&mut fs, &prov_fitted, crate::kit_ui::FONT_FAMILY, 9.5);
            assert!(
                prov_fitted_w + 2.0 <= head.prov_chip.w,
                "prov '{prov_fitted}' wider than chip"
            );
        }
    }

    /// FR-LLM-FIX-2: длинная локаль чипа (кламп 120px) — текст обрезается
    /// «…» и остаётся внутри чипа (защита от будущих переводов).
    #[test]
    fn head_row_prov_chip_ellipsizes_overlong_label() {
        let mut m = crate::kit_ui::new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let content_w = AI_STATUS_W - 2.0 * PAD_X;
        let long = "очень длинная подпись провайдера в локали";
        let (head, _, prov_fitted) =
            head_row_layout(&mut m, &mut fs, 0.0, content_w, 0.0, "glm-5.3-flash", long);
        // Чип сжат до максимума, текст — внутри (с «…»).
        assert_eq!(head.prov_chip.w, PROV_CHIP_MAX_W);
        assert!(prov_fitted != long, "overlong label must be ellipsized");
        assert!(prov_fitted.ends_with('\u{2026}'));
        let fitted_w = m.width_of(&mut fs, &prov_fitted, crate::kit_ui::FONT_FAMILY, 9.5);
        assert!(fitted_w + 2.0 <= head.prov_chip.w);
    }

    /// FR-LLM-FIX-2: короткая модель не съедает строку — при коротких
    /// текстах распорка остаётся (чип+кнопки не растягиваются на всю ширину).
    #[test]
    fn head_row_keeps_spacer_for_short_labels() {
        let mut m = crate::kit_ui::new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let content_w = AI_STATUS_W - 2.0 * PAD_X;
        let (head, _, _) = head_row_layout(
            &mut m,
            &mut fs,
            0.0,
            content_w,
            0.0,
            "glm-5.3-flash",
            "BYOK",
        );
        // model_area меньше доступного (нет растяжения) — CSS: модель
        // content-width, распорка `.ais-sp` занимает остаток.
        let model_w = m.width_of(&mut fs, "glm-5.3-flash", crate::kit_ui::FONT_FAMILY, 12.5);
        assert!(head.model_area.w < content_w - HEAD_BTN_SIZE * 2.0 - 40.0);
        assert!(model_w <= head.model_area.w + 1.0);
    }

    /// FR-LLM-FIX-2: feature-чипы — замер вместо эвристики len*5.5; три чипа
    /// (все активны, с « ✓») помещаются в строку контента с зазорами.
    #[test]
    fn feat_chips_fit_content_row() {
        let mut m = crate::kit_ui::new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let content_w = AI_STATUS_W - 2.0 * PAD_X;
        let chips = feat_chips_measured(&mut m, &mut fs, true, true, true, false);
        let total: f32 = chips.iter().map(|(_, w)| *w).sum::<f32>() + FEAT_GAP * 2.0;
        assert!(
            total <= content_w,
            "chips row {total} exceeds content {content_w}"
        );
        // Активные чипы подписаны « ✓» (как aiSyncFeats() прототипа).
        assert_eq!(chips[0].0, "Suggest ✓");
        assert_eq!(chips[2].0, "Agent ✓");
        // Пауза — все чипы выключенные (без галочки).
        let paused = feat_chips_measured(&mut m, &mut fs, true, true, true, true);
        assert_eq!(paused[0].0, "Suggest");
        assert_eq!(paused[2].0, "Agent");
        // Замер не уже эвристики для кириллических глифов « ✓» — ширины
        // положительны и разумны (чип не схлопнулся, не распух).
        for (_, w) in chips.iter() {
            assert!(*w > FEAT_PAD_X * 2.0 && *w < content_w);
        }
    }
}
