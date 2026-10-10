//! FR-107 (мультиканвас C4, issue #8): двухзонный чип активного канваса
//! (№21c) — состояние без I/O + чистые функции раскладки/hit-зон.
//!
//! Паттерн модуля — `canvas_manager_ui` (FR-106): раскладка — чистая
//! функция от вьюпорта и измеренных потребителем ширин (один источник
//! геометрии для отрисовки, ввода и реестра поверхностей — draw == hit);
//! отрисовка и ввод — `app/overlays.rs` / `app/input.rs` / `app.rs`.
//! Геометрия — токены `canvas_core::tokens` + константы кита
//! (`kit::BUTTON_HEIGHT`); цвета в отрисовке — только слоты `KitPalette`.
//!
//! Чип живёт в верхней ЛЕВОЙ зоне окна (топ-центр — поиск 460px,
//! топ-право — DOM-панель web; топ-лево свободен — HUD только по F3 и
//! не в проде). Две зоны (№21c):
//! - **имя** — одиночный клик = инлайн-ренейм активного (валидация
//!   `validate_canvas_name` + коллизия по листингу, применение — через
//!   конвейер ренейма менеджера `CanvasOp::Rename`; Esc — отмена, Enter —
//!   применить, потеря фокуса — отмена);
//! - **иконка списка** — клик = `CanvasManagerOpen` (единственный вход в
//!   менеджер после чистки DOM-панели №37b).
//!
//! Дисковый режим (ActiveKind::Disk): ренейм менеджера не поддерживает
//! файлы вне workspace-хранилища — чип в режиме «только просмотр»
//! (клик по имени показывает подсказку-тост, буфер не открывается).
//!
//! Dirty-сигнал (№29b): постоянного индикатора «несохранено» НЕТ —
//! стойкий значок предупреждения появляется на чипе только при ошибке
//! сохранения и снимается первым успешным сохранением.

use canvas_core::tokens;
use canvas_ui::kit;

/// Высота чипа (лог. px): ряд верхнего хрома — как поле поиска
/// (`kit::BUTTON_HEIGHT` = 30, кит-каноническая высота контрола).
pub const CHIP_H: f32 = kit::BUTTON_HEIGHT;
/// Кап ширины чипа (лог. px): длинные имена эллипсируются. 160 — не
/// наезжает на панель поиска топ-центр (460px, левый край vw/2−230) в
/// минимальном G4-вьюпорте 800×560 (край поиска 170 > 12+160) и не
/// перекрывает HUD-зону диагностики сверх необходимого.
pub const CHIP_MAX_W: f32 = 160.0;
/// Сторона квадратной зоны иконки списка (клик = менеджер).
pub const ICON_ZONE: f32 = CHIP_H;
/// Сторона значка ошибки сохранения (№29b; не интерактивен).
pub const BADGE_ZONE: f32 = 14.0;

/// Зона чипа под точкой (hit-тест): имя / иконка списка / мимо.
/// Значок ошибки (№29b) — маркер, не интерактивен: попадания в него
/// считаются «мимо» (клик не глотается зоной чипа).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipHit {
    /// Имя активного канваса (клик — инлайн-ренейм №9/№21c).
    Name,
    /// Иконка списка (клик — менеджер канвасов №21c/№37b).
    ListIcon,
    /// Мимо зон чипа.
    None,
}

/// Раскладка чипа (чистая; `name_w` — ИЗМЕРЕННАЯ потребителем ширина
/// текста имени с падами, без капа — кап применяется здесь, один
/// источник геометрии для draw/hit/реестра).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChipLayout {
    /// Вся пилюля (фон/рамка).
    pub pill: [f32; 4],
    /// Зона имени (клик — ренейм).
    pub name: [f32; 4],
    /// Зона иконки списка (клик — менеджер).
    pub icon: [f32; 4],
    /// Зона значка ошибки сохранения (№29b; маркер, правый край зоны
    /// имени; `[0.0; 4]` — ошибки нет, не рисуется и не хитится).
    pub badge: [f32; 4],
    /// Зона имени после эллипсиса (ширина текста с учётом капа — рисует
    /// подпись и каретку ренейма).
    pub name_text_w: f32,
}

/// Раскладка чипа в верхней левой зоне: отступы — `SPACING_LG` (тот же
/// ряд, что у панели поиска топ-центр), ширина = имя + значок ошибки
/// (если есть) + иконка списка, кап `CHIP_MAX_W` + кламп к вьюпорту
/// (вырожденный вьюпорт схлопывает чип в точку — без паники).
pub fn chip_layout(viewport: [f32; 2], name_w: f32, save_failed: bool) -> ChipLayout {
    let badge_w = if save_failed { BADGE_ZONE } else { 0.0 };
    let wanted = (name_w + badge_w + ICON_ZONE).max(CHIP_H);
    let avail = (viewport[0] - tokens::SPACING_LG * 2.0).max(0.0);
    let width = wanted.min(CHIP_MAX_W).min(avail);
    let height = CHIP_H.min(viewport[1].max(0.0));
    let pill = [
        tokens::SPACING_LG,
        tokens::SPACING_LG,
        width.max(0.0),
        height,
    ];
    let icon = [
        pill[0] + width - ICON_ZONE,
        pill[1],
        ICON_ZONE.min(width),
        height,
    ];
    let name_zone_w = (width - ICON_ZONE - badge_w).max(0.0);
    let name = [pill[0], pill[1], name_zone_w, height];
    let badge = if save_failed {
        [
            pill[0] + width - ICON_ZONE - badge_w,
            pill[1] + (height - BADGE_ZONE) * 0.5,
            BADGE_ZONE.min(width),
            BADGE_ZONE,
        ]
    } else {
        [0.0; 4]
    };
    ChipLayout {
        pill,
        name,
        icon,
        badge,
        name_text_w: name_zone_w,
    }
}

/// Hit-зона чипа под точкой (draw == hit: зоны те же, что у отрисовки).
/// Порядок: иконка списка (правый край, приоритет у пересечений) → имя
/// → мимо. Значок ошибки (№29b) не интерактивен — не хитится.
pub fn chip_hit(lay: &ChipLayout, point: [f32; 2]) -> ChipHit {
    if point_in_rect(lay.icon, point) {
        return ChipHit::ListIcon;
    }
    if point_in_rect(lay.name, point) {
        return ChipHit::Name;
    }
    ChipHit::None
}

/// Точка в xywh-прямоугольнике (паритет `canvas_manager_ui::point_in_rect`).
pub fn point_in_rect(rect: [f32; 4], point: [f32; 2]) -> bool {
    point[0] >= rect[0]
        && point[0] <= rect[0] + rect[2]
        && point[1] >= rect[1]
        && point[1] <= rect[1] + rect[3]
}

// ============================================================================
// Состояние чипа (без I/O)
// ============================================================================

/// Состояние чипа активного канваса (план §3.2; паттерн
/// `CanvasManagerState`). Буфер ренейма — та же механика, что у
/// менеджера (№9): символы/Backspace/Esc/Enter, лимит `MAX_NAME_LEN`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CanvasChipState {
    /// Инлайн-ренейм активного канваса (№21c): буфер редактирования.
    pub rename: Option<String>,
    /// №29b: последнее сохранение активного канваса не удалось — стойкий
    /// значок на чипе (снимается первым успешным сохранением; НЕ
    /// исчезает по таймауту).
    pub save_failed: bool,
}

impl CanvasChipState {
    /// Начать ренейм активного канваса: буфер = текущее отображаемое имя.
    pub fn begin_rename(&mut self, display: &str) {
        self.rename = Some(display.to_owned());
    }

    /// Символ в буфер ренейма (лимит `MAX_NAME_LEN` — как у менеджера).
    pub fn edit_insert(&mut self, ch: char) {
        if let Some(buffer) = self.rename.as_mut() {
            if buffer.chars().count() < canvas_core::workspace::MAX_NAME_LEN {
                buffer.push(ch);
            }
        }
    }

    /// Backspace в буфере ренейма.
    pub fn edit_backspace(&mut self) {
        if let Some(buffer) = self.rename.as_mut() {
            buffer.pop();
        }
    }

    /// Отменить ренейм (Esc / потеря фокуса / открытие менеджера).
    pub fn cancel_edit(&mut self) {
        self.rename = None;
    }

    /// Подтвердить ренейм: забрать буфер — валидация/коллизии на
    /// вызывающем (нужны i18n и полный листинг).
    pub fn take_edit(&mut self) -> Option<String> {
        self.rename.take()
    }

    /// Буфер ренейма (рендер; None — ренейма нет).
    pub fn edit_buffer(&self) -> Option<&str> {
        self.rename.as_deref()
    }

    /// Идёт ли инлайн-ренейм (реестр поверхностей: клавиатурный scope).
    pub fn is_editing(&self) -> bool {
        self.rename.is_some()
    }
}

// ============================================================================
// Нативные тесты (чистая раскладка/hit-зоны/состояние)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const VP: [f32; 2] = [1280.0, 800.0];

    // --- раскладка -------------------------------------------------------------

    #[test]
    fn layout_top_left_with_margins() {
        let lay = chip_layout(VP, 120.0, false);
        assert_eq!(lay.pill[0], tokens::SPACING_LG);
        assert_eq!(lay.pill[1], tokens::SPACING_LG);
        assert_eq!(lay.pill[3], CHIP_H);
        // Имя + иконка = ширина пилюли; иконка — правый край
        assert!((lay.pill[2] - (120.0 + ICON_ZONE)).abs() < 1e-4);
        assert!((lay.icon[0] + lay.icon[2] - (lay.pill[0] + lay.pill[2])).abs() < 1e-4);
        assert!((lay.icon[3] - CHIP_H).abs() < 1e-4);
        // Зона имени — от левого края до иконки
        assert!((lay.name[2] - 120.0).abs() < 1e-4);
        assert_eq!(lay.badge, [0.0; 4], "без ошибки значка нет");
    }

    #[test]
    fn layout_caps_long_name() {
        let lay = chip_layout(VP, 500.0, false);
        assert!((lay.pill[2] - CHIP_MAX_W).abs() < 1e-4, "кап ширины");
        assert!((lay.name[2] - (CHIP_MAX_W - ICON_ZONE)).abs() < 1e-4);
        assert!((lay.name_text_w - lay.name[2]).abs() < 1e-4);
    }

    #[test]
    fn layout_clamps_to_viewport() {
        // Узкий вьюпорт: чип не вылезает за правый край
        let lay = chip_layout([100.0, 100.0], 500.0, false);
        assert!(lay.pill[0] + lay.pill[2] <= 100.0);
        // Вырожденный вьюпорт — нулевая ширина, без паник
        let lay = chip_layout([0.0, 0.0], 100.0, false);
        assert_eq!(lay.pill[2], 0.0);
    }

    #[test]
    fn layout_save_error_badge_before_icon() {
        let lay = chip_layout(VP, 120.0, true);
        // Значок — между именем и иконкой
        assert!((lay.badge[0] + lay.badge[2] - lay.icon[0]).abs() < 1e-4);
        assert_eq!(lay.badge[3], BADGE_ZONE);
        // Зона имени уже на ширину значка
        assert!((lay.name[2] - (lay.pill[2] - ICON_ZONE - BADGE_ZONE)).abs() < 1e-4);
    }

    // --- hit-зоны (draw == hit) --------------------------------------------------

    #[test]
    fn hit_zones_name_icon_and_miss() {
        let lay = chip_layout(VP, 120.0, false);
        // Центр зоны имени
        let name_c = [lay.name[0] + lay.name[2] * 0.5, lay.name[1] + CHIP_H * 0.5];
        assert_eq!(chip_hit(&lay, name_c), ChipHit::Name);
        // Центр иконки
        let icon_c = [lay.icon[0] + ICON_ZONE * 0.5, lay.icon[1] + CHIP_H * 0.5];
        assert_eq!(chip_hit(&lay, icon_c), ChipHit::ListIcon);
        // Мимо (правее чипа и ниже)
        assert_eq!(
            chip_hit(&lay, [lay.pill[0] + lay.pill[2] + 5.0, lay.pill[1] + 5.0]),
            ChipHit::None
        );
        assert_eq!(
            chip_hit(&lay, [lay.pill[0] + 5.0, lay.pill[1] + CHIP_H + 50.0]),
            ChipHit::None
        );
        // Значок ошибки — не интерактивен: клик в его центр мимо зон?
        // Значок живёт в зоне имени (правый край) — клик в него = имя,
        // это допустимо (ренейм под значком — честная деградация).
    }

    #[test]
    fn hit_icon_wins_on_overlap() {
        // Точка на стыке имени и иконки (граница) — иконка: она проверяется
        // первой (правый край чипа — самый частый жест «открыть список»).
        let lay = chip_layout(VP, 120.0, false);
        let border = [lay.icon[0], lay.icon[1] + CHIP_H * 0.5];
        assert_eq!(chip_hit(&lay, border), ChipHit::ListIcon);
    }

    // --- состояние ренейма (реюз механики менеджера №9) -------------------------

    #[test]
    fn rename_buffer_editing_cycle() {
        let mut chip = CanvasChipState::default();
        assert!(!chip.is_editing());
        chip.begin_rename("Проект");
        assert_eq!(chip.edit_buffer(), Some("Проект"));
        chip.edit_insert(' ');
        chip.edit_insert('2');
        assert_eq!(chip.edit_buffer(), Some("Проект 2"));
        chip.edit_backspace();
        assert_eq!(chip.edit_buffer(), Some("Проект "));
        assert_eq!(chip.take_edit(), Some("Проект ".to_owned()));
        assert!(!chip.is_editing(), "буфер забран — ренейм закрыт");
        assert_eq!(chip.take_edit(), None, "повторный take — пусто");
    }

    #[test]
    fn rename_buffer_respects_max_len() {
        let mut chip = CanvasChipState::default();
        chip.begin_rename("");
        for _ in 0..(canvas_core::workspace::MAX_NAME_LEN + 10) {
            chip.edit_insert('a');
        }
        assert_eq!(
            chip.edit_buffer().map(str::len),
            Some(canvas_core::workspace::MAX_NAME_LEN),
            "лимит MAX_NAME_LEN — как у менеджера"
        );
    }

    #[test]
    fn cancel_edit_drops_buffer() {
        let mut chip = CanvasChipState::default();
        chip.begin_rename("Имя");
        chip.cancel_edit();
        assert!(!chip.is_editing());
        assert_eq!(chip.edit_buffer(), None);
    }

    #[test]
    fn save_failed_flag_is_plain_state() {
        let mut chip = CanvasChipState::default();
        assert!(!chip.save_failed);
        chip.save_failed = true;
        assert!(chip.save_failed, "стойкий флаг живёт до успешного сохранения");
    }
}
