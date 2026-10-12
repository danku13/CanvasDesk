#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Вычищает хардкод GitHub PAT из скриптов предыдущих сессий:
заменяет присваивания вида VAR = "github_pat_..."/"ghp_..." на os.environ.get(...),
добавляет import os при необходимости. Запуск один раз, идемпотентен."""
import re

FILES = [
    "/home/z/my-project/scripts/epic55_status_comment.py",
    "/home/z/my-project/scripts/m4_big_screens_update.py",
    "/home/z/my-project/scripts/m4_macbook_mapping_comment.py",
    "/home/z/my-project/scripts/structure_acceptance_issues.py",
    "/home/z/my-project/scripts/wave_m_issues.py",
]

PAT_RE = re.compile(r'github_pat_[A-Za-z0-9_]{20,}|ghp_[A-Za-z0-9]{20,}')


def env_name_for(token: str) -> str:
    return "GITHUB_PROJECT_TOKEN" if token.startswith("ghp_") else "GITHUB_PAT"


def scrub(path: str) -> int:
    with open(path, "r", encoding="utf-8") as f:
        lines = f.readlines()
    changed = 0
    for i, line in enumerate(lines):
        m = PAT_RE.search(line)
        if not m or "=" not in line:
            continue
        var = line.split("=", 1)[0].strip()
        env = env_name_for(m.group(0))
        lines[i] = f'{var} = os.environ.get("{env}", "")  # секрет вынесен в окружение (push protection)\n'
        changed += 1
    if changed:
        text = "".join(lines)
        if not re.search(r'^\s*import os\b', text, re.M):
            # вставить import os после первого блока докстринга/комментариев —
            # перед первой строкой кода
            lines_out = []
            inserted = False
            for line in lines:
                if not inserted and not line.startswith("#") and not line.startswith("!") and line.strip() and not line.startswith('"""') and not line.startswith("'''"):
                    lines_out.append("import os\n")
                    inserted = True
                lines_out.append(line)
            if not inserted:
                lines_out.append("import os\n")
            lines = lines_out
        with open(path, "w", encoding="utf-8") as f:
            f.writelines(lines)
    print(f"{path}: {changed} замен")
    return changed


if __name__ == "__main__":
    total = 0
    for p in FILES:
        total += scrub(p)
    print(f"Итого замен: {total}")
    # контроль: PAT больше нигде не остались
    import subprocess
    r = subprocess.run(
        ["rg", "-l", "github_pat_[A-Za-z0-9_]{20,}|ghp_[A-Za-z0-9]{20,}"] + FILES,
        capture_output=True, text=True,
    )
    print("Остались в:", r.stdout.strip() or "нигде (чисто)")
