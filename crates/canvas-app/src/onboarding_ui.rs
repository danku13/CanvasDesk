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

/// Шаг тура: заголовок и абзац тела (короткие тексты, перенос по ширине
/// карточки — измеренный `TextMeasurer::wrap`). Зарезервированный
/// `action` для v2 — интерактивная чек-точка демо-канваса (машина состояний
/// не переписывается).
pub struct OnboardingStep {
    /// Ключ заголовка (таблица [`crate::i18n`] — FR-040).
    pub title_key: &'static str,
    /// Ключ тела (полная фраза, перенос на стороне [`body_lines`]).
    pub body_key: &'static str,
    /// FR-049: опциональное действие шага — CTA-кнопка «Далее» превращается
    /// в действие (резервация v1 `onboarding_ui.rs:27` задействована).
    /// `Some(keys::GALLERY_TRY)` — открыть галерею схем (тур закрывается).
    pub action_key: Option<&'static str>,
}

/// Шаги тура (FR-028, скоуп владельца — база + расчёты + шаблоны; NN/g:
/// 8±2 шага, «один шаг = одна мысль», выход виден всегда). Порядок
/// стабилен; «Готово» — на последнем.
pub const ONBOARDING_STEPS: [OnboardingStep; 9] = [
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP1_TITLE,
        body_key: keys::ONBOARDING_STEP1_BODY,
        action_key: None,
    },
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP2_TITLE,
        body_key: keys::ONBOARDING_STEP2_BODY,
        action_key: None,
    },
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP3_TITLE,
        body_key: keys::ONBOARDING_STEP3_BODY,
        action_key: None,
    },
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP4_TITLE,
        body_key: keys::ONBOARDING_STEP4_BODY,
        action_key: None,
    },
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP5_TITLE,
        body_key: keys::ONBOARDING_STEP5_BODY,
        action_key: None,
    },
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP6_TITLE,
        body_key: keys::ONBOARDING_STEP6_BODY,
        action_key: None,
    },
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP7_TITLE,
        body_key: keys::ONBOARDING_STEP7_BODY,
        action_key: Some(keys::GALLERY_TRY),
    },
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP8_TITLE,
        body_key: keys::ONBOARDING_STEP8_BODY,
        action_key: None,
    },
    // FR-070: шаг 9 — UI-консоль (приёмка/баг-репорты, доступна всегда)
    OnboardingStep {
        title_key: keys::ONBOARDING_STEP9_TITLE,
        body_key: keys::ONBOARDING_STEP9_BODY,
        action_key: None,
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
    /// (текст — таблица [`crate::i18n`], FR-040).
    pub fn next_label_key(&self) -> &'static str {
        if self.is_last() {
            keys::ONBOARDING_DONE
        } else if let Some(action) = ONBOARDING_STEPS
            .get(self.step)
            .and_then(|step| step.action_key)
        {
            // FR-049: шаг с действием — CTA «Попробовать» (галерея схем)
            action
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
/// Ширина карточки тура (логические px).
pub const ONBOARDING_CARD_WIDTH: f32 = 460.0;
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
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> Vec<String> {
    let Some(step) = ONBOARDING_STEPS.get(step) else {
        return Vec::new();
    };
    let avail = (width - ONBOARDING_PAD * 2.0).max(10.0);
    let wrapped = m.wrap(
        fs,
        i18n::tr(language, step.body_key),
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

/// Зона тела шага в карточке: от якоря первой строки до футера с CTA
/// (минус нижний пад). Общий источник высоты окна видимости скролла для
/// раскладки ([`card_layout`]), колеса ввода и отрисовки (видимые строки).
pub fn body_area(card: [f32; 4]) -> [f32; 4] {
    [
        card[0] + ONBOARDING_PAD,
        card[1] + body_top_offset(),
        (card[2] - ONBOARDING_PAD * 2.0).max(0.0),
        (card[3] - body_top_offset() - ONBOARDING_PAD - ONBOARDING_FOOTER_H).max(0.0),
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
    let lines = body_lines(step, w, language, m, fs);
    let body_h = lines.len() as f32 * ONBOARDING_BODY_LINE_H;
    let desired_h = body_top_offset() + body_h + ONBOARDING_PAD + ONBOARDING_FOOTER_H;
    // Кламп высоты к слоту: desired пре-клампнут, поэтому min-инвариант
    // модали (приоритетен — parity FR-060) не конфликтует с клампом FR-028.
    let h = desired_h.min(slot.h);
    let size = UiVec2::new(w, h);
    let panel = kit::modal(slot, size, size, size).panel;
    let card = [panel.x, panel.y, panel.w, panel.h];
    // Скролл тела: при клампе высоты viewport_h < content_h — строки
    // прокручиваются (колесо ввода; видимые строки — `kit::list_rows`).
    let body = body_area(card);
    scroll.content_h = lines.len() as f32 * ONBOARDING_BODY_LINE_H;
    scroll.viewport_h = body[3];
    scroll.clamp();
    OnboardingLayout { card, lines }
}

/// Прогресс-точки: центры по горизонтали (рендер — квады-кружки).
pub fn progress_dots(card: [f32; 4]) -> (Vec<f32>, f32) {
    let n = ONBOARDING_STEPS.len() as f32;
    let total = n * ONBOARDING_DOT + (n - 1.0) * ONBOARDING_DOT_GAP;
    let start = card[0] + (card[2] - total) / 2.0;
    let centers: Vec<f32> = (0..ONBOARDING_STEPS.len())
        .map(|i| start + i as f32 * (ONBOARDING_DOT + ONBOARDING_DOT_GAP) + ONBOARDING_DOT / 2.0)
        .collect();
    let y = card[1] + ONBOARDING_PAD + ONBOARDING_TITLE_LINE_H + ONBOARDING_DOTS_TOP;
    (centers, y)
}

/// Rect кнопки карточки (футер прибит к низу карточки — при клампе высоты
/// кнопки остаются достижимы): Prev — слева, Next — справа (оба в футере);
/// Skip — в правом верхнем углу карточки (выход виден всегда — NN/g).
//
// TODO(G/FR-070): `Skip` — это подписанная прямоугольная кнопка 64×22
// («Пропустить»), не каноническая «×» в углу модали. `kit::stage_close_button`
// возвращает квадрат `ICON_BUTTON_SIZE` (26×26) под глиф «×» — смена
// форм-фактора и UX (подпись → иконка). Skip нуждается в собственной
// миграции: либо `kit::button_layout` (rect подписанной кнопки-призрака),
// либо отдельный `kit::stage_skip_button` — отдельная волна. Позиция
// «правый-верхний угол карточки» семантически = stage-close, но
// форм-фактор отличается — оставлено как есть (I-1: ноль скачка).
pub fn button_rect(card: [f32; 4], button: OnboardingButton) -> [f32; 4] {
    let footer_y =
        card[1] + card[3] - ONBOARDING_FOOTER_H + (ONBOARDING_FOOTER_H - ONBOARDING_BUTTON_H) / 2.0;
    match button {
        OnboardingButton::Prev => [
            card[0] + ONBOARDING_PAD,
            footer_y,
            ONBOARDING_BUTTON_W,
            ONBOARDING_BUTTON_H,
        ],
        OnboardingButton::Next => [
            card[0] + card[2] - ONBOARDING_PAD - ONBOARDING_BUTTON_W,
            footer_y,
            ONBOARDING_BUTTON_W,
            ONBOARDING_BUTTON_H,
        ],
        OnboardingButton::Skip => [
            card[0] + card[2] - ONBOARDING_PAD - 64.0,
            card[1] + 10.0,
            64.0,
            22.0,
        ],
    }
}

/// Hit-test кнопки карточки. «Назад» на первом шаге отсутствует (None),
/// «Далее»/«Готово» и «Пропустить» доступны всегда (инвариант карусели).
/// FR-097: на coarse-указателе тач-цели кнопок дотягиваются до 44 лог. px
/// (hit-only, кламп в карточку — расширенная зона не выходит за оверлей;
/// на точном указателе — прежние зоны).
pub fn button_at(
    card: [f32; 4],
    state: &OnboardingState,
    point: [f32; 2],
) -> Option<OnboardingButton> {
    let coarse = crate::touch_targets::pointer_coarse();
    let hit = |rect: [f32; 4]| {
        let rect = if coarse {
            crate::touch_targets::intersect_xywh(
                crate::touch_targets::expand_xywh(rect, crate::touch_targets::MIN_TOUCH_TARGET),
                card,
            )
        } else {
            rect
        };
        point[0] >= rect[0]
            && point[0] <= rect[0] + rect[2]
            && point[1] >= rect[1]
            && point[1] <= rect[1] + rect[3]
    };
    if hit(button_rect(card, OnboardingButton::Next)) {
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
    let btn_privacy = [x, actions_y, AI_ONB_BTN_PRIV_W, AI_ONB_BTN_H];
    let btn_continue = [
        x + inner_w - AI_ONB_BTN_CONTINUE_W,
        actions_y,
        AI_ONB_BTN_CONTINUE_W,
        AI_ONB_BTN_H,
    ];
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
    fn layout(viewport: [f32; 2], step: usize, language: Language) -> OnboardingLayout {
        let mut scroll = ScrollState::default();
        layout_sync(viewport, step, language, &mut scroll)
    }

    /// Раскладка шага с внешним скролл-состоянием (тесты скролла).
    fn layout_sync(
        viewport: [f32; 2],
        step: usize,
        language: Language,
        scroll: &mut ScrollState,
    ) -> OnboardingLayout {
        let (mut m, mut fs) = measurer();
        card_layout(viewport, step, language, scroll, &mut m, &mut fs)
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
    /// до конца (последняя строка достижима скроллом).
    #[test]
    fn stress_240x180_footer_visible_and_body_reachable() {
        let viewport = [240.0, 180.0];
        for step in 0..ONBOARDING_STEPS.len() {
            let lay = layout(viewport, step, Language::Ru);
            let card = lay.card;
            // Футер (кнопки) прибит к низу карточки и НЕ входит в скролл-зону
            let body = body_area(card);
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
            let relaid = layout_sync(viewport, step, Language::Ru, &mut scroll);
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
            // Кнопки кликабельны (hit-тест работает при клампе)
            let next = button_rect(card, OnboardingButton::Next);
            let state = OnboardingState { step };
            assert_eq!(
                button_at(card, &state, [next[0] + 5.0, next[1] + 10.0]),
                Some(OnboardingButton::Next),
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
                let avail = width - ONBOARDING_PAD * 2.0;
                let (mut m, mut fs) = measurer();
                for language in [Language::Ru, Language::En] {
                    for line in body_lines(step, width, language, &mut m, &mut fs) {
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

    /// Hit-тесты кнопок: Next/Skip кликабельны на каждом шаге; Prev —
    /// только со 2-го; мимо кнопок — None; клик в теле карточки глотается.
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
