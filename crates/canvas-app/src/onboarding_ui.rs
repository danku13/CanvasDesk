//! FR-028: онбординг по функциям сервиса — чистая модель (образец
//! [`crate::settings_ui`]): шаги-карточки ([`ONBOARDING_STEPS`], скоуп
//! владельца — база + расчёты + шаблоны), таблица решений авто-показа
//! ([`should_show_onboarding`]: не пройден И откладываний < 3), машина
//! карусели ([`OnboardingState`]: `next`/`prev` с клампом, подпись
//! «Далее»/«Готово») и геометрия карточки с кнопками и клампом к окну.
//!
//! Рендер и ввод — приложение (`main.rs`): затемнение канваса (паттерн
//! FR-022), карточка поверх, ввод канваса под ней блокируется. Поля
//! `onboarding_done`/`onboarding_defers` и кламп — `canvas-core`
//! settings.rs (схема `config.toml`, `#[serde(default)]`).

use canvas_core::{Language, Settings, ONBOARDING_MAX_DEFERS};

use crate::i18n::{self, keys};

/// Автопоказ тура при старте (таблица решений FR-028): не пройден до конца
/// И отложен менее `ONBOARDING_MAX_DEFERS` раз. Ручной вход из меню «?»
/// этой функцией не гейтится (явное намерение пользователя).
pub fn should_show_onboarding(settings: &Settings) -> bool {
    !settings.onboarding_done && settings.onboarding_defers < ONBOARDING_MAX_DEFERS
}

/// Консервативная оценка ширины глифа (доля от кегля) — ЛОКАЛЬНАЯ
/// эвристика онбординга (FR-054: эвристика `docs_ui::text_width` удалена
/// вместе с миграцией доков на измеренный текст; перенос онбординга
/// сознательно НЕ переведён на TextMeasurer — решение владельца «онбординг
/// не дорабатывать, пользовательских изменений нет»: консервативная
/// переоценка переносит строку раньше фактической границы и сохраняет
/// прежние точки переноса карточек).
const CHAR_W_FACTOR: f32 = 0.62;
/// Оценка ширины пробела (доля от кегля).
const SPACE_W_FACTOR: f32 = 0.34;

/// Оценка ширины текста (логические px) по кеглю — прежний контракт
/// `docs_ui::text_width`, живёт рядом с единственным потребителем.
fn text_width(text: &str, font: f32) -> f32 {
    text.chars()
        .map(|c| {
            if c == ' ' || c == '\u{00a0}' {
                SPACE_W_FACTOR
            } else {
                CHAR_W_FACTOR
            }
        })
        .sum::<f32>()
        * font
}

/// Шаг тура: заголовок и абзац тела (короткие тексты, перенос по ширине
/// карточки — локальная консервативная оценка `text_width`). Зарезервированный
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

/// Строки тела шага после переноса по ширине карточки (консервативная
/// оценка ширины глифа — перенос раньше реальной границы, строки не
/// вылезают за клип). Один источник для высоты карточки ([`card_rect`])
/// и рендера (main.rs) — раскладка и геометрия не разъезжаются.
pub fn body_lines(step: usize, width: f32, language: Language) -> Vec<String> {
    let Some(step) = ONBOARDING_STEPS.get(step) else {
        return Vec::new();
    };
    let avail = (width - ONBOARDING_PAD * 2.0).max(10.0);
    let mut lines: Vec<String> = Vec::new();
    let mut cur_w = 0.0;
    for word in i18n::tr(language, step.body_key).split(' ') {
        let w = text_width(word, ONBOARDING_BODY_FONT);
        let need = w + if cur_w > 0.0 {
            ONBOARDING_BODY_FONT * SPACE_W_FACTOR
        } else {
            0.0
        };
        if need <= avail - cur_w {
            match lines.last_mut() {
                Some(line) => {
                    line.push(' ');
                    line.push_str(word);
                }
                None => lines.push(word.to_owned()),
            }
            cur_w += need;
        } else {
            lines.push(word.to_owned());
            cur_w = w.min(avail);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Отступ от верха карточки до первой строки тела (заголовок +
/// прогресс-точки + зазор) — общий для геометрии и рендера.
pub fn body_top_offset() -> f32 {
    ONBOARDING_PAD + ONBOARDING_TITLE_LINE_H + ONBOARDING_DOTS_TOP + ONBOARDING_DOT + 10.0
}

/// Rect карточки тура `[x, y, w, h]`: центр окна, ширина клампится к окну,
/// высота — по контенту шага (заголовок + точки + тело + футер), на
/// маленьких окнах клампится к высоте окна (инвариант FR-028).
pub fn card_rect(viewport: [f32; 2], step: usize, language: Language) -> [f32; 4] {
    let w = ONBOARDING_CARD_WIDTH.min(viewport[0].max(0.0));
    let body_h = body_lines(step, w, language).len() as f32 * ONBOARDING_BODY_LINE_H;
    let h = (body_top_offset() + body_h + ONBOARDING_PAD + ONBOARDING_FOOTER_H)
        .min(viewport[1].max(0.0));
    [
        ((viewport[0] - w) / 2.0).max(0.0),
        ((viewport[1] - h) / 2.0).max(0.0),
        w,
        h,
    ]
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

/// Rect кнопки карточки: Prev — слева, Next — справа (оба в футере);
/// Skip — в правом верхнем углу карточки (выход виден всегда — NN/g).
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
pub fn button_at(
    card: [f32; 4],
    state: &OnboardingState,
    point: [f32; 2],
) -> Option<OnboardingButton> {
    let hit = |rect: [f32; 4]| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_core::ONBOARDING_MAX_DEFERS;

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

    /// Инвариант клампа: карточка целиком внутри окна на любом viewport
    /// (включая 320×240) и любом шаге; ширина не больше окна.
    #[test]
    fn card_clamped_to_window() {
        for viewport in [
            [1600.0, 900.0],
            [1280.0, 720.0],
            [320.0, 240.0],
            [240.0, 180.0],
        ] {
            for step in 0..ONBOARDING_STEPS.len() {
                let card = card_rect(viewport, step, Language::Ru);
                assert!(card[0] >= 0.0, "за левым краем: {card:?}");
                assert!(card[1] >= 0.0, "за верхним краем: {card:?}");
                assert!(
                    card[0] + card[2] <= viewport[0] + 1.0,
                    "за правым: {card:?}"
                );
                assert!(card[1] + card[3] <= viewport[1] + 1.0, "за низом: {card:?}");
                assert!(card[2] <= viewport[0] + 1.0, "шире окна: {card:?}");
                assert!(card[3] <= viewport[1] + 1.0, "выше окна: {card:?}");
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

    /// Hit-тесты кнопок: Next/Skip кликабельны на каждом шаге; Prev —
    /// только со 2-го; мимо кнопок — None; клик в теле карточки глотается.
    #[test]
    fn button_hit_tests() {
        let viewport = [1600.0, 900.0];
        let mut state = OnboardingState::default();
        let card = card_rect(viewport, state.step, Language::Ru);
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
        let card = card_rect(viewport, state.step, Language::Ru);
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
        let card = card_rect(viewport, 0, Language::Ru);
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
            .map(|step| card_rect(viewport, step, Language::Ru)[3])
            .collect();
        assert!(heights.iter().all(|&h| h > 100.0), "карточка не пустая");
        // Узкое окно не даёт карточке вылезти
        let narrow = card_rect([320.0, 240.0], 0, Language::Ru);
        assert!(narrow[2] <= 320.0);
    }
}
