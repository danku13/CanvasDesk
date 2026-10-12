# Worklog — CanvasDesk bug analysis session

---
Task ID: 1
Agent: main (Super Z)
Task: Разбор отчёта владельца по ручному тесту handtest.canvas (11 дефектов), docs/user-reporting, план доработок, CR/FR, Telegram-отчётность

Work Log:
- Клонирован danku13/CanvasDesk (PAT) → /home/z/my-project/canvasdesk
- Прочитан handtest.canvas: 10 пунктов в заметке «Косяки» + п.11 (тёмная тема) из сообщения = 11 дефектов
- 4 параллельных Explore-агента + 1 добор: все 11 локализованы до файл:строка
- 12 продуктовых/UX/UI вопросов заданы владельцу через AskUserQuestion; ответы получены (web-платформа; зона «?» только у расчётов + лупа на строку; входы видимы + маркер ƒ расчёта; подсказки по имени без $; ИИ-шаблоны «вообще не триггерить»; suppress до смены токена; control без тултипа; value-тултип «Нода.параметр = значение»; полный фикс звёздочки; полный набор шорткатов; корневой блендинг-фикс)
- Создано: docs/user-reporting/README.md + ur-001-handtest-canvas.md (11/11, матрица происхождения, Q&A); CR-019…CR-026, FR-100, FR-101; docs/plans/ur-001-handtest-rework-plan.md (4 волны, ~14-19 агент-дней)
- index-cr-fr.md +10 строк (восстановлена случайно подрезанная строка cr-017)
- worklog репо дополнен; конфликт ребейза с удалённой сессией ui-kit разрешён (обе записи сохранены)
- Коммит 47274e5 запушен в main

Stage Summary:
- Происхождение 11 дефектов: 6 «тихих» решений агента, 4 неполноты контрактов, 2 пересматриваемых согласованных решения (fr-050 Р-4, план FR-079 C1)
- Реализация не начата — план ждёт ревью владельца
- Открытый вопрос: двухсловные присваивания («Финальная корзина = …») невозможны грамматикой — кандидат в FR
- Telegram: план (msg 816), финальное саммари — отправлено

---
Task ID: LAY-VERIFY-e319cc1
Agent: main (Super Z)
Task: Верификация ревью LAY-W1..W12 (80af42b): судьба e319cc1, потерянная миграция W3a, слепая зона lay7-гейта, 8 P2

Work Log:
- main обновлён до 80af42b (ревью волн LAY-W1..W12 в design/layouts-w1-w12-review.md)
- e319cc1: invalid object + GitHub API 422; 10545b4/b517e0a/c4ae075 — тоже
- Цепочка: W3a=10545b4 (локально) → параллельная сессия отчиталась о переносе как e319cc1 → сессия доверилась (golden e37177a поверх, статусы аудита) → в origin миграция не попала никогда
- admin_ui.rs 32 y+= (17 литеральных + 15 смешанных); аудит «93→0» ложно
- lay7: вне скана 35 639 строк (88 310 всего, скан 52 671); rc=0 при нарушениях
- 8 P2 подтверждены спот-чеками (palette/ai_status coarse, off-scale константы, W11 маркеры, «Исключения» неполны, allowlist-лазейка, смешанные курсоры, 5 высот, draw≠hit)
- Коммит ee75ead запушен в main

Stage Summary:
- Ревью подтверждено: 2 P1 + 8 P2 реальны; предложение владельцу — LAY-W13/W14 агентами
- Коммиты: ee75ead (worklog верификации)
- Tokens: in≈82k/out≈11k/total≈93k

---
Task ID: LAY-KIT-REVIEW
Agent: Super Z (main)
Task: Анализ дефектов вёрстки Wave D v1 по скриншоту владельца

Work Log:
- Синхронизирован main (47979a4, Wave D v1 слит); инвентаризация скриншота кропами/пиксельными сканами
- Подтверждены: L1 (P1) обрезание хвоста сайдбара (PANEL_MAX 640 → окно 542px < 610px потребности, Тултип/Снекбар/Поповер недостижимы при любом окне), L2 (P1) клип подписей без эллипсиса (усечение по 36 символам вместо TextMeasurer), L3 (P2) заголовки контента вплотную к сайдбару, L4/L5 (P3)
- Сняты пиксально: Σ-строка выровнена, список ровный, «Обычно» — i18n-строка
- Системные причины: kit_ui.rs в allowlist lay7, G4 не проверяет текст, тест >=20 легализовал обрезание, PANEL_MAX не пересчитан (+тулбар 44px)
- Дизайн-док design/kit-demo-layout-review-2026-10.md (план F1–F5), worklog репо; коммит 32e337e запушен в main

Stage Summary:
- Дефекты — не регрессия LAY-W1..W12, а зона вне их юрисдикции (allowlist + ручной TO-BE-код вне скелета/измерителя)
- Следующий шаг: Wave D v1.1 (F1/F2 — P1) или LAY-W13/W14 по решению владельца

---
Task ID: MODAL-INVENTORY
Agent: Super Z (main)
Task: Инвентаризация всех поверхностей, работающих как модальное окно

Work Log:
- Источник: build_registry (app/ui_registry.rs:303-607) + VISUAL_ORDER + esc-лестница
- Классификация: 12 истинных модалей (Modals+Block): settings, stage, explain, migrate, canvas_manager, autolink, dialog, gallery, kit_gallery, admin, onboarding, ai_onboarding; 5 блокирующих popup (Block без затемнения): wheel, menu, choice_menu, help_menu, docs; 1 полумодаль: search (Block на Panels)
- Особенности зафиксированы: ai_onboarding backdrop НЕ закрывает; docs прямо названы «модальной поверхностью» в коде (app.rs:6014); explain поверх stage; autolink прячет explain

Stage Summary:
- 18 из ~30 поверхностей реестра блокирующие; список выведен в чат

---
Task ID: WAVE-M-PLAN
Agent: Super Z (main)
Task: План Wave M (адаптивные модали 80%) + правка владельца: mobile-style для мобильных разрешений

Work Log:
- Изучен репо danku13/ui-for-agents (стек-агностичная база знаний, формат Read when/Rules/Checklist/Anti-patterns)
- Извлечены правила: modals-and-overlays R1-R11 (size follows content, overflow scrolls inside, fixed header/actions; R5 anti-stack), touch-and-mobile R1-R10 (44×44 targets, thumb zone, no hover-only, bottom sheets), responsive-adaptive R1-R10 (named ranges, test extremes ~360px)
- Библиотека подключена как skill: /home/z/my-project/skills/ui-for-agents/ (SKILL.md, 7 разделов)
- Находка: R5 «не стыкуй модали» vs наш стек (canvas_manager→gallery, dialog поверх) — зафиксировать в аудите как осознанное отклонение
- План Wave M обновлён: 3 тира раскладки (compact ≤767 mobile-style / medium 768-1279 / expanded ≥1280), мобильные вьюпорты в приёмке; план выведен в чат на одобрение

Stage Summary:
- План v2 готов, жду одобрения; после одобрения — M0 (аудит+контракт)

---
Task ID: WAVE-M-ISSUES
Agent: Super Z (main)
Task: Создание эпика Wave M + 6 sub-issues в GitHub (разбивка под 4 параллельных сессии) + Project v2

Work Log:
- Владелец одобрил: «создавай issue и sub-issues с учётом разделения на 3–4 параллельных сессии»
- Сверены правила skills ui-for-agents: M-O R1–R11, T-M R1–R10, R-A R1–R12 (заголовки в телах issues)
- Проверены токены: PAT (issues) + ghp_ (Projects, login danku13); проект CanvasDesk PVT_kwHOAhnxHc4Bi3lO
- Скрипт scripts/wave_m_issues.py: создан эпик #55 + 6 sub-issues #49–54, нативная линковка sub_issues API (6/6), все 7 добавлены в Project v2
- Структура сессий: A=#49 M0 → #50 M1 → #51 M2 (фундамент); B=#52 M3-a простые; C=#53 M3-b сложные (+F1/F2); D=#54 M3-c специальные; M4 приёмка в эпике, M5 — вне серии

Stage Summary:
- Эпик: https://github.com/danku13/CanvasDesk/issues/55
- Тела issues самодостаточны для параллельных сессий: инвентарь, контракт 80% + тиры, правила AGENTS.md, гейты, коммит-футеры scope=wave-m-*
- Telegram: старт (msg 1056) + финальное саммари отправлены

---
Task ID: WAVE-M-SESSION-A
Agent: Super Z (main)
Task: Сессия A Wave M — фундамент: #49 M0 (аудит+контракт) → #50 M1 (kit modal_rect_flex) → #51 M2 (флаг+тумблер+G4)

Work Log:
- M0: Explore-аудит 12 модалей (три семейства окон: kit::modal ×5, panel_rect ×2, собственные формулы ×5; y+= только ai_onboarding ×4, autolink ×3, kit_gallery sy+= ×2); дизайн-док design/modals-audit-2026-10.md (§1–7: контракт, тиры, отклонения M-O R5/R4, карта конфликтов B/C/D); коммит d22924b
- M1: TDD canvas-ui/component/modal.rs — LayoutTier + layout_tier (≤767/768–1279/≥1280, границы = SETTINGS_BP_*) + modal_rect_flex(slot,margin,ratio,min,max) + MODAL_RATIO_DEFAULT 0.8; 8 юнит-тестов; реэкспорт kit.rs; коммит 49be540
- Ремонт красного main (чужие коммиты c83b888/cc4ea81 без гейтов): golden tobe_demo_layout_structure — INT добавила 2 секции (26→28), потребность сайдбара 650px > окно 538px, Feedback-группа за клампом → пин на документированное состояние + эскалация F1 → #53; fmt --all нормализация INT-файлов; #[allow(dead_code)] INT-2 API впрок (action_registry/command_actions)
- M2: settings.modals_adaptive (serde default false) + SettingsRow::ModalsAdaptive (58, таб «Общие») + тумблер RU/EN + flip/paint в overlays.rs; G4 lint_modals_adaptive: 8 headless-состояний × {1280×800, 800×560} × RU/EN + Compact-край 375×667 (5 модалей: границы+backdrop; глобальный overlap на 375 off — pre-existing empty×template_strip полотна); пины 2 тестов settings обновлены (3 строки таба)
- Все гейты зелёные ×2 (fmt/check/test/clippy/wasm/lay7/doc_lint); пуш cc4ea81..cd4b705; issues #49/#50/#51 закрыты с отчётами

Stage Summary:
- Фундамент Wave M в main: примитив + флаг + тумблер + lint-каркас; B/C/D стартуют с cd4b705
- Открытое: полотно на 375 даёт overlap empty-card × template-strip (вне Wave M); kit_gallery/admin 375-ветка линта — в M3-b
- Tokens: in≈420k, out≈52k, total≈472k, model=GLM-4.6, scope=wave-m-session-a

---
Task ID: WAVE-M-SESSION-B (issue #52)
Agent: Super Z (main)
Task: Сессия B Wave M — M3-a: адаптив простых модалей (dialog, migrate, canvas_manager, gallery)

Work Log:
- TDD: +24 теста; реализация 4 адаптивных веток (modal_rect_flex + тиры); скролл-threading visible_rows
- Проводка draw==hit: overlays/ui_registry/input/app — ветки modals_adaptive; G4 +migrate/+canvas_manager (10 состояний × 2 вьюпорта × RU/EN + 375-край)
- Гейты зелёные ×2 (fmt/check/test 96/clippy/wasm/lay7/doc_lint); rebase поверх Wave T docs (e2a5617/f91317f)
- Инцидент: отказ инструментов окружения в середине (5 подряд) → перезапуск сессии владельцем → продолжение без потерь
- Коммиты: 1dc509e (реализация) + b754400/ce8f45b (worklog) запушены; issue #52 закрыт с отчётом (комментарий 6107792403)

Stage Summary:
- M3-a в main; отклонения зафиксированы в #52 (gallery medium=1 колонка по аудиту; kit-чипы 24px — отдельная волна; migrate compact 9 строк)
- Открытое: Session C (#53) старт владельцем; M4 приёмка по чек-листу эпика #55
- Tokens: in≈560k, out≈68k, total≈628k, model=GLM-4.6, scope=wave-m-m3a

---
Task ID: WAVE-M-STATUS-AUDIT
Agent: Super Z (main)
Task: Статус-аудит эпика Wave M #55 при условии «готовы все sub-issues»; синхронизация Project v2; TG-отчётность

Work Log:
- GitHub API: 6/6 sub-issues #49–54 closed as completed (последняя #54 в 12:33 UTC); эпик #55 оставался open
- Карта коммитов: #49→d22924b, #50→49be540, #51→cd4b705, #52→1dc509e, #53→0b6d52e, #54→77fa204 — все предки HEAD 002dd0c; после #54 ядро Wave M не трогали
- Структурный спот-чек main: modal_rect_flex, LayoutTier, modals_adaptive, ModalsAdaptive, lint_modals_adaptive + 8 артефактов M3-b/c — на месте
- Project v2: эпик #55 Todo → Test; статус-комментарий issuecomment-6109384357
- Репо-лог: запись WAVE-M-STATUS-AUDIT, коммит f396e7b запушен в main
- Telegram: план (msg 1079), этапы 1–2 (msg 1080), финальное саммари — отправлены

Stage Summary:
- Wave M реализован полностью; эпик #55 оставлен открытым до M4-приёмки владельца, готов к закрытию как completed
- M5 (демонтаж старых веток) — вне серии, по отдельному запросу
- Tokens: in≈60k, out≈15k, total≈75k, model=GLM-4.7, scope=wave-m-status-audit

---
Task ID: ISSUE-STRUCTURE-64-66
Agent: Super Z (main)
Task: Структурирование issues: M4/M5 → sub-issues #55; #35-41 + #59 → новый эпик приёмки; Project v2

Work Log:
- Созданы #64 (M4 приёмка владельцем, матрица 12×2×6×2) и #65 (M5 демонтаж, вне серии, не блокирует закрытие) → нативно sub-issues #55 (8/8); тело #55 обновлено
- Создан эпик #66 «Приёмка — ручные проверки владельцем» → sub-issues #35-41 + #59 (8/8, parent: null устранён); тело: группировка, порядок, кросс-ссылка M4↔#59
- Project v2: #64/#65 авто-добавлены, #66 добавлен, 11 items → Todo
- Репо-лог: коммит 4fb2b96 запушен в main
- Telegram: план (1082), этап 1 (1083), этапы 2-3 (1084), финал — отправлены

Stage Summary:
- Сирот среди открытых приёмок нет; Wave M: M0-M3 + M4 + M5 полностью видимы в #55
- Tokens: in≈70k, out≈18k, total≈88k, model=GLM-4.7, scope=issue-structure

---
Task ID: M4-BIG-SCREENS-QA
Agent: Super Z (main)
Task: Ответ на вопрос владельца о покрытии 2K/4K/MacBook вьюпортами приёмки #55/#64

Work Log:
- Сверены по коду все max-клампы modal_rect_flex: settings 880×640, kit_gallery 900×720, admin 1120×720, autolink 760×760, dialog 440, migrate 480, canvas_manager 640, gallery 660, ai_onboarding=контент; stage/explain max=eff
- Вывод: тиры непрерывны (expanded ≥1280 без границы) — 2K/4K/MacBook покрыты по построению; лог. вьюпорты MacBook (1440×900/1512×982/1728×1117) и 4K@150-200% попадают в expanded; 1440×900 = тестовая точка
- Дыра: в матрице #64 не было точки >1440 → PATCH #64: блок «Большие экраны» (1920 все 12; 2560×1440 и 1512×982 выборочно; 4K@150-200% смоук; 4K@100% → плотность/DPR вне скоупа, CR-028)
- Скрипт scripts/m4_big_screens_update.py; репо-контент не менялся (без коммита)

Stage Summary:
- Вопрос владельца закрыт: перекрытие полное по построению + добавлены контрольные точки больших экранов в #64
- TG: разбор отправлен

---
Task ID: M4-MACBOOK-MAPPING
Agent: Super Z (main)
Task: Сопоставление реальных physical-разрешений MacBook (таблица владельца) с логическими вьюпортами приёмки

Work Log:
- Владелец прислал physical-таблицу: Air M1 2560×1600, Air M2–M4 2560×1664, MBP 14" 3024×1964, MBP 16" 3456×2234
- Спот-чек кода: layout_tier Compact ≤767 / Medium 768–1279.9 / Expanded ≥1280 без верхней границы (тест фиксирует 1920→Expanded); modal_rect_flex клампы подтверждены
- Маппинг (дефолтный Retina-скейл 2×): Air M1 → лог. 1280×800 (= LAY8.1 точь-в-точь), Air M2–M4 → 1280×832, MBP 14" → 1512×982, MBP 16" → 1728×1117; максимум «больше пространства» — 1680×1050 / 1728×1117 / 1920×1200; все лог. ширины ∈ [1280, 1920] ⊂ Expanded
- 4K: @2× → лог. 1920×1080 (= CR-014), @150% → 2560×1440 (точка #64), @100% → 3840×2160 (Expanded по построению, плотность = CR-028 вне скоупа)
- Комментарий-шпаргалка в #64 (issue_comment id 6110158729, скрипт scripts/m4_macbook_mapping_comment.py, с маркером-антидублем); чек-лист #64 не менялся — новые точки не нужны
- Репо-контент не менялся (без коммита)

Stage Summary:
- Вердикт: физические разрешения MacBook напрямую не тестируются — приложение видит логические пиксели; все MacBook M1–M4 попадают в непрерывный Expanded-тир, покрытие полное существующими точками (LAY8.1 1280×800 + 1512×982 из «Больших экранов»)
- TG: план (msg 1098) + финальное саммари отправлены

---
Task ID: KIT-BUTTON-POLISH
Agent: Super Z (main)
Task: Полировка ui-kit «кнопка»: миграция потребителей на kit::Button (onboarding TODO), хит-зоны MIN_TOUCH_TARGET (LAY8.3), размерные кегли подписи

Work Log:
- canvas-ui: ControlSize::font_size() (11/13/14/15; Sm ≡ BUTTON_FONT_SIZE) + pad_h() (8/12/14/16); MIN_TOUCH_TARGET=44 + touch_expand/touch_hit (LAY8.3, единый источник с FR-097); ButtonProps{size, touch_hit} + hit_test override (hit-only зона 44)
- canvas-app: touch_targets.rs → делегация математики киту; onboarding Skip → измеренная kit-кнопка (снятый TODO): skip_rect (текст+2·BUTTON_PAD_H·k × BUTTON_HEIGHT·k), button_at(_touch)/button_rect(_touch) с замерщиком, draw==hit (overlays.rs + input.rs); фикс 64×22 и кегль 11 удалены (риск клипа RU «Пропустить» при 64px)
- Инцидент: пуш отклонён — параллельная сессия запушила 10ed162 (их полировка той же кнопки: sized-layout #67 V1, state-layer M3, focus-ring, set_disabled, ButtonProps::new); ребейз с ручным слиянием: их frozen-контракты (эталоны V3) сохранены 1:1, мои кегли — через font_size() + Component-путь, дубли sized-функций/тестов устранены
- Гейты на слитой версии: fmt/check/test --workspace (3432 passed, 0 failed)/clippy/wasm_gate --check/lay7/doc_lint — зелёные
- Коммит 1862d04 запушен в main (поверх 0de9f49)

Stage Summary:
- Кнопка кита: 7 вариантов × 4 размера, кегль по шкале (Component), LAY8.3 в ките, Skip онбординга измеренная
- Вопрос владельцу: обновлять ли онбординг-скриншоты/доки (шифт Skip 64×22 → ~80×30 при k=1)
- TG: план (1111), этап 1 (1112), этап 2 + финал — отправлены

---
Task ID: UI-LAYOUT-AUDIT-SCREENSHOTS
Agent: Super Z (main)
Task: Расследование проблем вёрстки по скриншотам WASM/Chrome (папка Bugs_12-10-26), гипотезы и план доработок

Work Log:
- Скачаны 9 скриншотов (gdown) в /home/z/my-project/screenshots/Bugs_12-10-26; просмотрены все
- Зафиксированы дефекты: чип «Нажатие/Выбрано» двойной текст (витрина), каретка во всю высоту поля (агент-панель), title×Skip коллизия (онбординг шаг 1), «★» tofu (Об авторе), пустые икон-кнопки, обрезанный хинт «ключ пустой — введит» (настройки BYOK), поля модель-строк впритык к кромке ряда, «RU»+аватар витрины поверх хедера AI-агента, разнобой вертикальных отступов
- Корень 1 (текст): пайплайн текста top-anchored — screen_text_area (canvas-render/text.rs:456) ставит top=origin.y; label_center центрирует ТОЛЬКО горизонтально; каждый вызов крутит магические инсеты (+1/+2/+4/+5/+8) — «выровнен не везде»
- Корень 2 (каретка): 4 несогласованные геометрии — overlays.rs:52 (h-4, w1.5), overlays.rs:1654 (h-8, w1.5), kit_ui.rs:4844 (h-8, w1.0), agent_panel.rs:1076 (h-4, w1.5); caret = text_area.h (30) - inset ≈ 22-26px при line-height текста 12-13px×1.3 ≈ 16-17px; text_area у кита — вся высота поля (text_field.rs:597-602, PAD только горизонтальный)
- Корень 3 (чип): раскладка слота 2 = KIT_STATE_SELECTED («Выбрано», kit_ui.rs:1552), статичная отрисовка = Selected (overlays.rs:1558), демо-оверлей = STATE_LABELS[2] = PRESSED «Нажатие» (kit_ui.rs:4763-4766) → два разных текста на одном rect при TO-BE; тултип тоже STATE_LABELS (kit_ui.rs:5128)
- Корень 4 (онбординг): title рисуется однострочно на всю ширину текстовой колонки без ellipsis/клампа (overlays.rs:4441-4452), Skip — правый верх КАРТОЧКИ (skip_rect) → наложение на длинном RU-заголовке; точки центрируются по колонке (progress_dots), title/body слева — визуальный разнобой; на финальном шаге «Назад» висит в footer_h=46 зоне с 30px кнопкой → дыра над ним
- Корень 5 (настройки): badge BYOK фиксированные 110px (overlays.rs:6602) режут «ключ пустой — введите ключ» и могут уезжать под кнопку; поле TextInput field_y=22+30=52 = впритык к нижней кромке ряда (MODAL_ROW_HEIGHT=52, без нижнего пада)
- Корень 6 (глифы): «★» U+2605 отсутствует во встроенном шрифте → tofu (i18n.rs:1532); пустые икон-кнопки — вероятный fallback на пустой глиф/отсутствие в SVG-атласе (проверка в плане)
- Репо-контент не менялся (расследование, без коммита)

Stage Summary:
- Системный вывод: волны полировки добавляли компоненты (layout-контракты кита), но вертикальная типографика осталась «ручным трудом» на сотне call-site'ов; нужен TextMetrics-контракт (v-center + line-height) в рендер-пайплайне, caret-канон в ките, единый источник label'ов чипов
- План из 5 волн сформулирован и отправлен в TG + чат
