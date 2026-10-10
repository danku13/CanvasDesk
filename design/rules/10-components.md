# 10 — Реестр компонентов кита v2 и слотов палитры

> Норматив аудита 2026-10-09 (волна «rules ↔ ui-kit»): закрывает пробелы
> B1/B2 аудита `design/rules-audit-2026-10.md` — контракты компонентов волн
> FR-058/FR-068/FR-UI-*, полный реестр слотов `KitPalette`, скролл-модель,
> требование витрины. Источники: `00-principles.md` (П4/П6/П7),
> `08-states.md` (ST1–ST3, ST6), `07-theming.md` (TH6), `docs/ui-kit.md` §7.

## K1. Реестр компонентов

Каждый интерактивный компонент кита подчиняется ST1–ST3 (матрица состояний,
слоты, клик-контракт) и ST6 (декорация, ловящая клик, — контрол). Цвет —
только слоты K2; метрики — только шкалы 03 (SPACING_*/RADIUS_*/высоты S3)
и кегли T2. Новое состояние = новый слот (П4), не формула.

| Компонент | Модуль (`canvas-ui/src/component/`) | Анатомия | Состояния (ST1) | Ключевые константы |
|---|---|---|---|---|
| Button (+IconButton) | `button.rs` | rect + измеренный label (ellipsis), вариант Primary/Secondary/Ghost/Danger | все ST1; Ghost-hover — рамка accent | BUTTON_HEIGHT 30, BUTTON_PAD_H 12, BUTTON_FONT_SIZE 13, ICON_BUTTON_SIZE 26 |
| Switch | `button.rs` | трек-пилюля + кноб, inset SPACING_SM | Normal/Hovered/Disabled; on → control_primary | SWITCH_W×H 36×20, KNOB_PAD 2, радиус RADIUS_PILL |
| Chip | `chip.rs` | Fit-чип: измеренный текст + CHIP_PAD_H | Normal/Selected/Hovered/Disabled | CHIP_HEIGHT 24, CHIP_FONT_SIZE 12 |
| ChipStrip | `chip.rs` | Row Fit-чипов + опциональный preset «All», Wrap-вариант | наследуются от Chip | зазор SPACING_S |
| TextField | `text_field.rs` | контейнер + text_area + caret; модель IN1–IN7 | Normal/Focused/Disabled (+Error — потребитель) | TEXT_FIELD_HEIGHT 30, PAD_H 8, MIN_W 80, FONT_SIZE 13 |
| Dropdown / Tooltip / Toast | `dropdown.rs` | меню от якоря (flip у края), тултип по задержке, тост TTL | пункты — WidgetState потребителя | DROPDOWN_GAP 4, TOOLTIP_DELAY 500, OFFSET (14,18), TOAST_TTL 3000 |
| Panel / Card | `panel.rs` | заливка/рамка/пад слотами, card = panel с хедером | хром без состояний (контейнер) | panel_style().pad = SPACING_LG |
| PanelHeader | `panel_header.rs` | шапка S/M/L + aligned-кнопки (close/theme/…) | кнопки — ST1 | PANEL_HEADER_H_S/M/L 30/38/44 (см. S3) |
| Modal | `modal.rs` | затемнение stage_dim + панель + focus_order | контейнер; контент — ST1 | радиус RADIUS_PANEL |
| List + Scroll | `list.rs` | list_rows (окно видимости, частичные строки) + scroll_bar | строки — WidgetState потребителя | LIST_ROW_H 26, LIST_ROW_GAP 6, SCROLLBAR_* (K3) |
| Row (RowGuides) | `row.rs` | строка на колоночных направляющих: маркер/label/value/unit/badge | RowStyle по KitState | guide_gap 6 (кит) / 8 (нода), зебра ≥ 4 |
| Table | `table.rs` | retained-оркестрация Row: общий замер → направляющие → окно → per-row стиль → бегунок | per-row KitState | TABLE_ZEBRA_RUN_MIN 4, paint_scrollbar |
| Banner | `banner.rs` | полоса Info/Warning/Error: рамка-слот, текст, action-кнопка | кнопка — ST1; текст disabled_text | BANNER_TINT_ALPHA 0.10 (alpha-tint kind-слота) |
| ChatBubble | `chat_bubble.rs` | сообщение Normal/Error/Success + tool-call строки | фон/рамка — alpha-tint kind-слота (0.08/0.38) | CHAT_LINE_H, пад SPACING_SM |
| RadioCard | `radio_card.rs` | индикатор 12×12 + label + desc; selected-ринг | Normal/Hovered/Disabled + selected | RADIO_LABEL_LINE_H ≈ 17, tint accent α0.10 |
| Crumbs | `crumbs.rs` | ряд чипов-крошек, переполнение take_while от правого края | last-visible — Selected | CRUMB_H, CRUMB_FONT_SIZE 11 |
| Footer | `footer.rs` | 3-кнопочный правый футер (Cancel/OK…) | кнопки — ST1 | BUTTON_WIDTH 100 (reference-default; измеренная ширина — footer_buttons_measured) |
| TwoColumn | `two_column.rs` | [sidebar | content]: фиксированная колонка + заполняющий контент (лево/право) | контейнеры | — |
| Icon | `icon.rs` | IconKind → Vec<PaintItem> (данные, не глифы) | tint слотом | слот 18×18; геометрия — прежние магические числа (унификация v2) |

Правила:

- **K1.1** Новый компонент кита обязан появиться в этой таблице в той же
  волне, что и в коде (урок FR-058/FR-068: 11 компонентов ушли в отрыв от
  правил — см. аудит §2-B1).
- **K1.2** Выбор компонента: список строк — List; табличные направляющие —
  Row/Table; выбор из взаимоисключающих — RadioCard; статусная полоса —
  Banner; сообщение агента — ChatBubble. Не плодить параллельные решения.
- **K1.3** Компонент не анимирует себя (M3): переходы состояний мгновенны;
  длительности M1 — на потребителе.

## K2. Реестр слотов KitPalette (19)

Единственный источник цвета для кита (П4). Заполняется мостом
`From<&ThemeColors>` (TH6). Полный список с семантикой:

| Слот | Семантика | Источник темы |
|---|---|---|
| panel_fill | заливка панели/модали | menu_fill |
| panel_border | рамка панели | palette_border |
| control_fill | secondary/ghost-контрол | palette_chip_fill |
| control_border | рамка контрола | DIALOG_BUTTON_BORDER |
| control_primary | primary-заливка | DIALOG_BUTTON_PRIMARY |
| control_danger | разрушающее действие | error |
| hover_fill | hover обычного контрола | control_hover_fill (derived, TH4) |
| primary_hover_fill | hover primary | control_primary_hover_fill (derived) |
| selected_fill | выбранная строка/чип | control_selected_fill (derived; = hover, I-1) |
| text | основной текст | body |
| text_title | заголовки | title |
| text_muted | приглушённый текст | DIALOG_TEXT_MUTED |
| disabled_text | текст disabled | control_disabled_text (derived) |
| accent | фокус-рамка, Ghost-hover, ринги | accent |
| control_success | статус «успех» (FR-070) | derived theme.rs |
| control_warning | статус «предупреждение» (FR-070) | derived theme.rs |
| stage_dim | затемнение фона под модалью | stage_dim |
| scrollbar_thumb | бегунок скроллбара (FR-070) | derived theme.rs |
| rule_color | линия `---` markdown (FR-070) | derived theme.rs |

Правила: **K2.1** новый слот = правка KitPalette + моста + этой таблицы
одновременно; **K2.2** alpha-варианты слотов — только через `tint()` с
константой из C2 (запись в таблице K1); rgb не модифицируется никогда.

## K3. Скролл-модель

- `ScrollState { offset, content_h, viewport_h }` — единственная модель
  прокрутки кита (`scroll_by`/`clamp`/`needs_scroll`/`max_offset`); свои
  «offset: f32 + ручной кламп» в поверхностях запрещены (аналог IN1).
- `list_rows(area, s, row_h, gap, count)` — чистая функция окна видимости;
  частичные строки на краях включаются.
- `scroll_bar(area, s, p)` — бегунок: ширина SCROLLBAR_WIDTH 4, минимум
  SCROLLBAR_KNOB_MIN 20, слот scrollbar_thumb; hit-зона — rect бегунка
  (П5). Отдельный kit-компонент скроллбара (трек+инерция) — бэклог W3
  аудита; до тех пор потребители рисуют бегунок только этой функцией.
- Row-window поверх `ScrollState` — санкционированный паттерн списков со
  СМЕШАННЫМ шагом строк (заголовки секций + карточки — единого stride нет,
  `list_rows` по константе неприменим, LAY-W9): состояние — первая видимая
  строка, px-оффсет `ScrollState` = сумма шагов строк выше окна. Каноническая
  реализация — `canvas-app/src/template_ui.rs::RowScroll` (панель шаблонов
  и flyout; промоция в kit — при третьем потребителе). Анти-паттерн
  «offset: f32 + ручной кламп» в px остаётся запрещён (LAY10).

## K4. Витрина (расширение ST5)

UI-консоль («?» → «UI-консоль») обязана показывать живой образец каждого
компонента K1 во всех состояниях ST1 (матрица × RU/EN × темы). Состав
секций на 2026-10-09: buttons, icon_buttons, chips, dropdown, toast,
tooltip, text_field, switch, card, list, icons, table, panel_header —
**не хватает** banner, chat_bubble, crumbs, footer, radio_card, two_column
(бэклог W2 аудита). Новый компонент без секции витрины — незакрытая волна.

**Витрина раскладок** (норматив `design/rules/11-layouts.md`, чек-лист
LAY11 п.10): секция витрины обновляется в той же волне, что и новая
раскладка/механизм LAY. Состав layout-секций на 2026-10-10 (код —
`crates/canvas-app/src/kit_ui.rs`, константы `SECTION_*`/`SECTION_LAYOUT_*`):

| Секция | Механизм 11-layouts.md |
|---|---|
| measured | LAY6 — измеренный текст (`Row::lay_out_measured`, F-13) |
| grow | LAY4 — flex-факторы (`Child::flexible`, grow 2:1) |
| wrap | LAY3 — политика `RowPolicy::Wrap` |
| grid | LAY2/LAY5 — `grid_cells` (равные явные колонки) |
| focus | LAY11 — Tab-кольцо витрины (FocusRing) |
| squeeze | LAY3 — политика `RowPolicy::SqueezeTail` |
| align | LAY2 — `MainAlign::SpaceBetween` / `Column` + распорка (LAY7.3) |
| component_row / component_panel | компонентный слой K1 (Row/Table/Panel) |
| layout_constrain | LAY2 — `constrain(min, max, desired)` (clamp) |
| layout_pad | LAY2 — `pad(slot, EdgeInsets)` из шкалы S1 |
| layout_stack | LAY2 — `stack(slot, size, HAlign, VAlign)` (центр/угол) |
| layout_gaps | LAY7 — шкала зазоров S1 (S/SM/MD/LG/XL с подписями) |
| layout_percent | LAY5 — сцена `SceneDim::Percent` + `Fill` (доли ширины) |
| layout_aspect | LAY5 — сцена aspect-ratio (плитки 16:9) |
| layout_sticky | LAY5 — сцена Sticky в прокручиваемом окне (Hidden+offset) |
| layout_hide_below | LAY8 п.3 — `DegradationPolicy::HideBelow` (порог 900×600) |

Правила витрины раскладок:

- **K4.1** Секции-демо строятся ТОЛЬКО примитивами `canvas_ui::layout` и
  сценой `SceneNode` (`lay_out_scene`); зазоры/паддинги — только шкалы S1
  (LAY7), текст — через `TextMeasurer`/`MeasuredItem` (LAY6), никаких
  молчаливых клампов (G5), `Custom` — только с обоснованием (G8).
- **K4.2** Геометрия демо и подписи секций — из единственной раскладки
  (`gallery_layout`): hit-слоты (при появлении интерактива) и отрисовка
  строятся из одних rect'ов (LAY1.2); скролл-сдвиг/фильтр полной
  видимости — общий механизм витрины.
- **K4.3** Сцена-секции проверяют маску `LayoutFeatures` движка перед
  расширенными политиками (LAY5.2); механизм LAY, не показанный секцией
  витрины, — незакрытая волна (аналог K1.1 для компонентов).
