//! Оверлеи приложения — полноэкранные/модальные поверхности:
//! палитра, настройки, документы, шаблоны, what-if, галереи, wheel-меню,
//! контекстное меню, меню выбора, диалог подтверждения, подсказки.
//!
//! Выделены из `app.rs` сессией рефакторинга 2026-09-24 (этап 2) без
//! изменения поведения: это методы `App`, работающие с тем же состоянием.
//! Паттерн дочернего модуля — как `ui_registry` (FR-052): `use super::*`
//! даёт доступ к приватным полям `App` и импортам родителя.

use super::*;

/// Дефект №15 аудита адаптива (закрыт W-f, 04.10.2026): перенос текста с
/// индикатором продолжения. Строки сверх `max_lines` НЕ теряются молча —
/// на последней показанной строке добавляется индикатор «ещё N» (i18n-фраза
/// `more_key` с подставленным числом скрытых строк); если индикатор не
/// влезает в `max_w`, строка усекается ellipsis'ом до бюджета «индикатор +
/// пробел». Перенос — `TextMeasurer::wrap` (через `admin_ui::wrap_text`),
/// семейство — screen-тексты рендера (SANS); замер — тот же пул шейпинга,
/// что и отрисовка (FR-094), поэтому показанные строки совпадают с
/// нарисованными (кегль и ширина слота — как у `OwnedScreenText`).
#[allow(clippy::too_many_arguments)] // плоский контракт (замер + i18n + слот) — прецедент support.rs:728
fn wrap_lines_with_more(
    m: &mut canvas_ui::measure::TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    language: canvas_core::Language,
    more_key: &'static str,
    text: &str,
    size: f32,
    max_w: f32,
    max_lines: usize,
) -> Vec<String> {
    let lines = crate::admin_ui::wrap_text(m, fs, text, max_w, size);
    let extra = lines.len().saturating_sub(max_lines);
    if extra == 0 {
        return lines;
    }
    let more = i18n::trf(language, more_key, &[("n", &extra.to_string())]);
    let suffix = format!(" {more}");
    let mut shown: Vec<String> = lines.into_iter().take(max_lines).collect();
    let Some(last) = shown.last_mut() else {
        return shown;
    };
    let candidate = format!("{last}{suffix}");
    if m.width_of(fs, &candidate, canvas_render::text::SANS_FAMILY, size) <= max_w {
        *last = candidate;
    } else {
        let suffix_w = m.width_of(fs, &suffix, canvas_render::text::SANS_FAMILY, size);
        let cut = m.ellipsis(
            fs,
            last,
            canvas_render::text::SANS_FAMILY,
            size,
            (max_w - suffix_w).max(0.0),
        );
        *last = if cut.is_empty() {
            more
        } else {
            format!("{cut}{suffix}")
        };
    }
    shown
}

/// FR-CLIP: результат сборки `settings_overlay` — основная полоса
/// (кнопки + модалка + строки) и отдельная полоса выпадающего меню
/// (popup над модалкой, может выходить за её границы).
///
/// - `clip` — клип основной полосы (вьюпорт: разрозненные элементы
///   без единого rect; сужать нельзя — иначе угловые кнопки исчезнут).
/// - `dropdown_clip` — клип дропдаун-полосы = `menu rect` (или пустой,
///   если меню закрыто; пустая пара Vec'ов + пустой клип → `push`
///   пропускает полосу, рендер её не видит).
#[derive(Default)]
pub(super) struct SettingsOverlayRender {
    pub instances: Vec<CardInstance>,
    pub texts: Vec<OwnedScreenText>,
    pub icons: Vec<canvas_render::IconInstance>,
    pub clip: canvas_ui::UiRect,
    pub dropdown_instances: Vec<CardInstance>,
    pub dropdown_texts: Vec<OwnedScreenText>,
    pub dropdown_clip: canvas_ui::UiRect,
}

impl App {
    /// FR-017: данные таблицы сравнения (колонки + ячейки). Строки — union
    /// подменённых переменных всех сценариев; значения — прогон
    /// `propagate_with_lines` с подменами каждого сценария (по прогону на
    /// сценарий — 3–5 прогонов <10 мс, допустимо по роадмапу).
    ///
    /// FR-064 P2 (FR-017 v2): замороженный сценарий показывается ПО
    /// СНИМКУ (pinned значения — правки канваса их не двигают); строки
    /// таблицы строит ядро ([`canvas_core::whatif::compare_scenarios`]):
    /// построчные переменные + изменившиеся узловые итоги (дельты
    /// downstream — эталон ADR-0006 №2), дельты — формат FR-017.
    /// Замороженные сценарии — маркер «❄» в шапке колонки.
    pub(super) fn whatif_compare_table(&self) -> (Vec<String>, Vec<Vec<String>>) {
        let mut columns = vec![
            self.tr(keys::WHATIF_COLUMN_VAR).to_owned(),
            self.tr(keys::WHATIF_BASE).to_owned(),
        ];
        // Снимок на колонку: заморожен — pinned-снимок; иначе свежий прогон
        // (пустые подмены — значения базы: колонка честно равна базе).
        let mut snapshots: Vec<canvas_core::whatif::FrozenScenario> = Vec::new();
        for scenario in &self.scene.scenarios {
            let marker = if self.scene.whatif_is_frozen(&scenario.name) {
                " ❄"
            } else {
                ""
            };
            columns.push(format!("{}{}", scenario.name, marker));
            if let Some(frozen) = self
                .scene
                .frozen
                .iter()
                .find(|snapshot| snapshot.name == scenario.name)
            {
                snapshots.push(canvas_core::whatif::FrozenScenario {
                    name: scenario.name.clone(),
                    solutions: Arc::clone(&frozen.solutions),
                });
            } else {
                let line_exprs =
                    canvas_core::whatif::active_line_exprs(&self.scene.canvas, scenario);
                let overrides = flow::WhatIfOverrides {
                    line_exprs,
                    ..Default::default()
                };
                let solutions =
                    flow::propagate_with_lines(&self.scene.canvas, &overrides).unwrap_or_default();
                snapshots.push(canvas_core::whatif::FrozenScenario {
                    name: scenario.name.clone(),
                    solutions: Arc::new(solutions),
                });
            }
        }
        // Ключи: union валидных подмен всех сценариев (построчные
        // переменные таблицы; узловые итоги добавит ядро по дифу).
        let mut line_keys: Vec<(String, usize)> = Vec::new();
        for scenario in &self.scene.scenarios {
            for key in canvas_core::whatif::active_line_exprs(&self.scene.canvas, scenario).keys() {
                if !line_keys.contains(key) {
                    line_keys.push(key.clone());
                }
            }
        }
        line_keys.sort();
        // FR-064 P2: диф базы со снимками — единый источник в ядре.
        let base_solutions = canvas_scene::read_flow(&self.scene.flow_baseline);
        let comparison =
            canvas_core::whatif::compare_scenarios(&base_solutions, &snapshots, &line_keys);
        let rows: Vec<Vec<String>> = comparison
            .rows
            .iter()
            .map(|row| {
                let label = match row.line {
                    Some(line) => {
                        format!("{} : стр. {}", self.whatif_node_label(&row.node), line + 1)
                    }
                    None => format!(
                        "{} · {}",
                        self.whatif_node_label(&row.node),
                        self.tr(keys::WHATIF_ROW_TOTAL)
                    ),
                };
                let mut cells = vec![label];
                for (index, value) in row.values.iter().enumerate() {
                    let Some(value) = value else {
                        cells.push("—".to_owned());
                        continue;
                    };
                    let cell = match &row.deltas[index] {
                        Some(delta) => format!("{value} ({delta})"),
                        None => value.to_string(),
                    };
                    cells.push(cell);
                }
                cells
            })
            .collect();
        (columns, rows)
    }

    /// Диспетчер кликов по нижнему бару (пилюля вне режима).
    pub(super) fn apply_whatif_bar_action(&mut self, action: BarAction) {
        match action {
            BarAction::Enter => self.enter_whatif_mode(),
            BarAction::Close => self.exit_whatif_mode(),
            BarAction::Base => self.scene.whatif_activate(None),
            BarAction::Scenario(index) => self.scene.whatif_activate(Some(index)),
            BarAction::NewScenario => {
                // CR-016: имя — пустое, scene выбирает первый свободный
                // номер (иначе len+1 коллидирует с существующими).
                let snapshot = self.scene.canvas.clone();
                match self.scene.whatif_create_scenario("") {
                    Ok(index) => {
                        // Список сценариев персистентен: мутация
                        // `canvasdesk.whatif` одним undo-шагом (FR-006).
                        canvas_core::whatif::scenarios_to_canvas(
                            &mut self.scene.canvas,
                            &self.scene.scenarios,
                        );
                        if self.scene.canvas != snapshot {
                            self.scene.push_undo(snapshot);
                            self.scene.mark_dirty();
                        }
                        self.scene.whatif_activate(Some(index));
                    }
                    Err(err) => self.show_toast(err),
                }
            }
            BarAction::ToggleOverrides => {
                self.whatif_list_open = !self.whatif_list_open;
                if self.whatif_list_open {
                    self.whatif_compare_open = false;
                }
            }
            BarAction::Apply => {
                // Паттерн MCP whatif_apply: записать подмены в persisted-
                // строки/params, удалить сценарий (Q6b) — один undo-шаг.
                if self.scene.active_scenario.is_none() {
                    return;
                }
                let snapshot = self.scene.canvas.clone();
                let applied = self.scene.whatif_apply_active();
                canvas_core::whatif::scenarios_to_canvas(
                    &mut self.scene.canvas,
                    &self.scene.scenarios,
                );
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                    self.scene.mark_dirty();
                }
                self.whatif_list_open = false;
                self.scene.recompute_flow();
                self.show_toast(
                    self.trf(keys::TOAST_APPLY_DONE, &[("{count}", &applied.to_string())]),
                );
            }
            BarAction::Reset => {
                // Сброс подмен активного сценария — runtime-only.
                if let Some(index) = self.scene.active_scenario {
                    if let Some(scenario) = self.scene.scenarios.get_mut(index) {
                        scenario.line_exprs.clear();
                    }
                    self.scene.recompute_flow();
                }
            }
            BarAction::Freeze => {
                // FR-064 P2: заморозка/разморозка активного сценария —
                // снимок решений за Arc (runtime); имена — в
                // `canvasdesk.whatif.frozen` (персистентность, тот же
                // undo-шаг паттерн, что у create-scenario).
                let Some(index) = self.scene.active_scenario else {
                    return;
                };
                let Some(name) = self.scene.scenarios.get(index).map(|s| s.name.clone()) else {
                    return;
                };
                let snapshot = self.scene.canvas.clone();
                let was_frozen = self.scene.whatif_is_frozen(&name);
                if was_frozen {
                    self.scene.whatif_unfreeze(&name);
                } else if let Err(err) = self.scene.whatif_freeze_active() {
                    self.show_toast(err);
                    self.request_redraw();
                    return;
                }
                let frozen_names = self.scene.whatif_frozen_names();
                canvas_core::whatif::frozen_to_canvas(&mut self.scene.canvas, &frozen_names);
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                    self.scene.mark_dirty();
                }
                self.show_toast(if was_frozen {
                    self.trf(keys::WHATIF_UNFROZEN_TOAST, &[("{name}", &name)])
                } else {
                    self.trf(keys::WHATIF_FROZEN_TOAST, &[("{name}", &name)])
                });
            }
            BarAction::Compare => {
                self.whatif_compare_open = !self.whatif_compare_open;
                if self.whatif_compare_open {
                    self.whatif_list_open = false;
                }
            }
        }
        self.request_redraw();
    }

    /// FR-017: оверлей нижнего бара (пилюля вне режима, полоса, список
    /// подмен, таблица сравнения). Screen-space — константный размер при
    /// любом зуме (паттерн `canvas_menu_overlay`).
    pub(super) fn whatif_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let viewport = self.viewport_logical();
        let palette = self.effective_palette();
        // FR-053 (U3 F-9): disabled-текст — слот темы (бывший локальный hex).
        let dim = palette.control_disabled_text;
        if !self.scene.whatif_active {
            // Свёрнутый вид: пилюля входа.
            // FR-055 (этап U4): стиль пилюли — через кит (kit::control_style_of:
            // явные слоты menu_fill + DIALOG-рамка, капсула h/2 — каноническая
            // форма; I-1 ноль скачка; текст — слот title напрямую, как был).
            let pill = whatif_ui::enter_pill_rect(viewport);
            let pill_style = canvas_ui::kit::control_style_of(
                palette.menu_fill,
                canvas_core::tokens::DIALOG_BUTTON_BORDER,
                [0.0; 4],
                pill[3] / 2.0,
            );
            instances.push(CardInstance {
                pos: [pill[0], pill[1]],
                size: [pill[2], pill[3]],
                fill: pill_style.fill,
                border: pill_style.border,
                params: [pill_style.radius, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            let (pill_box, pill_width) = centered_box(pill, 4.0);
            texts.push(OwnedScreenText {
                text: self.tr(keys::WHATIF_PILL).to_owned(),
                origin: [pill_box[0], pill[1] + 8.0],
                width: pill_width,
                font_size: 13.0,
                color: palette.title,
                align: TextAlign::Center,
            });
            return (instances, texts);
        }
        let layout = self.whatif_bar_layout();
        let chip = |rect: [f32; 4], fill: [f32; 4], border: [f32; 4]| CardInstance {
            pos: [rect[0], rect[1]],
            size: [rect[2], rect[3]],
            fill,
            border,
            params: [6.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        };
        // FR-055 (этап U4): контейнер бара — через кит (kit::panel_style_of:
        // явные слоты menu_fill + DIALOG-рамка, канонический радиус 8 —
        // I-1 ноль скачка; унификация радиусов — v2 с токен-паритетом)
        let bar_style = canvas_ui::kit::panel_style_of(
            palette.menu_fill,
            canvas_core::tokens::DIALOG_BUTTON_BORDER,
            8.0,
            canvas_core::tokens::SPACING_MD,
        );
        instances.push(CardInstance {
            pos: [layout.rect[0], layout.rect[1]],
            size: [layout.rect[2], layout.rect[3]],
            fill: bar_style.fill,
            border: bar_style.border,
            params: [bar_style.radius, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        // Индикатор режима.
        texts.push(OwnedScreenText {
            text: "WHAT-IF".to_owned(),
            origin: [layout.indicator[0], layout.indicator[1] + 6.0],
            width: layout.indicator[2],
            font_size: 12.0,
            color: palette.whatif_accent,
            align: TextAlign::Center,
        });
        // Чипы: «База» + сценарии + «+». Активный — акцентной рамкой.
        let active = self.scene.active_scenario;
        let mut chip_text = |rect: [f32; 4], label: &str, current: bool| {
            instances.push(chip(
                rect,
                if current {
                    canvas_core::tokens::DIALOG_BUTTON_PRIMARY
                } else {
                    canvas_core::tokens::DIALOG_BUTTON_SECONDARY
                },
                if current {
                    canvas_core::tokens::WHATIF_CHIP
                } else {
                    canvas_core::tokens::DIALOG_BUTTON_BORDER
                },
            ));
            let (box_origin, box_width) = centered_box(rect, 3.0);
            texts.push(OwnedScreenText {
                text: label.to_owned(),
                origin: [box_origin[0], rect[1] + 6.0],
                width: box_width,
                font_size: 13.0,
                color: if current {
                    token_color(canvas_core::tokens::DIALOG_TEXT)
                } else {
                    palette.title
                },
                align: TextAlign::Center,
            });
        };
        chip_text(layout.base, self.tr(keys::WHATIF_BASE), active.is_none());
        for (i, rect) in layout.scenarios.iter().enumerate() {
            // FR-053 (U3): подпись — из раскладки (та же Ellipsis-строка,
            // по которой считалась ширина чипа — урок CR-015; при узком
            // окне подпись усечена по фактической ширине сжатого чипа).
            let label = layout.scenario_labels.get(i).cloned().unwrap_or_default();
            chip_text(*rect, &label, active == Some(i));
        }
        chip_text(layout.new_scenario, "+", false);
        // Счётчик подмен (клик — список; раскрыт — акцент).
        let count = self.scene.whatif_override_count();
        let counter_label = self.trf(keys::WHATIF_OVERRIDES, &[("{count}", &count.to_string())]);
        chip_text(
            layout.overrides,
            &counter_label,
            self.whatif_list_open && count > 0,
        );
        // Кнопки. Apply/Сброс — без активного сценария/подмен приглушены.
        // FR-064 P2: лейбл заморозки — по состоянию (та же строка, что в
        // раскладке); кнопка приглушена без активного сценария.
        let has_overrides = active.is_some() && count > 0;
        let has_active = active.is_some();
        let freeze_label = match active {
            Some(index)
                if self
                    .scene
                    .scenarios
                    .get(index)
                    .is_some_and(|scenario| self.scene.whatif_is_frozen(&scenario.name)) =>
            {
                self.tr(keys::WHATIF_UNFREEZE)
            }
            _ => self.tr(keys::WHATIF_FREEZE),
        };
        let buttons = [
            (layout.apply, self.tr(keys::WHATIF_APPLY), has_overrides),
            (layout.reset, self.tr(keys::WHATIF_RESET), has_overrides),
            (layout.freeze, freeze_label, has_active),
            (
                layout.compare,
                self.tr(keys::WHATIF_COMPARE),
                !self.scene.scenarios.is_empty(),
            ),
            (layout.close, "×", true),
        ];
        for (rect, label, enabled) in buttons {
            instances.push(chip(
                rect,
                if enabled {
                    canvas_core::tokens::DIALOG_BUTTON_SECONDARY
                } else {
                    canvas_core::tokens::WHATIF_CHIP_DIM
                },
                canvas_core::tokens::DIALOG_BUTTON_BORDER,
            ));
            let (box_origin, box_width) = centered_box(rect, 3.0);
            texts.push(OwnedScreenText {
                text: label.to_owned(),
                origin: [box_origin[0], rect[1] + 6.0],
                width: box_width,
                font_size: 13.0,
                color: if enabled { palette.title } else { dim },
                align: TextAlign::Center,
            });
        }
        // Раскрытый список подмен.
        if self.whatif_list_open {
            let rows = self.whatif_override_rows();
            let list = whatif_ui::overrides_list_layout(layout.rect, rows.len(), viewport);
            instances.push(CardInstance {
                pos: [list[0], list[1]],
                size: [list[2], list[3]],
                fill: palette.menu_fill,
                border: canvas_core::tokens::DIALOG_BUTTON_BORDER,
                params: [6.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            if rows.is_empty() {
                texts.push(OwnedScreenText {
                    text: self.tr(keys::WHATIF_NO_OVERRIDES).to_owned(),
                    origin: [
                        list[0] + whatif_ui::LIST_MARGIN,
                        list[1] + whatif_ui::LIST_MARGIN + 4.0,
                    ],
                    width: list[2] - whatif_ui::LIST_MARGIN * 2.0,
                    font_size: 13.0,
                    color: dim,
                    align: TextAlign::Left,
                });
            }
            // W-a: высота списка клампится к вьюпорту — рисуем только
            // вмещающиеся ряды (вместимость — из раскладки, расхождений
            // с hit-тестом/реестром нет).
            let visible = whatif_ui::list_visible_rows(rows.len(), list);
            for (i, row) in rows.iter().take(visible).enumerate() {
                let rect = whatif_ui::override_row_rect(list, i);
                let mut text = format!(
                    "{} → стр. {}: {} → {}",
                    row.node_label,
                    row.line + 1,
                    row.base,
                    row.whatif
                );
                let color = if row.stale.is_some() {
                    text = format!("{text}  ⚠ {}", row.stale.as_deref().unwrap_or_default());
                    Color::rgb(
                        canvas_core::tokens::ERROR[0],
                        canvas_core::tokens::ERROR[1],
                        canvas_core::tokens::ERROR[2],
                    )
                } else {
                    palette.title
                };
                texts.push(OwnedScreenText {
                    text,
                    origin: [rect[0] + 4.0, rect[1] + 4.0],
                    width: rect[2] - whatif_ui::REMOVE_WIDTH - 8.0,
                    font_size: 13.0,
                    color,
                    align: TextAlign::Left,
                });
                let remove = whatif_ui::remove_button_rect(list, i);
                texts.push(OwnedScreenText {
                    text: "×".to_owned(),
                    origin: [remove[0], remove[1] + 2.0],
                    width: remove[2],
                    font_size: 13.0,
                    color: dim,
                    align: TextAlign::Center,
                });
            }
            // W-a: усечённый хвост — честный индикатор (никаких молчаливых
            // take, CR-015) в зарезервированном раскладкой слоте.
            if rows.len() > visible {
                texts.push(OwnedScreenText {
                    text: format!("… ещё {}", rows.len() - visible),
                    origin: [
                        list[0] + whatif_ui::LIST_MARGIN,
                        list[1]
                            + whatif_ui::LIST_MARGIN
                            + visible as f32 * whatif_ui::LIST_ROW_H
                            + 4.0,
                    ],
                    width: list[2] - whatif_ui::LIST_MARGIN * 2.0,
                    font_size: 13.0,
                    color: dim,
                    align: TextAlign::Left,
                });
            }
        }
        // Таблица сравнения сценариев.
        if self.whatif_compare_open {
            let (columns, rows) = self.whatif_compare_table();
            let table = whatif_ui::table_layout(&columns, rows.len(), layout.rect, viewport);
            instances.push(CardInstance {
                pos: [table.rect[0], table.rect[1]],
                size: [table.rect[2], table.rect[3]],
                fill: palette.menu_fill,
                border: canvas_core::tokens::DIALOG_BUTTON_BORDER,
                params: [6.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            for (c, rect) in table.header.iter().enumerate() {
                texts.push(OwnedScreenText {
                    text: columns.get(c).cloned().unwrap_or_default(),
                    origin: [rect[0] + 6.0, rect[1] + 6.0],
                    width: rect[2] - 12.0,
                    font_size: 12.0,
                    color: if c == 0 { dim } else { palette.whatif_accent },
                    align: TextAlign::Left,
                });
            }
            for (r, row) in rows.iter().enumerate() {
                for (c, cell) in row.iter().enumerate() {
                    let Some(rect) = table.cells.get(r).and_then(|cells| cells.get(c)) else {
                        continue;
                    };
                    texts.push(OwnedScreenText {
                        text: cell.clone(),
                        origin: [rect[0] + 6.0, rect[1] + 4.0],
                        width: rect[2] - 12.0,
                        font_size: 13.0,
                        color: if c == 0 { palette.title } else { palette.body },
                        align: TextAlign::Left,
                    });
                }
            }
            // W-a: кламп высоты таблицы — усечённый хвост строк показываем
            // индикатором в слоте из раскладки (cells содержит только
            // вмещающиеся строки — за клампом ничего не рисуется).
            if let Some(tail) = table.tail {
                texts.push(OwnedScreenText {
                    text: format!("… ещё {}", rows.len() - table.cells.len()),
                    origin: [tail[0] + 6.0, tail[1] + 4.0],
                    width: tail[2] - 12.0,
                    font_size: 13.0,
                    color: dim,
                    align: TextAlign::Left,
                });
            }
        }
        (instances, texts)
    }

    /// FR-050 Н2 (этап C): открыть меню выбора у курсора (screen-space).
    pub(super) fn open_choice_menu(&mut self, title_key: &'static str, items: Vec<ChoiceItem>) {
        if items.is_empty() {
            return;
        }
        let viewport = self.viewport_logical();
        // Геометрия — та же, что у контекстного меню (T7): ширина/высота
        // пунктов константны (+ строка заголовка), меню не выезжает за
        // правый/нижний край окна
        let count = items.len();
        let [_, _, _, mh] = crate::ui::menu_rect_for([0.0; 2], count);
        let mh = mh + CHOICE_MENU_TITLE_H;
        let origin = [
            (self.cursor[0] + 8.0).min((viewport[0] - crate::ui::MENU_WIDTH).max(0.0)),
            (self.cursor[1] + 8.0).min((viewport[1] - mh).max(0.0)),
        ];
        self.choice_menu = Some(ChoiceMenu {
            origin,
            title_key,
            items,
            hovered: None,
        });
        self.request_redraw();
    }

    /// Н9-4: оверлей панели карты — screen-space квады + тексты (паттерн
    /// search_overlay): панель, заголовок, «✕», строки с hover-подсветкой,
    /// скроллбар; пустой канвас — строка-подсказка. Unmapped-строки —
    /// янтарным акцентом анализа (Р-3, тот же тон, что пунктир рёбер).
    /// FR-059 (волна 1 кита): раскладка — flowmap_ui на ките (stack +
    /// list_rows/scroll); отрисовка — через [`Painter`] (items → полоса,
    /// `paint_items_to_band`), состояние строк — [`WidgetState`]
    /// (Hovered — прежняя подсветка, 0 визуального скачка). Строка
    /// «… ещё N» удалена — переполнение честно прокручивается (бегунок
    /// кита, цвет — слот `control_border`).
    pub(super) fn flow_map_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        if !self.flow_map_open {
            return (instances, texts);
        }
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (instances, texts);
        }
        let rows = self.flow_map_rows();
        let mut scroll = self.flow_map_scroll.clone();
        let lay = flowmap_ui::flow_map_layout(viewport, rows.len(), &mut scroll);
        let palette = self.effective_palette();
        let kit_palette = palette.kit_palette();
        let mut d = Painter::new();
        // Панель — прежние слоты дословно
        d.rect(lay.panel, palette.menu_fill, palette.palette_border, 8.0);
        // Заголовок + «✕» (кнопка — тем же стилем, что панель поиска)
        d.label(
            canvas_ui::geometry::UiRect::new(
                lay.panel.x + 12.0,
                lay.panel.y + 10.0,
                (lay.panel.w - 48.0).max(0.0),
                20.0,
            ),
            self.tr(keys::FLOW_MAP_TITLE),
            kit_palette.text_title,
            13.0,
            PaintAlign::Left,
        );
        d.label(
            canvas_ui::geometry::UiRect::new(
                lay.close.x + 5.0,
                lay.close.y + 3.0,
                (lay.close.w - 8.0).max(0.0),
                18.0,
            ),
            "×",
            kit_palette.text_title,
            14.0,
            PaintAlign::Left,
        );
        // Пустое состояние — подсказка
        if rows.is_empty() {
            d.label(
                canvas_ui::geometry::UiRect::new(
                    lay.panel.x + 12.0,
                    lay.panel.y + flowmap_ui::HEADER_H + 8.0,
                    (lay.panel.w - 24.0).max(0.0),
                    20.0,
                ),
                self.tr(keys::FLOW_MAP_EMPTY),
                kit_palette.text,
                12.0,
                PaintAlign::Left,
            );
            paint_items_to_band(d.take_items(), &mut instances, &mut texts);
            return (instances, texts);
        }
        // Строки: hover-подсветка по курсору (аффорданс — как меню T7),
        // unmapped — янтарь анализа (Р-3). Состояние — WidgetState
        // (Hovered → прежняя подсветка), цвет — прежний (0 скачка)
        let amber = canvas_render::cards::UNMAPPED_EDGE_COLOR;
        let hovered = flowmap_ui::flow_map_row_at(&lay, self.cursor);
        for (index, rect) in &lay.rows {
            let row = &rows[*index];
            let mut state = WidgetState::default();
            state.set_pointer(hovered == Some(*index), false);
            if state.kit_state() == canvas_ui::kit::KitState::Hovered {
                d.rect(*rect, palette.control_hover_fill, [0.0; 4], 4.0);
            }
            d.label(
                canvas_ui::geometry::UiRect::new(
                    rect.x + 6.0,
                    rect.y + 5.0,
                    (rect.w - 10.0).max(0.0),
                    flowmap_ui::ROW_H,
                ),
                &self.flow_map_row_text(row),
                if row.value.is_some() {
                    kit_palette.text
                } else {
                    amber
                },
                12.0,
                PaintAlign::Left,
            );
        }
        // FR-059: бегунок скролла (кит scroll_bar) — цвет прежнего хрома
        // панелей (слот рамки панелей)
        if let Some(knob) = canvas_ui::kit::scroll_bar(lay.list, &scroll, &kit_palette) {
            d.rect(knob, palette.palette_border, [0.0; 4], 2.0);
        }
        paint_items_to_band(d.take_items(), &mut instances, &mut texts);
        (instances, texts)
    }

    /// Оверлей панели поиска (T14): квады + тексты в screen-space
    /// (FrameOverlay), геометрия — search_ui::layout.
    pub(super) fn search_overlay(
        &self,
    ) -> (
        Vec<CardInstance>,
        Vec<OwnedScreenText>,
        Vec<canvas_render::IconInstance>,
    ) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let mut icons = Vec::new();
        let viewport = self.viewport_logical();
        if !self.search.is_open() || viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (instances, texts, icons);
        }
        let lay = search_layout(viewport[0], viewport[1], &self.search);
        let palette = self.effective_palette();
        let panel = rect_xywh(lay.panel_rect);
        instances.push(CardInstance {
            pos: [panel[0], panel[1]],
            size: [panel[2], panel[3]],
            fill: palette.menu_fill,
            border: palette.palette_border,
            params: [8.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        let input = rect_xywh(lay.input_rect);
        instances.push(CardInstance {
            pos: [input[0], input[1]],
            size: [input[2], input[3]],
            fill: palette.search_input_fill,
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        // Каретка — литерал «|» в конце текста (MVP, без мерцания)
        let query_with_caret = format!("{}|", self.search.input.query());
        texts.push(OwnedScreenText {
            text: query_with_caret,
            origin: [input[0] + 10.0, input[1] + 9.0],
            width: (input[2] - 20.0).max(10.0),
            font_size: 14.0,
            color: palette.title,
            align: TextAlign::Left,
        });
        for (visible, rect) in lay.row_rects.iter().enumerate() {
            let row = self.search.scroll_top + visible;
            let Some(entry) = self.search.rows.get(row) else {
                break;
            };
            let selected = self.search.selected == Some(row);
            let row_rect = rect_xywh(*rect);
            // Hover-подсветка результата (не выбранного — выделенный несёт
            // акцент): практика списков результатов (VS Code)
            let row_hover = point_in_rect(row_rect, self.cursor);
            instances.push(CardInstance {
                pos: [row_rect[0], row_rect[1]],
                size: [row_rect[2], row_rect[3]],
                fill: if selected {
                    palette.control_selected_fill
                } else if row_hover {
                    palette.control_hover_fill
                } else {
                    palette.search_row_fill
                },
                border: [0.0; 4],
                params: [4.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            // Владелец 2026-10-02: строки документации помечены бейджем «?»
            // (SVG из атласа, в Glyph-наборе — текстовый фолбэк) — не поиск
            // по нодам; текст строк сдвинут вправо на зону бейджа.
            let is_docs = entry.kind == crate::search_ui::SearchRowKind::Docs;
            if is_docs {
                let badge_rect = [row_rect[0] + 8.0, row_rect[1] + 6.0, 16.0, 16.0];
                let badge_tint = crate::palette::color_to_rgba(palette.icon);
                let drawn = self.icon_set_active().and_then(|set| {
                    canvas_render::icon_uv(set, "question").map(|(uv_min, uv_max)| {
                        icons.push(canvas_render::IconInstance {
                            pos: [badge_rect[0], badge_rect[1]],
                            size: [badge_rect[2], badge_rect[3]],
                            uv_min,
                            uv_max,
                            tint: badge_tint,
                        });
                    })
                });
                if drawn.is_none() {
                    texts.push(OwnedScreenText {
                        text: "?".to_owned(),
                        origin: [badge_rect[0] + 2.0, badge_rect[1] + 1.0],
                        width: 12.0,
                        font_size: 13.0,
                        color: palette.icon,
                        align: TextAlign::Left,
                    });
                }
            }
            // FR-088: текст строки — готовый из раскладки (row_texts):
            // заголовок с ellipsis + перенесённый подзаголовок с позициями.
            // Рендер не дублирует замер/смещения — единый источник геометрии
            // (та же раскладка, что hit-rect'ы реестра).
            let Some(rt) = lay.row_texts.get(visible) else {
                continue;
            };
            texts.push(OwnedScreenText {
                text: rt.title.clone(),
                origin: rt.title_pos,
                width: rt.text_width,
                font_size: crate::search_ui::TITLE_FONT_SIZE,
                color: palette.title,
                align: TextAlign::Left,
            });
            for (line, pos) in rt.subtitle_lines.iter().zip(&rt.subtitle_pos) {
                texts.push(OwnedScreenText {
                    text: line.clone(),
                    origin: *pos,
                    width: rt.text_width,
                    font_size: crate::search_ui::SUB_FONT_SIZE,
                    color: palette.body,
                    align: TextAlign::Left,
                });
            }
        }
        (instances, texts, icons)
    }

    /// FR-070 (этап 1): оверлей админпанели — затемнение, панель, шапка
    /// (заголовок / «Сброс» / тема / «✕»), сайдбар секций (реальные
    /// kit-кнопки), демо-зона с заголовком секции и подсказкой; тела секций
    /// — этапы 2–4 FR-070. Эффективная палитра — с live-переопределением.
    pub(super) fn admin_panel_overlay(
        &self,
    ) -> (
        Vec<CardInstance>,
        Vec<OwnedScreenText>,
        Vec<canvas_render::IconInstance>,
    ) {
        let mut out = (Vec::new(), Vec::new(), Vec::new());
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return out;
        }
        let palette = self.admin_effective_palette();
        let lang = self.settings.language;
        let lay = self.admin_layout_current();
        let mut d = crate::kit_ui::KitDraw::new();
        // FR-ICONS: установить активный набор (None = Glyph fallback).
        d.set_icon_set(self.icon_set_active());
        let vp = canvas_ui::geometry::UiRect::new(0.0, 0.0, viewport[0], viewport[1]);
        let cursor = self.cursor;
        let hover = |r: &canvas_ui::geometry::UiRect| crate::kit_ui::cursor_in(r, cursor);

        // Затемнение + панель (kit Modal — паттерн витрины)
        d.rect(vp, canvas_core::tokens::WHEEL_DIM, [0.0; 4], 0.0);
        let panel_style = canvas_ui::kit::modal_style(&palette);
        d.rect(
            lay.panel,
            panel_style.fill,
            panel_style.border,
            panel_style.radius,
        );

        // Шапка: заголовок + «Сброс» (Disabled без переопределения) + тема + «✕»
        d.label_left(
            lay.title,
            crate::i18n::tr(lang, crate::i18n::keys::ADMIN_TITLE),
            palette.text_title,
            14.0,
        );
        let mut reset_widget = WidgetState::default();
        reset_widget.set_pointer(hover(&lay.reset), false);
        reset_widget.set_disabled(self.admin_palette_override.is_none());
        let reset_style = canvas_ui::kit::button_style(
            canvas_ui::kit::ButtonVariant::Danger,
            reset_widget.kit_state(),
            &palette,
        );
        d.control(lay.reset, &reset_style);
        d.label_center(
            lay.reset,
            crate::i18n::tr(lang, crate::i18n::keys::ADMIN_RESET),
            reset_style.text,
            13.0,
        );
        // Измеритель для кнопки темы (тот же паттерн витрины)
        let mut m = crate::kit_ui::new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let mut theme_widget = WidgetState::default();
        theme_widget.set_pointer(hover(&lay.theme), false);
        let theme_style = canvas_ui::kit::button_style(
            canvas_ui::kit::ButtonVariant::Primary,
            theme_widget.kit_state(),
            &palette,
        );
        let (theme_rect, theme_label) =
            crate::kit_ui::theme_button_layout(lay.theme, lang, &mut m, &mut fs);
        d.control(theme_rect, &theme_style);
        d.label_center(theme_rect, &theme_label, theme_style.text, 13.0);
        let mut close_widget = WidgetState::default();
        close_widget.set_pointer(hover(&lay.close), false);
        let close_style = canvas_ui::kit::icon_button_style(close_widget.kit_state(), &palette);
        d.control(lay.close, &close_style);
        d.icon(lay.close, "close", "×", close_style.text, 13.0);

        // Сайдбар: пункты — реальные kit-кнопки (активная — слот Selected)
        for (i, item) in lay.sidebar_items.iter().enumerate() {
            let active = crate::admin_ui::AdminSection::at(i) == Some(self.admin_section);
            let state = if active {
                canvas_ui::kit::KitState::Selected
            } else if hover(item) {
                canvas_ui::kit::KitState::Hovered
            } else {
                canvas_ui::kit::KitState::Normal
            };
            let style = canvas_ui::kit::button_style(
                canvas_ui::kit::ButtonVariant::Secondary,
                state,
                &palette,
            );
            d.control(*item, &style);
            if let Some(sec) = crate::admin_ui::AdminSection::at(i) {
                d.label_left(
                    canvas_ui::geometry::UiRect::new(
                        item.x + kit::BUTTON_PAD_H,
                        item.y,
                        (item.w - kit::BUTTON_PAD_H).max(0.0),
                        item.h,
                    ),
                    crate::i18n::tr(lang, sec.label_key()),
                    style.text,
                    13.0,
                );
            }
        }

        // Демо-зона: рамка + заголовок секции + подсказка (этап 1)
        d.rect(
            lay.demo,
            [0.0; 4],
            palette.control_border,
            canvas_core::tokens::RADIUS_PANEL,
        );
        let (origin, title_key) = lay.section_title;
        d.label_left(
            canvas_ui::geometry::UiRect::new(
                origin.x + 10.0,
                origin.y + 6.0,
                (lay.demo.w - 20.0).max(0.0),
                18.0,
            ),
            crate::i18n::tr(lang, title_key),
            palette.text_title,
            13.0,
        );
        // Подсказка: wrapped-строки раскладки (сдвиг на скролл демо-зоны)
        for (line_idx, line) in lay.hint_lines.iter().enumerate() {
            let y = origin.y + 24.0 + line_idx as f32 * 18.0 - self.admin_scroll.offset;
            if y + 18.0 < lay.demo.y || y > lay.demo.bottom() {
                continue;
            }
            d.label_left(
                canvas_ui::geometry::UiRect::new(
                    origin.x + 10.0,
                    y,
                    (lay.demo.w - 20.0).max(0.0),
                    18.0,
                ),
                line,
                palette.text_muted,
                crate::admin_ui::LABEL_SIZE,
            );
        }
        // Тело секции (этап 2): «Компоненты» / «Наполнение»
        if let Some(comp) = &lay.components {
            crate::admin_ui::draw_components(&mut d, comp, &palette, lang);
        }
        if let Some(fill) = &lay.fill {
            crate::admin_ui::draw_fill(&mut d, fill, &palette, lang);
        }
        if let Some(canvas) = &lay.canvas {
            crate::admin_ui::draw_canvas(&mut d, canvas, &palette, lang);
        }
        if let Some(tokens) = &lay.tokens {
            crate::admin_ui::draw_tokens(&mut d, tokens, &palette, lang);
        }
        // Бегунок скролла демо-зоны (контент выше окна)
        if let Some(knob) = canvas_ui::kit::scroll_bar(lay.demo, &self.admin_scroll, &palette) {
            d.rect(knob, palette.control_border, [0.0; 4], 2.0);
        }

        out.0 = d.quads;
        out.1 = d
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
        // FR-ICONS: иконки текущего кадра (SVG-атлас; пусто для Glyph).
        out.2 = d.icons;
        out
    }

    /// FR-070: эффективная палитра админпанели — live-переопределение
    /// слотов (если есть) или палитра темы.
    pub(crate) fn admin_effective_palette(&self) -> canvas_ui::kit::KitPalette {
        self.admin_palette_override
            .unwrap_or_else(|| self.effective_palette().kit_palette())
    }

    /// FR-ICONS: активный набор иконок как `Option<&'static str>` для
    /// `KitDraw::set_icon_set`. `None` = Glyph fallback (прежнее поведение),
    /// `Some(set_id)` = SVG-набор (`"lucide"`/`"material"`/`"feather"`/
    /// `"bootstrap"`). Читается из `settings.icon_style` каждый кадр.
    pub(crate) fn icon_set_active(&self) -> Option<&'static str> {
        if self.settings.icon_style.is_svg() {
            Some(self.settings.icon_style.id())
        } else {
            None
        }
    }

    /// FR-070: раскладка админпанели текущего состояния (один источник
    /// геометрии для hit-rect'ов реестра и отрисовки).
    pub(crate) fn admin_layout_current(&self) -> crate::admin_ui::AdminLayout {
        self.admin_layout_at(self.viewport_logical())
    }

    /// FR-070: раскладка админпанели на явном вьюпорте (headless-тесты —
    /// G4: 1280×800 / 800×560).
    pub(crate) fn admin_layout_at(&self, viewport: [f32; 2]) -> crate::admin_ui::AdminLayout {
        let palette = self.admin_effective_palette();
        let theme = self.effective_palette();
        let mut m = crate::kit_ui::new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        crate::admin_ui::admin_layout(
            viewport,
            self.admin_section,
            self.admin_scroll.offset,
            &palette,
            theme.card_fill,
            self.settings.language,
            &mut m,
            &mut fs,
        )
    }

    /// FR-055 (этап U4 PRD-0009, F-8): витрина кита — полоса Modals кадра.
    /// Компоненты × состояния × RU/EN × темы; слоты палитры — из
    /// `effective_palette().kit_palette()` (маппинг render→ui).
    /// FR-059 (волна 1 кита): секции компонентов v2 — TextField/Switch/
    /// Card/список+скролл/Icon-глифы (контракт FR-058); контент
    /// прокручивается ([`WidgetState`] для состояний шапки — вместо
    /// deprecated-делегатов; бегунок — kit::scroll_bar).
    pub(super) fn kit_gallery_overlay(
        &self,
    ) -> (
        Vec<CardInstance>,
        Vec<OwnedScreenText>,
        Vec<canvas_render::IconInstance>,
    ) {
        let mut out = (Vec::new(), Vec::new(), Vec::new());
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return out;
        }
        let palette = self.effective_palette().kit_palette();
        let lang = self.settings.language;
        let mut m = crate::kit_ui::new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let scroll = self.kit_gallery_scroll.clone();
        let lay = crate::kit_ui::gallery_layout(viewport, lang, &scroll, &palette, &mut m, &mut fs);
        let mut d = crate::kit_ui::KitDraw::new();
        // FR-ICONS: установить активный набор (None = Glyph fallback).
        d.set_icon_set(self.icon_set_active());
        let vp = canvas_ui::geometry::UiRect::new(0.0, 0.0, viewport[0], viewport[1]);
        let cursor = self.cursor;

        // Затемнение (kit Modal.dim) + панель (kit Modal.panel)
        d.rect(vp, canvas_core::tokens::WHEEL_DIM, [0.0; 4], 0.0);
        let panel_style = canvas_ui::kit::modal_style(&palette);
        d.rect(
            lay.panel,
            panel_style.fill,
            panel_style.border,
            panel_style.radius,
        );

        // Шапка: заголовок + кнопка темы (реальный kit::Button Primary) + «✕»
        // FR-059: состояния — WidgetState (машина состояний FR-057)
        let hover = |r: &canvas_ui::geometry::UiRect| crate::kit_ui::cursor_in(r, cursor);
        let mut theme_widget = WidgetState::default();
        theme_widget.set_pointer(hover(&lay.theme), false);
        let theme_style = canvas_ui::kit::button_style(
            canvas_ui::kit::ButtonVariant::Primary,
            theme_widget.kit_state(),
            &palette,
        );
        let (theme_rect, theme_label) =
            crate::kit_ui::theme_button_layout(lay.theme, lang, &mut m, &mut fs);
        d.control(theme_rect, &theme_style);
        d.label_center(theme_rect, &theme_label, theme_style.text, 13.0);
        let mut close_widget = WidgetState::default();
        close_widget.set_pointer(hover(&lay.close), false);
        let close_style = canvas_ui::kit::icon_button_style(close_widget.kit_state(), &palette);
        d.control(lay.close, &close_style);
        d.icon(lay.close, "close", "×", close_style.text, 13.0);
        d.label_left(
            canvas_ui::geometry::UiRect::new(lay.title.x, lay.title.y + 6.0, lay.title.w, 20.0),
            crate::i18n::tr(lang, crate::i18n::keys::KIT_GALLERY_TITLE),
            palette.text_title,
            14.0,
        );

        let label = |key: &'static str| crate::i18n::tr(lang, key).to_owned();

        // Подписи секций (приглушённые — слот text_muted)
        for (origin, key) in &lay.section_titles {
            let text = label(key);
            d.label_left(
                canvas_ui::geometry::UiRect::new(origin.x, origin.y, lay.content.w, 16.0),
                &text,
                palette.text_muted,
                11.0,
            );
        }
        // Buttons: 4 варианта × 4 состояния (слоты состояний палитры)
        for row in &lay.button_rows {
            let title_key = match row.variant {
                canvas_ui::kit::ButtonVariant::Primary => crate::i18n::keys::KIT_BTN_PRIMARY,
                canvas_ui::kit::ButtonVariant::Secondary => crate::i18n::keys::KIT_BTN_SECONDARY,
                canvas_ui::kit::ButtonVariant::Ghost => crate::i18n::keys::KIT_BTN_GHOST,
                canvas_ui::kit::ButtonVariant::Danger => crate::i18n::keys::KIT_BTN_DANGER,
            };
            let title = label(title_key);
            d.label_left(
                canvas_ui::geometry::UiRect::new(
                    row.slot.x,
                    row.slot.y + 8.0,
                    crate::kit_ui::STATE_LABEL_W,
                    16.0,
                ),
                &title,
                palette.text_muted,
                11.0,
            );
            for (i, br) in row.buttons.iter().enumerate() {
                let state = match i {
                    0 => canvas_ui::kit::KitState::Normal,
                    1 => canvas_ui::kit::KitState::Hovered,
                    2 => canvas_ui::kit::KitState::Pressed,
                    _ => canvas_ui::kit::KitState::Disabled,
                };
                let style = canvas_ui::kit::button_style(row.variant, state, &palette);
                d.control(*br, &style);
                let text = crate::i18n::tr(lang, crate::kit_ui::STATE_LABELS[i]);
                d.label_center(*br, text, style.text, 13.0);
            }
        }
        // IconButtons: 4 состояния
        let icons = ["×", "•••", "?", "+"];
        for (i, r) in lay.icon_buttons.iter().enumerate() {
            let state = match i {
                0 => canvas_ui::kit::KitState::Normal,
                1 => canvas_ui::kit::KitState::Hovered,
                2 => canvas_ui::kit::KitState::Pressed,
                _ => canvas_ui::kit::KitState::Disabled,
            };
            let style = canvas_ui::kit::icon_button_style(state, &palette);
            d.control(*r, &style);
            let area = canvas_ui::geometry::UiRect::new(r.x, r.y + 1.0, r.w, r.h);
            d.label_center(area, icons[i], style.text, 13.0);
        }
        // Chips: Normal/Hover/Selected/Disabled
        for (i, r) in lay.chips.iter().enumerate() {
            let state = match i {
                0 => canvas_ui::kit::KitState::Normal,
                1 => canvas_ui::kit::KitState::Hovered,
                2 => canvas_ui::kit::KitState::Selected,
                _ => canvas_ui::kit::KitState::Disabled,
            };
            let style = canvas_ui::kit::chip_style(state, &palette);
            d.control(*r, &style);
            let text = crate::i18n::tr(lang, crate::kit_ui::STATE_LABELS[i]);
            d.label_center(*r, text, style.text, 12.0);
        }
        // Dropdown: якорь + открытое меню (3 строки, hover по курсору)
        {
            let style = canvas_ui::kit::button_style(
                canvas_ui::kit::ButtonVariant::Secondary,
                canvas_ui::kit::KitState::Normal,
                &palette,
            );
            d.control(lay.dropdown_anchor, &style);
            let text = label(crate::i18n::keys::KIT_DROPDOWN_ANCHOR);
            d.label_center(lay.dropdown_anchor, &text, style.text, 13.0);
            d.rect(
                lay.dropdown_menu,
                palette.panel_fill,
                palette.panel_border,
                canvas_core::tokens::RADIUS_CHIP,
            );
            for r in &lay.dropdown_items {
                let mut item_widget = WidgetState::default();
                item_widget.set_pointer(crate::kit_ui::cursor_in(r, cursor), false);
                let style = canvas_ui::kit::chip_style(item_widget.kit_state(), &palette);
                d.rect(*r, style.fill, [0.0; 4], 4.0);
                let text = label(crate::i18n::keys::KIT_DROPDOWN_ITEM);
                d.label_left(
                    canvas_ui::geometry::UiRect::new(r.x + 8.0, r.y + 5.0, r.w - 16.0, r.h - 6.0),
                    &text,
                    style.text,
                    12.0,
                );
            }
        }
        // Toast: строка-демо (kit Toast: TTL/зона — контракт кита)
        {
            let text = label(crate::i18n::keys::KIT_TOAST_BODY);
            d.label_left(lay.toast, &text, palette.text_muted, 12.0);
        }
        // Tooltip: якорь-чип + пузырь (delay пройден — показан)
        {
            let style = canvas_ui::kit::chip_style(canvas_ui::kit::KitState::Normal, &palette);
            d.control(lay.tooltip_anchor, &style);
            let text = label(crate::i18n::keys::KIT_TOOLTIP_ANCHOR);
            d.label_center(lay.tooltip_anchor, &text, style.text, 12.0);
            if !lay.tooltip.is_empty() {
                d.rect(
                    lay.tooltip,
                    panel_style.fill,
                    palette.accent,
                    canvas_core::tokens::RADIUS_CHIP,
                );
                let text = label(crate::i18n::keys::KIT_TOOLTIP_BODY);
                d.label_left(
                    canvas_ui::geometry::UiRect::new(
                        lay.tooltip.x + 8.0,
                        lay.tooltip.y + 2.0,
                        lay.tooltip.w - 16.0,
                        14.0,
                    ),
                    &text,
                    palette.text,
                    12.0,
                );
            }
        }

        // === FR-059: секции компонентов v2 (FR-058) ===
        // TextField: Normal / Focused (каретка) / Disabled + placeholder
        for (state, focused, field) in &lay.text_fields {
            let style = canvas_ui::kit::control_style_of(
                palette.control_fill,
                if *focused {
                    palette.accent
                } else {
                    palette.control_border
                },
                palette.text,
                canvas_core::tokens::RADIUS_CHIP,
            );
            d.control(field.rect, &style);
            let text_color = if *state == canvas_ui::kit::KitState::Disabled {
                palette.disabled_text
            } else {
                palette.text
            };
            d.label_left(field.text_area, &field.text_shown, text_color, 13.0);
            if *focused && field.caret_x >= 0.0 {
                // Каретка — рамка 1.5 px слотом accent (фокус-рамка FR-057)
                d.rect(
                    canvas_ui::geometry::UiRect::new(
                        field.caret_x,
                        field.text_area.y + 4.0,
                        1.5,
                        field.text_area.h - 8.0,
                    ),
                    palette.accent,
                    [0.0; 4],
                    0.0,
                );
            }
        }
        // Switch: Off/On × Normal/Hovered/Pressed/Disabled (kit::switch —
        // слоты) + состояние «в фокусе» (рамка accent — контракт FR-057)
        for (slot, on, state, focused) in &lay.switches {
            let sw = canvas_ui::kit::switch(*slot, *on, *state, &palette);
            d.control(sw.track, &sw.track_style);
            d.rect(sw.knob, sw.knob_fill, [0.0; 4], sw.track_style.radius / 2.0);
            if *focused {
                d.rect(
                    *slot,
                    [0.0; 4],
                    palette.accent,
                    canvas_core::tokens::RADIUS_CHIP,
                );
            }
        }
        // Card: хедер + тело внутри пада панели (kit::card)
        if let Some(card) = &lay.card {
            d.rect(
                card.rect,
                panel_style.fill,
                panel_style.border,
                panel_style.radius,
            );
            d.label_left(
                canvas_ui::geometry::UiRect::new(
                    card.header.x,
                    card.header.y + 2.0,
                    card.header.w,
                    16.0,
                ),
                &label(crate::i18n::keys::KIT_CARD_TITLE),
                palette.text_title,
                13.0,
            );
            d.label_left(
                canvas_ui::geometry::UiRect::new(card.body.x, card.body.y, card.body.w, 16.0),
                &label(crate::i18n::keys::KIT_CARD_BODY),
                palette.text_muted,
                12.0,
            );
        }
        // Список + скролл: строки с выделением + бегунок (kit::list_rows)
        if lay.list_area.w > 0.0 {
            for (index, row) in &lay.list_rows {
                let mut row_widget = WidgetState::default();
                row_widget.set_selected(*index == lay.list_selected);
                let style = canvas_ui::kit::chip_style(row_widget.kit_state(), &palette);
                d.rect(*row, style.fill, [0.0; 4], 4.0);
                let text = crate::i18n::trf(
                    lang,
                    crate::i18n::keys::KIT_LIST_ROW,
                    &[("{n}", &(index + 1).to_string())],
                );
                d.label_left(
                    canvas_ui::geometry::UiRect::new(
                        row.x + 8.0,
                        row.y + 4.0,
                        row.w - 16.0,
                        row.h - 6.0,
                    ),
                    &text,
                    style.text,
                    12.0,
                );
            }
            if let Some(knob) =
                canvas_ui::kit::scroll_bar(lay.list_area, &lay.list_scroll, &palette)
            {
                d.rect(knob, palette.control_border, [0.0; 4], 2.0);
            }
        }
        // Icon-глифы v2: Search/ArrowLeft/ArrowRight/Refresh (icon_glyph).
        // FR-ICONS: если активный набор — SVG, рисуется SVG-иконка из атласа
        // (tint = слот text контрола); иначе — глиф шрифтом (прежнее поведение).
        for (rect, icon) in &lay.icon_glyphs {
            let style =
                canvas_ui::kit::icon_button_style(canvas_ui::kit::KitState::Normal, &palette);
            d.control(*rect, &style);
            let area = canvas_ui::geometry::UiRect::new(rect.x, rect.y + 1.0, rect.w, rect.h);
            d.icon(
                area,
                canvas_ui::kit::icon_name(*icon),
                canvas_ui::kit::icon_glyph(*icon),
                style.text,
                13.0,
            );
        }
        // === FR-068 W3 (этап M3): секция Table — табличные строки на
        // общих направляющих (W3.3: единственная табличная секция витрины —
        // Row v1 заменена компонентом). Отрисовка — kit::paint_row (строка
        // целиком одним вызовом кита); зебра — демо слотом hover_fill;
        // состояния — row_style (Selected — selected_fill + accent).
        // M5 Table v2 (аудит, docs/plans/fr-068-table-v2.md §2/§8): здесь
        // НЕ кандидат Table::visible_rows — это paint-половина демо-таблицы
        // витрины. Геометрия/скролл (Table::set_rows/row_layout_with,
        // сдвиг на offset, фильтр ПОЛНОЙ видимости) — в
        // kit_ui::gallery_layout; сама
        // отрисовка уже полностью на kit-функциях по предвычисленным
        // раскладкам, а семантика видимости витрины (полная видимость +
        // сдвиг ячеек) отличается от клипа Table::visible_rows
        // (пересечение — усечение частичных строк): замена меняла бы
        // рендер краёв окна (I-1) без выигрыша — строки уже на своих
        // (общих) направляющих.
        for demo in &lay.row_rows {
            let mut style = canvas_ui::kit::row_style(demo.state, &palette);
            if demo.zebra {
                style.fill = palette.hover_fill;
            }
            let mut p = canvas_ui::paint::Painter::new();
            canvas_ui::kit::paint_row(
                &mut p,
                &demo.lay,
                &demo.parts,
                &style,
                crate::kit_ui::LABEL_SIZE,
            );
            d.paint_items(p.take_items());
        }
        // === FR-062: секции layout v2 (F-13…F-17) ===
        // Measured-ряд (F-13): чипы, ширины которых посчитал TextMeasurer
        // внутри Row::lay_out_measured (подписи — те же строки, что в замере)
        {
            let style = canvas_ui::kit::chip_style(canvas_ui::kit::KitState::Normal, &palette);
            let keys = [
                crate::i18n::keys::KIT_MEASURED_A,
                crate::i18n::keys::KIT_MEASURED_B,
                crate::i18n::keys::KIT_MEASURED_C,
            ];
            for (i, rect) in lay.measured_chips.iter().enumerate() {
                d.control(*rect, &style);
                if let Some(key) = keys.get(i) {
                    d.label_center(*rect, &label(key), style.text, 12.0);
                }
            }
        }
        // Flex-факторы (F-14): fixed + grow ×2 + grow ×1
        for (rect, key) in &lay.grow_cells {
            let style = canvas_ui::kit::button_style(
                canvas_ui::kit::ButtonVariant::Secondary,
                canvas_ui::kit::KitState::Normal,
                &palette,
            );
            d.control(*rect, &style);
            d.label_center(*rect, &label(key), style.text, 13.0);
        }
        // Wrap (F-15): жадная упаковка measured-чипов в строки слота
        for (rect, index) in &lay.wrap_chips {
            let style = canvas_ui::kit::chip_style(canvas_ui::kit::KitState::Normal, &palette);
            d.control(*rect, &style);
            let text = crate::i18n::trf(
                lang,
                crate::i18n::keys::KIT_WRAP_CHIP,
                &[("{n}", &(index + 1).to_string())],
            );
            d.label_center(*rect, &text, style.text, 12.0);
        }
        // Сетка (F-16): 4×2 равных колонок (grid_cells)
        for (i, rect) in lay.grid_cells.iter().enumerate() {
            d.rect(
                *rect,
                palette.control_fill,
                palette.control_border,
                canvas_core::tokens::RADIUS_CHIP,
            );
            let text = (i + 1).to_string();
            d.label_center(*rect, &text, palette.text, 11.0);
        }
        // Фокус (F-17): Tab-кольцо — рамка accent на текущем слоте
        // (кольцо в контент-координатах; слоты здесь уже сдвинуты на off).
        // Пересборка поверхностей: текущий индекс — kit::focus_order (кит-
        // функция сопоставления кольца с Tab-порядком) + нумерация порядка
        // (визуальный порядок Tab-кольца, digits — без i18n).
        let focus_idx = canvas_ui::kit::focus_order(&lay.focus_targets, &self.kit_gallery_focus)
            .map(|(i, _)| i);
        for (i, rect) in lay.focus_buttons.iter().enumerate() {
            let focused = focus_idx == Some(i);
            let style = canvas_ui::kit::control_style_of(
                palette.control_fill,
                if focused {
                    palette.accent
                } else {
                    palette.control_border
                },
                palette.text,
                canvas_core::tokens::RADIUS_CHIP,
            );
            d.control(*rect, &style);
            d.label_center(
                *rect,
                &(i + 1).to_string(),
                if focused {
                    palette.accent
                } else {
                    palette.text_muted
                },
                11.0,
            );
        }

        // === Пересборка поверхностей (волна 2): демо компонентного слоя и
        // недостающих layout-примитивов ===
        // Компонент Row (FR-068 W3): retained-компонент из Props раскладки —
        // layout_row_with + paint_with (§4.7, FR-094; строка рисуется китом);
        // нулевой слот — секция за краем окна (компонент не собирается).
        if lay.component_row_slot.w > 0.0 {
            // FR-094: продакшн-контракт §4.7 — пулы шейпинга ВНЕШНИЕ (общий
            // FontSystem рендера, как у Table: guides_with/row_layout_with).
            // Дефолтный Component::layout/paint ходит во внутренний пул Row
            // (FontSystem::new()) — в wasm он ПУСТ (системных шрифтов нет):
            // первый шейпинг секции падал паникой «no default font found»
            // (cosmic-text shape.rs:251) и ронял rAF-цикл — витрина
            // «замирала» на последнем кадре. Дефолтные пути Row — только
            // тесты/standalone (дока Row::layout_row_with).
            let row = canvas_ui::component::row::Row::new(lay.component_row_props.clone());
            let rects = canvas_ui::component::row::row_rects(&row.layout_row_with(
                &mut m,
                &mut fs,
                lay.component_row_slot,
            ));
            let mut painter = canvas_ui::paint::Painter::new();
            row.paint_with(&mut painter, &mut m, &mut fs, &rects);
            d.paint_items(painter.take_items());
        }
        // Компонент Panel (FR-068 W3): хром панели — слоты panel_style,
        // контент — подпись (panel_content — минус пад SPACING_LG).
        if let Some((prect, crect)) = &lay.component_panel {
            let ps = canvas_ui::kit::panel_style(&palette);
            d.rect(*prect, ps.fill, ps.border, ps.radius);
            d.label_left(
                canvas_ui::geometry::UiRect::new(crect.x, crect.y + 2.0, crect.w, 16.0),
                &label(crate::i18n::keys::KIT_COMPONENT_PANEL_BODY),
                palette.text_muted,
                12.0,
            );
        }
        // SqueezeTail: рамка слота + боксы; хвост сжат политикой (у нулевой
        // ширины не рисуется — именованная деградация вместо переполнения).
        if lay.squeeze_slot.w > 0.0 {
            d.rect(
                lay.squeeze_slot,
                [0.0; 4],
                palette.control_border,
                canvas_core::tokens::RADIUS_CHIP,
            );
            let n = lay.squeeze_cells.len();
            for (i, rect) in lay.squeeze_cells.iter().enumerate() {
                if rect.w < 0.5 {
                    continue;
                }
                let border = if i + 1 == n {
                    palette.accent
                } else {
                    palette.control_border
                };
                d.rect(
                    *rect,
                    palette.control_fill,
                    border,
                    canvas_core::tokens::RADIUS_CHIP,
                );
            }
        }
        // SpaceBetween: чипы с подписями — свободное место распределено
        // в зазоры (Fit-ряд с базовым gap для сравнения — секция Grow).
        for (rect, key) in &lay.align_between {
            let style = canvas_ui::kit::chip_style(canvas_ui::kit::KitState::Normal, &palette);
            d.control(*rect, &style);
            d.label_center(*rect, &label(key), style.text, 12.0);
        }
        // Column + распорка: ячейки вертикального стека с номерами;
        // распорка (нулевая ширина) — не рисуется.
        for (i, rect) in lay.column_cells.iter().enumerate() {
            if rect.w < 0.5 {
                continue;
            }
            d.rect(
                *rect,
                palette.control_fill,
                palette.control_border,
                canvas_core::tokens::RADIUS_CHIP,
            );
            d.label_center(*rect, &(i + 1).to_string(), palette.text, 11.0);
        }
        // FR-059: бегунок скролла контента витрины (контент выше панели)
        if let Some(knob) = canvas_ui::kit::scroll_bar(lay.sections_viewport, &scroll, &palette) {
            d.rect(knob, palette.control_border, [0.0; 4], 2.0);
        }

        // Конвертация владеемых текстов адаптера в OwnedScreenText кадра
        out.0 = d.quads;
        out.1 = d
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
        // FR-ICONS: иконки текущего кадра (SVG-атлас; пусто для Glyph).
        out.2 = d.icons;
        out
    }

    /// Отрисовка галереи схем (screen-space): панель, шапка, фильтр, чипы,
    /// строки схем, футер-подсказка. Цвета — слоты ThemeColors.
    pub(super) fn scheme_gallery_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (instances, texts);
        }
        let palette = self.effective_palette();
        let registry = canvas_core::schemes::SchemeRegistry::embedded();
        // FR-087: строки и чипы галереи — видимые категории (роль/фильтр).
        let list = scheme_gallery_ui::rows(
            registry,
            &self.scheme_gallery,
            &self.visible_scheme_categories(),
        );
        let lay = scheme_gallery_ui::layout(
            viewport,
            &list,
            &self.scheme_gallery,
            &self.visible_scheme_categories(),
        );
        // FR-053 (U3): измеренные подписи строк (Ellipsis по фактической
        // ширине строки) — раскладка и отрисовка используют одни строки.
        let ru = self.settings.language == canvas_core::Language::Ru;
        let mut measurer = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        let row_labels = scheme_gallery_ui::row_labels(
            viewport,
            &list,
            &self.scheme_gallery,
            ru,
            &mut measurer,
            &mut fs,
            &self.visible_scheme_categories(),
        );
        // Подложка панели
        instances.push(CardInstance {
            pos: [lay.panel_rect[0], lay.panel_rect[1]],
            size: [lay.panel_rect[2], lay.panel_rect[3]],
            fill: palette.menu_fill,
            border: palette.palette_border,
            params: [10.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        // Шапка + счётчик
        texts.push(OwnedScreenText {
            text: self.tr(keys::GALLERY_TITLE).to_owned(),
            origin: [lay.header_rect[0], lay.header_rect[1] + 8.0],
            width: lay.header_rect[2] - 40.0,
            font_size: 14.0,
            color: palette.title,
            align: TextAlign::Left,
        });
        texts.push(OwnedScreenText {
            text: format!("{}", list.len()),
            origin: [
                lay.header_rect[0] + lay.header_rect[2] - 36.0,
                lay.header_rect[1] + 10.0,
            ],
            width: 30.0,
            font_size: 11.0,
            color: palette.body,
            align: TextAlign::Center,
        });
        // Кнопка закрытия «×»
        // FR-055 (этап U4): стиль — через кит (kit::control_style_of: явные
        // слоты palette_chip_fill + title, RADIUS_CHIP — совпадает с прежним
        // литеральным quad'ом; I-1 ноль скачка)
        let close_style = canvas_ui::kit::control_style_of(
            palette.palette_chip_fill,
            [0.0; 4],
            [0.0; 4],
            canvas_core::tokens::RADIUS_CHIP,
        );
        instances.push(CardInstance {
            pos: [lay.close_rect[0], lay.close_rect[1]],
            size: [lay.close_rect[2], lay.close_rect[3]],
            fill: close_style.fill,
            border: close_style.border,
            params: [close_style.radius, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        texts.push(OwnedScreenText {
            text: "×".to_owned(),
            origin: [lay.close_rect[0], lay.close_rect[1] + 3.0],
            width: lay.close_rect[2],
            font_size: 13.0,
            color: palette.title,
            align: TextAlign::Center,
        });
        // Поле фильтра
        instances.push(CardInstance {
            pos: [lay.input_rect[0], lay.input_rect[1]],
            size: [lay.input_rect[2], lay.input_rect[3]],
            fill: palette.search_input_fill,
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        texts.push(OwnedScreenText {
            text: if self.scheme_gallery.filter.is_empty() {
                self.tr(keys::GALLERY_SEARCH).to_owned()
            } else {
                format!("{}|", self.scheme_gallery.filter)
            },
            origin: [lay.input_rect[0] + 8.0, lay.input_rect[1] + 8.0],
            width: lay.input_rect[2] - 16.0,
            font_size: 12.0,
            color: palette.body,
            align: TextAlign::Left,
        });
        // Чипы категорий («Все» + уникальные категории реестра)
        for (rect, category) in &lay.chip_rects {
            let active = self.scheme_gallery.category == *category;
            instances.push(CardInstance {
                pos: [rect[0], rect[1]],
                size: [rect[2], rect[3]],
                fill: if active {
                    palette.palette_chip_fill
                } else {
                    palette.menu_fill
                },
                border: palette.palette_border,
                params: [12.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            let label = match category {
                None => self.tr(keys::GALLERY_ALL).to_owned(),
                Some(key) => {
                    let manifest = registry.list().iter().find(|s| &s.category == key);
                    match manifest {
                        Some(m) => {
                            if ru {
                                m.category_ru.clone()
                            } else {
                                m.category_en.clone()
                            }
                        }
                        None => key.clone(),
                    }
                }
            };
            texts.push(OwnedScreenText {
                text: label,
                origin: [rect[0] + 8.0, rect[1] + 6.0],
                width: rect[2] - 16.0,
                font_size: 11.0,
                color: palette.title,
                align: TextAlign::Left,
            });
        }
        // Строки схем (окно видимости)
        let hovered = scheme_gallery_ui::row_at(&lay, self.cursor);
        for (label, (rect, index)) in row_labels
            .iter()
            .zip(lay.row_rects.iter().zip(lay.visible_rows.iter()))
        {
            let Some(scheme) = list.get(*index) else {
                continue;
            };
            let is_selected = *index == self.scheme_gallery.selected;
            instances.push(CardInstance {
                pos: [rect[0], rect[1]],
                size: [rect[2], rect[3]],
                fill: if is_selected || hovered == Some(*index) {
                    // FR-053 (U3 F-9): selected/hover строки — слоты темы
                    // (бывший hover_fill(palette.menu_fill); сегодня значения
                    // совпадают — ноль скачка, семантика разделена слотами).
                    if is_selected {
                        palette.control_selected_fill
                    } else {
                        palette.control_hover_fill
                    }
                } else {
                    palette.menu_fill
                },
                border: palette.palette_border,
                params: [8.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            texts.push(OwnedScreenText {
                // FR-053 (U3): метка из раскладки — измеренный Ellipsis.
                text: label.title.clone(),
                origin: [rect[0] + 10.0, rect[1] + 7.0],
                width: rect[2] - 20.0,
                font_size: 13.0,
                color: palette.title,
                align: TextAlign::Left,
            });
            texts.push(OwnedScreenText {
                text: label.desc.clone(),
                origin: [rect[0] + 10.0, rect[1] + 26.0],
                width: rect[2] - 20.0,
                font_size: 11.0,
                color: palette.body,
                align: TextAlign::Left,
            });
            // Фикс 2026-09-25 (wasm-аудит 17_gallery): мета «нод: N, связей: M»
            // прижималась к нижней кромке карточки (y+44 при inner 48) —
            // ROW_H 56 → 62, мета на y+40 с полным нижним полем.
            texts.push(OwnedScreenText {
                text: i18n::trf(
                    self.settings.language,
                    keys::GALLERY_META,
                    &[
                        ("nodes", scheme.content.nodes.len().to_string().as_str()),
                        ("edges", scheme.content.edges.len().to_string().as_str()),
                    ],
                ),
                origin: [rect[0] + 10.0, rect[1] + 40.0],
                width: rect[2] - 20.0,
                font_size: 10.0,
                color: palette.body,
                align: TextAlign::Left,
            });
        }
        // Футер-подсказка
        texts.push(OwnedScreenText {
            text: self.tr(keys::GALLERY_FOOTER).to_owned(),
            origin: [lay.footer_rect[0], lay.footer_rect[1] + 5.0],
            width: lay.footer_rect[2],
            font_size: 10.0,
            color: palette.body,
            align: TextAlign::Left,
        });
        (instances, texts)
    }

    /// Отрисовка empty-state пустого канваса (US-1): карточка с одной
    /// главной кнопкой и альтернативой «Пустой холст».
    pub(super) fn empty_state_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (instances, texts);
        }
        let palette = self.effective_palette();
        let card = scheme_gallery_ui::empty_card_rect(viewport);
        instances.push(CardInstance {
            pos: [card[0], card[1]],
            size: [card[2], card[3]],
            fill: palette.menu_fill,
            border: palette.palette_border,
            params: [10.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        texts.push(OwnedScreenText {
            text: self.tr(keys::GALLERY_EMPTY_TITLE).to_owned(),
            origin: [card[0] + 20.0, card[1] + 18.0],
            width: card[2] - 40.0,
            font_size: 16.0,
            color: palette.title,
            align: TextAlign::Left,
        });
        // Фикс среза 2026-09-25 (wasm-аудит 01_idle): подзаголовок
        // «Готовые схемы со связями и расчётами: …» рвался кромкой
        // карточки без переноса — до 2 строк тем же кеглем. Дефект №15
        // (закрыт W-f, 04.10.2026): строки сверх двух НЕ теряются молча —
        // на последней показанной строке индикатор «ещё N» (i18n).
        {
            let body = self.tr(keys::GALLERY_EMPTY_BODY);
            let mut m = canvas_ui::measure::TextMeasurer::new();
            let mut fs = canvas_render::text::measure_font_system();
            let body_w = (card[2] - 40.0).max(10.0);
            for (line_idx, line) in wrap_lines_with_more(
                &mut m,
                &mut fs,
                self.settings.language,
                keys::GALLERY_EMPTY_MORE,
                body,
                12.0,
                body_w,
                2,
            )
            .into_iter()
            .enumerate()
            {
                texts.push(OwnedScreenText {
                    text: line,
                    origin: [card[0] + 20.0, card[1] + 46.0 + line_idx as f32 * 15.0],
                    width: body_w,
                    font_size: 12.0,
                    color: palette.body,
                    align: TextAlign::Left,
                });
            }
        }
        let (open_btn, dismiss_btn) = scheme_gallery_ui::empty_buttons(card);
        for (rect, label_key, accent) in [
            (open_btn, keys::GALLERY_EMPTY_OPEN, true),
            (dismiss_btn, keys::GALLERY_EMPTY_DISMISS, false),
        ] {
            let hovered = scheme_gallery_ui::point_in_rect(rect, self.cursor);
            instances.push(CardInstance {
                pos: [rect[0], rect[1]],
                size: [rect[2], rect[3]],
                fill: if accent {
                    if hovered {
                        // FR-053 (U3 F-9): hover primary — слот темы (бывший
                        // hover_fill(DIALOG_BUTTON_PRIMARY), значение прежнее).
                        palette.control_primary_hover_fill
                    } else {
                        canvas_core::tokens::DIALOG_BUTTON_PRIMARY
                    }
                } else if hovered {
                    palette.control_hover_fill
                } else {
                    palette.menu_fill
                },
                border: palette.palette_border,
                params: [6.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            texts.push(OwnedScreenText {
                text: self.tr(label_key).to_owned(),
                origin: [rect[0], rect[1] + 9.0],
                width: rect[2],
                font_size: 12.0,
                color: palette.title,
                align: TextAlign::Center,
            });
        }
        (instances, texts)
    }

    /// Popup открывается только на Numi-строках каретки (вердикт
    /// `expr::line_kind`, вне код-фенсов) при непустом списке вариантов;
    /// якорь — низ каретки в логических px окна.
    /// FR-079 (S3): здесь же триггер C1 — ДО Numi-гейта L0: «пользователь
    /// печатает имя шаблона» не является Numi-выражением, ИИ-строки живут
    /// в том же попапе на своих правах (AI-only-открытие — по SuggestReady).
    /// Текст изменился (префикс новый) → поколение растёт, устаревшие
    /// ответы тонут; каретка двигалась без правки — поколение стабильно,
    /// ИИ-строки переживают пересинхронизацию L0 без мигания.
    pub(super) fn update_hints(&mut self) {
        let Some(session) = self.editing.as_ref() else {
            self.hints.reset();
            self.suggest.pending = None;
            return;
        };
        // Подсказки — только в тексте ноды (лейблы связей не Numi-редактор)
        if session.node_index().is_none() {
            self.hints.reset();
            self.suggest.pending = None;
            return;
        }
        let (line_i, line_text, caret) = session.caret_line();
        let prefix = &line_text[..caret.min(line_text.len())];
        // Код-фенсы выше строки каретки (``` toggling, как eval_lines)
        let full_text = session.text();
        let in_fence = full_text
            .split('\n')
            .take(line_i)
            .fold(false, |fence, line| {
                fence ^ line.trim_start().starts_with("```")
            });
        // FR-079 (S3): триггер C1 — любой ввод в тексте ноды вне фенсов
        // (включая прозу — это и есть сценарий «печатаю имя шаблона»);
        // выключенный движок гасит и накопленные ответы
        if self.suggest_active() && !in_fence {
            if prefix != self.suggest.last_prefix {
                self.suggest.generation += 1;
                self.suggest.last_prefix = prefix.to_owned();
            }
            self.suggest.pending = Some(suggest::PendingSuggest {
                // W1 (S3-fix): alias canvas_core::time — std::time::Instant
                // на wasm32 паникует («time not implemented» → фриз web)
                due: Instant::now(),
            });
        } else {
            self.suggest.pending = None;
            self.suggest.answers.clear();
        }
        // L0 (FR-021): Numi-строки каретки — прежний гейт попапа
        if in_fence
            || !matches!(
                expr::line_kind(prefix),
                expr::NumiLineKind::Assignment { .. } | expr::NumiLineKind::Expression
            )
        {
            self.hints.reset();
            // Актуальные ИИ-строки переживают закрытие L0 (AI-only-попап)
            self.suggest_remerge();
            return;
        }
        // Контекст ноды: переменные выше, value-входы, параметры шаблона
        let vars: Vec<String> = full_text
            .split('\n')
            .take(line_i)
            .filter_map(|line| match expr::line_kind(line) {
                expr::NumiLineKind::Assignment { name } => Some(name),
                _ => None,
            })
            .collect();
        let ctx = hints_ui::HintContext {
            vars,
            inbound: self
                .scene
                .canvas
                .edges
                .iter()
                .filter(|edge| {
                    edge.to_node
                        == self
                            .scene
                            .canvas
                            .nodes
                            .get(session.node_index().unwrap_or(usize::MAX))
                            .map(|node| node.id.clone())
                            .unwrap_or_default()
                        && edge.flow_kind() == FlowKind::Value
                })
                .count(),
            params: self
                .scene
                .canvas
                .nodes
                .get(session.node_index().unwrap_or(usize::MAX))
                .and_then(|node| node.template())
                .map(|template| template.params.keys().cloned().collect())
                .unwrap_or_default(),
        };
        let items = hints_ui::hint_items(prefix, &ctx, self.settings.language);
        let token = hints_ui::token_before_caret(prefix, prefix.len()).0;
        self.hints.sync(token, items);
        // FR-079 (S3): мердж актуальных ИИ-строк под L0 (триггер — выше)
        self.suggest_remerge();
        // Якорь — низ каретки (screen logical px) — см. sync_hints_anchor.
        self.sync_hints_anchor();
    }

    /// Волна 1 (п.5, H4): якорь popup подсказок — НИЗ КАРЕТКИ в логических
    /// px окна — пересчитывается КАЖДЫЙ кадр, пока popup открыт. Раньше
    /// якорь обновлялся только при изменении текста (update_hints): при
    /// зуме/панораме popup оставался на старом месте экрана и отрывался
    /// от каретки. Математика: caret rect — px буфера редактора (буфер
    /// рисуется 1:1 в физических px у origin области тела), /scale —
    /// перевод в логические px.
    /// FR-079 (S3): якорь живёт и при закрытом L0-попапе — AI-only-открытие
    /// по SuggestReady (пока взведён debounce-триггер C1).
    pub(super) fn sync_hints_anchor(&mut self) {
        if !self.hints.open && self.suggest.pending.is_none() {
            return;
        }
        let caret_rect = if let (Some(session), Some(renderer)) =
            (self.editing.as_mut(), self.renderer.as_mut())
        {
            session.caret_rect(renderer.font_system_mut())
        } else {
            None
        };
        if let (Some(session), Some(rect)) = (self.editing.as_ref(), caret_rect) {
            if let Some((origin, _, _)) =
                session_area_offset(&self.scene.canvas, session, self.settings.edges_avoid_nodes)
            {
                let screen = self.camera.world_to_screen(origin, self.viewport_logical());
                let scale = self.scale_factor();
                self.hints.anchor = [
                    screen[0] + rect[0] / scale,
                    screen[1] + (rect[1] + rect[3]) / scale,
                ];
            }
        }
    }

    /// FR-021: оверлей popup подсказок — подложка + строки
    /// (имя + серая деталь), выделение акцентом. Паттерн wheel_overlay.
    /// FR-059 (волна 1 кита): геометрия — `hints_ui::popup_rect`/
    /// `hint_rows` (`kit::dropdown_menu` + `kit::list_rows`); отрисовка —
    /// через [`Painter`] (items → полоса, `paint_items_to_band`);
    /// выделение строки — [`WidgetState`] → `KitState::Selected`
    /// (цвет подсветки — прежний, 0 визуального скачка).
    pub(super) fn hints_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        if !self.hints.open || self.hints.items.is_empty() {
            return (instances, texts);
        }
        let palette = self.effective_palette();
        let viewport = self.viewport_logical();
        let Some(popup) = hints_ui::popup_rect(self.hints.anchor, viewport, self.hints.items.len())
        else {
            return (instances, texts);
        };
        let mut d = Painter::new();
        let kit_palette = palette.kit_palette();
        // Подложка popup — прежние слоты дословно (заливка/рамка/радиус 6)
        d.rect(popup, palette.menu_fill, palette.palette_border, 6.0);
        for (index, row) in hints_ui::hint_rows(popup, self.hints.items.len()) {
            let Some(item) = self.hints.items.get(index) else {
                continue;
            };
            // Состояние строки — машина состояний виджета (FR-057):
            // клавиатурная селекция → Selected
            let mut state = WidgetState::default();
            state.set_selected(index == self.hints.selected);
            if state.kit_state() == canvas_ui::kit::KitState::Selected {
                d.rect(row, palette.control_selected_fill, [0.0; 4], 4.0);
            }
            // Подписи — прежние смещения/кегли/цвета дословно (слоты кита:
            // title → text_title, body → text)
            d.label(
                canvas_ui::geometry::UiRect::new(
                    popup.x + 10.0,
                    row.y + 4.0,
                    118.0,
                    hints_ui::HINT_ROW_H,
                ),
                &item.label,
                kit_palette.text_title,
                12.0,
                PaintAlign::Left,
            );
            d.label(
                canvas_ui::geometry::UiRect::new(
                    popup.x + 134.0,
                    row.y + 6.0,
                    (popup.w - 142.0).max(20.0),
                    hints_ui::HINT_ROW_H,
                ),
                &item.detail,
                kit_palette.text,
                10.0,
                PaintAlign::Left,
            );
        }
        paint_items_to_band(d.take_items(), &mut instances, &mut texts);
        (instances, texts)
    }

    /// FR-079 (S3): C3-карточки «следующие ноды» — стопка чипов у правого
    /// края свежей шаблонной ноды (паттерн whatif-чипа: подложка меню-тона,
    /// рамка палитры, радиус кита; «✦» — маркер ИИ-источника). Геометрия —
    /// [`App::suggest_card_rects`] (тот же источник, что у hit-теста).
    pub(super) fn suggest_cards_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let Some(cards) = self.suggest.cards.as_ref() else {
            return (instances, texts);
        };
        let palette = self.effective_palette();
        // FR-079 follow-up: empty-state — предложений нет, рисуем
        // тултип «AI дополнений нет» (маленький чип, не призрак ноды).
        if cards.empty {
            let Some(node_screen) = self.suggest_anchor_screen_rect() else {
                return (instances, texts);
            };
            let viewport = self.viewport_logical();
            if let Some(rect) = suggest::empty_tooltip_rect(node_screen, viewport) {
                instances.push(CardInstance {
                    pos: [rect.x, rect.y],
                    size: [rect.w, rect.h],
                    fill: palette.menu_fill,
                    border: palette.palette_border,
                    params: [6.0, 0.0, 0.0, 1.0],
                    corners: [0.0; 4],
                });
                let label =
                    crate::i18n::tr(self.settings.language, crate::i18n::keys::SUGGEST_EMPTY)
                        .to_owned();
                texts.push(OwnedScreenText {
                    text: label,
                    origin: [rect.x + 10.0, rect.y + 8.0],
                    width: rect.w - 16.0,
                    font_size: 12.0,
                    color: palette.body,
                    align: TextAlign::Left,
                });
            }
            return (instances, texts);
        }
        if cards.items.is_empty() {
            return (instances, texts);
        }
        // FR-079 redesign: призрачные ноды (не чипы). card_fill + card_edge
        // с alpha × SUGGEST_GHOST_ALPHA — пользователь видит «ноду, которую
        // предлагают», а не абстрактный чип. Шапка (имя шаблона) + тело
        // (категория). Hover — alpha повышается (+0.2).
        let rects = self.suggest_card_rects();
        let hovered = self.suggest_card_hover();
        let alpha = suggest::SUGGEST_GHOST_ALPHA;
        for (i, rect) in rects.iter().enumerate() {
            let is_hover = hovered == Some(i);
            let fa = if is_hover {
                (alpha + 0.2).min(1.0)
            } else {
                alpha
            };
            instances.push(CardInstance {
                pos: [rect.x, rect.y],
                size: [rect.w, rect.h],
                fill: [
                    palette.card_fill[0],
                    palette.card_fill[1],
                    palette.card_fill[2],
                    palette.card_fill[3] * fa,
                ],
                border: [
                    palette.card_edge[0],
                    palette.card_edge[1],
                    palette.card_edge[2],
                    palette.card_edge[3] * fa,
                ],
                params: [6.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            // Шапка: «✦ ИмяШаблона»
            texts.push(OwnedScreenText {
                text: format!("✦ {}", cards.items[i].label),
                origin: [rect.x + 12.0, rect.y + 8.0],
                width: rect.w - 20.0,
                font_size: 14.0,
                color: palette.title,
                align: TextAlign::Left,
            });
            // Тело: категория шаблона (приглушённым тоном)
            let body = self
                .suggest
                .catalog
                .as_ref()
                .and_then(|c| {
                    c.templates
                        .iter()
                        .find(|t| t.key == cards.items[i].template_key)
                })
                .map(|t| t.category.clone())
                .unwrap_or_default();
            if !body.is_empty() {
                texts.push(OwnedScreenText {
                    text: body,
                    origin: [rect.x + 12.0, rect.y + 40.0],
                    width: rect.w - 20.0,
                    font_size: 12.0,
                    color: palette.body,
                    align: TextAlign::Left,
                });
            }
        }
        (instances, texts)
    }

    /// Клавиатура палитры шаблонов в фокусе (Ctrl+P/клик по поиску):
    /// ввод фильтра, стрелки/Enter/Esc. Вызывается из on_key, когда панель
    /// развёрнута и сфокусирована. true — клавиша потреблена панелью.
    pub(super) fn on_template_panel_key(&mut self, event: &KeyEvent) -> bool {
        if event.state != ElementState::Pressed {
            return true; // отпускания глотаются — канвасу не достаются
        }
        // Ctrl+P не глотаем: on_key ниже фокусирует поиск развёрнутого
        // дока (FR-025) или разворачивает свёрнутый
        if self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("p") || c.eq_ignore_ascii_case("з"))
        {
            return false;
        }
        if event.logical_key == Key::Named(NamedKey::Escape) && !event.repeat {
            // FR-025: Esc сворачивает постоянный док (не закрывает модал —
            // док не модален; повторный Ctrl+P или ручка слева развернут)
            self.template_panel.close();
            self.persist_palette_dock();
            self.request_redraw();
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::Enter) && !event.repeat {
            let rows = template_panel_rows(
                &self.templates,
                &self.template_panel,
                self.settings.language,
                &self.template_category_names(),
            );
            // FR-024: выделение — ординал среди строк-шаблонов (секции —
            // заголовки, не цели)
            if let Some(row_idx) = template_row_of_ordinal(&rows, self.template_panel.selected) {
                if let template_ui::PanelRow::Template(index) = rows[row_idx] {
                    let manifest = self.templates.list()[index].clone();
                    let center = self.viewport_center_world();
                    // FR-025: док остаётся развёрнут — только фокус снят
                    self.template_panel.unfocus();
                    self.instantiate_template_at(&manifest, center);
                    self.request_redraw();
                }
            }
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::ArrowDown) && !event.repeat {
            let rows = template_panel_rows(
                &self.templates,
                &self.template_panel,
                self.settings.language,
                &self.template_category_names(),
            );
            // W-e: следование выделения — по измеренной высоте окна строк
            // (та же panel_layout, что у рисования/hit-теста)
            let rows_area_h = self.template_rows_area_h(&rows);
            self.template_panel.move_selection(1, &rows, rows_area_h);
            self.request_redraw();
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::ArrowUp) && !event.repeat {
            let rows = template_panel_rows(
                &self.templates,
                &self.template_panel,
                self.settings.language,
                &self.template_category_names(),
            );
            let rows_area_h = self.template_rows_area_h(&rows);
            self.template_panel.move_selection(-1, &rows, rows_area_h);
            self.request_redraw();
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::Backspace) && !event.repeat {
            self.template_panel.backspace();
            self.template_panel.selected = 0;
            self.template_panel.scroll_top = 0;
            self.request_redraw();
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::ArrowLeft) && !event.repeat {
            self.template_panel.move_left();
            self.request_redraw();
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::ArrowRight) && !event.repeat {
            self.template_panel.move_right();
            self.request_redraw();
            return true;
        }
        // Печатаемый символ (включая кириллицу — logical_key уже раскладка)
        if let Key::Character(text) = &event.logical_key {
            if !event.repeat && !self.modifiers.control_key() {
                self.template_panel.insert_str(text.as_str());
                self.template_panel.selected = 0;
                self.template_panel.scroll_top = 0;
                self.request_redraw();
                return true;
            }
        }
        true
    }

    /// W-e: измеренная высота окна строк развёрнутого дока (`rows_area`
    /// из `template_panel_layout`) — для клавиатурного следования
    /// выделения (`TemplatePanel::move_selection`) без дублирования
    /// геометрии: та же раскладка, что у рисования и hit-теста.
    fn template_rows_area_h(&self, rows: &[template_ui::PanelRow]) -> f32 {
        let viewport = self.viewport_logical();
        // FR-054: ширины чипов — измеренные (measurer на вызов, паттерн U3).
        let mut measurer = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        template_panel_layout(
            viewport[0],
            viewport[1],
            &self.templates,
            &self.template_panel,
            rows,
            &mut measurer,
            &mut fs,
            &self.template_category_names(),
        )
        .rows_area[3]
    }

    /// Оверлей палитры шаблонов (FR-018, Ctrl+P; FR-024 — стиль Miro
    /// Template picker; FR-025 — постоянная палитра: ПРИМАРНО свёрнутая
    /// вертикальная полоса категорий по центру слева, hover раскрывает
    /// flyout справа; развёрнутый док по Ctrl+P; ghost-превью drag):
    /// свёрнутый режим — [`template_strip_overlay`]; развёрнутый — левый док
    /// во всю высоту (чистая геометрия — `template_ui::panel_layout`), шапка
    /// «Шаблоны», поиск с placeholder, чипы категорий, секции с заголовками,
    /// строки-карточки, hover/выбранное состояние, футер-подсказка.
    pub(super) fn template_panel_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (instances, texts);
        }
        let palette = self.effective_palette();
        let icon_tint = color_to_rgba(palette.icon);
        // FR-025 (ревизия): свёрнутый режим — полоса категорий + flyout
        if !self.template_panel.open {
            let (mut strip_instances, mut strip_texts) =
                self.template_strip_overlay(viewport, &palette);
            instances.append(&mut strip_instances);
            texts.append(&mut strip_texts);
        } else {
            let rows = template_panel_rows(
                &self.templates,
                &self.template_panel,
                self.settings.language,
                &self.template_category_names(),
            );
            // FR-054: ширины чипов — измеренные (measurer на вызов, паттерн U3).
            let mut measurer = canvas_ui::measure::TextMeasurer::new();
            let mut fs = canvas_render::text::measure_font_system();
            let lay = template_panel_layout(
                viewport[0],
                viewport[1],
                &self.templates,
                &self.template_panel,
                &rows,
                &mut measurer,
                &mut fs,
                &self.template_category_names(),
            );
            let panel = rect_xywh(lay.panel_rect);
            // Подложка дока: плотная, с рамкой (отделяет панель от канваса).
            // CR-011: рамка палитурная (была захардкожена тёмной — ломала
            // светлую тему).
            instances.push(CardInstance {
                pos: [panel[0], panel[1]],
                size: [panel[2], panel[3]],
                fill: palette.menu_fill,
                border: palette.palette_border,
                params: [8.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            // Шапка: название + счётчик шаблонов
            let total = template_ui::template_row_count(&rows);
            texts.push(OwnedScreenText {
                text: self.tr(keys::TEMPLATES_TITLE).to_owned(),
                origin: [lay.header_rect[0], lay.header_rect[1] + 6.0],
                width: lay.header_rect[2] * 0.5,
                font_size: 14.0,
                color: palette.title,
                align: TextAlign::Left,
            });
            texts.push(OwnedScreenText {
                text: format!("{total}"),
                origin: [
                    lay.header_rect[0] + lay.header_rect[2] * 0.5,
                    lay.header_rect[1] + 8.0,
                ],
                width: lay.header_rect[2] * 0.5 - 4.0,
                font_size: 11.0,
                color: palette.body,
                align: TextAlign::Center,
            });
            // FR-025: кнопка сворачивания дока («‹» у правого края шапки)
            let collapse = rect_xywh(lay.collapse_rect);
            instances.push(CardInstance {
                pos: [collapse[0], collapse[1]],
                size: [collapse[2], collapse[3]],
                fill: palette.palette_chip_fill,
                border: [0.0; 4],
                params: [6.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            texts.push(OwnedScreenText {
                text: "‹".to_owned(),
                origin: [collapse[0], collapse[1] + 3.0],
                width: collapse[2],
                font_size: 13.0,
                color: palette.title,
                align: TextAlign::Center,
            });
            // Поле фильтра: placeholder при пустом вводе, иначе текст с кареткой
            let input = rect_xywh(lay.input_rect);
            instances.push(CardInstance {
                pos: [input[0], input[1]],
                size: [input[2], input[3]],
                fill: palette.search_input_fill,
                border: [0.0; 4],
                params: [6.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            texts.push(OwnedScreenText {
                text: if self.template_panel.filter.is_empty() {
                    self.tr(keys::TEMPLATES_SEARCH).to_owned()
                } else {
                    format!("{}|", self.template_panel.filter)
                },
                origin: [input[0] + 10.0, input[1] + 8.0],
                width: (input[2] - 20.0).max(10.0),
                font_size: 13.0,
                color: if self.template_panel.filter.is_empty() {
                    palette.body
                } else {
                    palette.title
                },
                align: TextAlign::Left,
            });
            // Чипы категорий (CR-011: заливки палитурные, не хардкод)
            for (rect, name, active) in &lay.category_rects {
                instances.push(CardInstance {
                    pos: [rect[0], rect[1]],
                    size: [rect[2], rect[3]],
                    fill: if *active {
                        palette.palette_selected_fill
                    } else {
                        palette.palette_chip_fill
                    },
                    border: [0.0; 4],
                    params: [11.0, 0.0, 0.0, 1.0],
                    corners: [0.0; 4],
                });
                texts.push(OwnedScreenText {
                    // FR-040 v2: чипы категорий — по языку приложения
                    // (raw-токен — только для поиска по реестру)
                    text: template_ui::category_display_name(self.settings.language, name),
                    origin: [rect[0] + 10.0, rect[1] + 6.0],
                    width: rect[2] - 12.0,
                    font_size: 12.0,
                    color: palette.title,
                    align: TextAlign::Left,
                });
            }
            // Строки: секции-заголовки и карточки шаблонов (Miro-стиль)
            for (row_i, (rect, row)) in lay.row_rects.iter().zip(lay.rows.iter()).enumerate() {
                match row {
                    PanelRow::Section(name) => {
                        texts.push(OwnedScreenText {
                            // FR-040 v2: секции — по языку приложения
                            text: template_ui::category_display_name(self.settings.language, name),
                            origin: [rect[0] + 2.0, rect[1] + 5.0],
                            width: rect[2] - 4.0,
                            font_size: 11.0,
                            color: palette.body,
                            align: TextAlign::Left,
                        });
                    }
                    PanelRow::Template(index) => {
                        let Some(manifest) = self.templates.list().get(*index) else {
                            continue;
                        };
                        // Ординал строки среди шаблонов (секции не считаются)
                        let ordinal = lay.rows[..row_i]
                            .iter()
                            .filter(|other| matches!(other, PanelRow::Template(_)))
                            .count();
                        let selected = self.template_panel.selected == ordinal;
                        let row_rect = rect_xywh(*rect);
                        let row_hover = point_in_rect(row_rect, self.cursor);
                        // Подложка-карточка строки (Miro: карточка с фоном;
                        // CR-011: заливки палитурные, не хардкод)
                        let fill = if selected {
                            palette.palette_selected_fill
                        } else if row_hover {
                            palette.palette_hover_fill
                        } else {
                            palette.palette_row_fill
                        };
                        let border = if selected || row_hover {
                            palette.palette_border
                        } else {
                            [0.0; 4]
                        };
                        template_card_row(
                            manifest,
                            row_rect,
                            fill,
                            border,
                            &palette,
                            icon_tint,
                            self.settings.language,
                            &mut instances,
                            &mut texts,
                            &mut measurer,
                            &mut fs,
                        );
                    }
                }
            }
            // Футер-подсказка (CR-011: позиция из footer_rect чистой геометрии —
            // строки списка в него не заходят; FR-025: Esc сворачивает док)
            let footer = rect_xywh(lay.footer_rect);
            texts.push(OwnedScreenText {
                text: self.tr(keys::TEMPLATES_FOOTER).to_owned(),
                origin: [footer[0], footer[1] + 7.0],
                width: footer[2],
                font_size: 10.0,
                color: palette.body,
                align: TextAlign::Left,
            });
            // W-e: бегунок окна строк — kit scroll_bar (образец FR-059/flowmap):
            // усечение списка показывается явно вместо прежнего молчаливого
            // кэпа MAX_VISIBLE_ROWS; цвет — слот рамки панелей (как у flowmap)
            let kit_palette = palette.kit_palette();
            if let Some(knob) = canvas_ui::kit::scroll_bar(
                canvas_ui::geometry::UiRect::new(
                    lay.rows_area[0],
                    lay.rows_area[1],
                    lay.rows_area[2],
                    lay.rows_area[3],
                ),
                &lay.scroll,
                &kit_palette,
            ) {
                instances.push(CardInstance {
                    pos: [knob.x, knob.y],
                    size: [knob.w, knob.h],
                    fill: palette.palette_border,
                    border: [0.0; 4],
                    params: [2.0, 0.0, 0.0, 1.0],
                    corners: [0.0; 4],
                });
            }
        } // else: развёрнутый док
          // FR-025: ghost-превью drag карточки шаблона — призрак дропа
          // (Т9) в world-точке курсора, отрисованный screen-space поверх
        if let Some(drag) = &self.template_drag {
            if drag.active {
                if let Some(manifest) = self.templates.list().get(drag.index) {
                    if let Ok(mut preview) = canvas_core::templates::instantiate_with_language(
                        manifest,
                        &BTreeMap::new(),
                        "preview".to_owned(),
                        0.0,
                        0.0,
                        self.settings.language,
                    ) {
                        fit_template_node_height(&mut preview);
                        let world = self.cursor_world();
                        let top_left = [
                            world[0] - preview.width / 2.0,
                            world[1] - preview.height / 2.0,
                        ];
                        let ghost = drop_ghost(top_left, [preview.width, preview.height]);
                        let zoom = self.camera.zoom();
                        instances.push(CardInstance {
                            pos: self.camera.world_to_screen(top_left, viewport),
                            size: [ghost.size[0] * zoom, ghost.size[1] * zoom],
                            fill: ghost.fill,
                            border: ghost.border,
                            params: ghost.params,
                            corners: [0.0; 4],
                        });
                    }
                }
            }
        }
        (instances, texts)
    }

    /// Оверлей свёрнутой палитры (ревизия FR-025, 2026-09-16): вертикальная
    /// полоса категорий по центру левого края (подложка + строки с именем и
    /// счётчиком + шеврон «развернуть док» внизу) и flyout раскрытой
    /// категории справа — строки шаблонов тем же карточным рендером, что и
    /// развёрнутый док, плюс индикаторы прокрутки при переполнении.
    pub(super) fn template_strip_overlay(
        &self,
        viewport: Vec2,
        palette: &ThemeColors,
    ) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let raw_categories = self.template_category_names();
        // FR-040 v2: подписи строк дока — по языку приложения; геометрия
        // мерится по ТЕМ ЖЕ локализованным строкам, что и hit-test
        // (единая точка template_category_display_names — расхождений нет).
        let categories = self.template_category_display_names();
        // FR-054: ширины чипов — измеренные (measurer на вызов, паттерн U3).
        let mut measurer = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        let strip =
            template_ui::dock_strip_layout(&categories, viewport[1], &mut measurer, &mut fs);
        let icon_tint = color_to_rgba(palette.icon);
        let open_category = self.template_hover.as_ref().and_then(|h| h.open);
        // W-c (аудит ui-kit §10): хром полосы/строк/flyout — kit-стили
        // (ControlStyle из СЛОТОВ темы — контракт F-8; слоты palette_* —
        // источник вида строк шаблонов, U5 «ноль визуального скачка»,
        // паттерн пилота whatif_overlay: panel_style_of/control_style_of).
        // Радиусы — шкала токенов (RADIUS_CHIP = прежний литерал 6).
        let strip_panel_style = canvas_ui::kit::panel_style_of(
            palette.menu_fill,
            palette.palette_border,
            8.0,
            canvas_core::tokens::SPACING_MD,
        );
        let row_style = |state: canvas_ui::kit::KitState| {
            // Состояния строки: hover/раскрытая категория — прежняя пара
            // palette_hover_fill + рамка palette_border; обычная — чип.
            match state {
                canvas_ui::kit::KitState::Hovered
                | canvas_ui::kit::KitState::Pressed
                | canvas_ui::kit::KitState::Selected => canvas_ui::kit::control_style_of(
                    palette.palette_hover_fill,
                    palette.palette_border,
                    color_to_rgba(palette.title),
                    canvas_core::tokens::RADIUS_CHIP,
                ),
                _ => canvas_ui::kit::control_style_of(
                    palette.palette_chip_fill,
                    [0.0; 4],
                    color_to_rgba(palette.title),
                    canvas_core::tokens::RADIUS_CHIP,
                ),
            }
        };
        // Подложка полосы (kit PanelStyle: слоты menu_fill/palette_border)
        instances.push(CardInstance {
            pos: [strip.rect[0], strip.rect[1]],
            size: [strip.rect[2], strip.rect[3]],
            fill: strip_panel_style.fill,
            border: strip_panel_style.border,
            params: [strip_panel_style.radius, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        // Строки категорий: состояния — WidgetState кита (FR-057: hover/
        // селекция раскрытой категории — матрица приоритетов), стиль —
        // ControlStyle из слотов
        for (i, ((rect, name), raw)) in strip.rows.iter().zip(raw_categories.iter()).enumerate() {
            let mut row_widget = WidgetState::default();
            row_widget.set_pointer(point_in_rect(*rect, self.cursor), false);
            row_widget.set_selected(open_category == Some(i));
            let style = row_style(row_widget.kit_state());
            instances.push(CardInstance {
                pos: [rect[0], rect[1]],
                size: [rect[2], rect[3]],
                fill: style.fill,
                border: style.border,
                params: [style.radius, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            // FR-087: счётчик — по видимым шаблонам (роль/ручной фильтр).
            let count = self.visible_templates_by_category(raw).len();
            texts.push(OwnedScreenText {
                text: format!("{name} · {count}"),
                origin: [rect[0] + 8.0, rect[1] + 6.0],
                width: rect[2] - 12.0,
                font_size: 12.0,
                color: palette.title,
                align: TextAlign::Left,
            });
        }
        // Шеврон «развернуть док» — строка внизу полосы (kit-стиль чипа)
        let chevron_style = canvas_ui::kit::control_style_of(
            palette.palette_chip_fill,
            [0.0; 4],
            color_to_rgba(palette.title),
            canvas_core::tokens::RADIUS_CHIP,
        );
        instances.push(CardInstance {
            pos: [strip.chevron_rect[0], strip.chevron_rect[1]],
            size: [strip.chevron_rect[2], strip.chevron_rect[3]],
            fill: chevron_style.fill,
            border: chevron_style.border,
            params: [chevron_style.radius, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        texts.push(OwnedScreenText {
            text: "»".to_owned(),
            origin: [strip.chevron_rect[0], strip.chevron_rect[1] + 5.0],
            width: strip.chevron_rect[2],
            font_size: 13.0,
            color: palette.title,
            align: TextAlign::Center,
        });
        // Flyout раскрытой категории: строки шаблонов (только видимое окно)
        if let (Some(cat), Some(fly)) = (
            open_category,
            self.template_flyout_geometry(viewport, &strip),
        ) {
            // Поиск по реестру — по raw-токену (имена в strip — локализованные)
            let Some(raw) = raw_categories.get(cat) else {
                return (instances, texts);
            };
            // FR-087: строки flyout — видимые шаблоны (роль/ручной фильтр).
            let items = self.visible_templates_by_category(raw);
            // W-c: панель flyout — kit PanelStyle (те же слоты, что у полосы)
            instances.push(CardInstance {
                pos: [fly.rect[0], fly.rect[1]],
                size: [fly.rect[2], fly.rect[3]],
                fill: strip_panel_style.fill,
                border: strip_panel_style.border,
                params: [strip_panel_style.radius, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            for (v, rect) in fly.row_rects.iter().enumerate() {
                let Some(manifest) = items.get(fly.scroll_top + v) else {
                    break;
                };
                // W-c: состояние строки — WidgetState кита (hover), стиль —
                // ControlStyle из слотов (карточка = palette_row_fill,
                // hover = palette_hover_fill + рамка palette_border — прежний
                // вид; радиус карточки рисует template_card_row).
                let mut row_widget = WidgetState::default();
                row_widget.set_pointer(point_in_rect(*rect, self.cursor), false);
                let row_state = row_widget.kit_state();
                let hovered = matches!(
                    row_state,
                    canvas_ui::kit::KitState::Hovered | canvas_ui::kit::KitState::Pressed
                );
                let row_style = canvas_ui::kit::control_style_of(
                    if hovered {
                        palette.palette_hover_fill
                    } else {
                        palette.palette_row_fill
                    },
                    if hovered {
                        palette.palette_border
                    } else {
                        [0.0; 4]
                    },
                    color_to_rgba(palette.title),
                    canvas_core::tokens::RADIUS_CHIP,
                );
                template_card_row(
                    manifest,
                    *rect,
                    row_style.fill,
                    row_style.border,
                    palette,
                    icon_tint,
                    self.settings.language,
                    &mut instances,
                    &mut texts,
                    &mut measurer,
                    &mut fs,
                );
            }
            // Индикаторы прокрутки: стрелки ▲/▼ у правого края flyout
            if fly.max_scroll > 0 {
                if fly.scroll_top > 0 {
                    texts.push(OwnedScreenText {
                        text: "▲".to_owned(),
                        origin: [fly.rect[0] + fly.rect[2] - 20.0, fly.rect[1] + 2.0],
                        width: 16.0,
                        font_size: 10.0,
                        color: palette.body,
                        align: TextAlign::Center,
                    });
                }
                if fly.scroll_top < fly.max_scroll {
                    texts.push(OwnedScreenText {
                        text: "▼".to_owned(),
                        origin: [
                            fly.rect[0] + fly.rect[2] - 20.0,
                            fly.rect[1] + fly.rect[3] - 15.0,
                        ],
                        width: 16.0,
                        font_size: 10.0,
                        color: palette.body,
                        align: TextAlign::Center,
                    });
                }
            }
        }
        (instances, texts)
    }

    /// Оверлей радиального wheel-меню шаблонов (FR-018, Shift+клик):
    /// плашки-мини-карточки (шаблоны + категории) из чистой геометрии
    /// `template_ui::wheel_geometry` — раскладка отталкивается от размера
    /// плашек, зазор гарантирован (правка владельца 2026-09-16). Пайплайн
    /// квадов без поворотов; hover — по тем же плашкам (WYSIWYG).
    /// FR-022 (бест-практики радиальных меню): затемнение фона под
    /// модальным пикером (паттерн Miro Template picker), круглая кнопка
    ///-хаб «назад/закрыть» (Kurtenbach/Buxton — центр отменяет уровень),
    /// крошки глубины в хабе (выбранная категория).
    /// FR-018/FR-022: wheel-меню шаблонов (Shift+клик) — donut-сектора
    /// (`SectorsPipeline`, SDF annular-wedge) + квады иконок/хаба + подписи.
    /// Сектора — screen-space; рендерер конвертирует их в world тем же
    /// способом, что и screen_instances (центр через screen_to_world,
    /// радиусы / zoom — `screen_sector_to_world` в renderer.rs). Возвращает
    /// (сектора, квады, тексты) — квады рисуются ПОВЕРХ секторов.
    pub(super) fn wheel_overlay(
        &self,
    ) -> (Vec<SectorInstance>, Vec<CardInstance>, Vec<OwnedScreenText>) {
        // Цвета wheel-меню. FR-046: открытый вопрос FR-022 закрыт — значения
        // из design-токенов (wheel.*; дифференциация светлой темы — v2).
        const FILL_DIM: [f32; 4] = canvas_core::tokens::WHEEL_DIM;
        const FILL_CATEGORY: [f32; 4] = canvas_core::tokens::WHEEL_CATEGORY;
        const FILL_TEMPLATE: [f32; 4] = canvas_core::tokens::WHEEL_TEMPLATE;
        const FILL_HOVER: [f32; 4] = canvas_core::tokens::WHEEL_HOVER;
        const FILL_HUB_ACTIVE: [f32; 4] = canvas_core::tokens::WHEEL_HUB_ACTIVE;
        const BORDER: [f32; 4] = canvas_core::tokens::WHEEL_BORDER;

        let mut sectors = Vec::new();
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let Some(menu) = &self.wheel_menu else {
            return (sectors, instances, texts);
        };
        let palette = self.effective_palette();
        let icon_tint = color_to_rgba(palette.icon);
        let [vw, vh] = self.viewport_logical();
        // FR-087: категории и шаблоны wheel-меню — видимые по роли/ручному
        // фильтру (тот же хелпер, что у click_wheel_menu — индексы совпадают).
        let categories = self.template_category_names();
        let templates: Vec<_> = menu
            .category
            .as_deref()
            .map(|c| self.visible_templates_by_category(c))
            .unwrap_or_default();
        let geo =
            template_ui::wheel_geometry(menu.screen, vw, vh, categories.len(), templates.len());
        let hovered = geo.hit(self.cursor);
        // CR-015: перенос подписи шаблона — по измеренной ширине слота
        // ([`template_ui::WHEEL_TPL_TEXT_W`]) при кегле подписи; замер —
        // тот же пул шейпинга, что у screen-текстов (FR-094)
        let mut m = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        // Затемнение фона — диском-полным-кругом: первый инстанс секторного
        // прохода, под ним ничего рисовать не нужно; радиус — до дальнего
        // угла viewport (кламп центра гарантирует покрытие окна)
        let dim_r = geo.center[0]
            .max(vw - geo.center[0])
            .hypot(geo.center[1].max(vh - geo.center[1]))
            + 4.0;
        sectors.push(SectorInstance {
            center: geo.center,
            r0: 0.0,
            r1: dim_r,
            a0: 0.0,
            a1: std::f32::consts::TAU,
            fill: FILL_DIM,
        });
        // Donut-сектора меню: что нарисовано — по тому и клик (geo.hit —
        // тот же полярный тест, что и SDF-шейдер)
        for sector in &geo.sectors {
            let active = hovered.as_ref() == Some(&sector.hit);
            let fill = if active {
                FILL_HOVER
            } else {
                match sector.hit {
                    WheelHit::Category(_) => FILL_CATEGORY,
                    WheelHit::Template(_) => FILL_TEMPLATE,
                }
            };
            sectors.push(SectorInstance {
                center: geo.center,
                r0: sector.r0,
                r1: sector.r1,
                a0: sector.a0,
                a1: sector.a1,
                fill,
            });
            // Иконка + подпись внутри сектора (как circular-menu: вертикально,
            // иконка выше текста; подпись всегда рисуем — слот
            // template_ui::WHEEL_TPL_TEXT_W × 2 строки кегля
            // template_ui::WHEEL_TPL_FONT; перенос — split_two_lines по
            // измеренной ширине слота — CR-015)
            let [px, py] =
                template_ui::sector_point(geo.center, sector.mid_angle(), sector.mid_radius());
            match sector.hit {
                WheelHit::Category(i) => {
                    texts.push(OwnedScreenText {
                        text: categories[i].to_owned(),
                        origin: [px - 40.0, py - 7.0],
                        width: 80.0,
                        font_size: 12.0,
                        color: palette.title,
                        align: TextAlign::Center,
                    });
                }
                WheelHit::Template(i) => {
                    let Some(manifest) = templates.get(i) else {
                        continue;
                    };
                    instances.extend(template_icon_quads(
                        template_ui::icon_key(manifest),
                        [px - 8.0, py - 14.0, 16.0, 16.0],
                        icon_tint,
                    ));
                    let (line1, line2) = split_two_lines(
                        &mut m,
                        &mut fs,
                        manifest.display_name(self.settings.language),
                        template_ui::WHEEL_TPL_TEXT_W,
                    );
                    let push_line = |text: String, dy: f32| OwnedScreenText {
                        text,
                        origin: [px - template_ui::WHEEL_TPL_TEXT_W / 2.0, py + dy],
                        width: template_ui::WHEEL_TPL_TEXT_W,
                        font_size: template_ui::WHEEL_TPL_FONT,
                        color: palette.title,
                        align: TextAlign::Center,
                    };
                    match line2 {
                        None => texts.push(push_line(line1, 4.0)),
                        Some(line2) => {
                            texts.push(push_line(line1, 2.0));
                            texts.push(push_line(line2, 15.0));
                        }
                    }
                }
            }
        }
        // Хаб: круглая кнопка «назад/закрыть» (FR-022). Без категории —
        // подсказка «закрыть»; с категорией — крошки глубины «← имя»
        let [hx, hy, hw, hh] = geo.hub;
        instances.push(CardInstance {
            pos: [hx, hy],
            size: [hw, hh],
            fill: if menu.category.is_some() {
                FILL_HUB_ACTIVE
            } else {
                FILL_CATEGORY
            },
            border: BORDER,
            params: [hw / 2.0, 0.0, 0.0, 1.0], // круг — радиус = половина стороны
            corners: [0.0; 4],
        });
        texts.push(OwnedScreenText {
            text: if let Some(category) = &menu.category {
                format!("← {category}")
            } else {
                "закрыть".to_owned()
            },
            origin: [geo.center[0] - 44.0, geo.center[1] - 6.0],
            width: 88.0,
            font_size: 10.0,
            color: palette.title,
            align: TextAlign::Center,
        });
        (sectors, instances, texts)
    }

    /// Оверлей контекстного меню пустого канваса (T7): фон, подписи.
    /// Screen-space — логические px, константный читаемый размер при любом
    /// зуме (уточнение владельца). Меню ноды/связи заменены палитрой.
    /// FR-038 (T-038.5): batch-пункты выравнивания — хвост меню, видны
    /// только при N≥3 выделенных нодах (единый список с хит-тестом).
    /// FR-050 Н2 (этап C): оверлей меню выбора — панель + заголовок +
    /// пункты (имена параметров / строки-источники со значениями) +
    /// hover-подсветка. Screen-space (как контекстное меню T7):
    /// константный размер при любом зуме; клик по пункту — действие,
    /// мимо/Esc — отмена.
    pub(super) fn choice_menu_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let Some(menu) = &self.choice_menu else {
            return (instances, texts);
        };
        let palette = self.effective_palette();
        // Панель: высота пунктов + строка заголовка (геометрия меню T7)
        let [x, y, w, h] = menu_rect_for(menu.origin, menu.items.len());
        instances.push(CardInstance {
            pos: [x, y],
            size: [w, h + CHOICE_MENU_TITLE_H],
            fill: palette.menu_fill,
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        // Заголовок — приглушённым тоном (не пункт, не интерактивен)
        texts.push(OwnedScreenText {
            text: self.tr(menu.title_key).to_owned(),
            origin: [x + MENU_PADDING + 4.0, y + MENU_PADDING + 5.0],
            width: w - MENU_PADDING * 2.0 - 8.0,
            font_size: 12.0,
            color: palette.body,
            align: TextAlign::Left,
        });
        // Пункты — геометрия меню T7, сдвинутая на высоту заголовка;
        // хит-тест — той же геометрией (choice_menu_item_at), клик по
        // заголовку = «мимо пункта» = отмена
        for (i, item) in menu.items.iter().enumerate() {
            let rect = menu_item_rect(menu.origin, i);
            let rect = [rect[0], rect[1] + CHOICE_MENU_TITLE_H, rect[2], rect[3]];
            if menu.hovered == Some(i) {
                instances.push(CardInstance {
                    pos: [rect[0], rect[1]],
                    size: [rect[2], rect[3]],
                    fill: palette.control_hover_fill,
                    border: [0.0; 4],
                    params: [4.0, 0.0, 0.0, 1.0],
                    corners: [0.0; 4],
                });
            }
            texts.push(OwnedScreenText {
                text: item.label.clone(),
                origin: [rect[0] + 8.0, rect[1] + 5.0],
                width: rect[2] - 12.0,
                font_size: 14.0,
                color: palette.title,
                align: TextAlign::Left,
            });
        }
        (instances, texts)
    }

    pub(super) fn canvas_menu_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let Some(menu) = &self.menu else {
            return (instances, texts);
        };
        let items =
            canvas_menu_visible_items_ext(self.align_menu_visible(), self.autowidth_menu_visible());
        let palette = self.effective_palette();
        let [x, y, w, h] = menu_rect_for(menu.origin, items.len());
        instances.push(CardInstance {
            pos: [x, y],
            size: [w, h],
            fill: palette.menu_fill,
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        // Hover-подсветка пункта (аффорданс — как строки палитры/поиска:
        // интерактивный элемент отвечает на курсор).
        // FR-060: пункты — `kit::list_rows` (однородные строки
        // MENU_ITEM_HEIGHT с зазором 0 — прежняя стопка menu_item_rect
        // дословно), состояние — WidgetState (Hovered), отрисовка —
        // Painter (панель с тенью — квад: флаг params.w вне контракта
        // PaintItem); hit-тест menu_item_at_for — прежний (lib.rs).
        let hovered_item = menu_item_at_for(menu.origin, self.cursor, items.len());
        let kit_palette = palette.kit_palette();
        let mut d = Painter::new();
        let rows_area = canvas_ui::geometry::UiRect::new(
            x + MENU_PADDING,
            y + MENU_PADDING,
            (w - MENU_PADDING * 2.0).max(0.0),
            (h - MENU_PADDING * 2.0).max(0.0),
        );
        let rows_scroll = canvas_ui::kit::ScrollState {
            offset: 0.0,
            content_h: items.len() as f32 * MENU_ITEM_HEIGHT,
            viewport_h: rows_area.h,
        };
        for (i, rect) in
            canvas_ui::kit::list_rows(rows_area, &rows_scroll, MENU_ITEM_HEIGHT, 0.0, items.len())
        {
            let Some(item) = items.get(i) else {
                continue;
            };
            let mut state = WidgetState::default();
            state.set_pointer(hovered_item == Some(i), false);
            if state.kit_state() == kit::KitState::Hovered {
                d.rect(rect, palette.control_hover_fill, [0.0; 4], 4.0);
            }
            d.label(
                canvas_ui::geometry::UiRect::new(
                    rect.x + CANVAS_MENU_LABEL_X,
                    rect.y + 5.0,
                    (rect.w - CANVAS_MENU_LABEL_X).max(0.0),
                    MENU_ITEM_HEIGHT,
                ),
                &canvas_menu_label(
                    *item,
                    self.settings.focus_mode,
                    self.hotkeys_open,
                    self.desktop_menu_checked(),
                    self.settings.bottleneck_overlay,
                    self.scene.whatif_active,
                    self.settings.language,
                ),
                kit_palette.text_title,
                14.0,
                PaintAlign::Left,
            );
        }
        // M5 (T20-F): колонка подменю «Виджеты ▸» — справа от меню
        if let Some(submenu) = &menu.submenu {
            let [sx, sy, sw, sh] = submenu_rect(submenu);
            instances.push(CardInstance {
                pos: [sx, sy],
                size: [sw, sh],
                fill: palette.menu_fill,
                border: [0.0; 4],
                params: [6.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            if submenu.entries.is_empty() {
                texts.push(OwnedScreenText {
                    text: self.tr(keys::WIDGETS_EMPTY).to_owned(),
                    origin: [
                        submenu.origin[0] + MENU_PADDING + 4.0,
                        submenu.origin[1] + MENU_PADDING + 5.0,
                    ],
                    width: MENU_WIDTH - MENU_PADDING * 2.0 - 8.0,
                    font_size: 13.0,
                    color: palette.body,
                    align: TextAlign::Left,
                });
            } else {
                // FR-060: строки подменю — `kit::list_rows` (те же контракты,
                // что у главного меню: стопка menu_item_rect дословно)
                let hovered_sub = submenu_item_at(submenu, self.cursor);
                let sub_area = canvas_ui::geometry::UiRect::new(
                    sx + MENU_PADDING,
                    sy + MENU_PADDING,
                    (sw - MENU_PADDING * 2.0).max(0.0),
                    (sh - MENU_PADDING * 2.0).max(0.0),
                );
                let sub_scroll = canvas_ui::kit::ScrollState {
                    offset: 0.0,
                    content_h: submenu.entries.len() as f32 * MENU_ITEM_HEIGHT,
                    viewport_h: sub_area.h,
                };
                for (i, rect) in canvas_ui::kit::list_rows(
                    sub_area,
                    &sub_scroll,
                    MENU_ITEM_HEIGHT,
                    0.0,
                    submenu.entries.len(),
                ) {
                    let Some(entry) = submenu.entries.get(i) else {
                        continue;
                    };
                    let mut state = WidgetState::default();
                    state.set_pointer(hovered_sub == Some(i), false);
                    if state.kit_state() == kit::KitState::Hovered {
                        d.rect(rect, palette.control_hover_fill, [0.0; 4], 4.0);
                    }
                    d.label(
                        canvas_ui::geometry::UiRect::new(
                            rect.x + CANVAS_MENU_LABEL_X,
                            rect.y + 5.0,
                            (rect.w - CANVAS_MENU_LABEL_X).max(0.0),
                            MENU_ITEM_HEIGHT,
                        ),
                        &entry.label,
                        kit_palette.text_title,
                        13.0,
                        PaintAlign::Left,
                    );
                }
            }
        }
        paint_items_to_band(d.take_items(), &mut instances, &mut texts);
        (instances, texts)
    }

    /// FR-027: оверлей меню помощи кнопки «?» — колонка у кнопки и
    /// раскрытое подменю разделов (паттерн контекстного меню T7).
    pub(super) fn help_menu_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let Some(menu) = &self.help_menu else {
            return (instances, texts);
        };
        let viewport = self.viewport_logical();
        let palette = self.effective_palette();
        let [x, y, w, h] = docs_ui::help_menu_rect(menu.origin);
        instances.push(CardInstance {
            pos: [x, y],
            size: [w, h],
            fill: palette.menu_fill,
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        let hovered = docs_ui::help_menu_item_at(menu.origin, self.cursor);
        for (i, item) in docs_ui::HELP_MENU_ITEMS.iter().enumerate() {
            let rect = docs_ui::help_menu_item_rect(menu.origin, i);
            if hovered == Some(*item) {
                instances.push(CardInstance {
                    pos: [rect[0], rect[1]],
                    size: [rect[2], rect[3]],
                    fill: palette.control_hover_fill,
                    border: [0.0; 4],
                    params: [4.0, 0.0, 0.0, 1.0],
                    corners: [0.0; 4],
                });
            }
            texts.push(OwnedScreenText {
                text: docs_ui::help_menu_item_label(*item, self.settings.language).to_owned(),
                origin: [rect[0] + 8.0, rect[1] + 6.0],
                width: rect[2] - 8.0,
                font_size: 13.0,
                color: palette.title,
                align: TextAlign::Left,
            });
        }
        if menu.docs_open {
            let sub = docs_ui::help_submenu_origin(menu.origin, viewport);
            let [sx, sy, sw, sh] = docs_ui::help_submenu_rect(sub);
            instances.push(CardInstance {
                pos: [sx, sy],
                size: [sw, sh],
                fill: palette.menu_fill,
                border: [0.0; 4],
                params: [6.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            let hovered_sub = docs_ui::help_submenu_item_at(sub, self.cursor);
            for (i, page) in docs_ui::DOCS_PAGES.iter().enumerate() {
                let rect = docs_ui::help_submenu_item_rect(sub, i);
                if hovered_sub == Some(i) {
                    instances.push(CardInstance {
                        pos: [rect[0], rect[1]],
                        size: [rect[2], rect[3]],
                        fill: palette.control_hover_fill,
                        border: [0.0; 4],
                        params: [4.0, 0.0, 0.0, 1.0],
                        corners: [0.0; 4],
                    });
                }
                texts.push(OwnedScreenText {
                    text: self.tr(page.label_key).to_owned(),
                    origin: [rect[0] + 8.0, rect[1] + 6.0],
                    width: rect[2] - 8.0,
                    font_size: 13.0,
                    color: palette.title,
                    align: TextAlign::Left,
                });
            }
        }
        (instances, texts)
    }

    /// FR-027: оверлей просмотрщика документации — правый док: шапка
    /// (раздел + ×), скроллируемый контент (кламп строк к видимой зоне),
    /// внутренние ссылки — акцент + подчёркивание, скроллбар-аффорданс.
    pub(super) fn docs_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let Some(viewer) = &self.docs else {
            return (instances, texts);
        };
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (instances, texts);
        }
        let palette = self.effective_palette();
        // FR-054: сдвиг по спанам — измеренными ширинами (те же метрики, что
        // у раскладки страницы); measurer на кадр (паттерн пилотов U3).
        let mut measurer = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        let panel = docs_ui::viewer_rect(viewport);
        let content = docs_ui::viewer_content_rect(panel);
        // Затемнение канваса вокруг панели (паттерн wheel FR-022)
        if panel[0] > 0.0 {
            instances.push(CardInstance {
                pos: [0.0, 0.0],
                size: [viewport[0], viewport[1]],
                // FR-070 (W-d): слот `stage_dim` темы (бывший инлайн-литерал
                // `[0.02, 0.02, 0.04, 0.85]` — alpha 0.6 канонич. коридора
                // PRD 0.55–0.65, не переопределяем — single source of truth).
                fill: palette.stage_dim,
                border: [0.0; 4],
                params: [0.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
        }
        // Панель
        instances.push(CardInstance {
            pos: [panel[0], panel[1]],
            size: [panel[2], panel[3]],
            fill: palette.menu_fill,
            border: [0.0; 4],
            params: [8.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        // Шапка: раздел + × (hover-аффорданс)
        let close = docs_ui::viewer_close_rect(panel);
        let close_hovered = point_in_rect(close, self.cursor);
        instances.push(CardInstance {
            pos: [close[0], close[1]],
            size: [close[2], close[3]],
            fill: if close_hovered {
                hover_fill(palette.menu_fill)
            } else {
                palette.menu_fill
            },
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        texts.push(OwnedScreenText {
            text: "×".to_owned(),
            origin: [close[0], close[1] + (close[3] - 14.0 * 1.3) / 2.0],
            width: close[2],
            font_size: 14.0,
            color: palette.title,
            align: TextAlign::Center,
        });
        let title = docs_ui::DOCS_PAGES
            .get(viewer.page)
            .map(|p| self.tr(p.label_key).to_owned())
            .unwrap_or_default();
        texts.push(OwnedScreenText {
            text: title,
            origin: [panel[0] + docs_ui::DOCS_PADDING, panel[1] + 11.0],
            width: close[0] - panel[0] - docs_ui::DOCS_PADDING * 2.0,
            font_size: 15.0,
            color: palette.title,
            align: TextAlign::Left,
        });
        // Контент: строки раскладки со сдвигом −offset; строки вне видимой
        // зоны не рендерим (кламп, длинные страницы дешевле кадра)
        let view_top = content[1];
        let view_bottom = content[1] + content[3];
        for line in &viewer.layout.lines {
            let (_, line_h) = docs_ui::kind_metrics(line.kind);
            let y = content[1] + line.y - viewer.scroll.offset;
            if y + line_h < view_top || y > view_bottom {
                continue;
            }
            let (font, _) = docs_ui::kind_metrics(line.kind);
            let color = match line.kind {
                docs_ui::RowKind::Heading(_) => palette.title,
                docs_ui::RowKind::Quote | docs_ui::RowKind::Code => palette.icon,
                docs_ui::RowKind::TableCell { header: true } => palette.title,
                _ => palette.body,
            };
            let mut x = content[0] + line.x;
            for span in &line.spans {
                // Внутренние ссылки — акцент; внешние — обычный текст
                // (v1 не кликабельны, FR-027)
                let span_color = if span.href.as_deref().is_some_and(|href| {
                    matches!(docs_ui::link_target(href), docs_ui::LinkTarget::Page(_))
                }) {
                    palette.link
                } else {
                    color
                };
                texts.push(OwnedScreenText {
                    text: span.text.clone(),
                    origin: [x, y],
                    width: (content[0] + content[2] - x).max(10.0),
                    font_size: font,
                    color: span_color,
                    align: TextAlign::Left,
                });
                x += measurer.width_of(&mut fs, &span.text, canvas_render::text::SANS_FAMILY, font);
            }
        }
        // Квады раскладки (линии/подчёркивания шапок таблиц/бары цитат) —
        // видимые по y
        for quad in &viewer.layout.quads {
            let y = content[1] + quad.y - viewer.scroll.offset;
            if y + quad.height < view_top || y > view_bottom {
                continue;
            }
            instances.push(CardInstance {
                pos: [content[0] + quad.x, y],
                size: [quad.width, quad.height.max(1.0)],
                fill: match quad.kind {
                    // FR-070 (W-d): слот `rule_color` (бывший инлайн-литерал
                    // `[0.30, 0.33, 0.40, 0.8]` — ранее тот же RGB, что
                    // `gfm_muted_fill`, но отличался по тону — теперь свой слот).
                    docs_ui::QuadKind::Rule => palette.rule_color,
                    docs_ui::QuadKind::QuoteBar => color_to_rgba(palette.link),
                },
                border: [0.0; 4],
                params: [0.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
        }
        // Подчёркивания внутренних ссылок (кликабельный аффорданс)
        for link in &viewer.layout.links {
            let y = content[1] + link.rect[1] + link.rect[3] - 2.0 - viewer.scroll.offset;
            if y < view_top || y > view_bottom {
                continue;
            }
            instances.push(CardInstance {
                pos: [content[0] + link.rect[0], y],
                size: [link.rect[2], 1.0],
                fill: color_to_rgba(palette.link),
                border: [0.0; 4],
                params: [0.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
        }
        // Скроллбар-аффорданс справа (ползунок по пропорции offset/max)
        if viewer.scroll.max_offset() > 0.0 {
            let track_h = content[3];
            let thumb_h = (track_h * track_h / viewer.layout.content_height).clamp(24.0, track_h);
            let free = (track_h - thumb_h).max(0.0);
            let thumb_y = content[1] + free * viewer.scroll.offset / viewer.scroll.max_offset();
            instances.push(CardInstance {
                pos: [
                    panel[0] + panel[2] - docs_ui::DOCS_SCROLLBAR_W - 2.0,
                    thumb_y,
                ],
                size: [docs_ui::DOCS_SCROLLBAR_W, thumb_h],
                // FR-070 (W-d): слот `scrollbar_thumb` (бывший инлайн-литерал
                // `[0.35, 0.38, 0.46, 0.7]`).
                fill: palette.scrollbar_thumb,
                border: [0.0; 4],
                params: [3.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
        }
        // Футер-подсказка
        texts.push(OwnedScreenText {
            text: self.tr(keys::DOCS_FOOTER).to_owned(),
            origin: [panel[0] + docs_ui::DOCS_PADDING, panel[1] + panel[3] - 18.0],
            width: panel[2] - docs_ui::DOCS_PADDING * 2.0,
            font_size: 11.0,
            color: palette.icon,
            align: TextAlign::Left,
        });
        (instances, texts)
    }

    /// FR-028: оверлей онбординга — затемнение канваса + карточка по центру
    /// (заголовок, тело, прогресс-точки, кнопки Назад/Далее|Готово/Пропустить).
    ///
    /// W-e (разморозка 03.10.2026, миграция на кит; аудит ui-kit §4 A1):
    /// отрисовка — [`Painter`] (образец `autolink_frame` после FR-060),
    /// конвертация — `paint_items_to_band` (screen-конвенция полосы
    /// `Modals`); ручных квадов/`CardInstance` в пути больше нет. Состояния
    /// кнопок — [`WidgetState`] → `KitState` → `kit::button_style`
    /// (замена ад-хок «ярления» `hover_fill` ×1.3+0.04). Цвета — только
    /// слоты темы/кита (бывшие 6 литералов draw): затемнение — `stage_dim`,
    /// карточка — `modal_style` (`panel_fill`/`panel_border`,
    /// `RADIUS_PANEL` = прежний литерал 10), точки — `link`/`panel_border`,
    /// кнопки — слоты `button_style` (CTA `control_primary` = прежний
    /// литерал `DIALOG_BUTTON_PRIMARY`). Тело — видимое окно
    /// `kit::list_rows` над скролл-состоянием ([`App::onboarding_scroll`]):
    /// при клампе высоты карточки строки прокручиваются, футер с CTA всегда
    /// виден (аудит §8 №12).
    pub(super) fn onboarding_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut quads = Vec::new();
        let mut texts = Vec::new();
        let Some(state) = &self.onboarding else {
            return (quads, texts);
        };
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (quads, texts);
        }
        let palette = self.effective_palette();
        let Some(step) = onboarding_ui::ONBOARDING_STEPS.get(state.step) else {
            return (quads, texts);
        };
        // W-e: раскладка измеренная (kit::modal + TextMeasurer::wrap), общая
        // с реестром/вводом; скролл-состояние тела синхронизируется кадром
        // (offset сохраняется, content/viewport клампятся раскладкой).
        let mut scroll = self.onboarding_scroll.clone();
        let mut m = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        let lay = onboarding_ui::card_layout(
            viewport,
            state.step,
            self.settings.language,
            &mut scroll,
            &mut m,
            &mut fs,
        );
        let card = lay.card;
        let kit_palette = palette.kit_palette();
        // FR-057: draw-журнал кита — items конвертируются в инстансы полосы
        // одним проходом в конце (порядок = draw-порядок)
        let mut d = Painter::new();
        // Затемнение (паттерн wheel FR-022): тур поверх неинтерактивного
        // канваса — фокус на карточке. Слот stage_dim (бывший литерал
        // [0.02, 0.02, 0.04, 0.85] — та же триада RGB, альфа слота темы).
        d.rect(
            canvas_ui::geometry::UiRect::new(0.0, 0.0, viewport[0], viewport[1]),
            palette.stage_dim,
            [0.0; 4],
            0.0,
        );
        // Карточка — kit-панель модали (слоты panel_fill/panel_border,
        // радиус RADIUS_PANEL = прежний литерал 10; как dialog/autolink)
        d.panel(
            canvas_ui::geometry::UiRect::new(card[0], card[1], card[2], card[3]),
            &kit::modal_style(&kit_palette),
        );
        // Заголовок (слот title)
        d.label(
            canvas_ui::geometry::UiRect::new(
                card[0] + onboarding_ui::ONBOARDING_PAD,
                card[1] + onboarding_ui::ONBOARDING_PAD,
                (card[2] - onboarding_ui::ONBOARDING_PAD * 2.0).max(0.0),
                onboarding_ui::ONBOARDING_TITLE_LINE_H,
            ),
            self.tr(step.title_key),
            color_to_rgba(palette.title),
            onboarding_ui::ONBOARDING_TITLE_FONT,
            PaintAlign::Left,
        );
        // Прогресс-точки: текущая — слот link (как прежде), остальные —
        // panel_border (бывший литерал [0.30, 0.33, 0.40, 0.9])
        let (centers, dots_y) = onboarding_ui::progress_dots(card);
        let dot_r = onboarding_ui::ONBOARDING_DOT / 2.0;
        for (i, cx) in centers.iter().enumerate() {
            let fill = if i == state.step {
                color_to_rgba(palette.link)
            } else {
                palette.palette_border
            };
            d.rect(
                canvas_ui::geometry::UiRect::new(cx - dot_r, dots_y, dot_r * 2.0, dot_r * 2.0),
                fill,
                [0.0; 4],
                dot_r,
            );
        }
        // Тело шага: видимое окно строк — kit::list_rows над скролл-состоянием
        // тела (строки — тот же измеренный источник, что высота карточки:
        // lay.lines). При клампе высоты видны только строки окна — контент
        // ужимается прокруткой, футер с CTA не перекрывается.
        let body = onboarding_ui::body_area(card);
        for (idx, row) in kit::list_rows(
            canvas_ui::geometry::UiRect::new(body[0], body[1], body[2], body[3]),
            &scroll,
            onboarding_ui::ONBOARDING_BODY_LINE_H,
            0.0,
            lay.lines.len(),
        ) {
            let Some(line) = lay.lines.get(idx) else {
                continue;
            };
            d.label(
                canvas_ui::geometry::UiRect::new(
                    body[0],
                    row.y,
                    body[2],
                    onboarding_ui::ONBOARDING_BODY_LINE_H,
                ),
                line,
                color_to_rgba(palette.body),
                onboarding_ui::ONBOARDING_BODY_FONT,
                PaintAlign::Left,
            );
        }
        // Кнопки: Назад (слева, не на первом шаге), Далее/Готово (справа,
        // primary), Пропустить (правый верх — ghost, выход виден всегда —
        // NN/g). Состояния — WidgetState (FR-057: переходы указателя ведёт
        // потребитель, стиль — матрица кита); стиль — слоты button_style
        // (бывшие литералы CTA/вторичной заливки/рамки и hover_fill).
        // Контракт кита: зажатие не ведётся (pressed_now = false) — клик
        // обрабатывает `click_onboarding` по button_at.
        let buttons = [
            (
                OnboardingButton::Prev,
                kit::ButtonVariant::Secondary,
                state.prev_label_key().map(|key| self.tr(key).to_owned()),
                13.0,
            ),
            (
                OnboardingButton::Next,
                kit::ButtonVariant::Primary,
                Some(self.tr(state.next_label_key()).to_owned()),
                13.0,
            ),
            (
                OnboardingButton::Skip,
                kit::ButtonVariant::Ghost,
                Some(self.tr(keys::ONBOARDING_SKIP).to_owned()),
                11.0,
            ),
        ];
        for (button, variant, label, font) in &buttons {
            let Some(label) = label else {
                continue;
            };
            let rect = onboarding_ui::button_rect(card, *button);
            let mut widget = WidgetState::default();
            widget.set_pointer(point_in_rect(rect, self.cursor), false);
            let style = kit::button_style(*variant, widget.kit_state(), &kit_palette);
            d.control(
                canvas_ui::geometry::UiRect::new(rect[0], rect[1], rect[2], rect[3]),
                &style,
            );
            d.label(
                canvas_ui::geometry::UiRect::new(
                    rect[0],
                    rect[1] + (rect[3] - font * 1.3) / 2.0,
                    rect[2],
                    font * 1.3,
                ),
                label,
                style.text,
                *font,
                PaintAlign::Center,
            );
        }
        paint_items_to_band(d.take_items(), &mut quads, &mut texts);
        (quads, texts)
    }

    /// FR-LLM-B / PRD-0010 F-8: AI-онбординг — модальный оверлей выбора
    /// режима (Local / Cloud / Self-hosted). Карточка по центру вьюпорта,
    /// 3 радио-карточки, кнопки «Подробнее о privacy» и «Продолжить» (disabled
    /// пока режим не выбран). Раскрытый блок privacy — под кнопками. Рендер
    /// — квады + screen-текст (как все оверлеи); полоса Modals (блокирует
    /// ввод канваса через `ai_onboarding_point_in_card`).
    pub(super) fn ai_onboarding_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut quads = Vec::new();
        let mut texts = Vec::new();
        let Some(state) = &self.ai_onboarding else {
            return (quads, texts);
        };
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (quads, texts);
        }
        let palette = self.effective_palette();
        let kit_palette = palette.kit_palette();
        let lay = onboarding_ui::ai_onboarding_layout(viewport, state);
        let card = lay.card;
        // Затемнение канваса (паттерн FR-022/FR-028) — фокус на карточке.
        quads.push(CardInstance {
            pos: [0.0, 0.0],
            size: [viewport[0], viewport[1]],
            fill: palette.stage_dim,
            border: [0.0; 4],
            params: [0.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        // Карточка — kit-панель модали (слоты panel_fill/panel_border).
        quads.push(CardInstance {
            pos: [card[0], card[1]],
            size: [card[2], card[3]],
            fill: palette.menu_fill,
            border: palette.palette_border,
            params: [10.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        let inner_x = card[0] + onboarding_ui::AI_ONB_PAD_X;
        let inner_w = (card[2] - onboarding_ui::AI_ONB_PAD_X * 2.0).max(0.0);
        let mut y = card[1] + onboarding_ui::AI_ONB_PAD_TOP;
        // Шаг «Шаг 3 из 3 · AI-режим» (акцентный цвет, маленький кегль).
        texts.push(OwnedScreenText {
            text: self.tr(keys::AI_ONB_STEP).to_owned(),
            origin: [inner_x, y],
            width: inner_w,
            font_size: onboarding_ui::AI_ONB_STEP_FONT,
            color: palette.link,
            align: TextAlign::Left,
        });
        y += onboarding_ui::AI_ONB_STEP_H;
        // Заголовок «Выберите режим AI».
        texts.push(OwnedScreenText {
            text: self.tr(keys::AI_ONB_TITLE).to_owned(),
            origin: [inner_x, y],
            width: inner_w,
            font_size: onboarding_ui::AI_ONB_TITLE_FONT,
            color: palette.title,
            align: TextAlign::Left,
        });
        y += onboarding_ui::AI_ONB_TITLE_H;
        // Подзаголовок (приглушённый, перенос — на стороне рендера).
        texts.push(OwnedScreenText {
            text: self.tr(keys::AI_ONB_SUB).to_owned(),
            origin: [inner_x, y],
            width: inner_w,
            font_size: onboarding_ui::AI_ONB_SUB_FONT,
            color: palette.icon,
            align: TextAlign::Left,
        });
        // FR-LLM-B: y не используется дальше — радио-карточки позиционируются
        // через lay.mode_cards[i], не через y (clippy unused_assignments).
        // 3 радио-карточки режимов (Local / Cloud / SelfHosted).
        let choices = [
            AiOnboardingChoice::Local,
            AiOnboardingChoice::Cloud,
            AiOnboardingChoice::SelfHosted,
        ];
        for (i, &choice) in choices.iter().enumerate() {
            let mode_rect = lay.mode_cards[i];
            let selected = state.selected == Some(choice);
            let hovered = point_in_rect(mode_rect, self.cursor);
            // Радио-карточка: selected — акцентная рамка + полупрозрачная
            // заливка; hovered — нейтральная подсветка; иначе — chip-заливка.
            quads.push(CardInstance {
                pos: [mode_rect[0], mode_rect[1]],
                size: [mode_rect[2], mode_rect[3]],
                fill: if selected {
                    [
                        kit_palette.accent[0],
                        kit_palette.accent[1],
                        kit_palette.accent[2],
                        0.10,
                    ]
                } else if hovered {
                    palette.palette_hover_fill
                } else {
                    palette.palette_row_fill
                },
                border: if selected {
                    kit_palette.accent
                } else {
                    palette.palette_border
                },
                params: [8.0, selected as u8 as f32, 0.0, 1.0],
                corners: [0.0; 4],
            });
            // Радио-кружок слева (selected — залитый, иначе — контурный).
            let dot_r = 6.0;
            let dot_cx = mode_rect[0] + 14.0;
            let dot_cy = mode_rect[1] + 18.0;
            quads.push(CardInstance {
                pos: [dot_cx - dot_r, dot_cy - dot_r],
                size: [dot_r * 2.0, dot_r * 2.0],
                fill: if selected {
                    kit_palette.accent
                } else {
                    [0.0; 4]
                },
                border: if selected {
                    kit_palette.accent
                } else {
                    palette.palette_border
                },
                params: [dot_r, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            // Заголовок режима (Local only / Cloud / Self-hosted).
            texts.push(OwnedScreenText {
                text: self.tr(choice.title_key()).to_owned(),
                origin: [mode_rect[0] + 30.0, mode_rect[1] + 4.0],
                width: mode_rect[2] - 30.0 - 80.0,
                font_size: onboarding_ui::AI_ONB_MODE_TITLE_FONT,
                color: palette.title,
                align: TextAlign::Left,
            });
            // Тег режима (правый верхний угол — «offline» / «лучшее качество» / «в контуре»).
            let tag_w = 80.0;
            quads.push(CardInstance {
                pos: [
                    mode_rect[0] + mode_rect[2] - tag_w - 8.0,
                    mode_rect[1] + 6.0,
                ],
                size: [tag_w, 14.0],
                fill: [
                    kit_palette.accent[0],
                    kit_palette.accent[1],
                    kit_palette.accent[2],
                    0.13,
                ],
                border: [
                    kit_palette.accent[0],
                    kit_palette.accent[1],
                    kit_palette.accent[2],
                    0.30,
                ],
                params: [7.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            texts.push(OwnedScreenText {
                text: self.tr(choice.tag_key()).to_owned(),
                origin: [
                    mode_rect[0] + mode_rect[2] - tag_w - 8.0,
                    mode_rect[1] + 7.0,
                ],
                width: tag_w,
                font_size: 9.0,
                color: palette.link,
                align: TextAlign::Center,
            });
            // Описание режима (приглушённый, под заголовком).
            texts.push(OwnedScreenText {
                text: self.tr(choice.desc_key()).to_owned(),
                origin: [mode_rect[0] + 30.0, mode_rect[1] + 24.0],
                width: mode_rect[2] - 30.0 - 12.0,
                font_size: onboarding_ui::AI_ONB_MODE_DESC_FONT,
                color: palette.icon,
                align: TextAlign::Left,
            });
        }
        // Кнопка «Подробнее о privacy» (secondary).
        let btn_priv = lay.btn_privacy;
        let priv_hovered = point_in_rect(btn_priv, self.cursor);
        quads.push(CardInstance {
            pos: [btn_priv[0], btn_priv[1]],
            size: [btn_priv[2], btn_priv[3]],
            fill: if priv_hovered {
                palette.palette_hover_fill
            } else {
                palette.palette_chip_fill
            },
            border: palette.palette_border,
            params: [6.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        texts.push(OwnedScreenText {
            text: self.tr(keys::AI_ONB_PRIV).to_owned(),
            origin: [btn_priv[0], btn_priv[1] + 8.0],
            width: btn_priv[2],
            font_size: onboarding_ui::AI_ONB_BTN_FONT,
            color: palette.body,
            align: TextAlign::Center,
        });
        // Кнопка «Продолжить» (primary, disabled если selected = None).
        let btn_cont = lay.btn_continue;
        let cont_enabled = state.selected.is_some();
        let cont_hovered = cont_enabled && point_in_rect(btn_cont, self.cursor);
        quads.push(CardInstance {
            pos: [btn_cont[0], btn_cont[1]],
            size: [btn_cont[2], btn_cont[3]],
            fill: if !cont_enabled {
                palette.palette_chip_fill
            } else if cont_hovered {
                palette.palette_hover_fill
            } else {
                kit_palette.control_primary
            },
            border: if cont_enabled {
                kit_palette.control_primary
            } else {
                palette.palette_border
            },
            params: [6.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        texts.push(OwnedScreenText {
            text: self.tr(keys::AI_ONB_CONTINUE).to_owned(),
            origin: [btn_cont[0], btn_cont[1] + 8.0],
            width: btn_cont[2],
            font_size: onboarding_ui::AI_ONB_BTN_FONT,
            color: if cont_enabled {
                palette.title
            } else {
                palette.icon
            },
            align: TextAlign::Center,
        });
        // Раскрытый блок «Подробнее о privacy» — под кнопками.
        if state.privacy_open && lay.privacy_block[3] > 0.0 {
            let pb = lay.privacy_block;
            quads.push(CardInstance {
                pos: [pb[0], pb[1]],
                size: [pb[2], pb[3]],
                fill: palette.palette_row_fill,
                border: palette.palette_border,
                params: [8.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            // 4 параграфа: title + body + title + body + title + body + docs.
            let paras: [(&'static str, &'static str); 7] = [
                (keys::AI_ONB_PRIV_TITLE, keys::AI_ONB_PRIV_BODY),
                (keys::AI_ONB_PRIV_KEYS, keys::AI_ONB_PRIV_KEYS_BODY),
                (
                    keys::AI_ONB_PRIV_TELEMETRY,
                    keys::AI_ONB_PRIV_TELEMETRY_BODY,
                ),
                (keys::AI_ONB_PRIV_DOCS, ""),
                ("", ""),
                ("", ""),
                ("", ""),
            ];
            let mut py = pb[1] + 8.0;
            for (idx, (title_key, body_key)) in paras.iter().enumerate() {
                if title_key.is_empty() {
                    continue;
                }
                texts.push(OwnedScreenText {
                    text: self.tr(title_key).to_owned(),
                    origin: [pb[0] + 10.0, py],
                    width: pb[2] - 20.0,
                    font_size: 11.5,
                    color: palette.title,
                    align: TextAlign::Left,
                });
                py += 14.0;
                if !body_key.is_empty() {
                    texts.push(OwnedScreenText {
                        text: self.tr(body_key).to_owned(),
                        origin: [pb[0] + 10.0, py],
                        width: pb[2] - 20.0,
                        font_size: 11.0,
                        color: palette.icon,
                        align: TextAlign::Left,
                    });
                    py += 30.0;
                }
                let _ = idx;
            }
        }
        (quads, texts)
    }

    /// Оверлей палитры выделения: бар с кнопками групп (иконка + подпись),
    /// открытая hover'ом колонка (строки с иконками и подписями).
    /// Screen-space: константный размер при любом зуме.
    pub(super) fn palette_overlay(
        &self,
        lay: &PaletteLayout,
        groups: &[crate::palette::PaletteGroup],
        open: Option<usize>,
    ) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let palette = self.effective_palette();
        let tint = color_to_rgba(palette.icon);
        let title = palette.title;
        // FR-060 (волна 2 кита): заливки/подписи — Painter, состояния —
        // WidgetState (приоритет Hovered > Selected — матрица кита даёт
        // прежнюю раскраску строк дословно). Векторные иконки-квады
        // (icon_quads — SDF-композиции с параметрами вне контракта
        // PaintItem) остаются квадами и добираются после заливок:
        // пересечений у них нет — только своя кнопка/строка (0 скачка)
        let mut d = Painter::new();
        let mut icon_draws: Vec<(crate::palette::PaletteIcon, [f32; 4])> = Vec::new();
        // Фон бара
        d.rect(
            canvas_ui::geometry::UiRect::new(lay.bar[0], lay.bar[1], lay.bar[2], lay.bar[3]),
            palette.menu_fill,
            palette.palette_border,
            8.0,
        );
        for (i, group) in groups.iter().enumerate() {
            let button = lay.groups[i].button;
            let hovered = open == Some(i);
            // Кнопка группы: подсветка при наведении (выпадашка открыта) —
            // WidgetState (Hovered)
            let mut btn_state = WidgetState::default();
            btn_state.set_pointer(hovered, false);
            let btn_hovered = btn_state.kit_state() == kit::KitState::Hovered;
            d.rect(
                canvas_ui::geometry::UiRect::new(button[0], button[1], button[2], button[3]),
                if btn_hovered {
                    palette.palette_hover_fill
                } else {
                    palette.palette_chip_fill
                },
                [0.0; 4],
                6.0,
            );
            // Иконка группы (текстовый глиф — ScreenText'ом по центру)
            let icon_rect = [
                button[0] + (button[2] - PAL_ICON) / 2.0,
                button[1] + (button[3] - PAL_ICON) / 2.0,
                PAL_ICON,
                PAL_ICON,
            ];
            icon_draws.push((group.icon, icon_rect));
            if let Some(glyph) = icon_text(group.icon) {
                d.label(
                    canvas_ui::geometry::UiRect::new(
                        icon_rect[0],
                        icon_rect[1] + 2.0,
                        icon_rect[2],
                        16.0,
                    ),
                    glyph,
                    color_to_rgba(title),
                    12.0,
                    PaintAlign::Center,
                );
            }
            // Подпись группы под кнопкой (кегль — единый источник с замером
            // ширины кнопки: palette::PAL_CAPTION_FONT, CR-015)
            d.label(
                canvas_ui::geometry::UiRect::new(
                    lay.groups[i].caption[0],
                    lay.groups[i].caption[1],
                    button[2],
                    12.0,
                ),
                &group.label,
                color_to_rgba(palette.body),
                PAL_CAPTION_FONT,
                PaintAlign::Center,
            );
            // Открытая колонка (hover): фон + строки
            if hovered {
                let drop = lay.groups[i].dropdown;
                d.rect(
                    canvas_ui::geometry::UiRect::new(drop[0], drop[1], drop[2], drop[3]),
                    palette.menu_fill,
                    palette.palette_border,
                    6.0,
                );
                for (k, entry) in group.entries.iter().enumerate() {
                    let row = lay.groups[i].rows[k];
                    let row_hovered = point_in_rect(row, self.cursor);
                    // Состояние строки — WidgetState: Hovered (курсор)
                    // сильнее Selected (текущее значение) — прежняя
                    // раскраска accent/dim-accent дословно
                    let mut row_state = WidgetState::default();
                    row_state.set_pointer(row_hovered, false);
                    row_state.set_selected(entry.current);
                    let fill = match row_state.kit_state() {
                        kit::KitState::Hovered | kit::KitState::Pressed => {
                            palette.palette_selected_fill
                        }
                        kit::KitState::Selected => palette.palette_selected_dim_fill,
                        _ => [0.0; 4],
                    };
                    if fill[3] > 0.0 {
                        d.rect(
                            canvas_ui::geometry::UiRect::new(row[0], row[1], row[2], row[3]),
                            fill,
                            [0.0; 4],
                            4.0,
                        );
                    }
                    if let Some(icon) = entry.icon {
                        let icon_rect = [
                            row[0] + 5.0,
                            row[1] + (row[3] - PAL_ICON) / 2.0,
                            PAL_ICON,
                            PAL_ICON,
                        ];
                        icon_draws.push((icon, icon_rect));
                        if let Some(glyph) = icon_text(icon) {
                            d.label(
                                canvas_ui::geometry::UiRect::new(
                                    icon_rect[0],
                                    icon_rect[1] + 2.0,
                                    icon_rect[2],
                                    17.0,
                                ),
                                glyph,
                                color_to_rgba(title),
                                13.0,
                                PaintAlign::Center,
                            );
                        }
                        d.label(
                            canvas_ui::geometry::UiRect::new(
                                row[0] + 28.0,
                                row[1] + 5.0,
                                (row[2] - 32.0).max(0.0),
                                17.0,
                            ),
                            &entry.label,
                            color_to_rgba(title),
                            13.0,
                            PaintAlign::Left,
                        );
                    } else {
                        // Строка без иконки — текст по всей ширине
                        d.label(
                            canvas_ui::geometry::UiRect::new(
                                row[0] + 8.0,
                                row[1] + 5.0,
                                (row[2] - 12.0).max(0.0),
                                17.0,
                            ),
                            &entry.label,
                            color_to_rgba(title),
                            13.0,
                            PaintAlign::Left,
                        );
                    }
                }
            }
        }
        paint_items_to_band(d.take_items(), &mut instances, &mut texts);
        for (icon, rect) in icon_draws {
            instances.extend(icon_quads(icon, rect, tint, &palette));
        }
        (instances, texts)
    }

    /// Действие палитры: клик по строке выпадашки. Мутирующие действия —
    /// undo-шаг (FR-006); настройки ноды — через apply_node_setting.
    pub(super) fn apply_palette_action(&mut self, action: PaletteAction) {
        match action {
            PaletteAction::Node {
                node_index,
                setting,
            } => self.apply_node_setting(node_index, setting),
            PaletteAction::NodeColor { targets, preset } => {
                let snapshot = self.scene.canvas.clone();
                for index in targets {
                    if let Some(node) = self.scene.canvas.nodes.get_mut(index) {
                        node.color = preset.map(str::to_owned);
                    }
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
            }
            PaletteAction::NodeGroup(index) => {
                if let Some(mut group) =
                    plan_group_around(&self.scene.canvas, index, crate::ui::GROUP_PADDING)
                {
                    group.label = Some(self.tr(keys::GROUP_DEFAULT_LABEL).to_owned());
                    self.insert_group(group);
                }
            }
            PaletteAction::Layout { seed, mode } => self.apply_related_layout(seed, mode),
            PaletteAction::EdgeStyle { edge_index, style } => {
                let snapshot = self.scene.canvas.clone();
                if let Some(edge) = self.scene.canvas.edges.get_mut(edge_index) {
                    edge.style = Some(style);
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
            }
            PaletteAction::EdgeThickness {
                edge_index,
                thickness,
            } => {
                let snapshot = self.scene.canvas.clone();
                if let Some(edge) = self.scene.canvas.edges.get_mut(edge_index) {
                    edge.thickness = Some(thickness);
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
            }
            PaletteAction::EdgeColor { edge_index, preset } => {
                let snapshot = self.scene.canvas.clone();
                if let Some(edge) = self.scene.canvas.edges.get_mut(edge_index) {
                    edge.color = preset.map(str::to_owned);
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
            }
            // FR-014: тогл типа потока (Value ↔ Control) — undo-шаг +
            // живой пересчёт (downstream может потерять/обрести входы).
            // Тогл в Value, замыкающий цикл, отвергается (isError в MCP;
            // в палитре — toast с участниками). Правка 3: логика тогла —
            // в SceneState::toggle_edge_flow (мутация живого канваса,
            // тестируемо); палитра только показывает toast при цикле.
            PaletteAction::EdgeFlowKind { edge_index, kind } => {
                match self.scene.toggle_edge_flow(edge_index, kind) {
                    Err(participants) => {
                        let participants = participants.join(" → ");
                        self.show_toast(
                            self.trf(keys::TOAST_FLOW_CYCLE, &[("{participants}", &participants)]),
                        );
                        self.request_redraw();
                    }
                    Ok(true) => self.request_redraw(),
                    Ok(false) => {}
                }
            }
            // CR-008: закрепить/освободить конец связи. Закрепление —
            // WYSIWYG: в fromSide/toSide фиксируется текущая эффективная
            // сторона (что видели — то и закрепили). Undo-шаг (FR-006);
            // no-op (состояние не изменилось) шаг не копит.
            PaletteAction::EdgePortsPin {
                edge_index,
                end,
                pin,
            } => self.set_edge_port_pin(edge_index, end, pin),
            PaletteAction::EdgePortsAuto { edge_index } => {
                let Some(edge) = self.scene.canvas.edges.get(edge_index) else {
                    return;
                };
                if !edge.ports_pinned() {
                    return; // no-op — шаг не копится
                }
                let mut snapshot = self.scene.canvas.clone();
                if let Some(edge) = snapshot.edges.get_mut(edge_index) {
                    edge.clear_port_pins();
                }
                self.scene.push_undo(snapshot);
                self.scene.mark_dirty();
                self.show_toast(self.tr(keys::TOAST_PORT_AUTO));
                self.request_redraw();
            }
            // FR-019: ручной update шаблонной ноды (linked-связь):
            // expr/version/icon/color — из манифеста реестра, params — по
            // именам (совпавшие сохраняются, новые — дефолты). Один
            // undo-шаг, пересчёт потока.
            PaletteAction::TemplateUpdate { node_index } => {
                let Some(node) = self.scene.canvas.nodes.get(node_index) else {
                    return;
                };
                let Some(template) = node.template() else {
                    return;
                };
                let Some(manifest) = self.templates.find(&template.id) else {
                    return;
                };
                if manifest.version == template.version {
                    return; // no-op — шаг не копится
                }
                let mut updated = template.clone();
                updated.version = manifest.version.clone();
                updated.expr = manifest.expr.clone();
                updated.icon = manifest.icon.clone();
                updated.color = manifest.color.clone();
                // FR-023: имя шаблона тоже синхронизируется с манифестом
                // (заголовок ноды — актуальное имя из реестра).
                // FR-040 v2: имя берётся по текущему языку интерфейса.
                updated.name = Some(manifest.display_name(self.settings.language).to_owned());
                let mut params = BTreeMap::new();
                for spec in &manifest.params {
                    let value = template.params.get(&spec.name).cloned().unwrap_or(
                        canvas_core::templates::TemplateParam {
                            num: spec.default,
                            unit: spec.unit.clone(),
                        },
                    );
                    params.insert(spec.name.clone(), value);
                }
                updated.params = params;
                let snapshot = self.scene.canvas.clone();
                if let Some(node) = self.scene.canvas.nodes.get_mut(node_index) {
                    node.set_template(Some(updated));
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
                self.scene.recompute_flow();
                self.show_toast(self.trf(
                    keys::TOAST_TEMPLATE_UPDATED,
                    &[
                        ("{name}", manifest.display_name(self.settings.language)),
                        ("{version}", manifest.version.as_str()),
                    ],
                ));
                self.request_redraw();
            }
            // FR-020: «Сохранить как шаблон» — снимок template-ссылки ноды
            // в custom-манифест (~/.canvasdesk/templates/<id>/template.json).
            // Файловая операция — НЕ undo-able; реестр перечитается.
            PaletteAction::SaveAsTemplate { node_index } => {
                let Some(node) = self.scene.canvas.nodes.get(node_index) else {
                    return;
                };
                let Some(template) = node.template() else {
                    return;
                };
                let name = node.label.clone().unwrap_or_else(|| template.id.clone());
                let id = unique_custom_id(&slugify(&name), &self.templates, &self.templates_root);
                let params: Vec<canvas_core::templates::ParamSpec> = template
                    .params
                    .iter()
                    .map(|(name_param, value)| canvas_core::templates::ParamSpec {
                        name: name_param.clone(),
                        // Тип — подсказка UI: выводим по токену единицы
                        kind: infer_param_type(value.unit.as_deref()),
                        default: value.num,
                        unit: value.unit.clone(),
                        min: None,
                        max: None,
                    })
                    .collect();
                let manifest = canvas_core::templates::TemplateManifest {
                    id: id.clone(),
                    name: name.clone(),
                    name_ru: None,
                    version: "1.0.0".to_owned(),
                    category: "custom".to_owned(),
                    description: format!("Сохранено с канваса {}", self.scene.path.display()),
                    description_en: None,
                    params,
                    // FR-029: именованные выходы переезжают в custom-шаблон
                    // вместе со снимком (снапшот переживает пересохранение)
                    outputs: template.outputs.clone(),
                    expr: template.expr.clone(),
                    // FR-046: дефолтный цвет манифеста — константа данных
                    // templates::DEFAULT_TEMPLATE_COLOR (контракт FR-018)
                    color: canvas_core::templates::DEFAULT_TEMPLATE_COLOR.to_owned(),
                    icon: "custom".to_owned(),
                    source: canvas_core::templates::TemplateSource::Custom,
                };
                match canvas_core::templates::save_custom(&manifest, &self.templates_root) {
                    Ok(path) => {
                        // Перезагрузка реестра: custom появится в
                        // палитре/wheel и в MCP template_list
                        self.templates = canvas_core::templates::TemplateRegistry::all_with_custom(
                            &self.templates_root,
                        );
                        // FR-PERF-A: реестр шаблонов не входит в сигнатуру
                        // кэша UI-кадра (дорогой хэш) — явная инвалидация
                        // кэша (WHEEL/TEMPLATE_STRIP/TEMPLATE_PANEL hit-rect'ы
                        // зависят от categories/count шаблонов).
                        self.cached_ui_frame = None;
                        // FR-061 этап D (D-8): описания обновились — снять
                        // новый снимок в сцену и рендер.
                        self.sync_template_descs();
                        self.show_toast(self.trf(
                            keys::TOAST_TEMPLATE_SAVED,
                            &[("{name}", &name), ("{path}", &path.display().to_string())],
                        ));
                    }
                    Err(err) => {
                        self.show_toast(self.trf(
                            keys::TOAST_TEMPLATE_SAVE_FAILED,
                            &[("{err}", &err.to_string())],
                        ));
                    }
                }
                self.request_redraw();
            }
            // FR-042 (правка 2026-09-25): удалить весь пучок N ≥ 2 рёбер
            // одним действием из палитры связи (группа «Пучок»). Все N
            // рёбер между упорядоченной парой (from, to) удаляются; один
            // undo-шаг (FR-006); пересчёт потока (downstream теряет
            // входы). Открытый stage закрылся бы валидацией при потере
            // любого ребра среза — закрываем сразу, без фантома на кадре.
            PaletteAction::EdgeBundleDelete { edge_index } => {
                // Все N рёбер пучка по edge_index (один undo-шаг)
                let bundle_edges: Vec<usize> = self
                    .scene
                    .bundles
                    .bundle_of_edge(edge_index)
                    .map(|b| b.edges.clone())
                    .unwrap_or_else(|| vec![edge_index]);
                // Собираем id рёбер (индексы могут сдвинуться после remove)
                let ids: Vec<String> = bundle_edges
                    .iter()
                    .filter_map(|&idx| self.scene.canvas.edges.get(idx).map(|e| e.id.clone()))
                    .collect();
                if ids.is_empty() {
                    return;
                }
                self.push_undo();
                for id in &ids {
                    self.scene.canvas.remove_edge(id);
                }
                self.selected = None;
                self.close_main_stage();
                self.scene.mark_dirty();
                self.scene.recompute_flow();
                self.request_redraw();
            }
        }
    }

    /// FR-026: применить переключение булевой строки панели настроек
    /// (тумблер) и сохранить конфиг. Многозначные строки — через
    /// выпадающее меню ([`App::apply_dropdown_choice`]).
    pub(super) fn apply_toggle_row(&mut self, row: SettingsRow) {
        match row {
            SettingsRow::Grid => {
                self.settings.grid_visible = !self.settings.grid_visible;
            }
            SettingsRow::EdgesAvoid => {
                self.settings.edges_avoid_nodes = !self.settings.edges_avoid_nodes;
            }
            // FR-025: построчные точки выхода — рендер/hit-тест читают флаг
            // на кадре (SceneView.line_ports), синхронизация рендера не нужна
            SettingsRow::LinePorts => {
                self.settings.line_ports = !self.settings.line_ports;
            }
            // FR-016 (CP5): оверлей узких мест — рендер читает флаг на кадре
            // (SceneView.analysis_overlay), синхронизация рендера не нужна
            SettingsRow::BottleneckOverlay => {
                self.settings.bottleneck_overlay = !self.settings.bottleneck_overlay;
                self.bottleneck_auto_enabled = true;
            }
            // FR-038 (п.19): тумблеры магнитной раскладки — движок и рендер
            // читают флаги на кадр. Мастер-тумблер (п.5) гасит весь снаппинг
            // без сброса остальных настроек — активный предпросмотр убираем
            // сразу (drag с открытой панелью невозможен, но защита дешёвая)
            SettingsRow::SnapEnabled => {
                self.settings.snap_enabled = !self.settings.snap_enabled;
                if !self.settings.snap_enabled {
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.clear_guides();
                    }
                }
            }
            SettingsRow::SnapGrid => {
                self.settings.snap_to_grid = !self.settings.snap_to_grid;
            }
            SettingsRow::SnapGuides => {
                self.settings.snap_to_guides = !self.settings.snap_to_guides;
            }
            SettingsRow::SnapCollision => {
                self.settings.snap_collision = !self.settings.snap_collision;
            }
            // FR-073: тумблеры расталкивания — физика читает флаги на кадр
            // (тик/пайплайн драга), синхронизация рендера не нужна;
            // мастер-тумблер гасит сессию физики (как snap_enabled — гасит
            // снап) без сброса остальных настроек
            SettingsRow::DragPushEnabled => {
                self.settings.drag_push_enabled = !self.settings.drag_push_enabled;
                if !self.settings.drag_push_enabled {
                    self.drag_push_live = false;
                }
            }
            SettingsRow::DragPushPredictive => {
                self.settings.drag_push_predictive = !self.settings.drag_push_predictive;
            }
            SettingsRow::DragPushRebase => {
                self.settings.drag_push_rebase = !self.settings.drag_push_rebase;
            }
            // T23: состояние синхронно с settings — сохранение общим хвостом
            SettingsRow::FocusMode => self.toggle_focus_mode(),
            // FR-042 (F-13): агрегация рендер/ввод читают на кадр
            // (SceneView.bundles + гейты открытия stage); при выключении
            // открытый stage закрывается (инвариант согласованности)
            SettingsRow::EdgeAggregation => {
                self.settings.edge_aggregation = !self.settings.edge_aggregation;
                if !self.settings.edge_aggregation {
                    self.close_main_stage();
                    self.bundle_hover = None;
                }
            }
            // PRD-0007 (FR-048 X4, AC-5.5): тумблер фонового детектора —
            // при выключении бейдж скрывается и отложенный скан отменяется;
            // предложения остаются кэшем (не создают ничего сами — D1)
            SettingsRow::AutolinkEnabled => {
                self.settings.autolink_enabled = !self.settings.autolink_enabled;
                self.autolink_scan_due = None;
            }
            // PRD-0007 (FR-048 X6, F-12): индикатор покрытия цепочками —
            // opt-in; кэш расчёта инвалидируется при переключении
            SettingsRow::ExplainCoverage => {
                self.settings.explain_coverage = !self.settings.explain_coverage;
                self.coverage_cache = None;
            }
            // FR-079 (S3): мастер-тумблер подсказок — выключение гасит
            // накопленные ответы/карточки и отменяет дебаунс (мгновенно)
            SettingsRow::SuggestEnabled => {
                self.settings.suggest.enabled = !self.settings.suggest.enabled;
                if !self.settings.suggest.enabled {
                    self.suggest.pending = None;
                    self.suggest.answers.clear();
                    self.close_suggest_cards();
                }
            }
            SettingsRow::HudOnStart => {
                self.settings.hud_on_start = !self.settings.hud_on_start;
                // Мгновенная обратная связь: HUD переключается сразу
                self.hud_visible = self.settings.hud_on_start;
            }
            // FR-089: согласия телеметрии — флаг меняется, персист общим
            // хвостом (save_settings → persist_settings_web синхронизирует
            // canvasdesk.consent и эмитит canvasdesk:consent-changed для
            // JS-модуля index.html: PostHog opt-in/out без перезагрузки;
            // счётчик просто перестаёт отправляться). На нативе поле
            // персистится в config.toml, отправки нет.
            SettingsRow::TelemetryCounter => {
                self.settings.telemetry_counter = !self.settings.telemetry_counter;
            }
            SettingsRow::TelemetryAnalytics => {
                self.settings.telemetry_analytics = !self.settings.telemetry_analytics;
            }
            // FR-087: тумблеры категорий шаблонных нод — материализация
            // дефолта роли в явный allow-list + инверсия видимости.
            // Палитра/wheel/поиск читают флаг на кадре (единый хелпер
            // template_category_names), синхронизация рендера не нужна.
            SettingsRow::TplCatBackend => self.toggle_template_category("backend"),
            SettingsRow::TplCatNetwork => self.toggle_template_category("network"),
            SettingsRow::TplCatUnitEconomics => self.toggle_template_category("unit-economics"),
            SettingsRow::TplCatProductAnalytics => {
                self.toggle_template_category("product-analytics")
            }
            // FR-087: тумблеры категорий схем (галерея) — паттерн шаблонов.
            SettingsRow::SchemeCatArchitecture => self.toggle_scheme_category("architecture"),
            SettingsRow::SchemeCatBusiness => self.toggle_scheme_category("business"),
            SettingsRow::SchemeCatFramework => self.toggle_scheme_category("framework"),
            SettingsRow::SchemeCatPlanning => self.toggle_scheme_category("planning"),
            SettingsRow::SchemeCatOnboarding => self.toggle_scheme_category("onboarding"),
            // FR-LLM-B / PRD-0010 F-7: тумблер AI-телеметрии (Q7, default OFF).
            // `LlmSettings.telemetry_opt_in` — персист в config.toml, отправки
            // на стороне Stream C/D (posthog track из canvas_app::app::telemetry
            // гейтится этим флагом).
            SettingsRow::AiTelemetry => {
                self.settings.llm.telemetry_opt_in = !self.settings.llm.telemetry_opt_in;
            }
            // FR-087: Role — dropdown, применяется apply_dropdown_choice
            SettingsRow::Role
            | SettingsRow::ButtonCorner
            | SettingsRow::GridStyle
            | SettingsRow::GridDensity
            | SettingsRow::PortZone
            | SettingsRow::Language
            | SettingsRow::ThemePreset
            | SettingsRow::SnapTolerance
            | SettingsRow::SnapSubZoom
            | SettingsRow::SnapCoarseZoom
            // FR-073: зазор/ореол — dropdown, применяется в apply_dropdown_choice
            | SettingsRow::DragPushSafeGap
            | SettingsRow::DragPushHalo
            // PRD-0007 (AC-2.3): dropdown «Лимит глубины explain-дерева» —
            // применяется в apply_dropdown_choice, тумблером не является
            | SettingsRow::ExplainDepthLimit
            // FR-079 (S3): dropdown «Движок подсказок» — применяется в
            // apply_dropdown_choice, тумблером не является
            | SettingsRow::SuggestEngine
            // FR-ICONS: dropdown «Набор иконок» — применяется в
            // apply_dropdown_choice, тумблером не является
            | SettingsRow::IconStyle
            // FR-LLM-B: dropdown-строки таба «AI и модели» — применяются в
            // apply_dropdown_choice (apply_dropdown_value), не тумблеры
            | SettingsRow::AiProvSuggest
            | SettingsRow::AiProvGraph
            | SettingsRow::AiProvAgent
            // FR-LLM-FIX: per-feature BYOK-модель (3 dropdown'а)
            | SettingsRow::AiModelSuggest
            | SettingsRow::AiModelGraph
            | SettingsRow::AiModelAgent
            | SettingsRow::AiResidency
            | SettingsRow::AiConfidenceThreshold
            | SettingsRow::AiCostLimit
            // FR-LLM-FIX: Button-строки (API-ключ / self-hosted URL / key) —
            // применяются в apply_button_row (mock health-check), не тумблеры.
            | SettingsRow::AiApiKey
            | SettingsRow::AiSelfhostUrl
            | SettingsRow::AiSelfhostKey => {
                debug_assert!(false, "не тумблер: {row:?}");
                return;
            }
        }
        self.sync_settings_row(row);
        self.save_settings();
    }

    // FR-LLM-FIX: применить клик по Button-строке (API-ключ BYOK /
    // self-hosted URL / self-hosted key) — mock health-check, переключает
    // бейдж. Реальный health-check (/v1/models) — Stream C/D TODO,
    // помечен `// FR-LLM-FIX-TODO:`. Сохранение конфига не нужно (флаги
    // in-memory only, как и ai_paused).
    pub(super) fn apply_button_row(&mut self, row: SettingsRow) {
        match row {
            // FR-LLM-FIX: mock health-check BYOK-ключа — toggle ai_key_ok.
            // FR-LLM-FIX-TODO: реальный health-check — GET /v1/models с
            // api_key, при 200 заполнить список моделей (модель-строки
            // получат dropdown_options не пустой), при 4xx — бейдж «ключ
            // невалиден». Stream C/D.
            SettingsRow::AiApiKey => {
                self.ai_key_ok = !self.ai_key_ok;
            }
            // FR-LLM-FIX: mock health-check self-hosted endpoint — toggle
            // ai_selfhost_ok. FR-LLM-FIX-TODO: реальный health-check —
            // GET {endpoint}/v1/models с api_key, при 200 — бейдж «endpoint
            // отвечает», при ошибке — «неверный URL»/«endpoint не отвечает».
            // Stream C/D.
            SettingsRow::AiSelfhostUrl | SettingsRow::AiSelfhostKey => {
                self.ai_selfhost_ok = !self.ai_selfhost_ok;
            }
            _ => {
                debug_assert!(false, "не Button-строка: {row:?}");
                return;
            }
        }
        self.request_redraw();
    }

    /// Screen-space оверлей настроек: летающая кнопка всегда, панель — когда
    /// открыта. Координаты — логические px от левого верхнего угла окна.
    ///
    /// FR-CLIP: возвращает основную полосу (кнопки + модалка + строки) и
    /// отдельную полосу выпадающего меню (popup над модалкой, может выходить
    /// за её границы). Клипы: `clip` = вьюпорт (разрозненные элементы без
    /// единого rect), `dropdown_clip` = `menu rect` (или пустой, если
    /// меню закрыто — `push` пропустит пустую полосу).
    pub(super) fn settings_overlay(&self) -> SettingsOverlayRender {
        let mut render = SettingsOverlayRender::default();
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let mut icons = Vec::new();
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return render;
        }
        let palette = self.effective_palette();
        // Вертикальная центровка иконки: лайн-бокс высотой font*1.3 по центру
        // кнопки (так же считает рендер screen-текстов).
        let icon_top = |rect: [f32; 4], font_size: f32| rect[1] + (rect[3] - font_size * 1.3) / 2.0;
        // W-c (аудит ui-kit §10): угловой кластер ⚙/тема/язык/«?» — kit
        // IconButton: рамка/фон/hover — `icon_button_style` (Ghost-слоты
        // палитры кита, паттерн kit_gallery_overlay), состояния —
        // WidgetState (FR-057) вместо ручного hover-ярления. Сами иконки —
        // прежние (⚙ — квады: U+2699 вне покрытия шрифта; тема — квад-круг;
        // «?» — глиф кита icon_glyph(Icon::Question)).
        // Hit==draw: rect'ы lib.rs (button_rect/theme_button_rect/…) — ОДНИ
        // и те же для hit-теста реестра и отрисовки; слот кита здесь = сама
        // кнопка (ICON_BUTTON_SIZE 26 ≠ SETTINGS_BUTTON 36 — inset
        // icon_button_rect не применяется, хром рисуется на весь слот).
        let corner_palette = self.effective_palette().kit_palette();
        let corner_state = |rect: [f32; 4], selected: bool| {
            let mut widget = WidgetState::default();
            widget.set_pointer(point_in_rect(rect, self.cursor), false);
            widget.set_selected(selected);
            widget.kit_state()
        };
        let button = button_rect(self.settings.button_corner, viewport);
        // Kit-стиль кнопки: Normal — control_fill без рамки, hover — слот
        // hover_fill + accent-рамка ( Selected — подсветка панели, рамка
        // выделения — params.y ниже, слота в ките нет).
        let settings_style = canvas_ui::kit::icon_button_style(
            corner_state(button, self.settings_open),
            &corner_palette,
        );
        instances.push(CardInstance {
            pos: [button[0], button[1]],
            size: [button[2], button[3]],
            fill: settings_style.fill,
            border: settings_style.border,
            // params.y = рамка выделения: подсветка кнопки при открытой панели
            params: [
                settings_style.radius,
                self.settings_open as u8 as f32,
                0.0,
                0.0,
            ],
            corners: [0.0; 4],
        });
        // Иконка настроек — КВАДАМИ, не текстовым глифом: «⚙» (U+2699)
        // отсутствует в Noto Sans Display/Mono и рисовалась тофу-квадом
        // (wasm-аудит 2026-09-25, v2_corner). Ползунки: 2 трека + 2 ручки.
        {
            let icon = [button[0] + 9.0, button[1] + 12.0, 18.0, 12.0];
            let track_fill = color_to_rgba(palette.icon);
            for (row, knob_x) in [(0.0_f32, 4.0_f32), (7.0_f32, 10.0_f32)] {
                instances.push(CardInstance {
                    pos: [icon[0], icon[1] + row],
                    size: [icon[2], 2.0],
                    fill: track_fill,
                    border: [0.0; 4],
                    params: [1.0, 0.0, 0.0, 1.0],
                    corners: [0.0; 4],
                });
                instances.push(CardInstance {
                    pos: [icon[0] + knob_x, icon[1] + row - 2.0],
                    size: [6.0, 6.0],
                    fill: track_fill,
                    border: [0.0; 4],
                    params: [3.0, 0.0, 0.0, 1.0],
                    corners: [0.0; 4],
                });
            }
        }
        // Кнопка переключения темы — рядом с кнопкой настроек (в тот же угол).
        // Иконка показывает ЦЕЛЬ: в тёмной теме «солнце» (клик — светлая).
        // W-c: хром — kit IconButton (icon_button_style), состояние — WidgetState.
        let theme_button = theme_button_rect(self.settings.button_corner, viewport);
        let theme_style =
            canvas_ui::kit::icon_button_style(corner_state(theme_button, false), &corner_palette);
        instances.push(CardInstance {
            pos: [theme_button[0], theme_button[1]],
            size: [theme_button[2], theme_button[3]],
            fill: theme_style.fill,
            border: theme_style.border,
            params: [theme_style.radius, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        // Иконка темы — квадами: «☀»/«🌙» (U+2600/U+1F319) тоже вне
        // покрытия шрифтов (тофу). Показ цели: тёмная тема — залитый круг
        // («переключить на светлый»), светлая — контурный.
        let theme_circle_fill = match self.settings.theme {
            Theme::Dark => color_to_rgba(palette.title),
            Theme::Light => [0.0; 4],
        };
        instances.push(CardInstance {
            pos: [theme_button[0] + 10.0, theme_button[1] + 10.0],
            size: [16.0, 16.0],
            fill: theme_circle_fill,
            border: color_to_rgba(palette.title),
            params: [8.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        // FR-040 v2: кнопка переключения языка (RU/EN) — третий элемент
        // кластера (⚙ → ☼ → RU/EN → ?). Иконка — короткий код активного
        // языка (2 буквы), чтобы не зависеть от покрытия шрифтов глифами
        // эмодзи. W-c: kit-стиль кнопки с текстовой меткой (цвет текста —
        // слот style.text кита), состояние — WidgetState.
        let language_button = language_button_rect(self.settings.button_corner, viewport);
        let language_style = canvas_ui::kit::icon_button_style(
            corner_state(language_button, false),
            &corner_palette,
        );
        instances.push(CardInstance {
            pos: [language_button[0], language_button[1]],
            size: [language_button[2], language_button[3]],
            fill: language_style.fill,
            border: language_style.border,
            params: [language_style.radius, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        texts.push(OwnedScreenText {
            // Код языка — на языке самого языка («RU»/«EN»), конвенция
            // FR-040 §4 (названия в переключателе — на языке самого языка).
            text: match self.settings.language {
                canvas_core::Language::Ru => "RU".to_owned(),
                canvas_core::Language::En => "EN".to_owned(),
            },
            origin: [language_button[0], icon_top(language_button, 13.0)],
            width: language_button[2],
            font_size: 13.0,
            color: palette.title,
            align: TextAlign::Center,
        });
        // FR-027: кнопка «?» — четвёртый элемент кластера (⚙/тема/язык/помощь):
        // вход в меню документации и онбординга. W-c: хром — kit IconButton,
        // глиф — кит icon_glyph(Icon::Question) (та же «?»), цвет — слот
        // style.text; открытому меню — KitState::Selected + рамка выделения.
        let help_button = help_button_rect(self.settings.button_corner, viewport);
        let help_style = canvas_ui::kit::icon_button_style(
            corner_state(help_button, self.help_menu.is_some()),
            &corner_palette,
        );
        instances.push(CardInstance {
            pos: [help_button[0], help_button[1]],
            size: [help_button[2], help_button[3]],
            fill: help_style.fill,
            border: help_style.border,
            params: [
                help_style.radius,
                self.help_menu.is_some() as u8 as f32,
                0.0,
                0.0,
            ],
            corners: [0.0; 4],
        });
        texts.push(OwnedScreenText {
            text: canvas_ui::kit::icon_glyph(canvas_ui::kit::Icon::Question).to_owned(),
            origin: [help_button[0], icon_top(help_button, 18.0)],
            width: help_button[2],
            font_size: 18.0,
            color: palette.title,
            align: TextAlign::Center,
        });
        // Панель горячих клавиш (FR-004): у левого края, по центру;
        // рендерится независимо от панели настроек
        if self.hotkeys_open {
            // Фикс среза 2026-09-25 (wasm-аудит, скриншот 11_hotkeys):
            // константная ширина 340 рвала самое длинное описание
            // («…режим защиты: раскрыть следующий уровень») у кромки —
            // панель ДОТЯГИВАЕТСЯ до самого длинного описания
            // (измерение тем же лицом, что рисует строки).
            // W-a (аудит §8 п.6): измерение вынесено в общую функцию
            // ui_registry::hotkeys_panel_rect — тот же rect у hit-теста
            // реестра («ввод = тому, что видно»).
            // left_offset ДО захвата FontSystem: hotkeys_left_offset сам
            // захватывает этот Mutex (меряет полосу доков) — вложенный
            // захват неповторно-входящего Mutex = самозаблокировка.
            let left = self.hotkeys_left_offset(viewport);
            let mut m = canvas_ui::measure::TextMeasurer::new();
            let mut fs = canvas_render::text::measure_font_system();
            let panel = ui_registry::hotkeys_panel_rect(
                viewport,
                left,
                self.settings.button_corner,
                &mut m,
                &mut fs,
                |k| self.tr(k),
            );
            instances.push(CardInstance {
                pos: [panel[0], panel[1]],
                size: [panel[2], panel[3]],
                fill: palette.menu_fill,
                border: [0.0; 4],
                params: [8.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            let pad = crate::ui::HOTKEYS_PADDING;
            let header_h = crate::ui::HOTKEYS_HEADER_HEIGHT;
            let row_h = crate::ui::HOTKEYS_ROW_HEIGHT;
            let key_w = crate::ui::HOTKEYS_KEY_COLUMN;
            let key_x = panel[0] + pad;
            let desc_x = panel[0] + pad + key_w;
            let desc_w = (panel[2] - pad * 2.0 - key_w).max(10.0);
            texts.push(OwnedScreenText {
                text: self.tr(keys::HOTKEYS_TITLE).to_owned(),
                origin: [key_x, panel[1] + pad + 7.0],
                width: panel[2] - pad * 2.0,
                font_size: 15.0,
                color: palette.title,
                align: TextAlign::Left,
            });
            // FR-060: строки — `kit::list_rows` (однородные HOTKEYS_ROW_HEIGHT
            // с зазором 0; прежняя стопка «pad + header + i·row_h» дословно).
            // Прежний break-кламп «строки ниже кромки −2 не рисуем» (G5 —
            // молчаливый срез частичных строк) заменён окном кита: частичные
            // строки у кромки клипует scissor-полоса (FR-056), срезов нет.
            let rows_area = canvas_ui::geometry::UiRect::new(
                key_x,
                panel[1] + pad + header_h,
                (panel[2] - pad * 2.0).max(0.0),
                (panel[1] + panel[3] - 2.0 - (panel[1] + pad + header_h)).max(0.0),
            );
            let hk_scroll = canvas_ui::kit::ScrollState {
                offset: 0.0,
                content_h: crate::ui::HOTKEYS.len() as f32 * row_h,
                viewport_h: rows_area.h,
            };
            for (i, rect) in canvas_ui::kit::list_rows(
                rows_area,
                &hk_scroll,
                row_h,
                0.0,
                crate::ui::HOTKEYS.len(),
            ) {
                let Some((key, description)) = crate::ui::HOTKEYS.get(i) else {
                    continue;
                };
                let y = rect.y + 3.0;
                texts.push(OwnedScreenText {
                    text: self.tr(key).to_owned(),
                    origin: [key_x, y],
                    width: key_w,
                    font_size: 12.0,
                    color: palette.link,
                    align: TextAlign::Left,
                });
                texts.push(OwnedScreenText {
                    text: self.tr(description).to_owned(),
                    origin: [desc_x, y],
                    width: desc_w,
                    font_size: 12.0,
                    color: palette.body,
                    align: TextAlign::Left,
                });
            }
        }
        if !self.settings_open {
            // FR-CLIP: модалка закрыта — основная полоса это только летающие
            // кнопки (угол), клип не сужаем (вьюпорт). Дропдаун-полоса пуста.
            render.instances = instances;
            render.texts = texts;
            render.icons = icons;
            render.clip = canvas_ui::UiRect::new(0.0, 0.0, viewport[0], viewport[1]);
            return render;
        }
        // FR-039: затемнение канваса под модалкой (паттерн онбординга
        // FR-028) — фокус на диалоге настроек, ввод под ним глушится
        instances.push(CardInstance {
            pos: [0.0, 0.0],
            size: [viewport[0], viewport[1]],
            // FR-070 (W-d): слот `stage_dim` темы (бывший инлайн-литерал
            // `[0.02, 0.02, 0.04, 0.85]` — alpha 0.6 канонич. коридора
            // PRD 0.55–0.65, не переопределяем — single source of truth).
            fill: palette.stage_dim,
            border: [0.0; 4],
            params: [0.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        // FR-039: модалка по центру — layout несёт rect'ы навигации,
        // заголовка раздела, строк активного таба и карточек темы.
        // W-a: контент правой панели — со скроллом (тот же offset, что
        // у hit-тестов ui_registry и pick input.rs — «ввод = тому, что
        // видно»); навигация/заголовок/карточки тем неподвижны.
        // FR-LLM-FIX: передаём настройки для фильтрации AI-таба (BYOK-
        // модель видна только когда провайдер = BYOK).
        let layout = modal_layout_scrolled_with_settings(
            self.settings_tab,
            viewport,
            self.settings_scroll_top,
            &self.settings,
        );
        let modal = layout.rect;
        instances.push(CardInstance {
            pos: [modal[0], modal[1]],
            size: [modal[2], modal[3]],
            fill: palette.menu_fill,
            border: [0.0; 4],
            params: [8.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        // Навигация: Desktop — левая колонка «иконка + название» (Obsidian);
        // компакт/мобайл (W-e) — горизонтальный таб-бар: иконка по центру
        // слота, имя активного раздела читается в заголовке ниже. Активный
        // раздел — акцентная подложка, неактивные — hover-подсветка
        // (заливки/слоты общие — различается только посадка иконки/подписи).
        let compact_tabs = layout.mode != ModalMode::Desktop;
        let tab_index = self.settings_tab.min(SETTINGS_TABS.len() - 1);
        for (i, tab) in SETTINGS_TABS.iter().enumerate() {
            let Some(item) = layout.nav_items.get(i) else {
                continue;
            };
            let active = i == tab_index;
            let hovered = point_in_rect(*item, self.cursor);
            if active {
                instances.push(CardInstance {
                    pos: [item[0] + 4.0, item[1] + 2.0],
                    size: [item[2] - 8.0, item[3] - 4.0],
                    fill: palette.palette_selected_fill,
                    border: [0.0; 4],
                    params: [6.0, 0.0, 0.0, 1.0],
                    corners: [0.0; 4],
                });
            } else if hovered {
                instances.push(CardInstance {
                    pos: [item[0] + 4.0, item[1] + 2.0],
                    size: [item[2] - 8.0, item[3] - 4.0],
                    fill: palette.palette_hover_fill,
                    border: [0.0; 4],
                    params: [6.0, 0.0, 0.0, 1.0],
                    corners: [0.0; 4],
                });
            }
            // FR-ICONS: иконка таба — SVG если активен набор, иначе глиф.
            // Ревизия владельца 2026-10-02: «Драг» и «Подсказки» не были
            // смаплены (_ => "") — рисовал Glyph-фолбэк ✥/✦, отсутствующий
            // во встроенных шрифтах (тофу в wasm).
            let icon_name = match tab.title_key {
                keys::TAB_GENERAL => "tab_general",
                keys::TAB_CANVAS => "tab_canvas",
                keys::TAB_SNAP => "tab_snap",
                keys::TAB_DRAG => "tab_drag",
                keys::TAB_EDGES => "tab_edges",
                keys::TAB_SUGGEST => "tab_suggest",
                keys::TAB_APPEARANCE => "tab_appearance",
                _ => "",
            };
            let icon_color = if active { palette.link } else { palette.icon };
            let icon_tint = crate::palette::color_to_rgba(icon_color);
            let icon_set = self.icon_set_active();
            // Посадка иконки: Desktop — слева колонки (отступ 12, слот 20);
            // компакт/мобайл — по центру слота таб-бара. Глиф-фолбэк —
            // той же посадке (центр слота / колонка).
            let icon_size = 16.0f32.min(item[3] - 4.0);
            let icon_y = item[1] + (item[3] - icon_size) / 2.0;
            let glyph_y = item[1] + (item[3] - 14.0 * 1.3) / 2.0;
            let (icon_x, glyph_origin, glyph_width, glyph_align) = if compact_tabs {
                (
                    item[0] + (item[2] - icon_size) / 2.0,
                    [item[0], glyph_y],
                    item[2],
                    TextAlign::Center,
                )
            } else {
                (
                    item[0] + 12.0 + (20.0 - icon_size) / 2.0,
                    [item[0] + 12.0, glyph_y],
                    20.0,
                    TextAlign::Left,
                )
            };
            if icon_set.is_some() && !icon_name.is_empty() {
                if let Some(set) = icon_set {
                    if let Some((uv_min, uv_max)) = canvas_render::icon_uv(set, icon_name) {
                        icons.push(canvas_render::IconInstance {
                            pos: [icon_x, icon_y],
                            size: [icon_size, icon_size],
                            uv_min,
                            uv_max,
                            tint: icon_tint,
                        });
                    } else {
                        texts.push(OwnedScreenText {
                            text: tab.icon.to_owned(),
                            origin: glyph_origin,
                            width: glyph_width,
                            font_size: 14.0,
                            color: icon_color,
                            align: glyph_align,
                        });
                    }
                }
            } else {
                texts.push(OwnedScreenText {
                    text: tab.icon.to_owned(),
                    origin: glyph_origin,
                    width: glyph_width,
                    font_size: 14.0,
                    color: icon_color,
                    align: glyph_align,
                });
            }
            // Название раздела — только в левой колонке (Desktop); в
            // компакт/мобайл имя активного раздела читается в заголовке
            // под таб-баром (слоты ~62px не вмещают подписи без эвристик).
            if !compact_tabs {
                texts.push(OwnedScreenText {
                    text: self.tr(tab.title_key).to_owned(),
                    origin: [item[0] + 36.0, item[1] + (item[3] - 13.0 * 1.3) / 2.0],
                    width: item[2] - 36.0 - 6.0,
                    font_size: 13.0,
                    color: if active { palette.title } else { palette.body },
                    align: TextAlign::Left,
                });
            }
        }
        // Подсказка внизу левой колонки (перенос из подвала панели FR-026);
        // компакт/мобайл — левой колонки нет (hint_rect пуст).
        if !compact_tabs {
            texts.push(OwnedScreenText {
                text: self.tr(keys::SETTINGS_HINT).to_owned(),
                origin: [layout.hint_rect[0], layout.hint_rect[1] + 4.0],
                width: layout.hint_rect[2],
                font_size: 11.0,
                color: palette.icon,
                align: TextAlign::Left,
            });
        }
        // Заголовок раздела (правая панель)
        let tab_def = &SETTINGS_TABS[tab_index];
        texts.push(OwnedScreenText {
            text: self.tr(tab_def.title_key).to_owned(),
            origin: [layout.title_rect[0], layout.title_rect[1] + 3.0],
            width: layout.title_rect[2],
            font_size: 16.0,
            color: palette.title,
            align: TextAlign::Left,
        });
        // FR-026/FR-039: открытое выпадающее меню — геометрия и пункты
        // (состояние не хранит список — вычисляется из настроек, устареть
        // не может); ширина меню = ширине контрола строки (FR-039 §1)
        let menu = self.settings_dropdown.open_row.map(|row| {
            let items = dropdown_options(row, &self.settings);
            let anchor = layout
                .row_rect(row)
                .map(|rect| control_rect(rect, RowKind::Dropdown))
                .unwrap_or([0.0; 4]);
            let rect = dropdown_layout(anchor, viewport, items.len(), anchor[2]);
            (row, items, rect)
        });
        // Карточки темы (таб «Внешний вид», паттерн Obsidian «Base theme»):
        // активная — акцентная рамка; клик — прямой выбор (логика кнопки
        // ☀/🌙). params.y = рамка выделения (паттерн кнопки ⚙)
        if tab_def.theme_cards {
            for (theme, label_key) in [
                (Theme::Dark, keys::THEME_DARK),
                (Theme::Light, keys::THEME_LIGHT),
            ] {
                let card = layout.theme_card_rect(theme);
                // FR-047: карточка отмечена только при БЕЗАКТИВНОМ пресете
                // (пресет перекрывает классику — отметка была бы ложной)
                let selected =
                    self.settings.theme_preset.is_empty() && self.settings.theme == theme;
                let hovered = point_in_rect(card, self.cursor);
                instances.push(CardInstance {
                    pos: [card[0], card[1]],
                    size: [card[2], card[3]],
                    fill: if selected {
                        palette.palette_selected_fill
                    } else if hovered {
                        palette.palette_hover_fill
                    } else {
                        palette.palette_row_fill
                    },
                    border: if selected {
                        color_to_rgba(palette.link)
                    } else {
                        palette.palette_border
                    },
                    params: [8.0, selected as u8 as f32, 0.0, 1.0],
                    corners: [0.0; 4],
                });
                texts.push(OwnedScreenText {
                    text: self.tr(label_key).to_owned(),
                    origin: [card[0], card[1] + card[3] / 2.0 - 8.5],
                    width: card[2],
                    font_size: 13.0,
                    color: palette.title,
                    align: TextAlign::Center,
                });
            }
        }
        // Строки единой сетки: лейбл (БЕЗ значения) + описание приглушённым
        // кеглем, контрол справа — pill-тумблер или dropdown-кнопка.
        // CR-018 v1.1 (репро владельца 2026-10-04): строки НЕ прячем при
        // открытом dropdown-меню. Прежний скип («строки, перекрытые меню,
        // не рисуем») остался от эпохи, когда меню рисовалось в той же
        // полосе; с FR-CLIP меню живёт в ОТДЕЛЬНОЙ полосе Popups поверх
        // модалки (UiLayer ascending → фон меню покрывает тексты строк
        // под ним), а скип целиком прятал ЛЕЙБЛЫ строк, чей полноширинный
        // rect задевал узкое меню справа — таб «Общие» выглядел пустым
        // (2 строки × пересечение с меню = контент исчезал целиком).
        for (row, rect) in &layout.rows {
            // Hover-подсветка кликабельной строки (аффорданс)
            if point_in_rect(*rect, self.cursor) {
                instances.push(CardInstance {
                    pos: [rect[0], rect[1] + 1.0],
                    size: [rect[2], rect[3] - 2.0],
                    fill: [0.24, 0.30, 0.42, 0.35],
                    border: [0.0; 4],
                    params: [4.0, 0.0, 0.0, 1.0],
                    corners: [0.0; 4],
                });
            }
            texts.push(OwnedScreenText {
                text: self.tr(row_label_key(*row)).to_owned(),
                origin: [rect[0] + 2.0, rect[1] + 4.0],
                width: rect[2] - MODAL_ROW_LABEL_W,
                font_size: 13.0,
                color: palette.body,
                align: TextAlign::Left,
            });
            // Фикс среза описаний 2026-09-25 (wasm-аудит 15/15b/04): текст
            // «В каком углу экрана прижата лета|» рвался границей текст-арии
            // у dropdown'а — описание переносится (до 2 строк, тем же
            // кеглем; высота строки MODAL_ROW_HEIGHT 52 — третья строка не
            // влезает). Дефект №15 (закрыт W-f, 04.10.2026): хвост больше
            // не теряется молча — на второй строке индикатор «ещё N» (i18n).
            //
            // FR-LLM-FIX: Button-строки (API-ключ / self-hosted URL / key)
            // описание НЕ рисуем — на его месте рисуем текстовое поле и
            // бейдж (см. ветку RowKind::Button ниже). Лейбл остаётся сверху.
            // FR-LLM-FIX (task FIX-TEXT-INPUT): TextInput-строки (модель /
            // API-ключ / URL / self-hosted key) — тот же паттерн: на месте
            // описания рисуем текстовое поле (+ бейдж для строк с кнопкой).
            if !matches!(row_kind(*row), RowKind::Button | RowKind::TextInput) {
                let desc = self.tr(row_desc_key(*row));
                let mut m = canvas_ui::measure::TextMeasurer::new();
                let mut fs = canvas_render::text::measure_font_system();
                let desc_w = (rect[2] - MODAL_ROW_LABEL_W).max(10.0);
                for (line_idx, line) in wrap_lines_with_more(
                    &mut m,
                    &mut fs,
                    self.settings.language,
                    keys::SETTINGS_DESC_MORE,
                    desc,
                    11.0,
                    desc_w,
                    2,
                )
                .into_iter()
                .enumerate()
                {
                    texts.push(OwnedScreenText {
                        text: line,
                        origin: [rect[0] + 2.0, rect[1] + 22.0 + line_idx as f32 * 14.0],
                        width: desc_w,
                        font_size: 11.0,
                        color: palette.icon,
                        align: TextAlign::Left,
                    });
                }
            }
            let kind = row_kind(*row);
            let control = control_rect(*rect, kind);
            match kind {
                RowKind::Toggle => {
                    let on = match row {
                        SettingsRow::Grid => self.settings.grid_visible,
                        SettingsRow::EdgesAvoid => self.settings.edges_avoid_nodes,
                        SettingsRow::LinePorts => self.settings.line_ports,
                        SettingsRow::BottleneckOverlay => self.settings.bottleneck_overlay,
                        // FR-038 (п.19): тумблеры магнитной раскладки
                        SettingsRow::SnapEnabled => self.settings.snap_enabled,
                        SettingsRow::SnapGrid => self.settings.snap_to_grid,
                        SettingsRow::SnapGuides => self.settings.snap_to_guides,
                        SettingsRow::SnapCollision => self.settings.snap_collision,
                        SettingsRow::FocusMode => self.settings.focus_mode,
                        SettingsRow::EdgeAggregation => self.settings.edge_aggregation,
                        // PRD-0007 (X4, AC-5.5): тумблер фонового детектора
                        SettingsRow::AutolinkEnabled => self.settings.autolink_enabled,
                        // PRD-0007 (X6, F-12): индикатор покрытия цепочками
                        SettingsRow::ExplainCoverage => self.settings.explain_coverage,
                        // FR-079 (S3): мастер-тумблер подсказок
                        SettingsRow::SuggestEnabled => self.settings.suggest.enabled,
                        SettingsRow::HudOnStart => self.settings.hud_on_start,
                        // FR-073: тумблеры расталкивания
                        SettingsRow::DragPushEnabled => self.settings.drag_push_enabled,
                        SettingsRow::DragPushPredictive => self.settings.drag_push_predictive,
                        SettingsRow::DragPushRebase => self.settings.drag_push_rebase,
                        // FR-089: согласия телеметрии — прямые bool-поля
                        SettingsRow::TelemetryCounter => self.settings.telemetry_counter,
                        SettingsRow::TelemetryAnalytics => self.settings.telemetry_analytics,
                        // FR-087: видимость категорий подсказок по роли/фильтру
                        SettingsRow::TplCatBackend => {
                            self.settings.template_category_visible("backend")
                        }
                        SettingsRow::TplCatNetwork => {
                            self.settings.template_category_visible("network")
                        }
                        SettingsRow::TplCatUnitEconomics => {
                            self.settings.template_category_visible("unit-economics")
                        }
                        SettingsRow::TplCatProductAnalytics => {
                            self.settings.template_category_visible("product-analytics")
                        }
                        SettingsRow::SchemeCatArchitecture => {
                            self.settings.scheme_category_visible("architecture")
                        }
                        SettingsRow::SchemeCatBusiness => {
                            self.settings.scheme_category_visible("business")
                        }
                        SettingsRow::SchemeCatFramework => {
                            self.settings.scheme_category_visible("framework")
                        }
                        SettingsRow::SchemeCatPlanning => {
                            self.settings.scheme_category_visible("planning")
                        }
                        SettingsRow::SchemeCatOnboarding => {
                            self.settings.scheme_category_visible("onboarding")
                        }
                        // FR-LLM-B / PRD-0010 F-7: тумблер AI-телеметрии —
                        // читаем `LlmSettings.telemetry_opt_in` (default OFF).
                        SettingsRow::AiTelemetry => self.settings.llm.telemetry_opt_in,
                        SettingsRow::ButtonCorner
                        | SettingsRow::GridStyle
                        | SettingsRow::GridDensity
                        | SettingsRow::PortZone
                        | SettingsRow::Language
                        | SettingsRow::ThemePreset
                        | SettingsRow::SnapTolerance
                        | SettingsRow::SnapSubZoom
                        | SettingsRow::SnapCoarseZoom
                        | SettingsRow::DragPushSafeGap
                        | SettingsRow::DragPushHalo
                        // PRD-0007: dropdown-строка в ветку Toggle не
                        // попадает (row_kind = Dropdown), arm — для полноты
                        | SettingsRow::ExplainDepthLimit
                        // FR-ICONS: dropdown-строка (row_kind = Dropdown)
                        | SettingsRow::IconStyle
                        // FR-079 (S3): dropdown-строка (row_kind = Dropdown)
                        // FR-087: Role — тоже dropdown (в ветку не попадает)
                        | SettingsRow::SuggestEngine
                        // FR-LLM-B: dropdown-строки таба «AI и модели» —
                        // в ветку Toggle не попадают (row_kind = Dropdown),
                        // arm — для полноты match
                        | SettingsRow::AiProvSuggest
                        | SettingsRow::AiProvGraph
                        | SettingsRow::AiProvAgent
                        // FR-LLM-FIX: per-feature BYOK-модель (3 dropdown'а)
                        | SettingsRow::AiModelSuggest
                        | SettingsRow::AiModelGraph
                        | SettingsRow::AiModelAgent
                        | SettingsRow::AiResidency
                        | SettingsRow::AiConfidenceThreshold
                        | SettingsRow::AiCostLimit
                        // FR-LLM-FIX: Button-строки (row_kind = Button) — в
                        // ветку Toggle не попадают, arm — для полноты match.
                        | SettingsRow::AiApiKey
                        | SettingsRow::AiSelfhostUrl
                        | SettingsRow::AiSelfhostKey
                        | SettingsRow::Role => false,
                    };
                    // Pill-тумблер: трек (включён — акцент) + ручка-квад,
                    // позиция отражает значение (рисуется квадами)
                    instances.push(CardInstance {
                        pos: [control[0], control[1]],
                        size: [control[2], control[3]],
                        fill: if on {
                            color_to_rgba(palette.link)
                        } else {
                            [0.30, 0.33, 0.40, 0.9]
                        },
                        border: [0.0; 4],
                        params: [control[3] / 2.0, 0.0, 0.0, 1.0],
                        corners: [0.0; 4],
                    });
                    let knob = pill_knob_rect(control, on);
                    instances.push(CardInstance {
                        pos: [knob[0], knob[1]],
                        size: [knob[2], knob[3]],
                        fill: [0.92, 0.92, 0.94, 1.0],
                        border: [0.0; 4],
                        params: [knob[2] / 2.0, 0.0, 0.0, 1.0],
                        corners: [0.0; 4],
                    });
                }
                RowKind::Dropdown => {
                    // Кнопка со значением + ▾ (HIG «Pop-Up Buttons»)
                    instances.push(CardInstance {
                        pos: [control[0], control[1]],
                        size: [control[2], control[3]],
                        fill: if point_in_rect(control, self.cursor) {
                            hover_fill(palette.palette_chip_fill)
                        } else {
                            palette.palette_chip_fill
                        },
                        border: palette.palette_border,
                        params: [6.0, 0.0, 0.0, 1.0],
                        corners: [0.0; 4],
                    });
                    if let Some(value) = dropdown_value(*row, &self.settings) {
                        texts.push(OwnedScreenText {
                            text: value,
                            origin: [control[0] + 10.0, control[1] + 4.0],
                            width: control[2] - 26.0,
                            font_size: 12.0,
                            color: palette.title,
                            align: TextAlign::Left,
                        });
                    }
                    texts.push(OwnedScreenText {
                        text: "▾".to_owned(),
                        origin: [control[0] + control[2] - 16.0, control[1] + 4.0],
                        width: 14.0,
                        font_size: 12.0,
                        color: palette.icon,
                        align: TextAlign::Left,
                    });
                }
                // FR-LLM-FIX: Button-строка (API-ключ BYOK / self-hosted
                // URL / self-hosted key) — текстовое поле + бейдж на месте
                // описания (под лейблом), кнопка «Проверить ключ»/«Проверить»
                // справа как dropdown-контрол. Клик по кнопке триггерит mock
                // health-check и переключает бейдж (реальный health-check —
                // Stream C/D TODO, помечен `// FR-LLM-FIX-TODO:`).
                RowKind::Button => {
                    // Контрол справа — кнопка «Проверить ключ»/«Проверить».
                    let button_label = match row {
                        SettingsRow::AiApiKey => self.tr(keys::AI_BTN_CHECK_KEY).to_owned(),
                        SettingsRow::AiSelfhostUrl | SettingsRow::AiSelfhostKey => {
                            self.tr(keys::AI_BTN_CHECK).to_owned()
                        }
                        // FR-LLM-FIX: другие Button-строки (если появятся) —
                        // единая подпись «Проверить»; match exhaustive.
                        _ => self.tr(keys::AI_BTN_CHECK).to_owned(),
                    };
                    instances.push(CardInstance {
                        pos: [control[0], control[1]],
                        size: [control[2], control[3]],
                        fill: if point_in_rect(control, self.cursor) {
                            hover_fill(palette.palette_chip_fill)
                        } else {
                            palette.palette_chip_fill
                        },
                        border: palette.palette_border,
                        params: [6.0, 0.0, 0.0, 1.0],
                        corners: [0.0; 4],
                    });
                    texts.push(OwnedScreenText {
                        text: button_label,
                        origin: [control[0] + 6.0, control[1] + 4.0],
                        width: control[2] - 12.0,
                        font_size: 11.0,
                        color: palette.title,
                        align: TextAlign::Center,
                    });
                    // FR-LLM-FIX: текстовое поле + бейдж на месте описания
                    // (y = rect[1] + 22, ширина = rect[2] - MODAL_ROW_LABEL_W).
                    // Поле — слева, бейдж — правее; между ними зазор gap.
                    // FR-LLM-FIX (task FIX-TEXTINPUT-KIT): высота поля —
                    // `kit::TEXT_FIELD_HEIGHT` (30.0) вместо литерала 16.0
                    // (единая метрика кита, паритет с кнопками/dropdown'ами).
                    let field_y = rect[1] + 22.0;
                    let field_h = canvas_ui::kit::TEXT_FIELD_HEIGHT;
                    let avail_w = (rect[2] - MODAL_ROW_LABEL_W).max(40.0);
                    let badge_w = 110.0;
                    let gap = 6.0;
                    let field_x = rect[0] + 2.0;
                    let field_w = (avail_w - badge_w - gap).max(40.0);
                    instances.push(CardInstance {
                        pos: [field_x, field_y],
                        size: [field_w, field_h],
                        fill: palette.palette_row_fill,
                        border: palette.palette_border,
                        params: [4.0, 0.0, 0.0, 1.0],
                        corners: [0.0; 4],
                    });
                    // FR-070 (W-d): 5 бейджей success (AiApiKey, AiSelfhostUrl,
                    // AiSelfhostKey — дважды: кнопка + текст-поле) — цвет
                    // `palette.control_success` (бывший инлайн `Color::rgba(77,
                    // 191, 140, 255)` — тождество: round(0.30·255)=77,
                    // round(0.75·255)=191, round(0.55·255)=140, alpha 255 →
                    // ноль визуального скачка I-1, но теперь через слот темы).
                    let (field_text, badge_text, badge_color) = match row {
                        SettingsRow::AiApiKey => {
                            // FR-LLM-FIX: masked ключ — «•» длиной как ключ
                            // (или плейсхолдер «sk-…» если ключ пустой).
                            let key = &self.settings.llm.api_key;
                            let shown = if key.is_empty() {
                                "sk-…".to_owned()
                            } else {
                                "•".repeat(key.chars().count().min(20))
                            };
                            let badge = if self.ai_key_ok {
                                // FR-LLM-FIX-TODO: реальный health-check заменит
                                // «0» на число моделей из /v1/models (Stream C/D).
                                self.tr(keys::AI_KEY_VALID).replace("{n}", "0")
                            } else {
                                self.tr(keys::AI_KEY_NOT_CHECKED).to_owned()
                            };
                            let color: canvas_render::Color = if self.ai_key_ok {
                                crate::kit_ui::color4(palette.control_success)
                            } else {
                                palette.icon
                            };
                            (shown, badge, color)
                        }
                        SettingsRow::AiSelfhostUrl => {
                            // FR-LLM-FIX: URL endpoint'а или плейсхолдер.
                            let url = &self.settings.llm.endpoint;
                            let shown = if url.is_empty() {
                                "https://llm.corp.local/v1".to_owned()
                            } else {
                                url.clone()
                            };
                            let badge = if self.ai_selfhost_ok {
                                self.tr(keys::AI_SH_OK).to_owned()
                            } else {
                                self.tr(keys::AI_SH_NOT_CHECKED).to_owned()
                            };
                            let color: canvas_render::Color = if self.ai_selfhost_ok {
                                crate::kit_ui::color4(palette.control_success)
                            } else {
                                palette.icon
                            };
                            (shown, badge, color)
                        }
                        SettingsRow::AiSelfhostKey => {
                            // FR-LLM-FIX: masked ключ endpoint'а (поле
                            // переиспользует api_key, как в прототипе — один
                            // ключ на endpoint); плейсхолдер если пустой.
                            let key = &self.settings.llm.api_key;
                            let shown = if key.is_empty() {
                                "API key endpoint'а".to_owned()
                            } else {
                                "•".repeat(key.chars().count().min(20))
                            };
                            let badge = if self.ai_selfhost_ok {
                                self.tr(keys::AI_SH_OK).to_owned()
                            } else {
                                self.tr(keys::AI_SH_NOT_CHECKED).to_owned()
                            };
                            let color: canvas_render::Color = if self.ai_selfhost_ok {
                                crate::kit_ui::color4(palette.control_success)
                            } else {
                                palette.icon
                            };
                            (shown, badge, color)
                        }
                        // FR-LLM-FIX: другие Button-строки (если появятся) —
                        // пустое поле/бейдж; match exhaustive.
                        _ => (String::new(), String::new(), palette.icon),
                    };
                    if !field_text.is_empty() {
                        texts.push(OwnedScreenText {
                            text: field_text,
                            origin: [field_x + 8.0, field_y + 2.0],
                            width: field_w - 12.0,
                            font_size: 11.0,
                            color: palette.body,
                            align: TextAlign::Left,
                        });
                    }
                    if !badge_text.is_empty() {
                        texts.push(OwnedScreenText {
                            text: badge_text,
                            origin: [field_x + field_w + gap, field_y + 2.0],
                            width: badge_w,
                            font_size: 10.5,
                            color: badge_color,
                            align: TextAlign::Left,
                        });
                    }
                }
                // FR-LLM-FIX (task FIX-TEXT-INPUT): TextInput-строки —
                // редактируемое текстовое поле (имя модели / API-ключ / URL /
                // self-hosted ключ). Для модель-строк — только поле на всю
                // ширину (кнопки/бейджа нет); для AiApiKey/AiSelfhostUrl/
                // AiSelfhostKey — поле + кнопка «Проверить» + бейдж (как
                // RowKind::Button выше). При фокусе (settings_text_edit ==
                // Some(row)) — рамка акцентом + каретка «|» в конце текста.
                //
                // FR-LLM-FIX (task FIX-TEXTINPUT-KIT): вёрстка поля —
                // `kit::text_field()` (FR-055): `layout.rect` — контейнер
                // (CardInstance), `layout.text_area` — слот текста/плейсхолдера,
                // `layout.caret_x` — позиция каретки (замер префикса тем же
                // кеглем 11.0, что и у текста; CR-015 — без эвристики chars×7).
                // `text_shown` из layout НЕ используем: отображаемый текст
                // маскируется отдельно (для API-key/selfhost-key — «•»),
                // контракт кита «цвет/текст — отдельно от геометрии» (§F-8).
                RowKind::TextInput => {
                    let [field_x, field_y, field_w, _] = text_input_field_rect(*row, *rect);
                    let editing_now = self.settings_text_edit == Some(*row);
                    // Кнопка «Проверить» — только для строк с кнопкой (URL/
                    // key/api_key). Модель-строки кнопки не имеют.
                    if text_input_has_button(*row) {
                        let button_label = match row {
                            SettingsRow::AiApiKey => self.tr(keys::AI_BTN_CHECK_KEY).to_owned(),
                            SettingsRow::AiSelfhostUrl | SettingsRow::AiSelfhostKey => {
                                self.tr(keys::AI_BTN_CHECK).to_owned()
                            }
                            // FR-LLM-FIX (task FIX-TEXT-INPUT): другие
                            // TextInput-строки с кнопкой (если появятся) —
                            // единая подпись «Проверить»; match exhaustive.
                            _ => self.tr(keys::AI_BTN_CHECK).to_owned(),
                        };
                        instances.push(CardInstance {
                            pos: [control[0], control[1]],
                            size: [control[2], control[3]],
                            fill: if point_in_rect(control, self.cursor) {
                                hover_fill(palette.palette_chip_fill)
                            } else {
                                palette.palette_chip_fill
                            },
                            border: palette.palette_border,
                            params: [6.0, 0.0, 0.0, 1.0],
                            corners: [0.0; 4],
                        });
                        texts.push(OwnedScreenText {
                            text: button_label,
                            origin: [control[0] + 6.0, control[1] + 4.0],
                            width: control[2] - 12.0,
                            font_size: 11.0,
                            color: palette.title,
                            align: TextAlign::Center,
                        });
                    }
                    // FR-LLM-FIX (task FIX-TEXTINPUT-KIT): вёрстка поля —
                    // `kit::text_field()`. Модель — сырое значение (caret_x
                    // измеряется по нему); placeholder — статическая подсказка
                    // строки. min/max — kit-метрики (TEXT_FIELD_MIN_W/H).
                    let value = self.settings_text_value_for_row(*row);
                    let placeholder = self.settings_text_placeholder_for_row(*row);
                    let model = canvas_ui::kit::TextFieldModel {
                        text: value.to_owned(),
                        caret: value.chars().count(),
                        sel: None,
                    };
                    let slot = canvas_ui::UiRect::new(
                        field_x,
                        field_y,
                        field_w,
                        canvas_ui::kit::TEXT_FIELD_HEIGHT,
                    );
                    let min = canvas_ui::UiVec2::new(
                        canvas_ui::kit::TEXT_FIELD_MIN_W,
                        canvas_ui::kit::TEXT_FIELD_HEIGHT,
                    );
                    let max = canvas_ui::UiVec2::new(field_w, canvas_ui::kit::TEXT_FIELD_HEIGHT);
                    let kit_palette = palette.kit_palette();
                    let mut m = canvas_ui::measure::TextMeasurer::new();
                    let mut fs = canvas_render::text::measure_font_system();
                    let layout = canvas_ui::kit::text_field(
                        slot,
                        min,
                        max,
                        &model,
                        placeholder,
                        editing_now,
                        canvas_ui::kit::KitState::Normal,
                        &kit_palette,
                        &mut m,
                        &mut fs,
                        canvas_render::text::SANS_FAMILY,
                        11.0,
                    );
                    drop(fs);
                    // FR-LLM-FIX (task FIX-TEXTINPUT-KIT): фон поля — row_fill;
                    // при фокусе — рамка акцентом (params.y = 1.0). Rect —
                    // `layout.rect` (constrain+stack в слоте).
                    instances.push(CardInstance {
                        pos: [layout.rect.x, layout.rect.y],
                        size: [layout.rect.w, layout.rect.h],
                        fill: palette.palette_row_fill,
                        border: if editing_now {
                            color_to_rgba(palette.link)
                        } else {
                            palette.palette_border
                        },
                        params: [4.0, editing_now as u8 as f32, 0.0, 1.0],
                        corners: [0.0; 4],
                    });
                    // FR-LLM-FIX (task FIX-TEXT-INPUT): отображаемое значение
                    // поля — для API-ключа/key маскируем «•» (как в прототипе),
                    // для модель-строк/URL показываем как есть. Пустое поле —
                    // плейсхолдер (приглушённым цветом). Текст рисуется по
                    // `layout.text_area` (min kit-пада TEXT_FIELD_PAD_H=8).
                    let (field_text, is_placeholder) = match row {
                        SettingsRow::AiApiKey => {
                            let key = &self.settings.llm.api_key;
                            if key.is_empty() {
                                ("sk-…".to_owned(), true)
                            } else {
                                ("•".repeat(key.chars().count().min(20)), false)
                            }
                        }
                        SettingsRow::AiSelfhostUrl => {
                            let url = &self.settings.llm.endpoint;
                            if url.is_empty() {
                                ("https://llm.corp.local/v1".to_owned(), true)
                            } else {
                                (url.clone(), false)
                            }
                        }
                        SettingsRow::AiSelfhostKey => {
                            let key = &self.settings.llm.selfhost_key;
                            if key.is_empty() {
                                ("API key endpoint'а".to_owned(), true)
                            } else {
                                ("•".repeat(key.chars().count().min(20)), false)
                            }
                        }
                        SettingsRow::AiModelSuggest
                        | SettingsRow::AiModelGraph
                        | SettingsRow::AiModelAgent => {
                            let model = match row {
                                SettingsRow::AiModelSuggest => &self.settings.llm.model_suggest,
                                SettingsRow::AiModelGraph => &self.settings.llm.model_graph,
                                SettingsRow::AiModelAgent => &self.settings.llm.model_agent,
                                _ => unreachable!("модель-строка вне match"),
                            };
                            if model.is_empty() {
                                ("glm-5.3-flash".to_owned(), true)
                            } else {
                                (model.clone(), false)
                            }
                        }
                        // FR-LLM-FIX (task FIX-TEXT-INPUT): другие
                        // TextInput-строки (если появятся) — пустое поле;
                        // match exhaustive.
                        _ => (String::new(), false),
                    };
                    if !field_text.is_empty() {
                        texts.push(OwnedScreenText {
                            text: field_text,
                            origin: [layout.text_area.x, layout.text_area.y + 2.0],
                            width: layout.text_area.w,
                            font_size: 11.0,
                            color: if is_placeholder {
                                palette.icon
                            } else {
                                palette.body
                            },
                            align: TextAlign::Left,
                        });
                    }
                    // FR-LLM-FIX (task FIX-TEXTINPUT-KIT): каретка «|» —
                    // позиция из `layout.caret_x` (kit::text_field замеряет
                    // префикс до каретки тем же кеглем 11.0; -1.0 — не в
                    // фокусе). Кладётся ПОВЕРХ текста, как в explain —
                    // перекрытие последнего глифа 1.5 px не мешает.
                    if editing_now && layout.caret_x >= 0.0 {
                        instances.push(CardInstance {
                            pos: [layout.caret_x, layout.text_area.y + 2.0],
                            size: [1.5, layout.text_area.h - 4.0],
                            fill: color_to_rgba(palette.link),
                            border: [0.0; 4],
                            params: [0.0, 0.0, 0.0, 1.0],
                            corners: [0.0; 4],
                        });
                    }
                    // FR-LLM-FIX (task FIX-TEXT-INPUT): бейдж справа от поля
                    // (только для строк с кнопкой — модель-строки бейджа не
                    // имеют, поле занимает всю доступную ширину).
                    if text_input_has_button(*row) {
                        let (badge_text, badge_color) = match row {
                            SettingsRow::AiApiKey => {
                                let badge = if self.ai_key_ok {
                                    self.tr(keys::AI_KEY_VALID).replace("{n}", "0")
                                } else {
                                    self.tr(keys::AI_KEY_NOT_CHECKED).to_owned()
                                };
                                let color: canvas_render::Color = if self.ai_key_ok {
                                    crate::kit_ui::color4(palette.control_success)
                                } else {
                                    palette.icon
                                };
                                (badge, color)
                            }
                            SettingsRow::AiSelfhostUrl | SettingsRow::AiSelfhostKey => {
                                let badge = if self.ai_selfhost_ok {
                                    self.tr(keys::AI_SH_OK).to_owned()
                                } else {
                                    self.tr(keys::AI_SH_NOT_CHECKED).to_owned()
                                };
                                let color: canvas_render::Color = if self.ai_selfhost_ok {
                                    crate::kit_ui::color4(palette.control_success)
                                } else {
                                    palette.icon
                                };
                                (badge, color)
                            }
                            _ => (String::new(), palette.icon),
                        };
                        if !badge_text.is_empty() {
                            // FR-LLM-FIX (task FIX-TEXT-INPUT): бейдж — справа
                            // от поля (field_x + field_w + gap), ширина = 110.
                            let badge_w = 110.0;
                            let gap = 6.0;
                            texts.push(OwnedScreenText {
                                text: badge_text,
                                origin: [field_x + field_w + gap, field_y + 2.0],
                                width: badge_w,
                                font_size: 10.5,
                                color: badge_color,
                                align: TextAlign::Left,
                            });
                        }
                    }
                }
            }
        }
        // FR-026: выпадающее меню — поверх модалки: фон чуть ярче панели,
        // hover/клавиатурное выделение пункта, галочка у текущего значения.
        // FR-CLIP: дропдаун рисуется в ОТДЕЛЬНУЮ полосу (popup над модалкой,
        // клип = menu rect — может выходить за границы модалки).
        if let Some((row, items, menu_rect)) = &menu {
            render.dropdown_instances.push(CardInstance {
                pos: [menu_rect[0], menu_rect[1]],
                size: [menu_rect[2], menu_rect[3]],
                fill: hover_fill(palette.menu_fill),
                border: [0.0; 4],
                params: [8.0, 0.0, 0.0, 1.0],
                corners: [0.0; 4],
            });
            let hovered_item = dropdown_item_at(*menu_rect, items.len(), self.cursor);
            for (i, (label, current)) in items.iter().enumerate() {
                let y = menu_rect[1] + DROPDOWN_MARGIN + i as f32 * DROPDOWN_ROW_H;
                let highlighted = hovered_item == Some(i)
                    || (self.settings_dropdown.open_row == Some(*row)
                        && self.settings_dropdown.selected == i);
                if highlighted {
                    render.dropdown_instances.push(CardInstance {
                        pos: [menu_rect[0] + 3.0, y + 1.0],
                        size: [menu_rect[2] - 6.0, DROPDOWN_ROW_H - 2.0],
                        fill: [0.24, 0.30, 0.42, 0.6],
                        border: [0.0; 4],
                        params: [4.0, 0.0, 0.0, 1.0],
                        corners: [0.0; 4],
                    });
                }
                if *current {
                    render.dropdown_texts.push(OwnedScreenText {
                        text: "✓".to_owned(),
                        origin: [menu_rect[0] + 8.0, y + 5.0],
                        width: 18.0,
                        font_size: 12.0,
                        color: palette.link,
                        align: TextAlign::Left,
                    });
                }
                render.dropdown_texts.push(OwnedScreenText {
                    text: label.clone(),
                    origin: [menu_rect[0] + MENU_LABEL_X, y + 4.0],
                    width: menu_rect[2] - MENU_LABEL_X - DROPDOWN_MARGIN,
                    font_size: 13.0,
                    color: palette.body,
                    align: TextAlign::Left,
                });
            }
            // FR-CLIP: клип дропдаун-полосы = rect меню (popup над модалкой).
            render.dropdown_clip =
                canvas_ui::UiRect::new(menu_rect[0], menu_rect[1], menu_rect[2], menu_rect[3]);
        }
        // FR-CLIP: клип основной полосы = вьюпорт. Settings_overlay рисует
        // РАЗРОЗНЕННЫЕ элементы (угловые кнопки + модалка + панель hotkeys) —
        // единого bounding rect нет, клип не сужаем (контракт задачи: tight
        // clip только там, где есть единый rect — это дропдаун). Задача B
        // (FR-CLIP) отдельно сужает тени params.w → 1.0 у popup-квадов.
        render.instances = instances;
        render.texts = texts;
        render.icons = icons;
        render.clip = canvas_ui::UiRect::new(0.0, 0.0, viewport[0], viewport[1]);
        render
    }

    /// Подтверждение диалога (Enter/клик «Да»): установка или удаление.
    pub(super) fn confirm_dialog(&mut self) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        match dialog {
            AppDialog::InstallWidget {
                src,
                manifest,
                pos,
                updating,
            } => {
                match self.widgets.install_package(&src) {
                    Ok(canvas_widgets::registry::InstallOutcome::Installed) => {
                        self.show_toast(self.trf(
                            keys::TOAST_WIDGET_INSTALLED,
                            &[("{name}", manifest.name.as_str())],
                        ));
                    }
                    Ok(canvas_widgets::registry::InstallOutcome::Updated) => {
                        self.show_toast(self.trf(
                            keys::TOAST_WIDGET_UPDATED,
                            &[
                                ("{name}", manifest.name.as_str()),
                                ("{version}", manifest.version.as_str()),
                            ],
                        ));
                    }
                    Ok(canvas_widgets::registry::InstallOutcome::SameVersion) => {
                        self.show_toast(self.trf(
                            keys::TOAST_WIDGET_SAME_VERSION,
                            &[("{name}", manifest.name.as_str())],
                        ));
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "установка виджета не удалась");
                        self.show_toast(
                            self.trf(keys::TOAST_INSTALL_FAILED, &[("{err}", &e.to_string())]),
                        );
                        self.request_redraw();
                        return;
                    }
                }
                // Нода в точке дропа (SPEC §10: «виджет ставится на канвас»);
                // при обновлении — не дублируем (П5: props/ноды сохраняются)
                if !updating {
                    self.push_undo();
                    let id = self.widgets.next_node_id(&self.scene.canvas);
                    let node = self.widgets.build_widget_node(
                        &manifest.id,
                        id,
                        [pos[0] + 40.0, pos[1] + 30.0],
                    );
                    if let Some(node) = node {
                        self.scene.canvas.nodes.push(node);
                        let index = self.scene.canvas.nodes.len() - 1;
                        let node_ref = &self.scene.canvas.nodes[index];
                        self.scene.spatial.insert(index, node_ref);
                        self.selected = Some(Selection::Node(index));
                        self.scene.mark_dirty();
                    }
                }
                self.request_redraw();
            }
            AppDialog::RemovePackage { widget_id, name } => {
                match self.widgets.remove_package(&widget_id) {
                    Ok(()) => {
                        self.show_toast(
                            self.trf(keys::TOAST_PACKAGE_REMOVED, &[("{name}", &name)]),
                        );
                        // Ноды пакета остаются (деградируют в заглушки —
                        // package_ok=false в LOD); пересборка spatial не нужна
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "удаление пакета не удалось");
                        self.show_toast(
                            self.trf(keys::TOAST_REMOVE_FAILED, &[("{err}", &e.to_string())]),
                        );
                    }
                }
                self.request_redraw();
            }
            // FR-014: подтверждение цикла — ребро создаётся как
            // контрольная связь (без потока значений); FR-025: построчный
            // исток не сохраняется — control-ребро значения не переносит
            AppDialog::EdgeCycle {
                from_node,
                from_side,
                to_node,
                to_side,
            } => {
                self.create_edge(
                    from_node,
                    from_side,
                    to_node,
                    to_side,
                    FlowKind::Control,
                    None,
                );
            }
            // FR-050 Н4: замена источника — ОДИН undo-шаг (FR-006): старые
            // рёбра (легаси-дубли — все) удаляются, новое создаётся;
            // инвариант «после успешного создания нет двух value-рёбер в
            // один toParam» восстанавливается
            AppDialog::ReplaceSource {
                from_node,
                from_side,
                from_line,
                to_node,
                to_side,
                param,
                old_edges,
                ..
            } => {
                self.push_undo();
                for id in &old_edges {
                    self.scene.canvas.remove_edge(id);
                }
                let mut edge = Edge::new(
                    self.scene.canvas.next_edge_id(),
                    from_node,
                    Some(from_side),
                    to_node.clone(),
                    Some(to_side),
                );
                edge.set_flow_kind(FlowKind::Value);
                edge.from_line = from_line;
                edge.to_param = Some(param.clone());
                self.scene.canvas.add_edge(edge);
                self.scene.mark_dirty();
                self.scene.recompute_flow();
                // FR-050 Н9-6 (этап E): замена источника — тот же тост
                // «подтянулся из …» (новый источник виден владельцу)
                self.spill_connected_toast(&to_node, &param);
                self.request_redraw();
            }
            // PRD-0007 (FR-048 X4, AC-5.3): откат пачки подтверждён —
            // штатный undo (тег снялся вместе со снапшотом), подсветка гаснет
            AppDialog::AutolinkRollback { edge_ids, .. } => {
                self.dialog = None;
                self.autolink_batch = None;
                self.focus_edges.clear();
                let _ = edge_ids;
                if let Some(before) = self.scene.take_undo() {
                    self.restore_canvas(before);
                    tracing::debug!(depth = self.scene.undo_stack.len(), "undo");
                }
                self.request_redraw();
            }
        }
    }

    /// Применить настройку/действие ноды из палитры выделения
    /// (FR-009; диспетчер для `PaletteAction::Node`). Каждая мутирующая
    /// настройка — «push_undo → мутация → mark_dirty»; переименование
    /// входит в редактирование (его undo — commit сессии).
    pub(super) fn apply_node_setting(
        &mut self,
        node_index: usize,
        setting: crate::ui::NodeSetting,
    ) {
        use crate::ui::NodeSetting;
        match setting {
            // FR-009: переименовать = вход в редактирование (двойной клик).
            // FR-072: у text-ноды переименование правит ЗАГОЛОВОК (однострочный
            // редактор в шапке, с переносом legacy-первой строки); группа —
            // подпись label прежним begin_editing.
            NodeSetting::Rename => {
                let is_group = self
                    .scene
                    .canvas
                    .nodes
                    .get(node_index)
                    .is_some_and(|node| node.kind() == NodeKind::Group);
                if is_group {
                    self.begin_editing(node_index);
                } else {
                    self.begin_editing_title(node_index);
                }
            }
            NodeSetting::Duplicate => {
                // Дублирование одной ноды (паттерн duplicate_selection)
                let Some(node) = self.scene.canvas.nodes.get(node_index).cloned() else {
                    return;
                };
                let copies = reassign_ids(&self.scene.canvas, &[node]);
                let nodes = paste_nodes(
                    &copies,
                    PastePlacement::Offset([DUPLICATE_OFFSET, DUPLICATE_OFFSET]),
                );
                self.insert_nodes(nodes, true);
            }
            // FR-011: mindmap из меню
            NodeSetting::AddChild => self.mindmap_add_child(node_index),
            NodeSetting::AddSibling => self.mindmap_add_sibling(node_index),
            NodeSetting::CollapseBranch => self.mindmap_set_collapsed(node_index, true),
            NodeSetting::ExpandBranch => self.mindmap_set_collapsed(node_index, false),
            // FR-009: файловые операции (открытие — Windows, SPEC §7.4)
            NodeSetting::OpenFile => {
                #[cfg(windows)]
                {
                    let path = self
                        .scene
                        .canvas
                        .nodes
                        .get(node_index)
                        .and_then(|node| node.file.clone())
                        .map(|file| resolve_node_path(&file, &self.scene.canvas_dir()));
                    if let Some(path) = path {
                        if let Err(err) = canvas_shell::desktop::interop::open_file(&path) {
                            tracing::warn!(%err, path = %path.display(), "не удалось открыть файл");
                        }
                    }
                }
                #[cfg(not(windows))]
                let _ = node_index;
            }
            NodeSetting::OpenFolder => {
                // Папка файла через ShellExecuteEx на директорию (Win)
                #[cfg(windows)]
                {
                    let dir = self
                        .scene
                        .canvas
                        .nodes
                        .get(node_index)
                        .and_then(|node| node.file.clone())
                        .map(|file| resolve_node_path(&file, &self.scene.canvas_dir()))
                        .and_then(|path| path.parent().map(|p| p.to_path_buf()));
                    if let Some(dir) = dir {
                        if let Err(err) = canvas_shell::desktop::interop::open_file(&dir) {
                            tracing::warn!(%err, dir = %dir.display(), "не удалось открыть папку");
                        }
                    }
                }
                #[cfg(not(windows))]
                let _ = node_index;
            }
            NodeSetting::CopyPath => {
                let text = self
                    .scene
                    .canvas
                    .nodes
                    .get(node_index)
                    .and_then(|node| node.file.clone().or_else(|| node.text.clone()))
                    .unwrap_or_default();
                if !text.is_empty() {
                    self.clipboard.set_text(text);
                    self.show_toast(self.tr(keys::TOAST_PATH_COPIED));
                }
            }
            NodeSetting::ClearText => {
                // FR-006: очистка текста — undo-шаг (no-op на пустой — без шага)
                let snapshot = self.scene.canvas.clone();
                if let Some(node) = self.scene.canvas.nodes.get_mut(node_index) {
                    node.text = Some(String::new());
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
            }
            NodeSetting::Ungroup => {
                // Разгруппировать = удалить группу-ноду без каскада по детям
                // (removing_group_keeps_children); дети остаются на местах
                let is_group = self
                    .scene
                    .canvas
                    .nodes
                    .get(node_index)
                    .is_some_and(|node| node.kind() == NodeKind::Group);
                if !is_group {
                    return;
                }
                self.push_undo();
                self.scene.canvas.remove_node(node_index);
                // Индексы сдвинулись — spatial/кэши перестраиваются
                // (паттерн delete_selected)
                self.scene.spatial = SpatialIndex::build(&self.scene.canvas);
                self.selected = None;
                self.selected_nodes.clear();
                self.scene.mark_dirty();
                self.show_toast(self.tr(keys::TOAST_GROUP_UNGROUPED));
            }
            NodeSetting::WidgetReload => {
                let node_id = self
                    .scene
                    .canvas
                    .nodes
                    .get(node_index)
                    .map(|node| node.id.clone());
                if let Some(node_id) = node_id {
                    self.widgets.reload_widget(&node_id);
                    self.show_toast(self.tr(keys::TOAST_WIDGET_RELOADING));
                }
            }
            NodeSetting::WidgetPermissions => {
                let node_id = self
                    .scene
                    .canvas
                    .nodes
                    .get(node_index)
                    .map(|node| node.id.clone());
                let summary = node_id
                    .as_deref()
                    .and_then(|id| self.widgets.permissions_of_node(&self.scene.canvas, id))
                    .map(|permissions| {
                        let list: Vec<&str> =
                            permissions.list().iter().map(|p| p.as_str()).collect();
                        if list.is_empty() {
                            "нет особых разрешений".to_owned()
                        } else {
                            list.join(", ")
                        }
                    })
                    .unwrap_or_else(|| "пакет не установлен".to_owned());
                self.show_toast(self.trf(keys::TOAST_PERMISSIONS, &[("{summary}", &summary)]));
            }
        }
    }
}
