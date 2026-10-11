//! FR-105 (мультиканвас C2, issue #6): UI-модели хранилища рабочего
//! пространства — баннер потери доступа к granted-папке (№44b) и диалог
//! миграции OPFS → папка (№42a/№52a).
//!
//! Паттерн модуля — `scheme_gallery_ui` (FR-049): состояние без I/O,
//! раскладка — чистые функции от вьюпорта (один источник геометрии для
//! hit-rect'ов реестра поверхностей и отрисовки), отрисовка и ввод —
//! `app.rs`/`app/overlays.rs`/`app/input.rs`. Геометрия — токены
//! `canvas_core::tokens` + константы кита (`kit::LIST_ROW_H`,
//! `BUTTON_HEIGHT`); цвета в отрисовке — только слоты `KitPalette`.

use canvas_core::tokens;
use canvas_core::workspace::CanvasEntry;
use canvas_ui::geometry::{UiRect, UiVec2};
use canvas_ui::kit;

/// Ширина панели диалога миграции (лог. px): чекбокс + имя + зазоры —
/// 480 влезает в минимальный G4-вьюпорт (800) с запасом.
pub const MIGRATE_PANEL_W: f32 = 480.0;
/// Высота шапки диалога (титул + «✕»).
pub const MIGRATE_HEADER_H: f32 = 40.0;
/// Полный шаг строки списка (строка + зазор `LIST_ROW_GAP`).
pub const MIGRATE_ROW_STEP: f32 = kit::LIST_ROW_H + kit::LIST_ROW_GAP;
/// Строк списка в окне видимости (без скролла до 10 канвасов).
pub const MIGRATE_VISIBLE_ROWS: usize = 10;
/// Высота блока подсказки (2 строки caption + отступы).
pub const MIGRATE_HINT_H: f32 = 40.0;
/// Высота футера с кнопками («Переехать…»/«Отмена»).
pub const MIGRATE_FOOTER_H: f32 = kit::BUTTON_HEIGHT + tokens::SPACING_LG;
/// Сторона чекбокса (индикатор выбора строки) — радиус-карта кита.
pub const MIGRATE_CHECKBOX: f32 = kit::RADIO_INDICATOR_SIZE;

/// Wave M M3-a (issue #52, T-M R1): тач-минимум интерактивных контролов
/// (кнопки футера/«✕») в Compact-тире адаптивной ветки.
pub const MIGRATE_TOUCH_TARGET: f32 = 44.0;
/// Шаг строки чекбокс-листа в Compact-тире (строка 44 + зазор кита) —
/// тач-цели строк ≥44×44 (T-M R1).
pub const MIGRATE_ROW_STEP_COMPACT: f32 = MIGRATE_TOUCH_TARGET + kit::LIST_ROW_GAP;
/// Высота шапки в Compact-тире («✕» 44 центрируется без вылета за панель).
pub const MIGRATE_HEADER_H_COMPACT: f32 = 48.0;
/// Высота баннера потери доступа: кнопка + вертикальные поля.
pub const BANNER_H: f32 = kit::BUTTON_HEIGHT + tokens::SPACING_SM;
/// TTL тоста с действием (кнопка «Перезагрузить» №45b): дольше простого
/// (`kit::TOAST_TTL_MS` = 3 с) — пользователь должен успеть прочитать и
/// нажать.
pub const TOAST_ACTION_TTL_MS: u64 = 8000;
/// Высота полосы тоста, когда в нём есть кнопка-действие (№45b): строка
/// текста (28 — потребитель-клип тоста, см. handler.rs) не вмещает кнопку
/// кита (`kit::BUTTON_HEIGHT` = 34) — scissor-клип полосы (FR-CLIP) резал
/// бы её низ. С действием полоса растёт до высоты кнопки (позиция строки
/// не меняется: y тот же, текст и кнопка рисуются от верха полосы).
pub const TOAST_ACTION_STRIP_H: f32 = kit::BUTTON_HEIGHT;
/// Кегль строки тоста (потребительский клип/текст — handler.rs; единая
/// точка для отрисовки кнопки-действия и её hit-зоны в реестре).
pub const TOAST_ACTION_FONT: f32 = 14.0;

// ============================================================================
// Баннер потери доступа (№44b)
// ============================================================================

/// Раскладка баннера: панель сверху-по-центру, текст слева, кнопки
/// «Переподключить» и «Переключиться в браузерное» справа.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StorageBannerLayout {
    /// Панель баннера.
    pub panel: [f32; 4],
    /// Область подписи (текст = `canvas.storage.lost_banner`).
    pub label: [f32; 4],
    /// Кнопка «Переподключить» (`canvas.storage.reconnect`).
    pub reconnect: [f32; 4],
    /// Кнопка «Переключиться в браузерное» (`canvas.storage.switch_browser`).
    pub switch: [f32; 4],
}

/// Раскладка баннера потери доступа: ширины подписи/кнопок измерены
/// потребителем (`TextMeasurer`), здесь — только геометрия.
pub fn storage_banner_layout(
    viewport: [f32; 2],
    label_w: f32,
    reconnect_w: f32,
    switch_w: f32,
) -> StorageBannerLayout {
    // Ширина панели: пад + подпись + зазор + кнопка₁ + зазор + кнопка₂ + пад
    let content = label_w + tokens::SPACING_LG + reconnect_w + tokens::SPACING_SM + switch_w;
    let mut width = content + tokens::SPACING_LG * 2.0;
    // Кламп к вьюпорту (узкий экран): панель не шире окна минус поля
    let max_w = (viewport[0] - tokens::SPACING_LG * 2.0).max(0.0);
    if width > max_w {
        width = max_w;
    }
    let x = ((viewport[0] - width) * 0.5).max(0.0);
    let y = tokens::SPACING_LG;
    let panel = [x, y, width, BANNER_H];
    // Кнопки — от правого края панели, вертикально по центру
    let by = y + (BANNER_H - kit::BUTTON_HEIGHT) * 0.5;
    let switch = [
        x + width - tokens::SPACING_LG - switch_w,
        by,
        switch_w,
        kit::BUTTON_HEIGHT,
    ];
    let reconnect = [
        switch[0] - tokens::SPACING_SM - reconnect_w,
        by,
        reconnect_w,
        kit::BUTTON_HEIGHT,
    ];
    // Подпись — от левого пада до зазора перед первой кнопкой
    let label_w_avail = (reconnect[0] - tokens::SPACING_SM - (x + tokens::SPACING_LG)).max(0.0);
    let label = [
        x + tokens::SPACING_LG,
        y + (BANNER_H - kit::BANNER_LABEL_LINE_H) * 0.5,
        label_w_avail,
        kit::BANNER_LABEL_LINE_H,
    ];
    StorageBannerLayout {
        panel,
        label,
        reconnect,
        switch,
    }
}

// ============================================================================
// Состояние диалога миграции (№42a)
// ============================================================================

/// Состояние диалога миграции OPFS → папка: чекбокс-лист канвасов OPFS
/// (источник — AppEvent::MigrateOpfsList от web-слоя), активный канвас
/// обязателен к переезду (чекбокс заблокирован — сцена не может остаться
/// в OPFS при переключении режима, №52a).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MigrateState {
    /// Диалог открыт.
    pub open: bool,
    /// Канвасы OPFS (листинг приехал по событию; до этого — пусто).
    pub entries: Vec<CanvasEntry>,
    /// Отметки чекбоксов (параллелно `entries`).
    pub checked: Vec<bool>,
    /// Ординал выбранной строки (клавиатура/клик).
    pub selected: usize,
    /// Верх окна видимости списка.
    pub scroll_top: usize,
    /// Имя файла активного канваса (обязателен к переезду; чекбокс заблокирован).
    pub active: String,
}

impl MigrateState {
    /// Открыть диалог (листинг запросит вызывающий через мост).
    pub fn open_with(&mut self, active: &str) {
        self.open = true;
        self.entries.clear();
        self.checked.clear();
        self.selected = 0;
        self.scroll_top = 0;
        self.active = active.to_owned();
    }

    /// Закрыть диалог.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Листинг OPFS приехал: все галочки выставлены (№42a — по умолчанию
    /// переезжает всё; пользователь снимает лишнее).
    pub fn set_entries(&mut self, entries: Vec<CanvasEntry>) {
        let count = entries.len();
        self.checked = vec![true; count];
        self.entries = entries;
        self.selected = 0;
        self.scroll_top = 0;
    }

    /// Строка обязательна к переезду (активный канвас).
    pub fn is_mandatory(&self, index: usize) -> bool {
        self.entries
            .get(index)
            .is_some_and(|entry| entry.name.eq_ignore_ascii_case(&self.active))
    }

    /// Переключить чекбокс строки (обязательную — нельзя снять).
    pub fn toggle(&mut self, index: usize) {
        if index < self.checked.len() && !self.is_mandatory(index) {
            self.checked[index] = !self.checked[index];
        }
    }

    /// Выбранные имена файлов (для плана миграции).
    pub fn selected_names(&self) -> Vec<String> {
        self.entries
            .iter()
            .zip(&self.checked)
            .filter(|(_, checked)| **checked)
            .map(|(entry, _)| entry.name.clone())
            .collect()
    }

    /// Есть ли что переносить (≥1 галочка).
    pub fn any_checked(&self) -> bool {
        self.checked.iter().any(|checked| *checked)
    }

    /// Кламп выбора после смены листинга/удаления строк.
    pub fn clamp_selection(&mut self) {
        if self.selected >= self.entries.len() {
            self.selected = self.entries.len().saturating_sub(1);
        }
        self.scroll_top = self.scroll_top.min(self.max_scroll_top());
    }

    fn max_scroll_top(&self) -> usize {
        self.entries.len().saturating_sub(MIGRATE_VISIBLE_ROWS)
    }
}

// ============================================================================
// Раскладка диалога миграции
// ============================================================================

/// Строка чекбокс-листа.
#[derive(Debug, Clone, PartialEq)]
pub struct MigrateRowLayout {
    /// Прямоугольник строки (hit-зона).
    pub row: [f32; 4],
    /// Чекбокс (квадрат-индикатор).
    pub checkbox: [f32; 4],
    /// Область имени (эллипсис — потребитель через `TextMeasurer`).
    pub label: [f32; 4],
    /// Отмечена ли галочкой (рисует потребитель).
    pub checked: bool,
    /// Обязательная строка (активный канвас — чекбокс заблокирован).
    pub mandatory: bool,
}

/// Раскладка диалога миграции.
#[derive(Debug, Clone, PartialEq)]
pub struct MigrateLayout {
    /// Панель диалога.
    pub panel: [f32; 4],
    /// Титул (`canvas.migrate.title`).
    pub title: [f32; 4],
    /// Кнопка «✕».
    pub close: [f32; 4],
    /// Подсказка (`canvas.migrate.hint`, 2 строки).
    pub hint: [f32; 4],
    /// Видимые строки (окно `scroll_top..+MIGRATE_VISIBLE_ROWS`).
    pub rows: Vec<MigrateRowLayout>,
    /// Кнопка «Переехать на диск…» (canvas.storage.move_to_disk).
    pub go: [f32; 4],
    /// Кнопка «Отмена» (dialog.cancel).
    pub cancel: [f32; 4],
    /// Ординал первой видимой строки.
    pub first_row: usize,
    /// Окно видимости (штук строк) — источник скролл-математики
    /// (`migrate_wheel_scroll`/`migrate_scroll_to_reveal`); в прежней ветке
    /// — `MIGRATE_VISIBLE_ROWS`, в адаптивной — эластичное (M-O R7).
    pub visible_rows: usize,
}

/// Раскладка диалога миграции (чистая; `go_w`/`cancel_w` — измеренные
/// потребителем ширины кнопок через `kit::button_size` — здесь только
/// геометрия от вьюпорта и состояния).
pub fn migrate_layout(
    viewport: [f32; 2],
    state: &MigrateState,
    go_w: f32,
    cancel_w: f32,
) -> MigrateLayout {
    let visible = MIGRATE_VISIBLE_ROWS.min(state.entries.len().max(1));
    let rows_h = visible as f32 * MIGRATE_ROW_STEP;
    let height = MIGRATE_HEADER_H + MIGRATE_HINT_H + rows_h + MIGRATE_FOOTER_H + tokens::SPACING_LG;
    let width = MIGRATE_PANEL_W.min(viewport[0]);
    let height = height.min(viewport[1]);
    let x = ((viewport[0] - width) * 0.5).max(0.0);
    let y = ((viewport[1] - height) * 0.5).max(0.0);
    let panel = [x, y, width, height];
    let title = [
        x + tokens::SPACING_LG,
        y + (MIGRATE_HEADER_H - kit::BANNER_LABEL_LINE_H) * 0.5,
        width - MIGRATE_HEADER_H,
        kit::BANNER_LABEL_LINE_H,
    ];
    let close = [
        x + width - MIGRATE_HEADER_H,
        y + (MIGRATE_HEADER_H - kit::ICON_BUTTON_SIZE) * 0.5,
        kit::ICON_BUTTON_SIZE,
        kit::ICON_BUTTON_SIZE,
    ];
    let hint = [
        x + tokens::SPACING_LG,
        y + MIGRATE_HEADER_H,
        width - tokens::SPACING_LG * 2.0,
        MIGRATE_HINT_H,
    ];
    let rows_top = y + MIGRATE_HEADER_H + MIGRATE_HINT_H;
    let first_row = state.scroll_top;
    let rows = state
        .entries
        .iter()
        .enumerate()
        .skip(first_row)
        .take(MIGRATE_VISIBLE_ROWS)
        .map(|(index, _entry)| {
            let ry = rows_top + (index - first_row) as f32 * MIGRATE_ROW_STEP;
            let row = [
                x + tokens::SPACING_LG,
                ry,
                width - tokens::SPACING_LG * 2.0,
                kit::LIST_ROW_H,
            ];
            let checkbox = [
                row[0] + tokens::SPACING_SM,
                ry + (kit::LIST_ROW_H - MIGRATE_CHECKBOX) * 0.5,
                MIGRATE_CHECKBOX,
                MIGRATE_CHECKBOX,
            ];
            let label = [
                checkbox[0] + MIGRATE_CHECKBOX + tokens::SPACING_MD,
                ry,
                (row[0] + row[2] - (checkbox[0] + MIGRATE_CHECKBOX + tokens::SPACING_MD)).max(0.0),
                kit::LIST_ROW_H,
            ];
            MigrateRowLayout {
                row,
                checkbox,
                label,
                checked: state.checked.get(index).copied().unwrap_or(false),
                mandatory: state.is_mandatory(index),
            }
        })
        .collect();
    // Футер: «Переехать…» справа (primary), «Отмена» левее (secondary);
    // ширины измерены потребителем (kit::button_size), кламп — к панели
    let avail = (width - tokens::SPACING_LG * 2.0).max(0.0);
    let go_w = go_w.min(avail);
    let cancel_w = cancel_w.min((avail - go_w - tokens::SPACING_SM).max(0.0));
    let fy = y + height - MIGRATE_FOOTER_H + tokens::SPACING_LG * 0.5;
    let go = [
        x + width - tokens::SPACING_LG - go_w,
        fy,
        go_w,
        kit::BUTTON_HEIGHT,
    ];
    let cancel = [
        go[0] - tokens::SPACING_SM - cancel_w,
        fy,
        cancel_w,
        kit::BUTTON_HEIGHT,
    ];
    MigrateLayout {
        panel,
        title,
        close,
        hint,
        rows,
        go,
        cancel,
        first_row,
        visible_rows: visible,
    }
}

// === Wave M M3-a (issue #52): адаптивная ветка (флаг modals_adaptive) ========

/// Бюджет высоты окна адаптивной модали: Compact — весь слот (full-bleed
/// лист), Medium/Expanded — 80% слота минус margin (формула контракта
/// Wave M, примитив `kit::modal_rect_flex`).
fn migrate_budget_h(viewport: [f32; 2]) -> f32 {
    let margin = tokens::SPACING_LG;
    let eff_h = (viewport[1] - margin * 2.0).max(0.0);
    if kit::layout_tier(viewport[0]) == kit::LayoutTier::Compact {
        eff_h
    } else {
        eff_h * kit::MODAL_RATIO_DEFAULT
    }
}

/// Окно видимости чекбокс-листа в адаптивной ветке (M-O R7 «скролл
/// внутри»): эластичное — из бюджета высоты; потолок прежний
/// (`MIGRATE_VISIBLE_ROWS`), пол — одна строка. Единый источник для
/// раскладки и клавиатурного скролла («ввод = тому, что видно»).
pub fn migrate_visible_rows(viewport: [f32; 2], entries_len: usize) -> usize {
    let compact = kit::layout_tier(viewport[0]) == kit::LayoutTier::Compact;
    let step = if compact {
        MIGRATE_ROW_STEP_COMPACT
    } else {
        MIGRATE_ROW_STEP
    };
    let chrome = if compact {
        MIGRATE_HEADER_H_COMPACT + MIGRATE_HINT_H + MIGRATE_TOUCH_TARGET + tokens::SPACING_LG * 2.0
    } else {
        MIGRATE_HEADER_H + MIGRATE_HINT_H + MIGRATE_FOOTER_H + tokens::SPACING_LG
    };
    (((migrate_budget_h(viewport) - chrome) / step)
        .floor()
        .max(1.0) as usize)
        .min(MIGRATE_VISIBLE_ROWS)
        .clamp(1, entries_len.max(1))
}

/// Раскладка диалога миграции в адаптивной ветке (Wave M M3-a, issue #52):
/// окно — `kit::modal_rect_flex` (80% слота; min = max = контент 480×N —
/// M-O R7 «size follows content», desktop-потолок прежний, окно не
/// разрастается); Compact — full-bleed лист: одноколоночный скелет,
/// строки/кнопки/«✕» на тач-минимуме 44 (T-M R1), футер прижат к нижней
/// thumb-зоне (T-M R2), список скроллится внутри (M-O R7). Скелет/хром
/// прежней раскладки — дословно; геометрия — токены kit.
pub fn migrate_layout_adaptive(
    viewport: [f32; 2],
    state: &MigrateState,
    go_w: f32,
    cancel_w: f32,
) -> MigrateLayout {
    let compact = kit::layout_tier(viewport[0]) == kit::LayoutTier::Compact;
    let margin = tokens::SPACING_LG;
    let step = if compact {
        MIGRATE_ROW_STEP_COMPACT
    } else {
        MIGRATE_ROW_STEP
    };
    let header_h = if compact {
        MIGRATE_HEADER_H_COMPACT
    } else {
        MIGRATE_HEADER_H
    };
    let btn_h = if compact {
        MIGRATE_TOUCH_TARGET
    } else {
        kit::BUTTON_HEIGHT
    };
    let footer_h = btn_h + tokens::SPACING_LG;
    let visible = migrate_visible_rows(viewport, state.entries.len());
    let content_h =
        header_h + MIGRATE_HINT_H + visible as f32 * step + footer_h + tokens::SPACING_LG;
    let slot = UiRect::new(0.0, 0.0, viewport[0], viewport[1]);
    // Compact — примитив вернёт full-bleed eff; Medium/Expanded — панель
    // 480×content_h по центру eff (min = max = контент).
    let panel = kit::modal_rect_flex(
        slot,
        margin,
        kit::MODAL_RATIO_DEFAULT,
        UiVec2::new(MIGRATE_PANEL_W, content_h),
        UiVec2::new(MIGRATE_PANEL_W, content_h),
    );
    let (x, y, width, height) = (panel.x, panel.y, panel.w, panel.h);
    let title = [
        x + tokens::SPACING_LG,
        y + (header_h - kit::BANNER_LABEL_LINE_H) * 0.5,
        width - header_h,
        kit::BANNER_LABEL_LINE_H,
    ];
    let close = [
        x + width - header_h,
        y + (header_h - btn_h) * 0.5,
        btn_h,
        btn_h,
    ];
    let hint = [
        x + tokens::SPACING_LG,
        y + header_h,
        width - tokens::SPACING_LG * 2.0,
        MIGRATE_HINT_H,
    ];
    let rows_top = y + header_h + MIGRATE_HINT_H;
    let first_row = state.scroll_top;
    let row_h = step - kit::LIST_ROW_GAP;
    let rows = state
        .entries
        .iter()
        .enumerate()
        .skip(first_row)
        .take(visible)
        .map(|(index, _entry)| {
            let ry = rows_top + (index - first_row) as f32 * step;
            let row = [
                x + tokens::SPACING_LG,
                ry,
                width - tokens::SPACING_LG * 2.0,
                row_h,
            ];
            let checkbox = [
                row[0] + tokens::SPACING_SM,
                ry + (row_h - MIGRATE_CHECKBOX) * 0.5,
                MIGRATE_CHECKBOX,
                MIGRATE_CHECKBOX,
            ];
            let label = [
                checkbox[0] + MIGRATE_CHECKBOX + tokens::SPACING_MD,
                ry,
                (row[0] + row[2] - (checkbox[0] + MIGRATE_CHECKBOX + tokens::SPACING_MD)).max(0.0),
                row_h,
            ];
            MigrateRowLayout {
                row,
                checkbox,
                label,
                checked: state.checked.get(index).copied().unwrap_or(false),
                mandatory: state.is_mandatory(index),
            }
        })
        .collect();
    // Футер: «Переехать…» справа (primary), «Отмена» левее; в Compact —
    // нижняя thumb-зона листа (T-M R2). Ширины измерены потребителем.
    let avail = (width - tokens::SPACING_LG * 2.0).max(0.0);
    let go_w = go_w.min(avail);
    let cancel_w = cancel_w.min((avail - go_w - tokens::SPACING_SM).max(0.0));
    let fy = y + height - footer_h + tokens::SPACING_LG * 0.5;
    let go = [x + width - tokens::SPACING_LG - go_w, fy, go_w, btn_h];
    let cancel = [go[0] - tokens::SPACING_SM - cancel_w, fy, cancel_w, btn_h];
    MigrateLayout {
        panel: [x, y, width, height],
        title,
        close,
        hint,
        rows,
        go,
        cancel,
        first_row,
        visible_rows: visible,
    }
}

/// Строка под точкой (hit-тест списка; None — мимо строк).
pub fn migrate_row_at(lay: &MigrateLayout, point: [f32; 2]) -> Option<usize> {
    lay.rows
        .iter()
        .position(|row| {
            point[0] >= row.row[0]
                && point[0] <= row.row[0] + row.row[2]
                && point[1] >= row.row[1]
                && point[1] <= row.row[1] + row.row[3]
        })
        .map(|index| lay.first_row + index)
}

/// Скролл списка колесом (шаг — строки): возврат нового scroll_top.
/// `visible` — окно видимости раскладки (`MigrateLayout::visible_rows`):
/// в адаптивной ветке окно эластичное (M-O R7) — хвост списка обязан
/// оставаться достижимым.
pub fn migrate_wheel_scroll(current: usize, delta_rows: i32, len: usize, visible: usize) -> usize {
    let max = len.saturating_sub(visible.max(1));
    (current as i32 - delta_rows).clamp(0, max as i32) as usize
}

/// Скролл вслед за клавиатурным выбором (↑/↓): выбранная строка обязана
/// попасть в окно видимости — сверху, если ушла выше, и прижаться к низу
/// (`selected + 1 - MIGRATE_VISIBLE_ROWS`), если ушла ниже. Уже видимая —
/// без изменений. Тот же инвариант, что у `RowScroll::ensure_visible`
/// (LAY-W9/K3).
pub fn migrate_scroll_to_reveal(scroll_top: usize, selected: usize, visible: usize) -> usize {
    let visible = visible.max(1);
    if selected < scroll_top {
        selected
    } else if selected + 1 > scroll_top + visible {
        selected + 1 - visible
    } else {
        scroll_top
    }
}

// ============================================================================
// Тост с действием (№45b)
// ============================================================================

/// FR-105 (№45b): rect кнопки-действия тоста («Перезагрузить») — справа от
/// центрированного текста тоста, вертикально по центру строки. `toast` —
/// rect строки тоста (`kit::toast_area` + высота 28 у потребителя),
/// `text_w` — измеренная ширина текста тоста, `action_w` — ширина кнопки
/// (`kit::button_size`). Узкий экран (текст шире половины тоста) — кнопка
/// прижимается к правому краю строки (кламп), не перекрывая центр.
pub fn toast_action_rect(toast: [f32; 4], text_w: f32, action_w: f32) -> [f32; 4] {
    let center = toast[0] + toast[2] * 0.5;
    let right = toast[0] + toast[2];
    let x = (center + text_w * 0.5 + tokens::SPACING_MD)
        .min((right - action_w).max(0.0))
        .max(0.0);
    let y = toast[1] + (toast[3] - kit::BUTTON_HEIGHT).max(0.0) * 0.5;
    [x, y, action_w, kit::BUTTON_HEIGHT]
}

// ============================================================================
// Нативные тесты (чистая раскладка/состояние)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str) -> CanvasEntry {
        CanvasEntry {
            name: name.to_owned(),
            ts: 1,
            kind: canvas_core::workspace::EntryKind::Opfs,
            repo: None,
        }
    }

    // --- баннер (№44b) ------------------------------------------------------

    #[test]
    fn banner_layout_places_label_and_two_buttons() {
        let lay = storage_banner_layout([1280.0, 800.0], 180.0, 140.0, 190.0);
        // панель по центру сверху
        let center = lay.panel[0] + lay.panel[2] * 0.5;
        assert!((center - 640.0).abs() < 0.5, "центр панели {center}");
        assert_eq!(lay.panel[1], tokens::SPACING_LG, "отступ от верха");
        assert_eq!(lay.panel[3], BANNER_H);
        // кнопки правее подписи, без пересечений
        assert!(lay.label[0] + lay.label[2] <= lay.reconnect[0]);
        assert!(lay.reconnect[0] + lay.reconnect[2] <= lay.switch[0]);
        assert!(
            (lay.switch[0] + lay.switch[2] - (lay.panel[0] + lay.panel[2] - tokens::SPACING_LG))
                .abs()
                < 0.5,
            "switch у правого пада панели"
        );
        // высоты кнопок — канонические
        assert_eq!(lay.reconnect[3], kit::BUTTON_HEIGHT);
        assert_eq!(lay.switch[3], kit::BUTTON_HEIGHT);
    }

    #[test]
    fn banner_layout_clamps_to_narrow_viewport() {
        let lay = storage_banner_layout([320.0, 240.0], 400.0, 140.0, 190.0);
        assert!(
            lay.panel[2] <= 320.0 - tokens::SPACING_LG * 2.0,
            "панель не шире вьюпорта минус поля"
        );
        assert!(lay.panel[0] >= 0.0 && lay.panel[0] + lay.panel[2] <= 320.0);
        // кнопки всё равно внутри панели
        assert!(lay.switch[0] + lay.switch[2] <= lay.panel[0] + lay.panel[2]);
        assert!(lay.label[2] >= 0.0, "область подписи не уходит в минус");
    }

    // --- состояние диалога (№42a) --------------------------------------------

    #[test]
    fn migrate_state_defaults_all_checked_and_mandatory_active() {
        let mut state = MigrateState::default();
        state.open_with("рабочий.canvas");
        assert!(state.open);
        state.set_entries(vec![
            entry("рабочий.canvas"),
            entry("draft.canvas"),
            entry("old.canvas"),
        ]);
        assert_eq!(state.checked, vec![true, true, true], "по умолчанию всё");
        assert!(state.is_mandatory(0));
        assert!(!state.is_mandatory(1));
        // активный нельзя снять
        state.toggle(0);
        assert!(state.checked[0], "обязательная галочка не снимается");
        state.toggle(1);
        assert!(!state.checked[1], "обычная галочка снимается");
        assert_eq!(
            state.selected_names(),
            vec!["рабочий.canvas".to_owned(), "old.canvas".to_owned()]
        );
        assert!(state.any_checked());
    }

    #[test]
    fn migrate_state_empty_entries() {
        let mut state = MigrateState::default();
        state.open_with("x.canvas");
        state.set_entries(vec![]);
        assert!(!state.any_checked());
        assert!(state.selected_names().is_empty());
        state.clamp_selection();
        assert_eq!(state.selected, 0);
    }

    // --- раскладка диалога ----------------------------------------------------

    #[test]
    fn migrate_layout_rows_and_footer() {
        let mut state = MigrateState::default();
        state.open_with("a.canvas");
        let names: Vec<_> = (0..15).map(|i| entry(&format!("c{i:02}.canvas"))).collect();
        state.set_entries(names);
        let lay = migrate_layout([1280.0, 800.0], &state, 170.0, 110.0);
        assert_eq!(lay.rows.len(), MIGRATE_VISIBLE_ROWS, "окно видимости");
        assert_eq!(lay.first_row, 0);
        // чекбоксы внутри строк, подписи правее чекбоксов
        let row = &lay.rows[0];
        assert!(
            row.checkbox[0] >= row.row[0]
                && row.checkbox[0] + row.checkbox[2] <= row.row[0] + row.row[2]
        );
        assert!(row.label[0] > row.checkbox[0] + row.checkbox[2]);
        assert!(row.checked, "по умолчанию всё отмечено");
        // футер: обе кнопки в панели, go правее cancel
        assert!(lay.go[0] > lay.cancel[0]);
        assert!(lay.go[0] + lay.go[2] <= lay.panel[0] + lay.panel[2]);
        assert!(lay.cancel[0] >= lay.panel[0]);
    }

    #[test]
    fn migrate_layout_scroll_window_and_row_at() {
        let mut state = MigrateState::default();
        state.open_with("z.canvas");
        let names: Vec<_> = (0..15).map(|i| entry(&format!("c{i:02}.canvas"))).collect();
        state.set_entries(names);
        state.scroll_top = 5;
        let lay = migrate_layout([1280.0, 800.0], &state, 170.0, 110.0);
        assert_eq!(lay.first_row, 5);
        assert_eq!(lay.rows.len(), 10, "хвост списка до конца");
        // hit-тест первой видимой строки
        let point = [lay.rows[0].row[0] + 4.0, lay.rows[0].row[1] + 4.0];
        assert_eq!(migrate_row_at(&lay, point), Some(5));
        // мимо строк — None
        assert_eq!(migrate_row_at(&lay, [640.0, 6.0]), None);
    }

    #[test]
    fn migrate_wheel_scroll_clamps() {
        assert_eq!(
            migrate_wheel_scroll(0, -3, 15, MIGRATE_VISIBLE_ROWS),
            3,
            "вниз на 3 строки"
        );
        assert_eq!(
            migrate_wheel_scroll(4, 2, 15, MIGRATE_VISIBLE_ROWS),
            2,
            "вверх на 2 строки"
        );
        assert_eq!(
            migrate_wheel_scroll(3, 100, 15, MIGRATE_VISIBLE_ROWS),
            0,
            "кламп вверх"
        );
        assert_eq!(
            migrate_wheel_scroll(3, -100, 15, MIGRATE_VISIBLE_ROWS),
            5,
            "кламп вниз (15-10)"
        );
        assert_eq!(
            migrate_wheel_scroll(0, -5, 3, MIGRATE_VISIBLE_ROWS),
            0,
            "короткий список — без скролла"
        );
    }

    #[test]
    fn migrate_scroll_to_reveal_follows_keyboard_selection() {
        // выбор ушёл выше окна — окно к нему прижалось
        assert_eq!(migrate_scroll_to_reveal(5, 3, MIGRATE_VISIBLE_ROWS), 3);
        // выбор ушёл ниже окна — хвост окна к нему
        assert_eq!(
            migrate_scroll_to_reveal(0, 12, MIGRATE_VISIBLE_ROWS),
            3,
            "12+1-10"
        );
        assert_eq!(
            migrate_scroll_to_reveal(5, 14, MIGRATE_VISIBLE_ROWS),
            5,
            "14 в окне 5..15"
        );
        // уже видимая — без изменений (обе границы окна включительно)
        assert_eq!(migrate_scroll_to_reveal(5, 5, MIGRATE_VISIBLE_ROWS), 5);
        assert_eq!(migrate_scroll_to_reveal(5, 14, MIGRATE_VISIBLE_ROWS), 5);
        assert_eq!(
            migrate_scroll_to_reveal(5, 9, MIGRATE_VISIBLE_ROWS),
            5,
            "середина окна"
        );
        // короткий список (окно длиннее) — 0 без паники usize
        assert_eq!(migrate_scroll_to_reveal(0, 2, MIGRATE_VISIBLE_ROWS), 0);
        assert_eq!(migrate_scroll_to_reveal(3, 0, MIGRATE_VISIBLE_ROWS), 0);
    }

    // === Wave M M3-a (issue #52): адаптивная ветка ==========================

    fn state_with(n: usize) -> MigrateState {
        let mut state = MigrateState::default();
        state.open_with("a.canvas");
        state.set_entries((0..n).map(|i| entry(&format!("c{i:02}.canvas"))).collect());
        state
    }

    /// Expanded (1280×800): бюджет 80% выше контента 10-строчного окна —
    /// раскладка совпадает с прежней бит-в-бит (окно не разрастается,
    /// M-O R7).
    #[test]
    fn migrate_adaptive_expanded_matches_legacy_window() {
        let state = state_with(15);
        let legacy = migrate_layout([1280.0, 800.0], &state, 170.0, 110.0);
        let lay = migrate_layout_adaptive([1280.0, 800.0], &state, 170.0, 110.0);
        assert_eq!(lay.panel, legacy.panel, "окно — прежнее (480×10 строк)");
        assert_eq!(lay.visible_rows, MIGRATE_VISIBLE_ROWS);
        assert_eq!(lay.rows.len(), MIGRATE_VISIBLE_ROWS);
        assert_eq!(lay.rows[0].row, legacy.rows[0].row, "скелет строк тот же");
        assert_eq!(lay.go, legacy.go, "кнопки футера — прежние");
    }

    /// Medium (800×560): окно сжимается к бюджету 80% — окно строк
    /// эластично (9 вместо 10), панель по центру eff, без выхода за слот.
    #[test]
    fn migrate_adaptive_medium_shrinks_to_budget() {
        let state = state_with(15);
        let lay = migrate_layout_adaptive([800.0, 560.0], &state, 170.0, 110.0);
        assert_eq!(lay.visible_rows, 9, "floor((0.8×536 − 134)/32) = 9");
        assert_eq!(lay.rows.len(), 9);
        assert_eq!(lay.panel[2], MIGRATE_PANEL_W, "ширина канона не растёт");
        let expected_h = MIGRATE_HEADER_H
            + MIGRATE_HINT_H
            + 9.0 * MIGRATE_ROW_STEP
            + MIGRATE_FOOTER_H
            + tokens::SPACING_LG;
        assert!((lay.panel[3] - expected_h).abs() < 0.01);
        assert!(
            lay.panel[3] <= 0.8 * (560.0 - tokens::SPACING_LG * 2.0) + 0.01,
            "панель в бюджете 80% слота"
        );
        assert!((lay.panel[0] - (12.0 + (776.0 - MIGRATE_PANEL_W) * 0.5)).abs() < 0.01);
        // скролл-математика из окна раскладки: хвост достижим
        let top = migrate_wheel_scroll(0, -100, 15, lay.visible_rows);
        assert_eq!(top, 15 - 9, "максимум = список − окно");
        let list_bottom = lay.rows[8].row[1] + lay.rows[8].row[3];
        assert!(
            list_bottom <= lay.panel[1] + lay.panel[3] + 0.01,
            "строки внутри панели"
        );
    }

    /// Compact (375×667): full-bleed лист — панель = весь слот с
    /// safe-margin, строки/кнопки/«✕» на тач-минимуме 44 (T-M R1), футер
    /// прижат к нижней thumb-зоне (T-M R2), всё внутри вьюпорта.
    #[test]
    fn migrate_adaptive_compact_full_bleed_touch_sheet() {
        let state = state_with(15);
        let lay = migrate_layout_adaptive([375.0, 667.0], &state, 170.0, 110.0);
        assert_eq!(lay.panel, [12.0, 12.0, 351.0, 643.0], "full-bleed eff");
        assert_eq!(lay.rows.len(), 9, "floor((643 − 156)/50) = 9");
        for row in &lay.rows {
            assert_eq!(row.row[3], 44.0, "тач-цель строки ≥44 (T-M R1)");
            assert!(row.row[0] >= 12.0 && row.row[0] + row.row[2] <= 363.0);
        }
        assert_eq!(lay.go[3], MIGRATE_TOUCH_TARGET, "кнопка ≥44 (T-M R1)");
        assert_eq!(lay.cancel[3], MIGRATE_TOUCH_TARGET);
        assert_eq!(lay.close[2], MIGRATE_TOUCH_TARGET, "«✕» ≥44 (T-M R1)");
        // футер в нижней зоне: низ кнопок — SPACING_LG×0.5 от низа панели
        assert!((lay.go[1] + lay.go[3] - (12.0 + 643.0 - 6.0)).abs() < 0.01);
        // всё внутри вьюпорта
        assert!(lay.go[0] >= 12.0 && lay.go[0] + lay.go[2] <= 363.0);
        assert!(lay.cancel[0] >= 12.0);
        assert!(lay.close[1] >= 12.0 && lay.close[1] + lay.close[3] <= 667.0);
    }

    /// Мало строк — панель по контенту (без мёртвого низа), окно = списку.
    #[test]
    fn migrate_adaptive_few_entries_no_dead_space() {
        let state = state_with(3);
        let lay = migrate_layout_adaptive([1280.0, 800.0], &state, 170.0, 110.0);
        assert_eq!(lay.visible_rows, 3);
        assert_eq!(lay.rows.len(), 3);
        let expected_h = MIGRATE_HEADER_H
            + MIGRATE_HINT_H
            + 3.0 * MIGRATE_ROW_STEP
            + MIGRATE_FOOTER_H
            + tokens::SPACING_LG;
        assert!((lay.panel[3] - expected_h).abs() < 0.01);
    }

    /// Окно helper'а совпадает с окном раскладки на всех канонических
    /// вьюпортах (единый источник для клавиатурного скролла).
    #[test]
    fn migrate_visible_rows_matches_layout_window() {
        for vp in [[1280.0, 800.0], [800.0, 560.0], [375.0, 667.0]] {
            let state = state_with(15);
            let lay = migrate_layout_adaptive(vp, &state, 170.0, 110.0);
            assert_eq!(
                migrate_visible_rows(vp, 15),
                lay.visible_rows,
                "helper == раскладка при {vp:?}"
            );
        }
    }

    /// Скролл с эластичным окном: хвост списка достижим при окне 9.
    #[test]
    fn migrate_adaptive_scroll_threading_elastic_window() {
        assert_eq!(migrate_wheel_scroll(0, -3, 15, 9), 3);
        assert_eq!(migrate_wheel_scroll(11, -100, 15, 9), 6, "хвост = 15 − 9");
        assert_eq!(migrate_scroll_to_reveal(0, 9, 9), 1, "9+1 − 9");
        assert_eq!(migrate_scroll_to_reveal(1, 8, 9), 1, "8 в окне 1..10");
    }

    // --- тост с действием (№45b) --------------------------------------------

    #[test]
    fn toast_action_rect_sits_right_of_centered_text() {
        // тост [40, 756, 1200, 34] — текст центрирован в [40, vp-40],
        // полоса с действием — TOAST_ACTION_STRIP_H (кнопка не режется клипом)
        let toast = [40.0, 756.0, 1200.0, TOAST_ACTION_STRIP_H];
        let r = toast_action_rect(toast, 200.0, 90.0);
        let center = toast[0] + toast[2] * 0.5;
        assert!(
            (r[0] - (center + 100.0 + tokens::SPACING_MD)).abs() < 0.5,
            "кнопка сразу за текстом (x={})",
            r[0]
        );
        assert_eq!(r[3], kit::BUTTON_HEIGHT);
        // вертикально: кнопка целиком в полосе (раньше клип 28 резал низ 34)
        assert_eq!(r[1], toast[1]);
        assert!(
            r[1] + r[3] <= toast[1] + toast[3] + 0.5,
            "кнопка не выходит за полосу тоста"
        );
        // кламп: длинный текст — кнопка у правого края
        let r = toast_action_rect(toast, 1400.0, 90.0);
        assert!(
            (r[0] + 90.0 - (40.0 + 1200.0)).abs() < 0.5,
            "правый край строки"
        );
        // узкий тост — не уходит в минус
        let r = toast_action_rect([40.0, 756.0, 60.0, TOAST_ACTION_STRIP_H], 200.0, 90.0);
        assert!(r[0] >= 0.0);
        assert!(r[2] > 0.0, "ширина не схлопнута");
    }

    #[test]
    fn migrate_layout_fits_minimal_viewport() {
        let mut state = MigrateState::default();
        state.open_with("a.canvas");
        state.set_entries((0..15).map(|i| entry(&format!("c{i:02}.canvas"))).collect());
        let lay = migrate_layout([800.0, 600.0], &state, 170.0, 110.0);
        assert!(lay.panel[0] >= 0.0);
        assert!(lay.panel[1] >= 0.0);
        assert!(lay.panel[0] + lay.panel[2] <= 800.0);
        assert!(
            lay.panel[1] + lay.panel[3] <= 600.0,
            "кламп высоты к вьюпорту"
        );
    }
}
