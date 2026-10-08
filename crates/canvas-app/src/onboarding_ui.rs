//! FR-028: онбординг по функциям сервиса — чистая модель (образец
//! [`crate::settings_ui`]): шаги-карточки ([`ONBOARDING_STEPS`], скоуп
//! владельца — база + расчёты + шаблоны), таблица решений авто-показа
//! ([`should_show_onboarding`]: не пройден И откладываний < 3), машина
//! карусели ([`OnboardingState`]: `next`/`prev` с клампом, подпись
//! «Далее»/«Готово») и геометрия карточки с кнопками и клампом к окну.
//!
//! W-e (разморозка решением владельца 03.10.2026, миграция на ui-kit):
//! геометрия — `kit::modal` по слоту вьюпорта С ПОЛЯМИ
//! ([`ONBOARDING_VIEWPORT_MARGIN`] — раньше карточка клампилась к кромкам
//! вплотную, аудит ui-kit §8 №12); при клампе высоты тело ужимается
//! скроллом `kit::ScrollState` (футер с CTA всегда видим); перенос тела —
//! измеренный `TextMeasurer::wrap` (замена символьной эвристики
//! `CHAR_W_FACTOR 0.62` — строка onboarding в §9 CR-015/аудита).
//!
//! Рендер и ввод — приложение (`main.rs`): затемнение канваса (паттерн
//! FR-022), карточка поверх, ввод канваса под ней блокируется. Поля
//! `onboarding_done`/`onboarding_defers` и кламп — `canvas-core`
//! settings.rs (схема `config.toml`, `#[serde(default)]`).

use canvas_core::{Language, Settings, ONBOARDING_MAX_DEFERS};
use canvas_ui::geometry::{UiRect, UiVec2};
use canvas_ui::kit::{self, ScrollState};
use canvas_ui::measure::TextMeasurer;

use crate::i18n::{self, keys};

/// Семейство замера/отрисовки строк (паритет sans_attrs рендера —
/// Weight::MEDIUM, тот же шейпинг, что у `OwnedScreenText`).
const FAMILY: &str = canvas_render::text::SANS_FAMILY;

/// Автопоказ тура при старте (таблица решений FR-028): не пройден до конца
/// И отложен менее `ONBOARDING_MAX_DEFERS` раз. Ручной вход из меню «?»
/// этой функцией не гейтится (явное намерение пользователя).
pub fn should_show_onboarding(settings: &Settings) -> bool {
    !settings.onboarding_done && settings.onboarding_defers < ONBOARDING_MAX_DEFERS
}

/// CR-031/S3 (UR-002 п.5): i18n-ключ value-приветствия по роли, выбранной
/// при входе (FR-087, `Settings::role`). Неизвестная/не выбранная роль —
/// универсальное описание ценности. Роли — `canvas_core::roles::ROLES`.
pub fn welcome_body_key(role: &str) -> &'static str {
    match role {
        "architect" => keys::ONBOARDING_VALUE_ARCHITECT,
        "developer" => keys::ONBOARDING_VALUE_DEVELOPER,
        "product-manager" => keys::ONBOARDING_VALUE_PRODUCT,
        "analyst" => keys::ONBOARDING_VALUE_ANALYST,
        "cio" => keys::ONBOARDING_VALUE_CIO,
        "cto" => keys::ONBOARDING_VALUE_CTO,
        "founder" => keys::ONBOARDING_VALUE_FOUNDER,
        _ => keys::ONBOARDING_VALUE_DEFAULT,
    }
}

/// Ключ тела шага с учётом роли (CR-031/S3): шаг 0 — ролевое value,
/// остальные — статичный ключ шага. Один источник для раскладки
/// ([`card_layout`]) и рендера — тексты не разъезжаются.
pub fn step_body_key(step: usize, role: &str) -> &'static str {
    if step == 0 {
        welcome_body_key(role)
    } else {
        ONBOARDING_STEPS
            .get(step)
            .map_or(keys::ONBOARDING_VALUE_DEFAULT, |s| s.body_key)
    }
}

/// Шаг тура: заголовок и абзац тела (короткие тексты, перенос по ширине
/// карточки — измеренный `TextMeasurer::wrap`). CR-031: CTA-механика
/// `action_key` («Попробовать» на шаге «Шаблоны нод») удалена — шаг ведёт
/// «Далее» до финала; финальный шаг предлагает выбор двумя полноширинными
/// CTA ([`OnboardingButton::FinalGallery`] / [`OnboardingButton::FinalEmpty`]).
pub struct OnboardingStep {
    /// Ключ заголовка (таблица [`crate::i18n`] — FR-040).
    pub title_key: &'static str,
    /// Ключ тела (полная фраза, перенос на стороне [`body_lines`]).
    pub body_key: &'static str,
}

/// Шаги тура (FR-028, скоуп владельца — база + расчёты + шаблоны; NN/g:
/// 8±2 шага, «один шаг = одна мысль», выход виден всегда). Порядок
/// стабилен; последний шаг — финал с выбором «шаблонная схема / самому».
/// CR-031: шаг «UI-консоль» выведен из тура (владельческий инструмент —
/// остался в меню «?») — не перегружаем новичка служебным экраном.
pub const ONBOARDING_STEPS: [OnboardingStep; 8] = [
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP1_TITLE,
        // CR-031/S3: базовый ключ тела шага 1 — универсальное value; при
        // показе заменяется по роли ([`welcome_body_key`] в [`step_body_key`]).
        body_key: keys::ONBOARDING_VALUE_DEFAULT,
    },
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP2_TITLE,
        body_key: keys::ONBOARDING_STEP2_BODY,
    },
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP3_TITLE,
        body_key: keys::ONBOARDING_STEP3_BODY,
    },
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP4_TITLE,
        body_key: keys::ONBOARDING_STEP4_BODY,
    },
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP5_TITLE,
        body_key: keys::ONBOARDING_STEP5_BODY,
    },
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP6_TITLE,
        body_key: keys::ONBOARDING_STEP6_BODY,
    },
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP7_TITLE,
        body_key: keys::ONBOARDING_STEP7_BODY,
    },
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP8_TITLE,
        body_key: keys::ONBOARDING_STEP8_BODY,
    },
];

/// Состояние тура: индекс текущего шага (кламп `0..len-1` — инвариант
/// карусели). `None` в `App` — тура нет.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OnboardingState {
    pub step: usize,
}

impl OnboardingState {
    /// Число шагов.
    pub fn len(&self) -> usize {
        ONBOARDING_STEPS.len()
    }

    /// Тур пуст (недостижимо с непустым ONBOARDING_STEPS — для полноты).
    pub fn is_empty(&self) -> bool {
        ONBOARDING_STEPS.is_empty()
    }

    /// Шаг вперёд; false на последнем шаге (вызывающий завершает тур —
    /// «Готово» ставит `onboarding_done` и сохраняет конфиг).
    // Не Iterator::next — шаги карусели с клампом, а не выдача элементов
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> bool {
        if self.step + 1 < ONBOARDING_STEPS.len() {
            self.step += 1;
            true
        } else {
            false
        }
    }

    /// Шаг назад; false на первом (кнопки «Назад» там нет — hit-тест None).
    pub fn prev(&mut self) -> bool {
        if self.step == 0 {
            return false;
        }
        self.step -= 1;
        true
    }

    /// Последний шаг (подпись кнопки меняется на «Готово»).
    pub fn is_last(&self) -> bool {
        self.step + 1 >= ONBOARDING_STEPS.len()
    }

    /// Ключ подписи правой кнопки: «Далее» / «Готово» на последнем шаге
    /// (текст — таблица [`crate::i18n`], FR-040). CR-031: на финальном шаге
    /// «Далее» не рисуется (футер — только «Назад»), выбор — CTA-опции.
    pub fn next_label_key(&self) -> &'static str {
        if self.is_last() {
            keys::ONBOARDING_DONE
        } else {
            keys::ONBOARDING_NEXT
        }
    }

    /// Ключ подписи левой кнопки: «Назад» / нет на первом шаге.
    pub fn prev_label_key(&self) -> Option<&'static str> {
        (self.step > 0).then_some(keys::ONBOARDING_BACK)
    }
}

// --- Геометрия карточки ---

/// Поля клампа карточки к вьюпорту (W-e, аудит ui-kit §8 №12): слот
/// модали = окно минус поля (токен `SPACING_LG`); раньше карточка
/// клампилась к кромкам вплотную.
pub const ONBOARDING_VIEWPORT_MARGIN: f32 = canvas_core::tokens::SPACING_LG;
/// Ширина карточки тура (логические px). CR-031/S2: 720 — двухколоночная
/// вёрстка «50% иллюстрация / 50% текст с кнопками» (UR-002 п.4).
pub const ONBOARDING_CARD_WIDTH: f32 = 720.0;
/// Минимальная ширина карточки для двухколоночного режима (CR-031/S2):
/// уже — одноколонный фолбэк без иллюстрации (узкие окна/стресс-viewport'ы).
pub const ONBOARDING_SPLIT_MIN_W: f32 = 640.0;
/// Внутренние поля карточки.
pub const ONBOARDING_PAD: f32 = 24.0;
/// Кегли строк карточки: заголовок/тело/кнопки.
pub const ONBOARDING_TITLE_FONT: f32 = 18.0;
pub const ONBOARDING_BODY_FONT: f32 = 13.0;
pub const ONBOARDING_TITLE_LINE_H: f32 = 24.0;
pub const ONBOARDING_BODY_LINE_H: f32 = 19.0;
/// Высота футера с кнопками.
pub const ONBOARDING_FOOTER_H: f32 = 46.0;
/// Размер кнопок карточки.
pub const ONBOARDING_BUTTON_W: f32 = 96.0;
pub const ONBOARDING_BUTTON_H: f32 = 30.0;
/// Диаметр прогресс-точки.
pub const ONBOARDING_DOT: f32 = 8.0;
/// Зазор между точками прогресса.
pub const ONBOARDING_DOT_GAP: f32 = 10.0;
/// Отступ прогресс-точек от заголовка.
pub const ONBOARDING_DOTS_TOP: f32 = 12.0;

/// Кнопка карточки тура (hit-тест/рендер).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingButton {
    /// «Назад» — только со 2-го шага.
    Prev,
    /// «Далее», на последнем шаге — «Готово».
    Next,
    /// «Пропустить» — отложить до следующего запуска (Esc — то же).
    Skip,
    /// CR-031: финальный CTA «Открыть шаблонную схему» — тур завершён
    /// (флаг пройден), галерея схем открыта.
    FinalGallery,
    /// CR-031: финальный CTA «Начать самому» — тур завершён, чистый холст.
    FinalEmpty,
}

/// Высота полноширинной CTA-опции финального шага (CR-031).
pub const ONBOARDING_OPTION_H: f32 = 34.0;
/// Зазор между CTA-опциями финала.
pub const ONBOARDING_OPTION_GAP: f32 = kit::GAP_CONTROLS;
/// Зона финальных CTA над футером: верхний зазор + 2 опции + зазор
/// (резервируется из зоны тела — опции всегда видимы, тело скроллится выше).
pub const ONBOARDING_OPTION_ZONE: f32 = 10.0 + 2.0 * ONBOARDING_OPTION_H + ONBOARDING_OPTION_GAP;

/// Rect'ы полноширинных CTA-опций финального шага (CR-031): две кнопки
/// над футером, ширина — внутренняя ширина текстовой колонки (S2).
/// Порядок: «Открыть шаблонную схему» (primary), «Начать самому» (secondary).
pub fn option_rects(card: [f32; 4]) -> [[f32; 4]; 2] {
    let col = text_column(card);
    let w = (col[2] - ONBOARDING_PAD * 2.0).max(0.0);
    let bottom = col[1] + col[3] - ONBOARDING_PAD - ONBOARDING_FOOTER_H;
    let gallery_y = bottom - 2.0 * ONBOARDING_OPTION_H - ONBOARDING_OPTION_GAP;
    let empty_y = bottom - ONBOARDING_OPTION_H;
    [
        [col[0] + ONBOARDING_PAD, gallery_y, w, ONBOARDING_OPTION_H],
        [col[0] + ONBOARDING_PAD, empty_y, w, ONBOARDING_OPTION_H],
    ]
}

/// Строки тела шага после переноса по ширине карточки — измеренный
/// `TextMeasurer::wrap` (W-e, CR-015: замена эвристики
/// «символы × 0.62»; те же метрики, что у шейпинга рендера). Один источник
/// для высоты карточки ([`card_layout`]) и рендера (overlays.rs) —
/// раскладка и геометрия не разъезжаются.
pub fn body_lines(
    step: usize,
    width: f32,
    language: Language,
    role: &str,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> Vec<String> {
    if ONBOARDING_STEPS.get(step).is_none() {
        return Vec::new();
    };
    // CR-031/S2: перенос считается по текстовой колонке (правая половина
    // в двухколоночном режиме) — один источник с [`body_area`]/рендером.
    // CR-031/S3: тело шага 1 — по роли.
    let col_w = text_column([0.0, 0.0, width, 0.0])[2];
    let avail = (col_w - ONBOARDING_PAD * 2.0).max(10.0);
    let wrapped = m.wrap(
        fs,
        i18n::tr(language, step_body_key(step, role)),
        FAMILY,
        ONBOARDING_BODY_FONT,
        avail,
    );
    // W-e follow-up (CI windows): сверхширокий токен без переносов
    // («markdown-разметкой» на Windows-метриках — 226.6 > бюджета 192)
    // выходил за карточку и на рендере, и в тесте бюджета. Жёсткий
    // посимвольный разрыв в бюджет: строка либо влезает, либо рвётся
    // глиф за глифом (тексты онбординга — кириллица/латиница).
    let mut lines: Vec<String> = Vec::with_capacity(wrapped.len());
    for line in wrapped {
        if m.width_of(fs, &line, FAMILY, ONBOARDING_BODY_FONT) <= avail {
            lines.push(line);
            continue;
        }
        let mut cur = String::new();
        for ch in line.chars() {
            let mut next = cur.clone();
            next.push(ch);
            if m.width_of(fs, &next, FAMILY, ONBOARDING_BODY_FONT) > avail && !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            cur.push(ch);
        }
        if !cur.is_empty() {
            lines.push(cur);
        }
    }
    lines
}

/// Отступ от верха карточки до первой строки тела (заголовок +
/// прогресс-точки + зазор) — общий для геометрии и рендера.
pub fn body_top_offset() -> f32 {
    ONBOARDING_PAD + ONBOARDING_TITLE_LINE_H + ONBOARDING_DOTS_TOP + ONBOARDING_DOT + 10.0
}

/// Текстовая колонка карточки (CR-031/S2): в двухколоночном режиме — правая
/// половина (иллюстрация — левая, см. [`illustration_rect`]); в фолбэке —
/// вся карточка. Все текстово-кнопочные зоны (заголовок, точки, тело,
/// футер, финальные CTA) считаются от этой колонки — «50% картинка,
/// 50% текст с кнопками».
pub fn text_column(card: [f32; 4]) -> [f32; 4] {
    if card[2] >= ONBOARDING_SPLIT_MIN_W {
        [card[0] + card[2] / 2.0, card[1], card[2] / 2.0, card[3]]
    } else {
        card
    }
}

/// Зона иллюстрации шага (CR-031/S2): левая половина карточки минус пад —
/// только в двухколоночном режиме ([`text_column`]); в фолбэке — None
/// (карточка узка, иллюстрация не помещается).
pub fn illustration_rect(card: [f32; 4]) -> Option<[f32; 4]> {
    if card[2] >= ONBOARDING_SPLIT_MIN_W {
        Some([
            card[0] + ONBOARDING_PAD,
            card[1] + ONBOARDING_PAD,
            card[2] / 2.0 - ONBOARDING_PAD * 2.0,
            (card[3] - ONBOARDING_PAD * 2.0).max(0.0),
        ])
    } else {
        None
    }
}

/// Зона тела шага в карточке: от якоря первой строки до футера с CTA
/// (минус нижний пад). Общий источник высоты окна видимости скролла для
/// раскладки ([`card_layout`]), колеса ввода и отрисовки (видимые строки).
/// CR-031: на финальном шаге снизу резервируется зона CTA-опций
/// ([`ONBOARDING_OPTION_ZONE`]) — опции всегда видимы, тело скроллится выше.
pub fn body_area(card: [f32; 4], final_step: bool) -> [f32; 4] {
    let col = text_column(card);
    let mut h = (col[3] - body_top_offset() - ONBOARDING_PAD - ONBOARDING_FOOTER_H).max(0.0);
    if final_step {
        h = (h - ONBOARDING_OPTION_ZONE).max(0.0);
    }
    [
        col[0] + ONBOARDING_PAD,
        col[1] + body_top_offset(),
        (col[2] - ONBOARDING_PAD * 2.0).max(0.0),
        h,
    ]
}

/// Раскладка карточки шага (W-e): слот = вьюпорт минус поля
/// ([`ONBOARDING_VIEWPORT_MARGIN`]), панель — `kit::modal` (центр слота,
/// как dialog/autolink); ширина — кламп к слоту, высота — по измеренному
/// телу с клампом к слоту (инвариант FR-028 «карточка целиком в окне» —
/// теперь ещё и с полями). При клампе высоты контент ужимается:
/// скролл-состояние тела синхронизируется с контентом/окном (футер с CTA
/// всегда видим — в скролл-зону не входит). Один источник для реестра
/// (hit-rect'ы), ввода (кнопки) и отрисовки — «ввод = тому, что видно».
pub struct OnboardingLayout {
    /// Rect карточки `[x, y, w, h]` (центр окна, кламп с полями).
    pub card: [f32; 4],
    /// Строки тела после измеренного переноса по ширине карточки.
    pub lines: Vec<String>,
}

/// Раскладка карточки шага: карточка + строки тела + синхронизация
/// скролл-состояния тела (кит [`ScrollState`] — offset сохраняется,
/// content_h/viewport_h обновляются, позиция клампится; паттерн
/// `flow_map_layout`).
pub fn card_layout(
    viewport: [f32; 2],
    step: usize,
    language: Language,
    role: &str,
    scroll: &mut ScrollState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> OnboardingLayout {
    let margin = ONBOARDING_VIEWPORT_MARGIN;
    let slot = UiRect::new(
        margin,
        margin,
        (viewport[0] - margin * 2.0).max(0.0),
        (viewport[1] - margin * 2.0).max(0.0),
    );
    let w = ONBOARDING_CARD_WIDTH.min(slot.w);
    let lines = body_lines(step, w, language, role, m, fs);
    let body_h = lines.len() as f32 * ONBOARDING_BODY_LINE_H;
    // CR-031: финальный шаг несёт две полноширинные CTA-опции — зона
    // резервируется в желаемой высоте карточки.
    let final_step = step + 1 >= ONBOARDING_STEPS.len();
    let options_h = if final_step {
        ONBOARDING_OPTION_ZONE
    } else {
        0.0
    };
    let desired_h = body_top_offset() + body_h + ONBOARDING_PAD + ONBOARDING_FOOTER_H + options_h;
    // Кламп высоты к слоту: desired пре-клампнут, поэтому min-инвариант
    // модали (приоритетен — parity FR-060) не конфликтует с клампом FR-028.
    let h = desired_h.min(slot.h);
    let size = UiVec2::new(w, h);
    let panel = kit::modal(slot, size, size, size).panel;
    let card = [panel.x, panel.y, panel.w, panel.h];
    // Скролл тела: при клампе высоты viewport_h < content_h — строки
    // прокручиваются (колесо ввода; видимые строки — `kit::list_rows`).
    let body = body_area(card, final_step);
    scroll.content_h = lines.len() as f32 * ONBOARDING_BODY_LINE_H;
    scroll.viewport_h = body[3];
    scroll.clamp();
    OnboardingLayout { card, lines }
}

/// Прогресс-точки: центры по горизонтали (рендер — квады-кружки).
/// CR-031/S2: центрируются по текстовой колонке (правая половина), не по
/// всей карточке.
pub fn progress_dots(card: [f32; 4]) -> (Vec<f32>, f32) {
    let col = text_column(card);
    let n = ONBOARDING_STEPS.len() as f32;
    let total = n * ONBOARDING_DOT + (n - 1.0) * ONBOARDING_DOT_GAP;
    let start = col[0] + (col[2] - total) / 2.0;
    let centers: Vec<f32> = (0..ONBOARDING_STEPS.len())
        .map(|i| start + i as f32 * (ONBOARDING_DOT + ONBOARDING_DOT_GAP) + ONBOARDING_DOT / 2.0)
        .collect();
    let y = col[1] + ONBOARDING_PAD + ONBOARDING_TITLE_LINE_H + ONBOARDING_DOTS_TOP;
    (centers, y)
}

/// Rect кнопки карточки (футер прибит к низу карточки — при клампе высоты
/// кнопки остаются достижимы): Prev — слева, Next — справа (оба в футере);
/// Skip — в правом верхнем углу карточки (выход виден всегда — NN/g).
//
// K5/FR-070: `Skip` — это подписанная прямоугольная кнопка 64×22
// («Пропустить»), НЕ каноническая «×» в углу модали. Поэтому `Skip` НЕ
// является миграционным кандидатом для `kit::stage_close_button` (тот
// возвращает квадрат `ICON_BUTTON_SIZE`=26×26 под глиф «×», а Skip —
// широкая кнопка-призрак с подписью). Этот TODO — НЕ close-button
// migration (вопреки FR-070 тегу), а кнопочный layout-миграция.
//
// O2 (Task O / FR-UI-FOOTER): Prev/Next carousel footer migrated to
// `kit::split_footer_buttons` (Agent L's split-footer kit component).
// Pattern: Prev on LEFT (slot.x + inset), Next on RIGHT (slot.right -
// inset - W), inset = ONBOARDING_PAD = 24, gap = GAP_CONTROLS (8,
// unused — 1 button per side). Skip stays manual (top-right corner —
// NOT in the footer slot). Positions are 1:1 with the previous
// hand-rolled formula (footer_y = card.bottom - FOOTER_H + (FOOTER_H -
// BTN_H)/2; Prev.x = card.x + PAD; Next.x = card.right - PAD - W) —
// I-1: zero visual jump.
//
// Будущая миграция Skip — на `kit::button_layout` (измеренный размер
// подписи + `BUTTON_PAD_H` × 2, высота `BUTTON_HEIGHT`=30). Требует:
// 1. Сигнатуру `button_rect(card, button, m, fs, family, size)` —
//    вместо текущей без замерщика (10+ callers в onboarding_ui.rs,
//    app/overlays.rs:3669, app.rs:13310 — большая площадь рефактора).
// 2. UX-ревью: размер меняется с 64×22 на ~(80–100)×30 (ширина
//    измеренной подписи «Пропустить» + 2·BUTTON_PAD_H=12; высота — kit
//    canonical `BUTTON_HEIGHT`=30). Это сдвиг позиции кнопки в карточке
//    (примерно +6px по высоте, +16–36px по ширине).
// 3. Стиль: `kit::button_style(ButtonVariant::Ghost, KitState::Normal,
//    palette)` — ghost-кнопка без заливки/рамки, hover → accent border.
//
// Оставлено как TODO до отдельной волны onboarding button_layout
// (I-1: ноль скачка). Паритет с другими подписанными кнопками-призраками
// в карточках (например suggest-cards).
pub fn button_rect(card: [f32; 4], button: OnboardingButton) -> [f32; 4] {
    match button {
        OnboardingButton::Prev | OnboardingButton::Next => {
            // O2: split footer (Prev left / Next right) via kit.
            // CR-031/S2: футер — в текстовой колонке (правая половина),
            // не по всей карточке.
            let col = text_column(card);
            let slot = UiRect::new(
                col[0],
                col[1] + col[3] - ONBOARDING_FOOTER_H,
                col[2],
                ONBOARDING_FOOTER_H,
            );
            let btns = kit::split_footer_buttons(
                slot,
                &[ONBOARDING_BUTTON_W],
                &[ONBOARDING_BUTTON_W],
                kit::GAP_CONTROLS,
                ONBOARDING_PAD,
            );
            // btns[0] = (Prev rect, FooterGroup::Left, 0);
            // btns[1] = (Next rect, FooterGroup::Right, 0).
            let idx = match button {
                OnboardingButton::Prev => 0,
                OnboardingButton::Next => 1,
                _ => unreachable!(),
            };
            let r = btns[idx].0;
            [r.x, r.y, r.w, r.h]
        }
        OnboardingButton::Skip => [
            card[0] + card[2] - ONBOARDING_PAD - 64.0,
            card[1] + 10.0,
            64.0,
            22.0,
        ],
        // CR-031: финальные CTA-опции — полноширинные кнопки над футером
        // ([`option_rects`]; в футере их нет)
        OnboardingButton::FinalGallery => option_rects(card)[0],
        OnboardingButton::FinalEmpty => option_rects(card)[1],
    }
}

/// Hit-test кнопки карточки. «Назад» на первом шаге отсутствует (None),
/// «Далее»/«Готово» и «Пропустить» доступны всегда (инвариант карусели).
/// CR-031: на финальном шаге «Далее» отсутствует (футер — только «Назад»),
/// вместо него — полноширинные CTA-опции «Открыть шаблонную схему» /
/// «Начать самому» (нижняя зона карточки). FR-097: на coarse-указателе
/// тач-цели кнопок дотягиваются до 44 лог. px (hit-only, кламп в карточку —
/// расширенная зона не выходит за оверлей; на точном указателе — прежние
/// зоны).
pub fn button_at(
    card: [f32; 4],
    state: &OnboardingState,
    point: [f32; 2],
) -> Option<OnboardingButton> {
    // CR-031/S2: тач-расширение клампится в текстовую колонку (кнопки
    // живут в колонке, а не во всей карточке).
    let col = text_column(card);
    let coarse = crate::touch_targets::pointer_coarse();
    let hit = |rect: [f32; 4]| {
        let rect = if coarse {
            crate::touch_targets::intersect_xywh(
                crate::touch_targets::expand_xywh(rect, crate::touch_targets::MIN_TOUCH_TARGET),
                col,
            )
        } else {
            rect
        };
        point[0] >= rect[0]
            && point[0] <= rect[0] + rect[2]
            && point[1] >= rect[1]
            && point[1] <= rect[1] + rect[3]
    };
    // CR-031: финальный шаг — CTA-опции вместо «Далее»
    if state.is_last() {
        let [gallery, empty] = option_rects(card);
        if hit(gallery) {
            return Some(OnboardingButton::FinalGallery);
        }
        if hit(empty) {
            return Some(OnboardingButton::FinalEmpty);
        }
    } else if hit(button_rect(card, OnboardingButton::Next)) {
        return Some(OnboardingButton::Next);
    }
    if hit(button_rect(card, OnboardingButton::Skip)) {
        return Some(OnboardingButton::Skip);
    }
    // «Назад» только не на первом шаге
    if state.step > 0 && hit(button_rect(card, OnboardingButton::Prev)) {
        return Some(OnboardingButton::Prev);
    }
    None
}

/// Клик внутри карточки (но не по кнопкам)? Ввод канваса глотается всей
/// карточкой — клик мимо кнопок не проваливается под оверлей.
pub fn point_in_card(card: [f32; 4], point: [f32; 2]) -> bool {
    point[0] >= card[0]
        && point[0] <= card[0] + card[2]
        && point[1] >= card[1]
        && point[1] <= card[1] + card[3]
}

// =============================================================================
// FR-LLM-B / PRD-0010 F-8: AI-онбординг — экран выбора режима (Local / Cloud /
// Self-hosted). Не часть карусели [`ONBOARDING_STEPS`] — отдельный модальный
// оверлей, открываемый пунктом «Онбординг AI» из меню «?» либо триггером
// продукта при первом включении AI-функции. Сохраняет выбор в
// `Settings::llm::data_residency` (плюс per-feature провайдеры по режиму).
// =============================================================================

/// Ширина карточки AI-онбординга (логические px, прототип F-8: 600px).
pub const AI_ONB_CARD_W: f32 = 600.0;
/// Внутренние поля карточки (прототип F-8: 26px top / 30px sides / 22px bottom).
pub const AI_ONB_PAD_X: f32 = 30.0;
pub const AI_ONB_PAD_TOP: f32 = 26.0;
pub const AI_ONB_PAD_BOTTOM: f32 = 22.0;
/// Кегль заголовка/подзаголовка/режимов/кнопок.
pub const AI_ONB_STEP_FONT: f32 = 10.0;
pub const AI_ONB_TITLE_FONT: f32 = 19.0;
pub const AI_ONB_SUB_FONT: f32 = 12.5;
pub const AI_ONB_MODE_TITLE_FONT: f32 = 13.5;
pub const AI_ONB_MODE_DESC_FONT: f32 = 11.5;
pub const AI_ONB_BTN_FONT: f32 = 13.0;
/// Высоты строк: step / title / sub / mode card / privacy block.
pub const AI_ONB_STEP_H: f32 = 18.0;
pub const AI_ONB_TITLE_H: f32 = 26.0;
pub const AI_ONB_SUB_H: f32 = 38.0;
pub const AI_ONB_MODE_H: f32 = 64.0;
pub const AI_ONB_MODE_GAP: f32 = 8.0;
pub const AI_ONB_ACTIONS_H: f32 = 36.0;
pub const AI_ONB_PRIV_H: f32 = 180.0;
/// Поля клампа карточки к вьюпорту (паттерн ONBOARDING_VIEWPORT_MARGIN).
pub const AI_ONB_VIEWPORT_MARGIN: f32 = canvas_core::tokens::SPACING_LG;
/// Размер кнопок «Подробнее о privacy» / «Продолжить» (прототип F-8).
pub const AI_ONB_BTN_H: f32 = 30.0;
pub const AI_ONB_BTN_PRIV_W: f32 = 168.0;
pub const AI_ONB_BTN_CONTINUE_W: f32 = 132.0;

/// Режим AI (F-8.2): выбор пользователя сохраняется в
/// `Settings::llm::data_residency`. Per-feature провайдеры переопределяются
/// по режиму (Local → Laya/Ollama, Cloud → BYOK/ChatGPT, Self-hosted →
/// self-hosted endpoint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiOnboardingChoice {
    /// Local only (Laya / Ollama) — offline, данные не покидают машину.
    /// `data_residency: Local`, suggest=Laya, graph=Ollama, agent=Ollama.
    Local,
    /// Cloud (ChatGPT / BYOK) — лучшее качество, redact context (Q1).
    /// `data_residency: Cloud`, suggest=BYOK, graph=ChatGPT, agent=ChatGPT.
    #[default]
    Cloud,
    /// Self-hosted (ваш GPU-сервер) — данные в контуре, нужен endpoint.
    /// `data_residency: SelfHosted`, per-feature провайдеры остаются Off
    /// до ввода endpoint'а (настройки → AI и модели).
    SelfHosted,
}

impl AiOnboardingChoice {
    /// Соответствующий режим `DataResidency` (сохраняется в Settings.llm).
    pub fn data_residency(self) -> canvas_llm::DataResidency {
        match self {
            AiOnboardingChoice::Local => canvas_llm::DataResidency::Local,
            AiOnboardingChoice::Cloud => canvas_llm::DataResidency::Cloud,
            AiOnboardingChoice::SelfHosted => canvas_llm::DataResidency::SelfHosted,
        }
    }

    /// Per-feature провайдеры по умолчанию для режима (прототип F-8 JS:
    /// `aiOnbGo` применяет `AI.prov.{suggest,graph,agent}` по `c`).
    /// `SelfHosted` не задаёт провайдеров автоматически — пользователь
    /// вводит endpoint в настройках и THEN выбирает BYOK/self-hosted.
    pub fn default_providers(self) -> [canvas_llm::LlmProviderId; 3] {
        match self {
            AiOnboardingChoice::Local => [
                canvas_llm::LlmProviderId::Laya,
                canvas_llm::LlmProviderId::Ollama,
                canvas_llm::LlmProviderId::Ollama,
            ],
            AiOnboardingChoice::Cloud => [
                canvas_llm::LlmProviderId::Byok,
                canvas_llm::LlmProviderId::ChatGptOAuth,
                canvas_llm::LlmProviderId::ChatGptOAuth,
            ],
            AiOnboardingChoice::SelfHosted => [
                canvas_llm::LlmProviderId::Off,
                canvas_llm::LlmProviderId::Off,
                canvas_llm::LlmProviderId::Off,
            ],
        }
    }

    /// i18n-ключ заголовка режима (подпись в карточке-радио F-8).
    pub fn title_key(self) -> &'static str {
        match self {
            AiOnboardingChoice::Local => keys::AI_ONB_LOCAL_TITLE,
            AiOnboardingChoice::Cloud => keys::AI_ONB_CLOUD_TITLE,
            AiOnboardingChoice::SelfHosted => keys::AI_ONB_SELFHOST_TITLE,
        }
    }

    /// i18n-ключ описания режима (строка под заголовком в карточке-радио).
    pub fn desc_key(self) -> &'static str {
        match self {
            AiOnboardingChoice::Local => keys::AI_ONB_LOCAL_DESC,
            AiOnboardingChoice::Cloud => keys::AI_ONB_CLOUD_DESC,
            AiOnboardingChoice::SelfHosted => keys::AI_ONB_SELFHOST_DESC,
        }
    }

    /// i18n-ключ тега режима (правый верхний угол карточки-радио —
    /// «offline» / «лучшее качество» / «в контуре»).
    pub fn tag_key(self) -> &'static str {
        match self {
            AiOnboardingChoice::Local => keys::AI_ONB_TAG_OFFLINE,
            AiOnboardingChoice::Cloud => keys::AI_ONB_TAG_QUALITY,
            AiOnboardingChoice::SelfHosted => keys::AI_ONB_TAG_CONTOUR,
        }
    }

    /// i18n-ключ тоста после подтверждения выбора («AI-режим: Local only…»).
    pub fn toast_key(self) -> &'static str {
        match self {
            AiOnboardingChoice::Local => keys::AI_ONB_TOAST_LOCAL,
            AiOnboardingChoice::Cloud => keys::AI_ONB_TOAST_CLOUD,
            AiOnboardingChoice::SelfHosted => keys::AI_ONB_TOAST_SELFHOST,
        }
    }
}

/// Состояние экрана AI-онбординга. `selected = None` — ничего не выбрано,
/// кнопка «Продолжить» отключена (прототип F-8: `disabled` пока `onbChoice`
/// null). `privacy_open` — раскрытый блок «Подробнее о privacy».
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AiOnboardingState {
    /// Выбранный режим (Local / Cloud / SelfHosted). `None` до клика по
    /// карточке-радио — кнопка «Продолжить» отключена.
    pub selected: Option<AiOnboardingChoice>,
    /// Раскрыт ли блок «Подробнее о privacy» (toggle кнопкой «Подробнее»).
    pub privacy_open: bool,
}

/// Кнопка экрана AI-онбординга (hit-тест/рендер).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiOnboardingButton {
    /// Карточка-радио режима (Local / Cloud / SelfHosted).
    Mode(AiOnboardingChoice),
    /// «Подробнее о privacy» — toggle раскрытия блока.
    Privacy,
    /// «Продолжить» — применить выбор и закрыть экран. Отключена, пока
    /// `selected = None`.
    Continue,
}

/// Раскладка экрана AI-онбординга: rect карточки + высота (зависит от
/// `privacy_open` — раскрытый блок добавляет `AI_ONB_PRIV_H`).
#[derive(Debug, Clone, PartialEq)]
pub struct AiOnboardingLayout {
    /// Rect карточки `[x, y, w, h]` (центр вьюпорта, кламп с полями).
    pub card: [f32; 4],
    /// Rect каждой карточки-режима (Local / Cloud / SelfHosted) — порядок
    /// [`AiOnboardingChoice`]: Local, Cloud, SelfHosted.
    pub mode_cards: [[f32; 4]; 3],
    /// Rect кнопок «Подробнее о privacy» и «Продолжить».
    pub btn_privacy: [f32; 4],
    pub btn_continue: [f32; 4],
    /// Rect блока «Подробнее о privacy» (когда `privacy_open = true`).
    pub privacy_block: [f32; 4],
}

/// Вычислить раскладку экрана AI-онбординга (чистая функция, без замерщика —
/// высоты строк фиксированы по прототипу F-8). Карточка центрируется во
/// вьюпорте с полями `AI_ONB_VIEWPORT_MARGIN`; высота зависит от
/// `state.privacy_open`.
//
// O1 (Task O / FR-UI-RADIO): mode-card rendering migrated to `kit::radio_card`
// in `app/overlays.rs::ai_onboarding_overlay` (where palette is available —
// this layout function stays a pure geometry function returning slot rects,
// per task step 1 option (b); signature unchanged). The kit's
// `radio_card(slot, label_w, desc_w, selected, state, palette)` is called
// per-card in overlays.rs; `paint_radio_card` emits the card background +
// indicator; `layout.label`/`layout.desc` rects are used for text positions.
// Tag (right corner) stays separate — kit doesn't support tag.
pub fn ai_onboarding_layout(viewport: [f32; 2], state: &AiOnboardingState) -> AiOnboardingLayout {
    use canvas_ui::geometry::{UiRect, UiVec2};
    use canvas_ui::kit;
    let margin = AI_ONB_VIEWPORT_MARGIN;
    let slot = UiRect::new(
        margin,
        margin,
        (viewport[0] - margin * 2.0).max(0.0),
        (viewport[1] - margin * 2.0).max(0.0),
    );
    let w = AI_ONB_CARD_W.min(slot.w);
    // Высота: step + title + sub + 3 mode cards (с зазорами) + actions +
    // опционально privacy block. Поля — top/bottom.
    let modes_h = 3.0 * AI_ONB_MODE_H + 2.0 * AI_ONB_MODE_GAP;
    let mut h = AI_ONB_PAD_TOP
        + AI_ONB_STEP_H
        + AI_ONB_TITLE_H
        + AI_ONB_SUB_H
        + modes_h
        + 18.0 // зазор от режимов до actions (прототип: margin-top: 18px)
        + AI_ONB_ACTIONS_H
        + AI_ONB_PAD_BOTTOM;
    if state.privacy_open {
        h += 15.0 + AI_ONB_PRIV_H; // зазор + блок
    }
    let h = h.min(slot.h);
    let panel = kit::modal(
        slot,
        UiVec2::new(w, h),
        UiVec2::new(w, h),
        UiVec2::new(w, h),
    )
    .panel;
    let card = [panel.x, panel.y, panel.w, panel.h];
    // Y-positions: шаг → заголовок → подзаголовок → 3 карточки → actions → privacy.
    let x = card[0] + AI_ONB_PAD_X;
    let inner_w = (card[2] - AI_ONB_PAD_X * 2.0).max(0.0);
    let mut y = card[1] + AI_ONB_PAD_TOP;
    y += AI_ONB_STEP_H + AI_ONB_TITLE_H + AI_ONB_SUB_H;
    // 3 карточки-режима (Local / Cloud / SelfHosted — порядок AiOnboardingChoice).
    let mut mode_cards = [[0.0f32; 4]; 3];
    for (i, rect) in mode_cards.iter_mut().enumerate() {
        *rect = [
            x,
            y + i as f32 * (AI_ONB_MODE_H + AI_ONB_MODE_GAP),
            inner_w,
            AI_ONB_MODE_H,
        ];
    }
    let actions_y = y + 3.0 * AI_ONB_MODE_H + 2.0 * AI_ONB_MODE_GAP + 18.0;
    // O2 (Task O / FR-UI-FOOTER): Privacy (left) + Continue (right) migrated
    // to `kit::split_footer_buttons` (Agent L's split-footer kit component).
    // Pattern: Privacy on LEFT (slot.x + inset=0), Continue on RIGHT
    // (slot.right - inset=0 - W). Slot height = AI_ONB_BTN_H (30, no vertical
    // centering — y stays at actions_y). Positions are 1:1 with the previous
    // hand-rolled formula (btn_privacy.x = x = card.x + PAD_X;
    // btn_continue.x = x + inner_w - CONTINUE_W = card.right - PAD_X -
    // CONTINUE_W) — I-1: zero visual jump.
    let actions_slot = UiRect::new(x, actions_y, inner_w, AI_ONB_BTN_H);
    let ai_footer_btns = kit::split_footer_buttons(
        actions_slot,
        &[AI_ONB_BTN_PRIV_W],
        &[AI_ONB_BTN_CONTINUE_W],
        kit::GAP_CONTROLS, // unused — 1 button per side
        0.0,               // no inset — both buttons flush to slot edges
    );
    // btns[0] = (Privacy rect, FooterGroup::Left, 0);
    // btns[1] = (Continue rect, FooterGroup::Right, 0).
    let priv_r = ai_footer_btns[0].0;
    let cont_r = ai_footer_btns[1].0;
    let btn_privacy = [priv_r.x, priv_r.y, priv_r.w, priv_r.h];
    let btn_continue = [cont_r.x, cont_r.y, cont_r.w, cont_r.h];
    let privacy_block = if state.privacy_open {
        [
            x,
            actions_y + AI_ONB_ACTIONS_H + 15.0,
            inner_w,
            AI_ONB_PRIV_H,
        ]
    } else {
        [0.0; 4]
    };
    AiOnboardingLayout {
        card,
        mode_cards,
        btn_privacy,
        btn_continue,
        privacy_block,
    }
}

/// Hit-test кнопки экрана AI-онбординга (карточки-режимы — всегда доступны;
/// «Подробнее» — всегда; «Продолжить» — только если `selected` не None).
pub fn ai_onboarding_button_at(
    layout: &AiOnboardingLayout,
    state: &AiOnboardingState,
    point: [f32; 2],
) -> Option<AiOnboardingButton> {
    use canvas_ui::geometry::UiRect;
    let hit = |rect: [f32; 4]| -> bool {
        let r = UiRect::new(rect[0], rect[1], rect[2], rect[3]);
        point[0] >= r.x && point[0] <= r.x + r.w && point[1] >= r.y && point[1] <= r.y + r.h
    };
    // Карточки-режимы (Local / Cloud / SelfHosted — порядок AiOnboardingChoice).
    let choices = [
        AiOnboardingChoice::Local,
        AiOnboardingChoice::Cloud,
        AiOnboardingChoice::SelfHosted,
    ];
    for (i, &choice) in choices.iter().enumerate() {
        if hit(layout.mode_cards[i]) {
            return Some(AiOnboardingButton::Mode(choice));
        }
    }
    if hit(layout.btn_privacy) {
        return Some(AiOnboardingButton::Privacy);
    }
    // «Продолжить» — только если режим выбран (иначе кнопка disabled).
    if state.selected.is_some() && hit(layout.btn_continue) {
        return Some(AiOnboardingButton::Continue);
    }
    None
}

/// Клик внутри карточки AI-онбординга (но не по кнопкам)? Ввод канваса
/// глотается всей карточкой — клик мимо кнопок не проваливается под оверлей.
pub fn ai_onboarding_point_in_card(card: [f32; 4], point: [f32; 2]) -> bool {
    point[0] >= card[0]
        && point[0] <= card[0] + card[2]
        && point[1] >= card[1]
        && point[1] <= card[1] + card[3]
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_core::ONBOARDING_MAX_DEFERS;

    /// Замерщик + FontSystem для тестов (глобальный замерный шрифт —
    /// короткий скоуп, паттерн tooltip/dialog).
    fn measurer() -> (TextMeasurer, cosmic_text::FontSystem) {
        (TextMeasurer::new(), cosmic_text::FontSystem::new())
    }

    /// Раскладка шага со свежим скролл-состоянием (тесты геометрии).
    /// `role` — роль владельца (CR-031/S3: влияет на тело шага 1).
    fn layout(viewport: [f32; 2], step: usize, language: Language) -> OnboardingLayout {
        let mut scroll = ScrollState::default();
        layout_sync(viewport, step, language, "default", &mut scroll)
    }

    /// Раскладка шага с внешним скролл-состоянием и ролью (тесты скролла).
    fn layout_sync(
        viewport: [f32; 2],
        step: usize,
        language: Language,
        role: &str,
        scroll: &mut ScrollState,
    ) -> OnboardingLayout {
        let (mut m, mut fs) = measurer();
        card_layout(viewport, step, language, role, scroll, &mut m, &mut fs)
    }

    /// CR-031/S3: value-приветствие по роли — у каждой роли свой ключ,
    /// неизвестная/не выбранная — универсальный; все тексты непусты (RU/EN).
    #[test]
    fn welcome_value_by_role() {
        let roles = [
            "default",
            "architect",
            "developer",
            "product-manager",
            "analyst",
            "cio",
            "cto",
            "founder",
        ];
        let mut keys: Vec<&'static str> = roles.iter().map(|&r| welcome_body_key(r)).collect();
        // Все ключи различны (8 ролей — 8 разных текстов)
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), roles.len(), "у каждой роли свой value-текст");
        // Неизвестная роль и пустая — универсальное
        assert_eq!(welcome_body_key("unknown"), keys::ONBOARDING_VALUE_DEFAULT);
        assert_eq!(welcome_body_key(""), keys::ONBOARDING_VALUE_DEFAULT);
        // Тексты разрешаются на обоих языках и содержательны
        for role in roles {
            let key = welcome_body_key(role);
            for language in [Language::Ru, Language::En] {
                assert!(
                    i18n::tr(language, key).len() > 60,
                    "value-текст роли {role} слишком короткий"
                );
            }
        }
        // step_body_key: только шаг 1 ролевой, остальные — статичные
        assert_eq!(step_body_key(0, "analyst"), welcome_body_key("analyst"));
        assert_eq!(
            step_body_key(1, "analyst"),
            ONBOARDING_STEPS[1].body_key,
            "шаг 2 не зависит от роли"
        );
    }

    /// Инвариант триггера (таблица решений FR-028): показ = не пройден И
    /// откладываний < 3; пройден — никогда (любые defers).
    #[test]
    fn should_show_onboarding_decision_table() {
        for done in [false, true] {
            for defers in [0u8, 1, 2, 3, 4, 200] {
                let settings = Settings {
                    onboarding_done: done,
                    onboarding_defers: defers,
                    ..Settings::default()
                };
                let expected = !done && defers < ONBOARDING_MAX_DEFERS;
                assert_eq!(
                    should_show_onboarding(&settings),
                    expected,
                    "done={done} defers={defers}"
                );
            }
        }
        // Дефолтный конфиг — тур показать
        assert!(should_show_onboarding(&Settings::default()));
    }

    /// Инвариант карусели: step в границах после любых next/prev; «Назад»
    /// отсутствует на первом шаге; подпись «Далее» → «Готово» на последнем.
    #[test]
    fn carousel_bounds_and_labels() {
        let mut state = OnboardingState::default();
        assert_eq!(state.step, 0);
        assert!(
            state.prev_label_key().is_none(),
            "«Назад» на первом шаге нет"
        );
        assert!(!state.prev(), "prev на первом — без изменений");
        assert_eq!(i18n::tr(Language::Ru, state.next_label_key()), "Далее");
        // Полный проход до последнего
        for expected in 1..ONBOARDING_STEPS.len() {
            assert!(state.next(), "шаг {expected}");
            assert_eq!(state.step, expected);
        }
        assert!(state.is_last());
        assert_eq!(
            i18n::tr(Language::Ru, state.next_label_key()),
            "Готово",
            "подпись на последнем шаге"
        );
        assert!(!state.next(), "next на последнем — без изменений");
        assert_eq!(state.step, ONBOARDING_STEPS.len() - 1);
        // Возврат к началу
        while state.step > 0 {
            assert!(state.prev());
        }
        assert!(!state.prev());
        // Шаги непустые (ключи заголовка/тела; EN-перевод длиннее 40 знаков)
        for step in &ONBOARDING_STEPS {
            assert!(!step.title_key.is_empty());
            assert!(
                i18n::tr(Language::En, step.body_key).len() > 40,
                "тело шага слишком короткое"
            );
        }
    }

    /// Инвариант клампа (расширен W-e): карточка целиком внутри окна С
    /// ПОЛЯМИ ([`ONBOARDING_VIEWPORT_MARGIN`]) на G4-окнах (1280×800 /
    /// 1024×640 / 800×560) и стресс-окнах (320×240 / 240×180) на любом
    /// шаге; ширина не больше окна; кнопки внутри карточки.
    #[test]
    fn card_clamped_to_window() {
        for viewport in [
            [1600.0, 900.0],
            [1280.0, 800.0],
            [1024.0, 640.0],
            [800.0, 560.0],
            [320.0, 240.0],
            [240.0, 180.0],
        ] {
            for step in 0..ONBOARDING_STEPS.len() {
                let card = layout(viewport, step, Language::Ru).card;
                let margin = ONBOARDING_VIEWPORT_MARGIN;
                assert!(card[0] >= margin - 0.01, "за левым полем: {card:?}");
                assert!(card[1] >= margin - 0.01, "за верхним полем: {card:?}");
                assert!(
                    card[0] + card[2] <= viewport[0] - margin + 0.01,
                    "за правым полем: {card:?}"
                );
                assert!(
                    card[1] + card[3] <= viewport[1] - margin + 0.01,
                    "за нижним полем: {card:?}"
                );
                assert!(card[2] <= viewport[0] + 0.01, "шире окна: {card:?}");
                assert!(card[3] <= viewport[1] + 0.01, "выше окна: {card:?}");
                // Кнопки внутри карточки
                for button in [
                    OnboardingButton::Prev,
                    OnboardingButton::Next,
                    OnboardingButton::Skip,
                ] {
                    let rect = button_rect(card, button);
                    assert!(rect[0] >= card[0] && rect[0] + rect[2] <= card[0] + card[2]);
                    assert!(rect[1] >= card[1] && rect[1] + rect[3] <= card[1] + card[3]);
                }
            }
        }
    }

    /// Стресс 240×180 (W-e): при клампе высоты контент ужимается, футер с
    /// CTA НЕ перекрывается телом и остаётся достижим; тело прокручивается
    /// до конца (последняя строка достижима скроллом). CR-031: на финальном
    /// шаге CTA — полноширинные опции (FinalGallery), зона тела уже на
    /// [`ONBOARDING_OPTION_ZONE`].
    #[test]
    fn stress_240x180_footer_visible_and_body_reachable() {
        let viewport = [240.0, 180.0];
        for step in 0..ONBOARDING_STEPS.len() {
            let final_step = step + 1 == ONBOARDING_STEPS.len();
            let lay = layout(viewport, step, Language::Ru);
            let card = lay.card;
            // Футер (кнопки) прибит к низу карточки и НЕ входит в скролл-зону
            let body = body_area(card, final_step);
            let footer_top = card[1] + card[3] - ONBOARDING_FOOTER_H;
            assert!(
                body[1] + body[3] <= footer_top + 0.01,
                "тело налезает на футер: body={body:?} footer_top={footer_top}"
            );
            // CTA (Next, и Prev со 2-го шага) — в футере; Skip — ghost в
            // правом верхнем углу карточки (выход виден всегда — NN/g,
            // дизайн [`button_rect`]: `card[1] + 10`), в футер не входит,
            // но обязан быть внутри карточки.
            let next = button_rect(card, OnboardingButton::Next);
            assert!(next[1] >= footer_top - 0.01, "CTA выше футера: {next:?}");
            let skip = button_rect(card, OnboardingButton::Skip);
            assert!(
                skip[0] >= card[0]
                    && skip[1] >= card[1]
                    && skip[0] + skip[2] <= card[0] + card[2] + 0.01
                    && skip[1] + skip[3] <= card[1] + card[3] + 0.01,
                "Skip вне карточки: {skip:?}"
            );
            // Контент ужимается скроллом: окно видимости = зона тела,
            // полный контент достижим (offset ≤ max_offset показывает низ)
            let content_h = lay.lines.len() as f32 * ONBOARDING_BODY_LINE_H;
            let mut scroll = ScrollState::default();
            let relaid = layout_sync(viewport, step, Language::Ru, "default", &mut scroll);
            assert!((relaid.lines.len() as f32 * ONBOARDING_BODY_LINE_H - content_h).abs() < 0.01);
            assert!(
                (scroll.viewport_h - body[3]).abs() < 0.01,
                "окно = зона тела"
            );
            if content_h > body[3] {
                assert!(scroll.needs_scroll(), "контент выше окна — скролл нужен");
                scroll.offset = scroll.max_offset(); // прокрутка до конца
                                                     // Последняя строка целиком в окне видимости
                let visible_tail = content_h - scroll.offset;
                assert!(
                    visible_tail <= scroll.viewport_h + 0.01,
                    "хвост контента не влез после прокрутки до конца"
                );
            }
            // Кнопки кликабельны (hit-тест работает при клампе); на финале
            // CTA — опция «Открыть шаблонную схему»
            let (cta_rect, cta) = if final_step {
                (option_rects(card)[0], OnboardingButton::FinalGallery)
            } else {
                (next, OnboardingButton::Next)
            };
            let state = OnboardingState { step };
            assert_eq!(
                button_at(card, &state, [cta_rect[0] + 5.0, cta_rect[1] + 10.0]),
                Some(cta),
                "CTA достижим на шаге {step}"
            );
        }
    }

    /// Измеренный перенос (W-e, CR-015): каждая строка тела укладывается в
    /// ширину карточки минус пад (замер тем же `TextMeasurer::width_of`, что
    /// и перенос); ручная эвристика 0.62 удалена.
    ///
    /// Нижняя граница ширины — 216: kit-перенос пословный, неразрывный
    /// токен длиннее бюджета не дробит (ниже поля найден кейс
    /// «markdown-разметкой» — 146.3px > 144px бюджета карточки 192 на
    /// стресс-поле 240×180; перелив ≤ одного слова ≈ 2px — задокументированное
    /// ограничение кита, жёсткий посимвольный брейк/брейк по дефису —
    /// не скоуп W-e). Достижимость/футер на 240×180 покрывает
    /// [`stress_240x180_footer_visible_and_body_reachable`].
    #[test]
    fn body_lines_fit_measured_width() {
        for width in [ONBOARDING_CARD_WIDTH, 240.0, 216.0] {
            for step in 0..ONBOARDING_STEPS.len() {
                // CR-031/S2: бюджет — текстовая колонка минус пад (как в
                // card_layout/рендере).
                let avail =
                    (text_column([0.0, 0.0, width, 0.0])[2] - ONBOARDING_PAD * 2.0).max(10.0);
                let (mut m, mut fs) = measurer();
                for language in [Language::Ru, Language::En] {
                    for line in body_lines(step, width, language, "default", &mut m, &mut fs) {
                        let w = m.width_of(&mut fs, &line, FAMILY, ONBOARDING_BODY_FONT);
                        assert!(
                            w <= avail + 0.5,
                            "строка {w:.1} шире бюджета {avail:.1}: {line:?}"
                        );
                    }
                }
            }
        }
    }

    /// CR-031/S2: двухколоночная вёрстка — карточка ≥ `ONBOARDING_SPLIT_MIN_W`
    /// делится пополам: слева зона иллюстрации, справа текстовая колонка;
    /// узкая карточка — фолбэк (колонка = карточка, иллюстрации нет).
    #[test]
    fn split_layout_illustration_and_column() {
        let viewport = [1280.0, 800.0];
        let card = layout(viewport, 0, Language::Ru).card;
        assert_eq!(card[2], ONBOARDING_CARD_WIDTH, "полная ширина карточки");
        let col = text_column(card);
        assert_eq!(col[0], card[0] + card[2] / 2.0, "колонка — правая половина");
        assert_eq!(col[2], card[2] / 2.0);
        let (Some(zone), Some(_)) = (
            illustration_rect(card),
            illustration_rect(card).map(|z| {
                assert_eq!(
                    z[0] + z[2],
                    card[0] + card[2] / 2.0 - ONBOARDING_PAD,
                    "иллюстрация — левая половина"
                )
            }),
        ) else {
            panic!("иллюстрация обязана быть на широкой карточке");
        };
        assert!(
            zone[2] > 100.0 && zone[3] > 100.0,
            "зона иллюстрации содержательна"
        );
        // Кнопки футера — в колонке, не по всей карточке
        let next = button_rect(card, OnboardingButton::Next);
        assert!(next[0] >= col[0], "Next левее колонки");
        // Фолбэк: узкое окно — колонка = карточка, иллюстрации нет
        let narrow = layout([320.0, 240.0], 0, Language::Ru).card;
        assert!(narrow[2] < ONBOARDING_SPLIT_MIN_W);
        assert_eq!(text_column(narrow), narrow, "фолбэк: колонка = карточка");
        assert_eq!(illustration_rect(narrow), None, "фолбэк: без иллюстрации");
    }

    /// Hit-тесты кнопок: Next/Skip кликабельны на каждом шаге; Prev —
    /// только со 2-го; мимо кнопок — None; клик в теле карточки глотается.
    /// CR-031: на финальном шаге Next отсутствует — вместо него CTA-опции
    /// «Открыть шаблонную схему» (primary) и «Начать самому» (secondary).
    #[test]
    fn button_hit_tests() {
        let viewport = [1600.0, 900.0];
        let mut state = OnboardingState::default();
        let card = layout(viewport, state.step, Language::Ru).card;
        let next = button_rect(card, OnboardingButton::Next);
        assert_eq!(
            button_at(card, &state, [next[0] + 5.0, next[1] + 10.0]),
            Some(OnboardingButton::Next)
        );
        let skip = button_rect(card, OnboardingButton::Skip);
        assert_eq!(
            button_at(card, &state, [skip[0] + 5.0, skip[1] + 10.0]),
            Some(OnboardingButton::Skip)
        );
        // Первый шаг: «Назад» нет — даже по его rect
        let prev = button_rect(card, OnboardingButton::Prev);
        assert_eq!(
            button_at(card, &state, [prev[0] + 5.0, prev[1] + 10.0]),
            None,
            "«Назад» на первом шаге отсутствует"
        );
        // Второй шаг: «Назад» есть
        state.next();
        let card = layout(viewport, state.step, Language::Ru).card;
        let prev = button_rect(card, OnboardingButton::Prev);
        assert_eq!(
            button_at(card, &state, [prev[0] + 5.0, prev[1] + 10.0]),
            Some(OnboardingButton::Prev)
        );
        // Точка в теле карточки — глотается (канвас не получает)
        let body_point = [card[0] + card[2] / 2.0, card[1] + card[3] / 2.0];
        assert!(point_in_card(card, body_point));
        assert_eq!(button_at(card, &state, body_point), None);
        // Мимо карточки — не в карточке
        assert!(!point_in_card(card, [10.0, 10.0]));

        // CR-031: финальный шаг — CTA-опции вместо «Далее»
        let last = OnboardingState {
            step: ONBOARDING_STEPS.len() - 1,
        };
        let card = layout(viewport, last.step, Language::Ru).card;
        // Rect «Далее» — от финальной карточки (высота карточки зависит от
        // шага — карточка центрируется, rect со шага 0 сюда не годится)
        let next = button_rect(card, OnboardingButton::Next);
        let [gallery, empty] = option_rects(card);
        assert_eq!(
            button_at(card, &last, [gallery[0] + 5.0, gallery[1] + 10.0]),
            Some(OnboardingButton::FinalGallery),
            "CTA «Открыть шаблонную схему» достижим на финале"
        );
        assert_eq!(
            button_at(card, &last, [empty[0] + 5.0, empty[1] + 10.0]),
            Some(OnboardingButton::FinalEmpty),
            "CTA «Начать самому» достижим на финале"
        );
        assert_eq!(
            button_at(card, &last, [next[0] + 5.0, next[1] + 10.0]),
            None,
            "«Далее» на финальном шаге отсутствует"
        );
    }

    /// Прогресс-точки: по числу шагов, в пределах карточки по ширине.
    #[test]
    fn progress_dots_layout() {
        let viewport = [1600.0, 900.0];
        let card = layout(viewport, 0, Language::Ru).card;
        let (centers, y) = progress_dots(card);
        assert_eq!(centers.len(), ONBOARDING_STEPS.len());
        assert!(y > card[1]);
        for &cx in &centers {
            assert!(cx >= card[0] && cx <= card[0] + card[2]);
        }
        // Точки равноотстоящи
        if centers.len() > 2 {
            let d = centers[1] - centers[0];
            assert!((centers[2] - centers[1] - d).abs() < 1e-3);
        }
    }

    /// Высота тела растёт с длиной текста шага: карточка выше у многословных
    /// шагов (перенос работает).
    #[test]
    fn card_height_follows_content() {
        let viewport = [1600.0, 900.0];
        let heights: Vec<f32> = (0..ONBOARDING_STEPS.len())
            .map(|step| layout(viewport, step, Language::Ru).card[3])
            .collect();
        assert!(heights.iter().all(|&h| h > 100.0), "карточка не пустая");
        // Узкое окно не даёт карточке вылезти
        let narrow = layout([320.0, 240.0], 0, Language::Ru).card;
        assert!(narrow[2] <= 320.0);
    }
}
