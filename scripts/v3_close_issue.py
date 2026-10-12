#!/usr/bin/env python3
"""V3 (issue #68): [x] V3 в чек-листе, summary-коммент, close, board → Done."""
import json
import os
import urllib.request

PAT = os.environ["GITHUB_PAT"]
PTOK = os.environ["GITHUB_PROJECT_TOKEN"]
REPO = "danku13/CanvasDesk"
ISSUE = 68
PROJECT = "PVT_kwHOAhnxHc4Bi3lO"
STATUS_FIELD = "PVTSSF_lAHOAhnxHc4Bi3lOzhhuC9M"


def req(url: str, *, method: str = "GET", token: str = PAT, payload: dict | None = None) -> dict:
    data = json.dumps(payload).encode() if payload is not None else None
    r = urllib.request.Request(url, data=data, method=method, headers={
        "Authorization": f"Bearer {token}",
        "Accept": "application/vnd.github+json",
        "Content-Type": "application/json",
    })
    with urllib.request.urlopen(r) as resp:
        body = resp.read().decode()
        return json.loads(body) if body else {}


def gql(query: str, variables: dict) -> dict:
    return req("https://api.github.com/graphql", method="POST", token=PTOK,
               payload={"query": query, "variables": variables})


# 1) Тело: [ ] V3 → [x] V3
issue = req(f"https://api.github.com/repos/{REPO}/issues/{ISSUE}")
body = issue["body"].replace(
    "- [ ] **V3 — гейты**",
    "- [x] **V3 — гейты**",
)
req(f"https://api.github.com/repos/{REPO}/issues/{ISSUE}", method="PATCH",
    payload={"body": body})
print("body: V3 отмечена [x]")

# 2) Summary-коммент + close
comment = """**Волна V3 выполнена — все волны #68 закрыты** (V1 `cf54c55b`, V2 `bef8205a`, V3 `c4ed09c1`…`6dc2c68f`).

**Гейт (триада «Конвенция → Миграция → Гейт» теперь полная):**
- Новый CI-job `v3-ui-lint` — `scripts/v3_ui_scan.py` + `v3_ui_lint.sh`: **L1** полный-rect label (`label_center/label_left` вне `kit::text_area_vcentered`), **L2** магические вертикальные офсеты на текст/каретка-сайтах (литеральные y-сдвиги, ручные v-центрирующие формулы, литеральный dy в `push_line`), **L3** литералы ×1.3/×1.1 вне `measure.rs`; selftest 26 фикстур; инлайн-исключения `// v3:allow <причина>` (файловых allowlist нет); rustfmt-устойчивость standalone-маркеров
- Спека `design/rules/09-input.md` IN13 — якорь на фактический гейт

**Миграция V3:** baseline 171 хит (L1 51 / L2 120 / L3 0) → ~126 сайтов мигрировано на `text_area_vcentered`/`caret_rect` (число вызовов 153→312), 45 осознанных сайтов аннотированы `// v3:allow` (топ-анкерные стеки IN11, компонентные строковые боксы IN13, арт онбординга). Хвосты V2 (wheel-меню, d.icon-области, ~70 ручных пушей) закрыты этим же проходом.

**WASM self-check (L0+L2):**
- L0 `scripts/wasm_gate.sh --check` — зелёный (wasm-bindgen-cli 0.2.127 по Cargo.lock)
- L2 — новый сценарий `scripts/wasm_ui_lay81_scenario.mjs`: LAY8.1 (`?ui=demo`), канонические вьюпорты **1280×800** и **1024×640**, pixel-diff оракулы: стабильность кадра **0 px** (оба вьюпорта), клик-оракулы **999 791 px** (1280: смена темы Nord→Light) и **5 484 px** (1024: тултип демо). Скриншоты подтверждают канон вертикали визуально в двух темах и двух вьюпортах — подписи всех контролов центрированы
- Вне скоупа #68: дефолтный `wasm_ui_scenario.mjs` дрейфанул по координатам онбординга (новый шаг «выбор языка» + хром CR-036) — перекалибровка кандидатом в отдельную задачу

**Гейты приёмки:** fmt, clippy `--workspace -D warnings`, `cargo test --workspace` (96 наборов, 0 failed), doc_lint (108 pre-existing в `skills/`, вне сессии), v3-ui-lint — зелёные.

Онбординг/user-docs: правки геометрические, тексты и шаги тура не менялись — обновления не требуются."""
req(f"https://api.github.com/repos/{REPO}/issues/{ISSUE}", method="POST",
    payload={"body": comment})
print("comment: summary отправлен")
req(f"https://api.github.com/repos/{REPO}/issues/{ISSUE}", method="PATCH",
    payload={"state": "closed", "state_reason": "completed"})
print("issue #68 closed (completed)")

# 3) Board: item issue #68 → Status Done
q = """
query($project: ID!) {
  node(id: $project) {
    ... on ProjectV2 {
      items(first: 100) {
        nodes {
          id
          content { ... on Issue { number } }
        }
      }
      field(name: "Status") {
        ... on ProjectV2SingleSelectField {
          id
          options { id name }
        }
      }
    }
  }
}"""
res = gql(q, {"project": PROJECT})
field = res["data"]["node"]["field"]
done = next(o["id"] for o in field["options"] if o["name"] == "Done")
item = next(i["id"] for i in res["data"]["node"]["items"]["nodes"]
            if i["content"] and i["content"].get("number") == ISSUE)
m = """
mutation($item: ID!, $field: ID!, $option: String!) {
  updateProjectV2ItemFieldValue(input: {projectId: "%s", itemId: $item, fieldId: $field, value: {singleSelectOptionId: $option}}) {
    projectV2Item { id }
  }
}""" % PROJECT
gql(m, {"item": item, "field": field["id"], "option": done})
print(f"board: issue #{ISSUE} → Done")
