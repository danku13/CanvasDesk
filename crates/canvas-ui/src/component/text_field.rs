//! FR-068 W3: текстовое поле (модель/каретка/выделение/вёрстка) — перенос из kit.rs 1:1 (W3).
//!
//! W3 (агент 3-c): слой компонентной модели ДОБАВЛЕН — [`TextFieldProps`]/
//! [`TextField`] + `impl Component` (layout/paint/hit_test); [`TextFieldModel`]
//! и [`text_field`] — стабильный API (FR-058), не менялись.

use super::{
    Component, KitPalette, KitState, TEXT_FIELD_HEIGHT, TEXT_FIELD_MIN_W, TEXT_FIELD_PAD_H,
};
use crate::geometry::{UiRect, UiVec2};
use crate::layout::{constrain, stack, HAlign, LayoutBackend, VAlign};
use crate::measure::TextMeasurer;
use crate::paint::Painter;
use crate::widget::WidgetState;

// === FR-058: Компоненты кита v2 ============================================
//
// Чистые модели/функции в стиле kit v1: геометрия + стиль + модель состояния;
// рисование — через Painter (FR-057), ввод не перехватывают, событий не владеют.
//
// Инварианты (замороженные контракты FR-059/060 кодируют против них):
// - каретка/селекция `TextFieldModel` — в СИМВОЛАХ (`chars().count()`), не
//   байтах; IME/UTF-16-конвертация — на стороне ввода потребителя;
// - `list_rows` — чистая функция (без мутаций `ScrollState`);
// - `switch`/`card` — только геометрия и слоты (цвет — отдельной функцией);
// - 0 новых внешних зависимостей (G7); Slider НЕ включён (спекулятивный
//   компонент без потребителя — вернуть в постановку при появлении экрана
//   со слайдером).

// --- TextField --------------------------------------------------------------

/// Модель текстового поля: текст + каретка + селекция. Позиции — в СИМВОЛАХ
/// (`chars().count()`), не байтах: вставка/удаление/движение корректны на
/// юникоде (emoji, multi-byte). IME/UTF-16-конвертация — на стороне ввода
/// потребителя (контракт FR-058).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextFieldModel {
    /// Текст поля.
    pub text: String,
    /// Позиция каретки в СИМВОЛАХ (`chars().count()` от начала).
    pub caret: usize,
    /// Селекция `(anchor, head)` в символах; `None` — нет селекции.
    /// `head` — текущая позиция каретки; `anchor` — начало выделения.
    pub sel: Option<(usize, usize)>,
}

impl TextFieldModel {
    /// Вставить строку на месте каретки (или заменить селекцию).
    pub fn insert(&mut self, s: &str) {
        let (start, end) = self.selection_range();
        let chars: Vec<char> = self.text.chars().collect();
        let insert_chars: Vec<char> = s.chars().collect();
        let mut new_chars: Vec<char> =
            Vec::with_capacity(chars.len() + insert_chars.len() - (end - start));
        new_chars.extend_from_slice(&chars[..start]);
        new_chars.extend_from_slice(&insert_chars);
        new_chars.extend_from_slice(&chars[end..]);
        self.text = new_chars.into_iter().collect();
        self.caret = start + insert_chars.len();
        self.sel = None;
    }

    /// Backspace: удалить символ перед кареткой (или селекцию).
    pub fn backspace(&mut self) {
        if self.sel.is_some() {
            self.delete_selection();
            return;
        }
        if self.caret == 0 {
            return;
        }
        let mut chars: Vec<char> = self.text.chars().collect();
        chars.remove(self.caret - 1);
        self.text = chars.into_iter().collect();
        self.caret -= 1;
    }

    /// Delete: удалить символ после каретки (или селекцию).
    pub fn delete(&mut self) {
        if self.sel.is_some() {
            self.delete_selection();
            return;
        }
        let total = self.text.chars().count();
        if self.caret >= total {
            return;
        }
        let mut chars: Vec<char> = self.text.chars().collect();
        chars.remove(self.caret);
        self.text = chars.into_iter().collect();
    }

    /// Сдвинуть каретку на `chars` символов (отрицательное — влево).
    /// `extend = true` — расширять селекцию (shift+стрелки).
    pub fn move_caret(&mut self, chars: isize, extend: bool) {
        let total = self.text.chars().count();
        let new_pos = (self.caret as isize + chars).max(0).min(total as isize) as usize;
        if extend {
            match self.sel {
                None => self.sel = Some((self.caret, new_pos)),
                Some((anchor, _)) => self.sel = Some((anchor, new_pos)),
            }
        } else {
            self.sel = None;
        }
        self.caret = new_pos;
    }

    /// Выделить весь текст.
    pub fn select_all(&mut self) {
        let total = self.text.chars().count();
        self.sel = Some((0, total));
        self.caret = total;
    }

    /// Снять выделение (каретка остаётся на месте).
    pub fn clear_selection(&mut self) {
        self.sel = None;
    }

    /// Заменить текст целиком; каретка — в конец, селекция снята.
    pub fn set_text(&mut self, s: String) {
        let total = s.chars().count();
        self.text = s;
        self.caret = total;
        self.sel = None;
    }

    /// Диапазон удаления (start, end) в символах: селекция или пустая каретка.
    fn selection_range(&self) -> (usize, usize) {
        match self.sel {
            Some((a, b)) => (a.min(b), a.max(b)),
            None => (self.caret, self.caret),
        }
    }

    /// Удалить выделенный диапазон (приватный — публично через backspace/delete).
    fn delete_selection(&mut self) {
        let (start, end) = self.selection_range();
        let chars: Vec<char> = self.text.chars().collect();
        let new_chars: Vec<char> = chars[..start]
            .iter()
            .chain(&chars[end..])
            .copied()
            .collect();
        self.text = new_chars.into_iter().collect();
        self.caret = start;
        self.sel = None;
    }
}

/// Раскладка текстового поля. `caret_x = -1.0` — каретка не рисуется (поле не
/// в фокусе); иначе — x-координата каретки в `text_area` по замеру префикса.
#[derive(Debug, Clone, PartialEq)]
pub struct TextFieldLayout {
    /// Rect поля (constrain+stack в слоте).
    pub rect: UiRect,
    /// Внутренняя область текста (минус горизонтальный пад).
    pub text_area: UiRect,
    /// X каретки в `text_area` (по замеру текста до каретки); `-1.0` — нет каретки.
    pub caret_x: f32,
    /// Отображаемый текст: placeholder (если пусто) или сам текст — с `ellipsis`
    /// по ширине `text_area`.
    pub text_shown: String,
}

/// Текстовое поле в слоте. Стиль (фон/рамка/фокус-рамка) — отдельной функцией
/// (`control_style_of`/`button_style` — потребитель красит через Painter);
/// контракт: цвет отдельно от геометрии. `focused` управляет видимостью
/// каретки; `state`/`p` зарезервированы для будущих расширений стиля.
#[allow(clippy::too_many_arguments)]
pub fn text_field(
    slot: UiRect,
    min: UiVec2,
    max: UiVec2,
    model: &TextFieldModel,
    placeholder: &str,
    focused: bool,
    _state: KitState,
    _p: &KitPalette,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    size: f32,
) -> TextFieldLayout {
    // Стиль — отдельной функцией потребителя (цвет отдельно от геометрии).
    let _ = (_state, _p);
    // Размер: constrain(min, max, desired=slot), позиция — stack по центру.
    let desired = max;
    let sz = constrain(min, max, desired);
    let rect = stack(slot, sz, HAlign::Center, VAlign::Center);
    // Пад: SPACING_SM горизонтально; вертикаль — вся высота rect.
    let text_area = UiRect::new(
        rect.x + TEXT_FIELD_PAD_H,
        rect.y,
        (rect.w - TEXT_FIELD_PAD_H * 2.0).max(0.0),
        rect.h,
    );
    // Текст/плейсхолдер: пустое → placeholder с ellipsis; иначе текст с ellipsis.
    // Каретка: по замеру префикса до caret в ИСХОДНОМ тексте, клампленный к
    // text_area; -1.0 — не сфокусировано (потребитель не рисует каретку).
    let (text_shown, caret_x) = if model.text.is_empty() {
        let ph = if placeholder.is_empty() {
            String::new()
        } else {
            m.ellipsis(fs, placeholder, family, size, text_area.w)
        };
        let cx = if focused { text_area.x } else { -1.0 };
        (ph, cx)
    } else {
        let shown = m.ellipsis(fs, &model.text, family, size, text_area.w);
        let chars: Vec<char> = model.text.chars().collect();
        let caret_idx = model.caret.min(chars.len());
        let prefix: String = chars[..caret_idx].iter().collect();
        let prefix_w = m.width_of(fs, &prefix, family, size);
        let cx = if focused {
            text_area.x + prefix_w.min(text_area.w)
        } else {
            -1.0
        };
        (shown, cx)
    };
    TextFieldLayout {
        rect,
        text_area,
        caret_x,
        text_shown,
    }
}

// --- TextField: компонентная модель (FR-068 W3) -----------------------------

/// Семейство дежурной раскладки [`TextField::layout`] — те же строковые
/// данные, что в тестах кита (`test_support::FAMILY`) и `Family::Name`
/// рендера (`canvas-render/text.rs`, CR-015).
const FONT_FAMILY: &str = "Noto Sans Display";

/// Кегль дежурной раскладки [`TextField::layout`] — как в тестах кита и в
/// пилоте (`canvas-app/overlays.rs`: label текстового поля 13.0).
const FONT_SIZE: f32 = 13.0;

thread_local! {
    /// Дежурный пул шрифтов standalone-раскладки [`TextField::layout`].
    ///
    /// `FontSystem::new()` дорогой (скан системных шрифтов — см. доку
    /// `shaper.rs` «Почему собственный пул — lazy»), а сигнатура
    /// [`Component::layout`] пул потребителя НЕ передаёт (инвариант «замер
    /// тем же пулом, что рендер», CR-015, на компонентном уровне W3 даёт
    /// потребитель, вызывающий [`text_field`] напрямую со своим пулом —
    /// горячий путь). Standalone-путь лениво создаёт ОДИН пул на поток:
    /// стоимость — первый вызов, далее переиспользуется.
    static FONT_POOL: std::cell::RefCell<Option<cosmic_text::FontSystem>> =
        const { std::cell::RefCell::new(None) };
}

/// Выполнить `f` с дежурным пулом ([`FONT_POOL`], ленивая инициализация).
fn with_font_pool<R>(f: impl FnOnce(&mut cosmic_text::FontSystem) -> R) -> R {
    FONT_POOL.with(|slot| {
        let mut slot = slot.borrow_mut();
        f(slot.get_or_insert_with(cosmic_text::FontSystem::new))
    })
}

/// Свойства [`TextField`] (декларативный вход кадра; FR-068 W3).
#[derive(Debug, Clone, PartialEq)]
pub struct TextFieldProps {
    /// Подсказка пустого поля (рисует потребитель — `Painter::label` по
    /// `text_area` из [`TextFieldLayout`]).
    pub placeholder: String,
    /// Желаемая ширина поля (ui px); клампится к [`TEXT_FIELD_MIN_W`]
    /// (высота — [`TEXT_FIELD_HEIGHT`], константы кита).
    pub width: f32,
    /// Срез слотов палитры (контракт F-8: цвета — только слоты).
    pub palette: KitPalette,
}

/// Текстовое поле — retained-компонент (FR-068 W3): `Props` + стабильная
/// [`TextFieldModel`] (FR-058) + [`WidgetState`] (FR-057; `is_focused` →
/// видимость каретки, `kit_state` — слот стиля состояния).
///
/// Разделение труда (контракт кита «цвет отдельно от геометрии», ввод
/// компонент не перехватывает): [`Component::layout`] — rect КОНТЕЙНЕРА
/// ([`text_field`]); [`Component::paint`] — ТОЛЬКО контейнер (заливка/рамка
/// слотами + радиус шкалы). Текст/placeholder/каретку рисует ПОТРЕБИТЕЛЬ
/// (`Painter::label` по `text_area` из [`TextFieldLayout`], каретка 1.5 px
/// слотом `accent` — образец `canvas-app/overlays.rs`, секция TextField):
/// усечение `ellipsis`/замер префикса — layout-данные с `TextMeasurer`,
/// у paint замера нет — рисовать текст здесь недетерминированно по усечению.
#[derive(Debug, Clone, PartialEq)]
pub struct TextField {
    /// Свойства кадра.
    pub props: TextFieldProps,
    /// Модель текста/каретки/селекции (стабильный API FR-058).
    pub model: TextFieldModel,
    /// Состояние виджета (hover/press/focus/…; FR-057).
    pub state: WidgetState,
}

impl TextField {
    /// Новое поле: модель и состояние — дефолтные (пустой текст, Normal).
    pub fn new(props: TextFieldProps) -> Self {
        Self {
            props,
            model: TextFieldModel::default(),
            state: WidgetState::default(),
        }
    }
}

impl Component for TextField {
    type Props = TextFieldProps;

    fn props(&self) -> &Self::Props {
        &self.props
    }

    /// Rect контейнера в слоте: [`text_field`] (constrain+stack по центру;
    /// ширина — `props.width` с клампом к [`TEXT_FIELD_MIN_W`], высота —
    /// [`TEXT_FIELD_HEIGHT`]). Backend не используется: геометрия поля —
    /// чистая функция слота (паритет Native/Flex/Taffy тривиален, §Контракт-3
    /// FR-068). Фокус каретки — `state.is_focused()`, состояние стиля —
    /// `state.kit_state()` (сейчас `text_field` резервирует оба входа).
    fn layout(&self, _backend: &dyn LayoutBackend, slot: UiRect) -> Vec<UiRect> {
        let min = UiVec2::new(TEXT_FIELD_MIN_W, TEXT_FIELD_HEIGHT);
        let max = UiVec2::new(self.props.width, TEXT_FIELD_HEIGHT);
        let mut m = TextMeasurer::new();
        let lay = with_font_pool(|fs| {
            text_field(
                slot,
                min,
                max,
                &self.model,
                &self.props.placeholder,
                self.state.is_focused(),
                self.state.kit_state(),
                &self.props.palette,
                &mut m,
                fs,
                FONT_FAMILY,
                FONT_SIZE,
            )
        });
        vec![lay.rect]
    }

    /// ТОЛЬКО контейнер (см. доку [`TextField`]): слоты `control_fill` /
    /// (`control_border`, в фокусе — `accent`) / радиус RADIUS_CHIP — те же,
    /// что у потребителя-пилота (`canvas-app/overlays.rs`). Пустой `rects` —
    /// 0 items (нечего рисовать). Текст/каретку НЕ рисует.
    fn paint(&self, painter: &mut Painter, rects: &[UiRect]) {
        let Some(&rect) = rects.first() else {
            return;
        };
        let border = if self.state.is_focused() {
            self.props.palette.accent
        } else {
            self.props.palette.control_border
        };
        let style = super::panel::control_style_of(
            self.props.palette.control_fill,
            border,
            self.props.palette.text,
            canvas_core::tokens::RADIUS_CHIP,
        );
        painter.control(rect, &style);
    }

    // hit_test — дефолтный: ComponentHit::pick по rects (index 0 — контейнер).
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::test_support::{font_system, palette_a, palette_b, FAMILY};
    use crate::component::ComponentHit;
    use crate::geometry::UiPoint;
    use crate::layout::default_backend;
    use crate::paint::{PaintItem, Painter};
    #[test]
    fn text_field_insert_at_start_middle_end() {
        let mut m = TextFieldModel::default();
        m.set_text("hello".to_owned());
        // Вставка в начало
        m.caret = 0;
        m.insert("X");
        assert_eq!(m.text, "Xhello");
        assert_eq!(m.caret, 1);
        // Вставка в середину
        m.caret = 3;
        m.insert("Y");
        assert_eq!(m.text, "XheYllo");
        assert_eq!(m.caret, 4);
        // Вставка в конец
        m.caret = m.text.chars().count();
        m.insert("Z");
        assert_eq!(m.text, "XheYlloZ");
        assert_eq!(m.caret, 8);
    }
    #[test]
    fn text_field_insert_replaces_selection() {
        let mut m = TextFieldModel::default();
        m.set_text("hello world".to_owned());
        // Выделить "lo wo"
        m.sel = Some((3, 8));
        m.caret = 8;
        m.insert("XYZ");
        assert_eq!(m.text, "helXYZrld");
        assert_eq!(m.caret, 6);
        assert!(m.sel.is_none(), "селекция снята после insert");
    }
    #[test]
    fn text_field_backspace_at_start_is_noop() {
        let mut m = TextFieldModel::default();
        m.set_text("abc".to_owned());
        m.caret = 0;
        m.backspace();
        assert_eq!(m.text, "abc");
        assert_eq!(m.caret, 0);
    }
    #[test]
    fn text_field_backspace_deletes_char_before_caret() {
        let mut m = TextFieldModel::default();
        m.set_text("abc".to_owned());
        m.caret = 2;
        m.backspace();
        assert_eq!(m.text, "ac");
        assert_eq!(m.caret, 1);
    }
    #[test]
    fn text_field_backspace_deletes_selection() {
        let mut m = TextFieldModel::default();
        m.set_text("hello".to_owned());
        m.sel = Some((1, 4)); // "ell"
        m.caret = 4;
        m.backspace();
        assert_eq!(m.text, "ho");
        assert_eq!(m.caret, 1);
        assert!(m.sel.is_none());
    }
    #[test]
    fn text_field_delete_at_end_is_noop() {
        let mut m = TextFieldModel::default();
        m.set_text("abc".to_owned());
        m.caret = 3;
        m.delete();
        assert_eq!(m.text, "abc");
        assert_eq!(m.caret, 3);
    }
    #[test]
    fn text_field_delete_deletes_char_after_caret() {
        let mut m = TextFieldModel::default();
        m.set_text("abc".to_owned());
        m.caret = 0;
        m.delete();
        assert_eq!(m.text, "bc");
        assert_eq!(m.caret, 0);
    }
    #[test]
    fn text_field_delete_deletes_selection() {
        let mut m = TextFieldModel::default();
        m.set_text("hello".to_owned());
        m.sel = Some((1, 4));
        m.caret = 1;
        m.delete();
        assert_eq!(m.text, "ho");
        assert_eq!(m.caret, 1);
        assert!(m.sel.is_none());
    }
    #[test]
    fn text_field_move_caret_left_right_clamps_at_edges() {
        let mut m = TextFieldModel::default();
        m.set_text("abc".to_owned());
        // Левее начала — кламп к 0
        m.move_caret(-5, false);
        assert_eq!(m.caret, 0);
        // Правее конца — кламп к 3
        m.move_caret(10, false);
        assert_eq!(m.caret, 3);
        // В середину
        m.move_caret(-1, false);
        assert_eq!(m.caret, 2);
        assert!(m.sel.is_none(), "без extend — селекция снята");
    }
    #[test]
    fn text_field_move_caret_extend_creates_and_extends_selection() {
        let mut m = TextFieldModel::default();
        m.set_text("hello".to_owned());
        m.caret = 3;
        // extend влево — селекция (3, 2)
        m.move_caret(-1, true);
        assert_eq!(m.caret, 2);
        assert_eq!(m.sel, Some((3, 2)));
        // extend дальше — anchor сохраняется, head движется
        m.move_caret(-1, true);
        assert_eq!(m.caret, 1);
        assert_eq!(m.sel, Some((3, 1)));
        // extend вправо — head обратно к anchor
        m.move_caret(2, true);
        assert_eq!(m.caret, 3);
        assert_eq!(m.sel, Some((3, 3)));
    }
    #[test]
    fn text_field_select_all_and_clear_selection() {
        let mut m = TextFieldModel::default();
        m.set_text("hello".to_owned());
        m.select_all();
        assert_eq!(m.sel, Some((0, 5)));
        assert_eq!(m.caret, 5);
        m.clear_selection();
        assert!(m.sel.is_none());
        assert_eq!(m.caret, 5, "каретка остаётся после clear_selection");
    }
    #[test]
    fn text_field_set_text_resets_caret_and_selection() {
        let mut m = TextFieldModel::default();
        m.set_text("hello".to_owned());
        m.caret = 2;
        m.sel = Some((0, 2));
        m.set_text("new".to_owned());
        assert_eq!(m.text, "new");
        assert_eq!(m.caret, 3);
        assert!(m.sel.is_none());
    }
    /// Инвариант FR-058: каретка/селекция — в СИМВОЛАХ, не байтах.
    /// Тест на emoji (4-байтный глиф) и multi-byte (кириллица).
    #[test]
    fn text_field_unicode_emoji_and_cyrillic_positions() {
        let mut m = TextFieldModel::default();
        // 🎉 — U+1F389, 4 байта в UTF-8, 1 char (в utf-16 — 2 единицы).
        m.set_text("a🎉b".to_owned());
        assert_eq!(m.text.len(), 6, "4 байта для emoji + 2 ascii");
        assert_eq!(m.text.chars().count(), 3, "3 символа");
        // Каретка после emoji (position=2 в символах)
        m.caret = 2;
        m.insert("X");
        assert_eq!(m.text, "a🎉Xb");
        assert_eq!(m.caret, 3);
        // Backspace удаляет emoji целиком (1 символ)
        m.caret = 2;
        m.backspace();
        assert_eq!(m.text, "aXb");
        assert_eq!(m.caret, 1);
        // Кириллица (2 байта на символ в UTF-8)
        m.set_text("привет".to_owned());
        assert_eq!(m.text.len(), 12, "6 символов × 2 байта");
        assert_eq!(m.text.chars().count(), 6);
        m.caret = 3; // после "при" — delete() удаляет символ по индексу 3 ("в")
        m.delete();
        assert_eq!(m.text, "приет");
        assert_eq!(m.caret, 3);
        // Select all + delete selection — удаляет все 6 символов
        m.select_all();
        m.delete();
        assert_eq!(m.text, "");
        assert_eq!(m.caret, 0);
    }

    // --- TextFieldLayout: text_field() --------------------------------------
    #[test]
    fn text_field_layout_empty_shows_placeholder_and_caret_at_start() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let model = TextFieldModel::default();
        let slot = UiRect::new(0.0, 0.0, 200.0, TEXT_FIELD_HEIGHT);
        let lay = text_field(
            slot,
            UiVec2::new(TEXT_FIELD_MIN_W, TEXT_FIELD_HEIGHT),
            UiVec2::new(400.0, TEXT_FIELD_HEIGHT),
            &model,
            "Поиск…",
            true,
            KitState::Normal,
            &palette_a(),
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        assert_eq!(
            lay.text_shown, "Поиск…",
            "placeholder показан целиком (помещается)"
        );
        assert!(
            (lay.caret_x - lay.text_area.x).abs() < 0.01,
            "каретка у левого края"
        );
        assert!((lay.rect.h - TEXT_FIELD_HEIGHT).abs() < 0.01);
        // text_area уже rect на пад
        assert!((lay.text_area.x - (lay.rect.x + TEXT_FIELD_PAD_H)).abs() < 0.01);
        assert!((lay.text_area.w - (lay.rect.w - TEXT_FIELD_PAD_H * 2.0)).abs() < 0.01);
    }
    #[test]
    fn text_field_layout_empty_no_caret_when_not_focused() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let model = TextFieldModel::default();
        let slot = UiRect::new(0.0, 0.0, 200.0, TEXT_FIELD_HEIGHT);
        let lay = text_field(
            slot,
            UiVec2::new(TEXT_FIELD_MIN_W, TEXT_FIELD_HEIGHT),
            UiVec2::new(400.0, TEXT_FIELD_HEIGHT),
            &model,
            "Поиск…",
            false,
            KitState::Normal,
            &palette_a(),
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        assert_eq!(lay.caret_x, -1.0, "не сфокусировано — каретка не рисуется");
    }
    #[test]
    fn text_field_layout_non_empty_shows_text_and_caret_at_measured_prefix() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let mut model = TextFieldModel::default();
        model.set_text("hello".to_owned());
        model.caret = 2; // после "he"
        let slot = UiRect::new(0.0, 0.0, 200.0, TEXT_FIELD_HEIGHT);
        let lay = text_field(
            slot,
            UiVec2::new(TEXT_FIELD_MIN_W, TEXT_FIELD_HEIGHT),
            UiVec2::new(400.0, TEXT_FIELD_HEIGHT),
            &model,
            "",
            true,
            KitState::Normal,
            &palette_a(),
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        assert_eq!(lay.text_shown, "hello");
        let prefix_w = m.width_of(&mut fs, "he", FAMILY, 13.0);
        assert!((lay.caret_x - (lay.text_area.x + prefix_w)).abs() < 0.1);
    }
    #[test]
    fn text_field_layout_placeholder_ellipsized_when_too_wide() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let model = TextFieldModel::default();
        // Узкий слот — placeholder не помещается, усекается с ellipsis
        let slot = UiRect::new(0.0, 0.0, 30.0, TEXT_FIELD_HEIGHT);
        let lay = text_field(
            slot,
            UiVec2::new(0.0, TEXT_FIELD_HEIGHT),
            UiVec2::new(30.0, TEXT_FIELD_HEIGHT),
            &model,
            "Очень длинный placeholder",
            true,
            KitState::Normal,
            &palette_a(),
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        assert!(lay.text_shown != "Очень длинный placeholder");
        assert!(lay.text_shown.ends_with('\u{2026}') || lay.text_shown.is_empty());
    }
    #[test]
    fn text_field_layout_caret_clamped_to_text_area_when_prefix_too_wide() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let mut model = TextFieldModel::default();
        model.set_text("очень длинный текст не помещается в слот".to_owned());
        model.caret = model.text.chars().count(); // каретка в конце
        let slot = UiRect::new(0.0, 0.0, 50.0, TEXT_FIELD_HEIGHT);
        let lay = text_field(
            slot,
            UiVec2::new(0.0, TEXT_FIELD_HEIGHT),
            UiVec2::new(50.0, TEXT_FIELD_HEIGHT),
            &model,
            "",
            true,
            KitState::Normal,
            &palette_a(),
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        // Каретка клампнута к правому краю text_area
        assert!(lay.caret_x <= lay.text_area.right() + 0.01);
        assert!(lay.caret_x >= lay.text_area.x);
    }

    // --- TextField: компонентная модель (FR-068 W3) --------------------------
    #[test]
    fn text_field_component_layout_returns_container_rect_in_slot() {
        let tf = TextField::new(TextFieldProps {
            placeholder: "Поиск…".to_owned(),
            width: 200.0,
            palette: palette_a(),
        });
        let slot = UiRect::new(0.0, 0.0, 300.0, 60.0);
        let rects = tf.layout(default_backend(), slot);
        assert_eq!(rects.len(), 1, "layout — один rect контейнера");
        // Ширина = props.width, высота = TEXT_FIELD_HEIGHT, центр слота.
        assert!((rects[0].w - 200.0).abs() < 0.01);
        assert!((rects[0].h - TEXT_FIELD_HEIGHT).abs() < 0.01);
        assert!((rects[0].x - (300.0 - 200.0) / 2.0).abs() < 0.01);
        let cy = slot.y + (slot.h - TEXT_FIELD_HEIGHT) / 2.0;
        assert!((rects[0].y - cy).abs() < 0.01);
        // Ширина клампится к TEXT_FIELD_MIN_W (width=10 → 80).
        let narrow = TextField::new(TextFieldProps {
            placeholder: String::new(),
            width: 10.0,
            palette: palette_b(),
        });
        let rects = narrow.layout(default_backend(), slot);
        assert!((rects[0].w - TEXT_FIELD_MIN_W).abs() < 0.01, "кламп к min");
    }
    #[test]
    fn text_field_component_paint_emits_container_only() {
        let palette = palette_a();
        let mut focused = TextField::new(TextFieldProps {
            placeholder: "Поиск…".to_owned(),
            width: 200.0,
            palette,
        });
        focused.state.set_focused(true);
        let slot = UiRect::new(0.0, 0.0, 300.0, TEXT_FIELD_HEIGHT);
        let rects = focused.layout(default_backend(), slot);
        let mut p = Painter::new();
        focused.paint(&mut p, &rects);
        assert!(!p.items().is_empty(), "paint эмитит контейнер");
        assert_eq!(
            p.items()[0],
            PaintItem::Rect {
                rect: rects[0],
                fill: palette.control_fill,
                border: palette.accent,
                radius: canvas_core::tokens::RADIUS_CHIP,
            },
            "в фокусе: fill control_fill, рамка accent, радиус шкалы"
        );
        // Без фокуса — рамка control_border; ТЕКСТА/каретки в items нет
        // (paint рисует только контейнер — доку TextField).
        let unfocused = TextField::new(TextFieldProps {
            placeholder: String::new(),
            width: 200.0,
            palette,
        });
        let mut p = Painter::new();
        unfocused.paint(&mut p, &rects);
        assert_eq!(
            p.items()[0],
            PaintItem::Rect {
                rect: rects[0],
                fill: palette.control_fill,
                border: palette.control_border,
                radius: canvas_core::tokens::RADIUS_CHIP,
            }
        );
        // Пустой rects — 0 items (defensive: рисовать нечего).
        let mut p = Painter::new();
        focused.paint(&mut p, &[]);
        assert!(p.items().is_empty(), "пустой rects — ничего не рисуем");
    }
    #[test]
    fn text_field_component_hit_test_finds_container() {
        let tf = TextField::new(TextFieldProps {
            placeholder: String::new(),
            width: 120.0,
            palette: palette_b(),
        });
        let slot = UiRect::new(40.0, 50.0, 300.0, 60.0);
        let rects = tf.layout(default_backend(), slot);
        let r = rects[0];
        let inside = UiPoint::new(r.x + r.w / 2.0, r.y + r.h / 2.0);
        assert_eq!(
            tf.hit_test(&rects, inside),
            Some(ComponentHit { index: 0 }),
            "hit_test дефолтный: контейнер — index 0"
        );
        let outside = UiPoint::new(slot.x - 1.0, slot.y - 1.0);
        assert_eq!(tf.hit_test(&rects, outside), None);
    }

    // --- ScrollState: scroll_by/clamp/needs_scroll/max_offset ----------------
}
