//! AI-кластер приложения: explain-поверхность (разбор выделения),
//! autolink-ревью (предложенные связки) и их ховер-pill.
//!
//! Выделены из `app.rs` сессией рефакторинга 2026-09-25 (этап 6) без
//! изменения поведения: это методы `App`, работающие с тем же состоянием.
//! Паттерн дочернего модуля — как `ui_registry` (FR-052): `use super::*`
//! даёт доступ к приватным полям `App` и импортам родителя.

use super::*;

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
        if let Some(snap) = self.explain_cache.take() {
            if snap.root == root && snap.revision == revision {
                // Переоткрытие из кэша: Ready сразу, чип — если модель
                // всё-таки изменилась (from_snapshot сравнивает ревизии);
                // дельты what-if восстанавливаются из снапшота (AC-4.2)
                self.explain = Some(ExplainState::from_snapshot(snap, revision));
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
        // + WidgetState (состояния строк/кнопок); конверсия в полосу кадра —
        // paint_items_to_band (те же screen-квады/тексты — 0 визуального
        // скачка: та же последовательность квадов/текстов и те же слоты)
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
        // Баннер отклонённых (У8): виден, пока есть отклонённые
        if rejected > 0 {
            let banner = autolink_ui::banner_rect(win);
            d.rect(
                canvas_ui::geometry::UiRect::new(banner[0], banner[1], banner[2], banner[3]),
                [0.0; 4],
                color_to_rgba(palette.error),
                8.0,
            );
            d.label(
                canvas_ui::geometry::UiRect::new(
                    banner[0] + 10.0,
                    banner[1] + 6.0,
                    (banner[2] - autolink_ui::RESTORE_W - 30.0).max(0.0),
                    16.0,
                ),
                &self.trf(
                    keys::AUTOLINK_BANNER,
                    &[("{n}", rejected.to_string().as_str())],
                ),
                color_to_rgba(palette.error),
                11.5,
                PaintAlign::Left,
            );
            let restore = autolink_ui::restore_rect(banner);
            d.rect(
                canvas_ui::geometry::UiRect::new(restore[0], restore[1], restore[2], restore[3]),
                [0.0; 4],
                palette.palette_border,
                6.0,
            );
            d.label(
                canvas_ui::geometry::UiRect::new(restore[0], restore[1] + 4.0, restore[2], 16.0),
                self.tr(keys::AUTOLINK_RESTORE_ALL),
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
                    Color::rgb(255, 255, 255)
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
                    Color::rgb(255, 255, 255)
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
                Color::rgb(255, 255, 255)
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
        paint_items_to_band(d.take_items(), &mut quads, &mut texts);
        (quads, texts)
    }

    /// Кадр окна проверки (§6.4) — модальный проход кадра (паттерн
    /// stage_frame: квады + screen-тексты, рендерер выводит поверх всего).
    /// Loading: окно + честный лоадер (кольцо + ротация подписей), канвас
    /// НЕ затемняется (У5 — затемнение и подсветка атомарны с деревом).
    /// Ready: дерево (ветки-безье + карточки узлов), чип Stale, футер.
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
            quads.push(screen_rect_quad(
                camera,
                viewport,
                win,
                palette.menu_fill,
                palette.palette_border,
                14.0,
            ));
            // Шапка: заголовок + крошки + ✕ + чип Stale
            texts.push(OwnedScreenText {
                text: self.tr(keys::EXPLAIN_TITLE).to_owned(),
                origin: [win[0] + 16.0, win[1] + 10.0],
                width: (win[2] - 240.0).max(120.0),
                font_size: 15.0,
                color: palette.title,
                align: TextAlign::Left,
            });
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
                let (offset, rects) = explain_ui::crumb_rects(win, state.view_path.len());
                let last = state.view_path.len().saturating_sub(1);
                for (i, rect) in rects.iter().enumerate() {
                    let level = offset + i;
                    let Some(node) = tree.nodes.get(state.view_path[level]) else {
                        continue;
                    };
                    let current = level == last;
                    quads.push(screen_rect_quad(
                        camera,
                        viewport,
                        *rect,
                        if current { palette.accent } else { [0.0; 4] },
                        palette.palette_border,
                        6.0,
                    ));
                    texts.push(OwnedScreenText {
                        text: node.title.clone(),
                        origin: [rect[0] + 6.0, rect[1] + 2.5],
                        width: (rect[2] - 10.0).max(8.0),
                        font_size: 10.5,
                        color: if current {
                            Color::rgb(255, 255, 255)
                        } else {
                            palette.body
                        },
                        align: TextAlign::Left,
                    });
                }
            } else {
                texts.push(OwnedScreenText {
                    text: meta,
                    origin: [win[0] + 16.0, win[1] + 32.0],
                    width: (win[2] - 240.0).max(120.0),
                    font_size: 11.5,
                    color: palette.quote,
                    align: TextAlign::Left,
                });
            }
            let close = explain_ui::close_rect(win);
            quads.push(screen_rect_quad(
                camera,
                viewport,
                close,
                [0.0; 4],
                palette.palette_border,
                7.0,
            ));
            texts.push(OwnedScreenText {
                text: "×".to_owned(),
                origin: [close[0], close[1] + 2.0],
                width: close[2],
                font_size: 14.0,
                color: palette.body,
                align: TextAlign::Center,
            });
            // Чип «Данные изменены» (AC-3.3/F-5): модель изменилась после
            // сборки — канвас и дерево не перерисовываются сами
            if state.is_ready() && state.revision != self.scene.revision {
                let chip = explain_ui::chip_rect(win);
                quads.push(screen_rect_quad(
                    camera,
                    viewport,
                    chip,
                    color_to_rgba(palette.whatif_badge),
                    palette.palette_border,
                    14.0,
                ));
                texts.push(OwnedScreenText {
                    text: self.tr(keys::EXPLAIN_STALE).to_owned(),
                    origin: [chip[0], chip[1] + 5.0],
                    width: chip[2],
                    font_size: 11.5,
                    color: Color::rgb(255, 255, 255),
                    align: TextAlign::Center,
                });
            }
            // --- Loading: честный лоадер (AC-1.2, У5) -------------------
            if state.is_loading() {
                let body = explain_ui::body_rect(win);
                let cx = body[0] + body[2] / 2.0;
                let cy = body[1] + body[3] / 2.0 - 20.0;
                let elapsed = state.opened_at.elapsed().as_millis();
                let spin = (elapsed as f32 / 900.0) * std::f32::consts::TAU;
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
                return (quads, texts);
            };
            let body = explain_ui::body_rect(win);
            // X5: вид/масштаб едины с hit-тестами (explain_view); в защите —
            // defense_reveal + укрупнение ×1.5 (AC-6.2/6.3)
            let (vis, layout, scale) = self.explain_view(state, body);
            let local = |x: f32, y: f32| {
                [
                    body[0] + explain_ui::BODY_PAD + x * scale,
                    body[1] + explain_ui::BODY_PAD + y * scale,
                ]
            };
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
                    quads.push(screen_dot(camera, viewport, sp, 4.0, fill));
                }
            }
            // Разделитель футера + статистика видимого дерева
            let footer_line = [win[0], win[1] + win[3] - explain_ui::FOOTER_H, win[2], 1.0];
            quads.push(screen_rect_quad(
                camera,
                viewport,
                footer_line,
                palette.palette_border,
                [0.0; 4],
                0.0,
            ));
            texts.push(OwnedScreenText {
                text: self.trf(
                    keys::EXPLAIN_STATS,
                    &[
                        ("lv", layout.levels.to_string().as_str()),
                        ("n", layout.nodes.len().to_string().as_str()),
                    ],
                ),
                origin: [win[0] + 16.0, win[1] + win[3] - 27.0],
                width: 280.0,
                font_size: 11.0,
                color: palette.quote,
                align: TextAlign::Left,
            });
            // X5 (AC-6.1): тумблер режима защиты в шапке — одним действием
            {
                let toggle = explain_ui::defense_toggle_rect(win);
                let hovered = point_in_rect(toggle, self.cursor);
                let on = state.is_defense();
                quads.push(screen_rect_quad(
                    camera,
                    viewport,
                    toggle,
                    if on || hovered {
                        palette.accent
                    } else {
                        palette.card_fill
                    },
                    if on { [0.0; 4] } else { palette.palette_border },
                    14.0,
                ));
                texts.push(OwnedScreenText {
                    text: self
                        .tr(if on {
                            keys::EXPLAIN_DEFENSE_EXIT
                        } else {
                            keys::EXPLAIN_DEFENSE
                        })
                        .to_owned(),
                    origin: [toggle[0], toggle[1] + 5.0],
                    width: toggle[2],
                    font_size: 11.5,
                    color: if on || hovered {
                        Color::rgb(255, 255, 255)
                    } else {
                        palette.body
                    },
                    align: TextAlign::Center,
                });
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
                for (rect, key, active) in buttons {
                    let hovered = point_in_rect(rect, self.cursor);
                    quads.push(screen_rect_quad(
                        camera,
                        viewport,
                        rect,
                        if hovered && active {
                            palette.accent
                        } else {
                            palette.card_fill
                        },
                        if active {
                            palette.palette_border
                        } else {
                            [0.0; 4]
                        },
                        6.0,
                    ));
                    texts.push(OwnedScreenText {
                        text: self.tr(key).to_owned(),
                        origin: [rect[0], rect[1] + 5.5],
                        width: rect[2],
                        font_size: 11.0,
                        color: if hovered && active {
                            Color::rgb(255, 255, 255)
                        } else if active {
                            palette.body
                        } else {
                            palette.quote
                        },
                        align: TextAlign::Center,
                    });
                }
                texts.push(OwnedScreenText {
                    text: self.tr(keys::EXPLAIN_DEFENSE_HINT).to_owned(),
                    origin: [win[0] + 320.0, win[1] + win[3] - 27.0],
                    width: (win[2] - 480.0).max(120.0),
                    font_size: 10.5,
                    color: palette.quote,
                    align: TextAlign::Center,
                });
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
                quads.push(screen_rect_quad(
                    camera,
                    viewport,
                    rect,
                    palette.card_fill,
                    if hovered || picked {
                        palette.accent
                    } else {
                        palette.palette_border
                    },
                    8.0,
                ));
                // У2-вспышка: затухающая рамка поверх выделения (700 мс);
                // после затухания остаётся только рамка выделения выше.
                if picked {
                    let flash = state.pick_flash_at(Instant::now());
                    if flash > 0.0 {
                        let mut glow = palette.accent;
                        glow[3] = flash;
                        quads.push(screen_rect_quad(
                            camera, viewport, rect, [0.0; 4], glow, 12.0,
                        ));
                    }
                }
                let strip_rect = [rect[0], rect[1], (4.0 * scale).max(2.0), rect[3]];
                quads.push(screen_rect_quad(
                    camera, viewport, strip_rect, strip, [0.0; 4], 0.0,
                ));
                // Шрифт карточки сжимается fit-масштабом вместе с геометрией
                let font = |px: f32| (px * scale).max(8.0);
                let tx = rect[0] + 10.0;
                let text_w = rect[2] - 16.0;
                // 1) Заголовок ноды-таблицы
                texts.push(OwnedScreenText {
                    text: node.title.clone(),
                    origin: [tx, rect[1] + 7.0],
                    width: text_w,
                    font_size: font(12.0),
                    color: palette.title,
                    align: TextAlign::Left,
                });
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
                texts.push(OwnedScreenText {
                    text: value_str,
                    origin: [tx, rect[1] + 25.0],
                    width: text_w,
                    font_size: font(13.5),
                    color: value_color,
                    align: TextAlign::Left,
                });
                // 3) Формула узла/строки (у листа и терминалов нет)
                if let Some(formula) = &node.formula {
                    texts.push(OwnedScreenText {
                        text: formula.clone(),
                        origin: [tx, rect[1] + 44.0],
                        width: text_w,
                        font_size: font(10.5),
                        color: palette.body,
                        align: TextAlign::Left,
                    });
                } else if node.kind == canvas_core::LineageNodeKind::Leaf {
                    // Лист-константа: пометка «исходное значение» (AC-1.4)
                    texts.push(OwnedScreenText {
                        text: self.tr(keys::EXPLAIN_LEAF_TAG).to_owned(),
                        origin: [tx, rect[1] + 44.0],
                        width: text_w,
                        font_size: font(10.5),
                        color: palette.quote,
                        align: TextAlign::Left,
                    });
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
                        texts.push(OwnedScreenText {
                            text: addr,
                            origin: [tx, rect[1] + 58.0],
                            width: text_w,
                            font_size: font(9.5),
                            color: palette.quote,
                            align: TextAlign::Left,
                        });
                    }
                }
                // Бейдж фронтира «+N глубже» (AC-2.3: ручное разворачивание)
                if vis.frontier[laid.idx] {
                    let bw = 66.0;
                    let bh = 15.0;
                    let badge = [
                        rect[0] + rect[2] - bw - 6.0,
                        rect[1] + rect[3] - bh - 5.0,
                        bw,
                        bh,
                    ];
                    quads.push(screen_rect_quad(
                        camera,
                        viewport,
                        badge,
                        palette.accent,
                        [0.0; 4],
                        7.0,
                    ));
                    texts.push(OwnedScreenText {
                        text: self.trf(
                            keys::EXPLAIN_EXPAND_BADGE,
                            &[("n", vis.hidden_descendants[laid.idx].to_string().as_str())],
                        ),
                        origin: [badge[0], badge[1] + 1.5],
                        width: bw,
                        font_size: 9.5,
                        color: Color::rgb(255, 255, 255),
                        align: TextAlign::Center,
                    });
                }
                // X3 (AC-4.1): кнопка «Изменить» на редактируемом листе —
                // подмена значения через WhatIfOverrides (FR-017); кнопка
                // скрывается, пока открыто inline-поле (одно за раз)
                let editable = node.kind == canvas_core::LineageNodeKind::Leaf
                    && node.line.is_some()
                    && matches!(&node.value, Some(Ok(_)));
                if editable && state.edit.is_none() {
                    let btn = explain_ui::edit_rect(rect, scale);
                    let btn_hovered = point_in_rect(btn, self.cursor);
                    quads.push(screen_rect_quad(
                        camera,
                        viewport,
                        btn,
                        if btn_hovered {
                            palette.accent
                        } else {
                            palette.card_fill
                        },
                        palette.palette_border,
                        5.0,
                    ));
                    texts.push(OwnedScreenText {
                        text: self.tr(keys::EXPLAIN_EDIT).to_owned(),
                        origin: [btn[0], btn[1] + 2.0],
                        width: btn[2],
                        font_size: (10.0 * scale).max(8.0),
                        color: if btn_hovered {
                            Color::rgb(255, 255, 255)
                        } else {
                            palette.body
                        },
                        align: TextAlign::Center,
                    });
                }
            }
            // X3 (AC-4.1): inline-поле подмены — поверх дерева (правый
            // нижний угол тела); клик мимо/Enter — коммит, Esc — отмена
            if let Some(edit) = state.edit.as_ref() {
                let field = explain_ui::field_rect(body);
                quads.push(screen_rect_quad(
                    camera,
                    viewport,
                    field,
                    palette.menu_fill,
                    palette.accent,
                    6.0,
                ));
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
                // Каретка (мигающая полоса) — оценка ширины текста (0.62
                // кегля — синк whatif_ui::text_width)
                let caret_x = field[0] + 8.0 + edit.text.chars().count() as f32 * 12.0 * 0.62;
                if (state.opened_at.elapsed().as_millis() / 530) % 2 == 0 {
                    let caret = [
                        caret_x.min(field[0] + field[2] - 8.0),
                        field[1] + 5.0,
                        1.5,
                        16.0,
                    ];
                    quads.push(screen_rect_quad(
                        camera,
                        viewport,
                        caret,
                        palette.accent,
                        [0.0; 4],
                        0.0,
                    ));
                }
            }
            // Hover узла дерева (кадр) — рамка акцентом
            cursor_idx = explain_ui::node_at(&layout, scale, body, self.cursor);
        }
        if let Some(s) = self.explain.as_mut() {
            s.cursor = cursor_idx;
        }
        (quads, texts)
    }

    /// Hover-«?» у цифры результата (§6.4 Closed → Hover, hover-only —
    /// решение владельца): pill под курсором у полосы D; клик по цифре —
    /// фолбэк-триггер (AC-1.1, единственный путь на таче).
    pub(super) fn explain_hover_pill(
        &mut self,
        viewport: [f32; 2],
        quads: &mut Vec<CardInstance>,
        texts: &mut Vec<OwnedScreenText>,
    ) {
        // Pill — только когда окно/stage/редактор не перехватывают курсор
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
        let band = [
            node.x,
            node.y + node.height - BODY_PADDING - RESULT_LINE_HEIGHT,
            node.width,
            RESULT_LINE_HEIGHT,
        ];
        if !point_in_rect(band, world) {
            return;
        }
        let palette = ThemeColors::from_theme(self.settings.theme);
        let zoom = self.camera.zoom();
        let origin = self
            .camera
            .screen_to_world([self.cursor[0] + 14.0, self.cursor[1] - 30.0], viewport);
        quads.push(CardInstance {
            pos: origin,
            size: [22.0 / zoom, 18.0 / zoom],
            fill: palette.accent,
            border: [0.0; 4],
            params: [5.0 / zoom, 0.0, 0.0, 1.0],
        });
        texts.push(OwnedScreenText {
            text: "?".to_owned(),
            origin: [self.cursor[0] + 14.0, self.cursor[1] - 28.0],
            width: 22.0,
            font_size: 13.0,
            color: Color::rgb(255, 255, 255),
            align: TextAlign::Center,
        });
    }
}
