//! Обработка ввода — клавиатура, мышь, тачпад/пинч, drag-drop,
//! маршрутизация по surfaces реестра (FR-052) и Esc-лестница.
//!
//! Выделены из `app.rs` сессией рефакторинга 2026-09-24 (этап 3) без
//! изменения поведения: это методы `App`, работающие с тем же состоянием.
//! Паттерн дочернего модуля — как `ui_registry` (FR-052): `use super::*`
//! даёт доступ к приватным полям `App` и импортам родителя.

use super::*;

impl App {
    /// События drag-drop (T9): превью зоны на Enter/Over, вставка нод на
    /// Drop. Данные приходят сырыми из shell, план строит crate::ui.
    pub(super) fn on_drag_event(&mut self, drag: canvas_core::dragdrop::DragEvent) {
        use crate::ui::{plan_drop, DropInsertKind};
        match drag {
            canvas_core::dragdrop::DragEvent::Enter { data, client_pt } => {
                let world = self.drag_world_pt(client_pt);
                // T21-B: дроп одиночной папки с widget.json — призрак
                // установки виджета (перехват ДО plan_drop файлов)
                if let Some(src) = crate::ui::dropped_widget_package(&data) {
                    match canvas_widgets::manifest::WidgetManifest::from_dir(&src) {
                        Ok(manifest) => {
                            self.drop_preview = Some(DropPreview {
                                origin: world,
                                plan: vec![crate::ui::DropInsert {
                                    id: "widget-install".to_owned(),
                                    kind: crate::ui::DropInsertKind::InstallWidget(
                                        src,
                                        manifest.name.clone(),
                                    ),
                                    pos: world,
                                }],
                            });
                            self.request_redraw();
                            return;
                        }
                        // Битый манифест: честный призрак-ошибка + toast,
                        // как «файл недоступен» у битых ссылок (SPEC §7.5)
                        Err(e) => {
                            self.show_toast(self.trf(
                                keys::TOAST_WIDGET_NOT_INSTALLED,
                                &[("{err}", &e.to_string())],
                            ));
                            self.drop_preview = None;
                            self.request_redraw();
                            return;
                        }
                    }
                }
                let plan = plan_drop(&self.scene.canvas, &data, world);
                // Пустой план (нет поддерживаемых форматов) — не подсвечиваем
                self.drop_preview = if plan.is_empty() {
                    None
                } else {
                    Some(DropPreview {
                        origin: world,
                        plan,
                    })
                };
            }
            canvas_core::dragdrop::DragEvent::Over { client_pt } => {
                // Сетка призраков следует за курсором, сам план не меняется
                let world = self.drag_world_pt(client_pt);
                if let Some(preview) = self.drop_preview.as_mut() {
                    preview.origin = world;
                }
            }
            canvas_core::dragdrop::DragEvent::Leave => self.drop_preview = None,
            canvas_core::dragdrop::DragEvent::Drop { data, client_pt } => {
                let world = self.drag_world_pt(client_pt);
                // T21-B: дроп виджет-пакета — диалог П10 (Да/Нет), установка
                // и нода только после подтверждения; невалидный манифест —
                // toast (повторно не парсим успех — уже в призраке)
                if let Some(src) = crate::ui::dropped_widget_package(&data) {
                    match canvas_widgets::manifest::WidgetManifest::from_dir(&src) {
                        Ok(manifest) => {
                            let updating = self.widgets.registry.contains(&manifest.id);
                            self.dialog = Some(AppDialog::InstallWidget {
                                src,
                                manifest,
                                pos: world,
                                updating,
                            });
                        }
                        Err(e) => {
                            self.show_toast(self.trf(
                                keys::TOAST_WIDGET_NOT_INSTALLED,
                                &[("{err}", &e.to_string())],
                            ));
                        }
                    }
                    self.drop_preview = None;
                    self.request_redraw();
                    return;
                }
                // План пересчитываем по СВЕЖИМ данным Drop (не из превью,
                // план T9 §5): источник мог обновить содержимое
                let plan = plan_drop(&self.scene.canvas, &data, world);
                if !plan.is_empty() {
                    // FR-006: дроп файлов/заметок — undo-шаг
                    self.push_undo();
                }
                let mut last: Option<usize> = None;
                for ins in plan {
                    let node = match ins.kind {
                        DropInsertKind::File(path) => Node::file(
                            ins.id,
                            path.to_string_lossy().into_owned(),
                            ins.pos[0],
                            ins.pos[1],
                            crate::ui::DROP_CARD_W,
                            crate::ui::DROP_CARD_H,
                        ),
                        DropInsertKind::Note(text) => {
                            Node::text(ins.id, text, ins.pos[0], ins.pos[1])
                        }
                        // Установка виджета перехвачена выше (T21-B: дроп
                        // открывает диалог, не вставляет ноду напрямую) —
                        // сюда попасть не можем; рамка на случай будущих
                        // прямых вставок (MCP widget_add — T22+)
                        DropInsertKind::InstallWidget(_, _) => {
                            tracing::warn!("дроп виджета прошёл мимо диалога — пропущен");
                            continue;
                        }
                    };
                    // Вставка как в create_note_at: модель + spatial index
                    self.scene.canvas.nodes.push(node);
                    let index = self.scene.canvas.nodes.len() - 1;
                    let node_ref = &self.scene.canvas.nodes[index];
                    self.scene.spatial.insert(index, node_ref);
                    last = Some(index);
                }
                if let Some(index) = last {
                    // Выделяем последнюю ноду группы; тамбнейлы закажет
                    // order_thumbnails в ближайшем кадре, автосейв — сам
                    self.selected = Some(Selection::Node(index));
                    self.scene.mark_dirty();
                }
                // Поисковый индекс (T14): сброшенные файлы — сразу в FTS
                let canvas_dir = self.scene.canvas_dir();
                for node in &self.scene.canvas.nodes {
                    let Some(file) = node.file.as_ref() else {
                        continue;
                    };
                    self.search_service.command(SearchCommand::IndexFile {
                        path: resolve_node_path(file, &canvas_dir),
                        display_name: Path::new(file)
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| file.clone()),
                    });
                }
                // Дроп мог добавить файловые ноды в новые директории —
                // синхронизируем вотчер (T10)
                self.sync_watch_dirs();
                self.drop_preview = None;
            }
        }
        self.request_redraw();
    }

    /// FR-070: клик по админпанели — сайдбар/сброс/тема/«✕»; прочий клик
    /// по панели глотается (Block-модаль, паттерн витрины FR-055).
    fn click_admin_panel(&mut self, element: &str) {
        match element {
            "admin-close" => {
                self.admin_open = false;
            }
            "admin-theme" => {
                // Смена темы — сброс live-переопределения (слоты снова от
                // темы; иначе поверх новой темы остались бы старые правки)
                self.toggle_theme();
                self.admin_palette_override = None;
            }
            "admin-reset" => {
                self.admin_palette_override = None;
            }
            other => {
                if let Some(idx) = other
                    .strip_prefix("admin-section-")
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    if let Some(sec) = crate::admin_ui::AdminSection::at(idx) {
                        self.admin_section = sec;
                        // Смена секции — контент с начала
                        self.admin_scroll = canvas_ui::kit::ScrollState::default();
                    }
                } else if let Some(idx) = other
                    .strip_prefix("admin-token-")
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    // FR-070: live-правка слота — следующий кандидат цвета;
                    // применяется ко всей админпанели до «Сброса»
                    let mut pal = self
                        .admin_palette_override
                        .unwrap_or_else(|| self.effective_palette().kit_palette());
                    let current = crate::admin_ui::slot_color(&pal, idx);
                    crate::admin_ui::set_slot_color(
                        &mut pal,
                        idx,
                        crate::admin_ui::cycle_slot(current),
                    );
                    self.admin_palette_override = Some(pal);
                }
            }
        }
        self.request_redraw();
    }

    /// FR-055 U4: клик по витрине — кнопка темы (реальный kit-контрол:
    /// переключение темы — смена слотов палитры) или «✕»/паддинг (глотается).
    pub(super) fn click_kit_gallery(&mut self, element: &str) {
        match element {
            "kit-gallery-theme" => {
                // Паттерн кнопки темы угловых кнопок (toggle_theme)
                self.toggle_theme();
            }
            "kit-gallery-close" => {
                self.kit_gallery_open = false;
            }
            _ => {}
        }
        self.request_redraw();
    }

    /// FR-052 (U2): Esc-диспетчер реестра — тела прежней лестницы
    /// 8143–8232 дословно, порядок задаёт `SurfaceRegistry::esc_stack`.
    /// `true` — поверхность поглотила Esc (обход стека прекращается).
    pub(super) fn dispatch_esc(&mut self, surface: &str) -> bool {
        match surface {
            // FR-042 (E3, инвариант 8): первый Esc закрывает открытый
            // main stage (при открытом stage прочие оверлеи закрыты)
            // PRD-0007 (X2): окно проверки закрывается одним Esc
            ui_registry::id::EXPLAIN => {
                if self.explain.is_some() {
                    self.close_explain();
                    true
                } else {
                    false
                }
            }
            // PRD-0007 (X4): диалог ревью закрывается одним Esc —
            // отклонённые забываются (AC-5.2: возврат фоновой перепроверкой)
            ui_registry::id::AUTOLINK => {
                if self.autolink_review.is_some() {
                    self.close_autolink_review();
                    true
                } else {
                    false
                }
            }
            ui_registry::id::STAGE => {
                // FR-044 Р-7: при активной подсветке первое Esc гасит
                // подсветку (stage остаётся открытым), второе закрывает;
                // при неактивной — без изменений (первое Esc закрывает)
                if self.stage_calc_focus.take().is_some() {
                    self.stage_calc_hover = None;
                    true
                } else {
                    self.stage_calc_hover = None;
                    self.main_stage.take().is_some()
                }
            }
            // FR-027: двухэтапный Esc — подменю → меню → закрыто
            ui_registry::id::HELP_MENU => {
                if let Some(menu) = self.help_menu.take() {
                    // Подменю открыто — первый Esc закрывает только его
                    if menu.docs_open {
                        self.help_menu = Some(HelpMenuState {
                            origin: menu.origin,
                            docs_open: false,
                        });
                    }
                    true
                } else {
                    false
                }
            }
            ui_registry::id::DOCS => self.docs.take().is_some(),
            // FR-055 U4: витрина кита — Esc закрывает (один шаг, модаль)
            ui_registry::id::KIT_GALLERY => {
                if self.kit_gallery_open {
                    self.kit_gallery_open = false;
                    true
                } else {
                    false
                }
            }
            // FR-070: админпанель — Esc закрывает (один шаг, модаль)
            ui_registry::id::ADMIN => {
                if self.admin_open {
                    self.admin_open = false;
                    true
                } else {
                    false
                }
            }
            // Раскрытая колонка палитры закрывается без снятия выделения
            ui_registry::id::PALETTE => {
                if self.palette_hover.open.is_some() || self.palette_hover.pending() {
                    self.palette_hover.reset();
                    true
                } else {
                    false
                }
            }
            // Ревизия FR-025: Esc гасит flyout свёрнутой полосы палитры
            ui_registry::id::TEMPLATE_STRIP => {
                if self
                    .template_hover
                    .as_ref()
                    .is_some_and(|h| h.open.is_some() || h.pending())
                {
                    self.template_hover = None;
                    true
                } else {
                    false
                }
            }
            // FR-025 п.3: Esc сворачивает развёрнутый док и БЕЗ
            // клавиатурного фокуса — мышиный expand() даёт focused=false
            ui_registry::id::TEMPLATE_PANEL => {
                if self.template_panel.open {
                    self.template_panel.close();
                    self.persist_palette_dock();
                    true
                } else {
                    false
                }
            }
            ui_registry::id::MENU => self.menu.take().is_some(),
            // FR-050 Н2 (этап C): Esc закрывает меню выбора (отмена —
            // ребро не создаётся, «либо отмена» в постановке)
            ui_registry::id::CHOICE_MENU => self.choice_menu.take().is_some(),
            ui_registry::id::SETTINGS => {
                // FR-026: Esc при открытом меню закрывает ТОЛЬКО меню
                // (повторный Esc закроет панель — семантика popup FR-021)
                if self.settings_dropdown.is_open() {
                    self.settings_dropdown.reset();
                } else {
                    self.settings_open = false;
                }
                true
            }
            ui_registry::id::HOTKEYS => {
                if self.hotkeys_open {
                    self.hotkeys_open = false;
                    true
                } else {
                    false
                }
            }
            // FR-050 Н9-4 (этап E): Esc закрывает карту проливаний
            ui_registry::id::FLOW_MAP => {
                if self.flow_map_open {
                    self.flow_map_open = false;
                    true
                } else {
                    false
                }
            }
            // FR-017: выход из what-if режима (подмены не теряются —
            // они в персистентных сценариях `.canvas`, Q3b)
            ui_registry::id::WHATIF => {
                if self.scene.whatif_active {
                    self.exit_whatif_mode();
                    true
                } else {
                    false
                }
            }
            // FR-018: wheel-меню закрывается Esc (панель шаблонов — раньше)
            ui_registry::id::WHEEL => self.wheel_menu.take().is_some(),
            _ => false,
        }
    }

    /// FR-054 (Q4-a PRD-0009): обработчик клавиши владельцем-поверхностью —
    /// тела прежних head-веток on_key (FR-052) дословно; `true` — событие
    /// поглощено (доставка KeyboardRouter останавливается), `false` —
    /// скоуп пропускает событие вниз по стеку (к Canvas-лестнице).
    pub(super) fn route_owner_key(
        &mut self,
        owner: ui_registry::KeyOwner,
        event: &KeyEvent,
    ) -> bool {
        match owner {
            ui_registry::KeyOwner::Onboarding => {
                // FR-028: открытый онбординг глушит канвас-хоткеи (тур модален);
                // Esc — «Пропустить» (отложить до следующего запуска)
                if self.onboarding.is_some() {
                    if event.state == ElementState::Pressed
                        && !event.repeat
                        && event.logical_key == Key::Named(NamedKey::Escape)
                    {
                        self.defer_onboarding();
                        self.request_redraw();
                    }
                    return true;
                }
                true
            }
            ui_registry::KeyOwner::Gallery => {
                // FR-049: модальная галерея схем — клавиатура галереи (↑/↓/Enter/
                // Esc/фильтр), остальное глотается (канвас не получает)
                if self.scheme_gallery.open {
                    if event.state == ElementState::Pressed && self.on_gallery_key(event) {
                        self.request_redraw();
                    }
                    return true;
                }
                true
            }
            ui_registry::KeyOwner::KitGallery => {
                // FR-055 U4: витрина кита — модаль; Esc закрывает, прочие
                // клавиши глотаются (интерактив — только кнопки шапки).
                // FR-062 F-17: Tab/Shift+Tab — фокус-секция витрины
                // (FocusRing по слотам demo-ряда; рамка — accent).
                if self.kit_gallery_open {
                    if event.state == ElementState::Pressed && !event.repeat {
                        match &event.logical_key {
                            Key::Named(NamedKey::Escape) => {
                                self.kit_gallery_open = false;
                                self.request_redraw();
                            }
                            Key::Named(NamedKey::Tab) => {
                                let backwards = self.modifiers.shift_key();
                                self.gallery_focus_step(backwards);
                            }
                            _ => {}
                        }
                    }
                    return true;
                }
                true
            }
            ui_registry::KeyOwner::Admin => {
                // FR-070: админпанель — модаль; Esc закрывает, прочие
                // клавиши глотаются (интерактив — только контролы шапки
                // и сайдбара; фокус-кольцо — этап 2+ FR-070).
                if self.admin_open {
                    if event.state == ElementState::Pressed
                        && !event.repeat
                        && event.logical_key == Key::Named(NamedKey::Escape)
                    {
                        self.admin_open = false;
                        self.request_redraw();
                    }
                    return true;
                }
                true
            }
            ui_registry::KeyOwner::Editor => {
                // Активное редактирование (T7): клавиатура уходит в редактор
                if self.editing.is_some() {
                    if event.state != ElementState::Pressed {
                        return true;
                    }
                    let ctrl = self.modifiers.control_key();
                    let shift = self.modifiers.shift_key();
                    // FR-021: при открытом popup подсказок навигация/выбор
                    // перехватываются ДО команд редактора: Enter/Tab принимают
                    // подсказку (НЕ коммитят заметку), Esc закрывает только popup
                    // (повторный Esc — откат правки, прежнее поведение)
                    if self.hints.open {
                        match &event.logical_key {
                            Key::Named(NamedKey::ArrowDown) if !event.repeat => {
                                self.hints.move_selection(1);
                                self.request_redraw();
                                return true;
                            }
                            Key::Named(NamedKey::ArrowUp) if !event.repeat => {
                                self.hints.move_selection(-1);
                                self.request_redraw();
                                return true;
                            }
                            Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Tab)
                                if !event.repeat =>
                            {
                                self.accept_hint();
                                return true;
                            }
                            Key::Named(NamedKey::Escape) if !event.repeat => {
                                self.hints.reset();
                                self.request_redraw();
                                return true;
                            }
                            _ => {}
                        }
                    }
                    let Some(command) =
                        map_key(&event.logical_key, ctrl, shift).and_then(|command| {
                            match self.editing.as_ref() {
                                // FR-072: заголовок — однострочный (Enter — всегда
                                // коммит), маркеры стиля не применяются
                                Some(session) => session.adapt_command(command),
                                None => Some(command),
                            }
                        })
                    else {
                        return true;
                    };
                    match command {
                        KeyCommand::Commit => self.finish_editing(true),
                        KeyCommand::Cancel => self.finish_editing(false),
                        KeyCommand::Copy => {
                            if let Some(text) =
                                self.editing.as_ref().and_then(|s| s.copy_selection())
                            {
                                self.clipboard.set_text(text);
                            }
                        }
                        KeyCommand::Cut => {
                            let text = match (self.editing.as_mut(), self.renderer.as_mut()) {
                                (Some(session), Some(renderer)) => {
                                    session.cut_selection(renderer.font_system_mut())
                                }
                                _ => None,
                            };
                            if let Some(text) = text {
                                self.clipboard.set_text(text);
                                self.request_redraw();
                            }
                        }
                        KeyCommand::Paste => {
                            let text = self.clipboard.get_text();
                            let pasted = if let (Some(text), Some(session), Some(renderer)) =
                                (text, self.editing.as_mut(), self.renderer.as_mut())
                            {
                                session.insert_text(renderer.font_system_mut(), &text);
                                true
                            } else {
                                false
                            };
                            if pasted {
                                self.fit_note_size();
                                self.update_hints();
                                self.request_redraw();
                            }
                        }
                        other => {
                            let applied = if let (Some(session), Some(renderer)) =
                                (self.editing.as_mut(), self.renderer.as_mut())
                            {
                                session.apply(renderer.font_system_mut(), other);
                                true
                            } else {
                                false
                            };
                            if applied {
                                // Текст мог вырасти (wrap/новые строки) — подгоняем
                                // высоту заметки под контент прямо во время набора
                                self.fit_note_size();
                                // FR-021: popup подсказок — следом за правкой текста
                                self.update_hints();
                                self.request_redraw();
                            }
                        }
                    }
                    return true;
                }
                false
            }
            ui_registry::KeyOwner::Search => {
                // Панель поиска (T14): открыта — клавиатура уходит в панель
                // (ввод/каретка/Enter/Esc/F3), канвас-хоткеи приглушены
                if self.search.is_open() {
                    if event.state == ElementState::Pressed {
                        self.on_search_key(event);
                    }
                    return true;
                }
                true
            }
            ui_registry::KeyOwner::TemplatePanel => {
                // Прежний гейт 8036: панель без клавиатурного фокуса
                // клавиши не перехватывает — лестница (Ctrl+P и др.) работает
                if self.template_panel.focused && self.on_template_panel_key(event) {
                    return true;
                }
                false
            }
            ui_registry::KeyOwner::Dialog => {
                // T21: модальный диалог глушит весь ввод канваса — Enter/Esc —
                // подтвердить/отменить, остальное игнорируется (П10/П11)
                if self.dialog.is_some() && event.state == ElementState::Pressed && !event.repeat {
                    match event.logical_key {
                        Key::Named(NamedKey::Enter) => {
                            self.confirm_dialog();
                        }
                        Key::Named(NamedKey::Escape) => self.cancel_dialog(),
                        _ => {}
                    }
                    return true;
                }
                true
            }
            ui_registry::KeyOwner::Explain => {
                // PRD-0007 (X2): открытое окно проверки — Esc закрывает
                // (§6.4: Ready/Stale → Closed); прочие клавиши — в лестницу.
                // X3 (AC-4.1): открытое inline-поле подмены листа глушит
                // клавиатуру: символы — ввод, Enter — коммит, Esc — отмена.
                if event.state == ElementState::Pressed {
                    let edit_open = self.explain.as_ref().is_some_and(|s| s.edit.is_some());
                    if edit_open {
                        match &event.logical_key {
                            Key::Named(NamedKey::Enter) => {
                                self.finish_explain_edit();
                                self.request_redraw();
                                return true;
                            }
                            Key::Named(NamedKey::Escape) => {
                                if let Some(state) = self.explain.as_mut() {
                                    state.cancel_edit();
                                }
                                self.request_redraw();
                                return true;
                            }
                            Key::Named(NamedKey::Backspace) => {
                                if let Some(state) = self.explain.as_mut() {
                                    if let Some(edit) = state.edit.as_mut() {
                                        edit.backspace();
                                    }
                                }
                                self.request_redraw();
                                return true;
                            }
                            Key::Character(text) => {
                                if let Some(state) = self.explain.as_mut() {
                                    if let Some(edit) = state.edit.as_mut() {
                                        edit.type_str(text.as_str());
                                    }
                                }
                                self.request_redraw();
                                return true;
                            }
                            _ => {}
                        }
                        return true; // прочие клавиши глотаются, пока поле открыто
                    }
                    // X5 (AC-6.3): Space в защите — следующий уровень
                    // (вне защиты клавиша идёт по лестнице, как раньше)
                    let defense_now = self.explain.as_ref().is_some_and(|s| s.is_defense());
                    if defense_now
                        && event.logical_key == Key::Named(NamedKey::Space)
                        && !event.repeat
                    {
                        // шаг имеет смысл, только если есть скрытые узлы
                        let win = explain_ui::window_rect(self.viewport_logical());
                        let body = explain_ui::body_rect(win);
                        let can_step = self
                            .explain
                            .as_ref()
                            .map(|state| {
                                let (vis, _, _) = self.explain_view(state, body);
                                explain_ui::has_hidden(&vis)
                            })
                            .unwrap_or(false);
                        if can_step {
                            if let Some(state) = self.explain.as_mut() {
                                state.defense_step();
                            }
                        }
                        self.request_redraw();
                        return true;
                    }
                    if event.logical_key == Key::Named(NamedKey::Escape) && !event.repeat {
                        // X5 (AC-6.4): в защите Esc — выход в обычный вид
                        // окна (Defense → Ready); полное закрытие — второй
                        // Esc (Ready → Closed, ветка ниже)
                        if self.explain.as_ref().is_some_and(|s| s.is_defense()) {
                            if let Some(state) = self.explain.as_mut() {
                                state.exit_defense();
                            }
                            self.request_redraw();
                            return true;
                        }
                        self.close_explain();
                        self.request_redraw();
                        return true;
                    }
                }
                false
            }
            ui_registry::KeyOwner::Autolink => {
                // PRD-0007 (X4): открытый диалог ревью — Esc закрывает
                // (отклонённые забываются, AC-5.2); прочие клавиши глотаются
                // (модален поверх канваса, §6.5). FR-054: head-ветка
                // возвращает «поглощено» для доставки роутера.
                if self.autolink_review.is_some() {
                    if event.state == ElementState::Pressed
                        && !event.repeat
                        && event.logical_key == Key::Named(NamedKey::Escape)
                    {
                        self.close_autolink_review();
                    }
                    true
                } else {
                    false
                }
            }
            ui_registry::KeyOwner::Stage => {
                // FR-042 (правка 2026-09-25): только Esc закрывает stage.
                // Ранее любая клавиша закрывала (комментарий про «нужный
                // оверлей откроется следующим нажатием» — устарел: после
                // рефакторинга PRD-0009 overlay-openers сами закрывают
                // stage по инварианту Q6 — см. try_open_main_stage /
                // close_main_stage в каждом overlay-open). Закрытие на
                // любую клавишу блокировало Del внутри stage (FR-042
                // правка 2026-09-25: ЛКМ → палитра, ПКМ → stage + Del).
                // Esc — здесь явно (для discoverability); Esc-лестница
                // ниже (registry.esc_stack()) срабатывает когда Stage
                // возвращает false (но для Esc возвращаем true —
                // поглощено, чтобы лестница не дублировала).
                if event.state == ElementState::Pressed
                    && !event.repeat
                    && event.logical_key == Key::Named(NamedKey::Escape)
                {
                    self.close_main_stage();
                    self.request_redraw();
                    true
                } else {
                    // Любая другая клавиша — НЕ поглощать: событие идёт
                    // вниз по скоупам в Canvas (Del, Ctrl+P, F, и т.д.).
                    // Если клавиша открывает overlay (поиск/wheel/…),
                    // overlay-opener сам закроет stage (Q6 инвариант).
                    false
                }
            }
            ui_registry::KeyOwner::Canvas => false,
        }
    }

    pub(super) fn on_key(&mut self, event: &KeyEvent) {
        // FR-054 (Q4-a PRD-0009): весь on_key — доставка KeyboardRouter'ом
        // по скоуп-стеку из реестра (FR-051): верхний скоуп первым,
        // поглотивший гасит доставку; скоупы без обработчика
        // (settings/hotkeys/whatif/palette/…) пропускают событие вниз.
        // Canvas-лестница (команды и NUMI-хоткеи) — без изменений (Q4).
        // Дельта против U2: владелец ищется проходом по стеку (первая
        // поверхность с обработчиком), а не только верхом esc_stack, —
        // комбинация «док палитры в фокусе + палитра выделения видима»
        // снова отдаёт клавиши панели (прежний гейт 8036; регресс U2
        // устранён, тест router_delivery_matches_legacy_head).
        let registry = ui_registry::build_registry(self);
        let router = canvas_ui::KeyboardRouter::from_registry(&registry);
        if router
            .deliver(
                |activation| match ui_registry::owner_of(activation.surface.as_str()) {
                    Some(owner) => self.route_owner_key(owner, event),
                    None => false,
                },
            )
            .is_some()
        {
            return;
        }
        // Esc-лестница из реестра: порядок esc_stack воспроизводит прежнюю
        // ручную лестницу 8143–8232 дословно (первый поглотитель останавливает)
        if event.logical_key == Key::Named(NamedKey::Escape)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            for surface in registry.esc_stack() {
                if self.dispatch_esc(surface.as_str()) {
                    self.request_redraw();
                    return;
                }
            }
        }
        // FR-055 (этап U4, F-10): F9 — тогл DebugOverlay (слои/rect'ы/имя
        // под курсором/пересечения — G6). Вне KeyboardRouter НАМЕРЕННО:
        // диагностический тогл работает при любом скоупе (модали не глотают)
        // и не влияет на контракт поверхностей (в реестре не участвует).
        if event.state == ElementState::Pressed
            && !event.repeat
            && event.logical_key == Key::Named(NamedKey::F9)
        {
            self.debug_overlay = !self.debug_overlay;
            self.request_redraw();
            return;
        }
        // Ctrl+F — открыть панель поиска (T14; кириллическая раскладка — «а»);
        // активное редактирование сначала фиксируется
        if event.state == ElementState::Pressed
            && !event.repeat
            && self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("f") || c.eq_ignore_ascii_case("а"))
        {
            if self.editing.is_some() {
                self.finish_editing(true);
            }
            self.search.open();
            self.request_redraw();
            return;
        }
        // Ctrl+P — фокус в поиск палитры шаблонов (FR-018/FR-025;
        // кириллическая раскладка — «з»). Палитра — постоянный док:
        // Ctrl+P её не закрывает, а фокусирует (по свёрнутой — разворачивает
        // с чистым фильтром). Взаимоисключающе с wheel-меню
        if event.state == ElementState::Pressed
            && !event.repeat
            && self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("p") || c.eq_ignore_ascii_case("з"))
        {
            self.wheel_menu = None;
            // Ревизия FR-025: развёрнутый док гасит flyout полосы
            self.template_hover = None;
            if self.template_panel.open {
                self.template_panel.focus_search();
            } else {
                self.template_panel.open();
            }
            // W9 (web-приёмка): оракул браузерного дыма — палитра открылась
            // и реестр не пуст (DEBUG — на нативе под дефолтным фильтром
            // не виден; на web виден с ?log=debug)
            tracing::debug!(
                templates = self.templates.list().len(),
                categories = self.templates.categories().len(),
                "шаблонная палитра: док открыт/сфокусирован"
            );
            self.request_redraw();
            return;
        }
        // FR-017 (CP6): Ctrl+Shift+I — вход/выход из what-if режима
        // (Q3: Ctrl+W отклонён — мышечная память «закрыть вкладку»;
        // Ctrl+I занят курсивом в редакторе; кириллица — «Ш»).
        if event.state == ElementState::Pressed
            && !event.repeat
            && self.modifiers.control_key()
            && self.modifiers.shift_key()
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("i") || c.eq_ignore_ascii_case("ш"))
        {
            if self.scene.whatif_active {
                self.exit_whatif_mode();
            } else {
                self.enter_whatif_mode();
            }
            self.request_redraw();
            return;
        }
        // FR-050 Н9-4 (этап E): Ctrl+Shift+M — тогл панели «Карта
        // проливаний» (кириллица — «ь»/«Ь»; M = map, не занято: Ctrl+M
        // нет, Ctrl+Shift+M нет).
        if event.state == ElementState::Pressed
            && !event.repeat
            && self.modifiers.control_key()
            && self.modifiers.shift_key()
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("m") || c == "ь" || c == "Ь")
        {
            self.toggle_flow_map();
            return;
        }
        // FR-026: клавиатура выпадающего меню настроек — ↑/↓ сдвигают
        // выделение, Enter применяет (модель popup FR-021); Esc обрабатывается
        // ниже — первым делом закрывает меню, панель остаётся открытой
        if self.settings_open
            && self.settings_dropdown.is_open()
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            let row = self.settings_dropdown.open_row.expect("меню открыто");
            let count = dropdown_options(row, &self.settings).len();
            match &event.logical_key {
                Key::Named(NamedKey::ArrowDown) => {
                    self.settings_dropdown.move_selection(1, count);
                    self.request_redraw();
                    return;
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.settings_dropdown.move_selection(-1, count);
                    self.request_redraw();
                    return;
                }
                Key::Named(NamedKey::Enter) => {
                    let selected = self.settings_dropdown.selected;
                    self.apply_dropdown_choice(row, selected);
                    self.settings_dropdown.reset();
                    self.request_redraw();
                    return;
                }
                _ => {}
            }
        }
        // F1 — панель горячих клавиш (FR-004): раскладконезависимая
        // функциональная клавиша; внутри редактора/поиска не работает
        // (клавиатура ушла туда раньше — return выше)
        if event.logical_key == Key::Named(NamedKey::F1)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            self.hotkeys_open = !self.hotkeys_open;
            self.request_redraw();
            return;
        }
        // Ctrl+C/V/D/X — буфер нодов (FR-003/FR-007; кириллица: с/м/в/ч —
        // те же физические клавиши). Ctrl+Z/Y — undo/redo (FR-006;
        // кириллица: я/н). Внутри редактора эти клавиши — текстовые (выше
        // return: Ctrl+X там — вырезание текста), во время поиска — панель
        if event.state == ElementState::Pressed && !event.repeat && self.modifiers.control_key() {
            if let Key::Character(c) = &event.logical_key {
                match c.to_lowercase().as_str() {
                    "c" | "с" => self.copy_selection(),
                    "v" | "м" => self.paste_clipboard(),
                    "d" | "в" => self.duplicate_selection(),
                    "x" | "ч" => self.cut_selection(),
                    "z" | "я" => {
                        // Ctrl+Shift+Z — общепринятый синоним redo
                        if self.modifiers.shift_key() {
                            self.redo_action();
                        } else {
                            self.undo_action();
                        }
                    }
                    "y" | "н" => self.redo_action(),
                    "g" | "п" => self.group_selection(),
                    _ => {}
                }
            }
        }
        // FR-049: Ctrl+T — тогл галереи схем (кириллическая «е» — та же
        // физическая клавиша; во время редактирования не доходим — там
        // нет Ctrl+T-команды редактора)
        if event.state == ElementState::Pressed
            && !event.repeat
            && self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("t") || c == "е" || c == "Е")
        {
            if self.scheme_gallery.open {
                self.scheme_gallery.close();
            } else {
                self.scheme_gallery.open();
                self.empty_state_dismissed = false;
            }
            self.request_redraw();
            return;
        }
        // Ctrl+, — toggle панели настроек (кириллическая «б» — та же клавиша;
        // во время редактирования сюда не доходим — там Ctrl+Б это Bold)
        if event.state == ElementState::Pressed
            && !event.repeat
            && self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(c) if c == "," || c == "б" || c == "Б")
        {
            self.settings_open = !self.settings_open;
            self.settings_dropdown.reset();
            self.request_redraw();
            return;
        }
        // FR-016 (CP5): Ctrl+B — toggle оверлея узких мест (кириллическая
        // «и» — та же физическая клавиша; во время редактирования сюда не
        // доходим — там Ctrl+Б это Bold, конфликт решён комментарием выше).
        // Персистентная настройка — сохраняем конфиг сразу.
        if event.state == ElementState::Pressed
            && !event.repeat
            && self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("b") || c == "и" || c == "И")
        {
            self.toggle_bottleneck_overlay();
            self.save_settings();
            return;
        }
        if event.logical_key == Key::Named(NamedKey::Space) && !event.repeat {
            self.space_pressed = event.state == ElementState::Pressed;
            self.sync_cursor_icon();
            if !self.space_pressed {
                // Отпускание Space во время drag не должно оставлять ноду "прилипшей"
                // FR-006: применённое движение — undo-шаг; далее drag прерывается
                self.finish_interaction_undo();
                // FR-038 (п.9): drag прерван — предпросмотр направляющих гасится
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.clear_guides();
                }
                self.dragging = None;
            }
        }
        // T23 (brainstorm-focus): F (русская раскладка — «А») — переключить
        // режим фокуса связей. Конфликтов нет: Ctrl+F — поиск (обработан
        // выше с модификатором), F3 — HUD/цикл поиска (функциональная клавиша)
        if event.state == ElementState::Pressed
            && !event.repeat
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("f") || c == "а" || c == "А")
        {
            self.toggle_focus_mode();
            return;
        }
        // F3 — цикл по результатам поиска (T14), если они есть (в т.ч. после
        // закрытия панели — rows сохранены); иначе — HUD с fps/p95 (T5)
        if event.logical_key == Key::Named(NamedKey::F3)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            if !self.search.rows.is_empty() {
                self.cycle_search(if self.modifiers.shift_key() { -1 } else { 1 });
            } else {
                self.hud_visible = !self.hud_visible;
                self.request_redraw();
            }
        }
        // Del — удалить выделенную ноду (каскадно со связями) или связь (T8).
        // Во время редактирования сюда не доходим — там Delete работает в тексте
        if event.logical_key == Key::Named(NamedKey::Delete)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            self.delete_selected();
        }
        // FR-038 (п.22): клавиатурный nudge выделения — стрелки 1 px,
        // Shift+стрелки 10 px. Глобальный контур (ВНЕ оверлея поиска —
        // стрелки панели уходят в on_search_key с return выше): редактор/
        // поиск/диалог/настройки-меню тоже возвращают раньше. Ctrl не трогаем
        // — Ctrl+←/→ заняты mindmap-сворачиванием (ветка ниже). Повторы
        // клавиши НЕ глотаются: удержание двигает ноду, каждый шаг — своя
        // undo-операция (п.22 «каждый шаг в undo»)
        if event.state == ElementState::Pressed && !self.modifiers.control_key() {
            if let Key::Named(named) = &event.logical_key {
                let dir = match named {
                    NamedKey::ArrowUp => Some([0.0, -1.0]),
                    NamedKey::ArrowDown => Some([0.0, 1.0]),
                    NamedKey::ArrowLeft => Some([-1.0, 0.0]),
                    NamedKey::ArrowRight => Some([1.0, 0.0]),
                    _ => None,
                };
                if let Some(dir) = dir {
                    let screen_px = if self.modifiers.shift_key() {
                        10.0
                    } else {
                        1.0
                    };
                    self.nudge_selection(dir, screen_px);
                }
            }
        }
        // FR-011: mindmap-ветвление — Tab (дочерняя), Enter (сиблинг),
        // Ctrl+← (свернуть ветку), Ctrl+→ (развернуть). Только при выделенной
        // text-ноде; редактор/поиск/диалог приглушают канвас-хоткеи (return
        // выше — клавиатура уходит туда)
        if event.state == ElementState::Pressed && !event.repeat && !self.modifiers.shift_key() {
            let selected_index = match self.selected {
                Some(Selection::Node(index)) => Some(index),
                _ => None,
            };
            if let Some(index) = selected_index {
                let is_text = self
                    .scene
                    .canvas
                    .nodes
                    .get(index)
                    .is_some_and(|node| node.kind() == NodeKind::Text);
                if is_text {
                    let ctrl = self.modifiers.control_key();
                    match &event.logical_key {
                        Key::Named(NamedKey::Tab) => self.mindmap_add_child(index),
                        Key::Named(NamedKey::Enter) if !ctrl => self.mindmap_add_sibling(index),
                        Key::Named(NamedKey::ArrowLeft) if ctrl => {
                            self.mindmap_set_collapsed(index, true);
                        }
                        Key::Named(NamedKey::ArrowRight) if ctrl => {
                            self.mindmap_set_collapsed(index, false);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// FR-052 (U2): диспетчер кликов экрана — `HitStack::pick` решил, что
    /// точка принадлежит поверхности `surface` (элемент `element`).
    /// Тела обработчиков — дословный перенос прежних веток `on_left_button`
    /// (PRD-0009 §13 U2: реестр — единственный диспетчер). `false` —
    /// поверхность не поглотила (клик продолжает путь в канвас).
    pub(super) fn dispatch_surface_click(&mut self, surface: &str, element: &str) -> bool {
        match surface {
            ui_registry::id::GALLERY => {
                self.on_gallery_click();
                self.request_redraw();
                true
            }
            // FR-055 U4: витрина кита — «✕»/кнопка темы; прочий клик по панели
            // глотается (Block-модаль, backdrop-контракт закрывает мимо панели)
            ui_registry::id::KIT_GALLERY => {
                self.click_kit_gallery(element);
                true
            }
            // FR-070: админпанель — сайдбар/сброс/тема/«✕»; прочий клик
            // по панели глотается (Block-модаль)
            ui_registry::id::ADMIN => {
                self.click_admin_panel(element);
                true
            }
            ui_registry::id::ONBOARDING => {
                self.click_onboarding();
                true
            }
            ui_registry::id::EMPTY => {
                self.click_empty_state();
                true
            }
            ui_registry::id::STAGE => {
                self.click_main_stage();
                true
            }
            ui_registry::id::SEARCH => {
                self.click_search();
                true
            }
            // FR-050 Н9-4 (этап E): клик по панели карты — «✕»/строка;
            // мимо элементов внутри панели — глотается (панель жива)
            ui_registry::id::FLOW_MAP => {
                self.click_flow_map();
                true
            }
            ui_registry::id::WHEEL => {
                self.click_wheel_menu();
                true
            }
            ui_registry::id::TEMPLATE_PANEL => {
                self.click_template_panel();
                true
            }
            ui_registry::id::TEMPLATE_STRIP => {
                self.click_template_strip();
                true
            }
            ui_registry::id::HELP_MENU => {
                self.click_help_menu();
                true
            }
            ui_registry::id::DOCS => {
                self.click_docs();
                true
            }
            ui_registry::id::WHATIF => self.whatif_bar_click(),
            ui_registry::id::CORNER_BUTTONS => self.click_corner_button(element),
            ui_registry::id::SETTINGS => {
                self.click_settings();
                true
            }
            ui_registry::id::HOTKEYS => {
                self.click_hotkeys_panel();
                true
            }
            ui_registry::id::MINIMAP => {
                self.click_minimap();
                true
            }
            ui_registry::id::EXPLAIN => {
                // PRD-0007 (X2): ✕/чип/крошки/узлы; внутри окна мимо
                // элементов — глотается (канвас клик не получает)
                self.on_explain_click();
                true
            }
            ui_registry::id::AUTOLINK => {
                // PRD-0007 (X4): ✕/строки/баннер/футер; мимо элементов
                // внутри диалога — глотается
                self.on_autolink_click();
                true
            }
            ui_registry::id::DIALOG => {
                self.click_dialog();
                true
            }
            ui_registry::id::PALETTE => self.click_palette(),
            ui_registry::id::MENU => {
                self.click_context_menu();
                true
            }
            // FR-050 Н2 (этап C): клик по панели меню выбора — пункт
            // выполняет действие; заголовок/паддинг — отмена («мимо
            // пункта», как клик мимо панели — backdrop)
            ui_registry::id::CHOICE_MENU => {
                self.click_choice_menu();
                true
            }
            _ => false,
        }
    }

    /// FR-052 (U2): контракт «клик мимо Block-поверхности» (backdrop).
    /// gallery/search/settings/docs/help/stage/menu — закрыть и глотнуть;
    /// onboarding/dialog — глотнуть без закрытия (явный выбор); wheel —
    /// near/far логика внутри обработчика.
    pub(super) fn dispatch_surface_backdrop(&mut self, surface: &str) -> bool {
        match surface {
            // FR-055 U4: клик мимо витрины — закрыть и глотнуть (контракт
            // Block-поверхности, паттерн галереи схем)
            ui_registry::id::KIT_GALLERY => {
                self.kit_gallery_open = false;
                self.request_redraw();
                true
            }
            // FR-070: клик мимо админпанели — закрыть и глотнуть (паттерн
            // Block-поверхностей)
            ui_registry::id::ADMIN => {
                self.admin_open = false;
                self.request_redraw();
                true
            }
            ui_registry::id::GALLERY => {
                self.scheme_gallery.close();
                self.request_redraw();
                true
            }
            ui_registry::id::ONBOARDING => {
                self.request_redraw();
                true
            }
            ui_registry::id::STAGE => {
                self.main_stage = None;
                self.request_redraw();
                true
            }
            ui_registry::id::SEARCH => {
                self.search.close();
                self.request_redraw();
                true
            }
            // FR-050 Н9-4 (этап E): клик мимо панели карты — закрыть и
            // глотнуть (паттерн поиска)
            ui_registry::id::FLOW_MAP => {
                self.flow_map_open = false;
                self.request_redraw();
                true
            }
            ui_registry::id::SETTINGS => {
                // Двухэтапный dismiss (9533–9541 / 9592–9597): открытое меню —
                // закрывается только оно, модалка остаётся; иначе — модалка.
                if self.settings_dropdown.is_open() {
                    self.settings_dropdown.reset();
                } else {
                    self.settings_open = false;
                }
                self.request_redraw();
                true
            }
            ui_registry::id::MENU => {
                self.menu = None;
                self.request_redraw();
                true
            }
            // FR-050 Н2 (этап C): клик мимо панели меню выбора — отмена
            // («либо отмена» в постановке): закрыть и глотнуть
            ui_registry::id::CHOICE_MENU => {
                self.choice_menu = None;
                self.request_redraw();
                true
            }
            ui_registry::id::HELP_MENU => {
                self.help_menu = None;
                self.request_redraw();
                true
            }
            ui_registry::id::DOCS => {
                self.docs = None;
                self.request_redraw();
                true
            }
            ui_registry::id::DIALOG => {
                self.request_redraw();
                true
            }
            ui_registry::id::EXPLAIN => {
                // Клик по фону (мимо окна) — закрытие (on_explain_click X2)
                self.close_explain();
                true
            }
            ui_registry::id::AUTOLINK => {
                // Клик мимо диалога ревью — закрыть и глотнуть (§6.5:
                // модален поверх канваса); отклонённые забываются (AC-5.2)
                self.close_autolink_review();
                true
            }
            ui_registry::id::WHEEL => {
                self.click_wheel_menu();
                true
            }
            _ => false,
        }
    }

    /// Онбординг: кнопки карточки, остальное глотается.
    pub(super) fn click_onboarding(&mut self) {
        // FR-028: онбординг открыт — модальный оверлей: клики по
        // кнопкам карточки, остальное глотается (канвас не
        // реагирует; выход виден всегда — «Пропустить» в углу)
        if let Some(state) = &self.onboarding {
            let viewport = self.viewport_logical();
            let card = onboarding_ui::card_rect(viewport, state.step, self.settings.language);
            match onboarding_ui::button_at(card, state, self.cursor) {
                Some(OnboardingButton::Next) => {
                    if state.is_last() {
                        // «Готово»: тур пройден — флаг + сохранение
                        self.complete_onboarding();
                    } else if onboarding_ui::ONBOARDING_STEPS
                        .get(state.step)
                        .and_then(|step| step.action_key)
                        .is_some()
                    {
                        // FR-049: CTA шага («Попробовать») — тур
                        // пройден, галерея схем открыта
                        self.complete_onboarding();
                        self.scheme_gallery.open();
                    } else if let Some(state) = self.onboarding.as_mut() {
                        state.next();
                    }
                }
                Some(OnboardingButton::Prev) => {
                    if let Some(state) = self.onboarding.as_mut() {
                        state.prev();
                    }
                }
                Some(OnboardingButton::Skip) => self.defer_onboarding(),
                None => {}
            }
            self.request_redraw();
        }
    }

    /// Main stage: ✕/внутри — выделение ребра, мимо — закрыть.
    pub(super) fn click_main_stage(&mut self) {
        // FR-042 (E3, F-8/F-9): модальность main stage — клик вне
        // rect закрывает (канвас клик не получает, инвариант 8:
        // нода не создаётся, выделение не сбрасывается); внутри —
        // выделение ребра среза (живой индекс), без правки (PoC —
        // просмотр и выделение, non-goals PRD).
        // FR-044 Р-4/Р-5: внутри — строки панели «Как считается»
        // (фиксация подсветки), пилюли веера (= выделение ребра +
        // синхронная подсветка), клик по фону stage — сброс подсветки.
        if self.main_stage.is_some() {
            let viewport = self.viewport_logical();
            let rect = main_stage_rect(viewport);
            // FR-044 (прототип): кнопка ✕ в правом верхнем углу —
            // закрытие stage; геометрия — единый источник с рендером
            // stage_close_button_rect (FR-068 W3-продолжение — закрытие
            // класса дублированных формул CR-015)
            if point_in_rect(stage_close_button_rect(&rect), self.cursor) {
                self.close_main_stage();
                self.request_redraw();
                return;
            }
            if point_in_rect([rect.x, rect.y, rect.w, rect.h], self.cursor) {
                let stage = self.main_stage.as_ref().expect("stage открыт");
                let transform = StageTransform::new([rect.x, rect.y], stage.scale);
                let local = transform.unmap_point(self.cursor);
                let s = stage.scale.max(f32::EPSILON);
                // FR-044 Р-4/Р-5: общий контекст кадра — модель/панель/
                // зона (детерминизм: рендер и hit-test совпадают)
                let ctx = self.stage_frame_ctx(stage, &rect, s);
                let model = &ctx.model;
                // 1a) FR-044 Q2 (Scroll): клики по индикаторам прокрутки
                // пилюль — листание окна на видимое количество
                let (top_ind, bottom_ind) = self.stage_pill_scroll_indicators(stage, &ctx);
                let (pill_scroll, redraw) = match (
                    top_ind.filter(|r| point_in_rect([r.x, r.y, r.w, r.h], local)),
                    bottom_ind.filter(|r| point_in_rect([r.x, r.y, r.w, r.h], local)),
                ) {
                    (Some(_), _) => {
                        let page = match ctx.pill_mode {
                            calc_panel_ui::PillZoneMode::Scroll { visible, .. } => visible,
                            _ => 1,
                        };
                        (self.stage_pill_scroll.saturating_sub(page), true)
                    }
                    (_, Some(_)) => {
                        let page = match ctx.pill_mode {
                            calc_panel_ui::PillZoneMode::Scroll { visible, .. } => visible,
                            _ => 1,
                        };
                        (self.stage_pill_scroll + page, true)
                    }
                    _ => (self.stage_pill_scroll, false),
                };
                if redraw {
                    self.stage_pill_scroll = pill_scroll;
                    self.request_redraw();
                    return;
                }
                // 1) Строки панели «Как считается» — фиксация подсветки
                // (клик по панели мимо строк — глотается, фокус живёт)
                if let Some(panel) = &ctx.panel {
                    let rel = [self.cursor[0] - rect.x, self.cursor[1] - rect.y];
                    if panel.contains(rel) {
                        self.stage_calc_hover = None;
                        if let Some(i) = panel.var_row_at(rel) {
                            self.stage_calc_focus = Some(StageCalcFocus::for_var(model, i));
                        } else if let Some(i) = panel.formula_row_at(rel) {
                            self.stage_calc_focus = Some(StageCalcFocus::for_formula(model, i));
                        }
                        self.request_redraw();
                        return;
                    }
                }
                // 2) Пилюли веера (Р-5): клик = выделение ребра +
                // синхронная подсветка связанной строки и переменных
                if let Some((slice_i, _)) = self.stage_pill_hit(stage, &ctx, local) {
                    if let Some(live) = stage.live_edge(slice_i) {
                        self.selected = Some(Selection::Edge(live));
                        self.stage_calc_hover = None;
                        self.stage_calc_focus = Some(StageCalcFocus::for_edge(model, live));
                    }
                    self.request_redraw();
                    return;
                }
                // Допуск от толщины (F-5): max(EDGE_HIT_TOLERANCE, d/2 + 2).
                // Геометрия веера — та же чистая функция, что на кадре
                // (единый источник — ctx.lines)
                let tolerance = (bundle_thickness(stage.slice.edges.len()) / 2.0 + 2.0)
                    .max(canvas_core::EDGE_HIT_TOLERANCE);
                let hit = stage_edge_at_lines(&ctx.lines, local, tolerance)
                    .and_then(|slice_i| stage.live_edge(slice_i));
                // Заимствование stage закончено — можно мутировать
                if let Some(live) = hit {
                    self.selected = Some(Selection::Edge(live));
                    // Р-5: синхронная подсветка строки расчёта и переменных
                    self.stage_calc_hover = None;
                    self.stage_calc_focus = Some(StageCalcFocus::for_edge(model, live));
                } else {
                    // Р-5: клик по фону stage — сброс подсветки (выделение
                    // сохраняется — поведение прежнее)
                    self.stage_calc_focus = None;
                    self.stage_calc_hover = None;
                }
            } else {
                self.main_stage = None;
                self.stage_calc_focus = None;
                self.stage_calc_hover = None;
            }
            self.request_redraw();
        }
    }

    /// Wheel-меню: сектор/хаб/near/far — polar-геометрия.
    pub(super) fn click_wheel_menu(&mut self) {
        // FR-018: wheel-меню шаблонов — клики обрабатываются до
        // канваса (оверлей поверх всего). Сектор категории — выбор
        // категории (растут шаблонные кольца); сектор шаблона —
        // инстанциация в world-точку открытия; мимо секторов, но
        // рядом — глотаем, заметно дальше — закрыть.
        // Любой клик глотается — dismiss не создаёт заметку.
        if let Some(menu) = self.wheel_menu.clone() {
            let [vw, vh] = self.viewport_logical();
            let categories = self.templates.categories();
            let template_count = menu
                .category
                .as_deref()
                .map(|c| self.templates.by_category(c).len())
                .unwrap_or(0);
            let geo =
                template_ui::wheel_geometry(menu.screen, vw, vh, categories.len(), template_count);
            match geo.hit(self.cursor) {
                Some(WheelHit::Category(i)) => {
                    if let Some(menu_mut) = self.wheel_menu.as_mut() {
                        menu_mut.category = Some(categories[i].to_owned());
                    }
                }
                Some(WheelHit::Template(i)) => {
                    let category = menu.category.expect("категория выбрана");
                    let manifest = self.templates.by_category(&category)[i].clone();
                    let world = menu.world;
                    self.wheel_menu = None;
                    self.instantiate_template_at(&manifest, world);
                }
                None => {
                    // FR-022: клик по кнопке-хабу — «назад» (сброс
                    // категории) или «закрыть»; дальше — как раньше:
                    // рядом глотаем, заметно дальше — закрыть.
                    if geo.hub_hit(self.cursor) {
                        if let Some(menu_mut) = self.wheel_menu.as_mut() {
                            menu_mut.category = None;
                        }
                        if menu.category.is_none() {
                            self.wheel_menu = None;
                        }
                    } else {
                        let dx = self.cursor[0] - geo.center[0];
                        let dy = self.cursor[1] - geo.center[1];
                        let outside = (dx * dx + dy * dy).sqrt() > geo.extent + 12.0;
                        if outside {
                            self.wheel_menu = None;
                        }
                    }
                }
            }
            self.request_redraw();
        }
    }

    /// Док палитры: collapse/поиск/категории/строки.
    pub(super) fn click_template_panel(&mut self) {
        let viewport = self.viewport_logical();
        let rows = template_panel_rows(
            &self.templates,
            &self.template_panel,
            self.settings.language,
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
        );
        let mut handled = false;
        // Кнопка сворачивания дока («‹» в шапке)
        if point_in_rect(rect_xywh(lay.collapse_rect), self.cursor) {
            self.template_panel.close();
            self.persist_palette_dock();
            handled = true;
        }
        // Клик по полю поиска — клавиатурный фокус в панель
        if !handled && point_in_rect(rect_xywh(lay.input_rect), self.cursor) {
            self.template_panel.focus_search();
            handled = true;
        }
        if !handled {
            for (rect, name, _active) in &lay.category_rects {
                if point_in_rect(rect_xywh(*rect), self.cursor) {
                    self.template_panel.category =
                        if self.template_panel.category.as_deref() == Some(name) {
                            None
                        } else {
                            Some(name.clone())
                        };
                    self.template_panel.selected = 0;
                    self.template_panel.scroll_top = 0;
                    handled = true;
                    break;
                }
            }
        }
        if !handled {
            // FR-024: строки панели — секции (заголовки, клик
            // глотается) и карточки шаблонов (FR-025: нажатие
            // — кандидат в drag; вставка — на отпускании: клик —
            // в центр viewport, drag — в точку курсора)
            for (rect, row) in lay.row_rects.iter().zip(lay.rows.iter()) {
                if point_in_rect(rect_xywh(*rect), self.cursor) {
                    if let PanelRow::Template(index) = row {
                        self.template_drag = Some(template_ui::PanelDrag {
                            index: *index,
                            press: self.cursor,
                            active: false,
                        });
                    }
                    handled = true;
                    break;
                }
            }
        }
        if !handled && point_in_rect(rect_xywh(lay.panel_rect), self.cursor) {
            // Внутри дока, мимо элементов — глотаем
            handled = true;
        }
        if handled {
            self.request_redraw();
        }
    }

    /// Свёрнутая полоса + flyout: drag-кандидаты/пин/expand.
    pub(super) fn click_template_strip(&mut self) {
        // FR-025 (ревизия): свёрнутая палитра — полоса категорий
        // по центру слева; hover/pin раскрывает flyout справа.
        // Клик по строке flyout — drag-кандидат (инстанциация на
        // отпускании); по строке категории — пин-переключение;
        // по шеврону или полосе мимо строк — развернуть док;
        // мимо полосы — закрыть flyout, клик уходит в канвас.
        let viewport = self.viewport_logical();
        // FR-040 v2: геометрия по локализованным подписям — ТЕМ ЖЕ,
        // что в рендере дока (overlays.rs) — расхождений hit-test нет;
        // поиск по реестру — по raw-токену (zip по индексу).
        let raw_categories = self.template_category_names();
        let categories = self.template_category_display_names();
        // FR-054: ширины чипов — измеренные (measurer на вызов, паттерн U3).
        let mut measurer = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        let strip =
            template_ui::dock_strip_layout(&categories, viewport[1], &mut measurer, &mut fs);
        // Строка flyout: кандидат в drag (тот же пайплайн, что
        // и у развёрнутого дока — ghost + вставка на отпускании)
        let flyout_hit = self
            .template_flyout_geometry(viewport, &strip)
            .and_then(|fly| {
                self.template_hover.as_ref().and_then(|hover| {
                    hover.open.and_then(|cat| {
                        raw_categories.get(cat).and_then(|raw| {
                            let items = self.templates.by_category(raw);
                            fly.row_rects
                                .iter()
                                .enumerate()
                                .find(|(_, rect)| point_in_rect(**rect, self.cursor))
                                .and_then(|(v, _)| {
                                    items.get(fly.scroll_top + v).and_then(|m| {
                                        self.templates.list().iter().position(|lm| lm.id == m.id)
                                    })
                                })
                        })
                    })
                })
            });
        if let Some(index) = flyout_hit {
            self.template_drag = Some(template_ui::PanelDrag {
                index,
                press: self.cursor,
                active: false,
            });
            self.request_redraw();
            return;
        }
        if let Some(i) = strip
            .rows
            .iter()
            .position(|(rect, _)| point_in_rect(*rect, self.cursor))
        {
            // Пин-переключение flyout категории (WAI-ARIA)
            self.template_hover
                .get_or_insert_with(template_ui::StripHover::new)
                .toggle_trigger(i);
            self.request_redraw();
            return;
        }
        if point_in_rect(strip.rect, self.cursor) {
            // Шеврон или полоса мимо строк — развернуть док
            self.template_panel.expand();
            self.persist_palette_dock();
            self.template_hover = None;
            self.request_redraw();
            return;
        }
        // Мимо полосы: flyout закрывается, клик уходит в канвас
        self.template_hover = None;
    }

    /// Меню помощи: подменю первым, паддинг глотается.
    pub(super) fn click_help_menu(&mut self) {
        // FR-027: меню помощи (кнопка «?») и просмотрщик
        // документации — поповеры поверх канваса: клики
        // обрабатываются до кнопок/панели настроек
        if let Some(menu) = self.help_menu.take() {
            let viewport = self.viewport_logical();
            // Подменю разделов — ПЕРВЫМ (колонка правее/левее меню):
            // выбор открывает просмотрщик, паддинг — глотается
            if menu.docs_open {
                let sub = docs_ui::help_submenu_origin(menu.origin, viewport);
                if let Some(page) = docs_ui::help_submenu_item_at(sub, self.cursor) {
                    self.open_docs_page(page);
                    self.request_redraw();
                    return;
                }
                if point_in_rect(docs_ui::help_submenu_rect(sub), self.cursor) {
                    self.help_menu = Some(menu);
                    self.request_redraw();
                    return;
                }
            }
            match docs_ui::help_menu_item_at(menu.origin, self.cursor) {
                // «Документация ▸» — тогл подменю (7 разделов)
                Some(docs_ui::HelpMenuItem::Docs) => {
                    self.help_menu = Some(HelpMenuState {
                        origin: menu.origin,
                        docs_open: !menu.docs_open,
                    });
                }
                // «Пройти онбординг» (FR-028): явное намерение —
                // счётчик откладываний не трогается
                Some(docs_ui::HelpMenuItem::Onboarding) => {
                    self.onboarding = Some(OnboardingState::default());
                }
                // «Галерея схем» (FR-049): открыть модальную галерею
                Some(docs_ui::HelpMenuItem::Schemes) => {
                    self.help_menu = None;
                    self.scheme_gallery.open();
                }
                // FR-055 U4 (Q5-a): «О интерфейсе» — витрина кита
                // (модаль; вход из меню «?», доступна всегда)
                Some(docs_ui::HelpMenuItem::Interface) => {
                    self.help_menu = None;
                    // FR-070: админпанель и витрина взаимоисключимы
                    self.admin_open = false;
                    self.kit_gallery_open = true;
                    // FR-059: контент витрины — с начала (скролл секций)
                    self.kit_gallery_scroll = canvas_ui::kit::ScrollState::default();
                    // FR-062 F-17: фокус секции Tab — с начала (кольцо пустое:
                    // первый Tab ставит фокус на первый слот)
                    self.kit_gallery_focus.clear();
                    self.request_redraw();
                }
                // FR-070: «UI-консоль» — админпанель (модаль; вход из
                // меню «?», доступна всегда)
                Some(docs_ui::HelpMenuItem::Admin) => {
                    self.help_menu = None;
                    // Витрина и админпанель взаимоисключимы
                    self.kit_gallery_open = false;
                    self.admin_open = true;
                    self.admin_scroll = canvas_ui::kit::ScrollState::default();
                    self.request_redraw();
                }
                None => {
                    // Поверхность меню (паддинг) — глотается, меню
                    // остаётся; мимо — закрыть (клик глотается,
                    // паттерн контекстного меню T7)
                    if point_in_rect(docs_ui::help_menu_rect(menu.origin), self.cursor) {
                        self.help_menu = Some(menu);
                    }
                }
            }
            self.request_redraw();
        }
    }

    /// Просмотрщик документации: ✕/ссылки/панель.
    pub(super) fn click_docs(&mut self) {
        if self.docs.is_some() {
            let viewport = self.viewport_logical();
            let panel = docs_ui::viewer_rect(viewport);
            // × — закрыть
            if point_in_rect(docs_ui::viewer_close_rect(panel), self.cursor) {
                self.docs = None;
                self.request_redraw();
                return;
            }
            if point_in_rect(panel, self.cursor) {
                // Внутренняя ссылка — переход на страницу
                let content = docs_ui::viewer_content_rect(panel);
                let link = self.docs.as_ref().and_then(|viewer| {
                    docs_ui::link_at(
                        &viewer.layout,
                        viewer.scroll,
                        [content[0], content[1]],
                        self.cursor,
                    )
                    .cloned()
                });
                if let Some(docs_ui::LinkTarget::Page(id)) = link.map(|l| l.target) {
                    if let Some(page) = docs_ui::page_index_by_id(id) {
                        self.open_docs_page(page);
                        return;
                    }
                }
                // Клик по панели без ссылки — глотается
                self.request_redraw();
                return;
            }
            // Клик мимо панели — закрыть (клик глотается)
            self.docs = None;
            self.request_redraw();
        }
    }

    /// Модалка настроек: dropdown/навигация/карточки/строки.
    pub(super) fn click_settings(&mut self) {
        // Панель настроек (screen-space): клики обрабатываются до
        // канваса — кнопка/панель поверх и «прозрачности» не дают
        let viewport = self.viewport_logical();
        // Кнопка переключения темы — рядом с кнопкой настроек
        if point_in_rect(
            theme_button_rect(self.settings.button_corner, viewport),
            self.cursor,
        ) {
            self.toggle_theme();
            self.request_redraw();
            return;
        }
        // FR-040 v2: кнопка переключения языка (RU/EN) — между темой и «?»
        if point_in_rect(
            language_button_rect(self.settings.button_corner, viewport),
            self.cursor,
        ) {
            self.toggle_language();
            self.request_redraw();
            return;
        }
        // FR-027: кнопка «?» — тогл меню помощи (как ⚙ у настроек)
        // Q6 FR-042: открытие оверлея закрывает main stage
        if point_in_rect(
            help_button_rect(self.settings.button_corner, viewport),
            self.cursor,
        ) {
            self.close_main_stage();
            self.help_menu = match self.help_menu.take() {
                Some(_) => None,
                None => {
                    let button = help_button_rect(self.settings.button_corner, viewport);
                    Some(HelpMenuState {
                        origin: docs_ui::help_menu_origin(button, viewport),
                        docs_open: false,
                    })
                }
            };
            self.request_redraw();
            return;
        }
        if point_in_rect(
            button_rect(self.settings.button_corner, viewport),
            self.cursor,
        ) {
            // Q6 FR-042: открытие настроек закрывает main stage
            self.close_main_stage();
            self.settings_open = !self.settings_open;
            self.settings_dropdown.reset();
            self.request_redraw();
            return;
        }
        if self.settings_open {
            // FR-039: layout модалки — hit-тесты по навигации,
            // строкам и карточкам темы
            let layout = modal_layout(self.settings_tab, viewport);
            // Открытое выпадающее меню — первый приоритет: клик по
            // пункту применяет значение; клик мимо меню закрывает
            // ТОЛЬКО меню (модалка остаётся открытой — двухэтапный
            // dismiss), клик по другой строке обработается ниже
            if let Some(open_row) = self.settings_dropdown.open_row {
                let items = dropdown_options(open_row, &self.settings);
                let anchor = layout
                    .row_rect(open_row)
                    .map(|rect| control_rect(rect, RowKind::Dropdown))
                    .unwrap_or([0.0; 4]);
                let menu_rect = dropdown_layout(anchor, viewport, items.len(), anchor[2]);
                if point_in_rect(menu_rect, self.cursor) {
                    if let Some(index) = dropdown_item_at(menu_rect, items.len(), self.cursor) {
                        self.apply_dropdown_choice(open_row, index);
                    }
                    self.settings_dropdown.reset();
                    self.request_redraw();
                    return;
                }
                self.settings_dropdown.reset();
                if modal_row_at(&layout, self.cursor).is_none()
                    && !point_in_rect(layout.rect, self.cursor)
                {
                    // Клик вне меню, не по строке и не по модалке:
                    // меню закрыто, канвасу клик не достаётся
                    // (иначе создал бы заметку)
                    self.request_redraw();
                    return;
                }
            }
            // Пункт левой навигации — смена таба (+ сброс dropdown);
            // активный таб переживает закрытие модалки (в памяти App)
            if let Some(tab) = modal_nav_at(&layout, self.cursor) {
                self.settings_tab = tab;
                self.settings_dropdown.reset();
                self.request_redraw();
                return;
            }
            // Карточки темы (таб «Внешний вид») — прямой выбор
            // классики; клик по карточке сбрасывает пресет (FR-047:
            // карточки и пресет — взаимоисключающие источники темы)
            if let Some(theme) = modal_theme_card_at(&layout, self.cursor) {
                if self.settings.theme != theme || !self.settings.theme_preset.is_empty() {
                    self.settings.theme = theme;
                    self.settings.theme_preset.clear();
                    self.widgets.set_theme(theme == Theme::Dark);
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.set_theme(ThemeColors::from_theme(theme));
                    }
                    self.save_settings();
                }
                self.request_redraw();
                return;
            }
            if let Some(row) = modal_row_at(&layout, self.cursor) {
                match row_kind(row) {
                    RowKind::Toggle => self.apply_toggle_row(row),
                    RowKind::Dropdown => {
                        // Клик по dropdown-строке открывает меню
                        // значений (НЕ меняет значение); повторный
                        // клик по той же строке закрывает
                        if self.settings_dropdown.open_row == Some(row) {
                            self.settings_dropdown.reset();
                        } else {
                            self.settings_dropdown.open(row, &self.settings);
                        }
                    }
                }
            } else if !point_in_rect(layout.rect, self.cursor) {
                // FR-039: клик по затемнению (вне rect модалки) —
                // закрыть; канвасу клик не достаётся (иначе двойной
                // клик мимо создал бы заметку)
                self.settings_open = false;
                self.settings_dropdown.reset();
            }
            self.request_redraw();
        }
    }

    /// Модальный диалог: кнопки Да/Нет, мимо — глотается.
    pub(super) fn click_dialog(&mut self) {
        // T21: модальный диалог поверх всего — кнопки Да/Нет
        // (клики мимо панели не закрывают: установка — явный выбор)
        if self.dialog.is_some() {
            for (i, rect) in self.dialog_button_rects().iter().enumerate() {
                let [x, y, w, h] = *rect;
                if self.cursor[0] >= x
                    && self.cursor[0] <= x + w
                    && self.cursor[1] >= y
                    && self.cursor[1] <= y + h
                {
                    if i == 0 {
                        self.confirm_dialog();
                    } else {
                        self.cancel_dialog();
                    }
                    break;
                }
            }
            self.request_redraw();
        }
    }

    /// Контекстное меню: подменю/пункты/паддинг/мимо.
    pub(super) fn click_context_menu(&mut self) {
        // Открытое меню канваса (T7): клик по пункту — действие,
        // клик по поверхности меню (паддинг) — глотается, меню
        // ОСТАЁТСЯ открытым (Radix: клик внутри поверхности меню
        // не закрывает), клик мимо — закрыть (dismiss-клик в канвас
        // не проходит). M5: подменю проверяется ПЕРВЫМ — его колонка
        // правее базового меню. Hit-test — в логических px (курсор).
        if self.menu.is_some() {
            let in_base = self
                .menu_open_rect()
                .is_some_and(|rect| point_in_rect(rect, self.cursor));
            let in_submenu = self
                .menu
                .as_ref()
                .and_then(|m| m.submenu.as_ref())
                .map(submenu_rect)
                .is_some_and(|rect| point_in_rect(rect, self.cursor));
            // 1. Пункт подменю — действие
            if let Some(submenu) = self.menu.as_ref().and_then(|m| m.submenu.as_ref()) {
                if let Some(i) = submenu_item_at(submenu, self.cursor) {
                    let action = submenu.entries[i].action.clone();
                    self.menu = None;
                    match action {
                        crate::ui::SubmenuAction::Insert(widget_id) => {
                            self.insert_widget_from_menu(&widget_id);
                        }
                        // T21-C (П11): удаление пакета — с подтверждением;
                        // меню уже закрыто, модальный диалог поверх
                        crate::ui::SubmenuAction::Remove(widget_id) => {
                            let name = self
                                .widgets
                                .registry
                                .get(&widget_id)
                                .map(|p| p.manifest.name.clone())
                                .unwrap_or(widget_id.clone());
                            self.dialog = Some(AppDialog::RemovePackage { widget_id, name });
                        }
                    }
                    self.request_redraw();
                    return;
                }
            }
            // 2. Поверхность подменю без пункта — глотается, не закрывает
            if in_submenu {
                self.request_redraw();
                return;
            }
            // 3. Пункт или паддинг базового меню (список — тот же,
            // что в отрисовке: batch-пункты видны только при N≥3)
            if let Some(menu) = self.menu.take() {
                let items = canvas_menu_visible_items(self.align_menu_visible());
                if let Some(i) = menu_item_at_for(menu.origin, self.cursor, items.len()) {
                    match items[i] {
                        CanvasMenuItem::NewGroup => {
                            let center = self.viewport_center_world();
                            let mut group = plan_group_at(&self.scene.canvas, center);
                            group.label = Some(self.tr(keys::GROUP_DEFAULT_LABEL).to_owned());
                            self.insert_group(group);
                        }
                        // T23: переключение из меню — рантайм,
                        // без записи конфига (как и хоткей F)
                        CanvasMenuItem::FocusMode => self.toggle_focus_mode(),
                        // FR-004.1: тогл оверлея хоткеев из меню
                        // (панель «видно/не видно», галочка ✓)
                        CanvasMenuItem::Hotkeys => {
                            self.hotkeys_open = !self.hotkeys_open;
                        }
                        // M5 (T20-F): открыть подменю пакетов;
                        // повторный клик — тоггл (закрыть). Пустой
                        // список — честная строка «(нет установленных)».
                        // T21-C: под каждой вставкой — секция
                        // удаления пакетов (П11)
                        CanvasMenuItem::Widgets => {
                            if menu.submenu.is_some() {
                                // Тоггл: подменю уже открыто — закрыть
                                self.menu = Some(ContextMenu {
                                    origin: menu.origin,
                                    submenu: None,
                                });
                            } else {
                                let submenu_origin = submenu_origin_next_to(menu.origin);
                                let mut entries: Vec<SubmenuEntry> = self
                                    .widgets
                                    .menu_entries()
                                    .into_iter()
                                    .map(|(widget_id, label)| SubmenuEntry {
                                        action: crate::ui::SubmenuAction::Insert(widget_id),
                                        label,
                                    })
                                    .collect();
                                entries.extend(self.widgets.menu_entries().into_iter().map(
                                    |(widget_id, label)| SubmenuEntry {
                                        action: crate::ui::SubmenuAction::Remove(widget_id),
                                        label:
                                            self.trf(
                                                keys::WIDGETS_REMOVE_ENTRY,
                                                &[("{name}", &label)],
                                            ),
                                    },
                                ));
                                self.menu = Some(ContextMenu {
                                    origin: menu.origin,
                                    submenu: Some(Submenu {
                                        origin: submenu_origin,
                                        entries,
                                    }),
                                });
                            }
                        }
                        // T15: переключатель desktop-режима. Вход
                        // (runtime, без --desktop): перезапуск себя с
                        // --desktop через single-instance handoff —
                        // in-place SetParent не работает (Vulkan-swapchain
                        // не презентует в ребёнка Progman, Renderer
                        // фиксируется с prefer_dx12 при старте). Выход
                        // (уже встроены): in-place detach — DX12-рендерер
                        // в обычном окне презентует, пересоздание не нужно.
                        // На не-Windows — warn.
                        CanvasMenuItem::DesktopMode => {
                            #[cfg(windows)]
                            {
                                if self.desktop_mode && self.desktop_hierarchy.is_some() {
                                    self.leave_desktop();
                                } else {
                                    match self.spawn_desktop_relaunch() {
                                        Ok(()) => tracing::info!(
                                            "перезапуск на --desktop: новый инстанс \
                                             закроет текущий (single-instance handoff)"
                                        ),
                                        Err(err) => {
                                            tracing::warn!(
                                                %err,
                                                "перезапуск на --desktop не удался"
                                            );
                                            canvas_shell::desktop::attach::fallback_message_box(
                                                &format!(
                                                    "Не удалось перезапустить CanvasDesk \
                                                 в режиме десктопа:\n{err}\n\nЗапустите \
                                                 приложение вручную с флагом --desktop."
                                                ),
                                            );
                                        }
                                    }
                                }
                            }
                            #[cfg(not(windows))]
                            {
                                tracing::warn!("desktop-режим не поддерживается на этой платформе");
                            }
                        }
                        // FR-016 (CP5): тогл оверлея узких мест из меню —
                        // персистентная настройка (как Ctrl+B)
                        CanvasMenuItem::BottleneckOverlay => {
                            self.toggle_bottleneck_overlay();
                        }
                        // FR-017 (CP6): тогл what-if режима из меню
                        // (эквивалент Ctrl+Shift+I; подмены в
                        // сценариях переживают выход — Q3b)
                        CanvasMenuItem::WhatIf => {
                            if self.scene.whatif_active {
                                self.exit_whatif_mode();
                            } else {
                                self.enter_whatif_mode();
                            }
                        }
                        // FR-050 Н9-4 (этап E): тогл панели карты проливаний
                        CanvasMenuItem::FlowMap => self.toggle_flow_map(),
                        // PRD-0007 (FR-048 X4, AC-5.1): «Найти связи по именам»
                        // — немедленный скан детектора + диалог ревью
                        CanvasMenuItem::AutolinkFind => {
                            self.open_autolink_review();
                        }
                        // FR-038 п.16-17 (T-038.5): batch-операции
                        // выделения — ОДНА undo-операция на все ноды;
                        // хоткеи не назначаются (F1 HOTKEYS не трогаем,
                        // кандидат — на приёмку FR-038)
                        CanvasMenuItem::AlignHorizontal => {
                            // ряд: общая ось Y (центры на одной горизонтали)
                            self.run_batch_op(BatchOp::Align, Some(AlignAxis::Y));
                        }
                        CanvasMenuItem::AlignVertical => {
                            // колонна: общая ось X (центры на одной вертикали)
                            self.run_batch_op(BatchOp::Align, Some(AlignAxis::X));
                        }
                        CanvasMenuItem::DistributeEvenly => {
                            // ось раскладки — из контекста выделения
                            // (решение T-038.5: одна кнопка, правило в
                            // distribute_axis_for)
                            self.run_batch_op(BatchOp::Distribute, None);
                        }
                    }
                    self.request_redraw();
                    return;
                }
                if in_base {
                    // Паддинг базового меню — меню остаётся открытым
                    self.menu = Some(menu);
                    self.request_redraw();
                    return;
                }
                // Клик мимо — меню закрыто (take выше), клик глотается
                self.request_redraw();
            }
        }
    }

    /// Палитра выделения: Entry/Trigger/Bar поглощают, мимо rect'ов —
    /// `false` (клик уходит в канвас). palette_view() обновляет
    /// hover-intent — как в прежней цепочке.
    pub(super) fn click_palette(&mut self) -> bool {
        if let Some((lay, groups, open)) = self.palette_view() {
            match palette_hit(&lay, self.cursor, open) {
                Some(PaletteHit::Entry { group, entry }) => {
                    let action = groups[group].entries[entry].action.clone();
                    self.apply_palette_action(action);
                    // Действие выполнено — раскрытие закрывается
                    // (состав групп мог измениться; Radix: закрытие
                    // меню по выбору пункта)
                    self.palette_hover.reset();
                    self.request_redraw();
                    true
                }
                Some(PaletteHit::Trigger(group)) => {
                    // Пин: клик открывает без задержки / закрывает
                    // повторным кликом — стабильность для точного
                    // наведения, как у menu-button в вебе
                    self.palette_hover.toggle_trigger(group);
                    self.request_redraw();
                    true
                }
                Some(PaletteHit::Bar) => {
                    self.request_redraw();
                    true
                }
                None => false,
            }
        } else {
            false
        }
    }

    /// Клик при открытом диалоге ревью (AC-5.2): ✕/строка/баннер/футер;
    /// мимо элементов внутри диалога — глотается.
    pub(super) fn on_autolink_click(&mut self) {
        let viewport = self.viewport_logical();
        let win = autolink_ui::dialog_rect(viewport);
        if point_in_rect(autolink_ui::close_rect(win), self.cursor) {
            self.close_autolink_review();
            return;
        }
        // Баннер отклонённых: «Вернуть все» (У8/AC-5.2)
        if let Some(review) = self.autolink_review.as_ref() {
            let (_, rejected, _) = review.counts();
            if rejected > 0 {
                let banner = autolink_ui::banner_rect(win);
                if point_in_rect(autolink_ui::restore_rect(banner), self.cursor) {
                    if let Some(review) = self.autolink_review.as_mut() {
                        review.set_all(ItemState::Pending);
                    }
                    self.request_redraw();
                    return;
                }
            }
        }
        // Строки и заголовки групп (та же раскладка, что в рендере)
        if let Some(review) = self.autolink_review.as_ref() {
            let layout = autolink_ui::rows_layout(review, win, self.autolink_scroll);
            let (accepted_count, _, _) = review.counts();
            for (item_idx, rects) in &layout.rows {
                if !point_in_rect(rects.row, self.cursor) {
                    continue;
                }
                if point_in_rect(rects.accept, self.cursor) {
                    if let Some(review) = self.autolink_review.as_mut() {
                        review.toggle(*item_idx, ItemState::Accepted);
                    }
                } else if point_in_rect(rects.reject, self.cursor) {
                    if let Some(review) = self.autolink_review.as_mut() {
                        review.toggle(*item_idx, ItemState::Rejected);
                    }
                }
                self.request_redraw();
                return;
            }
            for (group_idx, head) in &layout.group_heads {
                if point_in_rect(*head, self.cursor) {
                    if let Some(review) = self.autolink_review.as_mut() {
                        if !review.collapsed.remove(group_idx) {
                            review.collapsed.insert(*group_idx);
                        }
                    }
                    self.request_redraw();
                    return;
                }
            }
            // Футер: массовые действия + создание (AC-5.2/AC-5.3)
            let [create, accept_all, reject_all] = autolink_ui::footer_buttons(win);
            if point_in_rect(create, self.cursor) && accepted_count > 0 {
                let accepted = self
                    .autolink_review
                    .as_ref()
                    .map(|review| review.accepted())
                    .unwrap_or_default();
                self.close_autolink_review();
                self.create_autolink_edges(accepted);
                return;
            }
            if point_in_rect(accept_all, self.cursor) {
                if let Some(review) = self.autolink_review.as_mut() {
                    review.set_all(ItemState::Accepted);
                }
                self.request_redraw();
                return;
            }
            if point_in_rect(reject_all, self.cursor) {
                if let Some(review) = self.autolink_review.as_mut() {
                    review.set_all(ItemState::Rejected);
                }
                self.request_redraw();
                return;
            }
        }
        // Мимо элементов внутри диалога — глотается (канвас не получает)
        self.request_redraw();
    }

    pub(super) fn on_explain_click(&mut self) {
        let viewport = self.viewport_logical();
        let win = explain_ui::window_rect(viewport);
        // ✕ — закрыть (снапшот → сессионный кэш; работает и в защите —
        // Defense → Closed, §6.4)
        if point_in_rect(explain_ui::close_rect(win), self.cursor) {
            self.close_explain();
            return;
        }
        let ready = self.explain.as_ref().is_some_and(|s| s.is_ready());
        if ready {
            // --- X3 (AC-4.1): inline-поле подмены листа -----------------
            if self.explain.as_ref().is_some_and(|s| s.edit.is_some()) {
                let body = explain_ui::body_rect(win);
                // Геометрия поля (та же, что в рендере): правый нижний
                // угол тела — чистая функция explain_ui (детерминизм)
                let field = explain_ui::field_rect(body);
                if point_in_rect(field, self.cursor) {
                    // Клик по самому полю — глотаем (текст уже сфокусирован)
                    self.request_redraw();
                    return;
                }
                // Клик мимо поля (но внутри окна) — коммит (паттерн
                // FR-017: клик мимо override-редактора фиксирует подмену);
                // после — клик обработан (не переходит в узлы дерева)
                self.finish_explain_edit();
                return;
            }
            let stale_now = self
                .explain
                .as_ref()
                .is_some_and(|s| s.revision != self.scene.revision);
            // Чип «Данные изменены» — единственный путь Stale → Ready
            // (AC-3.3): перестройка из нового снапшота, тот же корень
            if stale_now && point_in_rect(explain_ui::chip_rect(win), self.cursor) {
                let root = self.explain.as_ref().expect("готово").root.clone();
                self.open_explain(root);
                return;
            }
            // X5 (AC-6.1): тумблер режима защиты — вход/выход одним действием
            if point_in_rect(explain_ui::defense_toggle_rect(win), self.cursor) {
                let depth = self.settings.explain_depth_limit;
                if let Some(state) = self.explain.as_mut() {
                    if state.is_defense() {
                        state.exit_defense();
                    } else {
                        state.enter_defense(depth);
                    }
                }
                self.request_redraw();
                return;
            }
            // X5 (AC-6.3): кнопки пошагового раскрытия в защите
            let defense_now = self.explain.as_ref().is_some_and(|s| s.is_defense());
            if defense_now && point_in_rect(explain_ui::defense_step_rect(win), self.cursor) {
                // Шаг имеет смысл, только если есть скрытые уровни
                let body = explain_ui::body_rect(win);
                let can_step = self
                    .explain
                    .as_ref()
                    .map(|state| {
                        let (vis, _, _) = self.explain_view(state, body);
                        explain_ui::has_hidden(&vis)
                    })
                    .unwrap_or(false);
                if can_step {
                    if let Some(state) = self.explain.as_mut() {
                        state.defense_step();
                    }
                }
                self.request_redraw();
                return;
            }
            if defense_now && point_in_rect(explain_ui::defense_all_rect(win), self.cursor) {
                if let Some(state) = self.explain.as_mut() {
                    state.defense_reveal_all();
                }
                self.request_redraw();
                return;
            }
            // Мета-строка с крошками вида (X6 — полные чипы: клик по чипу
            // уровня обрезает путь AC-2.3; клик мимо чипов — ничего);
            // в защите крошки глушатся (вид зафиксирован на корне)
            if !defense_now && point_in_rect(explain_ui::meta_rect(win), self.cursor) {
                let path_len = self
                    .explain
                    .as_ref()
                    .map(|s| s.view_path.len())
                    .unwrap_or(0);
                if path_len > 1 {
                    let (offset, rects) = explain_ui::crumb_rects(win, path_len);
                    for (i, rect) in rects.iter().enumerate() {
                        if point_in_rect(*rect, self.cursor) {
                            if let Some(state) = self.explain.as_mut() {
                                state.click_crumb(offset + i);
                            }
                            self.request_redraw();
                            return;
                        }
                    }
                }
                self.request_redraw();
                return;
            }
            // Узел дерева: hit по лейауту кадра (та же чистая функция,
            // что в рендере — детерминизм рендер/ввод)
            let body = explain_ui::body_rect(win);
            // X3 (AC-4.1): кнопка «Изменить» на листе — приоритет перед
            // кликом по карточке (кнопка поверх); работает и в защите
            // (AC-4.4 — подмена из режима защиты)
            let edit_hit = self.explain.as_ref().and_then(|state| {
                let (_, layout, scale) = self.explain_view(state, body);
                let tree = state.tree()?;
                explain_ui::edit_at(tree, &layout, scale, body, self.cursor)
            });
            if let Some(idx) = edit_hit {
                // Preset — текущая подмена активного сценария (правка
                // существующей подмены), иначе исходник строки
                let preset = self.explain_leaf_preset(idx);
                if let Some(state) = self.explain.as_mut() {
                    let tree = state.tree().expect("дерево есть").clone();
                    state.start_edit(idx, &tree, preset);
                }
                self.request_redraw();
                return;
            }
            let hit = self.explain.as_ref().and_then(|state| {
                let (_, layout, scale) = self.explain_view(state, body);
                explain_ui::node_at(&layout, scale, body, self.cursor)
            });
            if let Some(idx) = hit {
                // Вид до мут-бейлка — hit-тест и клик используют одну
                // геометрию (explain_view — чистая функция)
                let vis = self.explain.as_ref().map(|state| {
                    let (vis, _, _) = self.explain_view(state, body);
                    vis
                });
                if let Some(state) = self.explain.as_mut() {
                    if let Some(vis) = vis {
                        let _click = state.click_node(idx, &vis);
                    }
                }
                self.request_redraw();
                return;
            }
        }
        // Клик по фону (мимо окна): §6.4 Ready/Stale → Closed; У2 (§6.5,
        // X6) — если под курсором подсвеченная нода канваса (есть в дереве,
        // в поддереве вида) — СИНХРОНИЗАЦИЯ канвас→дерево: выделить и
        // подвести узел дерева (вспышка), панель не закрывать;Defense —
        // канвас не отвечает (§6.5), клик по фону — полное закрытие.
        if !point_in_rect(win, self.cursor) {
            let defense_now = self.explain.as_ref().is_some_and(|s| s.is_defense());
            let ready_now = self.explain.as_ref().is_some_and(|s| s.is_ready());
            if ready_now && !defense_now {
                let index = self.selective_hit(self.cursor_world());
                if let Some(node_index) = index {
                    if let Some(node) = self.scene.canvas.nodes.get(node_index) {
                        let node_id = node.id.clone();
                        let depth = self.settings.explain_depth_limit;
                        let picked = self
                            .explain
                            .as_mut()
                            .map(|state| state.pick_by_node_id(&node_id, depth))
                            .unwrap_or(false);
                        if picked {
                            self.request_redraw();
                            return;
                        }
                    }
                }
            }
            self.close_explain();
            return;
        }
        self.request_redraw();
    }

    pub(super) fn on_left_button(&mut self, state: ElementState) {
        // Оракул браузерного дыма: приход события кнопки (координатная
        // сверка headless-тестов, ?log=debug)
        tracing::debug!(
            target: "canvas_app",
            pressed = matches!(state, ElementState::Pressed),
            cursor = ?self.cursor,
            "mouse: левая кнопка"
        );
        self.left_pressed = state == ElementState::Pressed;
        // T15: первый клик по канвасу снимает WS_EX_NOACTIVATE — с этого
        // момента окно может получать фокус («WS_EX_NOACTIVATE до первого
        // клика», TASKS T15); ошибки не критичны, флаг ставим до вызова
        // (повторные клики не ретраят). Клавиатурный фокус ставим явно на
        // КАЖДОМ нажатии: в ребёнке Progman клик активирует top-level-предка,
        // а фокус ввода нашему окну системой не передаётся — без SetFocus
        // WM_KEYDOWN не доходят и текст в нодах не редактируется
        // (attach::focus_window, идемпотентен — внутри GetFocus-проверка)
        #[cfg(windows)]
        if self.desktop_mode && state == ElementState::Pressed && self.desktop_hierarchy.is_some() {
            if !self.desktop_activation_enabled {
                self.desktop_activation_enabled = true;
                if let Some(raw) = self.window_hwnd() {
                    let hwnd = Self::hwnd(raw);
                    if let Err(err) = canvas_shell::desktop::attach::enable_activation(hwnd) {
                        tracing::warn!(%err, "не удалось снять WS_EX_NOACTIVATE");
                    }
                }
            }
            if let Some(raw) = self.window_hwnd() {
                let hwnd = Self::hwnd(raw);
                if let Err(err) = canvas_shell::desktop::attach::focus_window(hwnd) {
                    tracing::warn!(%err, "не удалось передать клавиатурный фокус");
                }
            }
        }
        if self.space_pressed {
            return; // Space+drag — панорамирование (SPEC §8)
        }
        match state {
            ElementState::Pressed => {
                // FR-052 (U2 PRD-0009): единый диспетчер поверхностей —
                // HitStack::pick по кадру реестра решает, кто получает клик
                // (порядок = слои/визуальный верх, а не порядок веток).
                // Block-модали глотают backdrop по контракту поверхности;
                // Capture — только в своих rect'ах; None → dismiss
                // транзиентов и прежняя canvas-цепочка (мир L0).
                // FR-PERF-A: кадр — из кэша (`App::ui_frame`), не прямой
                // вызов `build_frame` (он же идёт в `RedrawRequested`).
                let ui_frame = self.ui_frame();
                let pick = HitStack::pick(&ui_frame, UiPoint::new(self.cursor[0], self.cursor[1]));
                // Оракул браузерного дыма: кто забрал клик в точке (или None
                // — клик уходит канвасу), ?log=debug
                tracing::debug!(
                    target: "canvas_app",
                    pick = ?match &pick {
                        Some(HitTarget::Element { surface, .. }) => Some(surface.surface.as_str()),
                        Some(HitTarget::Backdrop { surface }) => Some(surface.surface.as_str()),
                        None => None,
                    },
                    "mouse: pick поверхностей"
                );
                match pick {
                    Some(HitTarget::Element { surface, rect }) => {
                        let surface_id = surface.surface.as_str().to_owned();
                        let element = rect.element.clone();
                        if self.dispatch_surface_click(&surface_id, &element) {
                            return;
                        }
                    }
                    Some(HitTarget::Backdrop { surface }) => {
                        let surface_id = surface.surface.as_str().to_owned();
                        if self.dispatch_surface_backdrop(&surface_id) {
                            return;
                        }
                    }
                    None => self.dismiss_transients_on_miss(),
                }
                let world = self.cursor_world();
                // PRD-0007 (F-1/AC-1.1): клик по цифре результата (полоса D)
                // — фолбэк-триггер окна проверки цепочки; у константы —
                // панель одного узла (AC-1.4)
                if self.explain.is_none() {
                    if let Some(root) = self.result_band_root_at(world) {
                        self.open_explain(root);
                        self.request_redraw();
                        return;
                    }
                }
                // FR-061 хвосты (D-7/D-8 runtime v1): клик по заголовку
                // блока-ведомости тогглит свёрнутость, по экспандеру описания
                // — раскрытость («Раскрыть+авто»); клик поглощается
                // (не доходит до выделения/драга — решение владельца).
                if self.handle_body_hit_click() {
                    self.request_redraw();
                    return;
                }
                // Выборочный hit-test (T5 + группы): ребёнок группы раньше
                // самой группы, не-group с меньшей площадью в приоритете
                let hit = self.selective_hit(world);
                // Оракул браузерного дыма: вход Pressed — cursor/world/hit
                // (координатная сверка headless-тестов, ?log=debug)
                tracing::debug!(
                    target: "canvas_app",
                    cursor = ?self.cursor,
                    world = ?world,
                    hit = ?hit.map(|i| self.scene.canvas.nodes[i].id.clone()),
                    "press: вход в канвас"
                );
                // FR-061 хвосты (D-8, «Раскрыть+авто»): клик мимо ноды
                // сворачивает раскрытые описания; клик по телу ноды
                // сохраняет её раскрытое описание (решение владельца).
                {
                    let keep = hit
                        .and_then(|index| self.scene.canvas.nodes.get(index))
                        .map(|n| n.id.clone());
                    self.scene.collapse_descs_except(keep.as_deref());
                }
                // Активное редактирование (T7/T8): клик внутри области
                // редактирования — в курсор, клик снаружи — commit и обычная
                // обработка
                if let Some(target) = self.editing.as_ref().map(EditingSession::target) {
                    let avoid = self.settings.edges_avoid_nodes;
                    let inside = match target {
                        EditTarget::Node(index) => hit == Some(index),
                        // FR-072: у заголовка зона клика — строка в шапке
                        EditTarget::NodeTitle(index) => self
                            .scene
                            .canvas
                            .nodes
                            .get(index)
                            .map(canvas_render::text::title_edit_area)
                            .is_some_and(|(origin, width, height)| {
                                world[0] >= origin[0]
                                    && world[0] <= origin[0] + width
                                    && world[1] >= origin[1]
                                    && world[1] <= origin[1] + height
                            }),
                        EditTarget::Edge(index) => edge_edit_area(&self.scene.canvas, index, avoid)
                            .is_some_and(|(origin, width, height)| {
                                world[0] >= origin[0]
                                    && world[0] <= origin[0] + width
                                    && world[1] >= origin[1]
                                    && world[1] <= origin[1] + height
                            }),
                    };
                    if inside {
                        let zoom_px = self.zoom_px();
                        if let (Some(session), Some(renderer)) =
                            (self.editing.as_mut(), self.renderer.as_mut())
                        {
                            if let Some((origin, _, _)) =
                                session_area(&self.scene.canvas, session, avoid)
                            {
                                let x = ((world[0] - origin[0]) * zoom_px) as i32;
                                let y = ((world[1] - origin[1]) * zoom_px) as i32;
                                session.click(renderer.font_system_mut(), x, y);
                                self.editor_dragging = true;
                            }
                        }
                        self.request_redraw();
                        return;
                    }
                    self.finish_editing(true);
                }
                // Хэндлы концов выделенной связи (CR-002): захват хэндла —
                // drag перепривязки без удаления. Проверка ДО портов: хэндл
                // сидит на порту, занятом существующей связью. Зона — та же,
                // что у портов (CR-003, из настроек).
                if let Some(Selection::Edge(edge_index)) = self.selected {
                    if let Some(end) = self.edge_handle_at(edge_index, world) {
                        self.edge_drag = Some(EdgeDrag::Rebind { edge_index, end });
                        self.request_redraw();
                        return;
                    }
                }
                // FR-025: ПОСТРОЧНЫЕ точки выхода (флаг line_ports) —
                // приоритет над сторонными портами в пределах своих рядов:
                // drag от кружка строки создаёт value-ребро со значением
                // именно этой строки (from_port, всегда value). Правка 2:
                // hit-test по кандидатам spatial-индекса — не привязан к
                // hovered (кружки наполовину торчат из ноды; см.
                // line_port_hit)
                if let Some((node_index, port)) = self.line_port_hit(world) {
                    let from_node = self.scene.canvas.nodes[node_index].id.clone();
                    self.edge_drag = Some(EdgeDrag::New {
                        from_node,
                        from_side: Side::Right,
                        // Точка выхода расчёта семантически value
                        value_flow: true,
                        from_port: Some(port),
                    });
                    self.request_redraw();
                    return;
                }
                // Порт hover-ноды (T8): начало drag резиновой линии новой
                // связи — drag ноды/resize/двойной клик не начинаются.
                // У групп портов нет: edge-drag с группы не начинается.
                // Зона захвата — из настроек (CR-003).
                if let Some(node_index) = self.hovered {
                    let port = self
                        .scene
                        .canvas
                        .nodes
                        .get(node_index)
                        .filter(|node| node.kind() != NodeKind::Group)
                        .and_then(|node| {
                            port_at(node, world, self.camera.zoom(), self.settings.port_zone_px)
                        });
                    if let Some(side) = port {
                        let from_node = self.scene.canvas.nodes[node_index].id.clone();
                        // FR-014: Shift+drag — value-ребро (поток значений),
                        // обычный drag — контрольная связь (дефолт)
                        let value_flow = self.modifiers.shift_key();
                        self.edge_drag = Some(EdgeDrag::New {
                            from_node,
                            from_side: side,
                            value_flow,
                            from_port: None,
                        });
                        self.request_redraw();
                        return;
                    }
                }
                // FR-018: Shift+клик по пустому месту — wheel-меню шаблонов
                // в точке курсора (мишень инстанциации — world-точка).
                // Нода/связь под курсором — обычная обработка выше.
                if self.modifiers.shift_key()
                    && hit.is_none()
                    && edge_at(&self.scene.canvas, world, self.settings.edges_avoid_nodes).is_none()
                {
                    self.wheel_menu = Some(template_ui::WheelMenu {
                        screen: self.cursor,
                        world,
                        category: None,
                    });
                    // W9 (web-приёмка): оракул браузерного дыма —
                    // Shift+клик поднял wheel-меню категорий (FR-018)
                    tracing::debug!(
                        categories = self.templates.categories().len(),
                        "wheel-меню шаблонов: категории"
                    );
                    self.request_redraw();
                    return;
                }
                // Двойной клик (winit его не даёт — свой детектор, T7):
                // по пустому месту — новая заметка, по text-ноде —
                // редактирование, по линии связи — лейбл связи (T8)
                if self.double_click.register(Instant::now(), self.cursor) {
                    let avoid = self.settings.edges_avoid_nodes;
                    match hit {
                        None => match edge_at(&self.scene.canvas, world, avoid) {
                            Some(edge_index) => {
                                // FR-042 (E3): двойной клик по пучку — main
                                // stage (лейбл-редактор у пучка неопределён;
                                // подписи отдельных рёбер видны в stage);
                                // одиночное ребро — лейбл, как раньше.
                                if self.try_open_main_stage(edge_index) {
                                    return;
                                }
                                self.begin_editing_edge(edge_index);
                            }
                            None => {
                                let index = self.create_note_at(world);
                                // FR-072: новая заметка стартует с заголовка:
                                // Enter после коммита заголовка откроет тело
                                // (цепочка title_then_body, см. finish_editing)
                                self.title_then_body = Some(index);
                                self.begin_editing_title(index);
                            }
                        },
                        // T17 (SPEC §7.4 п.7): двойной клик по файловой
                        // ноде — открыть ассоциацией «как в Explorer»
                        // (ShellExecuteEx SEE_MASK_INVOKEIDLIST);
                        // text-ноды — редактирование (T7)
                        Some(index) => {
                            // CR-006: у виджет-ноды текстового редактора нет —
                            // двойной клик (по хрому) не открывает его; клики
                            // по контенту до этой ветки не доходят (guard выше)
                            if self.scene.canvas.nodes[index].kind() == NodeKind::Widget {
                                self.request_redraw();
                                return;
                            }
                            // FR-017: в what-if режиме двойной клик по строке
                            // расчёта — override-поле подмены (база не
                            // редактируется — Q8); по прозаической строке —
                            // toast с объяснением.
                            if self.scene.whatif_active
                                && self.scene.canvas.nodes[index].kind() == NodeKind::Text
                            {
                                match self.calc_line_at(index, world) {
                                    Some(line) => {
                                        self.begin_whatif_override(index, line);
                                    }
                                    None => {
                                        self.show_toast(self.tr(keys::WHATIF_ONLY_CALC_LINES));
                                    }
                                }
                                self.request_redraw();
                                return;
                            }
                            #[cfg(windows)]
                            if let Some(file) = self.scene.canvas.nodes[index].file.clone() {
                                let path = resolve_node_path(&file, &self.scene.canvas_dir());
                                if let Err(err) = canvas_shell::desktop::interop::open_file(&path) {
                                    tracing::warn!(
                                        %err,
                                        path = %path.display(),
                                        "не удалось открыть файл"
                                    );
                                }
                            } else {
                                self.begin_edit_node(index, world);
                            }
                            #[cfg(not(windows))]
                            self.begin_edit_node(index, world);
                        }
                    }
                    self.request_redraw();
                    return;
                }
                // Ручной resize (T7): захват за правый нижний угол ноды
                if let Some(index) = hit {
                    if in_resize_corner(&self.scene.canvas.nodes[index], world) {
                        self.selected = Some(Selection::Node(index));
                        self.resizing = Some(index);
                        // FR-006: отложенный снапшот «до» resize — шаг
                        // закроется на отпускании при изменении размеров
                        self.begin_pending_undo();
                        self.request_redraw();
                        return;
                    }
                }
                match hit {
                    Some(index) => {
                        // CR-006: ЛКМ по КОНТЕНТУ виджет-ноды — ввод принадлежит
                        // виджету (WIDGETS.md §8.5). Ни выделения, ни drag,
                        // ни рамки выделения, ни семени фокуса: в live ввод
                        // и так уходит в HWND WebView2, в snapshot/placeholder
                        // клик глотается канвасом без оверлеев. Хром
                        // (заголовок 28 px / рамка 8 px) — прежнее поведение.
                        if self.scene.canvas.nodes[index].kind() == NodeKind::Widget {
                            let node = &self.scene.canvas.nodes[index];
                            let rect = [node.x, node.y, node.width, node.height];
                            if canvas_widgets::layout::hit_test(&rect, world)
                                == canvas_widgets::layout::WidgetHit::Content
                            {
                                tracing::debug!(
                                    node_id = %node.id,
                                    "клик по контенту виджета — канвас без оверлеев"
                                );
                                self.request_redraw();
                                return;
                            }
                        }
                        // Ctrl/Shift + клик (CR-001.3): уже выделенное
                        // (в т.ч. одиночный якорь) остаётся, клик-нутая
                        // тоглится; drag с модификатором не начинается
                        // (это правка выделения, не перемещение)
                        if self.modifiers.control_key() || self.modifiers.shift_key() {
                            let primary = self.selected.and_then(|selection| match selection {
                                Selection::Node(index) => Some(index),
                                Selection::Edge(_) => None,
                            });
                            let anchor = toggle_selection_with_primary(
                                primary,
                                &mut self.selected_nodes,
                                index,
                            );
                            self.selected = anchor.map(Selection::Node);
                            self.request_redraw();
                            return;
                        }
                        // Обычный клик: нода вне набора — набор сбрасывается
                        // (одиночное выделение); нода В наборе — тянем набор
                        let in_set = self.selected_nodes.contains(&index);
                        self.selected = Some(Selection::Node(index));
                        if !in_set {
                            self.selected_nodes.clear();
                        }
                        // Drag (T7/CR-001): исходные позиции — одна нода или
                        // весь набор (+ дети групп); на движении delta к всем
                        let origins = drag_origins(&self.scene.canvas, index, &self.selected_nodes);
                        // FR-006: отложенный снапшот «до» перемещения — шаг
                        // закроется на отпускании при фактическом сдвиге
                        self.begin_pending_undo();
                        // FR-012: новый drag отменяет settle-анимацию и
                        // сбрасывает цель втягивания
                        self.settle_anim = None;
                        self.group_drop_target = None;
                        // FR-073: сессия расталкивания — якоря всех нод =
                        // текущие позиции (внешние сдвиги между драгами
                        // поглощаются)
                        self.drag_push_begin();
                        self.dragging = Some(DragState {
                            primary: index,
                            grab_world: world,
                            origins,
                        });
                    }
                    // Промах по нодам: hit-test связей (T8) — ближайшая
                    // в допуске EDGE_HIT_TOLERANCE, иначе сброс выделения.
                    // Рамка (CR-001): drag с пустого места тянет выделение —
                    // финал на отпускании (порог клик/драг отсекает клики)
                    // FR-042 (правка 2026-09-25): ЛКМ по пучку БОЛЕЕ НЕ
                    // открывает main stage — выделяет конкретное ребро и
                    // показывает палитру связи (как для одиночного ребра).
                    // Палитра для пучка содержит доп. группу «Пучок» с
                    // действием «Удалить пучок» (все N рёбер сразу). Stage
                    // открывается ПКМ — см. on_right_button.
                    None => {
                        if let Some(edge_index) =
                            edge_at(&self.scene.canvas, world, self.settings.edges_avoid_nodes)
                        {
                            self.selected = Some(Selection::Edge(edge_index));
                        } else {
                            self.selected = None;
                        }
                        self.selected_nodes.clear();
                        self.select_rect = Some((world, world, self.cursor));
                    }
                }
                self.request_redraw();
            }
            ElementState::Released => {
                // FR-025: отпускание нажатия на строке палитры — вставка:
                // drag (порог пройден) — в world-точку курсора, клик — в
                // центр viewport. Инстанциация на отпускании, а не на
                // нажатии, чтобы отличить drag от клика
                if let Some(drag) = self.template_drag.take() {
                    let manifest = self.templates.list().get(drag.index).cloned();
                    if let Some(manifest) = manifest {
                        if drag.active {
                            let world = self.cursor_world();
                            self.instantiate_template_at(&manifest, world);
                        } else {
                            let center = self.viewport_center_world();
                            self.instantiate_template_at(&manifest, center);
                        }
                    }
                    self.request_redraw();
                }
                // Рамка выделения (CR-001): движение больше порога —
                // выделяем ноды, пересекающие прямоугольник (AABB,
                // частичное вхождение считается); клик без движения уже
                // отработал в Pressed (edge/сброс)
                if let Some((start, _, press)) = self.select_rect.take() {
                    let moved = (self.cursor[0] - press[0]).abs() > SELECT_DRAG_THRESHOLD
                        || (self.cursor[1] - press[1]).abs() > SELECT_DRAG_THRESHOLD;
                    if moved {
                        let rect = rubber_band_rect(start, self.cursor_world());
                        self.selected_nodes = nodes_in_rect(&self.scene.canvas, rect);
                        self.selected = None;
                    }
                    self.request_redraw();
                }
                // Drop резиновой линии: новая связь (T8) или перепривязка
                // конца существующей (CR-002). На другую ноду — применяем,
                // в пустоту/на ту же ноду/на зеркальный конец — отмена
                if let Some(drag) = self.edge_drag.take() {
                    // FR-050 Н2 (этап C): подсветка целей drag гасится на
                    // отпускании (drag завершён)
                    self.param_drop = None;
                    let world = self.cursor_world();
                    match drag {
                        EdgeDrag::New {
                            from_node,
                            from_side,
                            value_flow,
                            from_port,
                        } => {
                            // FR-025: drag от построчного порта всегда
                            // value-ребро (точка выхода расчёта)
                            let value_flow = value_flow || from_port.is_some();
                            // FR-050 Н2 (этап C): drop value-drag на якорь
                            // параметра шаблонной ноды — value-ребро с toParam
                            // (занятость/цикл/W-AMBIGUOUS-SRC — внутри);
                            // приоритет над портом стороны (якорь сидит на
                            // левом краю, как порт — но семантика точнее)
                            if value_flow {
                                if let Some((target, port)) = self.param_port_hit(world) {
                                    let to_id = self.scene.canvas.nodes[target].id.clone();
                                    if to_id != from_node {
                                        self.drop_to_param(
                                            from_node.clone(),
                                            from_side,
                                            from_port.as_ref(),
                                            to_id,
                                            port.param.clone(),
                                        );
                                        self.request_redraw();
                                        return;
                                    }
                                }
                            }
                            if let Some(target) = self.selective_hit(world) {
                                let to_node = &self.scene.canvas.nodes[target];
                                let to_id = to_node.id.clone();
                                if to_id != from_node {
                                    // FR-050 Н2 (этап C): drop value-ребра на
                                    // шаблонную ноду мимо якоря — меню выбора
                                    // параметра приёмника (иначе позиционное
                                    // ребро даст W-UNUSED-SLOT и авто-строку
                                    // вместо проливания в параметр)
                                    if value_flow && to_node.template().is_some() {
                                        self.open_param_choice_menu(
                                            from_node.clone(),
                                            from_side,
                                            from_port.as_ref(),
                                            to_id,
                                        );
                                        self.request_redraw();
                                        return;
                                    }
                                    let to_side = nearest_side(to_node, world);
                                    // FR-025: построчный исток — индекс строки
                                    // (футер шаблонной ноды — None: узловое
                                    // значение)
                                    let from_line = from_port.and_then(|port| port.line);
                                    if value_flow {
                                        // FR-014: value-ребро, замыкающее цикл,
                                        // — диалог (контрольная связь / отмена);
                                        // валидное — создаётся сразу
                                        if canvas_core::creates_value_cycle(
                                            &self.scene.canvas,
                                            &from_node,
                                            &to_id,
                                        ) {
                                            // FR-025: from_line в диалог не
                                            // попадает — фолбэк (control)
                                            // построчную семантику отбрасывает
                                            self.dialog = Some(AppDialog::EdgeCycle {
                                                from_node,
                                                from_side,
                                                to_node: to_id,
                                                to_side,
                                            });
                                        } else {
                                            self.create_edge(
                                                from_node,
                                                from_side,
                                                to_id,
                                                to_side,
                                                FlowKind::Value,
                                                from_line,
                                            );
                                        }
                                    } else {
                                        self.create_edge(
                                            from_node,
                                            from_side,
                                            to_id,
                                            to_side,
                                            FlowKind::Control,
                                            None,
                                        );
                                    }
                                }
                            }
                        }
                        // CR-002: перепривязка конца — id/лейбл/цвет/стиль
                        // сохраняются (retarget_edge), сторона — ближайшая
                        // к курсору сторона целевой ноды
                        EdgeDrag::Rebind { edge_index, end } => {
                            if let Some(target) = self.selective_hit(world) {
                                let target_node = &self.scene.canvas.nodes[target];
                                let target_id = target_node.id.clone();
                                let side = nearest_side(target_node, world);
                                // FR-006: перепривязка — undo-шаг ДО мутации;
                                // retarget сам отклонит бесполезный перенос —
                                // тогда шаг снимается (no-op клики не копятся)
                                self.push_undo();
                                if canvas_core::retarget_edge(
                                    &mut self.scene.canvas,
                                    edge_index,
                                    end,
                                    &target_id,
                                    side,
                                ) {
                                    self.scene.mark_dirty();
                                    // FR-014: перепривязка могла изменить
                                    // топологию value-потока
                                    self.scene.recompute_flow();
                                } else {
                                    self.scene.undo_stack.pop_back();
                                }
                            }
                        }
                    }
                    self.request_redraw();
                }
                // FR-012: отпускание drag — втягивание в группу (зона была
                // подсвечена) или вынос из группы (отпускание вне rect своей
                // явной группы); каждое — свой undo-шаг membership
                let drop_target = self.group_drop_target.take();
                if let Some(group_index) = drop_target {
                    self.group_insert_dragged(group_index);
                } else {
                    self.group_drag_out_released();
                }
                // FR-038 (п.2/21): snap-at-release — коррекция свободной позиции
                // отпускания по сетке/направляющим. При сработавшем снапе — ДВА
                // undo-шага: drag-шаг закрывается внутри (снапшот начала → до
                // коррекции), затем шаг коррекции (до → после); без снапа —
                // обычный одиночный путь FR-006
                if !self.apply_snap_at_release() {
                    // FR-006: закрытие отложенного drag/resize — undo-шаг при
                    // фактическом изменении (клик без движения не шаг)
                    self.finish_interaction_undo();
                }
                // FR-038 (п.9): направляющие не переживают отпускание
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.clear_guides();
                }
                // FR-073: drop — активные закрепляются где брошены (включая
                // snap-коррекцию выше), накрытые ореолом получают новые
                // якоря; остальные расселяются пружиной (кадровый тик)
                if self.settings.drag_push_enabled {
                    let active = self.drag_push_active();
                    let params = self.drag_push_params();
                    canvas_core::drag_push::commit_drop(
                        &self.scene.canvas,
                        &active,
                        &mut self.drag_push,
                        &params,
                    );
                    // Оракул браузерного дыма: якоря после коммита (финал
                    // сессии — где кто закреплён)
                    tracing::debug!(
                        target: "canvas_app",
                        anchors = ?self.scene.canvas.nodes.iter().map(|n| (n.id.as_str(), self.drag_push.anchors.get(&n.id).copied().unwrap_or([n.x, n.y]))).collect::<Vec<_>>(),
                        "drag_push: drop — якоря закоммичены"
                    );
                }
                self.dragging = None;
                self.editor_dragging = false;
                self.resizing = None;
                self.minimap_drag = false;
            }
        }
    }

    pub(super) fn on_right_button(&mut self, state: ElementState, event_loop: &ActiveEventLoop) {
        // Вне Windows параметр не читается (системное меню T17 — Win32);
        // явный let вместо underscore-имени: имя остаётся осмысленным
        #[cfg(not(windows))]
        let _ = event_loop;
        if state != ElementState::Pressed {
            return;
        }
        // FR-028: открытый онбординг модален — ПКМ глотается (меню
        // канваса/палитра не всплывают под оверлеем)
        if self.onboarding.is_some() {
            self.request_redraw();
            return;
        }
        // FR-050 Н2 (этап C): ПКМ закрывает меню выбора (отмена) — канвасное
        // меню не всплывает поверх активного выбора
        if self.choice_menu.take().is_some() {
            self.request_redraw();
            return;
        }
        // FR-027: ПКМ над меню помощи/просмотрщиком — не открывает меню
        // канваса (клик в поповер — его поверхность, паттерн popover)
        {
            let viewport = self.viewport_logical();
            let over_help = self.help_menu.as_ref().is_some_and(|menu| {
                let in_menu = point_in_rect(docs_ui::help_menu_rect(menu.origin), self.cursor);
                let in_sub = menu.docs_open && {
                    let sub = docs_ui::help_submenu_origin(menu.origin, viewport);
                    point_in_rect(docs_ui::help_submenu_rect(sub), self.cursor)
                };
                in_menu || in_sub
            });
            let over_docs =
                self.docs.is_some() && point_in_rect(docs_ui::viewer_rect(viewport), self.cursor);
            if over_help || over_docs {
                self.request_redraw();
                return;
            }
        }
        // ПКМ во время редактирования — сначала commit (T7)
        if self.editing.is_some() {
            self.finish_editing(true);
        }
        // FR-050 Н9-3 (этап E): ПКМ по пролитой строке/авто-строке —
        // контекст-меню параметра («Показать источник» / «Отключить
        // проливание» / «Что если…») вместо палитры ноды. Зоны — экранной
        // раскладки тела (Н9-2, логические px) с прошлого кадра; клик по
        // строке без ребра (кэш протух) — проваливается в обычный путь.
        if let Some(hit) = self.spill_hit_at(self.cursor).cloned() {
            if self.open_param_menu(&hit) {
                self.request_redraw();
                return;
            }
        }
        let world = self.cursor_world();
        match self.selective_hit(world) {
            // Нода: выделить → палитра выделения под нодой (FR-009/FR-010).
            // Мультивыделение сохраняется при ПКМ по выделенной ноде
            Some(index) => {
                if !self.selected_nodes.contains(&index) {
                    self.selected_nodes.clear();
                }
                self.selected = Some(Selection::Node(index));
                self.menu = None;
            }
            // Связь: выделить → палитра связи (Стиль/Толщина/Цвет/
            // Поток/Порты; для пучка N ≥ 2 — дополнительно «Пучок» с
            // действием «Удалить пучок» — все N рёбер сразу); мимо —
            // меню пустого канваса или закрытие (десктоп-меню T17).
            // FR-042 (правка 2026-09-25): ПКМ по пучку открывает main
            // stage (детализация с веером рёбер и подписями). Внутри
            // stage клик по ребру выделяет живую связь, Del — удаляет
            // (валидация среза закрывает stage при потере ребра).
            // Одиночное ребро — выделение + палитра (как раньше).
            None => {
                let avoid = self.settings.edges_avoid_nodes;
                match edge_at(&self.scene.canvas, world, avoid) {
                    Some(edge_index) => {
                        if self.try_open_main_stage(edge_index) {
                            self.request_redraw();
                            return;
                        }
                        self.selected = Some(Selection::Edge(edge_index));
                        self.selected_nodes.clear();
                        self.menu = None;
                    }
                    None => {
                        // T17 (SPEC §7.4 п.6): в --desktop ПКМ по пустому месту —
                        // системное меню десктопа (нативное Win32: Открыть
                        // канвас / Новый текстовый файл / иконки / автозапуск /
                        // Выход); вне --desktop — меню пустого канваса
                        // (создание группы), повторный ПКМ мимо закрывает его.
                        // Исключение: Shift+ПКМ в --desktop открывает canvas-меню
                        // (пункт «✓ Режим десктопа» — выход из встройки без
                        // выхода из приложения). Origin — логические px
                        // (screen-space меню).
                        // Взаимоисключение поповеров: открытие меню прячет
                        // палитру и сбрасывает её раскрытие
                        self.palette_hover.reset();
                        #[cfg(windows)]
                        let desktop_menu = self.desktop_mode
                            && self.desktop_hierarchy.is_some()
                            && !self.modifiers.shift_key();
                        #[cfg(not(windows))]
                        let desktop_menu = false;
                        if desktop_menu {
                            self.menu = None;
                            #[cfg(windows)]
                            self.desktop_menu(event_loop);
                        } else {
                            self.menu = match self.menu.take() {
                                // Повторный ПКМ по тому же пустому месту —
                                // закрыть (тоггл, как у ноды/связи)
                                Some(_) => None,
                                _ => Some(ContextMenu {
                                    origin: self.cursor,
                                    submenu: None,
                                }),
                            };
                        }
                    }
                }
            }
        }
        self.request_redraw();
    }

    pub(super) fn on_cursor_moved(&mut self, position: PhysicalPosition<f64>) {
        let scale = self.scale_factor();
        let logical = [position.x as f32 / scale, position.y as f32 / scale];
        // Drag по миникарте (T13): пан следует за курсором — раньше
        // канвас-панорамирования, дрги не конкурируют (нажатие перехвачено)
        if self.minimap_drag {
            self.cursor = logical;
            self.center_camera_on_minimap_cursor();
            self.request_redraw();
            return;
        }
        if self.panning() && self.main_stage.is_none() {
            let delta = [logical[0] - self.cursor[0], logical[1] - self.cursor[1]];
            self.camera.pan(delta);
            self.request_redraw();
        }
        self.cursor = logical;
        // FR-050 Н2 (этап C): hover-пункт меню выбора (подсветка следует
        // за курсором — паттерн аффорданса контекстного меню; геометрия —
        // сдвиг пунктов на заголовок, как в отрисовке)
        if let Some(menu) = self.choice_menu.as_mut() {
            let count = menu.items.len();
            let shifted = [menu.origin[0], menu.origin[1] + CHOICE_MENU_TITLE_H];
            let hovered = crate::ui::menu_item_at_for(shifted, logical, count);
            if menu.hovered != hovered {
                menu.hovered = hovered;
                self.request_redraw();
            }
        }
        // FR-025: нажатие на строку палитры — порог переводит его в drag
        // (ghost-превью следует за курсором до отпускания)
        if let Some(drag) = self.template_drag.as_mut() {
            if drag.update(logical) || drag.active {
                self.request_redraw();
            }
        }
        // FR-044 Р-5: hover-превью подсветки в main stage — живой отклик
        // по строкам панели «Как считается» без фиксации; при уходе
        // курсора превью гаснет (возвращается фиксированная подсветка)
        if let Some(stage) = self.main_stage.as_ref() {
            let viewport = self.viewport_logical();
            let rect = main_stage_rect(viewport);
            if point_in_rect([rect.x, rect.y, rect.w, rect.h], self.cursor) {
                let s = stage.scale.max(f32::EPSILON);
                let ctx = self.stage_frame_ctx(stage, &rect, s);
                let rel = [self.cursor[0] - rect.x, self.cursor[1] - rect.y];
                let preview = ctx.panel.and_then(|panel| {
                    if let Some(i) = panel.var_row_at(rel) {
                        Some(StageCalcFocus::for_var(&ctx.model, i))
                    } else {
                        panel
                            .formula_row_at(rel)
                            .map(|i| StageCalcFocus::for_formula(&ctx.model, i))
                    }
                });
                if self.stage_calc_hover != preview {
                    self.stage_calc_hover = preview;
                    self.request_redraw();
                }
            } else if self.stage_calc_hover.take().is_some() {
                self.request_redraw();
            }
        }
        // Ревизия FR-025: hover-раскрытие категорий свёрнутой полосы палитры
        // (hover-intent / grace; подсветка строки следует за курсором)
        if self.update_template_hover() {
            self.request_redraw();
        }
        // Драг внутри редактора — расширение выделения мышью (T7/T8)
        if self.editor_dragging && !self.space_pressed {
            let world = self.cursor_world();
            let zoom_px = self.zoom_px();
            if let (Some(session), Some(renderer)) = (self.editing.as_mut(), self.renderer.as_mut())
            {
                if let Some((origin, _, _)) =
                    session_area(&self.scene.canvas, session, self.settings.edges_avoid_nodes)
                {
                    let x = ((world[0] - origin[0]) * zoom_px) as i32;
                    let y = ((world[1] - origin[1]) * zoom_px) as i32;
                    session.drag(renderer.font_system_mut(), x, y);
                }
            }
            self.request_redraw();
        }
        if !self.space_pressed {
            // Рамка выделения (CR-001): тянется за курсором (перерисовка на
            // каждое движение — квад в оверлее); пан во время рамки —
            // Space недоступен (guard выше), средняя кнопка замораживает
            if self.select_rect.is_some() {
                let world = self.cursor_world();
                if let Some(rect) = self.select_rect.as_mut() {
                    rect.1 = world;
                }
                self.request_redraw();
            }
            // Ручной resize за правый нижний угол (T7): размеры клампятся
            // минимумом, spatial index обновляется инкрементально
            if let Some(index) = self.resizing {
                let world = self.cursor_world();
                if let Some(node) = self.scene.canvas.nodes.get_mut(index) {
                    node.width = (world[0] - node.x).max(MIN_NODE_WIDTH);
                    node.height = (world[1] - node.y).max(MIN_NODE_HEIGHT);
                    self.scene.spatial.update(index, node);
                    self.scene.mark_dirty();
                }
                self.request_redraw();
            } else if let Some(drag) = self.dragging.clone() {
                // Drag (T7/CR-001): каждая перемещаемая нода — в исходную
                // позицию + дельта курсора от захвата (ровно один сдвиг за
                // кадр; дети групп — в origins с старта, дубликатов нет).
                // FR-038: нода следует курсору СВОБОДНО (п.2 v2) — дельта
                // кадра клампится только live-collision (п.15, если включён);
                // grid/guides — предпросмотр + release-time, позиции не трогают
                let world = self.cursor_world();
                let delta = [world[0] - drag.grab_world[0], world[1] - drag.grab_world[1]];
                let snap_frame = self.compute_snap_frame(&drag, delta);
                let eff = snap_frame.as_ref().map_or(delta, |frame| frame.eff_delta);
                for (index, origin) in &drag.origins {
                    self.scene
                        .move_node(*index, origin[0] + eff[0], origin[1] + eff[1]);
                }
                // FR-012: зона втягивания — группа под центром первичной ноды
                let target = self.group_drop_target(&drag);
                if target != self.group_drop_target {
                    self.group_drop_target = target;
                }
                self.scene.mark_dirty();
                // FR-038 (п.2/8/9): предпросмотр — направляющие + ghost
                // snapped-позиции; без снапа слой гасится
                self.update_snap_preview(snap_frame.as_ref());
                self.request_redraw();
            } else if self.edge_drag.is_some() {
                // Резиновая линия (T8) следует за курсором — курсор уже
                // обновлён выше, нужна только перерисовка.
                // FR-050 Н2 (этап C): цель value-drag — пересчёт подсветки
                // якорей параметров (совместимость Н5), смена — перерисовка
                let param_drop = self.compute_param_drop();
                if param_drop != self.param_drop {
                    self.param_drop = param_drop;
                }
                self.request_redraw();
            } else if !self.panning() && !self.editor_dragging && self.editing.is_none() {
                // Hover (T8): порты ноды под курсором; перерисовка — только
                // при смене ноды, чтобы не крутить кадры на каждый пиксель.
                // Выборочный hit: над ребёнком группы hover уходит ему,
                // а не группе (порты групп не рисуются — cards.rs).
                // Модальность (практики UI): над открытым диалогом/поиском/
                // меню канваса hover-порты гасятся — сквозь оверлей
                // не подсвечивают
                // FR-PERF-C: throttle hit-test — не чаще 16мс и skip при
                // сдвиге <2px от последней проверки. Браузер шлёт 100+
                // pointermove/сек; hit-test нужен только при смене ноды под
                // курсором, не на каждый пиксель. Курсор (`self.cursor`)
                // уже обновлён выше — аффорданс (cursor-icon), drag/pan и
                // рамка выделения работают как прежде; пропускаем только
                // `ui_frame`+`selective_hit`+`edge_at`. Лаг смены hover —
                // ≤ 16мс (один пропущенный pointermove), визуально незаметен;
                // быстрое движение через ноды всегда проходит (moved_enough =
                // true при сдвиге ≥2px — каждая нода на канвасе крупнее).
                let now = Instant::now();
                let dx = logical[0] - self.hover_last_cursor[0];
                let dy = logical[1] - self.hover_last_cursor[1];
                // 2px² порог: меньше — дрожь, не пересчитываем hit-test.
                let moved_enough = dx * dx + dy * dy >= 4.0;
                // `map_or(true, …)` = `is_none_or(…)` — но стабилен с 1.41
                // (MSRV 1.80); true если проверки ещё не было, либо прошло
                // ≥16мс с последней.
                let time_ok = self
                    .hover_last_check
                    .map_or(true, |t| now.duration_since(t).as_millis() >= 16);
                if moved_enough || time_ok {
                    self.hover_last_check = Some(now);
                    self.hover_last_cursor = logical;
                    let world = self.cursor_world();
                    // FR-052 (U2): hover-порты гасятся, если клик в точке
                    // курсора перехватила экранная поверхность (pick по кадру
                    // реестра) — прежний список dialog/search/menu заменён
                    // правилом (модали/панели не подсвечивают мир под собой)
                    // FR-PERF-A: кадр — из кэша (`App::ui_frame`), не прямой
                    // вызов `build_frame` (он же идёт в `RedrawRequested` и
                    // `on_mouse_press`). Mut-заём `ui_frame` освобождается
                    // возвратом owned-кадра — последующие `&self.cursor` и
                    // `self.selective_hit(world)` компилируются без конфликтов.
                    let hovered = {
                        let frame = self.ui_frame();
                        if HitStack::absorbs(&frame, UiPoint::new(self.cursor[0], self.cursor[1])) {
                            None
                        } else {
                            self.selective_hit(world)
                        }
                    };
                    if hovered != self.hovered {
                        self.hovered = hovered;
                        self.request_redraw();
                    } else if self.palette_target().is_some()
                        || self.menu.is_some()
                        || self.search.is_open()
                        || self.settings_open
                    {
                        // Палитра/меню/поиск/настройки: hover-подсветка элементов
                        // следует за курсором
                        self.request_redraw();
                    }
                    // FR-042 (E2): BundleHover — ребро пучка веса ≥ 2 под
                    // курсором (hover-бамп агрегированной линии + курсор);
                    // вычисляется на кадр ввода, в кэш не пишется. Нода под
                    // курсором / открытый stage / drag — hover пучка нет.
                    let bundle_hover = if self.main_stage.is_none()
                        && self.edge_drag.is_none()
                        && self.settings.edge_aggregation
                        && hovered.is_none()
                    {
                        edge_at(&self.scene.canvas, world, self.settings.edges_avoid_nodes).filter(
                            |&i| {
                                self.scene
                                    .bundles
                                    .bundle_of_edge(i)
                                    .is_some_and(|b| b.weight >= 2)
                            },
                        )
                    } else {
                        None
                    };
                    if bundle_hover != self.bundle_hover {
                        self.bundle_hover = bundle_hover;
                        self.request_redraw();
                    }
                }
                // else: пропускаем тяжёлый hit-test на этом pointermove —
                // курсор уже обновлён, sync_cursor_icon в конце on_cursor_moved
                // отработает нормально; следующий move (≥2px или ≥16мс) поймает
                // смену hover.
            }
        }
        // Аффорданс курсора (Grabbing/Text/NwseResize/Arrow) — после всех
        // смен состояний этого события
        self.sync_cursor_icon();
    }

    pub(super) fn on_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        // PRD-0007 (X4, D10): колесо над телом диалога ревью автосвязи
        // скроллит список предложений (12+), а не панорамирует канвас
        if self.autolink_review.is_some() {
            let viewport = self.viewport_logical();
            let win = autolink_ui::dialog_rect(viewport);
            // Баннер отклонённых сдвигает тело вниз — тот же флаг, что у
            // rows_layout (review.counts). Без этого hit-test body_rect
            // не совпадал бы с раскладкой строк (баннер не учтён).
            let (_, rejected, _) = self
                .autolink_review
                .as_ref()
                .map(|r| r.counts())
                .unwrap_or((0, 0, 0));
            let body = autolink_ui::body_rect(win, rejected > 0);
            if point_in_rect(body, self.cursor) {
                let max = self
                    .autolink_review
                    .as_ref()
                    .map(|review| autolink_ui::rows_layout(review, win, 0.0).scroll_max)
                    .unwrap_or(0.0);
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * PAN_PX_PER_LINE,
                    MouseScrollDelta::PixelDelta(pos) => -pos.y as f32 / self.scale_factor(),
                };
                self.autolink_scroll = (self.autolink_scroll + dy).clamp(0.0, max);
                self.request_redraw();
                return;
            }
        }
        // FR-042 (E3, F-9): открытое main stage модально — колесо глушится
        // (пан/зум канваса в stage недоступны, инвариант 8)
        if self.main_stage.is_some() {
            // FR-059: исключение — колесо над колонками панели «Как
            // считается» прокручивает список колонки (кит список+скролл);
            // остальной stage — глушится (инвариант 8)
            self.stage_calc_wheel_scroll(delta);
            return;
        }
        // FR-059: колесо над окном списка карты проливаний прокручивает
        // список (кит список+скролл — замена капа «… ещё N»); знак —
        // как у списков: колесо от себя (y<0) увеличивает offset
        if self.flow_map_open {
            let lay = self.flow_map_layout();
            if flowmap_ui::flow_map_list_at(&lay, self.cursor) {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * PAN_PX_PER_LINE,
                    MouseScrollDelta::PixelDelta(pos) => -pos.y as f32 / self.scale_factor(),
                };
                self.flow_map_scroll_by(dy);
                return;
            }
        }
        // Ревизия FR-025: колесо над flyout свёрнутой палитры прокручивает
        // список шаблонов, а не панорамирует канвас (знак — как у списков:
        // колесо от себя, y<0, увеличивает scroll_top)
        if !self.template_panel.open {
            let viewport = self.viewport_logical();
            // FR-040 v2: геометрия по локализованным подписям — паритет
            // с рендером дока и предыдущим hit-test блоком.
            let categories = self.template_category_display_names();
            // FR-054: ширины чипов — измеренные (measurer на вызов, паттерн U3).
            let mut measurer = canvas_ui::measure::TextMeasurer::new();
            let mut fs = canvas_render::text::measure_font_system();
            let strip =
                template_ui::dock_strip_layout(&categories, viewport[1], &mut measurer, &mut fs);
            let fly = self.template_flyout_geometry(viewport, &strip);
            if let (Some(hover), Some(fly)) = (self.template_hover.as_mut(), fly) {
                if fly.max_scroll > 0 && point_in_rect(fly.rect, self.cursor) {
                    let lines = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y.round() as i32,
                        MouseScrollDelta::PixelDelta(pos) => (pos.y / 40.0).round() as i32,
                    };
                    hover.scroll_by(-lines, fly.max_scroll);
                    self.request_redraw();
                    return;
                }
            }
        }
        // FR-027: просмотрщик документации открыт — колесо над панелью
        // скроллит его контент (кламп; раскладка пересобирается, если
        // ширина панели изменилась — окно resize/миграция монитора)
        if self.docs.is_some() {
            let viewport = self.viewport_logical();
            let panel = docs_ui::viewer_rect(viewport);
            if point_in_rect(panel, self.cursor) {
                let scale = self.scale_factor();
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * PAN_PX_PER_LINE,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / scale,
                };
                let content = docs_ui::viewer_content_rect(panel);
                if let Some(viewer) = self.docs.as_mut() {
                    if (viewer.layout_width - content[2]).abs() > 0.5 {
                        let mut measurer = canvas_ui::measure::TextMeasurer::new();
                        let mut fs = canvas_render::text::measure_font_system();
                        viewer.layout =
                            docs_ui::layout_page(viewer.page, content[2], &mut measurer, &mut fs);
                        viewer.layout_width = content[2];
                        viewer
                            .scroll
                            .resize(viewer.layout.content_height, content[3]);
                    }
                    if viewer.scroll.wheel(dy) {
                        self.request_redraw();
                    }
                }
                return;
            }
        }
        // FR-059: колесо над контентом витрины кита прокручивает секции
        // (кит список+скролл; шапка фиксирована; знак — как у списков)
        if self.kit_gallery_open {
            let viewport = self.viewport_logical();
            let sections = crate::kit_ui::gallery_scroll_viewport(viewport);
            if crate::kit_ui::cursor_in(&sections, self.cursor) {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * PAN_PX_PER_LINE,
                    MouseScrollDelta::PixelDelta(pos) => -pos.y as f32 / self.scale_factor(),
                };
                // Контент синхронизируется раскладкой на кадре; здесь —
                // оценка по полной высоте последнего кадра не нужна:
                // scroll_by + clamp по фактической высоте раскладки ниже
                let mut scroll = self.kit_gallery_scroll.clone();
                let mut m = crate::kit_ui::new_measurer();
                let mut fs = canvas_render::text::measure_font_system();
                let palette = self.effective_palette().kit_palette();
                let lay = crate::kit_ui::gallery_layout(
                    viewport,
                    self.settings.language,
                    &scroll,
                    &palette,
                    &mut m,
                    &mut fs,
                );
                scroll.viewport_h = lay.sections_viewport.h;
                scroll.content_h = lay.content_h;
                scroll.scroll_by(dy);
                scroll.clamp();
                self.kit_gallery_scroll = scroll;
                self.request_redraw();
                return;
            }
        }
        // FR-070: колесо над демо-зоной админпанели прокручивает тело
        // секции (кит список+скролл; шапка/сайдбар фиксированы)
        if self.admin_open {
            let viewport = self.viewport_logical();
            let demo = crate::admin_ui::admin_demo_viewport(viewport);
            if crate::kit_ui::cursor_in(&demo, self.cursor) {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * PAN_PX_PER_LINE,
                    MouseScrollDelta::PixelDelta(pos) => -pos.y as f32 / self.scale_factor(),
                };
                let mut scroll = self.admin_scroll.clone();
                scroll.viewport_h = demo.h;
                scroll.content_h = self.admin_layout_current().demo_content_h;
                scroll.scroll_by(dy);
                scroll.clamp();
                self.admin_scroll = scroll;
                self.request_redraw();
                return;
            }
        }
        // Колесо над screen-space UI (панели/меню/палитра/миникарта) холст
        // не двигает — практика canvas-приложений (Miro/Figma)
        if self.cursor_over_screen_surface() {
            return;
        }
        // Тачпады шлют PixelDelta (физические px), колёсики мышей — LineDelta
        let scale = self.scale_factor();
        let (dx, dy) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (x * PAN_PX_PER_LINE, y * PAN_PX_PER_LINE),
            MouseScrollDelta::PixelDelta(pos) => (pos.x as f32 / scale, pos.y as f32 / scale),
        };
        let viewport = self.viewport_logical();
        if self.modifiers.control_key() {
            // Ctrl+колесо — зум к позиции курсора (SPEC §8)
            let factor = match delta {
                MouseScrollDelta::LineDelta(_, y) => ZOOM_STEP_PER_LINE.powf(y),
                MouseScrollDelta::PixelDelta(pos) => (pos.y as f32 * 0.005).exp(),
            };
            self.camera.zoom_at(factor, self.cursor, viewport);
        } else {
            // Двухпальцевый скролл тачпада — панорамирование (SPEC §8)
            self.camera.pan([dx, dy]);
        }
        self.request_redraw();
    }

    pub(super) fn on_pinch(&mut self, delta: f64) {
        // FR-042 (E3, F-9): пинч при открытом stage глушится
        if self.main_stage.is_some() {
            return;
        }
        // Пинч над screen-space UI — холст не зумит (как колесо выше)
        if self.cursor_over_screen_surface() {
            return;
        }
        let viewport = self.viewport_logical();
        let factor = (delta as f32).exp();
        self.camera.zoom_at(factor, self.cursor, viewport);
        self.request_redraw();
    }

    /// FR-050 Н2 (этап C): клик по меню выбора — пункт выполняет действие
    /// (геометрия отрисовки: сдвиг на заголовок); клик по заголовку/
    /// паддингу — отмена: ребро не создаётся (меню уже снято диспетчером
    /// НЕ было — берём сами, контракт как у контекстного меню T7).
    pub(super) fn click_choice_menu(&mut self) {
        if let Some(menu) = self.choice_menu.take() {
            if let Some(i) = self.choice_menu_item_at(self.cursor) {
                let action = menu.items[i].action.clone();
                self.apply_choice_action(action);
            }
            // Клик по заголовку/паддингу — отмена («мимо пункта»)
        }
        self.request_redraw();
    }
}
