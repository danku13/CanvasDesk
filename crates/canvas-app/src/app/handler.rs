//! ApplicationHandler — интеграция с event loop winit (T6):
//! window_event, user_event, about_to_wait, resumed.
//!
//! Выделен из `app.rs` сессией рефакторинга 2026-09-24 (этап 4) без
//! изменения поведения. Паттерн дочернего модуля — как `ui_registry`
//! (FR-052): `use super::*` даёт доступ к приватным полям `App`.

use super::*;

impl ApplicationHandler<AppEvent> for App {
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                // Штатный выход (T17): форс-сейв (SPEC §9 — не ждать
                // debounce) + восстановление иконок (R5) — единая точка
                // с пунктом меню «Выход»
                self.shutdown(event_loop);
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
                self.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.set_scale_factor(scale_factor);
                }
                // Миникарта (T13): буфер растеризован в физических px —
                // пересоберётся на ближайшем кадре (размер в сигнатуре)
                self.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. } => self.on_key(&event),
            // Фикс ввода кириллицы 2026-09-25 (wasm-аудит): Chromium шлёт
            // кириллицу из keyboard.type()/IME через insertText → winit
            // отдаёт Ime::Commit, которое раньше ТЕРЯЛОСЬ молча — на web
            // кириллица не вводилась ни в поиск, ни в редактор (латиница
            // шла Key::Character и работала). Маршрут — в активный
            // текстовый приёмник (см. on_ime).
            WindowEvent::Ime(ime) => self.on_ime(ime),
            WindowEvent::MouseInput { state, button, .. } => match button {
                MouseButton::Middle => {
                    self.middle_pressed = state == ElementState::Pressed;
                    self.sync_cursor_icon();
                }
                MouseButton::Left => self.on_left_button(state),
                MouseButton::Right => self.on_right_button(state, event_loop),
                _ => {}
            },
            WindowEvent::CursorMoved { position, .. } => self.on_cursor_moved(position),
            WindowEvent::CursorLeft { .. } => {
                // Курсор ушёл из окна: hover-порты гаснут (иначе подсветка
                // «залипает» до следующего входа)
                if self.hovered.take().is_some() {
                    self.request_redraw();
                }
            }
            WindowEvent::Focused(false) => {
                // Потеря фокуса окна — сброс залипших жестов (alt-tab во
                // время drag: Released придёт другому окну — нода/пан
                // оставались «прилипшими»)
                self.cancel_pointer_transients();
                self.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => self.on_mouse_wheel(delta),
            WindowEvent::PinchGesture { delta, .. } => self.on_pinch(delta),
            WindowEvent::RedrawRequested => {
                // FR-049 (US-5): ?template=<id> — применить на первом кадре
                // (вьюпорт известен — zoom-to-fit корректен); неизвестный
                // id — мягкий отказ (тост), канвас остаётся как есть
                if let Some(id) = self.pending_scheme.take() {
                    match canvas_core::schemes::SchemeRegistry::embedded().get(&id) {
                        Some(manifest) => {
                            let manifest = manifest.clone();
                            self.apply_scheme(&manifest);
                        }
                        None => {
                            tracing::warn!(scheme = %id, "?template: схема не найдена");
                            self.show_toast(i18n::trf(
                                self.settings.language,
                                keys::GALLERY_UNKNOWN,
                                &[("id", id.as_str())],
                            ));
                        }
                    }
                }
                // Замер интервала между кадрами для HUD (T5)
                let now = Instant::now();
                if let Some(prev) = self.last_frame {
                    self.frame_meter.push(now - prev);
                }
                self.last_frame = Some(now);
                // M8/W4 (wasm-port §3.4): async-инициализация (web) — футура
                // кладёт Renderer в слот и будит цикл request_redraw'ом;
                // забираем здесь, до отрисовки. Натив: слот пуст всегда —
                // ветка мертва, накладных расходов нет.
                let delivered = self.renderer_slot.as_ref().and_then(RendererSlot::take);
                match delivered {
                    Some(Ok(renderer)) => self.install_renderer(renderer),
                    Some(Err(err)) => {
                        // Полная цепочка anyhow в сообщение: на web консоль —
                        // единственный канал диагностики (поля %err
                        // компактный ConsoleLayer суффиксует, но цепочка
                        // источников видна только в alternate-формате)
                        tracing::error!("не удалось инициализировать рендер (async): {err:#}");
                        self.renderer_slot = None;
                    }
                    None => {}
                }
                // Полёт камеры к результату поиска (T14): семпл ease-out —
                // пока полёт активен, about_to_wait держит кадры идущими
                if let Some((flight, start)) = self.flight.take() {
                    let elapsed = start.elapsed().as_millis() as u32;
                    let (center, zoom) = flight.sample(elapsed);
                    self.camera.set_center(center);
                    self.camera.set_zoom(zoom);
                    if !flight.is_finished(elapsed) {
                        self.flight = Some((flight, start));
                    }
                }
                // FR-012: settle-анимация вставки в группу — группа и
                // раздвинутые соседи едут к целевым позициям ease_out_cubic
                if let Some(anim) = &self.settle_anim {
                    let t = (anim.start.elapsed().as_millis() as f32 / SETTLE_ANIM_MS).min(1.0);
                    let k = ease_out_cubic(t);
                    for (index, from, to) in &anim.moves {
                        if let Some(node) = self.scene.canvas.nodes.get_mut(*index) {
                            node.x = from[0] + (to[0] - from[0]) * k;
                            node.y = from[1] + (to[1] - from[1]) * k;
                        }
                        if let Some(node) = self.scene.canvas.nodes.get(*index) {
                            self.scene.spatial.update(*index, node);
                        }
                    }
                    if t >= 1.0 {
                        self.settle_anim = None;
                    }
                    self.request_redraw();
                }
                // Миникарта (T13): пересборка по dirty-условиям ДО отрисовки
                // (текстура должна быть готова к проходу кадра)
                self.update_minimap();
                let hud = self.hud_text();
                // World-оверлеи: Т9 призраки дропа добавляются в конец — mutable
                let mut overlay_instances: Vec<CardInstance> = Vec::new();
                // FR-022: donut-сектора wheel-меню (screen-space, мирится
                // рендерером в world)
                let mut overlay_sectors: Vec<SectorInstance> = Vec::new();
                let mut overlay_labels: Vec<String> = Vec::new();
                let mut overlay_label_pos: Vec<Vec2> = Vec::new();
                // Ширины подписей оверлея: призраки дропа — по ширине
                // карточки-призрака (Т9)
                let mut overlay_widths: Vec<f32> = Vec::new();
                // FR-052 (U2 PRD-0009): экран собирается в ПОЛОСЫ слоёв
                // (ui_registry::ScreenBands): порядок полос выводится из
                // реестра поверхностей (UiLayer по возрастанию — контракт
                // UiFrame::draw_bands), порядок внутри полосы сохранён
                // дословно — визуальный порядок канонических состояний
                // не меняется.
                let mut screen_bands = ui_registry::ScreenBands::default();
                {
                    let (settings_instances, settings_texts) = self.settings_overlay();
                    screen_bands.push(UiLayer::Panels, settings_instances, settings_texts);
                }
                // FR-055 (этап U4): витрина кита — модаль поверх всего
                // (Modals/Block: pick через реестр, backdrop закрывает);
                // взаимоисключима с галереей схем/empty-state (прежняя
                // цепочка if/else сохранена — 0 дельт канонических состояний)
                // FR-070: админпанель — модаль поверх всего (Modals/Block);
                // взаимоисключима с витриной кита/галереей схем
                if self.admin_open {
                    let (admin_instances, admin_texts) = self.admin_panel_overlay();
                    screen_bands.push(UiLayer::Modals, admin_instances, admin_texts);
                } else if self.kit_gallery_open {
                    let (kit_instances, kit_texts) = self.kit_gallery_overlay();
                    screen_bands.push(UiLayer::Modals, kit_instances, kit_texts);
                } else if self.scheme_gallery.open {
                    let (gal_instances, gal_texts) = self.scheme_gallery_overlay();
                    screen_bands.push(UiLayer::Modals, gal_instances, gal_texts);
                } else if self.empty_state_visible() {
                    let (es_instances, es_texts) = self.empty_state_overlay();
                    screen_bands.push(UiLayer::Panels, es_instances, es_texts);
                }
                // Меню пустого канваса (T7): screen-space, константный размер
                {
                    let (menu_instances, menu_texts) = self.canvas_menu_overlay();
                    screen_bands.push(UiLayer::Popups, menu_instances, menu_texts);
                }
                // FR-050 Н2 (этап C): меню выбора (параметр приёмника /
                // строка-источник) — полоса Popups поверх меню канваса
                // (клики/Escape — через реестр поверхностей FR-052 U2)
                {
                    let (choice_instances, choice_texts) = self.choice_menu_overlay();
                    screen_bands.push(UiLayer::Popups, choice_instances, choice_texts);
                }
                // FR-027: меню помощи «?» и просмотрщик документации —
                // поверх канваса (просмотрщик выше меню: открытие закрывает
                // меню, но порядок безопасен в любом состоянии)
                {
                    let (help_instances, help_texts) = self.help_menu_overlay();
                    screen_bands.push(UiLayer::Popups, help_instances, help_texts);
                    let (docs_instances, docs_texts) = self.docs_overlay();
                    screen_bands.push(UiLayer::Popups, docs_instances, docs_texts);
                }
                // FR-028: онбординг-карусель — модальный оверлей первого запуска
                {
                    let (onb_instances, onb_texts) = self.onboarding_overlay();
                    screen_bands.push(UiLayer::Modals, onb_instances, onb_texts);
                }
                // Палитра выделения (FR-009/FR-010): тулбар под выделением;
                // rect'ы запоминаются для airspace виджетов
                let palette_view = self.palette_view();
                if let Some((lay, groups, open)) = &palette_view {
                    let (pal_instances, pal_texts) = self.palette_overlay(lay, groups, *open);
                    screen_bands.push(UiLayer::Widgets, pal_instances, pal_texts);
                }
                // Панель поиска (T14): квады/тексты поверх всего канваса
                {
                    let (search_instances, search_texts) = self.search_overlay();
                    screen_bands.push(UiLayer::Panels, search_instances, search_texts);
                }
                // FR-050 Н9-4 (этап E): панель «Карта проливаний» — справа
                // сверху (Block: клик мимо — закрыть; строки — переход)
                {
                    let (map_instances, map_texts) = self.flow_map_overlay();
                    screen_bands.push(UiLayer::Panels, map_instances, map_texts);
                }
                // PRD-0007 (X4, AC-5.5): бейдж предложений автосвязи —
                // верх по центру, ненавязчивый (та же видимость, что у hit-rect)
                if self.autolink_badge_visible() {
                    let palette = self.effective_palette();
                    let badge_viewport = self.viewport_logical();
                    let badge = crate::autolink_ui::badge_rect(badge_viewport);
                    let mut badge_quads = Vec::with_capacity(2);
                    // Правка дрейфа 2026-09-25: квад полосы — сырые screen-px (единственный
                    // screen→world делает рендер; бейдж не едет с камерой)
                    badge_quads.push(crate::app::band_rect_quad_pub(
                        badge,
                        palette.palette_chip_fill,
                        palette.accent,
                        16.0,
                    ));
                    let count = self.autolink_proposals.len().to_string();
                    let badge_texts = vec![OwnedScreenText {
                        text: self.trf(keys::AUTOLINK_BADGE, &[("{n}", count.as_str())]),
                        origin: [badge[0], badge[1] + 6.0],
                        width: badge[2],
                        font_size: 12.0,
                        color: palette.body,
                        align: TextAlign::Center,
                    }];
                    screen_bands.push(UiLayer::Panels, badge_quads, badge_texts);
                }
                // PRD-0007 (X6, F-12): индикатор покрытия цепочками —
                // левый нижний угол канваса, opt-in (настройка FR-039);
                // пересчёт — по ревизии (кэш), скрывается при цифрах нет
                if self.coverage_indicator_visible() {
                    if let Some(percent) = self.coverage_percent() {
                        let palette = self.effective_palette();
                        let cov_viewport = self.viewport_logical();
                        let chip = [16.0, cov_viewport[1] - 44.0, 132.0, 26.0];
                        // Правка дрейфа 2026-09-25: квад полосы — сырые screen-px (как у бейджа)
                        let cov_quads = vec![crate::app::band_rect_quad_pub(
                            chip,
                            palette.palette_chip_fill,
                            palette.palette_border,
                            8.0,
                        )];
                        let percent_text = percent.to_string();
                        let cov_texts = vec![OwnedScreenText {
                            text: self
                                .trf(keys::EXPLAIN_COVERAGE, &[("{n}", percent_text.as_str())]),
                            origin: [chip[0], chip[1] + 6.0],
                            width: chip[2],
                            font_size: 12.0,
                            color: palette.body,
                            align: TextAlign::Center,
                        }];
                        screen_bands.push(UiLayer::Panels, cov_quads, cov_texts);
                    }
                }
                // FR-018: палитра шаблонов (Ctrl+P) и wheel-меню
                // (Shift+клик) — поверх канваса; иконки/хаб wheel — полоса
                // WorldOverlay (над секторами, под панелями)
                {
                    let (tpl_instances, tpl_texts) = self.template_panel_overlay();
                    screen_bands.push(UiLayer::Panels, tpl_instances, tpl_texts);
                    let (wheel_sectors, wheel_instances, wheel_texts) = self.wheel_overlay();
                    overlay_sectors.extend(wheel_sectors);
                    screen_bands.push(UiLayer::WorldOverlay, wheel_instances, wheel_texts);
                }
                // FR-021: popup подсказок Numi-ввода — поверх редактора
                {
                    let (hint_instances, hint_texts) = self.hints_overlay();
                    screen_bands.push(UiLayer::Popups, hint_instances, hint_texts);
                }
                // FR-017 (CP6): what-if нижний бар (пилюля/полоса/список/
                // таблица сравнения) — поверх канваса
                {
                    let (whatif_instances, whatif_texts) = self.whatif_overlay();
                    screen_bands.push(UiLayer::Panels, whatif_instances, whatif_texts);
                }
                // Тултип битой ссылки (T10, SPEC §7.5): у курсора — старый путь
                // файла; screen-space, константный размер при любом зуме.
                // Main stage — модален: тултипы живого канваса глушатся
                // (иначе тултип «просвечивает» поверх затемнения, дефект
                // скриншота — stage должен быть единственным источником
                // контента поверх затемнения)
                if self.main_stage.is_none() {
                    let mut tooltip_texts: Vec<OwnedScreenText> = Vec::new();
                    if let Some(file) = self.hovered.and_then(|index| {
                        self.scene.canvas.nodes.get(index).and_then(|node| {
                            (node.broken_link == Some(true))
                                .then(|| node.file.clone())
                                .flatten()
                        })
                    }) {
                        // Ограничиваем правым краём окна, чтобы длинный путь
                        // не вылез за экран (width — только клип-бounds)
                        let viewport = self.viewport_logical();
                        let origin_x = (self.cursor[0] + 14.0)
                            .min(viewport[0].max(0.0) - TOOLTIP_WIDTH.max(0.0));
                        tooltip_texts.push(OwnedScreenText {
                            text: self.trf(keys::TOAST_FILE_UNAVAILABLE, &[("{file}", &file)]),
                            origin: [origin_x.max(0.0), self.cursor[1] + 18.0],
                            width: TOOLTIP_WIDTH,
                            font_size: 13.0,
                            color: Color::rgb(0xd4, 0xd4, 0xd4),
                            align: TextAlign::Left,
                        });
                    }
                    // Тултип ошибки формульной строки (FR-013, правка 4): курсор
                    // над бейджем «!» (зоны — с прошлого кадра) — сообщение об
                    // ошибке у курсора; так видно, ЧТО именно не так в расчёте
                    if let Some(hit) = self.expr_error_hit_at(self.cursor) {
                        let viewport = self.viewport_logical();
                        let origin_x = (self.cursor[0] + 14.0)
                            .min(viewport[0].max(0.0) - TOOLTIP_WIDTH.max(0.0));
                        tooltip_texts.push(OwnedScreenText {
                            text: hit.message.clone(),
                            origin: [origin_x.max(0.0), self.cursor[1] + 18.0],
                            width: TOOLTIP_WIDTH,
                            font_size: 13.0,
                            color: Color::rgb(0xe5, 0x5c, 0x5c),
                            align: TextAlign::Left,
                        });
                    }
                    // FR-045 F-5 v1/v2: лейблы порта канваса (qualified-адрес
                    // R-5) — точечная цель, приоритет над линейными тултипами:
                    // unmapped/проливание ниже гасятся, пока активен лейбл
                    // порта (оба рисуются у курсора — двойной нечитаем).
                    // v2: входные слоты — список истоков (до 3 строк + «+N
                    // ещё»), строки стеком с шагом 16 px (кегль 13)
                    let port_label = if self.edge_drag.is_none()
                        && self.choice_menu.is_none()
                        && self.expr_error_hit_at(self.cursor).is_none()
                        && self.formula_ellipsis_hit_at(self.cursor).is_none()
                    {
                        self.port_tooltip_at(self.cursor_world())
                    } else {
                        None
                    };
                    // FR-061 коммит 3: тултип усечённой формулы (лестница
                    // §3.4, Q8) — курсор над усечённой строкой узкой ноды:
                    // полная формула у курсора (нейтральный тон — не ошибка)
                    if let Some(hit) = self.formula_ellipsis_hit_at(self.cursor) {
                        let viewport = self.viewport_logical();
                        let origin_x = (self.cursor[0] + 14.0)
                            .min(viewport[0].max(0.0) - TOOLTIP_WIDTH.max(0.0));
                        tooltip_texts.push(OwnedScreenText {
                            text: hit.message.clone(),
                            origin: [origin_x.max(0.0), self.cursor[1] + 18.0],
                            width: TOOLTIP_WIDTH,
                            font_size: 13.0,
                            color: Color::rgb(0xd4, 0xd4, 0xd4),
                            align: TextAlign::Left,
                        });
                    }
                    if let Some(lines) = port_label.clone() {
                        let viewport = self.viewport_logical();
                        let origin_x = (self.cursor[0] + 14.0)
                            .min(viewport[0].max(0.0) - TOOLTIP_WIDTH.max(0.0));
                        for (row, line) in lines.iter().enumerate() {
                            tooltip_texts.push(OwnedScreenText {
                                text: line.text.clone(),
                                origin: [
                                    origin_x.max(0.0),
                                    self.cursor[1] + 18.0 + (row as f32) * 16.0,
                                ],
                                width: TOOLTIP_WIDTH,
                                font_size: 13.0,
                                // Тон строки (v2): unmapped-исток — янтарный
                                // акцент анализа (тот же, что у тултипа
                                // unmapped-ребра); значения — спокойный
                                // сине-серый акцент потока значений (v1)
                                color: if line.unmapped {
                                    Color::rgb(0xf5, 0xa6, 0x23)
                                } else {
                                    Color::rgb(0x9c, 0xc3, 0xe6)
                                },
                                align: TextAlign::Left,
                            });
                        }
                    }
                    // FR-050 Р-3 (этап C): тултип unmapped-ребра «проблема +
                    // решение» (контракт Р-3 — ровно два пункта): курсор над
                    // пунктирной янтарной связью «не подставлено». Параметр с
                    // fromOutput — точный диагноз (какой выход у какой ноды);
                    // прочие (позиционный слот / параметр без адреса выхода) —
                    // общий шаблон. Янтарный тон — тот же, что у ребра.
                    if port_label.is_none()
                        && self.edge_drag.is_none()
                        && self.choice_menu.is_none()
                    {
                        let world = self.cursor_world();
                        if let Some(index) =
                            edge_at(&self.scene.canvas, world, self.settings.edges_avoid_nodes)
                        {
                            let edge = &self.scene.canvas.edges[index];
                            if self.scene.unmapped_edges.iter().any(|id| id == &edge.id) {
                                let text = if let (Some(param), Some(output)) =
                                    (&edge.to_param, &edge.from_output)
                                {
                                    let node_label = self
                                        .scene
                                        .canvas
                                        .node(&edge.from_node)
                                        .map(node_display_label)
                                        .unwrap_or_else(|| edge.from_node.clone());
                                    self.trf(
                                        keys::TOOLTIP_UNMAPPED_PARAM,
                                        &[
                                            ("{param}", param),
                                            ("{output}", output),
                                            ("{node}", &node_label),
                                        ],
                                    )
                                } else {
                                    self.tr(keys::TOOLTIP_UNMAPPED_SLOT).to_owned()
                                };
                                let viewport = self.viewport_logical();
                                let origin_x = (self.cursor[0] + 14.0)
                                    .min(viewport[0].max(0.0) - TOOLTIP_WIDTH.max(0.0));
                                tooltip_texts.push(OwnedScreenText {
                                    text,
                                    origin: [origin_x.max(0.0), self.cursor[1] + 18.0],
                                    width: TOOLTIP_WIDTH,
                                    font_size: 13.0,
                                    // Янтарный акцент анализа (severity warning)
                                    color: Color::rgb(0xf5, 0xa6, 0x23),
                                    align: TextAlign::Left,
                                });
                            }
                        }
                    }
                    // FR-050 Н9-2 (этап D): тултип источника пролитой строки —
                    // курсор над наклонной строкой (параметр с toParam /
                    // авто-строка приёмника): «пролито: Трафик.peak_rps =
                    // 1389 rps (локально было: 500 rps)» (Н7: полный путь —
                    // здесь, на теле — короткая форма). Не спорит с тултипом
                    // ошибки (бейдж «!» приоритетнее) и глушится при drag;
                    // F-5: лейбл порта приоритетнее (точечная цель).
                    if port_label.is_none()
                        && self.edge_drag.is_none()
                        && self.choice_menu.is_none()
                        && self.expr_error_hit_at(self.cursor).is_none()
                    {
                        if let Some(hit) = self.spill_hit_at(self.cursor) {
                            let text = match &hit.kind {
                                SpillHitKind::Param {
                                    path, value, local, ..
                                } => match (value, local) {
                                    (Some(value), Some(local)) => self.trf(
                                        keys::TOOLTIP_SPILL_PARAM,
                                        &[("{path}", path), ("{value}", value), ("{local}", local)],
                                    ),
                                    (Some(value), None) => self.trf(
                                        keys::TOOLTIP_SPILL_PARAM_NO_LOCAL,
                                        &[("{path}", path), ("{value}", value)],
                                    ),
                                    (None, _) => self.trf(
                                        keys::TOOLTIP_SPILL_PARAM_NOVALUE,
                                        &[("{path}", path)],
                                    ),
                                },
                                SpillHitKind::AutoRow {
                                    path,
                                    slot,
                                    value,
                                    template,
                                    edge_id: _,
                                } => {
                                    let slot_no = (slot + 1).to_string();
                                    match (value, template) {
                                        (Some(value), true) => self.trf(
                                            keys::TOOLTIP_SPILL_AUTOROW_TPL,
                                            &[
                                                ("{path}", path),
                                                ("{value}", value),
                                                ("{slot}", &slot_no),
                                            ],
                                        ),
                                        (Some(value), false) => self.trf(
                                            keys::TOOLTIP_SPILL_AUTOROW_TEXT,
                                            &[
                                                ("{path}", path),
                                                ("{value}", value),
                                                ("{slot}", &slot_no),
                                            ],
                                        ),
                                        (None, _) => self.trf(
                                            keys::TOOLTIP_SPILL_AUTOROW_NOVALUE,
                                            &[("{path}", path), ("{slot}", &slot_no)],
                                        ),
                                    }
                                }
                            };
                            let viewport = self.viewport_logical();
                            let origin_x = (self.cursor[0] + 14.0)
                                .min(viewport[0].max(0.0) - TOOLTIP_WIDTH.max(0.0));
                            tooltip_texts.push(OwnedScreenText {
                                text,
                                origin: [origin_x.max(0.0), self.cursor[1] + 18.0],
                                width: TOOLTIP_WIDTH,
                                font_size: 13.0,
                                // Спокойный сине-серый акцент потока значений
                                // (тултип источника, не диагностика)
                                color: Color::rgb(0x9c, 0xc3, 0xe6),
                                align: TextAlign::Left,
                            });
                        }
                    }
                    screen_bands.push(UiLayer::Popups, Vec::new(), tooltip_texts);
                }
                // T21: модальный диалог (screen-space): панель + тексты +
                // кнопки; рендер после битой ссылки — поверх всего канваса.
                // FR-060: геометрия — измеренная (kit::modal + button_size),
                // длинные тексты — ellipsis (деградация видима, G5/G8)
                if let Some(dialog) = &self.dialog {
                    let (d_rect, buttons, shown) = self.dialog_measured();
                    let [dx, dy, dw, dh] = d_rect;
                    let mut dialog_instances: Vec<CardInstance> = Vec::new();
                    let mut dialog_texts: Vec<OwnedScreenText> = Vec::new();
                    dialog_instances.push(CardInstance {
                        pos: [dx, dy],
                        size: [dw, dh],
                        fill: canvas_core::tokens::DIALOG_FILL,
                        border: canvas_core::tokens::DIALOG_BORDER,
                        params: [10.0, 0.0, 0.0, 1.0],
                    });
                    for (i, (label, _)) in dialog.buttons(self.settings.language).iter().enumerate()
                    {
                        let [bx, by, bw, bh] = buttons[i];
                        dialog_instances.push(CardInstance {
                            pos: [bx, by],
                            size: [bw, bh],
                            fill: if i == 0 {
                                canvas_core::tokens::DIALOG_BUTTON_PRIMARY
                            } else {
                                canvas_core::tokens::DIALOG_BUTTON_SECONDARY
                            },
                            border: canvas_core::tokens::DIALOG_BUTTON_BORDER,
                            params: [6.0, 0.0, 0.0, 1.0],
                        });
                        let (btn_box, btn_width) = centered_box(buttons[i], 4.0);
                        dialog_texts.push(OwnedScreenText {
                            text: (*label).to_owned(),
                            origin: [btn_box[0], buttons[i][1] + 7.0],
                            width: btn_width,
                            font_size: DIALOG_BTN_FS,
                            color: token_color(canvas_core::tokens::DIALOG_TEXT),
                            align: TextAlign::Center,
                        });
                    }
                    dialog_texts.push(OwnedScreenText {
                        text: shown[0].clone(),
                        origin: [dx + DIALOG_PAD_X, dy + DIALOG_TITLE_Y],
                        width: dw - DIALOG_PAD_X * 2.0,
                        font_size: DIALOG_TITLE_FS,
                        color: token_color(canvas_core::tokens::DIALOG_TEXT),
                        align: TextAlign::Left,
                    });
                    dialog_texts.push(OwnedScreenText {
                        text: shown[1].clone(),
                        origin: [dx + DIALOG_PAD_X, dy + DIALOG_BODY_Y],
                        width: dw - DIALOG_PAD_X * 2.0,
                        font_size: DIALOG_BODY_FS,
                        color: token_color(canvas_core::tokens::DIALOG_TEXT_MUTED),
                        align: TextAlign::Left,
                    });
                    screen_bands.push(UiLayer::Modals, dialog_instances, dialog_texts);
                }
                // T21: toast — строка внизу центра, живёт 3 с (T21-A).
                // Истечение проверяем ДО рендера (без borrow-конфликта)
                let toast_alive = self
                    .toast
                    .as_ref()
                    .is_some_and(|(_, at)| at.elapsed().as_secs_f32() < 3.0);
                if !toast_alive {
                    self.toast = None;
                } else if let Some((text, _)) = &self.toast {
                    let viewport = self.viewport_logical();
                    // CR-016: при активном what-if бар занимает низ окна
                    // [viewport−BAR_MARGIN−BAR_HEIGHT, viewport−BAR_MARGIN] —
                    // toast поднимаем над ним, чтобы не перекрывать чипы.
                    let ty = if self.scene.whatif_active {
                        viewport[1] - whatif_ui::BAR_MARGIN - whatif_ui::BAR_HEIGHT - 26.0
                    } else {
                        viewport[1] - 44.0
                    };
                    // CR-015: origin — левый край области (контракт ScreenText):
                    // область [40, viewport−40] по центру окна, текст в её центре.
                    screen_bands.push(
                        UiLayer::Toasts,
                        Vec::new(),
                        vec![OwnedScreenText {
                            text: text.clone(),
                            origin: [40.0, ty],
                            width: viewport[0] - 80.0,
                            font_size: 14.0,
                            color: token_color(canvas_core::tokens::TOAST_TEXT),
                            align: TextAlign::Center,
                        }],
                    );
                }
                // FR-042 (E3) + FR-044: main stage — МОДАЛЬНЫЙ проход кадра.
                // Валидация среза (инвариант 9: фоновые мутации MCP/undo
                // закрывают), затем сборка квадов/текстов stage в ОТДЕЛЬНЫЕ
                // списки: рендерер выводит их после всех z-сегментов, текст-
                // групп и миникарты — ни живой текст канваса (тела нод,
                // подписи связей, бейджи анализа), ни тултипы не «просвечивают»
                // сквозь затемнение (раньше stage шёл в мир-хвост ПОД текстом
                // финального сегмента — регрессия «каши», дефект скриншота).
                // PRD-0007 (X2): опрос фоновой сборки, чип устаревания и
                // реакция на ошибку — ДО заимствований рендера (тост мутирует
                // App, паттерн CP5)
                if let Some(state) = self.explain.as_mut() {
                    // AC-3.3/F-5: чип при любом изменении модели (канвас,
                    // MCP, файл, подмена листа, Apply — все идут через
                    // recompute_flow → ревизия)
                    state.stale = state.revision != self.scene.revision;
                    // Loading → Ready: затемнение и подсветка появляются
                    // атомарно с деревом (У5) — фокус-набор соберёт
                    // update_focus_state на этом же кадре
                    state.poll();
                }
                if self.explain.as_ref().is_some_and(|s| s.is_failed()) {
                    // Корень пропал между кликом и сборкой (AC-3.3):
                    // закрыть с сообщением
                    self.close_explain();
                    self.show_toast(self.tr(keys::EXPLAIN_GONE).to_owned());
                }
                if let Some(stage) = self.main_stage.as_mut() {
                    if !stage.valid(&self.scene.canvas) {
                        self.main_stage = None;
                    }
                }
                // Раскладка среза — единственный mut-заём stage (кадр);
                // далее только чтение — совместимо с методами self.tr/….
                let stage_viewport = self.viewport_logical();
                let mut stage_instances: Vec<CardInstance> = Vec::new();
                let mut stage_owned_texts: Vec<OwnedScreenText> = Vec::new();
                let relaid = self
                    .main_stage
                    .as_mut()
                    .map(|stage| stage.relayout(stage_viewport));
                if let (Some(stage), Some(layout)) = (self.main_stage.as_ref(), relaid) {
                    let (insts, texts) = self.stage_frame(stage_viewport, stage, layout);
                    stage_instances = insts;
                    stage_owned_texts = texts;
                }
                // PRD-0007 (X2): окно проверки — модальный проход кадра
                // (взаимоисключительно с stage, F-10); при закрытом окне —
                // hover-«?» у цифры результата (Closed → Hover).
                // X4 (§6.5): панель прячется на время диалога ревью автосвязи
                if self.explain.is_some() && self.autolink_review.is_none() {
                    let (insts, texts) = self.explain_frame(stage_viewport);
                    stage_instances = insts;
                    stage_owned_texts = texts;
                } else if self.main_stage.is_none() && self.autolink_review.is_none() {
                    self.explain_hover_pill(
                        stage_viewport,
                        &mut stage_instances,
                        &mut stage_owned_texts,
                    );
                }
                // PRD-0007 (X4): диалог ревью автосвязи — верхний модальный
                // проход кадра (§6.5 — поверх канваса; панель объяснения
                // спрятана условием выше, stage закрыт при открытии ревью —
                // поэтому списки stage пусты и порядок квадов/текстов корректен)
                if self.autolink_review.is_some() {
                    let (insts, texts) = self.autolink_frame(stage_viewport);
                    stage_instances.extend(insts);
                    stage_owned_texts.extend(texts);
                }
                // FR-055 (этап U4, F-10): DebugOverlay — ПОСЛЕДНЯЯ полоса
                // кадра (UiLayer::Debug, L8): рамки hit-rect'ов по слоям,
                // имя под курсором, подсветка пересечений (G6). Модель
                // чистая — по кадру реестра (тот же build_frame, что у ввода)
                if self.debug_overlay {
                    let ui_frame = ui_registry::build_frame(self);
                    let (dbg_instances, dbg_texts) = crate::debug_overlay::build(
                        self.viewport_logical(),
                        self.cursor,
                        &ui_frame,
                    );
                    screen_bands.push(UiLayer::Debug, dbg_instances, dbg_texts);
                }
                // FR-052 (U2): полосы в порядке отрисовки (слои по возрастанию)
                // + Owned-тексты → заимствованные ScreenText (заём живёт до
                // конца кадра, конфликтов с &mut self нет)
                let bands = screen_bands.finish();
                let band_screen_texts: Vec<Vec<ScreenText>> = bands
                    .iter()
                    .map(|(_, _, texts)| {
                        texts
                            .iter()
                            .map(|t| ScreenText {
                                text: &t.text,
                                origin: t.origin,
                                width: t.width,
                                font_size: t.font_size,
                                color: t.color,
                                align: t.align,
                            })
                            .collect()
                    })
                    .collect();
                let band_viewport = self.viewport_logical();
                let screen_band_refs: Vec<canvas_render::ScreenBand> = bands
                    .iter()
                    .zip(&band_screen_texts)
                    .map(|((layer, instances, _), texts)| canvas_render::ScreenBand {
                        layer: *layer,
                        // FR-056 (F-5 PRD-0009): клип полосы = SurfaceFrame.clip
                        // поверхности в кадре реестра (UiFrame::from_registry —
                        // сегодня вьюпорт; сужение клипов per-surface — волны
                        // миграции FR-059/060, аудит G5). Рендер конвертирует
                        // в физические px и исполняет scissor-бакетом.
                        clip: canvas_ui::UiRect::new(0.0, 0.0, band_viewport[0], band_viewport[1]),
                        instances,
                        texts,
                    })
                    .collect();
                // FR-042/FR-044: тексты main stage — отдельный список (не
                // screen_texts панелей): рисуются группой ПОСЛЕ модального
                // прохода квадов stage
                let stage_screen_texts: Vec<ScreenText> = stage_owned_texts
                    .iter()
                    .map(|t| ScreenText {
                        text: &t.text,
                        origin: t.origin,
                        width: t.width,
                        font_size: t.font_size,
                        color: t.color,
                        align: t.align,
                    })
                    .collect();
                // Призраки зоны дропа (T9): рамка bbox сетки + квады-призраки.
                // Кладём В КОНЕЦ оверлея: порядок инстансов = порядок рисования,
                // depth-теста нет — призраки поверх всего
                if let Some(preview) = &self.drop_preview {
                    let positions = crate::ui::drop_grid(preview.origin, preview.plan.len());
                    if let Some(frame) = canvas_render::cards::drop_zone_frame(
                        &positions,
                        [crate::ui::DROP_CARD_W, crate::ui::DROP_CARD_H],
                        crate::ui::DROP_GRID_GAP,
                    ) {
                        overlay_instances.push(frame);
                    }
                    overlay_instances.extend(canvas_render::cards::drop_ghosts(
                        &positions,
                        [crate::ui::DROP_CARD_W, crate::ui::DROP_CARD_H],
                        crate::ui::DROP_PREVIEW_MAX,
                    ));
                    // Подписи призраков (Т9): во время перетаскивания имена
                    // файлов/первая строка заметки видны до самого дропа —
                    // раньше призраки были пустыми рамками
                    let pad = crate::ui::DROP_GHOST_LABEL_PAD;
                    for (ins, pos) in preview.plan.iter().zip(&positions) {
                        overlay_labels.push(crate::ui::drop_ghost_label(&ins.kind));
                        overlay_label_pos.push([pos[0] + pad, pos[1] + 6.0]);
                        overlay_widths.push(crate::ui::DROP_CARD_W - pad * 2.0);
                    }
                }
                let overlay_texts: Vec<OverlayText> = overlay_labels
                    .iter()
                    .zip(&overlay_label_pos)
                    .zip(&overlay_widths)
                    .map(|((label, pos), width)| OverlayText {
                        text: label,
                        origin: *pos,
                        width: *width,
                    })
                    .collect();
                // Пульс подсветки ноды-результата (T14): world-квад с рамкой,
                // затухающей по pulse_alpha; за вырожденный — сброс (рамка
                // оверлейная — border.a, заливка прозрачна после фикса
                // cards.wgsl)
                if let Some((node, start)) = self.pulse {
                    let alpha = pulse_alpha(start.elapsed().as_millis() as u32);
                    if alpha <= 0.0 {
                        self.pulse = None;
                    } else if let Some(target) = self.scene.canvas.nodes.get(node) {
                        let grow = (1.0 - alpha) * 8.0;
                        overlay_instances.push(CardInstance {
                            pos: [target.x - grow, target.y - grow],
                            size: [target.width + grow * 2.0, target.height + grow * 2.0],
                            fill: [0.0; 4],
                            border: [
                                canvas_core::tokens::PULSE_RESULT[0],
                                canvas_core::tokens::PULSE_RESULT[1],
                                canvas_core::tokens::PULSE_RESULT[2],
                                alpha,
                            ],
                            params: [6.0, 0.0, 0.0, 1.0],
                        });
                    }
                }
                // Рамка выделения (CR-001): полупрозрачный world-квад с
                // акцентной рамкой (стиль зоны дропа T9), без тени
                if let Some((start, current, _)) = self.select_rect {
                    let rect = rubber_band_rect(start, current);
                    overlay_instances.push(CardInstance {
                        pos: [rect[0], rect[1]],
                        size: [rect[2], rect[3]],
                        fill: crate::ui::SELECT_RECT_FILL,
                        border: crate::ui::SELECT_RECT_BORDER,
                        params: [4.0, 0.0, 0.0, 1.0],
                    });
                }
                // FR-012: подсветка зоны втягивания — группа под drag-нодой
                if let Some(gi) = self.group_drop_target {
                    if let Some(group) = self.scene.canvas.nodes.get(gi) {
                        overlay_instances.push(CardInstance {
                            pos: [group.x - 4.0, group.y - 4.0],
                            size: [group.width + 8.0, group.height + 8.0],
                            fill: [
                                canvas_core::tokens::ACCENT[0],
                                canvas_core::tokens::ACCENT[1],
                                canvas_core::tokens::ACCENT[2],
                                canvas_core::tokens::ALPHA_10,
                            ],
                            border: [
                                canvas_core::tokens::ACCENT[0],
                                canvas_core::tokens::ACCENT[1],
                                canvas_core::tokens::ACCENT[2],
                                canvas_core::tokens::ALPHA_90,
                            ],
                            params: [8.0, 0.0, 0.0, 1.0],
                        });
                    }
                }
                // M5 (T20-F): airspace-прямоугольники оверлеев (план П7) —
                // до LOD-кадра виджетов; большие панели (поиск/настройки)
                // упрощённо гасят все live (транзиентно), точные rect'ы —
                // меню/подменю/палитра/хоткеи/миникарта
                let mut widget_airspace: Vec<[f32; 4]> = Vec::new();
                if let Some(rect) = self.menu_open_rect() {
                    widget_airspace.push(rect);
                    if let Some(menu) = self.menu.as_ref() {
                        if let Some(submenu) = &menu.submenu {
                            widget_airspace.push(submenu_rect(submenu));
                        }
                    }
                }
                // Палитра выделения: бар + открытая колонка (логические px)
                if let Some((lay, _, open)) = &palette_view {
                    widget_airspace.push(lay.bar);
                    if let Some(open) = open {
                        widget_airspace.push(lay.groups[*open].dropdown);
                    }
                }
                if self.hotkeys_open {
                    let vp = self.viewport_logical();
                    widget_airspace.push(hotkeys_panel_rect_at(vp, self.hotkeys_left_offset(vp)));
                }
                if let Some(renderer) = self.renderer.as_ref() {
                    if let Some(rect) = renderer.minimap_rect_logical() {
                        widget_airspace.push(rect);
                    }
                }
                let widget_overlay_active = self.select_rect.is_some()
                    || self.edge_drag.is_some()
                    || self.drop_preview.is_some()
                    || self.settings_open
                    || self.dialog.is_some()
                    || self.search.is_open();
                // T21: модальный диалог — airspace-зона (П7): живые виджеты
                // под ним гасятся в снапшоты, пока диалог открыт
                if self.dialog.is_some() {
                    widget_airspace.push(self.dialog_rect());
                }
                let widget_frame = self.widgets.update_frame(
                    &self.scene.canvas,
                    &self.camera,
                    self.viewport_logical(),
                    self.scale_factor(),
                    &widget_airspace,
                    widget_overlay_active,
                );
                // Owned-квады → ссылки для FrameOverlay (локально: заём
                // живёт до конца кадра, конфликтов с &mut self нет)
                let widget_quad_refs: Vec<canvas_render::WidgetQuad> = widget_frame
                    .quads
                    .iter()
                    .map(|q| canvas_render::WidgetQuad {
                        node_id: q.node_id.as_str(),
                        pos: q.pos,
                        size: q.size,
                    })
                    .collect();
                // CR-004: прозрачные виджет-ноды — контент реально виден
                // (live-HWND или снапшот-текстура); placeholder битых
                // пакетов (broken) остаётся серой карточкой. Виджет без
                // снапшота и вне live (транзиент первого кадра) — тоже
                // непрозрачен: иначе нода исчезала бы целиком.
                let live_ids: std::collections::HashSet<&str> =
                    widget_frame.live.iter().map(|s| s.as_str()).collect();
                let broken_ids: std::collections::HashSet<&str> =
                    widget_frame.broken.iter().map(|s| s.as_str()).collect();
                let widget_transparent: Vec<usize> = self
                    .scene
                    .canvas
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, node)| node.kind() == NodeKind::Widget)
                    .filter(|(_, node)| {
                        let id = node.id.as_str();
                        !broken_ids.contains(id)
                            && (live_ids.contains(id)
                                || self
                                    .renderer
                                    .as_ref()
                                    .is_some_and(|r| r.has_widget_snapshot(id)))
                    })
                    .map(|(i, _)| i)
                    .collect();
                // CR-004 v1: заголовок виджет-ноды виден при hover/выделении
                let widget_title_reveal: Vec<usize> = self
                    .scene
                    .canvas
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(i, node)| {
                        node.kind() == NodeKind::Widget
                            && (self.hovered == Some(*i)
                                || self.selected == Some(Selection::Node(*i))
                                || self.selected_nodes.contains(i))
                    })
                    .map(|(i, _)| i)
                    .collect();
                // FR-011: скрытые ноды (свернутые поддеревья) + бейджи «+N»
                let hidden_nodes = self.hidden_subtree_nodes();
                let collapsed_counts: Vec<(usize, usize)> = self
                    .scene
                    .canvas
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, node)| node.collapsed == Some(true))
                    .map(|(i, _)| (i, canvas_core::subtree_ids(&self.scene.canvas, i).len()))
                    .filter(|(_, count)| *count > 0)
                    .collect();
                let overlay = FrameOverlay {
                    instances: &overlay_instances,
                    texts: &overlay_texts,
                    screen_bands: &screen_band_refs,
                    stage_instances: &stage_instances,
                    stage_texts: &stage_screen_texts,
                    screen_sectors: &overlay_sectors,
                    widget_quads: &widget_quad_refs,
                };
                // Резиновая линия (T8/CR-002): от порта/неподвижного конца к
                // курсору; исходная линия перепривязываемой связи скрыта
                let edge_draft = self.edge_drag.as_ref().and_then(|drag| {
                    let (port, side) = drag.draft_origin(&self.scene.canvas)?;
                    Some((port, side, self.cursor_world()))
                });
                let hidden_edge = match self.edge_drag.as_ref() {
                    Some(EdgeDrag::Rebind { edge_index, .. }) => Some(*edge_index),
                    _ => None,
                };
                // FR-016 (CP5): авто-включение оверлея при первом появлении
                // риска (Warn и выше) — один раз за запуск, с тостом;
                // ручной тогл (Ctrl+B/меню/панель) отключает авто до
                // перезапуска (решение «Открытые вопросы» FR-016). До
                // заимствований рендера и FocusView — тост мутирует App.
                if !self.settings.bottleneck_overlay
                    && !self.bottleneck_auto_enabled
                    && analyze::has_risk(&self.scene.analysis)
                {
                    self.settings.bottleneck_overlay = true;
                    self.bottleneck_auto_enabled = true;
                    self.show_toast(self.tr(keys::TOAST_ANALYSIS_ON));
                    // авто-включение не персистим: конфиг не трогаем
                }
                // T23 (brainstorm-focus): пересчёт анимации и окрестности
                // семени ДО сборки сцены — FocusView заимствует поля App
                self.update_focus_state();
                // FR-050 Н9-1 (этап E): волна каскада — перестройка на новой
                // ревизии потока (изменённые ноды → рёбра вниз с порядком)
                // и тик времени; ДО сборки сцены — SpillWaveView
                // заимствует поля App
                self.update_spill_wave();
                // FR-044 Q3: тик анимации подсветки stage (до stage_frame —
                // рендер читает stage_calc_render)
                self.tick_stage_calc_fade();
                let spill_wave = self
                    .spill_wave
                    .as_ref()
                    .map(|(edges, start)| SpillWaveView {
                        edges,
                        elapsed_ms: start.elapsed().as_millis() as u32,
                        step_ms: SPILL_WAVE_STEP_MS,
                    })
                    .unwrap_or(SpillWaveView::EMPTY);
                let focus = FocusView {
                    nodes: &self.focus_nodes,
                    edges: &self.focus_edges,
                    dim: self.focus_dim,
                    pulse: self
                        .focus_pulse
                        .as_ref()
                        .map(|(_, start)| focus_pulse(start.elapsed().as_millis() as u32))
                        .unwrap_or(0.0),
                };
                let analysis_view: Option<&AnalysisState> = if self.settings.bottleneck_overlay {
                    Some(&self.scene.analysis)
                } else {
                    None
                };
                if let Some(renderer) = self.renderer.as_mut() {
                    // FR-042 (E2): контекст агрегации кадра — индекс сцены +
                    // hover пучка; None при выключенной агрегации (F-13)
                    let bundle_ctx = self.settings.edge_aggregation.then_some(BundleContext {
                        index: &self.scene.bundles,
                        hover: self.bundle_hover,
                    });
                    let bundle_ctx = bundle_ctx.as_ref();
                    // FR-013 (правка 4): живые построчные результаты (Numi —
                    // результаты по ходу набора): считаем из текста СЕССИИ
                    // на каждый кадр (дёшево: парсинг только формульных
                    // строк; fit_note_size уже вызывает eval_lines покадрово)
                    let editing_line_results: Option<Vec<Option<ExprOutcome>>> =
                        self.editing.as_ref().and_then(|session| {
                            let index = session.node_index()?;
                            let node = self.scene.canvas.nodes.get(index)?;
                            node.kind()
                                .eq(&NodeKind::Text)
                                .then(|| expr::eval_lines(&session.text()))
                        });
                    let scene = SceneView {
                        canvas: &self.scene.canvas,
                        spatial: &self.scene.spatial,
                        selected: self.selected,
                        selected_nodes: &self.selected_nodes,
                        hovered: self.hovered,
                        edge_draft,
                        hidden_edge,
                        edges_avoid: self.settings.edges_avoid_nodes,
                        port_zone_px: self.settings.port_zone_px,
                        line_ports: self.settings.line_ports,
                        // FR-050 Н2 (этап C): подсветка якорей параметров
                        // во время value-drag (совместимость Н5)
                        param_drop: self.param_drop.as_ref().map(|(node_index, params)| {
                            ParamDropView {
                                node_index: *node_index,
                                params,
                            }
                        }),
                        // FR-050 Р-3 (этап C): unmapped-рёбра — пунктир
                        // янтарным акцентом анализа
                        unmapped_edges: &self.scene.unmapped_edges,
                        focus,
                        widget_transparent: &widget_transparent,
                        widget_title_reveal: &widget_title_reveal,
                        hidden_nodes: &hidden_nodes,
                        collapsed_counts: &collapsed_counts,
                        expr_results: &self.scene.expr_results,
                        expr_line_results: &self.scene.expr_line_results,
                        expr_editing_results: editing_line_results.as_deref(),
                        param_spills: &self.scene.param_spills,
                        // FR-050 Н9-1 (этап E): волна каскада (вспышка
                        // потока значений по рёбрам downstream)
                        spill_wave,
                        // FR-050 Р-4 (этап D): авто-строки приёмников —
                        // префикс тела (наклонное начертание Р-2)
                        auto_rows: &self.scene.auto_rows,
                        whatif_nodes: &self.scene.whatif_nodes,
                        // FR-061 хвосты (D-7/D-8 runtime v1): состояние
                        // тогглов тела — свёрнутые блоки, раскрытые описания
                        block_collapsed: &self.scene.block_collapsed,
                        desc_expanded: &self.scene.desc_expanded,
                        analysis: analysis_view,
                        analysis_overlay: self.settings.bottleneck_overlay,
                        // FR-042 (E2): контекст агрегации пучков кадра
                        // (F-13: выкл — None, поведение байт-в-байт прежнее)
                        bundles: bundle_ctx,
                    };
                    // FR-061 этап D (D-14): язык таблицы тела (блок-заголовок
                    // Н-2) — глобальная настройка, применяется покадрово
                    // (идемпотентно; кэш точечно устаревает через results_key).
                    renderer.set_table_language(self.settings.language);
                    // FR-061 этап D (D-14/Q9): направляющие таблицы — только
                    // в DebugOverlay (F9 / ?ui=debug), в проде невидимы.
                    renderer.set_table_guides_visible(self.debug_overlay);
                    match renderer.render(
                        &self.camera,
                        &scene,
                        hud.as_deref(),
                        self.editing.as_mut(),
                        &overlay,
                    ) {
                        Ok(stats) => self.last_stats = stats,
                        Err(err) => {
                            tracing::error!(%err, "ошибка рендера, завершение");
                            event_loop.exit();
                        }
                    }
                    // FR-013 (правка 4): зоны ошибок кадра — для тултипа в
                    // оверлее следующего кадра (отставание в кадр незаметно)
                    self.expr_error_hits = renderer.line_error_hits().to_vec();
                    // FR-050 Н9-2 (этап D): зоны пролитых строк кадра —
                    // тултип источника («пролито: …») в оверлее следующего
                    // кадра (паттерн expr_error_hits)
                    self.spill_hits = renderer.spill_hits().to_vec();
                    // FR-061 коммит 3: зоны усечённых формул кадра — тултип
                    // полной формулы в оверлее следующего кадра
                    self.ellipsis_hits = renderer.formula_ellipsis_hits().to_vec();
                    // FR-061 хвосты (D-7/D-8): кликабельные зоны тела кадра —
                    // тогглы свёрнутости блока/раскрытости описания
                    self.body_hits = renderer.body_hits().to_vec();
                }
                // Тамбнейлы видимых нод (T6): заказ после кадра, когда камера
                // уже установилась; ответы придут через AppEvent::ThumbsReady
                self.order_thumbnails();
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::ThumbsReady => {
                // Забрать готовые тамбнейлы из канала и загрузить в атлас;
                // ошибки — в негативный кэш (не перезаказывать каждый кадр)
                let mut arrived = 0usize;
                for (node, result) in self.thumbs.drain() {
                    match result {
                        Some(thumb) => {
                            if let Some(renderer) = self.renderer.as_mut() {
                                renderer.set_thumbnail(node, &thumb);
                                arrived += 1;
                            }
                        }
                        None => {
                            self.thumbs_failed.insert(node);
                        }
                    }
                }
                if arrived > 0 {
                    self.request_redraw();
                }
            }
            AppEvent::Drag(event) => self.on_drag_event(event),
            AppEvent::FileEvents(events) => self.on_file_events(events),
            AppEvent::Search(event) => self.on_search_event(event),
            // Web-мост ввода кириллицы/IME (canvas-web beforeinput): тот же
            // маршрут приёмника, что у Ime::Commit (wasm-аудит 2026-09-25)
            AppEvent::ImeCommit(text) => self.insert_committed_text(&text),
            AppEvent::OpenScene {
                path,
                json,
                storage,
            } => self.on_open_scene(path, json, storage),
            #[cfg(windows)]
            AppEvent::Desktop(event) => self.on_desktop_event(event),
            #[cfg(windows)]
            AppEvent::Shell(event) => self.on_shell_event(event),
            #[cfg(windows)]
            AppEvent::McpWake => self.on_mcp_wake(),
            AppEvent::Widget(event) => self.on_widget_event(event),
            // FR-064 P1: воркер отдал снимки — завершить пересчёт
            // (публикация double buffer + выводка O(N) на UI-треде);
            // кадр нужен, если состояние сцены обновилось.
            #[cfg(not(target_arch = "wasm32"))]
            AppEvent::FlowReady { .. } => {
                if self.scene.complete_flow_recompute() {
                    self.request_redraw();
                }
            }
            // T15-relaunch: exit-сигнал от нового запуска (single-instance
            // handoff) — штатное завершение: форс-сейв сцены, восстановление
            // иконок, exit. Мьютекс освободится смертью процесса, новый
            // инстанс продолжит старт (актуально для перезапуска на --desktop).
            AppEvent::InstanceExit => self.shutdown(event_loop),
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.scene.autosave_if_due();
        // FR-064 P1: тик воркера потока — таймаут зависшего запроса →
        // sync-фолбэк + warn; попутный дренаж готовых снимков (если
        // wake-событие потерялось). Дешёвая проверка (Instant-сравнение).
        #[cfg(not(target_arch = "wasm32"))]
        if self.scene.flow_worker_tick() {
            self.request_redraw();
        }
        // M8/W6 (wasm-port §4.2): пока сцена грязная, цикл не засыпает —
        // запланированный кадр держит rAF-цепочку web-цикла живой, иначе
        // about_to_wait не вызывается после последнего события ввода и
        // debounce автосейва (2 с) никогда не срабатывает (правка → коммит
        // → тишина → сохранения нет). На нативе цена — пара лишних кадров
        // в течение 2 с после правки; поведение автосейва идентичное.
        if self.scene.dirty_since.is_some() {
            self.request_redraw();
        }
        // Debounce запроса поиска (T14): 200 мс покоя после правки — отправка.
        // Панель/анимации держат цикл красным через request_redraw ниже,
        // иначе ControlFlow::Wait уснул бы до следующего события
        if let Some((query, edited_at)) = self.search_pending.take() {
            if edited_at.elapsed() < SEARCH_DEBOUNCE {
                self.search_pending = Some((query, edited_at));
            } else {
                self.search_service.command(SearchCommand::Query {
                    query,
                    limit: SEARCH_RESULTS_LIMIT,
                });
            }
        }
        // PRD-0007 (FR-048 X4, AC-5.5): фоновый скан автосвязи с дебаунсом
        // после правок модели (переименование строки перепроверяется — П8).
        // При открытом диалоге ревью скан не перезапускается (решения
        // сеанса важнее свежести — перепроверка после закрытия, AC-5.2).
        if self.settings.autolink_enabled
            && self.autolink_review.is_none()
            && self.scene.revision != self.autolink_scanned_rev
        {
            match self.autolink_scan_due {
                None => self.autolink_scan_due = Some(Instant::now()),
                Some(edited_at) if edited_at.elapsed().as_millis() >= AUTOLINK_DEBOUNCE_MS => {
                    let fresh = canvas_core::find_proposals(&self.scene.canvas);
                    let changed = fresh != self.autolink_proposals;
                    self.autolink_proposals = fresh;
                    self.autolink_scanned_rev = self.scene.revision;
                    self.autolink_scan_due = None;
                    if changed {
                        self.request_redraw();
                    }
                }
                Some(_) => {}
            }
        }
        // Полёт камеры и пульс (T14) + фокус (T23): непрерывные кадры
        // до завершения анимаций; hover-ожидание палитры (FR-009):
        // hover-intent открытие / отсрочка закрытия при неподвижном курсоре;
        // ревизия FR-025: то же для flyout свёрнутой полосы шаблонов
        // (hover-intent 150 мс / grace 300 мс при неподвижном курсоре)
        if !self.template_panel.open
            && self.template_hover.is_some()
            && self.update_template_hover()
        {
            self.request_redraw();
        }
        if self.search_pending.is_some()
            || self.flight.is_some()
            || self.pulse.is_some()
            || self.spill_wave_animating()
            || self.focus_animating()
            // FR-044 Q3: переход подсветки stage — кадры до завершения
            || self.stage_calc_fade_animating()
            || self.palette_hover.pending()
            || self
                .template_hover
                .as_ref()
                .is_some_and(|hover| hover.pending())
        {
            self.request_redraw();
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("CanvasDesk");
        // Режим десктопа (T15): borderless-окно на весь виртуальный экран
        // без активации при создании (WS_EX_NOACTIVATE до первого клика —
        // TASKS T15; winit with_active(false)). Это же окно — фолбэк-режим,
        // если встройка не удастся (R14: не пересоздаём после winit-инициализации).
        // После attach winit-API окна НЕ трогаем — стили перезапишет
        // библиотека (R3-урок tao/Seelen); размеры — только SetWindowPos.
        let attrs = if self.desktop_mode {
            attrs
                .with_decorations(false)
                .with_resizable(false)
                .with_active(false)
        } else {
            attrs
        };
        // winit сам ставит свой IDropTarget (RegisterDragDrop с assert S_OK) —
        // отключаем и ставим свой в canvas-shell (план T9 §3)
        #[cfg(windows)]
        let attrs = attrs.with_drag_and_drop(false);
        // Точная геометрия десктоп-окна (физ. px) — только на Windows:
        // виртуальный экран из EnumDisplayMonitors; до attach — стартовый
        // размер по экрану (потом attach растянет SetWindowPos'ом).
        #[cfg(windows)]
        let attrs = if self.desktop_mode {
            let screen = canvas_shell::desktop::hierarchy::virtual_screen_rect().unwrap_or(
                canvas_shell::desktop::ScreenRect::from_ltrb(0, 0, 1280, 720),
            );
            attrs
                .with_position(winit::dpi::PhysicalPosition::new(screen.left, screen.top))
                .with_inner_size(winit::dpi::PhysicalSize::new(
                    screen.width().max(1) as u32,
                    screen.height().max(1) as u32,
                ))
        } else {
            attrs
        };
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                tracing::error!(%err, "не удалось создать окно");
                event_loop.exit();
                return;
            }
        };
        self.window = Some(window.clone());
        // Регистрация своего IDropTarget (T9) и встройка в десктоп (T15) — ДО
        // создания GPU-surface: так attach (SetParent/scrub) не конфликтует
        // с живым swapchain. Сама по себе невидимость встроенного окна
        // порядком не лечилась (проверено экспериментом): Vulkan-swapchain
        // не презентует в ребёнка Progman вне зависимости от момента
        // создания surface — лечится выбором DX12 для desktop-режима
        // (Renderer::new, prefer_dx12). HWND достаём через raw-window-handle
        // (winit 0.30 публично Win32-HWND не отдаёт); ошибка drag-drop —
        // warn и живём без него (graceful degradation).
        #[cfg(windows)]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            // HWND через raw-window-handle: winit 0.30 публично
            // Win32-HWND не отдаёт (внутренний windows-sys); окно
            // создано на этом потоке, handle доступен
            match window.window_handle() {
                Ok(handle) => match handle.as_raw() {
                    RawWindowHandle::Win32(win32) => {
                        match canvas_shell::dragdrop::install(
                            win32.hwnd.get(),
                            self.drag_sender.clone(),
                        ) {
                            Ok(watcher) => self.drag_watcher = Some(watcher),
                            Err(err) => {
                                tracing::warn!(%err, "drag-drop недоступен, приложение работает без него")
                            }
                        }
                        // Встройка в десктоп (T15): после всей winit-настройки
                        // окна (R3-урок: сначала окно настраивается библиотекой,
                        // репарентинг — последним, с верификацией стилей в
                        // attach), но ДО создания GPU-surface (см. выше).
                        if self.desktop_mode {
                            self.attach_desktop(win32.hwnd.get());
                        }
                        // M5 (T20-F): WebView2-хост виджетов — ребёнок окна
                        // канваса; события хоста идут через widget_sender
                        // (прокси из main: у ActiveEventLoop нет create_proxy,
                        // winit 0.30). User-data — единый корень приложения
                        // (~/.canvasdesk/webview2)
                        {
                            let sender = self.widget_sender.clone();
                            let user_data = canvas_shell::default_cache_dir()
                                .unwrap_or_default()
                                .join("webview2");
                            // hwnd.get() уже isize (raw-window-handle 0.6):
                            // без каста — иначе clippy needless_cast на Windows
                            self.widgets
                                .attach_host(win32.hwnd.get(), user_data, sender);
                        }
                    }
                    // На Windows бывает только Win32-handle
                    _ => tracing::warn!("неожиданный handle окна — drag-drop выключен"),
                },
                Err(err) => {
                    tracing::warn!(%err, "handle окна недоступен — drag-drop выключен")
                }
            }
        }
        // M8/W4 (wasm-port §3.4): запуск инициализации Renderer через
        // инъектированную стратегию. Натив — pollster::block_on (GPU-иниц
        // блокирующая, один раз при старте, SPEC §6.3: холодный старт < 2 с;
        // prefer_dx12 = desktop-режим — Vulkan не презентует в ребёнка
        // Progman). Web — spawn_local: результат придёт в слот с побудкой
        // кадра (браузерный главный поток блокировать нельзя — план §7).
        match self
            .renderer_launcher
            .launch(window.clone(), self.desktop_mode)
        {
            RendererLaunch::Ready(renderer) => self.install_renderer(*renderer),
            RendererLaunch::Pending(slot) => {
                tracing::info!("GPU-инициализация асинхронная (web): первый кадр по готовности");
                self.renderer_slot = Some(slot);
            }
            RendererLaunch::Failed(err) => {
                tracing::error!(%err, "не удалось инициализировать рендер");
                event_loop.exit();
            }
        }
    }
}
