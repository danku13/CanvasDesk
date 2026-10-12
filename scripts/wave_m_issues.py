#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Wave M: создание эпика + 6 sub-issues в danku13/CanvasDesk + добавление в Project v2."""
import os
import json, sys, urllib.request

PAT = os.environ.get("GITHUB_PAT", "")  # секрет вынесен в окружение (push protection)
PROJ_TOKEN = os.environ.get("GITHUB_PROJECT_TOKEN", "")  # секрет вынесен в окружение (push protection)
REPO = "danku13/CanvasDesk"
PROJECT_ID = "PVT_kwHOAhnxHc4Bi3lO"
API = f"https://api.github.com"

def rest(method, path, body=None, token=PAT):
    req = urllib.request.Request(f"{API}{path}", method=method)
    req.add_header("Authorization", f"Bearer {token}")
    req.add_header("Accept", "application/vnd.github+json")
    data = json.dumps(body).encode() if body is not None else None
    if data: req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, data) as r:
            return r.status, json.loads(r.read() or b"{}")
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read() or b"{}")

def gql(query, variables=None):
    body = {"query": query}
    if variables: body["variables"] = variables
    req = urllib.request.Request(f"{API}/graphql", method="POST")
    req.add_header("Authorization", f"token {PROJ_TOKEN}")
    req.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(req, json.dumps(body).encode()) as r:
        return json.loads(r.read())

# ---------- Общие блоки текста ----------
COMMON_RULES = """
## Общие правила для сессии (обязательны)
- Работай по `AGENTS.md`: TDD; UI только через токены kit (`canvas_core::tokens`); русскоязычные conventional commits; без `unwrap`/`expect` в проде; `git config core.filemode false`.
- Коммит-футер с токен-бухгалтерией: `Tokens: in≈<N>, out≈<N>, total≈<N>, model=<модель>, scope=wave-m-<part>`.
- Гейты перед пушем: `source scripts/cargo-env.sh && cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings && ./scripts/wasm_gate.sh --check` + lay7-гейт + G4 (`ui_layout_lint`).
- Правила вёрстки — skill [ui-for-agents](https://github.com/danku13/ui-for-agents): `20-components/modals-and-overlays.md` (M-O), `30-ux-patterns/touch-and-mobile.md` (T-M), `40-quality/responsive-adaptive.md` (R-A).
- Перед стартом: `git fetch origin && git merge --ff-only origin/main` (если ff невозможен — согласовать ребейз), прочитать `AGENTS.md`, `design/modals-audit-2026-10.md` (артефакт M0), `design/kit-demo-layout-review-2026-10.md`.
- По завершении: запись в `worklog.md` репо + комментарий в этот issue с итогами и логом гейтов.
"""

CONTRACT = """
## Контракт адаптивной модали (общий для всех M3-*)
- Флаг OFF → текущий рендер-путь не изменяется ни символом (весь новый код — в ветке `if settings.modals_adaptive`).
- Флаг ON → размер окна: `kit::modal_rect_flex` (примитив из M1); формула `rect = clamp(0.8 × (viewport − margin), min, max)`.
- Тиры (R-A R2 — именованные диапазоны): **compact ≤767** (вкл. 375) — mobile-style: одноколоночная вёрстка, скролл внутри (M-O R7), действия в нижней thumb-зоне (T-M R2), таргеты ≥44×44 (T-M R1), никакого hover-only (T-M R3); **medium 768–1279** — 80% окна, min-габариты; **expanded ≥1280** — 80% окна, max-ограничения.
- Ноль `y +=`: только Column/Row-скелет + TextMeasurer; отступы/шрифты — токены kit.
- Текст не обрезается молча: эллипсис по измеренной ширине (TextMeasurer), перенос или скролл.
- esc-лестница, VISUAL_ORDER, CapturePolicy, стек модалей — без изменений.
- Вьюпорты проверки: 1280×800, 800×560 (LAY8.1), 375×667 (CR-014) — R-A R6 «тестируй края».
"""

INVENTORY = """## Инвентарь — 12 истинных модалей (слой Modals + CapturePolicy::Block)
Источник: `crates/canvas-app/src/app/ui_registry.rs:303–607` (build_registry).
`settings`:372, `stage`:435, `explain`:464, `migrate`:482, `canvas_manager`:492, `autolink`:512, `dialog`:518, `gallery`:524, `kit_gallery`:533, `admin_panel`:541, `onboarding`:547, `ai_onboarding`:561.

Контракты взаимодействия сохраняем как есть (осознанные отклонения от ui-for-agents M-O R5 задокументируются в M0): `explain` поверх `stage`; `autolink` прячет `explain`; `ai_onboarding` — backdrop НЕ закрывает; стек canvas_manager→gallery→kit_gallery.

## Контракт раскладки v2
| Тир | Диапазон | Поведение |
|---|---|---|
| compact | ≤767 (вкл. 375) | mobile-style: одноколоночная вёрстка, fullscreen/bottom-sheet, touch-таргеты ≥44×44, thumb zone, без hover-only |
| medium | 768–1279 | 80% окна, min-габариты контракта, скролл внутри при нехватке |
| expanded | ≥1280 | 80% окна, max-ограничения, канонические пропорции |

Формула: `rect = clamp(0.8 × (viewport − margin), min, max)` — kit-примитив `modal_rect_flex` (M1).
Канонические вьюпорты приёмки: **1280×800 / 1024×640 / 800×560** (LAY8.1) + **375 / 768 / 1024 / 1280 / 1440 / 1920** (CR-014).
"""

SPLIT = """## Разбивка по сессиям (4 параллельных)
| Сессия | Sub-issues | Зависимости |
|---|---|---|
| **A — фундамент** (последовательно) | M0 → M1 → M2 | нет, стартует первой |
| **B — простые модали** | M3-a | merge A в main |
| **C — сложные модали** | M3-b | merge A в main |
| **D — специальные модали** | M3-c | merge A в main |

Карта конфликтов: A трогает `canvas-ui` + `settings.rs` + `settings_ui.rs` + `ui_layout_lint.rs`; B/C/D трогают только свои модальные файлы — пересечение минимально, но C стартует после A (settings_ui.rs).

## M4 — приёмка владельцем (после B/C/D)
Чек-лист: 12 модалей × флаг on/off × {1280×800, 1024×640, 800×560, 375, 768, 1440} × RU/EN.
## M5 — демонтаж старых версий
Отдельная волна по отдельному запросу владельца (удаление старых веток рендера, дефолт флага → true, чистка флага). В эту серию issues НЕ входит.
"""

# ---------- Тела sub-issues ----------
B_M0 = """Часть эпика Wave M. **Сессия A, шаг 1 из 3.** Без кода: только аудит + контракт. Это фундамент для партий B/C/D.

## Задачи
- [ ] Прочитать 3 файла skill ui-for-agents (ссылки ниже), выписать применимые правила в док.
- [ ] Аудит 12 модалей (инвентарь в эпике): для каждой — текущий размер/min-max, внутренний скелет (Column/Row vs `y +=`), поведение на 1280×800 / 800×560 / 375×667, что сломается при 80%.
- [ ] Осознанные отклонения от best practices — задокументировать, НЕ чинить в Wave M: стек canvas_manager→gallery→kit_gallery и dialog поверх (vs M-O R5 «одна модаль»); ai_onboarding — backdrop не закрывает (явное решение, сохраняем); explain поверх stage; autolink прячет explain.
- [ ] Дизайн-контракт: формула 80%, тиры compact ≤767 / medium 768–1279 / expanded ≥1280, min/max-таблица на каждую модаль, тир→скелет соответствие.
- [ ] Карта конфликтов мержа для параллельных сессий B/C/D (кто какие файлы трогает).

## DoD
- `design/modals-audit-2026-10.md` закоммичен (`docs(design): …`, `scope=wave-m-m0`).
- В чат + комментарий в эпик: резюме аудита, топ-риски по каждой партии.
- Для док-коммита гейты кода не нужны, но fmt/check прогнать для чистоты ветки.
""" + COMMON_RULES

B_M1 = """Часть эпика Wave M. **Сессия A, шаг 2 из 3.** Зависимость: M0 (контракт). TDD: тесты до реализации.

## Задачи
- [ ] В `crates/canvas-ui` (kit-примитивы) добавить:
  - `LayoutTier { Compact, Medium, Expanded }` + `layout_tier(width)`: ≤767 / 768–1279 / ≥1280;
  - `modal_rect_flex(viewport, margin, ratio=0.8, min, max) -> Rect`: формула `clamp(0.8 × (viewport − margin), min, max)`, центрирование; в **Compact** возвращает fullscreen-rect (full-bleed) — ветвление тиров инкапсулировано в примитиве;
  - сигнатуры согласовать с существующими типами kit, внешние зависимости не вводить.
- [ ] Юнит-тесты (сначала красные): границы 375×667 / 767/768 / 1279/1280 / 1920; защита min>max; margin; ratio; Compact full-bleed.
- [ ] Golden-тесты живы: `tobe_demo_layout_structure` (`kit_ui.rs:5560+`) не трогаем.

## DoD
- Гейты полные. Коммиты: `test(...)` → `feat(...)`, `scope=wave-m-m1`.
""" + COMMON_RULES

B_M2 = """Часть эпика Wave M. **Сессия A, шаг 3 из 3.** Зависимость: M1. После мержа — старт партий B/C/D.

## Задачи
- [ ] `crates/canvas-core/src/settings.rs`: поле `pub modals_adaptive: bool` с `#[serde(default)]` (default **false**). Существующий persistence (натив + web localStorage) подхватит поле автоматически, миграции не нужны.
- [ ] `settings_ui.rs`: тумблер «Адаптивные модали (β)» в подходящей секции; `i18n.rs` RU/EN («Adaptive modals (β)»); поясняющая подпись. Переключение применяется на лету, без перезапуска.
- [ ] G4 (`crates/canvas-app/src/app/ui_layout_lint.rs`): новое состояние `lint_modals_adaptive` — при флаге ON рендер каждой из 12 модалей × вьюпорты {1280×800, 800×560, 375×667} × RU/EN: хиты реестра, отсутствие паник, окно в пределах вьюпорта (по аналогии с `lint_kit_demo_tobe`, :265).
- [ ] Тесты: serde default false; round-trip сериализации; тумблер в хитах настроек.

## DoD
- Гейты полные. Коммиты `scope=wave-m-m2`.
""" + COMMON_RULES

B_M3A = """Часть эпика Wave M. **Сессия B.** Старт — только после мержа Сессии A (M1+M2) в main.
Партия «простых»: `dialog` (:518), `migrate` (:482), `canvas_manager` (:492), `gallery` (:524) — одна колонка контента, минимум вложенных панелей.

## Задачи (на каждую модаль)
- [ ] Флаг ON: окно = `kit::modal_rect_flex`; скелет по тирам; compact — одноколоночный mobile-style.
- [ ] `dialog`: кнопки подтверждения — нижняя зона, таргеты ≥44×44.
- [ ] `migrate`: прогресс/лог — скролл внутри (M-O R7).
- [ ] `canvas_manager`: список холстов — скролл, действия в thumb-зоне; medium/expanded — текущая плотность.
- [ ] `gallery`: сетка — compact 1 колонка, medium 2, expanded — текущая.
- [ ] Тесты (TDD): хиты по кадрам в 3 вьюпортах, отсутствие `y +=` в новых ветках, текст без молчаливых обрезаний.
""" + CONTRACT + COMMON_RULES

B_M3B = """Часть эпика Wave M. **Сессия C.** Старт — после мержа Сессии A. Самая тяжёлая партия.
Модали: `settings` (:372), `kit_gallery` (:533), `admin_panel` (:541).

## Задачи
- [ ] `settings`: текущие брейкпоинты 1280/768 (окно 560×400…880×640, `settings_ui.rs`) привести к тирам M; compact — табы → мобильная навигация (верхние сегменты или список→подэкран), скролл внутри таба; тумблер M2 «Адаптивные модали (β)» не сломать.
- [ ] `kit_gallery` — заодно фиксы Wave D v1 (`design/kit-demo-layout-review-2026-10.md`):
  - **F1 (P1)**: сайдбар режет хвост 26→23 секций (PANEL_MAX 640 → окно 542px < 610px потребности; Тултип/Снекбар/Поповер недостижимы): в adaptive — сайдбар со скроллом в medium/expanded; в compact — группы в мобильную навигацию (табы/аккордеон); PANEL_MAX пересчитать (+тулбар 44px).
  - **F2 (P1)**: усечение по 36 символам (`kit_ui.rs:3287–3292`) → TextMeasurer + эллипсис по измеренной ширине.
- [ ] `admin_panel` (`admin_ui.rs`, известны 32 `y +=`): минимальный объём Wave M — окно 80% + тир-скелет обвязки; если глубокая чистка `y +=` раздувает диф — вынести в follow-up issue и отчитаться в комментарии.
- [ ] Тесты (TDD): как в общем контракте.
""" + CONTRACT + COMMON_RULES

B_M3C = """Часть эпика Wave M. **Сессия D.** Старт — после мержа Сессии A.
Модали: `stage` (:435), `explain` (:464), `autolink` (:512), `onboarding` (:547), `ai_onboarding` (:561).

## Контракты взаимодействия НЕИЗМЕННЫ
- `explain` поверх `stage`; `autolink` прячет `explain`; `ai_onboarding` — backdrop НЕ закрывает; esc-лестница без изменений.
- Арт онбординга (`onboarding_art.rs`) НЕ трогать (задача «онбординг ≥2x» — отдельная волна, не Wave M).

## Задачи (на каждую модаль)
- [ ] Флаг ON: окно = `kit::modal_rect_flex`; скелет по тирам.
- [ ] `stage`: сцена-превью на всю ширину в compact; действия — нижняя зона.
- [ ] `explain`: компактная колонка, скролл внутри.
- [ ] `autolink`: поля ввода ≥44×44 в compact (T-M R1/R5).
- [ ] `onboarding` / `ai_onboarding`: compact — bottom-sheet поведение (T-M R7), шаги без горизонтального скролла.
- [ ] Тесты (TDD): как в общем контракте; проверить стек explain↔stage↔autolink в 3 вьюпортах.
""" + CONTRACT + COMMON_RULES

SUBS = [
    ("M0 · Аудит 12 модалей + дизайн-контракт 80% и mobile-style (Сессия A, шаг 1)", B_M0, ["documentation", "layout"]),
    ("M1 · kit-примитив modal_rect_flex + тир-детектор + юнит-тесты, TDD (Сессия A, шаг 2)", B_M1, ["enhancement", "layout"]),
    ("M2 · Фича-флаг modals_adaptive + тумблер настроек RU/EN + G4-lint (Сессия A, шаг 3)", B_M2, ["enhancement", "layout"]),
    ("M3-a · Адаптив простых модалей: dialog, migrate, canvas_manager, gallery (Сессия B)", B_M3A, ["enhancement", "layout"]),
    ("M3-b · Адаптив сложных модалей: settings, kit_gallery (+F1/F2), admin_panel (Сессия C)", B_M3B, ["enhancement", "layout"]),
    ("M3-c · Адаптив специальных модалей: stage, explain, autolink, onboarding, ai_onboarding (Сессия D)", B_M3C, ["enhancement", "layout"]),
]

def main():
    created = []
    for title, body, labels in SUBS:
        st, r = rest("POST", f"/repos/{REPO}/issues", {"title": title, "body": body, "labels": labels})
        if st not in (200, 201):
            print(f"FAIL create sub '{title}': {st} {r}"); sys.exit(1)
        created.append({"num": r["number"], "id": r["id"], "node": r["node_id"], "title": title})
        print(f"OK sub #{r['number']}: {title}")

    refs = "\n".join(f"- #{c['num']} — {c['title']}" for c in created)
    parent_body = f"""## Цель
Ремастер всех 12 истинных модалей (слой Modals + CapturePolicy::Block): размер **80% окна**, вёрстка по трём тирам раскладки с mobile-style веткой для компактных разрешений — под фича-флагом **«Адаптивные модали (β)»** в настройках. Старые ветки рендера сохраняются до отдельной приёмки (M5 — по запросу владельца).

{INVENTORY}
{SPLIT}

## Sub-issues
{refs}

## Источники
- Требование владельца: модали = 80% окна, адекватная вёрстка на всех важных разрешениях дизайн-концепции; для мобильных разрешений — mobile-style (одноколоночная вёрстка) по лучшим практикам [ui-for-agents](https://github.com/danku13/ui-for-agents).
- Контекст: `design/kit-demo-layout-review-2026-10.md` (дефекты Wave D v1, F1/F2 входят в M3-b).
"""
    st, p = rest("POST", f"/repos/{REPO}/issues", {"title": "Wave M: адаптивные модали 80% + mobile-style — ремастер 12 модалей под фича-флагом", "body": parent_body, "labels": ["enhancement", "layout"]})
    if st not in (200, 201):
        print(f"FAIL create parent: {st} {p}"); sys.exit(1)
    print(f"OK parent #{p['number']}: {p['title']}")
    parent = {"num": p["number"], "id": p["id"], "node": p["node_id"]}

    # Линка sub-issues (нативный sub-issues API; sub_issue_id = issue database ID)
    link_ok = True
    for c in created:
        st, r = rest("POST", f"/repos/{REPO}/issues/{parent['num']}/sub_issues", {"sub_issue_id": c["id"]})
        if st not in (200, 201):
            link_ok = False
            print(f"WARN link #{c['num']} -> parent: {st} {r}")
    if not link_ok:
        # фолбэк: task-list в теле эпика уже есть (refs списком) — достаточно
        print("FALLBACK: sub-issue API недоступен, остаётся список ссылок в теле эпика")

    # Проект v2
    added = []
    for item in created + [parent]:
        m = gql("""mutation($p:ID!,$c:ID!){ addProjectV2ItemById(input:{projectId:$p, contentId:$c}){ item{ id } } }""",
                {"p": PROJECT_ID, "c": item["node"]})
        if "errors" in m:
            print(f"WARN project add #{item['num']}: {m['errors']}")
        else:
            added.append(item["num"])
    print(json.dumps({"parent": parent['num'], "subs": [c['num'] for c in created],
                      "linked": link_ok, "project_added": added}, ensure_ascii=False))

if __name__ == "__main__":
    main()
