//! AI-кластер приложения: explain-поверхность (разбор выделения),
//! autolink-ревью (предложенные связки) и их ховер-pill.
//!
//! Выделены из `app.rs` сессией рефакторинга 2026-09-25 (этап 6) без
//! изменения поведения: это методы `App`, работающие с тем же состоянием.
//! Паттерн дочернего модуля — как `ui_registry` (FR-052): `use super::*`
//! даёт доступ к приватным полям `App` и импортам родителя.

use super::*;

// FR-060 (W-c): кит-геометрия прохода — rect'ы Painter'а в лог. px
use canvas_ui::geometry::UiRect;

impl App {
    /// Открыть окно проверки (§6.4): сессионный кэш — мгновенный Ready
    /// (AC-3.3, ≤ 1 с), иначе Loading с честным лоадером + фоновая сборка
    /// (AC-1.2/G5). Повторный «?» на другую цифру — перестройка на новый
    /// корень (У6: одна панель — один корень); main stage закрывается
    /// (F-10 — оверлеи взаимоисключительны, снапшот остаётся в кэше).
    pub(super) fn open_explain(&mut self, root: LineageNodeId) {
        // F-10 (Q6): открытие оверлея закрывает main stage
        self.close_main_stage();
        let revision = self.scene.revision;
        // FR-083: направление схемы — из настроек (`explain_sources_left`);
        // pan у нового состояния уже нулевой (открытие оверлея).
        let direction =
            explain_ui::LayoutDirection::from_sources_left(self.settings.explain_sources_left);
        if let Some(snap) = self.explain_cache.take() {
            if snap.root == root && snap.revision == revision {
                // Переоткрытие из кэша: Ready сразу, чип — если модель
                // всё-таки изменилась (from_snapshot сравнивает ревизии);
                // дельты what-if восстанавливаются из снапшота (AC-4.2)
                let mut state = ExplainState::from_snapshot(snap, revision);
                state.direction = direction;
                self.explain = Some(state);
                self.request_redraw();
                return;
            }
            // Чужой/устаревший снапшот не нужен: новый закэшируется при
            // закрытии окна (гигиена памяти — держим только последний)
        }
        // X3 (AC-4.2): при активном what-if база (flow_baseline) строится
        // тем же фоновым проходом — дельты в дереве Ready.
        // FR-064 P1: double buffer — снимки через read()-гарды; spawn_
        // lineage_build клонирует их под гардом (потоку — собственные копии).
        // Гард'ы — в блоке: после клонирования они не нужны (Drop-типы
        // держат заимствование до конца скоупа).
        let build = {
            let base_buf = self
                .scene
                .whatif_active
                .then(|| canvas_scene::read_flow(&self.scene.flow_baseline));
            let active_buf = canvas_scene::read_flow(&self.scene.flow_active);
            spawn_lineage_build(
                &self.scene.canvas,
                &active_buf,
                base_buf.as_deref(),
                self.scene.flow_cycle.as_ref(),
                root.clone(),
            )
        };
        self.explain = Some(ExplainState::loading(root, revision, build));
        if let Some(state) = self.explain.as_mut() {
            state.direction = direction;
        }
        self.request_redraw();
    }

    /// Создать связи из принятых предложений — ОДИН undo-бат (AC-5.3,
    /// паттерн FR-033/FR-006): один `push_undo` с тегом
    /// [`AUTOLINK_UNDO_TAG`] ДО мутации, затем рёбра адресованного
    /// проливания (`fromOutput` = `toParam` = имя присваивания, FR-029).
    /// Откат пачки — с подтверждением и подсветкой отменяемого
    /// (`undo_action` перехватывает по тегу верхнего снапшота).
    pub(super) fn create_autolink_edges(&mut self, accepted: Vec<canvas_core::AutolinkProposal>) {
        if accepted.is_empty() {
            return;
        }
        let snapshot = self.scene.canvas.clone();
        self.scene.set_undo_tag(AUTOLINK_UNDO_TAG);
        // FR-006: push_undo ДО мутации — один снапшот на всю пачку
        self.scene.push_undo(snapshot);
        let mut created: Vec<String> = Vec::new();
        for proposal in &accepted {
            let id = self.scene.canvas.next_edge_id();
            let mut edge = canvas_core::Edge::new(
                id,
                proposal.from_node.as_str(),
                None,
                proposal.to_node.as_str(),
                None,
            );
            edge.set_flow_kind(canvas_core::FlowKind::Value);
            edge.from_output = Some(proposal.param.clone());
            edge.to_param = Some(proposal.param.clone());
            created.push(edge.id.clone());
            self.scene.canvas.add_edge(edge);
        }
        // Живой пересчёт (дельта-семантика X3 не нужна: модель изменилась,
        // панель/оверлеи закрыты или обновятся чипом по новой ревизии)
        self.scene.recompute_flow();
        self.scene.mark_dirty();
        // Пачка запоминается для подсветки отката (AC-5.3); тег снимет
        // инвалидацию при любом другом действии
        self.autolink_batch = Some(created);
        let n = accepted.len();
        self.show_toast(
            self.trf(
                keys::AUTOLINK_TOAST_CREATED,
                &[("{n}", n.to_string().as_str())],
            )
            .to_owned(),
        );
        self.request_redraw();
    }

    /// Кадр диалога ревью (модальный проход, паттерн explain_frame):
    /// затемнение + окно + шапка + баннер + строки + футер. Одна геометрия
    /// с on_autolink_click (чистые layout-функции autolink_ui).
    pub(super) fn autolink_frame(
        &mut self,
        viewport: [f32; 2],
    ) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut quads: Vec<CardInstance> = Vec::new();
        let mut texts: Vec<OwnedScreenText> = Vec::new();
        let Some(review) = self.autolink_review.as_ref() else {
            return (quads, texts);
        };
        // FR-060 (волна 2 кита): отрисовка — Painter (данные canvas-ui, G7)
        // + WidgetState (состояния строк/кнопок). Правка дрейфа 2026-09-25:
        // кадр диалога уходит в СТАДИЙНЫЙ проход (stage_instances —
        // world-конвенция, рендерер дописывает без конверсии), поэтому
        // конвертация — paint_items_to_stage (screen→world здесь, как у
        // explain_window/stage_frame). Прежний paint_items_to_band оставлял
        // квад СЫРЫМ — рендер рисовал его как world, и при панорамировании
        // диалог «приклеивался» к канвасу, расъезжаясь со screen-текстами.
        let mut d = Painter::new();
        let palette = ThemeColors::from_theme(self.settings.theme);
        let win = autolink_ui::dialog_rect(viewport);
        // Затемнение фона (§6.5: диалог модален поверх канваса)
        d.rect(
            canvas_ui::geometry::UiRect::new(0.0, 0.0, viewport[0], viewport[1]),
            palette.stage_dim,
            [0.0; 4],
            0.0,
        );
        // Окно (стиль модалок: радиус 14)
        d.rect(
            canvas_ui::geometry::UiRect::new(win[0], win[1], win[2], win[3]),
            palette.menu_fill,
            palette.palette_border,
            14.0,
        );
        let (accepted, rejected, pending) = review.counts();
        // Шапка: заголовок + мета + ✕
        d.label(
            canvas_ui::geometry::UiRect::new(
                win[0] + 16.0,
                win[1] + 10.0,
                (win[2] - 120.0).max(120.0),
                20.0,
            ),
            self.tr(keys::AUTOLINK_TITLE),
            color_to_rgba(palette.title),
            15.0,
            PaintAlign::Left,
        );
        d.label(
            canvas_ui::geometry::UiRect::new(
                win[0] + 16.0,
                win[1] + 32.0,
                (win[2] - 120.0).max(120.0),
                15.0,
            ),
            &self.trf(
                keys::AUTOLINK_META,
                &[("{n}", review.items.len().to_string().as_str())],
            ),
            color_to_rgba(palette.quote),
            11.0,
            PaintAlign::Left,
        );
        let close = autolink_ui::close_rect(win);
        d.rect(
            canvas_ui::geometry::UiRect::new(close[0], close[1], close[2], close[3]),
            [0.0; 4],
            palette.palette_border,
            7.0,
        );
        d.label(
            canvas_ui::geometry::UiRect::new(close[0], close[1] + 2.0, close[2], 18.0),
            "×",
            color_to_rgba(palette.body),
            14.0,
            PaintAlign::Center,
        );
        // Баннер отклонённых (У8): виден, пока есть отклонённые.
        // FR-UI-BANNER: kit::banner (через autolink_ui::banner_layout) —
        // канон геометрии (rect, label_area, action_button) и стиля
        // (control_danger tint fill, control_danger border, control_danger
        // label_color). Канонизация: fill = tinted alpha 0.10 (был
        // transparent); radius RADIUS_PANEL=10 (был 8); action_button =
        // (text_w+24)×30 (было RESTORE_W×26). См. autolink_ui::banner_layout.
        if rejected > 0 {
            let banner_text = self.trf(
                keys::AUTOLINK_BANNER,
                &[("{n}", rejected.to_string().as_str())],
            );
            let restore_text = self.tr(keys::AUTOLINK_RESTORE_ALL);
            let mut m = canvas_ui::measure::TextMeasurer::new();
            let mut fs = canvas_render::text::measure_font_system();
            let label_w = m.width_of(
                &mut fs,
                &banner_text,
                canvas_render::text::SANS_FAMILY,
                11.5,
            );
            let action_w = m.width_of(
                &mut fs,
                restore_text,
                canvas_render::text::SANS_FAMILY,
                11.0,
            );
            drop(fs);
            let (lay, style) =
                autolink_ui::banner_layout(win, label_w, action_w, &palette.kit_palette());
            // Фон баннера — kit::paint_banner вернул бы Vec<PaintItem> с 1
            // Rect; здесь — d.rect напрямую с kit-style слотами.
            d.rect(lay.rect, style.fill, style.border, style.radius);
            // Подпись баннера (label_area + label_color kit-слот).
            d.label(
                lay.label_area,
                &banner_text,
                style.label_color,
                11.5,
                PaintAlign::Left,
            );
            // Action button «Вернуть все» — geometry kit (action_button
            // rect), стиль прежний (border-only + body-text): kit::paint_banner
            // рисует ТОЛЬКО фон баннера, action_button — забота потребителя.
            d.rect(lay.action_button, [0.0; 4], palette.palette_border, 6.0);
            d.label(
                UiRect::new(
                    lay.action_button.x,
                    lay.action_button.y + 4.0,
                    lay.action_button.w,
                    16.0,
                ),
                restore_text,
                color_to_rgba(palette.body),
                11.0,
                PaintAlign::Center,
            );
        }
        // Строки (группы «исток → приёмник», сортировка по имени — У7)
        let layout = autolink_ui::rows_layout(review, win, self.autolink_scroll);
        for (group_idx, head) in &layout.group_heads {
            let collapsed = review.collapsed.contains(group_idx);
            d.rect(
                canvas_ui::geometry::UiRect::new(head[0], head[1], head[2], head[3]),
                palette.palette_row_fill,
                palette.palette_border,
                6.0,
            );
            let group = &review.groups[*group_idx];
            d.label(
                canvas_ui::geometry::UiRect::new(
                    head[0] + 10.0,
                    head[1] + 8.0,
                    (head[2] - 20.0).max(0.0),
                    16.0,
                ),
                &format!(
                    "{}  →  {} · {}{}",
                    group.from,
                    group.to,
                    group.items.len(),
                    if collapsed { " ▸" } else { " ▾" }
                ),
                color_to_rgba(palette.title),
                12.0,
                PaintAlign::Left,
            );
        }
        for (item_idx, rects) in &layout.rows {
            let item = &review.items[*item_idx];
            // Принятое — акцентная рамка (будет создано); отклонённое —
            // приглушённый текст; нерешённое — обычная карточка.
            // FR-060: состояние строки — WidgetState → KitState::Selected
            // (рамка акцентом), деградация отклонённого — текст quote
            let mut row_state = WidgetState::default();
            row_state.set_selected(item.state == ItemState::Accepted);
            let selected = row_state.kit_state() == kit::KitState::Selected;
            let row_border = if selected {
                palette.accent
            } else {
                palette.palette_border
            };
            d.rect(
                canvas_ui::geometry::UiRect::new(
                    rects.row[0],
                    rects.row[1],
                    rects.row[2],
                    rects.row[3],
                ),
                palette.card_fill,
                row_border,
                6.0,
            );
            // «имя → приёмник» + процент/единицы.
            // БЕЗ дублирования имени параметра: прежний формат
            // `param → to (параметр param)` показывал имя дважды.
            // Локализованная подпись «параметр {name}» используется как
            // левая часть (информативнее голого имени).
            let param_label = self.trf(
                keys::AUTOLINK_PARAM,
                &[("{name}", item.proposal.param.as_str())],
            );
            d.label(
                canvas_ui::geometry::UiRect::new(
                    rects.row[0] + 10.0,
                    rects.row[1] + 8.0,
                    (rects.pct[0] - rects.row[0] - 20.0).max(0.0),
                    16.0,
                ),
                &format!("{} → {}", param_label, item.to_title),
                color_to_rgba(if item.state == ItemState::Rejected {
                    palette.quote
                } else {
                    palette.body
                }),
                11.5,
                PaintAlign::Left,
            );
            let pct_text = match item.proposal.unit_match {
                Some(true) => "100% ✓".to_owned(),
                Some(false) => "100% ✗".to_owned(),
                None => format!("{}%", item.proposal.percent),
            };
            d.rect(
                canvas_ui::geometry::UiRect::new(
                    rects.pct[0],
                    rects.pct[1],
                    rects.pct[2],
                    rects.pct[3],
                ),
                palette.palette_chip_fill,
                [0.0; 4],
                9.0,
            );
            d.label(
                canvas_ui::geometry::UiRect::new(
                    rects.pct[0],
                    rects.pct[1] + 3.0,
                    rects.pct[2],
                    14.0,
                ),
                &pct_text,
                color_to_rgba(palette.body),
                10.0,
                PaintAlign::Center,
            );
            // Кнопки строки — WidgetState::Selected у активной
            let accept_on = item.state == ItemState::Accepted;
            let reject_on = item.state == ItemState::Rejected;
            let mut accept_state = WidgetState::default();
            accept_state.set_selected(accept_on);
            let accept_on_kit = accept_state.kit_state() == kit::KitState::Selected;
            d.rect(
                canvas_ui::geometry::UiRect::new(
                    rects.accept[0],
                    rects.accept[1],
                    rects.accept[2],
                    rects.accept[3],
                ),
                if accept_on_kit {
                    palette.accent
                } else {
                    [0.0; 4]
                },
                if accept_on_kit {
                    palette.accent
                } else {
                    palette.palette_border
                },
                6.0,
            );
            d.label(
                canvas_ui::geometry::UiRect::new(
                    rects.accept[0],
                    rects.accept[1] + 3.0,
                    rects.accept[2],
                    15.0,
                ),
                self.tr(keys::AUTOLINK_ACCEPT),
                color_to_rgba(if accept_on_kit {
                    palette.text_on_accent
                } else {
                    palette.body
                }),
                10.5,
                PaintAlign::Center,
            );
            let mut reject_state = WidgetState::default();
            reject_state.set_selected(reject_on);
            let reject_on_kit = reject_state.kit_state() == kit::KitState::Selected;
            d.rect(
                canvas_ui::geometry::UiRect::new(
                    rects.reject[0],
                    rects.reject[1],
                    rects.reject[2],
                    rects.reject[3],
                ),
                if reject_on_kit {
                    color_to_rgba(palette.error)
                } else {
                    [0.0; 4]
                },
                if reject_on_kit {
                    color_to_rgba(palette.error)
                } else {
                    palette.palette_border
                },
                6.0,
            );
            d.label(
                canvas_ui::geometry::UiRect::new(
                    rects.reject[0],
                    rects.reject[1] + 3.0,
                    rects.reject[2],
                    15.0,
                ),
                self.tr(keys::AUTOLINK_REJECT),
                color_to_rgba(if reject_on_kit {
                    palette.text_on_accent
                } else {
                    palette.body
                }),
                10.5,
                PaintAlign::Center,
            );
        }
        // Разделитель футера + подсказка + кнопки
        let footer = autolink_ui::footer_rect(win);
        d.rect(
            canvas_ui::geometry::UiRect::new(footer[0], footer[1], footer[2], 1.0),
            palette.palette_border,
            [0.0; 4],
            0.0,
        );
        d.label(
            canvas_ui::geometry::UiRect::new(
                footer[0] + 16.0,
                footer[1] + 10.0,
                // Ширина подсказки: не перекрывать кнопки футера.
                // Прежняя формула `footer[2] - 3*FOOT_BTN_W - 40` давала
                // 366px — подсказка наезжала на «Отклонить все» (левая
                // кнопка начинается на 448px от правого края). Корректная
                // ширина = footer_w − правый отступ (16) − CREATE_W − 2·(
                // FOOT_BTN_W + 10) − левый отступ (16) − зазор (10).
                (footer[2]
                    - autolink_ui::CREATE_W
                    - 2.0 * autolink_ui::FOOT_BTN_W
                    - 2.0 * 10.0
                    - 16.0
                    - 16.0
                    - 10.0)
                    .max(120.0),
                15.0,
            ),
            self.tr(keys::AUTOLINK_HINT),
            color_to_rgba(palette.quote),
            10.5,
            PaintAlign::Left,
        );
        let [create, accept_all, reject_all] = autolink_ui::footer_buttons(win);
        // Кнопка «Создать связи (N)» — WidgetState: Selected при accepted>0
        let mut create_state = WidgetState::default();
        create_state.set_selected(accepted > 0);
        let create_on = create_state.kit_state() == kit::KitState::Selected;
        d.rect(
            canvas_ui::geometry::UiRect::new(create[0], create[1], create[2], create[3]),
            if create_on {
                palette.accent
            } else {
                palette.palette_chip_fill
            },
            [0.0; 4],
            7.0,
        );
        d.label(
            canvas_ui::geometry::UiRect::new(create[0], create[1] + 6.0, create[2], 16.0),
            &self.trf(
                keys::AUTOLINK_CREATE,
                &[("{n}", accepted.to_string().as_str())],
            ),
            color_to_rgba(if create_on {
                palette.text_on_accent
            } else {
                palette.quote
            }),
            12.0,
            PaintAlign::Center,
        );
        for (rect, label) in [
            (&accept_all, keys::AUTOLINK_ACCEPT_ALL),
            (&reject_all, keys::AUTOLINK_REJECT_ALL),
        ] {
            d.rect(
                canvas_ui::geometry::UiRect::new(rect[0], rect[1], rect[2], rect[3]),
                [0.0; 4],
                palette.palette_border,
                7.0,
            );
            d.label(
                canvas_ui::geometry::UiRect::new(rect[0], rect[1] + 6.0, rect[2], 16.0),
                self.tr(label),
                color_to_rgba(palette.body),
                11.5,
                PaintAlign::Center,
            );
        }
        let _ = pending;
        // Правка дрейфа 2026-09-25: stage-проход — world-конвенция (см. комментарий выше)
        let zoom = self.camera.zoom();
        paint_items_to_stage(
            d.take_items(),
            &self.camera,
            viewport,
            zoom,
            &mut quads,
            &mut texts,
        );
        (quads, texts)
    }

    /// Кадр окна проверки (§6.4) — модальный проход кадра (паттерн
    /// stage_frame: квады + screen-тексты, рендерер выводит поверх всего).
    /// Loading: окно + честный лоадер (кольцо + ротация подписей), канвас
    /// НЕ затемняется (У5 — затемнение и подсветка атомарны с деревом).
    /// Ready: дерево (ветки-безье + карточки узлов), чип Stale, футер.
    ///
    /// FR-060 (W-c, последний остаток волны): проход собирается через кит
    /// [`Painter`] — items → `paint_items_to_stage` (конвенция world,
    /// радиус/размер ÷ zoom — дословно прежние `screen_rect_quad`), а
    /// состояния (hover/selected) — через [`WidgetState`] → `KitState`
    /// (машина состояний FR-057; переходы указателя ведёт потребитель,
    /// стиль выбирается по матрице кита). Цвета — прежние слоты (0
    /// визуального скачка). Кружки-точки: у Painter точечного примитива
    /// нет — ветки-безье идут квадратами с радиусом d/2 ([`dot_rect`],
    /// геометрия `screen_dot`), кольцо лоадера — прямые `screen_dot`
    /// после flush (в Loading-ветке точки — последние квады прохода,
    /// порядок рисования сохранён дословно).
    pub(super) fn explain_frame(
        &mut self,
        viewport: [f32; 2],
    ) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut quads: Vec<CardInstance> = Vec::new();
        let mut texts: Vec<OwnedScreenText> = Vec::new();
        // Присваивается в ветке Ready; ранние выходы его не читают
        let cursor_idx;
        {
            let Some(state) = self.explain.as_ref() else {
                return (quads, texts);
            };
            let palette = ThemeColors::from_theme(self.settings.theme);
            let camera = &self.camera;
            let zoom = camera.zoom();
            // FR-060: draw-журнал кита — items конвертируются в инстансы
            // stage одним проходом в конце ветки (порядок = draw-порядок)
            let mut d = Painter::new();
            let win = explain_ui::window_rect(viewport);
            // Заголовок корня — из живой модели (тот же title_for, что у
            // подписей проливания); в Loading дерева ещё нет
            let root_title = self
                .scene
                .canvas
                .node(&state.root.node_id)
                .map(title_for)
                .unwrap_or_else(|| "—".to_owned());
            // Окно (затемнение фона НЕ рисуем: в Ready затемняет цепочку
            // FocusView (F-4), в Loading затемнения нет вообще — У5)
            d.rect(
                UiRect::new(win[0], win[1], win[2], win[3]),
                palette.menu_fill,
                palette.palette_border,
                14.0,
            );
            // Шапка: заголовок + крошки + ✕ + чип Stale
            d.label(
                UiRect::new(
                    win[0] + 16.0,
                    win[1] + 10.0,
                    (win[2] - 240.0).max(120.0),
                    18.0,
                ),
                self.tr(keys::EXPLAIN_TITLE),
                color_to_rgba(palette.title),
                15.0,
                PaintAlign::Left,
            );
            // Мета-строка шапки: X6 — полные чипы-крошки пути вида (по чипу
            // на уровень, клик по чипу — обрезка пути, AC-2.3); без фокуса
            // (путь в корень) и в защите — прежний текст-подзаголовок.
            // Чипы и hit — одна чистая геометрия crumb_rects (детерминизм).
            let meta = if state.is_ready() {
                let tree = state.tree().expect("готово");
                let path: Vec<String> = state
                    .view_path
                    .iter()
                    .filter_map(|&i| tree.nodes.get(i).map(|n| n.title.clone()))
                    .collect();
                if path.len() > 1 {
                    path.join(" → ")
                } else {
                    self.trf(keys::EXPLAIN_META, &[("title", root_title.as_str())])
                }
            } else {
                self.trf(keys::EXPLAIN_META, &[("title", root_title.as_str())])
            };
            let show_crumbs = state.is_ready() && !state.is_defense() && state.view_path.len() > 1;
            if show_crumbs {
                let tree = state.tree().expect("готово");
                // FR-UI-CRUMBS: kit::crumbs (через explain_ui::crumb_rects) —
                // измеряет ширины чипов по тексту, переполнение отбрасывает
                // корневые уровни + prepend'ит «…» (канон kit, замена прежнего
                // silent-drop CRUMB_MAX_CHIPS). path_labels собирается здесь
                // (title узлов пути вида) — рендер использует label из tuple.
                let path_labels: Vec<String> = state
                    .view_path
                    .iter()
                    .filter_map(|&i| tree.nodes.get(i).map(|n| n.title.clone()))
                    .collect();
                let mut m = canvas_ui::measure::TextMeasurer::new();
                let mut fs = canvas_render::text::measure_font_system();
                let crumbs = explain_ui::crumb_rects(win, &path_labels, &mut m, &mut fs);
                drop(fs);
                let last = state.view_path.len().saturating_sub(1);
                for (i, (rect, label)) in crumbs.iter().enumerate() {
                    // «…» crumb — не соответствует уровню, только рисуется.
                    let Some(level) =
                        explain_ui::crumb_path_index(i, state.view_path.len(), &crumbs)
                    else {
                        // Рисуем «…» crumb (без акцентной подсветки).
                        d.rect(*rect, [0.0; 4], palette.palette_border, 6.0);
                        d.label(
                            UiRect::new(rect.x, rect.y + 2.5, rect.w, 13.0),
                            label,
                            color_to_rgba(palette.body),
                            10.5,
                            PaintAlign::Center,
                        );
                        continue;
                    };
                    // FR-060: текущая крошка — WidgetState::Selected
                    let current = level == last;
                    let mut crumb_state = WidgetState::default();
                    crumb_state.set_selected(current);
                    let selected = crumb_state.kit_state() == kit::KitState::Selected;
                    d.rect(
                        *rect,
                        if selected { palette.accent } else { [0.0; 4] },
                        palette.palette_border,
                        6.0,
                    );
                    d.label(
                        UiRect::new(rect.x + 6.0, rect.y + 2.5, (rect.w - 10.0).max(8.0), 13.0),
                        label,
                        if selected {
                            color_to_rgba(palette.text_on_accent)
                        } else {
                            color_to_rgba(palette.body)
                        },
                        10.5,
                        PaintAlign::Left,
                    );
                }
            } else {
                d.label(
                    UiRect::new(
                        win[0] + 16.0,
                        win[1] + 32.0,
                        (win[2] - 240.0).max(120.0),
                        14.0,
                    ),
                    &meta,
                    color_to_rgba(palette.quote),
                    11.5,
                    PaintAlign::Left,
                );
            }
            let close = explain_ui::close_rect(win);
            d.rect(
                UiRect::new(close[0], close[1], close[2], close[3]),
                [0.0; 4],
                palette.palette_border,
                7.0,
            );
            d.label(
                UiRect::new(close[0], close[1] + 2.0, close[2], 17.0),
                "×",
                color_to_rgba(palette.body),
                14.0,
                PaintAlign::Center,
            );
            // Чип «Данные изменены» (AC-3.3/F-5): модель изменилась после
            // сборки — канвас и дерево не перерисовываются сами
            if state.is_ready() && state.revision != self.scene.revision {
                let chip = explain_ui::chip_rect(win);
                d.rect(
                    UiRect::new(chip[0], chip[1], chip[2], chip[3]),
                    color_to_rgba(palette.whatif_badge),
                    palette.palette_border,
                    14.0,
                );
                d.label(
                    UiRect::new(chip[0], chip[1] + 5.0, chip[2], 14.0),
                    self.tr(keys::EXPLAIN_STALE),
                    color_to_rgba(palette.text_on_accent),
                    11.5,
                    PaintAlign::Center,
                );
            }
            // --- Loading: честный лоадер (AC-1.2, У5) -------------------
            if state.is_loading() {
                let body = explain_ui::body_rect(win);
                let cx = body[0] + body[2] / 2.0;
                let cy = body[1] + body[3] / 2.0 - 20.0;
                let elapsed = state.opened_at.elapsed().as_millis();
                let spin = (elapsed as f32 / 900.0) * std::f32::consts::TAU;
                // FR-060: хром окна — Painter; кольцо лоадера — прямые
                // `screen_dot` ПОСЛЕ flush Painter'а (точечного примитива у
                // Painter нет; в Loading-ветке точки — последние квады
                // прохода, порядок рисования сохранён дословно).
                paint_items_to_stage(
                    d.take_items(),
                    camera,
                    viewport,
                    zoom,
                    &mut quads,
                    &mut texts,
                );
                for i in 0..10 {
                    let angle = spin + i as f32 * std::f32::consts::TAU / 10.0;
                    let p = [cx + angle.cos() * 14.0, cy + angle.sin() * 14.0];
                    let mut fill = palette.accent;
                    fill[3] = 0.25 + 0.75 * (i as f32 / 10.0);
                    quads.push(screen_dot(camera, viewport, p, 6.0, fill));
                }
                let captions = [
                    self.tr(keys::EXPLAIN_LOADER_1),
                    self.tr(keys::EXPLAIN_LOADER_2),
                    self.tr(keys::EXPLAIN_LOADER_3),
                    self.tr(keys::EXPLAIN_LOADER_4),
                    self.tr(keys::EXPLAIN_LOADER_5),
                    self.tr(keys::EXPLAIN_LOADER_6),
                    self.tr(keys::EXPLAIN_LOADER_7),
                    self.tr(keys::EXPLAIN_LOADER_8),
                    self.tr(keys::EXPLAIN_LOADER_9),
                    self.tr(keys::EXPLAIN_LOADER_10),
                    self.tr(keys::EXPLAIN_LOADER_11),
                    self.tr(keys::EXPLAIN_LOADER_12),
                ];
                texts.push(OwnedScreenText {
                    text: state.loader_caption(&captions).to_owned(),
                    origin: [body[0], cy + 34.0],
                    width: body[2],
                    font_size: 12.5,
                    color: palette.body,
                    align: TextAlign::Center,
                });
                return (quads, texts);
            }
            // --- Ready: дерево (ветки + карточки), F-5 ------------------
            let Some(tree) = state.tree() else {
                paint_items_to_stage(
                    d.take_items(),
                    camera,
                    viewport,
                    zoom,
                    &mut quads,
                    &mut texts,
                );
                return (quads, texts);
            };
            let body = explain_ui::body_rect(win);
            // X5: вид/масштаб/база контента едины с hit-тестами
            // (explain_view); в защите — defense_reveal + укрупнение ×1.5
            // (AC-6.2/6.3). FR-084: 4-е значение — content_origin (BODY_PAD
            // + центрирование оси, где контент влезает, + пан после
            // pan_clamp): переполнение закрывается панорамированием.
            let (vis, layout, scale, origin) = self.explain_view(state, body);
            let local = |x: f32, y: f32| {
                [
                    body[0] + explain_ui::BODY_PAD + x * scale + origin[0],
                    body[1] + explain_ui::BODY_PAD + y * scale + origin[1],
                ]
            };
            // FR-083: направление схемы (сторона источников/итога) — от
            // него зависят бейдж фронтира и полоса-акцент карточки.
            let ltr = state.direction == explain_ui::LayoutDirection::Ltr;
            // Ветки — под карточками (порядок рисования): безье из локальных
            // px лейаута → screen → мир (паттерн polyline_dots: линия —
            // цепочка перекрывающихся кружков)
            for curve in &layout.curves {
                let samples = bezier_samples(curve.points, 36);
                let mut fill = if curve.to_leaf {
                    palette.explain_leaf
                } else {
                    palette.accent
                };
                fill[3] = if curve.to_leaf { 0.9 } else { 0.55 };
                for p in samples {
                    let sp = local(p[0], p[1]);
                    // FR-060: кружок — Painter-rect ([`dot_rect`]: квадрат
                    // d×d с радиусом d/2 — та же геометрия, что screen_dot;
                    // точки должны идти ПОД карточками — только через
                    // draw-журнал Painter'а)
                    let (dot, radius) = dot_rect(sp, 4.0);
                    d.rect(dot, fill, [0.0; 4], radius);
                }
            }
            // Разделитель футера + статистика видимого дерева
            let footer_line = [win[0], win[1] + win[3] - explain_ui::FOOTER_H, win[2], 1.0];
            d.rect(
                UiRect::new(
                    footer_line[0],
                    footer_line[1],
                    footer_line[2],
                    footer_line[3],
                ),
                palette.palette_border,
                [0.0; 4],
                0.0,
            );
            d.label(
                UiRect::new(win[0] + 16.0, win[1] + win[3] - 28.0, 280.0, 16.0),
                &self.trf(
                    keys::EXPLAIN_STATS,
                    &[
                        ("lv", layout.levels.to_string().as_str()),
                        ("n", layout.nodes.len().to_string().as_str()),
                    ],
                ),
                color_to_rgba(palette.quote),
                13.0,
                PaintAlign::Left,
            );
            // FR-083: тумблер направления схемы в шапке (левее defense-
            // тумблера) — стрелка в сторону потока: «→» — источники слева
            // (Ltr), «←» — источники справа (Rtl). На узких окнах (кнопка
            // заходит в мета-зону) — не рисуется и не ловит хит.
            if explain_ui::direction_toggle_visible(win) {
                let toggle = explain_ui::direction_toggle_rect(win);
                // FR-060: hover тумблера — машина состояний виджета
                let mut toggle_state = WidgetState::default();
                toggle_state.set_pointer(point_in_rect(toggle, self.cursor), false);
                let hovered = toggle_state.kit_state() == kit::KitState::Hovered;
                d.rect(
                    UiRect::new(toggle[0], toggle[1], toggle[2], toggle[3]),
                    if hovered {
                        palette.accent
                    } else {
                        palette.card_fill
                    },
                    if hovered {
                        [0.0; 4]
                    } else {
                        palette.palette_border
                    },
                    14.0,
                );
                d.label(
                    UiRect::new(toggle[0], toggle[1] + 7.5, toggle[2], 16.0),
                    self.tr(if ltr {
                        keys::EXPLAIN_DIR_LTR
                    } else {
                        keys::EXPLAIN_DIR_RTL
                    }),
                    if hovered {
                        color_to_rgba(palette.text_on_accent)
                    } else {
                        color_to_rgba(palette.body)
                    },
                    13.0,
                    PaintAlign::Center,
                );
            }
            // X5 (AC-6.1): тумблер режима защиты в шапке — одним действием
            {
                let toggle = explain_ui::defense_toggle_rect(win);
                let on = state.is_defense();
                // FR-060: hover + включённость — матрица кита (Hovered >
                // Selected): акцент при любом непустом состоянии
                let mut toggle_state = WidgetState::default();
                toggle_state.set_pointer(point_in_rect(toggle, self.cursor), false);
                toggle_state.set_selected(on);
                let active = toggle_state.kit_state() != kit::KitState::Normal;
                d.rect(
                    UiRect::new(toggle[0], toggle[1], toggle[2], toggle[3]),
                    if active {
                        palette.accent
                    } else {
                        palette.card_fill
                    },
                    if on { [0.0; 4] } else { palette.palette_border },
                    14.0,
                );
                d.label(
                    UiRect::new(toggle[0], toggle[1] + 7.5, toggle[2], 16.0),
                    self.tr(if on {
                        keys::EXPLAIN_DEFENSE_EXIT
                    } else {
                        keys::EXPLAIN_DEFENSE
                    }),
                    if active {
                        color_to_rgba(palette.text_on_accent)
                    } else {
                        color_to_rgba(palette.body)
                    },
                    13.0,
                    PaintAlign::Center,
                );
            }
            // X5 (AC-6.3): кнопки шага и «Раскрыть всё» + подсказка — футер
            // защиты; крошки в защите глушатся (вид на корне)
            if state.is_defense() {
                let can_step = explain_ui::has_hidden(&vis);
                let step = explain_ui::defense_step_rect(win);
                let all = explain_ui::defense_all_rect(win);
                let buttons = [
                    (step, keys::EXPLAIN_DEFENSE_STEP, can_step),
                    (all, keys::EXPLAIN_DEFENSE_ALL, true),
                ];
                for (rect, key, enabled) in buttons {
                    // FR-060: состояние кнопки — машина виджета: Disabled
                    // глушит hover (матрица Disabled > Hovered)
                    let mut btn_state = WidgetState::default();
                    btn_state.set_pointer(point_in_rect(rect, self.cursor), false);
                    btn_state.set_disabled(!enabled);
                    let ks = btn_state.kit_state();
                    d.rect(
                        UiRect::new(rect[0], rect[1], rect[2], rect[3]),
                        if ks == kit::KitState::Hovered {
                            palette.accent
                        } else {
                            palette.card_fill
                        },
                        if ks == kit::KitState::Disabled {
                            [0.0; 4]
                        } else {
                            palette.palette_border
                        },
                        6.0,
                    );
                    d.label(
                        UiRect::new(rect[0], rect[1] + 6.5, rect[2], 16.0),
                        self.tr(key),
                        if ks == kit::KitState::Hovered {
                            color_to_rgba(palette.text_on_accent)
                        } else if ks == kit::KitState::Disabled {
                            color_to_rgba(palette.quote)
                        } else {
                            color_to_rgba(palette.body)
                        },
                        13.0,
                        PaintAlign::Center,
                    );
                }
                d.label(
                    UiRect::new(
                        win[0] + 320.0,
                        win[1] + win[3] - 27.0,
                        (win[2] - 480.0).max(120.0),
                        15.0,
                    ),
                    self.tr(keys::EXPLAIN_DEFENSE_HINT),
                    color_to_rgba(palette.quote),
                    12.0,
                    PaintAlign::Center,
                );
            }
            // Карточки узлов (F-3: значение + формула + адрес)
            for laid in &layout.nodes {
                let node = &tree.nodes[laid.idx];
                let rect = {
                    let [x, y] = local(laid.rect[0], laid.rect[1]);
                    [x, y, laid.rect[2] * scale, laid.rect[3] * scale]
                };
                let hovered = state.cursor == Some(laid.idx);
                // У2 (§6.5, X6): узел, выбранный кликом по подсвеченной
                // ноде канваса — рамка выделения (акцент; пока вспышка
                // не погасла — ярче, alpha от pick_flash_at)
                let picked = state.pick == Some(laid.idx);
                // Цвет полосы рода узла: расчётный — акцент, лист — слот
                // explain_leaf (контраст ≥ 3:1, AC-3.4), терминалы —
                // ошибка/предупреждение
                let strip = match node.kind {
                    canvas_core::LineageNodeKind::Calc => palette.accent,
                    canvas_core::LineageNodeKind::Leaf => palette.explain_leaf,
                    canvas_core::LineageNodeKind::Cycle => color_to_rgba(palette.error),
                    canvas_core::LineageNodeKind::Unmapped
                    | canvas_core::LineageNodeKind::Unlinked
                    | canvas_core::LineageNodeKind::Truncated => {
                        color_to_rgba(palette.whatif_badge)
                    }
                };
                // FR-060: состояние карточки — машина виджета (курсор —
                // Hovered, клик по подсвеченной ноде — Selected); рамка
                // акцентом при любом непустом состоянии (матрица кита)
                let mut card_state = WidgetState::default();
                card_state.set_pointer(hovered, false);
                card_state.set_selected(picked);
                let emphasized = card_state.kit_state() != kit::KitState::Normal;
                d.rect(
                    UiRect::new(rect[0], rect[1], rect[2], rect[3]),
                    palette.card_fill,
                    if emphasized {
                        palette.accent
                    } else {
                        palette.palette_border
                    },
                    8.0,
                );
                // У2-вспышка: затухающая рамка поверх выделения (700 мс);
                // после затухания остаётся только рамка выделения выше.
                if picked {
                    let flash = state.pick_flash_at(Instant::now());
                    if flash > 0.0 {
                        let mut glow = palette.accent;
                        glow[3] = flash;
                        d.rect(
                            UiRect::new(rect[0], rect[1], rect[2], rect[3]),
                            [0.0; 4],
                            glow,
                            12.0,
                        );
                    }
                }
                // Полоса-акцент на стороне РОДИТЕЛЯ (FR-083): Rtl — слева
                // (родитель левее), Ltr — справа (родитель правее).
                let strip_w = (4.0 * scale).max(2.0);
                let strip_rect = if ltr {
                    [rect[0] + rect[2] - strip_w, rect[1], strip_w, rect[3]]
                } else {
                    [rect[0], rect[1], strip_w, rect[3]]
                };
                d.rect(
                    UiRect::new(strip_rect[0], strip_rect[1], strip_rect[2], strip_rect[3]),
                    strip,
                    [0.0; 4],
                    0.0,
                );
                // Ревизия владельца 2026-10-02 (дефект «текст неадекватно
                // меняется при зуме»): паддинги карточки масштабируются
                // вместе с геометрией (прежде 10/16 px были захардкожены
                // в экранных px — на fit-масштабе 0.7 адресная строка
                // висела НИЖЕ карточки, при зуме 2.5 — слипалась вверху).
                let font = |px: f32| (px * scale).max(8.0);
                let tx = rect[0] + 10.0 * scale;
                let text_w = (rect[2] - 16.0 * scale).max(0.0);
                // FR-084: карточка-таблица — сестринские листья одного
                // узла-источника (одинаковый node_id под одним родителем):
                // шапка с заголовком узла + по строке на лист; кривые уже
                // приходят в порты строк (рендер кривых не меняется).
                if !laid.rows.is_empty() {
                    // Шапка: заголовок узла в верхней зоне TABLE_HEADER_H
                    d.label(
                        UiRect::new(tx, rect[1] + 6.0 * scale, text_w, 16.0),
                        &node.title,
                        color_to_rgba(palette.title),
                        font(12.0),
                        PaintAlign::Left,
                    );
                    // Разделитель шапки: линия по нижней границе зоны
                    // заголовка (полупрозрачный бордер)
                    let mut sep = palette.palette_border;
                    sep[3] *= 0.5;
                    d.rect(
                        UiRect::new(
                            rect[0],
                            rect[1] + explain_ui::TABLE_HEADER_H * scale,
                            rect[2],
                            1.0,
                        ),
                        sep,
                        [0.0; 4],
                        0.0,
                    );
                    for row in &laid.rows {
                        let leaf = &tree.nodes[row.leaf_idx];
                        let row_font = (10.0 * scale).max(8.0);
                        // Вертикальный центр строки (оптическая поправка +1 px)
                        let row_y =
                            rect[1] + row.y * scale + (row.h * scale - row_font) / 2.0 + 1.0;
                        // Текст строки не заходит в защищённую зону иконки
                        let row_w = explain_ui::table_row_text_width(rect[2], scale);
                        // Значение строки: дельта what-if (AC-4.2) — формат
                        // FR-017 «было → стало (+Δ)», цвет бейджа; Err —
                        // текст ошибки; Ok — цифра
                        let delta_str = match (
                            &leaf.value,
                            state.deltas.get(&(leaf.node_id.clone(), leaf.line)),
                        ) {
                            (Some(Ok(v)), Some(d)) => Some(
                                canvas_core::expr::whatif_full_delta(&d.base, &d.whatif)
                                    .unwrap_or_else(|| v.to_string()),
                            ),
                            _ => None,
                        };
                        // Адрес via строки (без дельты) — та же композиция,
                        // что в адресной строке обычной карточки; via —
                        // ребро родитель→лист (как в layout_tree)
                        let mut text = if delta_str.is_none() {
                            let via = vis.parent[row.leaf_idx].and_then(|p| {
                                tree.nodes[p]
                                    .children
                                    .iter()
                                    .find(|c| c.child == row.leaf_idx)
                                    .and_then(|c| c.via.clone())
                            });
                            match via {
                                Some(via) => {
                                    let mut addr = if let Some(name) = &via.from_output {
                                        self.trf(
                                            keys::EXPLAIN_ADDR_OUTPUT,
                                            &[("name", name.as_str())],
                                        )
                                    } else if let Some(line) = via.from_line {
                                        self.trf(
                                            keys::EXPLAIN_ADDR_SLOT,
                                            &[("n", (line + 1).to_string().as_str())],
                                        )
                                    } else {
                                        String::new()
                                    };
                                    if let Some(param) = &via.to_param {
                                        if !addr.is_empty() {
                                            addr.push_str(" · ");
                                        }
                                        addr.push_str(param);
                                    }
                                    addr
                                }
                                None => String::new(),
                            }
                        } else {
                            String::new()
                        };
                        // Значение строки и её цвет: дельта — бейдж what-if,
                        // ошибка — error, обычное значение — body
                        let (value_str, value_color) = match delta_str {
                            Some(s) => (s, palette.whatif_badge),
                            None => match &leaf.value {
                                Some(Ok(v)) => (v.to_string(), palette.body),
                                Some(Err(e)) => (e.clone(), palette.error),
                                None => (String::new(), palette.body),
                            },
                        };
                        // Строка читается целиком: «выход kafka · 220 usd» —
                        // имя и значение видны вместе; без адреса — просто
                        // значение (лист-константа — компакт, без пометки)
                        if !value_str.is_empty() && !text.contains(&value_str) {
                            if !text.is_empty() {
                                text.push_str(" · ");
                            }
                            text.push_str(&value_str);
                        }
                        if !text.is_empty() {
                            d.label(
                                UiRect::new(tx, row_y, row_w, 14.0),
                                &text,
                                color_to_rgba(value_color),
                                row_font,
                                PaintAlign::Left,
                            );
                        }
                        // Иконка-карандаш — только у редактируемых строк
                        // (лист со строкой Numi-листа и Ok-значением);
                        // геометрия — row_edit_rect, одна для рендера и
                        // hit-теста (детерминизм)
                        let editable = leaf.kind == canvas_core::LineageNodeKind::Leaf
                            && leaf.line.is_some()
                            && matches!(&leaf.value, Some(Ok(_)));
                        if editable {
                            // Ревизия владельца 2026-10-02: зона иконки
                            // отступает от полосы рода узла на её ширину
                            // (полоса на стороне иконки — правый край в Ltr,
                            // левый в Rtl — зона не трогается); рендер и
                            // hit-тест (edit_at) — одна геометрия.
                            let strip_inset = explain_ui::edit_icon_right_inset(scale, ltr);
                            let icon =
                                explain_ui::row_edit_rect(rect, row.y, row.h, scale, strip_inset);
                            // FR-060: hover иконки — машина состояний виджета
                            let mut icon_state = WidgetState::default();
                            icon_state.set_pointer(point_in_rect(icon, self.cursor), false);
                            let icon_hovered = icon_state.kit_state() == kit::KitState::Hovered;
                            d.rect(
                                UiRect::new(icon[0], icon[1], icon[2], icon[3]),
                                if icon_hovered {
                                    palette.accent
                                } else {
                                    [0.0; 4]
                                },
                                if icon_hovered {
                                    [0.0; 4]
                                } else {
                                    palette.palette_border
                                },
                                4.0,
                            );
                            // FR-085: SVG-иконка карандаша (FR-ICONS,
                            // screen-space физ. px) вместо текстового глифа
                            // (U+270E нет в вшитом шрифте — рисовался пустой
                            // квадрат). Фолбэк на глиф — если пара (набор,
                            // edit) в атласе нет (пользовательский набор без
                            // иконки) — мягкая деградация, паттерн KitDraw.
                            let tint = if icon_hovered {
                                [1.0, 1.0, 1.0, 1.0]
                            } else {
                                color_to_rgba(palette.body)
                            };
                            let icon_set = self.icon_set_active();
                            let uv = icon_set.and_then(|set| canvas_render::icon_uv(set, "edit"));
                            if let Some((uv_min, uv_max)) = uv {
                                // Квадратная вписка по центру зоны (паттерн
                                // KitDraw::icon); логические px → физические.
                                let sf = self.scale_factor();
                                let size = (icon[2].min(icon[3]) * 0.62) * sf;
                                let pos = [
                                    (icon[0] + (icon[2] - size / sf) * 0.5) * sf,
                                    (icon[1] + (icon[3] - size / sf) * 0.5) * sf,
                                ];
                                self.icon_instances.push(canvas_render::IconInstance {
                                    pos,
                                    size: [size, size],
                                    uv_min,
                                    uv_max,
                                    tint,
                                });
                            } else {
                                let icon_font = (11.0 * scale).max(8.0);
                                d.label(
                                    UiRect::new(
                                        icon[0],
                                        icon[1] + (icon[3] - icon_font) / 2.0,
                                        icon[2],
                                        icon_font + 2.0,
                                    ),
                                    explain_ui::EDIT_ICON_GLYPH,
                                    if icon_hovered {
                                        color_to_rgba(palette.text_on_accent)
                                    } else {
                                        color_to_rgba(palette.body)
                                    },
                                    icon_font,
                                    PaintAlign::Center,
                                );
                            }
                        }
                    }
                    // Бейдж фронтира таблице не нужен (листья без детей),
                    // обычный рендер ниже не выполняется
                    continue;
                }
                // 1) Заголовок ноды-таблицы
                d.label(
                    UiRect::new(tx, rect[1] + 7.0 * scale, text_w, 16.0),
                    &node.title,
                    color_to_rgba(palette.title),
                    font(12.0),
                    PaintAlign::Left,
                );
                // 2) Значение (Ok — цифра; Err — диагностика; None — метка
                // терминального узла: «цикл»/«не связано»/…)
                // X3 (AC-4.2): на затронутых узлах — дельта what-if в
                // формате FR-017 «было → стало (+Δ)» (цвет бейджа what-if)
                let (value_str, value_color) = match &node.value {
                    Some(Ok(v)) => match state.deltas.get(&(node.node_id.clone(), node.line)) {
                        Some(d) => (
                            canvas_core::expr::whatif_full_delta(&d.base, &d.whatif)
                                .unwrap_or_else(|| v.to_string()),
                            palette.whatif_badge,
                        ),
                        None => (v.to_string(), palette.title),
                    },
                    Some(Err(e)) => (e.clone(), palette.error),
                    None => (
                        match node.kind {
                            canvas_core::LineageNodeKind::Cycle => {
                                self.tr(keys::EXPLAIN_CYCLE).to_owned()
                            }
                            canvas_core::LineageNodeKind::Unmapped => {
                                self.tr(keys::EXPLAIN_UNMAPPED).to_owned()
                            }
                            canvas_core::LineageNodeKind::Unlinked => {
                                self.tr(keys::EXPLAIN_UNLINKED).to_owned()
                            }
                            canvas_core::LineageNodeKind::Truncated => {
                                self.tr(keys::EXPLAIN_TRUNCATED).to_owned()
                            }
                            _ => String::new(),
                        },
                        palette.error,
                    ),
                };
                d.label(
                    UiRect::new(tx, rect[1] + 25.0 * scale, text_w, 18.0),
                    &value_str,
                    color_to_rgba(value_color),
                    font(13.5),
                    PaintAlign::Left,
                );
                // 3) Формула узла/строки (у листа и терминалов нет)
                if let Some(formula) = &node.formula {
                    d.label(
                        UiRect::new(tx, rect[1] + 44.0 * scale, text_w, 14.0),
                        formula,
                        color_to_rgba(palette.body),
                        font(10.5),
                        PaintAlign::Left,
                    );
                } else if node.kind == canvas_core::LineageNodeKind::Leaf {
                    // Лист-константа: пометка «исходное значение» (AC-1.4)
                    d.label(
                        UiRect::new(tx, rect[1] + 44.0 * scale, text_w, 14.0),
                        self.tr(keys::EXPLAIN_LEAF_TAG),
                        color_to_rgba(palette.quote),
                        font(10.5),
                        PaintAlign::Left,
                    );
                }
                // 4) Адресная строка ребра (AC-2.1: квалифицированный адрес)
                if let Some(via) = &laid.via {
                    let mut addr = if let Some(name) = &via.from_output {
                        self.trf(keys::EXPLAIN_ADDR_OUTPUT, &[("name", name.as_str())])
                    } else if let Some(line) = via.from_line {
                        self.trf(
                            keys::EXPLAIN_ADDR_SLOT,
                            &[("n", (line + 1).to_string().as_str())],
                        )
                    } else {
                        String::new()
                    };
                    if let Some(param) = &via.to_param {
                        if !addr.is_empty() {
                            addr.push_str(" · ");
                        }
                        addr.push_str(param);
                    }
                    if !addr.is_empty() {
                        d.label(
                            UiRect::new(tx, rect[1] + 58.0 * scale, text_w, 12.0),
                            &addr,
                            color_to_rgba(palette.quote),
                            font(9.5),
                            PaintAlign::Left,
                        );
                    }
                }
                // Бейдж фронтира «+N глубже» (AC-2.3: ручное разворачивание)
                // на стороне ДЕТЕЙ (FR-083): Rtl — правый нижний угол,
                // Ltr — левый нижний.
                if vis.frontier[laid.idx] {
                    let bw = 66.0;
                    let bh = 15.0;
                    let badge = if ltr {
                        [rect[0] + 6.0, rect[1] + rect[3] - bh - 5.0, bw, bh]
                    } else {
                        [
                            rect[0] + rect[2] - bw - 6.0,
                            rect[1] + rect[3] - bh - 5.0,
                            bw,
                            bh,
                        ]
                    };
                    d.rect(
                        UiRect::new(badge[0], badge[1], badge[2], badge[3]),
                        palette.accent,
                        [0.0; 4],
                        7.0,
                    );
                    d.label(
                        UiRect::new(badge[0], badge[1] + 1.5, bw, 12.0),
                        &self.trf(
                            keys::EXPLAIN_EXPAND_BADGE,
                            &[("n", vis.hidden_descendants[laid.idx].to_string().as_str())],
                        ),
                        color_to_rgba(palette.text_on_accent),
                        9.5,
                        PaintAlign::Center,
                    );
                }
                // X3 (AC-4.1): старая текстовая кнопка «Изменить» удалена —
                // FR-084 заменяет её иконками-карандашами у строк таблиц
                // (edit_rect из рендера больше не вызывается; hit-тест
                // строк — за вводом через row_edit_rect)
            }
            // FR-083: индикатор «не всё влезло» — стрелки у краёв тела со
            // скрытым контентом + подсказка в футере справа. КЛИП: у
            // модального прохода (Painter → stage-инстансы) скиссоры нет
            // (scissor FR-056 — только полосы реестра; Painter::clip_rect
            // — прозрачный проход до scissor-конвертации), контент
            // рисуется как есть; пан ограничен pan_clamp, индикатор
            // обязателен.
            // FR-084: стрелки переполнения — оконная семантика в координатах
            // пана (не origin), поведение прежнее — пан клампится локально.
            let pan = explain_ui::pan_clamp(state.pan, layout.bounds, scale, body);
            let arrows = explain_ui::overflow_arrows(pan, layout.bounds, scale, body);
            if arrows.iter().any(|&a| a) {
                const ARROW: f32 = 22.0;
                const EDGE: f32 = 6.0;
                let cy = body[1] + body[3] / 2.0 - ARROW / 2.0;
                let cx = body[0] + body[2] / 2.0 - ARROW / 2.0;
                let edge_arrows = [
                    (arrows[0], [body[0] + EDGE, cy, ARROW, ARROW], "←"),
                    (
                        arrows[1],
                        [body[0] + body[2] - ARROW - EDGE, cy, ARROW, ARROW],
                        "→",
                    ),
                    (arrows[2], [cx, body[1] + EDGE, ARROW, ARROW], "↑"),
                    (
                        arrows[3],
                        [cx, body[1] + body[3] - ARROW - EDGE, ARROW, ARROW],
                        "↓",
                    ),
                ];
                for (shown, rect, glyph) in edge_arrows {
                    if !shown {
                        continue;
                    }
                    d.rect(
                        UiRect::new(rect[0], rect[1], rect[2], rect[3]),
                        palette.accent,
                        [0.0; 4],
                        7.0,
                    );
                    d.label(
                        UiRect::new(rect[0], rect[1] + 3.0, rect[2], 15.0),
                        glyph,
                        color_to_rgba(palette.text_on_accent),
                        12.0,
                        PaintAlign::Center,
                    );
                }
                // Подсказка в футере справа (в защите — левее кнопок футера);
                // origin считается от правого края (Right-выравнивания в
                // OwnedScreenText нет — сдвигает origin вручную).
                const HINT_W: f32 = 320.0;
                let hint_right = if state.is_defense() {
                    explain_ui::defense_all_rect(win)[0] - 10.0
                } else {
                    win[0] + win[2] - explain_ui::BODY_PAD
                };
                d.label(
                    UiRect::new(hint_right - HINT_W, win[1] + win[3] - 28.0, HINT_W, 15.0),
                    self.tr(keys::EXPLAIN_OVERFLOW_HINT),
                    color_to_rgba(palette.quote),
                    12.0,
                    PaintAlign::Left,
                );
            }
            // Hover узла дерева (кадр) — рамка акцентом. FR-084: база
            // контента (origin) не входит в геометрию node_at — точка
            // передаётся минус origin (инверсия замыкания local).
            cursor_idx = explain_ui::node_at(
                &layout,
                scale,
                body,
                [self.cursor[0] - origin[0], self.cursor[1] - origin[1]],
            );
            // FR-060: items кита → инстансы stage (screen→world, радиус и
            // размер ÷ zoom — конвенция прежних ручных квадов; порядок
            // items = draw-порядок — сохранён дословно)
            paint_items_to_stage(
                d.take_items(),
                camera,
                viewport,
                zoom,
                &mut quads,
                &mut texts,
            );
        }
        if let Some(s) = self.explain.as_mut() {
            s.cursor = cursor_idx;
        }
        (quads, texts)
    }

    /// Ревизия владельца 2026-10-02 (дефект «поле подмены пересекается с
    /// нижележащим текстом»): inline-поле подмены (X3/AC-4.1) — ВТОРОЙ
    /// модальный подпроход кадра. Прежде поле рисовалось в общем проходе
    /// окна: квад поля — среди квадов, тексты — в общей текст-группе ПОСЛЕ
    /// всех квадов, поэтому текст нижележащих строк ложился ПОВЕРХ заливки
    /// поля (меню-тон α 0.97 сквозь него просвечивал). Рендерер выводит
    /// этот подпроход после текстов окна (квады поля → каретка → текст),
    /// поле всегда поверх контента дерева.
    ///
    /// Геометрия — та же чистая функция `field_rect` у якоря строки
    /// (edit_anchor_rect + explain_view — единый вид с рендером и хитом);
    /// клик мимо/Enter — коммит, Esc — отмена (ввод — в input.rs).
    pub(super) fn explain_edit_overlay(
        &self,
        viewport: [f32; 2],
    ) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut quads: Vec<CardInstance> = Vec::new();
        let mut texts: Vec<OwnedScreenText> = Vec::new();
        let Some(state) = self.explain.as_ref() else {
            return (quads, texts);
        };
        let Some(edit) = state.edit.as_ref() else {
            return (quads, texts);
        };
        let palette = ThemeColors::from_theme(self.settings.theme);
        let win = explain_ui::window_rect(viewport);
        let body = explain_ui::body_rect(win);
        let (_, layout, scale, origin) = self.explain_view(state, body);
        let anchor = explain_ui::edit_anchor_rect(&layout, edit.idx, scale, body, origin)
            .unwrap_or([body[0], body[1], 0.0, 0.0]);
        let field = explain_ui::field_rect(body, anchor);
        quads.push(screen_rect_quad(
            &self.camera,
            viewport,
            field,
            palette.menu_fill,
            palette.accent,
            6.0,
        ));
        // Каретка (мигающая полоса) — ширина текста поля ИЗМЕРЕНА реальным
        // шейпингом (CR-015: прежняя эвристика `chars × 12 × 0.62` давала
        // caret не по глифам; замер — тем же кеглем, каким рисуется текст
        // поля ниже, 12.0); квад каретки — до текста (под ним, перекрытие
        // с последним глифом — 1.5 px, не мешает чтению).
        let mut m = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        let text_w = m.width_of(&mut fs, &edit.text, canvas_render::text::SANS_FAMILY, 12.0);
        drop(fs);
        let caret_x = field[0] + 8.0 + text_w;
        if (state.opened_at.elapsed().as_millis() / 530) % 2 == 0 {
            let caret = [
                caret_x.min(field[0] + field[2] - 8.0),
                field[1] + 5.0,
                1.5,
                16.0,
            ];
            quads.push(screen_rect_quad(
                &self.camera,
                viewport,
                caret,
                palette.accent,
                [0.0; 4],
                0.0,
            ));
        }
        let empty = edit.text.is_empty();
        texts.push(OwnedScreenText {
            text: if empty {
                self.tr(keys::EXPLAIN_EDIT_HINT).to_owned()
            } else {
                edit.text.clone()
            },
            origin: [field[0] + 8.0, field[1] + 5.5],
            width: field[2] - 16.0,
            font_size: 12.0,
            color: if empty { palette.quote } else { palette.body },
            align: TextAlign::Left,
        });
        (quads, texts)
    }

    /// Тултип «Проверка цепочки расчёта» у кнопки в полосе результата
    /// (§6.4 Closed → Hover): якорь — правый конец полосы (кнопка-иконка
    /// FR-088, рисуется рендером), клик по полосе — триггер (AC-1.1,
    /// единственный путь на таче). Раньше — безликий «?» у курсора: место
    /// триггера было неочевидно (ревизия владельца FR-088).
    pub(super) fn explain_hover_pill(
        &mut self,
        viewport: [f32; 2],
        quads: &mut Vec<CardInstance>,
        texts: &mut Vec<OwnedScreenText>,
    ) {
        // Тултип — только когда окно/stage/редактор не перехватывают курсор
        if self.explain.is_some()
            || self.main_stage.is_some()
            || self.scheme_gallery.open
            || self.onboarding.is_some()
            || self.editing.is_some()
        {
            return;
        }
        let world = self.cursor_world();
        let Some(index) = self.hovered else { return };
        let Some(node) = self.scene.canvas.nodes.get(index) else {
            return;
        };
        let Some(ExprOutcome::Ok(_)) = self.scene.expr_results.get(&node.id) else {
            return;
        };
        // CR-020 (Q2 «лупа на строку»): мини-лупа строки-результата —
        // приоритет над полосой ИТОГ; тултип анкерён к иконке строки.
        let btn = if let Some((_, rect)) = self.row_explain_hit_at(world) {
            rect
        } else {
            // CR-020 (Q1): полоса «ИТОГ» — тултип только при видимом футере
            // (у констант-присваиваний футер подавлен — зоны нет)
            if !self.scene.node_shows_result_footer(index) {
                return;
            }
            let band = [
                node.x,
                node.y + node.height - BODY_PADDING - RESULT_LINE_HEIGHT,
                node.width,
                RESULT_LINE_HEIGHT,
            ];
            if !point_in_rect(band, world) {
                return;
            }
            canvas_render::cards::explain_button_rect(node)
        };
        let palette = ThemeColors::from_theme(self.settings.theme);
        // Якорь — правый край кнопки/лупы (world → screen): тултип над
        // полосой/строкой, выровнен по правому краю; клампы к вьюпорту.
        let btn_top_right = self
            .camera
            .world_to_screen([btn[0] + btn[2], btn[1]], viewport);
        let label = self.tr(keys::EXPLAIN_TITLE).to_owned();
        let mut m = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        let text_w = m.width_of(&mut fs, &label, canvas_render::text::SANS_FAMILY, 12.0);
        drop(fs);
        let w = (text_w + 16.0).max(40.0);
        let h = 22.0;
        // FR-070 (агент N): kit::tooltip_rect_anchored — natural выше-левее
        // правого-верхнего угла якоря (`tooltip.right = anchor.right`,
        // `tooltip.bottom = anchor.top - 6`), flip ниже при нехватке места
        // сверху, clamp X/Y к вьюпорту. Якорь — rect кнопки в screen-коорд
        // (top-right = btn_top_right, w/h = btn.w/h, инвариант трансляции).
        // hovered_ms = u64::MAX (always show, без delay — тултип рисуется
        // сразу при наведении на полосу `band`, delay-state не хранится).
        let anchor_rect = UiRect::new(btn_top_right[0] - btn[2], btn_top_right[1], btn[2], btn[3]);
        let viewport_rect = UiRect::new(0.0, 0.0, viewport[0].max(0.0), viewport[1].max(0.0));
        let Some(tl) =
            canvas_ui::kit::tooltip_rect_anchored(anchor_rect, (w, h), viewport_rect, u64::MAX, 0)
        else {
            return;
        };
        let x = tl.rect.x;
        let y = tl.rect.y;
        quads.push(CardInstance {
            pos: [x, y],
            size: [tl.rect.w, tl.rect.h],
            fill: palette.menu_fill,
            border: palette.palette_border,
            params: [6.0, 0.0, 0.0, 1.0],
            corners: [0.0; 4],
        });
        texts.push(OwnedScreenText {
            text: label,
            origin: [x, y + (tl.rect.h - 12.0 * 1.3) / 2.0],
            width: tl.rect.w,
            font_size: 12.0,
            color: palette.title,
            align: TextAlign::Center,
        });
    }
}

/// FR-060 (W-c): кружок draw-прохода (ветки-безье) — Painter-rect:
/// квадрат d×d с радиусом d/2 — та же геометрия, что `screen_dot`
/// (конвертация stage делит размер и радиус на зум так же). Точечного
/// примитива у [`Painter`] нет ([`PaintItem`] — Rect/Text/Icon).
fn dot_rect(center: [f32; 2], diameter: f32) -> (UiRect, f32) {
    let r = diameter / 2.0;
    (
        UiRect::new(center[0] - r, center[1] - r, diameter, diameter),
        r,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Кружок через Painter-rect — геометрия `screen_dot` (квадрат d×d
    /// вокруг центра, радиус d/2 → окружность на рендере).
    #[test]
    fn dot_rect_matches_screen_dot_geometry() {
        let (rect, radius) = dot_rect([100.0, 50.0], 6.0);
        assert_eq!(
            rect,
            UiRect::new(97.0, 47.0, 6.0, 6.0),
            "квадрат d×d вокруг центра"
        );
        assert_eq!(radius, 3.0, "радиус = d/2 → окружность");
        let (rect, radius) = dot_rect([0.0, 0.0], 4.0);
        assert_eq!(rect, UiRect::new(-2.0, -2.0, 4.0, 4.0));
        assert_eq!(radius, 2.0);
    }

    /// FR-060: состояние кнопки футера защиты — матрица кита: Disabled
    /// глушит hover; активная кнопка ловит Hovered (стиль прохода:
    /// акцент при Hovered, рамка/текст — по Disabled/Hovered/Normal).
    #[test]
    fn defense_button_widget_state_matrix() {
        let state_of = |hovered: bool, enabled: bool| {
            let mut s = WidgetState::default();
            s.set_pointer(hovered, false);
            s.set_disabled(!enabled);
            s.kit_state()
        };
        assert_eq!(state_of(true, true), kit::KitState::Hovered);
        assert_eq!(state_of(false, true), kit::KitState::Normal);
        assert_eq!(state_of(true, false), kit::KitState::Disabled);
        assert_eq!(state_of(false, false), kit::KitState::Disabled);
    }

    /// FR-060: карточка узла — Hovered/Selected → «emphasized» (рамка
    /// акцентом); Normal — рамка панелей. Hover сильнее Selected —
    /// результат тот же (рамка акцента в обоих состояниях).
    #[test]
    fn node_card_widget_state_matrix() {
        let emphasized = |hovered: bool, picked: bool| {
            let mut s = WidgetState::default();
            s.set_pointer(hovered, false);
            s.set_selected(picked);
            s.kit_state() != kit::KitState::Normal
        };
        assert!(emphasized(false, true), "picked → Selected");
        assert!(emphasized(true, false), "hover → Hovered");
        assert!(emphasized(true, true), "hover + picked — рамка та же");
        assert!(
            !emphasized(false, false),
            "обычная карточка — без рамки акцента"
        );
    }
}
