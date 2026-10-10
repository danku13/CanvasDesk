#!/usr/bin/env python3
"""doc_lint.py — документационный гейт CanvasDesk (issue #18, волна 3 #33).

Проверки:
1. Ссылки (issue #18):
   1.1. Ссылки `*.md` обязаны существовать (разрешение относительно файла).
   1.2. Ссылки `*.html` из `user-docs/` — штатный механизм FR-031 (вшитый
        просмотрщик); проверяется, что соответствующий `*.md`-источник есть.
   1.3. Ссылки `*.html` вне `user-docs/` разрешены, только если `*.html`
        реально лежит на диске. Ссылаться на md-ИСТОЧНИК через .html запрещено.
   1.4. http(s)/mailto/якоря/абсолютные web-пути (/app, /CanvasDesk/app) —
        пропуск. Относительные ссылки .github/PULL_REQUEST_TEMPLATE.md
        разрешаются от корня репо (так делает GitHub).
2. Бэктик-пути (issue #33, план v2.1 этап 1): токен в бэктиках, выглядящий
   как путь репозитория (содержит `/`, только безопасные символы пути;
   одиночное имя — только для `*.md`), обязан существовать на диске.
   Исключения: web-пути с ведущим `/`, плейсхолдеры (`<>`, `*`, `$`), префиксы
   вне репо (`~/`, `%APPDATA%/`), генерируемые каталоги (target/, node_modules/).
3. Бюджеты файлов (issue #33, план v2.1 этап 10): контекстная экономика —
   входные точки агента не должны неконтролируемо расти. Бюджеты заданы в
   СИМВОЛАХ как приближение o200k-токенов (английская проза ~3.7-4.0
   символа/токен; связка зафиксирована комментарием у каждого бюджета).
   Измеритель оригинальных токенов в CI недоступен (stdlib-only), числа
   калиброваны замерами tiktoken o200k_base 2026-10-10.

Выход: exit 1 при наличии ошибок. Только стандартная библиотека.
"""
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SKIP_DIRS = {".git", "target", "node_modules", "dist", ".cargo"}

# Пути-префиксы, которые не обязаны существовать в чекауте (генерируемые/
# внешние). Проверка бэктик-путей их пропускает.
PATH_PREFIX_EXEMPT = ("target/", "node_modules/", "dist/", "~", "%", "$", "<", "*")
PATH_SAFE = re.compile(r"^[\w][\w./-]*$")
CODE_EXT = {
    ".md", ".rs", ".py", ".toml", ".yml", ".yaml", ".json", ".sh", ".js",
    ".html", ".css", ".txt", ".ps1", ".cmd", ".svg", ".woff2",
}

# Корневые каталоги репо (класс A: путь обязан существовать от корня).
ROOT_DIRS = {
    ".github", "assets", "cloud", "crates", "design", "docs", "examples",
    "scripts", "sdk", "skills", "user-docs", "worklog", "workspace",
}
# Крейты воркспейса (класс B: голое имя крейта в пути — известный дефект
# класса #20 «путь без префикса crates/»; проверяем, что crates/<путь> есть).
CRATES = {
    "canvas-app", "canvas-core", "canvas-graph-builder", "canvas-llm",
    "canvas-mcp", "canvas-mcp-headless", "canvas-preview-host",
    "canvas-render", "canvas-scene", "canvas-shell", "canvas-suggest",
    "canvas-ui", "canvas-web", "canvas-widgets",
}

# Файлы АГЕНТСКОГО КОНТУРА: жёсткая проверка бэктик-путей (агент идёт по
# путям буквально). Человеческий/исторический слой (ADR-тела, архивы,
# design/, researches) — под линк-чеком, но не под path-чеком: пути там
# исторические, шум проверки не оправдан (решение волны 3, #33).
AGENT_CONTOUR = {
    "AGENTS.md", "CONTEXT.md", "README.md", "docs/translation-guide.md",
    "docs/SPEC.md", "docs/TASKS.md", "docs/RECIPES.md", "docs/ui-kit.md",
    "docs/WASM-TESTING.md", "docs/change-requests/index-cr-fr.md",
    "docs/change-requests/cr-template.md", "docs/adr/README.md",
    "docs/adr/adr-template.md", "docs/plans/product-roadmap.md",
    "docs/DEMO.md", "docs/DEPENDENCIES.md", "docs/WIDGETS.md",
}

# Бюджеты: относительный путь -> максимум символов (включая пробелы).
# Комментарий фиксирует токен-бюджет плана и калибровку o200k_base.
BUDGETS = {
    # План v2.1 (этап 6): корень <= 6k токенов. Замер после разгрузки
    # (волна 3, #33): <достигнутое> симв. ~ <N> tok -> бюджет с запасом.
    "AGENTS.md": 23_500,          # ~6.4k tok @ 3.68 chars/tok (после волны 3)
    # Журнал: критерий #23 (<=25k токенов на момент ротации) + волны 1-3
    # (плотные EN-записи 2.9 симв/токен). Ротация по правилу в AGENTS.md.
    "worklog.md": 120_000,        # ~41k tok @ 2.93 (факт 10-10: 105.7k/36.1k)
    # Спецификация: замер волны 2 (#24): 58 920 симв = 16 054 tok (3.67).
    "docs/SPEC.md": 62_000,       # ~16.9k tok
    # Активный индекс CR/FR: критерий #22 (активная часть <= 10k токенов).
    "docs/change-requests/index-cr-fr.md": 39_000,   # ~10.6k tok
    # Навигационный слой агента (этап 5): карта должна оставаться лёгкой.
    "docs/agent/MAP.md": 6_000,   # ~1.6k tok (факт 10-10: 5 114/1 422)
}

INLINE = re.compile(r"!?\[[^\]]*\]\(\s*([^)\s]+)(?:\s+\"[^\"]*\")?\s*\)")
REFDEF = re.compile(r"^\s*!?\[[^\]]+\]:\s+(\S+)\s*$", re.M)
BACKTICK = re.compile(r"`([^`\n]+)`")


def iter_md_files():
    yield from sorted(REPO.rglob("*.md"))


def strip_code_fences(text: str) -> str:
    """Убирает содержимое блоков ```...``` (для проверок вне фенсов)."""
    return re.sub(r"```.*?\n.*?```", "", text, flags=re.S)


def check_link(src: Path, target: str):
    """Возвращает список ошибок для одной ссылки."""
    errs = []
    if target.startswith(("http://", "https://", "mailto:", "#")):
        return errs
    if target.startswith("/"):
        return errs  # абсолютный web-путь (артефакт Pages, напр. /CanvasDesk/app/)
    path_part = target.split("#")[0]
    if not path_part:
        return errs

    # GitHub разрешает относительные ссылки PR-шаблона от корня репо
    if src.name == "PULL_REQUEST_TEMPLATE.md" and ".github" in src.parts:
        resolved = REPO / path_part
    else:
        resolved = src.parent / path_part

    if path_part.endswith(".md"):
        if not resolved.is_file():
            errs.append(f"{src}: битая md-ссылка -> {path_part}")
    elif path_part.endswith(".html"):
        in_userdocs = "user-docs" in src.relative_to(REPO).parts
        if in_userdocs:
            # FR-031: html-ссылка должна иметь md-источник рядом
            md_target = src.parent / (path_part[:-5] + ".md")
            if not md_target.is_file():
                errs.append(f"{src}: html-ссылка без md-источника -> {path_part}")
        else:
            if not resolved.is_file():
                errs.append(
                    f"{src}: *.html на несобранный источник -> {path_part} "
                    f"(правило: ссылаться на *.md)"
                )
    else:
        # каталог или файл без расширения
        if not resolved.exists():
            errs.append(f"{src}: битая ссылка -> {path_part}")
    return errs


def check_backtick_path(src: Path, token: str):
    """Бэктик-токен, выглядящий как путь репо, обязан существовать.

    Проверяются ТОЛЬКО два позитивных класса (решение волны 3, #33 —
    точность вместо полноты; остальное — контекстно-относительные пути
    внутри цитируемых деревьев, внешние репо (Seelen-UI/...), рантайм-
    выходы (download/, dist-widget/) — шум, не проверяем):

    Класс A — путь от корневого каталога репо (docs/, scripts/, skills/,
    crates/, ...): обязан существовать от корня.
    Класс B — голое имя крейта в пути (`canvas-ui/src/...`): это дефект
    класса #20 («путь без префикса crates/») — проверяем crates/<путь>;
    ошибка с подсказкой добавить префикс.
    """
    errs = []
    if not token or len(token) > 200:
        return errs
    if any(p in token for p in "<>*$|={}()\\ \t\"'`"):
        return errs  # плейсхолдер, glob, команда, не путь
    if "/" not in token:
        return errs  # голое имя — неоднозначно, пропускаем
    if token.startswith(("/", "http", "mailto:")):
        return errs  # web-путь/URL
    if not PATH_SAFE.match(token):
        return errs
    if token.startswith(PATH_PREFIX_EXEMPT):
        return errs
    if token.endswith("/"):
        token_q = token.rstrip("/")
    else:
        if Path(token).suffix not in CODE_EXT:
            return errs  # не файловый путь (команда, id, имя функции)
        token_q = token

    first = token_q.split("/", 1)[0]

    if first in ROOT_DIRS:
        # класс A: от корня репо
        if not (REPO / token_q).exists():
            label = "каталог" if token.endswith("/") else "путь"
            errs.append(f"{src}: бэктик-{label} не найден -> {token}")
        return errs

    if first in CRATES:
        # класс B: дефект класса #20 — путь крейта без префикса crates/
        if (REPO / "crates" / token_q).exists():
            errs.append(
                f"{src}: путь крейта без префикса crates/ -> {token} "
                f"(полный путь: crates/{token_q})"
            )
        else:
            errs.append(
                f"{src}: путь крейта не найден -> {token} "
                f"(и crates/{token_q} не существует)"
            )
        return errs

    return errs  # не относится ни к одному классу — не проверяем


def check_budgets():
    errs = []
    for rel, limit in sorted(BUDGETS.items()):
        p = REPO / rel
        if not p.is_file():
            errs.append(f"{rel}: бюджет задан, но файл отсутствует")
            continue
        size = len(p.read_text(encoding="utf-8", errors="replace"))
        if size > limit:
            errs.append(
                f"{rel}: бюджет превышен: {size} > {limit} символов "
                f"(ротация/разгрузка по правилам в AGENTS.md; план v2.1 этап 10)"
            )
    return errs


def is_agent_contour(src: Path) -> bool:
    rel = src.relative_to(REPO).as_posix()
    return rel in AGENT_CONTOUR or rel.startswith(("docs/agent/", "skills/"))


def main():
    errors = []
    n_links = 0
    n_paths = 0
    for md in iter_md_files():
        if any(part in SKIP_DIRS for part in md.parts):
            continue
        text = md.read_text(encoding="utf-8", errors="replace")
        targets = [m.group(1) for m in INLINE.finditer(text)]
        targets += [m.group(1) for m in REFDEF.finditer(text)]
        for t in targets:
            n_links += 1
            errors += check_link(md, t)
        # бэктик-пути: во всём файле, включая фенсы (деревья каталогов в
        # фенсах — тоже факты о репо); фильтр PATH_SAFE отсеет команды.
        # Жёсткая проверка — только для агентского контура.
        if is_agent_contour(md):
            body = strip_code_fences(text)
            tokens = set(BACKTICK.findall(body)) | set(BACKTICK.findall(text))
            for tok in sorted(tokens):
                n_paths += 1
                errors += check_backtick_path(md, tok)

    errors += check_budgets()

    print(f"doc_lint: проверено ссылок: {n_links}, бэктик-путей: {n_paths}, "
          f"бюджетов: {len(BUDGETS)}, ошибок: {len(errors)}")
    for e in errors:
        print("  FAIL:", e)
    if errors:
        sys.exit(1)
    print("doc_lint: OK")


if __name__ == "__main__":
    main()
