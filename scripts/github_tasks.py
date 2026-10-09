#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""github_tasks.py — планирование работ CanvasDesk в GitHub: issues, sub-issues, Projects.

Реализует директиву владельца от 2026-10-10 (AGENTS.md, раздел «Планирование
работ: GitHub issues + Projects»): доработка начинается с high-level issue,
части реализации (волны, разбивка на сабагентов) — sub-issues, закрытие —
только по факту проверки.

Команды:
  issue create/list/close   — задачи: создание (тело из строки/файла, labels,
                              привязка к родителю, размещение на доске);
  sub add/list/remove       — sub-issues (части реализации high-level задачи);
  project add/status/items  — доска GitHub Projects (Todo / In Progress / Done);
  plan --spec FILE          — атомарно: high-level issue + sub-issues + доска.

Токены (окружение; флаги --token / --project-token):
  GITHUB_TOKEN          — issues: classic PAT scope `repo`, либо fine-grained
                         «Issues: Read and write» на репозиторий.
  GITHUB_PROJECT_TOKEN  — доска Projects (GraphQL): classic PAT scope
                         `project`, либо fine-grained «Projects: Read and
                         write». Не задан — используется GITHUB_TOKEN (удобно,
                         когда один токен имеет оба скоупа).

По умолчанию репозиторий danku13/CanvasDesk, доска Projects #1
(переопределение: --owner / --repo / --project-num, env GH_PROJECT_NUM).
Все команды идемпотентны (повторный запуск не дублирует). Только стандартная
библиотека Python 3.8+.

Примеры:
  GITHUB_TOKEN=ghp_... python scripts/github_tasks.py issue list --limit 20
  GITHUB_TOKEN=ghp_... python scripts/github_tasks.py issue create \\
      --title "C0 — контракты" --body-file c0.md --parent 14 --to-board
  GITHUB_TOKEN=ghp_... python scripts/github_tasks.py sub add --parent 14 --child 4 5 6
  GITHUB_TOKEN=ghp_... GITHUB_PROJECT_TOKEN=ghp_... \\
      python scripts/github_tasks.py plan --spec plan.json --dry-run
"""

import argparse
import json
import os
import sys
import urllib.error
import urllib.request

API = "https://api.github.com"
UA = "canvasdesk-github-tasks"
DEFAULT_OWNER = "danku13"
DEFAULT_REPO = "CanvasDesk"


class ApiError(Exception):
    """HTTP-ошибка GitHub API с телом ответа (для читаемой диагностики)."""


# ---------------------------------------------------------------- HTTP-слой

def _http(url, method, payload, token):
    data = json.dumps(payload).encode("utf-8") if payload is not None else None
    headers = {
        "Authorization": "Bearer " + token,
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": UA,
    }
    if data is not None:
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, method=method, data=data, headers=headers)
    try:
        with urllib.request.urlopen(req) as r:
            raw = r.read()
            return r.status, (json.loads(raw) if raw else {})
    except urllib.error.HTTPError as e:
        raw = e.read().decode("utf-8", "replace")
        try:
            body = json.loads(raw)
        except Exception:
            body = {"raw": raw[:400]}
        msg = body.get("message") or json.dumps(body, ensure_ascii=False)[:300]
        raise ApiError("HTTP %s %s %s: %s" % (e.code, method, url, msg)) from None


def token_issues(args):
    tok = args.token or os.environ.get("GITHUB_TOKEN")
    if not tok:
        raise SystemExit("Ошибка: GITHUB_TOKEN не задан (issues; classic PAT scope `repo`).")
    return tok


def token_project(args):
    tok = (args.project_token or os.environ.get("GITHUB_PROJECT_TOKEN")
           or args.token or os.environ.get("GITHUB_TOKEN"))
    if not tok:
        raise SystemExit("Ошибка: нет токена Projects (GITHUB_PROJECT_TOKEN или GITHUB_TOKEN; "
                         "classic PAT scope `project`).")
    return tok


def rest(args, path, method="GET", payload=None):
    """REST-вызов к API репозитория (issues/sub-issues)."""
    st, body = _http(API + path, method, payload, token_issues(args))
    return st, body


def gql(args, query, variables=None):
    """GraphQL-вызов (доска Projects). Ошибки GraphQL — исключение."""
    payload = {"query": query, "variables": variables or {}}
    _, body = _http(API + "/graphql", "POST", payload, token_project(args))
    if body.get("errors"):
        raise ApiError("GraphQL: %s" % json.dumps(body["errors"], ensure_ascii=False)[:400])
    return body.get("data") or {}


def repo_path(args, suffix=""):
    return "/repos/%s/%s%s" % (args.owner, args.repo, suffix)


# ---------------------------------------------------------------- REST: issues

def issue_get(args, number):
    st, issue = rest(args, repo_path(args, "/issues/%s" % number))
    if st != 200:
        raise ApiError("issue #%s не получен" % number)
    return issue


def issue_create_raw(args, title, body, labels=None):
    """Создание issue; при отказе из-за labels — ретрай без них (права триажа)."""
    payload = {"title": title, "body": body or ""}
    if labels:
        payload["labels"] = labels
    try:
        st, issue = rest(args, repo_path(args, "/issues"), "POST", payload)
    except ApiError:
        if labels:
            print("[warn] создание с labels отклонено — ретрай без labels")
            payload.pop("labels")
            st, issue = rest(args, repo_path(args, "/issues"), "POST", payload)
        else:
            raise
    print("[issue] OK  #%s %s" % (issue["number"], issue["html_url"]))
    return issue


def sub_link(args, parent, child_number, child_id):
    """Привязка child как sub-issue к parent (идемпотентно)."""
    st, subs = rest(args, repo_path(args, "/issues/%s/sub_issues?per_page=100" % parent))
    existing = {s.get("id") for s in (subs if isinstance(subs, list) else [])}
    if child_id in existing:
        print("[sub]   skip #%s уже sub-issue #%s" % (child_number, parent))
        return
    rest(args, repo_path(args, "/issues/%s/sub_issues" % parent), "POST",
         {"sub_issue_id": child_id})
    print("[sub]   OK  #%s -> sub-issue #%s" % (child_number, parent))


# ---------------------------------------------------------------- GraphQL: Projects

def project_of(args, number):
    """Поиск проекта по номеру: сначала user, затем organization."""
    for kind in ("user", "organization"):
        q = ("query($login: String!, $num: Int!) { %s(login: $login) "
             "{ projectV2(number: $num) { id title } } }" % kind)
        proj = (gql(args, q, {"login": args.owner, "num": number}).get(kind) or {}).get("projectV2")
        if proj:
            return proj
    raise ApiError("Проект #%s не найден или токену не хватает прав (scope `project`)" % number)


def board_nodes(args, project_number):
    proj = project_of(args, project_number)
    q = ('query($pid: ID!) { node(id: $pid) { ... on ProjectV2 { items(first: 100) '
         '{ nodes { id content { ... on Issue { number title state } } '
         'status: fieldValueByName(name: "Status") '
         '{ ... on ProjectV2ItemFieldSingleSelectValue { name } } } } } } }')
    nodes = gql(args, q, {"pid": proj["id"]})["node"]["items"]["nodes"]
    return proj, [n for n in nodes if (n.get("content") or {}).get("number")]


def project_add(args, number, project_number):
    """Добавление issue на доску (идемпотентно). Возвращает id карточки."""
    proj, nodes = board_nodes(args, project_number)
    for n in nodes:
        if n["content"]["number"] == number:
            print("[board] skip #%s уже на доске" % number)
            return n["id"]
    issue = issue_get(args, number)
    m = ("mutation($pid: ID!, $cid: ID!) { addProjectV2ItemById("
         "input: {projectId: $pid, contentId: $cid}) { item { id } } }")
    r = gql(args, m, {"pid": proj["id"], "cid": issue["node_id"]})
    item_id = r["addProjectV2ItemById"]["item"]["id"]
    print("[board] OK  #%s добавлен на доску #%s" % (number, project_number))
    return item_id


def project_set_status(args, number, value, project_number, item_id=None):
    """Установка поля Status у карточки issue (идемпотентно).

    item_id — известный id карточки из project_add: повторный листинг доски
    может не видеть только что добавленную карточку (консистентность GitHub).
    """
    proj = project_of(args, project_number)
    q = ('query($pid: ID!) { node(id: $pid) { ... on ProjectV2 { '
         'statusField: field(name: "Status") '
         '{ ... on ProjectV2SingleSelectField { id options { id name } } } } } }')
    field = gql(args, q, {"pid": proj["id"]})["node"]["statusField"]
    if not field:
        raise ApiError("На доске #%s нет поля Status" % project_number)
    opt = next((o for o in field["options"] if o["name"].lower() == value.lower()), None)
    if not opt:
        raise ApiError("Статус '%s' не найден; доступные: %s"
                       % (value, ", ".join(o["name"] for o in field["options"])))
    if item_id is None:
        _, nodes = board_nodes(args, project_number)
        node = next((n for n in nodes if n["content"]["number"] == number), None)
        if not node:
            raise ApiError("issue #%s не найден на доске #%s — сначала `project add`"
                           % (number, project_number))
        item_id = node["id"]
        cur = (node.get("status") or {}).get("name")
        if cur == opt["name"]:
            print("[status] skip #%s уже '%s'" % (number, cur))
            return
    m = ("mutation($pid: ID!, $iid: ID!, $fid: ID!, $oid: String!) { "
         "updateProjectV2ItemFieldValue(input: {projectId: $pid, itemId: $iid, "
         "fieldId: $fid, value: {singleSelectOptionId: $oid}}) "
         "{ projectV2Item { id } } }")
    gql(args, m, {"pid": proj["id"], "iid": item_id, "fid": field["id"], "oid": opt["id"]})
    print("[status] OK  #%s -> '%s'" % (number, opt["name"]))


# ---------------------------------------------------------------- Команды CLI

def cmd_issue_create(args):
    body = args.body or ""
    if args.body_file:
        with open(args.body_file, encoding="utf-8") as f:
            body = f.read()
    labels = [x.strip() for x in (args.labels or "").split(",") if x.strip()]
    issue = issue_create_raw(args, args.title, body, labels)
    if args.parent:
        sub_link(args, args.parent, issue["number"], issue["id"])
    if args.to_board:
        pnum = resolve_project_num(args)
        item_id = project_add(args, issue["number"], pnum)
        if args.status:
            project_set_status(args, issue["number"], args.status, pnum, item_id=item_id)
    return issue


def cmd_issue_list(args):
    st, items = rest(args, repo_path(args, "/issues?state=%s&per_page=%s" % (args.state, args.limit)))
    for it in items:
        if "pull_request" in it:  # /issues включает и PR — фильтруем
            continue
        print("#%-4s %-6s %s" % (it["number"], it["state"], it["title"]))
    return items


def cmd_issue_close(args):
    if args.comment:
        rest(args, repo_path(args, "/issues/%s/comments" % args.number), "POST",
             {"body": args.comment})
        print("[close] комментарий добавлен к #%s" % args.number)
    rest(args, repo_path(args, "/issues/%s" % args.number), "PATCH", {"state": "closed"})
    print("[close] OK  #%s закрыт" % args.number)


def cmd_sub_add(args):
    for child in args.child:
        issue = issue_get(args, child)
        sub_link(args, args.parent, child, issue["id"])


def cmd_sub_list(args):
    st, subs = rest(args, repo_path(args, "/issues/%s/sub_issues?per_page=100" % args.number))
    if not subs:
        print("у #%s нет sub-issues" % args.number)
        return
    for s in subs:
        print("#%-4s %-6s %s" % (s["number"], s["state"], s["title"]))


def cmd_sub_remove(args):
    issue = issue_get(args, args.child)
    rest(args, repo_path(args, "/issues/%s/sub_issues" % args.parent), "DELETE",
         {"sub_issue_id": issue["id"]})
    print("[sub] OK  #%s отвязан от #%s" % (args.child, args.parent))


def cmd_project_add(args):
    pnum = resolve_project_num(args)
    for number in args.numbers:
        item_id = project_add(args, number, pnum)
        if args.status:
            project_set_status(args, number, args.status, pnum, item_id=item_id)


def cmd_project_status(args):
    pnum = resolve_project_num(args)
    for number in args.numbers:
        project_set_status(args, number, args.value, pnum)


def cmd_project_items(args):
    pnum = resolve_project_num(args)
    proj, nodes = board_nodes(args, pnum)
    print("Доска #%s «%s» — %s карточек:" % (pnum, proj["title"], len(nodes)))
    for n in nodes:
        c = n["content"]
        status = (n.get("status") or {}).get("name") or "—"
        print("#%-4s %-8s %-6s %s" % (c["number"], status, c["state"], c["title"]))


def resolve_project_num(args):
    return args.project_num or int(os.environ.get("GH_PROJECT_NUM", "1"))


# ---------------------------------------------------------------- plan: атомарное создание

def _spec_body(spec, base):
    if spec.get("body_file"):
        with open(spec["body_file"], encoding="utf-8") as f:
            return f.read()
    return spec.get("body") or ""


def cmd_plan(args):
    """Создание high-level issue + sub-issues + доска по спецификации JSON.

    Формат spec:
      { "title": "...", "body": "..." | "body_file": "...", "labels": [...],
        "subs": [ {"title": "...", "body": "...", "labels": [...]}, ... ],
        "project": {"add": true, "number": 1, "status": "Todo"} }
    """
    with open(args.spec, encoding="utf-8") as f:
        spec = json.load(f)
    proj_cfg = spec.get("project") or {}
    pnum = args.project_num or proj_cfg.get("number") or int(os.environ.get("GH_PROJECT_NUM", "1"))
    status = proj_cfg.get("status", "Todo")

    print("== plan: %s" % spec["title"])
    print("   sub-issues: %s" % len(spec.get("subs", [])))
    if args.dry_run:
        print("   (dry-run: изменений нет)")
        return

    parent = issue_create_raw(args, spec["title"], _spec_body(spec, None), spec.get("labels"))
    children = []
    for sub_spec in spec.get("subs", []):
        child = issue_create_raw(args, sub_spec["title"], _spec_body(sub_spec, None),
                                 sub_spec.get("labels"))
        sub_link(args, parent["number"], child["number"], child["id"])
        children.append(child)

    if proj_cfg.get("add", True):
        try:
            item_id = project_add(args, parent["number"], pnum)
            if status:
                project_set_status(args, parent["number"], status, pnum, item_id=item_id)
            for child in children:
                item_id = project_add(args, child["number"], pnum)
                if status:
                    project_set_status(args, child["number"], status, pnum, item_id=item_id)
        except (ApiError, SystemExit) as e:
            print("[board] ПРОПУЩЕНО (issue созданы и связаны): %s" % e)

    print("== готово: high-level #%s + %s sub-issues" % (parent["number"], len(children)))
    print("   %s" % parent["html_url"])


# ---------------------------------------------------------------- argparse

def build_parser():
    p = argparse.ArgumentParser(description="Планирование CanvasDesk: GitHub issues + "
                                            "sub-issues + Projects (правило AGENTS.md).")
    p.add_argument("--owner", default=DEFAULT_OWNER)
    p.add_argument("--repo", default=DEFAULT_REPO)
    p.add_argument("--token", help="PAT для issues (default: env GITHUB_TOKEN)")
    p.add_argument("--project-token", help="PAT для Projects (default: env GITHUB_PROJECT_TOKEN "
                                            "-> GITHUB_TOKEN)")
    p.add_argument("--project-num", type=int, default=None,
                   help="номер Projects-доски (default: 1 / env GH_PROJECT_NUM)")
    sub = p.add_subparsers(dest="cmd", required=True)

    issue = sub.add_parser("issue", help="задачи-issues")
    isub = issue.add_subparsers(dest="issue_cmd", required=True)
    ic = isub.add_parser("create", help="создать issue")
    ic.add_argument("--title", required=True)
    ic.add_argument("--body", default=None)
    ic.add_argument("--body-file", default=None)
    ic.add_argument("--labels", default=None, help="через запятую")
    ic.add_argument("--parent", type=int, default=None, help="сразу привязать как sub-issue")
    ic.add_argument("--to-board", action="store_true", help="добавить на доску Projects")
    ic.add_argument("--status", default="Todo", help="статус на доске при --to-board")
    ic.set_defaults(fn=cmd_issue_create)

    il = isub.add_parser("list", help="список issues")
    il.add_argument("--state", default="open", choices=["open", "closed", "all"])
    il.add_argument("--limit", type=int, default=30)
    il.set_defaults(fn=cmd_issue_list)

    iclose = isub.add_parser("close", help="закрыть issue (по факту проверки)")
    iclose.add_argument("number", type=int)
    iclose.add_argument("--comment", default=None, help="сводка при закрытии (рекомендуется)")
    iclose.set_defaults(fn=cmd_issue_close)

    sp = sub.add_parser("sub", help="sub-issues (части реализации)")
    ssub = sp.add_subparsers(dest="sub_cmd", required=True)
    sa = ssub.add_parser("add", help="привязать children к родителю")
    sa.add_argument("--parent", type=int, required=True)
    sa.add_argument("--child", type=int, nargs="+", required=True)
    sa.set_defaults(fn=cmd_sub_add)
    sl = ssub.add_parser("list", help="sub-issues задачи")
    sl.add_argument("number", type=int)
    sl.set_defaults(fn=cmd_sub_list)
    sr = ssub.add_parser("remove", help="отвязать child")
    sr.add_argument("--parent", type=int, required=True)
    sr.add_argument("--child", type=int, required=True)
    sr.set_defaults(fn=cmd_sub_remove)

    pp = sub.add_parser("project", help="доска GitHub Projects")
    psub = pp.add_subparsers(dest="proj_cmd", required=True)
    pa = psub.add_parser("add", help="issue -> доска")
    pa.add_argument("numbers", type=int, nargs="+")
    pa.add_argument("--status", default=None, help="заодно установить статус")
    pa.set_defaults(fn=cmd_project_add)
    ps = psub.add_parser("status", help="установить Status")
    ps.add_argument("numbers", type=int, nargs="+")
    ps.add_argument("--value", required=True, help="Todo | In Progress | Done")
    ps.set_defaults(fn=cmd_project_status)
    pi = psub.add_parser("items", help="карточки доски")
    pi.set_defaults(fn=cmd_project_items)

    pl = sub.add_parser("plan", help="high-level issue + sub-issues + доска (одной командой)")
    pl.add_argument("--spec", required=True, help="JSON-файл спецификации")
    pl.add_argument("--dry-run", action="store_true")
    pl.set_defaults(fn=cmd_plan)
    return p


def main(argv=None):
    args = build_parser().parse_args(argv)
    try:
        args.fn(args)
    except ApiError as e:
        print("[FAIL] %s" % e, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
