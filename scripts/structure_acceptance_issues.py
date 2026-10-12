#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Структурирование issues CanvasDesk:
import os
1) M4/M5 как sub-issues эпика #55 + обновление тела эпика
2) Новый эпик приёмки → sub-issues #35-41 + #59
3) Project v2: добавление недостающих items, статусы Todo
"""
import json, sys, urllib.request, urllib.error

PAT = os.environ.get("GITHUB_PAT", "")  # секрет вынесен в окружение (push protection)
PROJ_TOKEN = os.environ.get("GITHUB_PROJECT_TOKEN", "")  # секрет вынесен в окружение (push protection)
REPO = "danku13/CanvasDesk"
PROJECT_ID = "PVT_kwHOAhnxHc4Bi3lO"
STATUS_FIELD_ID = "PVTSSF_lAHOAhnxHc4Bi3lOzhhuC9M"
TODO_OPTION_ID = "f75ad846"
API = "https://api.github.com"

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

def create_issue(title, body, labels):
    st, r = rest("POST", f"/repos/{REPO}/issues", {"title": title, "body": body, "labels": labels})
    if st not in (200, 201):
        print(f"FAIL create '{title}': {st} {r}"); sys.exit(1)
    print(f"OK created #{r['number']}: {title}")
    return {"num": r["number"], "id": r["id"], "node": r["node_id"]}

def link_sub(parent_num, sub_id):
    st, r = rest("POST", f"/repos/{REPO}/issues/{parent_num}/sub_issues", {"sub_issue_id": sub_id})
    ok = st in (200, 201)
    print(f"{'OK' if ok else 'WARN'} link sub -> #{parent_num}: {st}")
    return ok

# ---------- Тела ----------
B_M4 = """Приёмка Wave M (эпик #55). Ручной обход владельцем после завершения реализации: M0–M3 закрыты (#49–54), все 12 модалей адаптивны под флагом `modals_adaptive`. Кодовые гейты зелёные ×2 по отчётам сессий A/B/C/D — здесь только «глазами владельца».

## Как проверять
- Сборка `main` (HEAD ≥ `77fa204`; на момент заведения — `002dd0c`+), native и/или Pages-деплой.
- Тумблер **«Адаптивные модали (β)»** в настройках: проверять **on** и **off** (off — убедиться, что рендер не изменился, паритет бит-в-бит подтверждён тестами).
- Вьюпорты: **1280×800**, **1024×640**, **800×560** (LAY8.1), **375**, **768**, **1440** (CR-014). Языки: **RU и EN**.

## Матрица приёмки (12 × 2 × 6 × 2)
| Модаль | 1280×800 | 1024×640 | 800×560 | 375 | 768 | 1440 |
|---|---|---|---|---|---|---|
| settings |  |  |  |  |  |  |
| stage |  |  |  |  |  |  |
| explain |  |  |  |  |  |  |
| migrate |  |  |  |  |  |  |
| canvas_manager |  |  |  |  |  |  |
| autolink |  |  |  |  |  |  |
| dialog |  |  |  |  |  |  |
| gallery |  |  |  |  |  |  |
| kit_gallery |  |  |  |  |  |  |
| admin_panel |  |  |  |  |  |  |
| onboarding |  |  |  |  |  |  |
| ai_onboarding |  |  |  |  |  |  |

Каждая ячейка = модаль × вьюпорт × (флаг on, флаг off) × (RU, EN).

## На что смотреть (задокументированные отклонения сессий C/D)
- тач-цели тулбара витрины 24px — осознанное отклонение T-M R1 в compact, демо-контролы D3 (#53);
- кнопка «Вернуть все» 30px < 44 — канон `kit::banner` (#54);
- подсказка клавиатуры autolink не рисуется в compact — T-M R3, тач-поверхность (#54);
- onboarding compact — визуальные кнопки ×k-канон, hit-44 через прямые rect'ы футера; арт не тронут (#54);
- stage compact — «действия вниз» штатными нижними якорями, сцена full-bleed 327×619 (#54);
- kit_gallery/admin на 375 — окно в вьюпорте, без молчаливых клипов (G4 375-ветка, #53);
- стеки: explain поверх stage, autolink прячет explain, ai_onboarding backdrop НЕ закрывает — контракты прежние (#54).

## DoD
- [ ] Матрица просмотрена; дефекты либо отсутствуют, либо заведены отдельными issues со ссылкой сюда.
- [ ] Флаг OFF: рендер визуально идентичен pre-Wave M.
- [ ] Комментарий-вердикт в этом issue.
- [ ] После вердикта — закрытие эпика #55 как completed (M5 не блокирует — см. sub-issue M5).
"""

B_M5 = """**Вне серии Wave M — отдельная волна по запросу владельца. НА закрытие эпика #55 НЕ влияет** (эпик закрывается по вердикту M4).

Контекст: при флаге `modals_adaptive` все 12 модалей рисуются адаптивными ветками; старые ветки рендера оставались до приёмки (решение эпика #55). После вердикта M4 старые пути можно демонтировать.

## Задачи
- [ ] Удалить старые ветки рендера 12 модалей (`if settings.modals_adaptive` — оставить только adaptive-путь).
- [ ] Дефолт `settings.modals_adaptive` → `true`, затем чистка флага: убрать поле, тумблер «Адаптивные модали (β)» и строки i18n RU/EN.
- [ ] G4: `lint_modals_adaptive` слить с каноническим lint-путём (или объявить единственным).
- [ ] Пины/тесты OFF-путей (stage 0.7, autolink rows-стек, ai_onboarding M/E, footer M/E, пин формулы 0.45/0.60 settings) — перепинить на adaptive-геометрию или удалить.
- [ ] Доки: обновить `design/modals-audit-2026-10.md` (статус демонтажа), README/ACCEPTANCE если упоминают флаг.

## DoD
- Гейты полные (fmt/check/test/clippy/wasm/lay7/doc_lint), коммиты `scope=wave-m-m5`.
- Владелец подтвердил готовность демонтажа (после M4).
"""

B_EPIC = """## Цель
Одно место для всех накопившихся **ручных приёмок владельцем**: точечные смоуки волн раскладки LAY (W7–W21), финальный регресс-смоук поверхностей и приёмка мультиканваса + Wave M β. Кодовые гейты везде зелёные — здесь только проверки «глазами владельца», код не меняется.

## Sub-issues (группировка)
| # | Скоуп | Волна / опора |
|---|---|---|
| #39 | [LAY-W7] Визуальная приёмка высот S3: 9 сдвигов + 4 алиаса, эталон `6e3e6b1` | `04de817` |
| #40 | [LAY-W8+W15] Тач-приёмка coarse: 44px-зоны, резолюция «ближайший центр» | `9939413` |
| #41 | [LAY-W13] Админка: тела демо-секций после Column-миграции | `751e211` |
| #35 | [LAY-W17] G4-поверхности: смоук ai_onboarding, calc-строки, wheel/choice/dialog | `44ab017` |
| #36 | [LAY-W20] Агент-панель и AI-статус: Accept/Reject, ⏸/⚙, чипы | `e8d9050`+`d82a1a8` |
| #37 | [LAY-W16/W18/W21] «Исключения»: смоук инвариантности поверхностей | `ac1bf1c`/`366016a`/`e7f81c6` |
| #38 | [Финал] Регресс-смоук поверхностей LAY8.1: LAY-W13–W21 + параллельные волны | `main 4aca0fb+` |
| #59 | Мультиканвас C2–C5 (FR-105–108) + Wave M β: ручная приёмка | #6–#9 + #49–51 |

## Рекомендуемый порядок
1. Точечные смоуки: #39 → #40 → #41 → #35 → #36 → #37 (каждый ~15–30 мин, порядок свободный).
2. Финальный сквозной обход **#38** — после точечных (ловит пересечения волн, ui-kit/мультиканвас после `fafa59a`).
3. **#59** — не зависит, параллельно; в части Wave M β теперь покрыт отдельной приёмкой **M4** эпика #55 (см. ниже) — в #59 фокус на мультиканвасе C2–C5 и владельческой части FR-108 (Windows-диалоги, WASM L2).

## Примечания
- Wave M M4 (12 модалей × флаг × 6 вьюпортов × RU/EN) — отдельный sub-issue эпика #55: {M4_REF}.
- Дефекты, найденные при приёмке, заводить отдельными issues со ссылкой на источник (прецедент: `docs/user-reporting/`).
- Закрытие этого эпика — когда закрыты все 8 sub-issues.
"""

# ---------- Этап 1: M4/M5 → эпик #55 ----------
print("=== STAGE 1: M4/M5 sub-issues of #55 ===")
m4 = create_issue(
    "M4 · Приёмка владельцем: 12 модалей × флаг on/off × {1280×800, 1024×640, 800×560, 375, 768, 1440} × RU/EN (эпик #55)",
    B_M4, ["приёмка", "layout"])
m5 = create_issue(
    "M5 · Демонтаж старых веток рендера — вне серии (отдельный запрос владельца), не блокирует закрытие эпика #55",
    B_M5, ["enhancement", "layout"])
link_sub(55, m4["id"]); link_sub(55, m5["id"])

# обновление тела эпика #55
st, epic = rest("GET", f"/repos/{REPO}/issues/55")
body = epic["body"]
old54 = "- #54 — M3-c · Адаптив специальных модалей: stage, explain, autolink, onboarding, ai_onboarding (Сессия D)"
assert old54 in body, "sub-issues list not found in #55 body"
body = body.replace(old54, old54 + f"\n- #{m4['num']} — M4 · Приёмка владельцем: 12 модалей × флаг on/off × {{1280×800, 1024×640, 800×560, 375, 768, 1440}} × RU/EN\n- #{m5['num']} — M5 · Демонтаж старых веток рендера — вне серии, не блокирует закрытие эпика")
old_m4 = "## M4 — приёмка владельцем (после B/C/D)\nЧек-лист: 12 модалей × флаг on/off × {1280×800, 1024×640, 800×560, 375, 768, 1440} × RU/EN."
assert old_m4 in body, "M4 section not found in #55 body"
body = body.replace(old_m4, f"## M4 — приёмка владельцем (после B/C/D)\nЧек-лист и матрица приёмки — в sub-issue #{m4['num']}: 12 модалей × флаг on/off × {{1280×800, 1024×640, 800×560, 375, 768, 1440}} × RU/EN.")
old_m5 = "## M5 — демонтаж старых версий\nОтдельная волна по отдельному запросу владельца (удаление старых веток рендера, дефолт флага → true, чистка флага). В эту серию issues НЕ входит."
assert old_m5 in body, "M5 section not found in #55 body"
body = body.replace(old_m5, f"## M5 — демонтаж старых версий\nSub-issue #{m5['num']} — вне серии, по отдельному запросу владельца, НА закрытие эпика НЕ влияет (удаление старых веток рендера, дефолт флага → true, чистка флага).")
st, _ = rest("PATCH", f"/repos/{REPO}/issues/55", {"body": body})
print(f"{'OK' if st in (200,201) else 'FAIL'} PATCH #55 body: {st}")

# ---------- Этап 2: эпик приёмки → #35-41 + #59 ----------
print("=== STAGE 2: acceptance epic -> #35-41 + #59 ===")
epic_body = B_EPIC.replace("{M4_REF}", f"#{m4['num']}")
acc = create_issue(
    "Приёмка — ручные проверки владельцем: LAY-волны W7–W21, финальный регресс, мультиканвас C2–C5 + Wave M β",
    epic_body, ["приёмка"])
for n in [35, 36, 37, 38, 39, 40, 41, 59]:
    st, iss = rest("GET", f"/repos/{REPO}/issues/{n}")
    link_sub(acc["num"], iss["id"])

# ---------- Этап 3: Project v2 ----------
print("=== STAGE 3: Project v2 ===")
q = gql("""query($p:ID!){ node(id:$p){ ... on ProjectV2 { items(first:100){ nodes{ id content{ ... on Issue { number state } } } } } } }""", {"p": PROJECT_ID})
in_project = {it["content"]["number"]: it["id"] for it in q["data"]["node"]["items"]["nodes"] if it.get("content") and "number" in it["content"]}
print(f"in project already: {sorted(in_project)}")

targets = {m4["num"]: m4["node"], m5["num"]: m5["node"], acc["num"]: acc["node"],
           **{n: None for n in [35,36,37,38,39,40,41,59] if n not in in_project}}
# для существующих issues нужен node_id — добираем
for n in [k for k, v in targets.items() if v is None and k not in in_project]:
    st, iss = rest("GET", f"/repos/{REPO}/issues/{n}")
    targets[n] = iss["node_id"]

added = []
for num, node in targets.items():
    if num in in_project:
        print(f"skip #{num} (already in project)"); continue
    m = gql("""mutation($p:ID!,$c:ID!){ addProjectV2ItemById(input:{projectId:$p, contentId:$c}){ item{ id } } }""", {"p": PROJECT_ID, "c": node})
    if "errors" in m:
        print(f"WARN project add #{num}: {m['errors']}"); continue
    in_project[num] = m["data"]["addProjectV2ItemById"]["item"]["id"]
    added.append(num)
print(f"added to project: {added}")

# статусы Todo для новых items
for num, item_id in in_project.items():
    if num in targets or num in [35,36,37,38,39,40,41,59, m4["num"], m5["num"], acc["num"]]:
        m = gql("""mutation($p:ID!,$i:ID!,$f:ID!,$o:String!){ updateProjectV2ItemFieldValue(input:{projectId:$p, itemId:$i, fieldId:$f, value:{singleSelectOptionId:$o}}){ projectV2Item{ id } } }""",
                {"p": PROJECT_ID, "i": item_id, "f": STATUS_FIELD_ID, "o": TODO_OPTION_ID})
        ok = "errors" not in m
        print(f"{'OK' if ok else 'WARN'} status=Todo #{num}")

# ---------- Верификация ----------
print("=== VERIFY ===")
st, subs55 = rest("GET", f"/repos/{REPO}/issues/55/sub_issues")
print(f"#55 sub-issues ({len(subs55)}): {[s['number'] for s in subs55]}")
st, subsacc = rest("GET", f"/repos/{REPO}/issues/{acc['num']}/sub_issues")
print(f"#acc sub-issues ({len(subsacc)}): {[s['number'] for s in subsacc]}")
print(json.dumps({"m4": m4["num"], "m5": m5["num"], "acceptance_epic": acc["num"], "project_added": added}, ensure_ascii=False))
