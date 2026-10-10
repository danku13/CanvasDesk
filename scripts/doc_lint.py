#!/usr/bin/env python3
"""doc_lint.py — линк-чек документации CanvasDesk (issue #18).

Правила (AGENTS.md, «Именование ссылок в документации»):
1. Ссылки `*.md` обязаны существовать (разрешение относительно файла).
2. Ссылки `*.html` из `user-docs/` — штатный механизм FR-031 (вшитый
   просмотрщик); проверяется, что соответствующий `*.md`-источник существует.
3. Ссылки `*.html` вне `user-docs/` разрешены, только если `*.html` реально
   лежит на диске (артефакты вроде docs/prototypes/*.html, docs/404.html).
   Ссылаться на md-ИСТОЧНИК через .html запрещено.
4. http(s)/mailto/якоря/абсолютные web-пути (/app, /CanvasDesk/app) — пропуск.
5. Относительные ссылки на каталоги в .github/PULL_REQUEST_TEMPLATE.md
   разрешаются от корня репо (так делает GitHub для шаблонов).

Exit code 1 при наличии ошибок. Только стандартная библиотека.
"""
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SKIP_DIRS = {".git", "target", "node_modules", "dist", ".cargo"}

INLINE = re.compile(r"!?\[[^\]]*\]\(\s*([^)\s]+)(?:\s+\"[^\"]*\")?\s*\)")
REFDEF = re.compile(r"^\s*!?\[[^\]]+\]:\s+(\S+)\s*$", re.M)


def iter_md_files():
    yield from sorted(REPO.rglob("*.md"))


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


def main():
    errors = []
    n_links = 0
    for md in iter_md_files():
        if any(part in SKIP_DIRS for part in md.parts):
            continue
        text = md.read_text(encoding="utf-8", errors="replace")
        targets = [m.group(1) for m in INLINE.finditer(text)]
        targets += [m.group(1) for m in REFDEF.finditer(text)]
        for t in targets:
            n_links += 1
            errors += check_link(md, t)

    print(f"doc_lint: проверено ссылок: {n_links}, ошибок: {len(errors)}")
    for e in errors:
        print("  FAIL:", e)
    if errors:
        sys.exit(1)
    print("doc_lint: OK")


if __name__ == "__main__":
    main()
