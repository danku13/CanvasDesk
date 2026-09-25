//! Main stage — полноэкранная поверхность главной сцены: frame-пайплайн
//! (stage_frame), calc-подрежим (модель/колёсико/fade), pill-зона рёбер
//! (state/rects/corridor/scroll/hit + лейблы src/dst и addr/value тексты),
//! minimap (обновление, геометрия, камера, клик) и открытие/закрытие.
//!
//! Выделены из `app.rs` сессией рефакторинга 2026-09-25 (этап 5) без
//! изменения поведения: это методы `App`, работающие с тем же состоянием.
//! Паттерн дочернего модуля — как `ui_registry` (FR-052): `use super::*`
//! даёт доступ к приватным полям `App` и импортам родителя.

use super::*;

use canvas_ui::geometry::UiRect;

/// FR-068 (W3-продолжение): stage-локальный прямоугольник → экранная
/// область Painter-пути хрома stage (map_point/map_size — тот же
/// StageTransform, что у quad-пути). Высоты label-областей — номинал
/// (конвертацией не используются — семантика OwnedScreenText).
fn stage_area(t: &StageTransform, x: f32, y: f32, w: f32, h: f32) -> UiRect {
    let p = t.map_point([x, y]);
    UiRect::new(p[0], p[1], t.map_size(w), t.map_size(h))
}

/// FR-068 этап M2 (Table v2, `docs/plans/fr-068-table-v2.md` §8 — первый
/// кандидат): строки панели «Как считается» — 2×retained-Table
/// («Переменные»/«Расчёт»). Пересборка [`canvas_ui::kit::TableRow`] из
/// модели кадра (строки-владения — приёмлемая цена; state и
/// переопределения слотов — те же вычисления, что в прежнем ручном
/// kit-цикле FR-061 D-15, дословно: fill = search_row_fill с
/// `alpha·a`; border — фокус/`UNMAPPED_EDGE_COLOR`/прозрачный; маркер —
/// value-точка `FLOW_EDGE`/янтарная либо ƒ = `text_title`; значение —
/// text/danger/quote; label — text; всё с приглушением Q3
/// `1.0 − 0.5·dim`), затем синхронизация скролла (источник истины —
/// контекст кадра `ctx.vars_scroll`/`ctx.formulas_scroll`,
/// синхронизированные раскладкой панели; `content_h` держит
/// [`canvas_ui::kit::Table::set_rows`] — инвариант sync_scroll §4.3) и
/// отрисовка ТОЛЬКО строк ([`canvas_ui::kit::Table::paint_rows_with`])
/// в порядке stage: строки vars → строки формул. Бегунки рисует
/// существующий блок stage (цвет — слот `palette_border`, НЕ
/// `control_border` [`canvas_ui::kit::Table::paint_scrollbar`] —
/// бит-в-бит цвета), draw-порядок кадра сохранён дословно.
///
/// Замер — внешний общий (§4.7 дизайна): `m` + `fs`
/// (`canvas_render::text::measure_font_system`) передаются снаружи —
/// паритет замера с прежним kit-путём; собственные замерщики Table
/// (Component-слой) здесь не используются. Таблицы создаются при первом
/// кадре панели (`FontSystem::new()` дорогой — один раз, не на кадр,
/// §4.6); Props (кегль/палитра/opts) — кадровые, синхронизируются здесь.
///
/// Вьюпорты колонок — ЭКРАННЫЕ: `ox/oy` — НАЧАЛО STAGE в вьюпорте
/// (`rect.x/rect.y`), площади раскладки (`vars_area`/`formulas_area`)
/// — stage-локальные (та же база, что у hit-тестов input.rs: курсор −
/// начало stage). Слоты — экранные (x≠0), поэтому направляющие
/// строятся от ПРАВОГО КРАЯ ВЬЮПОРТА В КООРДИНАТАХ СЛОТОВ
/// (`viewport_right`, не ширина):
/// vars — `right_pad = 6` (прежний `vars_right`), формулы —
/// `right_pad = 0`: правых ячеек нет → правило деградации §4.2 внутри
/// Table (паритет прежнего ручного входа `RowGuides { value_x:
/// unit_x: slot.right() }`, stage.rs:1386–1392).
#[allow(clippy::too_many_arguments)] // плоский контракт кадра панели (все слоты явные — I-1)
fn paint_calc_panel_rows(
    tables: &mut Option<(canvas_ui::kit::Table, canvas_ui::kit::Table)>,
    d: &mut Painter,
    m: &mut canvas_ui::measure::TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    ctx: &StageFrameCtx,
    panel: &calc_panel_ui::CalcPanelLayout,
    ox: f32,
    oy: f32,
    kit_size: f32,
    kit_palette: &canvas_ui::kit::KitPalette,
    row_fill: [f32; 4],
    quote: [f32; 4],
    unmapped_text: &str,
) {
    // FR-044 Q3: приглушение строк вне фокуса — с анимацией перехода
    // (1.0 − 0.5·dim; токен focus_fade_ms); рамка фокуса гаснет вместе
    // с коэффициентом (прежние замыкания кадра — дословно)
    let any_focus = ctx.focus.is_some();
    let row_focused = |row: usize| -> bool {
        ctx.focus
            .as_ref()
            .is_some_and(|focus| focus.rows.contains(&row))
    };
    let row_alpha = |focused: bool| -> f32 {
        if any_focus && !focused {
            1.0 - 0.5 * ctx.dim
        } else {
            1.0
        }
    };
    let focus_border = || {
        let mut border = SELECTION_BORDER;
        border[3] *= ctx.dim;
        border
    };
    // Вьюпорты колонок (экранные координаты: начало stage + площади
    // раскладки — та же база, что у hit-тестов input.rs)
    let vars_viewport = UiRect::new(
        ox + panel.vars_area[0],
        oy + panel.vars_area[1],
        panel.vars_area[2],
        panel.vars_area[3],
    );
    let formulas_viewport = UiRect::new(
        ox + panel.formulas_area[0],
        oy + panel.formulas_area[1],
        panel.formulas_area[2],
        panel.formulas_area[3],
    );
    // Retained-таблицы: создание при первом кадре панели (§4.6 —
    // FontSystem::new() дорогой); кегль/палитра/opts — кадровые Props
    // (масштаб раскладки и тема меняются) — синхронизация каждый кадр
    let (vars_table, formulas_table) = tables.get_or_insert_with(|| {
        (
            canvas_ui::kit::Table::new(canvas_ui::kit::TableProps::default()),
            canvas_ui::kit::Table::new(canvas_ui::kit::TableProps::default()),
        )
    });
    for (table, right_pad) in [
        // «Переменные»: отступ направляющих от правого края — 6 (прежний
        // `vars_right`); «Расчёт»: 0 — направляющие не строятся (деградация)
        (&mut *vars_table, 6.0),
        (&mut *formulas_table, 0.0),
    ] {
        table.props.size = kit_size;
        table.props.palette = *kit_palette;
        table.props.opts = canvas_ui::kit::TableOpts {
            leader: false, // прототип FR-044 без лидера
            gap: canvas_core::tokens::TABLE_GUIDE_GAP,
            row_h: calc_panel_ui::PANEL_ROW_H,
            row_gap: 0.0,
            right_pad,
        };
    }
    // Строки «Переменных» (значение unmapped — «не подставлено»): маркер —
    // value-точка (Р-4), Ok/Err — FLOW_EDGE, Unmapped — янтарная
    let var_rows: Vec<canvas_ui::kit::TableRow> = ctx
        .model
        .vars
        .iter()
        .enumerate()
        .map(|(i, var)| {
            let focused = row_focused(i);
            let alpha = row_alpha(focused);
            let unmapped = var.value == RowValue::Unmapped;
            let mut fill = row_fill;
            fill[3] *= alpha;
            let (dot_fill, value_fill) = match &var.value {
                RowValue::Ok(_) => (FLOW_EDGE_COLOR, kit_palette.text),
                RowValue::Err(_) => (FLOW_EDGE_COLOR, kit_palette.control_danger),
                RowValue::Unmapped => (UNMAPPED_EDGE_COLOR, quote),
            };
            canvas_ui::kit::TableRow {
                marker: canvas_ui::kit::RowMarker::Dot,
                label: var.path.clone(),
                value: match &var.value {
                    RowValue::Ok(text) => text.clone(),
                    RowValue::Err(err) => err.clone(),
                    RowValue::Unmapped => unmapped_text.to_owned(),
                },
                unit: String::new(),
                badge: None,
                // Состояние строки — фокус Р-5 → Selected (прежний
                // WidgetState::set_selected → kit_state, дословно)
                state: if focused {
                    canvas_ui::kit::KitState::Selected
                } else {
                    canvas_ui::kit::KitState::Normal
                },
                style: canvas_ui::kit::TableRowStyle {
                    fill: Some(fill),
                    border: Some(if focused {
                        focus_border()
                    } else if unmapped {
                        // «пунктирная строка не подставлено» (Р-4): пунктир
                        // в примитивах квадов недоступен — янтарный контур
                        // (UNMAPPED_EDGE_COLOR, семантика Р-3 FR-050)
                        UNMAPPED_EDGE_COLOR
                    } else {
                        [0.0; 4]
                    }),
                    marker: Some(dim_color4(dot_fill, alpha)),
                    label: Some(dim_color4(kit_palette.text, alpha)),
                    value: Some(dim_color4(value_fill, alpha)),
                    ..canvas_ui::kit::TableRowStyle::default()
                },
            }
        })
        .collect();
    // Строки «Расчёта» — ƒ-маркер + формула с путями операндов (значения
    // нет — ячейки/лидера нет, текст до края строки)
    let formula_rows: Vec<canvas_ui::kit::TableRow> = ctx
        .model
        .formulas
        .iter()
        .enumerate()
        .map(|(i, formula)| {
            let focused = row_focused(ctx.model.vars.len() + i);
            let alpha = row_alpha(focused);
            let mut fill = row_fill;
            fill[3] *= alpha;
            canvas_ui::kit::TableRow {
                marker: canvas_ui::kit::RowMarker::Glyph("ƒ"),
                label: formula.display.clone(),
                value: String::new(),
                unit: String::new(),
                badge: None,
                state: if focused {
                    canvas_ui::kit::KitState::Selected
                } else {
                    canvas_ui::kit::KitState::Normal
                },
                style: canvas_ui::kit::TableRowStyle {
                    fill: Some(fill),
                    border: Some(if focused { focus_border() } else { [0.0; 4] }),
                    marker: Some(dim_color4(kit_palette.text_title, alpha)),
                    label: Some(dim_color4(kit_palette.text, alpha)),
                    ..canvas_ui::kit::TableRowStyle::default()
                },
            }
        })
        .collect();
    // Скролл — источник истины КОНТЕКСТ КАДРА (offset/clamp —
    // потребитель; раскладка панели синхронизирует копии ctx);
    // set_rows держит content_h (n·row_h + (n−1)·row_gap = инвариант
    // sync_scroll — row_gap 0), затем полная синхронизация с ctx
    vars_table.set_rows(var_rows);
    vars_table.scroll = ctx.vars_scroll.clone();
    formulas_table.set_rows(formula_rows);
    formulas_table.scroll = ctx.formulas_scroll.clone();
    // ТОЛЬКО строки (paint_rows_with) — бегунки рисует существующий блок
    // stage ниже: draw-порядок кадра дословно (строки vars → строки
    // формул → бегунки)
    vars_table.paint_rows_with(d, m, fs, vars_viewport);
    formulas_table.paint_rows_with(d, m, fs, formulas_viewport);
}

impl App {
    /// Пересобрать/обновить миникарту (T13, SPEC §6.1): не каждый кадр, а по
    /// dirty-условиям — правки сцены (dirty_until save), движение камеры
    /// (пан/зум двигают рамку viewport) или смена размера буфера (DPI/resize).
    pub(super) fn update_minimap(&mut self) {
        // Вычисления (immutable) — до mutable borrow рендерера
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return;
        }
        let scale = self.scale_factor();
        let width_px = (MINIMAP_W as f32 * scale).round().max(1.0) as u32;
        let height_px = (MINIMAP_H as f32 * scale).round().max(1.0) as u32;
        let sig = (
            self.camera.position(),
            self.camera.zoom(),
            width_px,
            height_px,
        );
        let size_changed = self
            .minimap_sig
            .is_some_and(|prev| prev.2 != width_px || prev.3 != height_px);
        // dirty_until-автосейва: правки сцены пересобирают снимок; между
        // правкой и сейвом (2 с debounce) каждый запрошенный кадр обновляет
        // миникарту — это и есть видимость перемещений в реальном времени
        let scene_dirty = self.scene.dirty_since.is_some();
        if self.minimap.is_some() && self.minimap_sig == Some(sig) && !scene_dirty {
            return;
        }
        let viewport_world = self.camera.visible_world_rect(viewport);
        if self.minimap.is_none() || scene_dirty || size_changed {
            // сцена/размер изменились — полный снимок (T13-A)
            // FR-011: свернутые поддеревья не рисуются на миникарте
            let hidden = self.hidden_subtree_nodes();
            let scene_view = if hidden.is_empty() {
                self.scene.canvas.clone()
            } else {
                let mut filtered = self.scene.canvas.clone();
                filtered.nodes = filtered
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| hidden.binary_search(i).is_err())
                    .map(|(_, node)| node.clone())
                    .collect();
                filtered.edges.retain(|edge| {
                    let from_exists = filtered.nodes.iter().any(|node| node.id == edge.from_node);
                    let to_exists = filtered.nodes.iter().any(|node| node.id == edge.to_node);
                    from_exists && to_exists
                });
                filtered
            };
            self.minimap = Some(Minimap::capture(
                &scene_view,
                viewport_world,
                width_px,
                height_px,
            ));
        } else if let Some(minimap) = self.minimap.as_mut() {
            // только камера — пересчёт подгонки и рамки (дешевле снимка)
            minimap.set_viewport(viewport_world);
        }
        self.minimap_sig = Some(sig);
        // Загрузка текстуры (mutable borrow) — кадр растеризован заранее
        if let Some(minimap) = self.minimap.as_ref() {
            let image = minimap.render();
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.set_minimap(&image);
            }
        }
    }

    /// Прямоугольник миникарты в логических px (T13): None — не задана или
    /// окно меньше 252×172 (квад скрыт).
    pub(super) fn minimap_rect(&self) -> Option<[f32; 4]> {
        self.renderer
            .as_ref()
            .and_then(|renderer| renderer.minimap_rect_logical())
    }

    /// Центрировать камеру на world-точке под курсором мыши в миникарте
    /// (T13): клик — прыжок, drag — world-точка следует за курсором.
    pub(super) fn center_camera_on_minimap_cursor(&mut self) {
        let Some(minimap) = self.minimap.as_ref() else {
            return;
        };
        let Some(rect) = self.minimap_rect() else {
            return;
        };
        // rect — логические px, маппинг минимапы — в физических буфера
        let scale = self.scale_factor();
        let px = [
            (self.cursor[0] - rect[0]) * scale,
            (self.cursor[1] - rect[1]) * scale,
        ];
        self.camera.set_center(minimap.map_to_world(px));
    }

    /// FR-044 Р-4: модель панели «Как считается» для приёмника среза —
    /// ВСЕ входы приёмника канваса (панель полная, Р-8) + значения по
    /// адресации из активных решений потока (what-if подмены видны).
    pub(super) fn stage_calc_model(&self, stage: &MainStageState) -> calc_panel_ui::CalcPanelModel {
        // FR-064 P1: double buffer — снимок активных решений через read()-гард.
        let active = canvas_scene::read_flow(&self.scene.flow_active);
        let values = PanelValues {
            lines: &active.lines,
            named: &active.named,
            outputs: &active.outputs,
        };
        calc_panel_ui::build_model(
            &self.scene.canvas,
            &stage.slice.nodes[1].id,
            &stage.edges,
            &values,
        )
    }

    /// FR-044 Р-5: активная подсветка — hover-превью перекрывает
    /// фиксированную (живой отклик; при уходе курсора возвращается
    /// зафиксированная).
    pub(super) fn stage_calc_active(&self) -> Option<&StageCalcFocus> {
        self.stage_calc_hover
            .as_ref()
            .or(self.stage_calc_focus.as_ref())
    }

    /// FR-044 Q3: тик анимации перехода подсветки (вызывается до сборки
    /// кадра и в about_to_wait). Ведёт коэффициент затемнения к цели
    /// (1.0 — подсветка активна, 0.0 — сброшена) по ease-out за токен
    /// `focus_fade_ms` (150 мс, animate.rs T23); во время фейд-аута
    /// рендер применяет СНИМОК последнего активного множества — приглушение
    /// возвращается к единице плавно, не мигая границами строк.
    /// Возвращает true — переход идёт (нужны кадры).
    pub(super) fn tick_stage_calc_fade(&mut self) -> bool {
        if self.main_stage.is_none() {
            // Stage закрыт — состояние рендера гаснет мгновенно (инвариант 8:
            // подсветка не переживает stage); анимации нет.
            self.stage_calc_render = (None, 0.0);
            self.stage_calc_fade = None;
            return false;
        }
        let live = self.stage_calc_active().cloned();
        let target_dim = if live.is_some() { 1.0 } else { 0.0 };
        let (prev_focus, prev_dim) = self.stage_calc_render.clone();
        // Целевое множество рендера: живое (включение/смена — мгновенно);
        // при сбросе — снимок (гаснет вместе с коэффициентом).
        let target_focus = match &live {
            Some(_) => live.clone(),
            None if prev_dim > f32::EPSILON => prev_focus.clone(),
            None => None,
        };
        // Смена множества без смены затемнения — мгновенный обмен (Р-5:
        // переход между строками панели не мигает).
        if (target_dim - prev_dim).abs() <= f32::EPSILON && target_dim > 0.0 {
            if target_focus != prev_focus {
                self.stage_calc_render = (target_focus, prev_dim);
                self.stage_calc_fade = None;
            }
            return false;
        }
        // Текущее значение коэффициента: из идущего перехода или устоявшееся.
        let (in_flight, current_dim) = match self.stage_calc_fade {
            Some((from, to, at)) => {
                let t = (at.elapsed().as_millis() as f32 / FOCUS_FADE_MS as f32).clamp(0.0, 1.0);
                (t < 1.0, from + (to - from) * ease_out_cubic(t))
            }
            None => (false, prev_dim),
        };
        if (target_dim - current_dim).abs() <= f32::EPSILON {
            // Затемнение устаканилось: финализация (при сбросе множество
            // очищается вместе с коэффициентом).
            self.stage_calc_render = (
                if target_dim > 0.0 { target_focus } else { None },
                target_dim,
            );
            self.stage_calc_fade = None;
            return false;
        }
        let target_changed = self
            .stage_calc_fade
            .as_ref()
            .is_some_and(|(_, to, _)| (*to - target_dim).abs() > f32::EPSILON);
        if !in_flight || target_changed {
            // Новый переход (или смена цели в полёте) — старт от текущего
            // значения, без скачка; множество применяется сразу.
            self.stage_calc_fade = Some((current_dim, target_dim, Instant::now()));
            self.stage_calc_render = (target_focus.clone(), current_dim);
            return true;
        }
        // Переход идёт — продвигаем коэффициент.
        self.stage_calc_render = (target_focus.clone(), current_dim);
        true
    }

    /// FR-044 Q3: переход подсветки ещё идёт (кадры нужны).
    pub(super) fn stage_calc_fade_animating(&self) -> bool {
        self.stage_calc_fade.is_some()
    }

    /// FR-059 (волна 1 кита): колесо над колонкой панели «Как считается»
    /// прокручивает её список (кит список+скролл). Курсор вне окон —
    /// ничего (wheel остаётся погашенным — stage модален).
    pub(super) fn stage_calc_wheel_scroll(&mut self, delta: MouseScrollDelta) {
        let Some(stage) = self.main_stage.as_ref() else {
            return;
        };
        let viewport = self.viewport_logical();
        let rect = main_stage_rect(viewport);
        if !point_in_rect([rect.x, rect.y, rect.w, rect.h], self.cursor) {
            return;
        }
        let dy = match delta {
            MouseScrollDelta::LineDelta(_, y) => -y * PAN_PX_PER_LINE,
            MouseScrollDelta::PixelDelta(pos) => -pos.y as f32 / self.scale_factor(),
        };
        // Раскладка синхронизирует КОПИИ скроллов (детерминизм с кадром);
        // мутация — только у целевой колонки, после конца заимствований
        let mut vars = self.stage_calc_vars_scroll.clone();
        let mut formulas = self.stage_calc_formulas_scroll.clone();
        let panel = {
            let model = self.stage_calc_model(stage);
            calc_panel_layout(&model, rect.w, rect.h, 96.0, &mut vars, &mut formulas)
        };
        // Площади колонок (`vars_area`/`formulas_area`) — stage-локальные
        // (та же база, что у кликов input.rs): курсор переводим в stage
        let rel = [self.cursor[0] - rect.x, self.cursor[1] - rect.y];
        if panel.as_ref().is_some_and(|p| p.vars_area_at(rel)) {
            vars.scroll_by(dy);
            vars.clamp();
            self.stage_calc_vars_scroll = vars;
            self.request_redraw();
        } else if panel.as_ref().is_some_and(|p| p.formulas_area_at(rel)) {
            formulas.scroll_by(dy);
            formulas.clamp();
            self.stage_calc_formulas_scroll = formulas;
            self.request_redraw();
        } else {
            // FR-044 Q2 (v2): колесо над зоной веера (вне колонок панели)
            // прокручивает окно пилюль в режиме Scroll (второй эшелон
            // переполнения); в Full/Compact колесо глотается — stage модален.
            let ctx = self.stage_frame_ctx(stage, &rect, stage.scale.max(f32::EPSILON));
            let local = [
                (self.cursor[0] - rect.x) / stage.scale.max(f32::EPSILON),
                (self.cursor[1] - rect.y) / stage.scale.max(f32::EPSILON),
            ];
            let in_pill_zone = local[0] >= ctx.zone.x
                && local[0] <= ctx.zone.x + ctx.zone.w
                && local[1] >= ctx.zone.y
                && local[1] <= ctx.zone.y + ctx.zone.h;
            if in_pill_zone && matches!(ctx.pill_mode, calc_panel_ui::PillZoneMode::Scroll { .. }) {
                if dy < 0.0 {
                    self.stage_pill_scroll += 1;
                } else if dy > 0.0 {
                    self.stage_pill_scroll = self.stage_pill_scroll.saturating_sub(1);
                }
                self.request_redraw();
            }
        }
    }

    /// FR-044 Р-1/Р-4: совместный контекст кадра main stage — геометрия
    /// веера, зона клампа пилюль с учётом панели «Как считается» (панель
    /// съедает низ зоны, Р-1 «между заголовком и панелью») и подсветка.
    /// Один расчёт для рендера и hit-теста (детерминизм, инвариант 3).
    pub(super) fn stage_frame_ctx(
        &self,
        stage: &MainStageState,
        rect: &canvas_core::bundles::Rect,
        s: f32,
    ) -> StageFrameCtx {
        let metrics = StageMetrics {
            header_h: HEADER_HEIGHT,
            body_top_gap: BODY_TOP_GAP,
            body_line: BODY_LINE_HEIGHT,
            result_line: RESULT_LINE_HEIGHT,
            body_padding: BODY_PADDING,
            strip_extra: 6.0,
        };
        let footers = [
            self.scene
                .expr_results
                .contains_key(&stage.slice.nodes[0].id),
            self.scene
                .expr_results
                .contains_key(&stage.slice.nodes[1].id),
        ];
        let lines = stage_edge_geometry(&stage.slice, &metrics, footers, 24);
        let model = self.stage_calc_model(stage);
        // Панель: нижняя зона stage; верх доступной зоны — 96 px от верха
        // (заголовок 56 + минимальная зона веера 40)
        // FR-059: скроллы колонок синхронизируются раскладкой (копии на
        // кадр — детерминизм рендер/hit)
        let mut vars_scroll = self.stage_calc_vars_scroll.clone();
        let mut formulas_scroll = self.stage_calc_formulas_scroll.clone();
        let panel = calc_panel_layout(
            &model,
            rect.w,
            rect.h,
            96.0,
            &mut vars_scroll,
            &mut formulas_scroll,
        );
        let panel_top_screen = panel.as_ref().map_or(rect.h - 46.0, |p| p.top);
        let zone = StageLocalRect {
            x: 0.0,
            y: 56.0 / s,
            w: rect.w / s,
            h: ((panel_top_screen - 8.0 - 56.0) / s).max(0.0),
        };
        // FR-044 Q3: рендер применяет анимированное состояние подсветки
        // (множество + коэффициент затемнения) — не живое; Q2: режим зоны
        // пилюль (эшелоны переполнения стопки).
        let (focus, dim) = self.stage_calc_render.clone();
        let pill_mode =
            calc_panel_ui::pill_zone_mode(stage.slice.edges.len(), zone.h, self.stage_pill_scroll);
        StageFrameCtx {
            lines,
            model,
            panel,
            vars_scroll,
            formulas_scroll,
            zone,
            focus,
            dim,
            pill_mode,
        }
    }

    /// FR-044 Р-1 + Q2: rect'ы пилюль веера и режим зоны (stage-локальные
    /// px) — тот же расчёт, что в кадре ([`Self::stage_frame_ctx`]);
    /// hit-тест клика по пилюле (Р-5: клик = выделение ребра + синхронная
    /// подсветка). Эшелоны Q2 ([`calc_panel_ui::pill_zone_mode`]): Full —
    /// двухстрочные пилюли (адрес + значение); Compact — однострочные
    /// «адрес · значение»; Scroll — окно стопки с прокруткой.
    pub(super) fn stage_pill_state(
        &self,
        stage: &MainStageState,
        ctx: &StageFrameCtx,
    ) -> (Vec<(usize, StageLocalRect)>, calc_panel_ui::PillZoneMode) {
        // Сортировка по вертикали середин (прототип R7: стопка следует
        // геометрии веера) с сохранением индекса ребра
        let mut order: Vec<usize> = (0..ctx.lines.len().min(stage.slice.edges.len())).collect();
        order.sort_by(|&a, &b| ctx.lines[a].mid[1].total_cmp(&ctx.lines[b].mid[1]));
        // Тексты и ширины пилюль в порядке стопки; режим — от числа рёбер
        // и высоты зоны (Q2)
        let mode = calc_panel_ui::pill_zone_mode(
            stage.slice.edges.len(),
            ctx.zone.h,
            self.stage_pill_scroll,
        );
        let one_line = !matches!(mode, calc_panel_ui::PillZoneMode::Full);
        let pill_h = if one_line {
            calc_panel_ui::PILL_H_ONE_LINE
        } else {
            calc_panel_ui::PILL_H_TWO_LINE
        };
        let mut texts: Vec<(usize, String, f32)> = Vec::with_capacity(order.len());
        for &oi in &order {
            let edge = &stage.slice.edges[oi];
            let addr = truncate_chars(&self.stage_edge_addr_text(edge), 42);
            let text = if one_line {
                let value = truncate_chars(&self.stage_edge_value_text(edge), 42);
                let combined = if value.is_empty() {
                    addr
                } else if addr.is_empty() {
                    value
                } else {
                    format!("{addr} · {value}")
                };
                truncate_chars(&combined, 56)
            } else {
                addr
            };
            let w = (text.chars().count() as f32 * 7.2 + 24.0).max(56.0);
            texts.push((oi, text, w));
        }
        // Окно прокрутки (Scroll): подмножество стопки
        let visible: Vec<(usize, String, f32)> = match mode {
            calc_panel_ui::PillZoneMode::Scroll { first, visible, .. } => {
                texts.into_iter().skip(first).take(visible).collect()
            }
            _ => texts,
        };
        let sorted: Vec<(usize, f32, f32, Option<f32>)> = visible
            .into_iter()
            .map(|(item, _text, w)| {
                let pref = ctx.lines[item].mid[0];
                (item, w, pill_h, Some(pref))
            })
            .collect();
        let corridor = self.stage_pill_corridor(stage);
        let axis_y = ctx.zone.y + ctx.zone.h / 2.0;
        let laid = stage_fan_label_layout(sorted, corridor, ctx.zone, axis_y);
        let rects = laid
            .pills
            .iter()
            .map(|pill| {
                (
                    pill.item,
                    StageLocalRect {
                        x: pill.rect.x,
                        y: pill.rect.y,
                        w: pill.rect.w,
                        h: pill.rect.h,
                    },
                )
            })
            .collect();
        (rects, mode)
    }

    /// FR-044 Р-1: rect'ы пилюль веера (обёртка hit-теста над
    /// [`Self::stage_pill_state`]).
    pub(super) fn stage_pill_rects(
        &self,
        stage: &MainStageState,
        ctx: &StageFrameCtx,
    ) -> Vec<(usize, StageLocalRect)> {
        self.stage_pill_state(stage, ctx).0
    }

    /// FR-044 Р-1: коридор пилюль между колонками нод, суженный на зоны
    /// подписей концов рёбер (7b): пилюли не наезжают на подписи.
    /// `src_label_max`/`dst_label_max` — ширины колонок подписей концов
    /// (владелец 2026-09-22).
    pub(super) fn stage_pill_corridor(&self, stage: &MainStageState) -> StageLocalRect {
        let src_title = title_for(&stage.slice.nodes[0]);
        let mut src_label_max = 0.0_f32;
        let mut dst_label_max = 0.0_f32;
        for edge in stage.slice.edges.iter() {
            src_label_max = src_label_max.max(self.stage_src_label_width(edge));
            dst_label_max = dst_label_max.max(self.stage_dst_label_width(edge, &src_title));
        }
        let src = &stage.slice.nodes[0];
        let dst = &stage.slice.nodes[1];
        let mut corridor = fan_corridor(
            StageLocalRect {
                x: src.x,
                y: src.y,
                w: src.width,
                h: src.height,
            },
            StageLocalRect {
                x: dst.x,
                y: dst.y,
                w: dst.width,
                h: dst.height,
            },
            70.0,
        );
        let left_needed = src.x + src.width + 10.0 + src_label_max + 6.0;
        let right_limit = dst.x - 10.0 - dst_label_max - 6.0;
        if left_needed > corridor.x {
            let d = left_needed - corridor.x;
            corridor.x += d;
            corridor.w -= d;
        }
        let over = corridor.x + corridor.w - right_limit;
        if over > 0.0 {
            corridor.w -= over;
        }
        corridor.w = corridor.w.max(0.0);
        corridor
    }

    /// FR-044 Q2 (v2): rect'ы индикаторов прокрутки пилюль в режиме Scroll
    /// — «↑ ещё N» у верхнего края зоны, «ещё N ↓» у нижнего (по центру
    /// коридора). `None` — индикатора нет (счётчик нулевой).
    pub(super) fn stage_pill_scroll_indicators(
        &self,
        stage: &MainStageState,
        ctx: &StageFrameCtx,
    ) -> (Option<StageLocalRect>, Option<StageLocalRect>) {
        let calc_panel_ui::PillZoneMode::Scroll { above, below, .. } = ctx.pill_mode else {
            return (None, None);
        };
        let corridor = self.stage_pill_corridor(stage);
        let w = 84.0_f32.min(corridor.w.max(0.0));
        let h = 18.0;
        let cx = corridor.x + (corridor.w - w) / 2.0;
        let top = (above > 0).then_some(StageLocalRect {
            x: cx,
            y: ctx.zone.y + 4.0,
            w,
            h,
        });
        let bottom = (below > 0).then_some(StageLocalRect {
            x: cx,
            y: ctx.zone.y + ctx.zone.h - h - 4.0,
            w,
            h,
        });
        (top, bottom)
    }

    /// FR-044 Р-3-а: строки подписи у истока — (верхняя строка, значение).
    /// У value-ребра с адресацией — лейбл слота выхода (прототип drawPort
    /// R5/R6: `out: <имя>` / «строка N») над значением; без адресации —
    /// только значение (прежний вид 7b); control-ребро значения не несёт
    /// и подписи у истока не имеет (управление — не значение, инвариант 5).
    pub(super) fn stage_src_label_lines(&self, edge: &Edge) -> (Option<String>, String) {
        if edge.flow_kind() != FlowKind::Value {
            return (None, String::new());
        }
        let value = truncate_chars(&self.stage_edge_value_text(edge), 24);
        if let Some(output) = edge.from_output.as_deref() {
            let slot = i18n::trf(
                self.settings.language,
                keys::STAGE_OUT_LABEL,
                &[("name", output)],
            );
            return (Some(slot), value);
        }
        if let Some(line_no) = edge.from_line {
            let slot = i18n::trf(
                self.settings.language,
                keys::STAGE_LINE_LABEL,
                &[("n", &(line_no + 1).to_string())],
            );
            return (Some(slot), value);
        }
        (None, value)
    }

    /// FR-044 (7b) + Р-3-а: ширина подписи у истока (максимум строк;
    /// 0 — подписи нет).
    pub(super) fn stage_src_label_width(&self, edge: &Edge) -> f32 {
        let (slot, value) = self.stage_src_label_lines(edge);
        let w = |text: &str| text.chars().count() as f32 * 6.3 + 12.0;
        let mut width = 0.0_f32;
        if let Some(slot) = slot.as_deref() {
            width = width.max(w(slot));
        }
        if !value.is_empty() {
            width = width.max(w(&value));
        }
        width
    }

    /// FR-044 (7b): ширина подписи квалифицированного адреса у приёмника
    /// (0 — адресации нет).
    pub(super) fn stage_dst_label_width(&self, edge: &Edge, src_title: &str) -> f32 {
        let qualified = self.stage_dst_label_text(edge, src_title);
        if qualified.is_empty() {
            0.0
        } else {
            qualified.chars().count() as f32 * 6.3 + 12.0
        }
    }

    /// FR-044 (7b) + Р-3-а: текст подписи адреса у приёмника —
    /// «{исток} · строка N» / «{исток}.{выход}»; control-ребро —
    /// «управление» (инвариант 5: control-рёбра не отображаются
    /// value-путями); без адресации — имя истока (фолбэк 7b).
    pub(super) fn stage_dst_label_text(&self, edge: &Edge, src_title: &str) -> String {
        if edge.flow_kind() != FlowKind::Value {
            return self.tr(keys::STAGE_CTRL_LABEL).to_owned();
        }
        if let Some(line_no) = edge.from_line {
            format!(
                "{src_title} · {}",
                i18n::trf(
                    self.settings.language,
                    keys::STAGE_LINE_LABEL,
                    &[("n", &(line_no + 1).to_string())],
                )
            )
        } else if let Some(output) = edge.from_output.as_deref() {
            format!("{src_title}.{output}")
        } else {
            src_title.to_owned()
        }
    }

    /// FR-042 (E3) + FR-044: кадр main stage — паритет с прототипом
    /// prototype-mainstage-anatomy.html (drawStage). Затемнение фона,
    /// подложка, заголовок «Пучок: A → B · ×N» с кнопкой ✕, веер рёбер
    /// с точками портов, ПОЛНЫЕ карточки среза (заголовок, построчные
    /// результаты, полоса результата), пилюли подписей лейн-стопкой в
    /// коридоре между колонками, панель «Как считается» с подсветкой
    /// зависимостей ([`StageFrameCtx`] — общий расчёт с hit-тестом).
    /// Возвращает (квады, screen-тексты) — рендерер выводит их модальным
    /// проходом после всего живого контента (инвариант 8: модальность).
    pub(super) fn stage_frame(
        &self,
        viewport: [f32; 2],
        stage: &MainStageState,
        layout: StageLayout,
    ) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let palette = ThemeColors::from_theme(self.settings.theme);
        let rect = main_stage_rect(viewport);
        let transform = StageTransform::new([rect.x, rect.y], layout.scale);
        let camera = &self.camera;
        let zoom = camera.zoom();
        let s = layout.scale.max(f32::EPSILON);
        let mut quads: Vec<CardInstance> = Vec::new();
        let mut texts: Vec<OwnedScreenText> = Vec::new();
        // Шрифт stage-текста: базовый размер * масштаб раскладки (геометрия
        // сжата тем же коэффициентом — карточка и её текст сжимаются вместе);
        // минимум 8 px — читаемость деградационных режимов.
        let font = |px: f32| (px * s).max(8.0);
        // FR-044: совместный контекст кадра — геометрия веера, зона пилюль
        // с учётом панели «Как считается» (Р-1/Р-4) и подсветка (Р-5);
        // hit-тест клика пересчитывает те же значения (детерминизм).
        let ctx = self.stage_frame_ctx(stage, &rect, s);
        let lines = &ctx.lines;
        // FR-044 Р-5 + Q3: альфа ребра среза по индексу — рёбра вне множества
        // фокуса приглушены (0.35, паттерн dim_factor); коэффициент анимирован
        // (переход подсветки — Q3, токен focus_fade_ms)
        let edge_alpha = |i: usize| -> f32 {
            match &ctx.focus {
                Some(focus) if focus.edges.contains(&stage.edges[i]) => 1.0,
                Some(_) => 1.0 - (1.0 - 0.35) * ctx.dim,
                None => 1.0,
            }
        };
        // 1) Затемнение фона (§7.5: тёмная 0.6 / светлая 0.5) — весь вьюпорт
        quads.push(CardInstance {
            pos: camera.screen_to_world([0.0, 0.0], viewport),
            size: [viewport[0] / zoom, viewport[1] / zoom],
            fill: palette.stage_dim,
            border: [0.0; 4],
            params: [0.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        // 2) Подложка и рамка stage — стиль модалок FR-039 (радиус 14).
        // FR-068 (W3-продолжение): каркас stage — компонентный путь
        // (Painter + panel_style_of — явные слоты темы F-8); конвертация
        // paint_items_to_stage даёт те же world-квады (screen_to_world,
        // радиус/зум), порядок кадра сохранён дословно (I-1)
        {
            let mut chrome = Painter::new();
            chrome.panel(
                UiRect::new(rect.x, rect.y, rect.w, rect.h),
                &kit::panel_style_of(palette.menu_fill, palette.palette_border, 14.0, 0.0),
            );
            paint_items_to_stage(
                chrome.take_items(),
                camera,
                viewport,
                zoom,
                &mut quads,
                &mut texts,
            );
        }
        // 3) Заголовок «Пучок: A → B · ×N», подсказка Esc и кнопка ✕.
        // FR-068 (W3-продолжение): блок — Painter-путь компонентной модели;
        // геометрия ✕ — единый источник stage_close_button_rect (ТОТ ЖЕ rect
        // в hit-тесте click_main_stage — закрытие класса CR-015), стиль
        // кнопки — control_style_of (явные слоты, F-8)
        let from_title = title_for(&stage.slice.nodes[0]);
        let to_title = title_for(&stage.slice.nodes[1]);
        let mut chrome = Painter::new();
        let title_origin = transform.map_point([16.0, 12.0]);
        chrome.label(
            UiRect::new(
                title_origin[0],
                title_origin[1],
                transform.map_size(rect.w) - 200.0,
                18.0,
            ),
            &self.trf(
                keys::STAGE_BUNDLE_TITLE,
                &[
                    ("from", from_title.as_str()),
                    ("to", to_title.as_str()),
                    ("n", stage.slice.edges.len().to_string().as_str()),
                ],
            ),
            color_to_rgba(palette.title),
            font(13.0),
            PaintAlign::Left,
        );
        // FR-044 Р-8: счётчик внешних входов приёмника (вне пучка) —
        // «+N внешн. вход(а/ов)» под заголовком (панель полная, источники
        // видны мини-карточками под истоком)
        if ctx.model.ext_count > 0 {
            let ext_origin = transform.map_point([16.0, 30.0]);
            chrome.label(
                UiRect::new(
                    ext_origin[0],
                    ext_origin[1],
                    transform.map_size(rect.w) - 200.0,
                    14.0,
                ),
                &self.trf(
                    keys::STAGE_CALC_EXT,
                    &[("n", &ctx.model.ext_count.to_string())],
                ),
                color_to_rgba(palette.quote),
                font(10.5),
                PaintAlign::Left,
            );
        }
        // Кнопка ✕ — правый верхний угол (rect — единый источник с
        // hit-тестом: stage_close_button_rect)
        let close = stage_close_button_rect(&rect);
        let hint_origin = [close[0] - 160.0, close[1] + 5.0];
        chrome.label(
            UiRect::new(hint_origin[0], hint_origin[1], 152.0, 14.0),
            self.tr(keys::STAGE_HINT),
            color_to_rgba(palette.quote),
            font(11.0),
            PaintAlign::Left,
        );
        // Рамка кнопки — прозрачный контрол (заливка 0 — прежний вид;
        // стиль из явных слотов control_style_of, не сырые поля квада)
        chrome.control(
            UiRect::new(close[0], close[1], close[2], close[3]),
            &kit::control_style_of(
                [0.0; 4],
                palette.palette_border,
                color_to_rgba(palette.body),
                7.0,
            ),
        );
        chrome.label(
            UiRect::new(close[0], close[1] + 3.0, close[2], 14.0),
            "×",
            color_to_rgba(palette.body),
            font(12.0),
            PaintAlign::Center,
        );
        paint_items_to_stage(
            chrome.take_items(),
            camera,
            viewport,
            zoom,
            &mut quads,
            &mut texts,
        );
        // 4) Рёбра среза веером (stage-локальные px → мир); выделение
        // ребра среза — live-индекс из Selection. Геометрия — те же линии,
        // что у точек портов и hit-test'а (единый источник)
        let selected_slice = self.selected.and_then(|sel| match sel {
            Selection::Edge(live) => stage.edges.iter().position(|&e| e == live),
            Selection::Node(_) => None,
        });
        for inst in build_stage_edge_instances_with_alpha(
            &stage.slice,
            lines,
            stage.slice.edges.len(),
            selected_slice,
            None,
            edge_alpha,
        ) {
            quads.push(transform.instance_to_world(&inst, camera, viewport));
        }
        // 5) Точки портов на концах веера (аффорданс входа/выхода, прототип):
        // цвет — класс потока ребра, выделенное ребро — акцент; точки сидят
        // на строках значений (якоря линий)
        for (i, edge) in stage.slice.edges.iter().enumerate() {
            let Some(line) = lines.get(i) else {
                continue;
            };
            let mut fill = if selected_slice == Some(i) {
                SELECTION_BORDER
            } else if edge.flow_kind() == FlowKind::Value {
                FLOW_EDGE_COLOR
            } else {
                EDGE_COLOR
            };
            // FR-044 Р-5: точки портов приглушаются вместе с ребром
            fill[3] *= edge_alpha(i);
            for p in [line.from, line.to] {
                let d = 7.0;
                quads.push(transform.instance_to_world(
                    &CardInstance {
                        pos: [p[0] - d / 2.0, p[1] - d / 2.0],
                        size: [d, d],
                        fill,
                        border: [0.0; 4],
                        params: [d / 2.0, 0.0, 0.0, 1.0],
                        corners: [0.0; 4],
                    },
                    camera,
                    viewport,
                ));
            }
        }
        // 6) Карточки среза: карточка + полоса категории шаблона (анатомия
        // A/B, PRD-0004); выделенная — рамка выделения
        let is_selected = |node: &Node| {
            self.scene
                .canvas
                .nodes
                .iter()
                .position(|n| n.id == node.id)
                .is_some_and(|idx| {
                    self.selected == Some(Selection::Node(idx))
                        || self.selected_nodes.contains(&idx)
                })
        };
        for node in stage.slice.nodes.iter() {
            quads.push(transform.instance_to_world(
                &card_instance(node, is_selected(node), &palette),
                camera,
                viewport,
            ));
            // FR-075 (вёрстка prototype-unified): чип категории в шапке —
            // у шаблонных нод main stage (полосы категории больше нет).
            // Метка — «ШАБЛОН» (в снапшоте canvasdesk.template категории
            // нет — категория читается по цвету чипа); ширина — замер тем
            // же шрифтом, что рендер чипа (TextMeasurer + статический
            // FontSystem рендера).
            if node.template().is_some() {
                let label = "ШАБЛОН".to_string();
                let label_px = {
                    let mut measure_fs = canvas_render::text::measure_font_system();
                    let mut measurer = canvas_ui::measure::TextMeasurer::new();
                    measurer.width_of_weighted(
                        &mut measure_fs,
                        &label,
                        canvas_render::text::SANS_FAMILY,
                        font(10.0),
                        cosmic_text::Weight::SEMIBOLD,
                    )
                };
                let label_w = label_px / s; // world px (замер — логические px stage)
                let chip = header_chip_instance(
                    node,
                    label_w,
                    chip_fill(node, ChipKind::Template, &palette),
                );
                quads.push(transform.instance_to_world(&chip, camera, viewport));
                // Текст метки — screen-space, тёмный на цветной заливке
                // (прототип chipTxt #14161c)
                let rect = header_chip_rect(node, label_w);
                let origin = transform.map_point([rect[0] + 7.0, rect[1] + 2.0]);
                texts.push(OwnedScreenText {
                    text: label,
                    origin,
                    width: label_px + 2.0,
                    font_size: font(10.0),
                    color: canvas_render::Color::rgb(0x14, 0x16, 0x1c),
                    align: TextAlign::Left,
                });
            }
        }
        // 7) Контент нод среза (анатомия C/D, PRD-0004): заголовок,
        // построчные результаты Numi-листа, полоса результата в футере —
        // screen-space тексты константного размера (стиль модальностей
        // FR-039), позиции — через transform (scale применён один раз)
        for node in stage.slice.nodes.iter() {
            let title = title_for(node);
            if !title.is_empty() {
                texts.push(OwnedScreenText {
                    text: title,
                    origin: transform.map_point([node.x + 12.0, node.y + 8.0]),
                    width: transform.map_size(node.width) - 24.0,
                    font_size: font(13.0),
                    color: palette.title,
                    align: TextAlign::Left,
                });
            }
            if node.kind() != NodeKind::Text {
                continue;
            }
            let Some(text) = node.text.as_deref() else {
                continue;
            };
            let results = self.scene.expr_line_results.get(&node.id);
            let has_footer = self.scene.expr_results.contains_key(&node.id);
            // Вертикали строк — ритм живой карточки (BODY_LINE_HEIGHT от
            // шапки, инвариант вертикали FR-025); строки сверх высоты тела
            // (минус футер результата) не рисуются
            let body_top = node.y + HEADER_HEIGHT + BODY_TOP_GAP;
            let footer_h = if has_footer {
                RESULT_LINE_HEIGHT + 6.0
            } else {
                0.0
            };
            let avail_h =
                (node.height - HEADER_HEIGHT - BODY_TOP_GAP - BODY_PADDING - footer_h).max(0.0);
            let max_rows = ((avail_h / BODY_LINE_HEIGHT).floor() as usize).max(1);
            // Оценка ширины моно-строки: advance ≈ 0.6·font (stage-локальные px)
            let char_w = 7.2f32;
            for (li, line) in text.lines().enumerate().take(max_rows) {
                let row_y = body_top
                    + li as f32 * BODY_LINE_HEIGHT
                    + (BODY_LINE_HEIGHT - RESULT_LINE_HEIGHT) / 2.0;
                let outcome = results.and_then(|l| l.get(li)).and_then(|o| o.as_ref());
                let (value_text, is_err) = match outcome {
                    Some(ExprOutcome::Ok(value)) => (value.to_string(), false),
                    Some(ExprOutcome::Err(_)) => ("!".to_owned(), true),
                    None => (String::new(), false),
                };
                let value_w = value_text.chars().count() as f32 * char_w;
                // Левая колонка — исходная строка; усечение под зазор до
                // колонки значения (прототип: truncate до ширины карточки)
                let fit =
                    ((node.width - BODY_PADDING * 2.0 - value_w - 14.0) / char_w).max(3.0) as usize;
                let color = if is_err {
                    palette.error
                } else if outcome.is_some() {
                    palette.body
                } else {
                    palette.quote
                };
                texts.push(OwnedScreenText {
                    text: truncate_chars(line.trim_end(), fit),
                    origin: transform.map_point([node.x + BODY_PADDING, row_y + 2.0]),
                    width: transform.map_size(node.width - BODY_PADDING * 2.0),
                    font_size: font(12.0),
                    color,
                    align: TextAlign::Left,
                });
                if !value_text.is_empty() {
                    let vw = value_text.chars().count() as f32 * char_w;
                    texts.push(OwnedScreenText {
                        text: value_text,
                        origin: transform
                            .map_point([node.x + node.width - BODY_PADDING - vw, row_y + 2.0]),
                        width: transform.map_size(vw) + 24.0,
                        font_size: font(12.0),
                        color: if is_err { palette.error } else { palette.body },
                        align: TextAlign::Left,
                    });
                }
            }
            // Полоса результата (D): узловое значение в футере карточки
            if let Some(outcome) = self.scene.expr_results.get(&node.id) {
                let (value_text, color) = match outcome {
                    ExprOutcome::Ok(value) => (format!("= {value}"), palette.body),
                    ExprOutcome::Err(msg) => {
                        (truncate_chars(&format!("! {msg}"), 40), palette.error)
                    }
                };
                let vw = value_text.chars().count() as f32 * char_w;
                let strip_h = RESULT_LINE_HEIGHT + 6.0;
                let strip_y = node.y + node.height - BODY_PADDING - strip_h;
                quads.push(transform.instance_to_world(
                    &CardInstance {
                        pos: [node.x + 6.0, strip_y],
                        size: [(node.width - 12.0).max(0.0), strip_h],
                        fill: palette.search_row_fill,
                        border: [0.0; 4],
                        params: [6.0, 0.0, 0.0, 1.0],
                        corners: [0.0; 4],
                    },
                    camera,
                    viewport,
                ));
                // Порт выхода (FR-025) — кружок value-цвета у правого края
                let d = 7.0;
                quads.push(transform.instance_to_world(
                    &CardInstance {
                        pos: [
                            node.x + node.width - d - 4.0,
                            strip_y + strip_h / 2.0 - d / 2.0,
                        ],
                        size: [d, d],
                        fill: FLOW_EDGE_COLOR,
                        border: [0.0; 4],
                        params: [d / 2.0, 0.0, 0.0, 1.0],
                        corners: [0.0; 4],
                    },
                    camera,
                    viewport,
                ));
                let vw = vw.min(node.width - BODY_PADDING * 2.0 - 14.0).max(0.0);
                texts.push(OwnedScreenText {
                    text: value_text,
                    origin: transform.map_point([
                        node.x + node.width - BODY_PADDING - d - 6.0 - vw,
                        strip_y + 3.0,
                    ]),
                    width: transform.map_size(vw) + 24.0,
                    font_size: font(12.0),
                    color,
                    align: TextAlign::Left,
                });
            }
        }
        // 7b) Подписи на концах рёбер (FR-044, владелец 2026-09-22: «на
        // концах edge — подписи значений», прототип R5/R6 — порты с
        // подложкой) + Р-3-а (лейблы слотов): у истока — лейбл слота
        // выхода («out: <имя>» / «строка N») над ЗНАЧЕНИЕМ ребра; у
        // приёмника — квалифицированный адрес «Объект · строка N /
        // Объект.output», control-ребро — «управление» (инвариант 5).
        // Подложка — цвет подложки stage (меню): подписи не сливаются с
        // линиями веера (прототип R6: подложка от рёбер). Оценка ширины —
        // advance ≈ 0.6·шрифта (как у строк карточки). Ширины колонок —
        // те же хелперы, что сужают коридор пилюль (единый расчёт).
        // FR-044 Р-5 + Q3: подписи рёбер вне фокуса приглушены с анимацией.
        // FR-068 (W3-продолжение): Painter-путь (stage_area — тот же
        // StageTransform; радиус 4 — stage-локальные px, масштаб s)
        let src_title = title_for(&stage.slice.nodes[0]);
        let mut chrome = Painter::new();
        for (i, edge) in stage.slice.edges.iter().enumerate() {
            let Some(line) = lines.get(i) else {
                continue;
            };
            let alpha = edge_alpha(i);
            // Исток: лейбл слота выхода + значение строки/ноды (Р-3-а)
            let (slot_line, value) = self.stage_src_label_lines(edge);
            let two_line = slot_line.is_some() && !value.is_empty();
            if !value.is_empty() || slot_line.is_some() {
                let w = self.stage_src_label_width(edge);
                let (h, by) = if two_line {
                    (30.0, line.from[1] - 15.0)
                } else {
                    (18.0, line.from[1] - 9.0)
                };
                let mut fill = palette.menu_fill;
                fill[3] *= alpha;
                let bx = line.from[0] + 10.0;
                chrome.rect(
                    stage_area(&transform, bx, by, w, h),
                    fill,
                    [0.0; 4],
                    4.0 * s,
                );
                if let Some(slot) = slot_line.as_deref() {
                    // Лейбл слота выхода — приглушённый тон (подпись порта)
                    chrome.label(
                        stage_area(
                            &transform,
                            bx + 6.0,
                            by + if two_line { 3.0 } else { 13.0 },
                            w,
                            12.0,
                        ),
                        slot,
                        color_to_rgba(dim_text_color(palette.quote, alpha)),
                        font(10.5),
                        PaintAlign::Left,
                    );
                }
                if !value.is_empty() {
                    chrome.label(
                        stage_area(
                            &transform,
                            bx + 6.0,
                            by + if two_line { 16.0 } else { 13.0 },
                            w,
                            12.0,
                        ),
                        &value,
                        color_to_rgba(dim_text_color(palette.body, alpha)),
                        font(10.5),
                        PaintAlign::Left,
                    );
                }
            }
            // Приёмник: квалифицированный адрес истока (Объект.Поле)
            let qualified = truncate_chars(&self.stage_dst_label_text(edge, &src_title), 26);
            if !qualified.is_empty() {
                let w = qualified.chars().count() as f32 * 6.3 + 12.0;
                let bx = line.to[0] - 10.0 - w;
                let by = line.to[1] - 9.0;
                let mut fill = palette.menu_fill;
                fill[3] *= alpha;
                chrome.rect(
                    stage_area(&transform, bx, by, w, 18.0),
                    fill,
                    [0.0; 4],
                    4.0 * s,
                );
                chrome.label(
                    stage_area(&transform, bx + 6.0, by + 13.0, w, 12.0),
                    &qualified,
                    color_to_rgba(dim_text_color(palette.edge_label, alpha)),
                    font(10.5),
                    PaintAlign::Left,
                );
            }
        }
        paint_items_to_stage(
            chrome.take_items(),
            camera,
            viewport,
            zoom,
            &mut quads,
            &mut texts,
        );
        // 8) Пилюли подписей веера (FR-044 Р-1 + Q2): адресация + значение,
        // лейн-стопка в коридоре между колонками — общий расчёт с hit-
        // тестом клика ([`Self::stage_pill_state`], детерминизм); зона
        // клампа сжата верхом панели «Как считается» (Р-1 «между
        // заголовком и панелью»); подсветка Р-5 — пилюли вне фокуса
        // приглушены (Q3: с анимацией); переполнение — эшелоны Q2
        // (Compact — однострочные, Scroll — окно с индикаторами).
        // FR-068 (W3-продолжение): Painter-путь (stage_area; радиус 9 —
        // stage-локальные px, масштаб s)
        let (pill_rects, pill_mode) = self.stage_pill_state(stage, &ctx);
        let mut chrome = Painter::new();
        for (item, pill_rect) in pill_rects {
            let edge = &stage.slice.edges[item];
            let addr = truncate_chars(&self.stage_edge_addr_text(edge), 42);
            let value = truncate_chars(&self.stage_edge_value_text(edge), 42);
            let sel = selected_slice == Some(item);
            let alpha = edge_alpha(item);
            let mut fill = palette.edge_label_fill;
            fill[3] *= alpha;
            chrome.rect(
                stage_area(
                    &transform,
                    pill_rect.x,
                    pill_rect.y,
                    pill_rect.w,
                    pill_rect.h,
                ),
                fill,
                if sel { SELECTION_BORDER } else { [0.0; 4] },
                9.0 * s,
            );
            if matches!(pill_mode, calc_panel_ui::PillZoneMode::Full) {
                chrome.label(
                    stage_area(
                        &transform,
                        pill_rect.x + 12.0,
                        pill_rect.y + 5.0,
                        pill_rect.w - 16.0,
                        14.0,
                    ),
                    &addr,
                    color_to_rgba(dim_text_color(palette.title, alpha)),
                    font(12.0),
                    PaintAlign::Left,
                );
                if !value.is_empty() {
                    chrome.label(
                        stage_area(
                            &transform,
                            pill_rect.x + 12.0,
                            pill_rect.y + 18.0,
                            pill_rect.w - 16.0,
                            13.0,
                        ),
                        &value,
                        color_to_rgba(dim_text_color(palette.edge_label, alpha)),
                        font(11.0),
                        PaintAlign::Left,
                    );
                }
            } else {
                // Compact/Scroll: одна строка «адрес · значение»
                let combined = if value.is_empty() {
                    addr
                } else if addr.is_empty() {
                    value
                } else {
                    format!("{addr} · {value}")
                };
                let combined = truncate_chars(&combined, 56);
                chrome.label(
                    stage_area(
                        &transform,
                        pill_rect.x + 12.0,
                        pill_rect.y + (pill_rect.h - 12.0) / 2.0,
                        pill_rect.w - 16.0,
                        13.0,
                    ),
                    &combined,
                    color_to_rgba(dim_text_color(palette.title, alpha)),
                    font(11.0),
                    PaintAlign::Left,
                );
            }
        }
        // 8a) Q2 (Scroll): индикаторы «↑ ещё N» / «ещё N ↓» — клики листают
        // окно пилюль (hit-тест — те же rect'ы в click_main_stage)
        if matches!(pill_mode, calc_panel_ui::PillZoneMode::Scroll { .. }) {
            let (top_ind, bottom_ind) = self.stage_pill_scroll_indicators(stage, &ctx);
            for (ind, text_key) in [
                (top_ind, keys::STAGE_PILL_ABOVE),
                (bottom_ind, keys::STAGE_PILL_BELOW),
            ] {
                let Some(ind) = ind else { continue };
                let counts = match pill_mode {
                    calc_panel_ui::PillZoneMode::Scroll { above, below, .. } => (above, below),
                    _ => (0, 0),
                };
                let n = if text_key == keys::STAGE_PILL_ABOVE {
                    counts.0
                } else {
                    counts.1
                };
                chrome.rect(
                    stage_area(&transform, ind.x, ind.y, ind.w, ind.h),
                    palette.menu_fill,
                    palette.palette_border,
                    9.0 * s,
                );
                chrome.label(
                    stage_area(
                        &transform,
                        ind.x + 6.0,
                        ind.y + (ind.h - 11.0) / 2.0,
                        ind.w - 12.0,
                        12.0,
                    ),
                    &self.trf(text_key, &[("n", &n.to_string())]),
                    color_to_rgba(palette.quote),
                    font(10.0),
                    PaintAlign::Center,
                );
            }
        }
        paint_items_to_stage(
            chrome.take_items(),
            camera,
            viewport,
            zoom,
            &mut quads,
            &mut texts,
        );
        // 8b) Панель «Как считается» (FR-044 Р-4): screen-space каркас
        // у приёмника (низ stage), две группы — «Переменные · входящие
        // значения» (value-точка, квалифицированный адрес + значение,
        // unmapped — янтарный контур и «не подставлено») и «Расчёт ·
        // формулы» (маркер ƒ, формула с путями операндов). Подсветка Р-5:
        // строки фокуса — акцентная рамка, остальные приглушены (0.5).
        // FR-059 (волна 1 кита): раскладка строк — kit::list_rows
        // (calc_panel_ui, скролл вместо среза «… ещё N»); отрисовка —
        // через [`Painter`] (items → модальный проход stage,
        // `paint_items_to_stage`); ширины текстов — ИЗМЕРЕННЫЕ
        // (TextMeasurer/ellipsis — замена эвристики «6.3·символ»,
        // правило U5); бегунок — kit::scroll_bar (цвет — слот рамки).
        // FR-068 этап M2 (Table v2): строки обеих колонок — 2×Table
        // (retained; см. [`paint_calc_panel_rows`] — геометрия/стили
        // прежнего kit-цикла дословно, I-1; оракул T7).
        if let Some(panel) = &ctx.panel {
            // Фон панели — от НАЧАЛА ПАНЕЛИ (panel.rect — stage-локальный)
            let px = rect.x + panel.rect[0];
            let py = rect.y + panel.rect[1];
            // Заголовки/строки/бегунки — от НАЧАЛА STAGE: площади
            // раскладки (`vars_title`/`vars_area`/…) stage-локальные —
            // та же база, что у hit-тестов input.rs (`cursor − rect.x/y`);
            // до фикса они смещались на panel.rect дважды и рисовались за
            // нижней границей stage (контент панели не был виден).
            let ox = rect.x;
            let oy = rect.y;
            let mut m = canvas_ui::measure::TextMeasurer::new();
            let mut fs = canvas_render::text::measure_font_system();
            let mut d = Painter::new();
            let kit_palette = palette.kit_palette();
            // quote — вне среза кита (значение/ошибка/unmapped — прежние
            // слоты); Color → [f32;4] — представление (как в KitPalette)
            let c4 = |c: Color| {
                [
                    c.r() as f32 / 255.0,
                    c.g() as f32 / 255.0,
                    c.b() as f32 / 255.0,
                    c.a() as f32 / 255.0,
                ]
            };
            // FR-068 (W3-продолжение): каркас панели — Panel-стиль
            // компонентной модели (panel_style_of — явные слоты, F-8);
            // вывод тот же (fill/border/radius 10 — прежние)
            d.panel(
                canvas_ui::geometry::UiRect::new(px, py, panel.rect[2], panel.rect[3]),
                &kit::panel_style_of(palette.menu_fill, palette.palette_border, 10.0, 0.0),
            );
            d.label(
                canvas_ui::geometry::UiRect::new(
                    ox + panel.vars_title[0],
                    oy + panel.vars_title[1] + 2.0,
                    panel.vars_title[2],
                    16.0,
                ),
                self.tr(keys::STAGE_CALC_VARS),
                kit_palette.text_title,
                font(11.0),
                PaintAlign::Left,
            );
            d.label(
                canvas_ui::geometry::UiRect::new(
                    ox + panel.formulas_title[0],
                    oy + panel.formulas_title[1] + 2.0,
                    panel.formulas_title[2],
                    16.0,
                ),
                self.tr(keys::STAGE_CALC_FORMULAS),
                kit_palette.text_title,
                font(11.0),
                PaintAlign::Left,
            );
            let unmapped_text = self.tr(keys::STAGE_CALC_UNMAPPED).to_owned();
            // FR-068 этап M2 (Table v2): строки панели — 2×retained-Table
            // (создание при первом кадре панели — FontSystem::new() дорогой,
            // §4.6 дизайна; Props кегля/палитры — кадровые). Пересборка
            // rows из модели кадра + отрисовка ТОЛЬКО строк
            // ([`paint_calc_panel_rows`]); бегунки — прежний блок ниже
            // (draw-порядок кадра дословно: строки vars → строки формул →
            // оба бегунка). Ширины текстов — ИЗМЕРЕННЫЕ (TextMeasurer +
            // общий FontSystem рендера — §4.7).
            let kit_size = font(11.0);
            let mut calc_tables = self.stage_calc_tables.borrow_mut();
            paint_calc_panel_rows(
                &mut calc_tables,
                &mut d,
                &mut m,
                &mut fs,
                &ctx,
                panel,
                ox,
                oy,
                kit_size,
                &kit_palette,
                palette.search_row_fill,
                c4(palette.quote),
                &unmapped_text,
            );
            // FR-059: бегунки скролла колонок (кит scroll_bar) — переполнение
            // честно прокручивается, срез «… ещё N» удалён (цвет — слот рамки)
            for (area, scroll) in [
                (&panel.vars_area, &ctx.vars_scroll),
                (&panel.formulas_area, &ctx.formulas_scroll),
            ] {
                if let Some(knob) = canvas_ui::kit::scroll_bar(
                    canvas_ui::geometry::UiRect::new(area[0], area[1], area[2], area[3]),
                    scroll,
                    &kit_palette,
                ) {
                    d.rect(
                        canvas_ui::geometry::UiRect::new(ox + knob.x, oy + knob.y, knob.w, knob.h),
                        palette.palette_border,
                        [0.0; 4],
                        2.0,
                    );
                }
            }
            paint_items_to_stage(
                d.take_items(),
                camera,
                viewport,
                zoom,
                &mut quads,
                &mut texts,
            );
        }
        // 8c) Р-8: мини-карточки внешних источников под истоком — панель
        // «Как считается» полна, контекст внешних входов не теряется
        // (Q3: приглушение вне фокуса — с анимацией)
        if !ctx.model.ext_sources.is_empty() {
            let src = &stage.slice.nodes[0];
            let mut ext_y = src.y + src.height + 10.0;
            // FR-068 (W3-продолжение): Painter-путь мини-карточек (как у
            // пилюль/подписей — stage_area; радиус 8 — stage-локальные px)
            let mut chrome = Painter::new();
            for ext in &ctx.model.ext_sources {
                let focused =
                    ctx.focus.as_ref().is_some_and(|focus| {
                        ctx.model.vars.iter().enumerate().any(|(ri, var)| {
                            var.from_node == ext.from_node && focus.rows.contains(&ri)
                        })
                    });
                let dim_alpha = if ctx.focus.is_some() && !focused {
                    1.0 - 0.5 * ctx.dim
                } else {
                    1.0
                };
                let mut fill = palette.edge_label_fill;
                fill[3] *= dim_alpha;
                let card_w = src.width.min(220.0);
                chrome.rect(
                    stage_area(&transform, src.x, ext_y, card_w, 26.0),
                    fill,
                    if focused {
                        let mut border = SELECTION_BORDER;
                        border[3] *= ctx.dim;
                        border
                    } else {
                        palette.palette_border
                    },
                    8.0 * s,
                );
                chrome.label(
                    stage_area(&transform, src.x + 8.0, ext_y + 4.0, card_w - 16.0, 12.0),
                    &truncate_chars(&ext.title, 26),
                    color_to_rgba(dim_text_color(palette.body, dim_alpha)),
                    font(10.5),
                    PaintAlign::Left,
                );
                chrome.label(
                    stage_area(&transform, src.x + 8.0, ext_y + 15.0, card_w - 16.0, 11.0),
                    &self.trf(keys::STAGE_CALC_EXT, &[("n", &ext.count.to_string())]),
                    color_to_rgba(dim_text_color(palette.quote, dim_alpha)),
                    font(9.5),
                    PaintAlign::Left,
                );
                ext_y += 32.0;
            }
            paint_items_to_stage(
                chrome.take_items(),
                camera,
                viewport,
                zoom,
                &mut quads,
                &mut texts,
            );
        }
        // 9) Подсказка внизу stage (i18n, §7.2) — Painter-путь
        let mut chrome = Painter::new();
        chrome.label(
            UiRect::new(rect.x + 40.0, rect.y + rect.h - 26.0, rect.w - 80.0, 14.0),
            self.tr(keys::STAGE_FOOT_HINT),
            color_to_rgba(palette.quote),
            font(11.5),
            PaintAlign::Center,
        );
        paint_items_to_stage(
            chrome.take_items(),
            camera,
            viewport,
            zoom,
            &mut quads,
            &mut texts,
        );
        (quads, texts)
    }

    /// FR-042 (E3) + FR-044 Р-3/инвариант 5: адресная часть подписи ребра
    /// в stage — квалифицированный путь «Объект.Поле» (единая точка
    /// [`canvas_core::dataref::display_ref_for_edge`]: fromLine → «строка N»,
    /// fromOutput → имя выхода, fallback edge.id) и параметр-приёмник
    /// (toParam). Control-ребро — «to: <метка|имя приёмника>» (управление,
    /// не значение — value-путь не показывается). Значение — отдельно,
    /// второй строкой пилюли (FR-044).
    pub(super) fn stage_edge_addr_text(&self, edge: &Edge) -> String {
        let language = self.settings.language;
        if edge.flow_kind() != FlowKind::Value {
            // Инвариант 5: control-рёбра — «to: <слот>»; слота в модели нет —
            // метка ребра (если задана) или имя приёмника управления.
            if let Some(label) = edge.label.as_deref() {
                return i18n::trf(language, keys::STAGE_CTRL_TO, &[("node", label)]);
            }
            let dst_title = self
                .scene
                .canvas
                .node(&edge.to_node)
                .map(title_for)
                .unwrap_or_else(|| edge.to_node.clone());
            return i18n::trf(language, keys::STAGE_CTRL_TO, &[("node", &dst_title)]);
        }
        let counts = canvas_core::dataref::display_name_counts(&self.scene.canvas);
        let mut parts: Vec<String> =
            vec![
                canvas_core::dataref::display_ref_for_edge(&self.scene.canvas, edge, &counts)
                    .path(),
            ];
        if let Some(param) = edge.to_param.as_deref() {
            parts.push(i18n::trf(
                language,
                keys::STAGE_PARAM_LABEL,
                &[("param", param)],
            ));
        }
        parts.join(" · ")
    }

    /// FR-042 (E3): значение ребра в stage (FR-014/FR-025/FR-029; для
    /// fromLine — построчный результат Numi-листа источника, для
    /// fromOutput — именованный выход (исправление FR-044 Р-4: значение
    /// ПО АДРЕСУ ребра, а не узловой итог приёмника-источника)).
    /// FR-044 Р-3-а/инвариант 5: control-ребро значения не переносит —
    /// пустая строка (узловой итог истока не показывается на control).
    pub(super) fn stage_edge_value_text(&self, edge: &Edge) -> String {
        if edge.flow_kind() != FlowKind::Value {
            return String::new();
        }
        if let Some(line) = edge.from_line {
            return self
                .scene
                .expr_line_results
                .get(&edge.from_node)
                .and_then(|lines| lines.get(line))
                .and_then(|outcome| outcome.as_ref())
                .map(|outcome| match outcome {
                    ExprOutcome::Ok(value) => value.to_string(),
                    ExprOutcome::Err(msg) => msg.clone(),
                })
                .unwrap_or_default();
        }
        if let Some(output) = edge.from_output.as_deref() {
            // FR-064 P1: double buffer — снимок активных решений.
            return canvas_scene::read_flow(&self.scene.flow_active)
                .named
                .get(&(edge.from_node.clone(), output.to_owned()))
                .map(|value| value.to_string())
                .unwrap_or_default();
        }
        match self.scene.expr_results.get(&edge.from_node) {
            Some(ExprOutcome::Ok(value)) => value.to_string(),
            Some(ExprOutcome::Err(msg)) => msg.clone(),
            None => String::new(),
        }
    }

    /// FR-042 (E3): открыть main stage, если ребро — часть пучка веса ≥ 2
    /// и агрегация включена (F-13). true — stage открыт (клик поглощён);
    /// false — одиночное ребро/агрегация выключена (поведение прежнее).
    pub(super) fn try_open_main_stage(&mut self, edge_index: usize) -> bool {
        if !self.settings.edge_aggregation {
            return false;
        }
        // X5 (§6.5): в режиме защиты другие оверлеи недоступны —
        // открытие stage глушится (канвас не изменяется, AC-6.4)
        if self.explain.as_ref().is_some_and(|s| s.is_defense()) {
            return false;
        }
        match MainStageState::open(&self.scene.canvas, &self.scene.bundles, edge_index) {
            Some(stage) => {
                self.main_stage = Some(stage);
                // FR-044 Р-5: свежий stage — без подсветки предыдущего
                self.stage_calc_focus = None;
                self.stage_calc_hover = None;
                // FR-059: скроллы колонок панели — с начала
                self.stage_calc_vars_scroll = canvas_ui::kit::ScrollState::default();
                self.stage_calc_formulas_scroll = canvas_ui::kit::ScrollState::default();
                // FR-044 Q2/Q3: окно пилюль и анимация подсветки — с начала
                self.stage_pill_scroll = 0;
                self.stage_calc_render = (None, 0.0);
                self.stage_calc_fade = None;
                // PRD-0007 (F-10, У10): stage и окно проверки взаимо-
                // исключительны; снапшот остаётся в сессионном кэше —
                // возврат через «?» мгновенный (≤ 1 с)
                self.close_explain();
                self.bundle_hover = None;
                self.request_redraw();
                true
            }
            None => false,
        }
    }

    /// FR-042 (E3): закрыть main stage (Esc/клик по фону/перед открытием
    /// оверлея — Q6). Выделение ребра сохраняется (AC-3.2). Подсветка
    /// зависимостей умирает вместе со stage (FR-044 Р-5, инвариант 8).
    pub(super) fn close_main_stage(&mut self) {
        if self.main_stage.take().is_some() {
            self.stage_calc_focus = None;
            self.stage_calc_hover = None;
            // FR-044 Q2/Q3: окно пилюль и анимация — состояние stage
            self.stage_pill_scroll = 0;
            self.stage_calc_render = (None, 0.0);
            self.stage_calc_fade = None;
            self.request_redraw();
        }
    }

    /// FR-044 Р-5: hit-тест пилюль веера (rect'ы — тот же расчёт, что в
    /// кадре). Возвращает (индекс ребра среза, rect) или None.
    pub(super) fn stage_pill_hit(
        &self,
        stage: &MainStageState,
        ctx: &StageFrameCtx,
        local: [f32; 2],
    ) -> Option<(usize, StageLocalRect)> {
        for (item, rect) in self.stage_pill_rects(stage, ctx) {
            if local[0] >= rect.x
                && local[0] <= rect.x + rect.w
                && local[1] >= rect.y
                && local[1] <= rect.y + rect.h
            {
                return Some((item, rect));
            }
        }
        None
    }

    /// Миникарта: центрирование + drag.
    pub(super) fn click_minimap(&mut self) {
        // Миникарта (T13, SPEC §6.1): клик — центрирование камеры,
        // drag — world-точка под курсором следует за ним. Квад
        // рисуется поверх всего — проверка до канвас-хит-тестов
        if let Some(rect) = self.minimap_rect() {
            if point_in_rect(
                [rect[0], rect[1], rect[2] - rect[0], rect[3] - rect[1]],
                self.cursor,
            ) {
                self.center_camera_on_minimap_cursor();
                self.minimap_drag = true;
                self.request_redraw();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // T7: семейство замера — то же, что шейпинг (прежний kit-цикл панели
    // брал SANS_FAMILY из импортов app.rs; после M2 production-путь берёт
    // семейство из TableProps — импорт нужен только референсу теста).
    use canvas_render::text::SANS_FAMILY;

    /// T7 (дизайн `fr-068-table-v2.md` §7, оракул M2): журнал Painter
    /// пути Table ([`paint_calc_panel_rows`]: `paint_rows_with` обеих
    /// таблиц в порядке stage) ≡ референсу — ручному kit-циклу прежнего
    /// кода панели (row_guides → row_layout → paint_row с теми же
    /// стилями). Равенство `Vec<PaintItem>` — побитовое (I-1).
    /// Параметры — точно stage: row_h 22 (`PANEL_ROW_H`), right_pad
    /// 6/0, кегль 11, leader off; направляющие — от ПРАВОГО КРАЯ
    /// ВЬЮПОРТА в координатах слотов (слоты панели — экранные, x≠0);
    /// формулы — деградация §4.2 (ручной вход
    /// `RowGuides{value_x: unit_x: slot.right()}` в референсе).
    /// Модель синтетическая: vars Ok/Err/Unmapped с прокруткой на
    /// полстроки (частичные строки — клип-семантика §4.3), формулы без
    /// значений, фокус (var 0 + formula 0) и dim 0.5 (приглушение Q3 +
    /// рамка фокуса с коэффициентом).
    #[test]
    fn t7_panel_rows_table_path_matches_manual_kit_loop() {
        // Модель: 20 vars (окно 13 строк — прокрутка активна), 2 формулы
        let mut model = calc_panel_ui::CalcPanelModel::default();
        for i in 0..20 {
            let value = match i % 3 {
                0 => RowValue::Ok(format!("{i}0")),
                1 => RowValue::Err(format!("ошибка {i}")),
                _ => RowValue::Unmapped,
            };
            model.vars.push(calc_panel_ui::VarRow {
                edge_index: i,
                from_node: format!("n{i}"),
                slot: Some(0),
                param: None,
                path: format!("Заявки.поле{i}"),
                in_bundle: true,
                value,
            });
        }
        model.formulas.push(calc_panel_ui::FormulaRow {
            line: 0,
            name: "x".into(),
            display: "x = Заявки.поле0 · Заявки.поле1".into(),
            operand_edges: vec![0, 1],
        });
        model.formulas.push(calc_panel_ui::FormulaRow {
            line: 1,
            name: "y".into(),
            display: "y = x + Заявки.поле2".into(),
            operand_edges: vec![2],
        });
        // Раскладка панели — реальная чистая функция; скролл vars — на
        // полстроки (частичные строки сверху/снизу — усечение §4.3)
        let mut vars_scroll = canvas_ui::kit::ScrollState::default();
        let mut formulas_scroll = canvas_ui::kit::ScrollState::default();
        vars_scroll.scroll_by(11.0);
        let panel = calc_panel_layout(
            &model,
            1344.0,
            756.0,
            96.0,
            &mut vars_scroll,
            &mut formulas_scroll,
        )
        .expect("панель построена");
        assert!(vars_scroll.offset > 0.0, "прокрутка vars активна");
        // Фокус: var 0 + formula 0 (индекс vars.len()); dim 0.5
        let ctx = StageFrameCtx {
            lines: Vec::new(),
            model: model.clone(),
            panel: Some(panel.clone()),
            vars_scroll: vars_scroll.clone(),
            formulas_scroll: formulas_scroll.clone(),
            zone: StageLocalRect {
                x: 0.0,
                y: 56.0,
                w: 1344.0,
                h: 400.0,
            },
            focus: Some(StageCalcFocus {
                rows: std::collections::BTreeSet::from([0, model.vars.len()]),
                edges: std::collections::BTreeSet::from([0]),
            }),
            dim: 0.5,
            pill_mode: calc_panel_ui::PillZoneMode::Full,
        };
        let settings = Settings::default();
        let palette = ThemeColors::from_theme(settings.theme);
        let kit_palette = palette.kit_palette();
        // quote — вне среза кита (Color → [f32;4], как c4 в stage_frame)
        let quote = {
            let c = palette.quote;
            [
                c.r() as f32 / 255.0,
                c.g() as f32 / 255.0,
                c.b() as f32 / 255.0,
                c.a() as f32 / 255.0,
            ]
        };
        let unmapped_text = i18n::tr(settings.language, keys::STAGE_CALC_UNMAPPED);
        // Слоты панели — stage-локальные (раскладка), отрисовка — от
        // начала stage (та же база, что в stage_frame: ox/oy = rect.x/y)
        let rect = main_stage_rect([1920.0, 1080.0]);
        let ox = rect.x;
        let oy = rect.y;
        let kit_size = 11.0; // font(11.0) при scale = 1

        // --- путь Table (stage): paint_calc_panel_rows (lazy-таблицы) ---
        let mut tables: Option<(canvas_ui::kit::Table, canvas_ui::kit::Table)> = None;
        let mut d_table = Painter::new();
        let mut m = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        paint_calc_panel_rows(
            &mut tables,
            &mut d_table,
            &mut m,
            &mut fs,
            &ctx,
            &panel,
            ox,
            oy,
            kit_size,
            &kit_palette,
            palette.search_row_fill,
            quote,
            unmapped_text,
        );
        let table_items = d_table.take_items();
        assert!(!table_items.is_empty(), "журнал Table-пути не пуст");
        {
            // Retained-таблицы созданы при первом использовании; скролл —
            // источник истины ctx; content_h — инвариант set_rows
            let (vars_table, _) = tables.as_ref().expect("таблицы созданы lazy");
            assert_eq!(
                vars_table.scroll.offset, vars_scroll.offset,
                "offset — из ctx (источник истины)"
            );
            assert_eq!(
                vars_table.scroll.content_h,
                model.vars.len() as f32 * calc_panel_ui::PANEL_ROW_H
            );
        }

        // --- референс: ручной kit-цикл (прежний код stage, D-15) ---
        let mut d_ref = Painter::new();
        let mut m_ref = canvas_ui::measure::TextMeasurer::new();
        let kit_opts = canvas_ui::kit::RowOpts {
            leader: false,
            ..canvas_ui::kit::RowOpts::default()
        };
        let row_focused = |row: usize| ctx.focus.as_ref().is_some_and(|f| f.rows.contains(&row));
        let row_alpha = |focused: bool| -> f32 {
            if ctx.focus.is_some() && !focused {
                1.0 - 0.5 * ctx.dim
            } else {
                1.0
            }
        };
        let focus_border = || {
            let mut border = SELECTION_BORDER;
            border[3] *= ctx.dim;
            border
        };
        let vars_viewport = UiRect::new(
            ox + panel.vars_area[0],
            oy + panel.vars_area[1],
            panel.vars_area[2],
            panel.vars_area[3],
        );
        let var_parts: Vec<canvas_ui::kit::RowParts<'_>> = model
            .vars
            .iter()
            .map(|var| canvas_ui::kit::RowParts {
                marker: canvas_ui::kit::RowMarker::Dot,
                label: &var.path,
                value: match &var.value {
                    RowValue::Ok(text) => text.as_str(),
                    RowValue::Err(err) => err.as_str(),
                    RowValue::Unmapped => unmapped_text,
                },
                unit: "",
                badge: "",
            })
            .collect();
        // Направляющие — от правого края ВЬЮПОРТА минус right_pad 6
        // (viewport_right в координатах слотов — те же, что у Table)
        let var_guides = canvas_ui::kit::row_guides(
            &mut m_ref,
            &mut fs,
            SANS_FAMILY,
            kit_size,
            &var_parts,
            vars_viewport.right() - 6.0,
            canvas_core::tokens::TABLE_GUIDE_GAP,
        );
        for (index, row) in &panel.var_rows {
            let Some(g) = var_guides else { break };
            let var = &model.vars[*index];
            let focused = row_focused(*index);
            let alpha = row_alpha(focused);
            let slot = UiRect::new(ox + row[0], oy + row[1], row[2], row[3]);
            let lay = canvas_ui::kit::row_layout(
                &mut m_ref,
                &mut fs,
                SANS_FAMILY,
                kit_size,
                slot,
                g,
                &var_parts[*index],
                &kit_opts,
            );
            let unmapped = var.value == RowValue::Unmapped;
            let mut fill = palette.search_row_fill;
            fill[3] *= alpha;
            let mut style = canvas_ui::kit::row_style(
                if focused {
                    canvas_ui::kit::KitState::Selected
                } else {
                    canvas_ui::kit::KitState::Normal
                },
                &kit_palette,
            );
            style.fill = fill;
            style.border = if focused {
                focus_border()
            } else if unmapped {
                UNMAPPED_EDGE_COLOR
            } else {
                [0.0; 4]
            };
            let (dot_fill, value_fill) = match &var.value {
                RowValue::Ok(_) => (FLOW_EDGE_COLOR, kit_palette.text),
                RowValue::Err(_) => (FLOW_EDGE_COLOR, kit_palette.control_danger),
                RowValue::Unmapped => (UNMAPPED_EDGE_COLOR, quote),
            };
            style.marker = dim_color4(dot_fill, alpha);
            style.value = dim_color4(value_fill, alpha);
            style.label = dim_color4(kit_palette.text, alpha);
            canvas_ui::kit::paint_row(&mut d_ref, &lay, &var_parts[*index], &style, kit_size);
        }
        let formula_parts: Vec<canvas_ui::kit::RowParts<'_>> = model
            .formulas
            .iter()
            .map(|formula| canvas_ui::kit::RowParts {
                marker: canvas_ui::kit::RowMarker::Glyph("ƒ"),
                label: &formula.display,
                value: "",
                unit: "",
                badge: "",
            })
            .collect();
        for (index, row) in &panel.formula_rows {
            let slot = UiRect::new(ox + row[0], oy + row[1], row[2], row[3]);
            // Направляющие не нужны (значения нет) — деградированный вход
            let g = canvas_ui::row_guides::RowGuides {
                value_w: 0.0,
                unit_w: 0.0,
                badge_w: 0.0,
                value_x: slot.right(),
                unit_x: slot.right(),
            };
            let lay = canvas_ui::kit::row_layout(
                &mut m_ref,
                &mut fs,
                SANS_FAMILY,
                kit_size,
                slot,
                g,
                &formula_parts[*index],
                &kit_opts,
            );
            let focused = row_focused(model.vars.len() + *index);
            let alpha = row_alpha(focused);
            let mut fill = palette.search_row_fill;
            fill[3] *= alpha;
            let mut style = canvas_ui::kit::row_style(
                if focused {
                    canvas_ui::kit::KitState::Selected
                } else {
                    canvas_ui::kit::KitState::Normal
                },
                &kit_palette,
            );
            style.fill = fill;
            style.border = if focused { focus_border() } else { [0.0; 4] };
            style.marker = dim_color4(kit_palette.text_title, alpha);
            style.label = dim_color4(kit_palette.text, alpha);
            canvas_ui::kit::paint_row(&mut d_ref, &lay, &formula_parts[*index], &style, kit_size);
        }
        // Оракул T7: журналы равны побитово (строки vars → строки формул)
        let ref_items = d_ref.take_items();
        assert_eq!(
            table_items, ref_items,
            "журнал Table-пути ≡ ручному kit-циклу (I-1)"
        );
    }
}
