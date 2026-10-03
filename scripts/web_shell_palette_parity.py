#!/usr/bin/env python3
"""Паритет shell-палитры web-оболочки после W-f (аудит ui-kit §6).

W-f заменил литеральные цвета DOM-палитр `crates/canvas-web/index.html`
и `crates/canvas-web/src/gpu_gate.rs` на CSS-переменные `--cd-*`
(блок `:root`). Значения переменных — байт-в-байт прежние литералы,
НИКАКИХ новых цветов. Скрипт доказывает паритет на статике:

  1. Разбирает CSS (оба <style>-блока index.html + JS-константа
     DEFAULT_STYLES инлайн-бандла тура) в старой (git HEAD) и новой
     версии файла; для каждого (media, селектор, свойство) сравнивает
     ЭФФЕКТИВНОЕ значение цвета: var(--cd-*) резолвится через таблицу
     кастомных свойств (включая :root), hex нормализуется в rgb().
  2. Проверяет политику слотов: после консолидации в CSS-регионах не
     осталось литералов вне `:root` и fallback'ов `var(--cd-*, литерал)`
     (плюс cssVar-fallback DIM_DEFAULTS тура).
  3. Проверяет, что набор значений `:root --cd-*` в точности равен
     набору прежних литералов (ничего не потеряно/не добавлено).
  4. Проверяет DIM_DEFAULTS тура: cssVar-fallback = старый литерал =
     значение --cd-dim из :root.
  5. gpu_gate.rs: инлайн-стиль заглушки использует var(--cd-bg,#14161a)
     / var(--cd-text,#d5d9e0), без голых литералов вне fallback.
  6. node --check для каждого <script>-блока index.html (синтаксис
     инлайн-бандла тура и шимов не сломан).

Запуск:  python3 scripts/web_shell_palette_parity.py
Выход:   0 — паритет подтверждён; 1 — найдены расхождения.
Зависимости: git (для старой версии), node (опционально — п.6,
             при отсутствии node проверка пропускается с пометкой).
"""
from __future__ import annotations

import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INDEX = ROOT / "crates" / "canvas-web" / "index.html"
GPU_GATE = ROOT / "crates" / "canvas-web" / "src" / "gpu_gate.rs"

LIT_RE = re.compile(r"#[0-9a-fA-F]{3,8}\b|rgba?\(")
LIT_FULL_RE = re.compile(r"#[0-9a-fA-F]{3,8}\b|rgba?\([^()]*\)")
HEX_RE = re.compile(r"#([0-9a-fA-F]{3,8})\b")
PROP_SPLIT_RE = re.compile(r"[a-zA-Z-]+\s*:")


# ── извлечение CSS-регионов ─────────────────────────────────────────
def read_old_new() -> tuple[str, str]:
    new = INDEX.read_text(encoding="utf-8")
    try:
        old = subprocess.run(
            ["git", "show", f"HEAD:{INDEX.relative_to(ROOT)}"],
            cwd=ROOT, check=True, capture_output=True, text=True,
        ).stdout
    except (subprocess.CalledProcessError, FileNotFoundError):
        env_old = sys.argv[1] if len(sys.argv) > 1 else None
        if env_old:
            old = Path(env_old).read_text(encoding="utf-8")
        else:
            print("WARN: git show недоступен — нужна старая версия аргументом")
            raise
    return old, new


def strip_html_comments(text: str) -> str:
    return re.sub(r"<!--.*?-->", "", text, flags=re.S)


def style_blocks(html: str) -> list[str]:
    return re.findall(r"<style>(.*?)</style>", html, flags=re.S)


def script_blocks(html: str) -> list[str]:
    # Только РЕАЛЬНЫЕ теги (в index.html они стоят в начале строки);
    # внутри JS-комментария бандла встречается текст «<script> tag …» —
    # он посреди строки и матчится не должен.
    return re.findall(r"(?m)^[ \t]*<script\b[^>]*>(.*?)</script\s*>", html, flags=re.S)


def bundle_styles_css(html: str) -> str:
    """DEFAULT_STYLES инлайн-бандла тура: склеить строки JS-массива."""
    m = re.search(r"var DEFAULT_STYLES = \[(.*?)\]\.join\(\"\"\);", html, flags=re.S)
    if not m:
        return ""
    parts = re.findall(r'"((?:[^"\\]|\\.)*)"', m.group(1), flags=re.S)
    return "".join(p.replace('\\"', '"') for p in parts)


def dim_defaults_fallback(html: str) -> str | None:
    m = re.search(r'dimColor:\s*cssVar\(\s*"(--cd-[a-z0-9-]+)"\s*,\s*"([^"]+)"\s*\)', html)
    return m.group(2) if m else None


def old_dim_defaults_literal(html: str) -> str | None:
    m = re.search(r'dimColor:\s*"([^"]+)"', html)
    return m.group(1) if m else None


# ── мини-парсер CSS (плоские правила + один уровень @media) ─────────
def strip_css_comments(css: str) -> str:
    return re.sub(r"/\*.*?\*/", "", css, flags=re.S)


def parse_css(css: str, media: str = "") -> list[tuple[str, str, str]]:
    """[(media, selector, decls_body)]"""
    rules: list[tuple[str, str, str]] = []
    i, n = 0, len(css)
    while i < n:
        j = css.find("{", i)
        if j == -1:
            break
        sel = css[i:j].strip()
        depth, k = 1, j + 1
        while k < n and depth:
            if css[k] == "{":
                depth += 1
            elif css[k] == "}":
                depth -= 1
            k += 1
        body = css[j + 1 : k - 1]
        if sel.startswith("@media"):
            rules += parse_css(body, sel)
        elif sel.startswith("@"):
            rules += parse_css(body, media)  # @supports и пр. — редко
        else:
            rules.append((media, sel, body))
        i = k
    return rules


def decls(body: str) -> list[tuple[str, str]]:
    out = []
    for part in body.split(";"):
        if ":" not in part:
            continue
        prop, val = part.split(":", 1)
        prop, val = prop.strip(), val.strip()
        if prop and " " not in prop and "(" not in prop:
            out.append((prop, val))
    return out


def build_varmap(rules) -> dict[str, str]:
    vm: dict[str, str] = {}
    for _media, _sel, body in rules:
        for prop, val in decls(body):
            if prop.startswith("--"):
                vm[prop] = val
    return vm


def split_top(text: str) -> list[str]:
    parts, buf, depth = [], [], 0
    for ch in text:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append("".join(buf))
            buf = []
        else:
            buf.append(ch)
    parts.append("".join(buf))
    return parts


def resolve_vars(value: str, vm: dict[str, str], depth: int = 0) -> str:
    if depth > 16 or "var(" not in value:
        return value
    out, i = [], 0
    while True:
        j = value.find("var(", i)
        if j == -1:
            out.append(value[i:])
            return "".join(out)
        out.append(value[i:j])
        k, d = j + 4, 1
        while k < len(value) and d:
            if value[k] == "(":
                d += 1
            elif value[k] == ")":
                d -= 1
            k += 1
        inner = value[j + 4 : k - 1]
        parts = split_top(inner)
        name = parts[0].strip()
        if name in vm:
            out.append(resolve_vars(vm[name], vm, depth + 1))
        elif len(parts) > 1:
            out.append(resolve_vars(parts[1].strip(), vm, depth + 1))
        else:
            out.append(value[j:k])  # неразрешённый var — оставить как есть
        i = k


def normalize(value: str, vm: dict[str, str]) -> str:
    v = resolve_vars(value, vm)
    v = re.sub(r"\s+", " ", v).strip()
    v = re.sub(r"\s*,\s*", ",", v)

    def hexrepl(m: re.Match) -> str:
        h = m.group(1)
        if len(h) in (3, 4):
            h = "".join(c * 2 for c in h)
        if len(h) == 6:
            return f"rgb({int(h[0:2],16)},{int(h[2:4],16)},{int(h[4:6],16)})"
        if len(h) == 8:
            return (
                f"rgba({int(h[0:2],16)},{int(h[2:4],16)},{int(h[4:6],16)},"
                f"{int(h[6:8],16)/255:g})"
            )
        return m.group(0)

    v = HEX_RE.sub(hexrepl, v)
    return v.lower()


def css_regions(html: str) -> list[str]:
    regions = style_blocks(html)
    bundle = bundle_styles_css(html)
    if bundle:
        regions.append(bundle)
    return [strip_css_comments(r) for r in regions]


def build_map(html: str) -> tuple[dict[tuple[str, str, str], str], list[str]]:
    """Карта (media, selector, prop) → нормализованное значение + все литералы."""
    rules: list[tuple[str, str, str]] = []
    for region in css_regions(html):
        rules += parse_css(region)
    vm = build_varmap(rules)
    out: dict[tuple[str, str, str], str] = {}
    literals: list[str] = []
    for media, sel, body in rules:
        for m in LIT_FULL_RE.finditer(body):
            literals.append(m.group(0))
        for prop, val in decls(body):
            if prop.startswith("--"):
                continue
            key = (media.strip(), sel, prop)
            out[key] = normalize(val, vm)
    return out, literals


def root_custom_props(html: str) -> dict[str, str]:
    """Значения --cd-* (и прочих custom props) из всех CSS-регионов."""
    vm: dict[str, str] = {}
    for region in css_regions(html):
        for media, sel, body in parse_css(region):
            for prop, val in decls(body):
                if prop.startswith("--"):
                    vm.setdefault(prop, val)
    return vm


# ── политика «литералов вне слотов не осталось» ─────────────────────
def policy_violations(html: str) -> list[str]:
    bad: list[str] = []
    for region in css_regions(html):
        text = strip_css_comments(region)
        # вырезаем :root-блок (там живут определения слотов)
        text = re.sub(
            r"(?s)(^|\})\s*:root\s*\{[^{}]*\}", r"\1", text
        )
        # вырезаем var(--cd-*, fallback) целиком (включая rgba-fallback)
        while True:
            new_text = re.sub(
                r"var\(--cd-[a-z0-9-]+\s*,(?:[^()]|\([^()]*\))*\)", "", text
            )
            if new_text == text:
                break
            text = new_text
        for m in LIT_RE.finditer(text):
            ctx = text[max(0, m.start() - 40) : m.end() + 40].replace("\n", " ")
            bad.append(f"index.html CSS: «{m.group(0)}» рядом с …{ctx}…")
    return bad


def gpu_policy_violations(src: str) -> list[str]:
    m = re.search(r'set_attribute\(\s*"style",\s*"((?:[^"\\]|\\.)*)"', src, flags=re.S)
    if not m:
        return ["gpu_gate.rs: инлайн-стиль заглушки не найден"]
    style = m.group(1)
    text = re.sub(r"var\(--cd-[a-z0-9-]+\s*,(?:[^()]|\([^()]*\))*\)", "", style)
    return [
        f"gpu_gate.rs: голый литерал «{x.group(0)}»"
        for x in LIT_RE.finditer(text)
    ]


# ── node --check ────────────────────────────────────────────────────
def node_check_scripts(html: str) -> tuple[int, list[str]]:
    if shutil.which("node") is None:
        return 0, ["node не найден — синтаксис JS не проверялся (SKIP)"]
    errors: list[str] = []
    checked = 0
    with tempfile.TemporaryDirectory() as td:
        for i, js in enumerate(script_blocks(html)):
            p = Path(td) / f"script_{i}.js"
            p.write_text(js, encoding="utf-8")
            r = subprocess.run(
                ["node", "--check", str(p)], capture_output=True, text=True
            )
            checked += 1
            if r.returncode != 0:
                errors.append(f"script #{i}: {r.stderr.strip()[:400]}")
    return checked, errors


# ── main ────────────────────────────────────────────────────────────
def main() -> int:
    old_html, new_html = read_old_new()
    failures: list[str] = []
    notes: list[str] = []

    # 1. Паритет деклараций old vs new
    old_map, old_lits = build_map(old_html)
    new_map, new_lits = build_map(new_html)
    # дефолт затемнения тура в старой версии жил в JS-конфиге DIM_DEFAULTS —
    # это тоже прежний литерал палитры (ср. проверку №4 ниже)
    old_dim = old_dim_defaults_literal(old_html)
    if old_dim:
        old_lits.append(old_dim)
    matched = 0
    for key, old_val in old_map.items():
        new_val = new_map.get(key)
        if new_val == old_val:
            matched += 1
        elif new_val is None:
            failures.append(f"ПРОПАЛО правило: {key} = {old_val}")
        else:
            failures.append(f"РАСХОЖДЕНИЕ {key}: было {old_val}, стало {new_val}")
    notes.append(
        f"паритет деклараций: совпало {matched}/{len(old_map)} "
        f"(media+селектор+свойство), расходится {len(old_map) - matched}"
    )

    # 2. Политика слотов в новой версии
    pol = policy_violations(new_html) + gpu_policy_violations(GPU_GATE.read_text("utf-8"))
    failures += pol
    notes.append(f"литералов вне :root/fallback в CSS-регионах: {len(pol)}")

    # 3. Слот-таблица == прежняя палитра (туда и обратно)
    old_unique = {l.lower() for l in old_lits}
    root_vals = {
        v.lower()
        for k, v in root_custom_props(new_html).items()
        if k.startswith("--cd-")
    }
    # #fff и #ffffff — один цвет; сравниваем нормализованные значения
    def normlit(s: str) -> str:
        return normalize(s, {})

    old_unique_n = {normlit(l) for l in old_unique}
    root_vals_n = {normlit(v) for v in root_vals}
    lost = old_unique_n - root_vals_n
    added = root_vals_n - old_unique_n
    if lost:
        failures.append(f"литералы без слота --cd-*: {sorted(lost)}")
    if added:
        failures.append(f"НОВЫЕ цвета в :root (запрещено): {sorted(added)}")
    notes.append(
        f"слотов --cd-*: {len(root_vals)}; прежних уникальных литералов: "
        f"{len(old_unique)}; потери {len(lost)}, новые {len(added)}"
    )

    # 4. DIM_DEFAULTS
    new_fb = dim_defaults_fallback(new_html)
    old_dim = old_dim_defaults_literal(old_html)
    root_dim = root_custom_props(new_html).get("--cd-dim")
    if new_fb is None:
        failures.append("DIM_DEFAULTS: cssVar(--cd-dim, …) не найден")
    elif old_dim is not None and new_fb != old_dim:
        failures.append(f"DIM_DEFAULTS fallback {new_fb} != старый литерал {old_dim}")
    elif root_dim is not None and new_fb != root_dim:
        failures.append(f"DIM_DEFAULTS fallback {new_fb} != :root --cd-dim {root_dim}")
    else:
        notes.append(f"DIM_DEFAULTS: cssVar(--cd-dim, {new_fb}) согласован")

    # 5. node --check всех <script>
    checked, js_errors = node_check_scripts(new_html)
    failures += [f"node --check: {e}" for e in js_errors]
    notes.append(
        f"node --check: {checked} скриптов, ошибок {len(js_errors)}"
        if checked
        else "node --check: SKIP"
    )

    print("=== W-f web shell palette parity ===")
    for n in notes:
        print(f"  • {n}")
    if failures:
        print("FAIL:")
        for f in failures:
            print(f"  ✗ {f}")
        return 1
    print("OK: визуальный паритет подтверждён (эффективные цвета идентичны)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
