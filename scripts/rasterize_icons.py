#!/usr/bin/env python3
"""
FR-ICONS: растеризация SVG → RGBA и генерация Rust-файла с встроенными байтами.

Берёт 13 иконок × 4 набора (lucide/material/feather/bootstrap) из
`assets/icons/<set>/<name>.svg`, растеризует в RGBA8 (32×32 px для 24-viewbox
наборов, 24×24 px для bootstrap 16-viewbox) и пишет Rust-файл:

  crates/canvas-render/src/icon_data.rs

с массивами `pub const ICON_<SET>_<NAME>_RGBA: &[u8; N] = [...];`
и реестром `pub fn icon_rgba(set, name) -> Option<&'static [u8]>`.

Размеры растеризации:
  - lucide/material/feather: 32×32 (увеличение с 24px viewBox — антиалиасинг
    остаётся чистым; для 13px-кегля кнопки 26×26 попадание в пиксели хорошее).
  - bootstrap: 24×24 (увеличение с 16px viewBox — 50% зум, тот же принцип).

Все байты — `currentColor` заменён наOpaque White (255,255,255,255) —
тint делается в шейдере умножением. Это даёт монохромный «белый силуэт»,
который рендер красит в цвет слота `icon` темы.

wasm-gate (ADR-0011): растеризация в build-time, байты вшиты в бинарник
через `pub const` — никаких runtime FS-доступа, никаких новых runtime-зависимостей.
"""

import re
import sys
from pathlib import Path
import cairosvg
from PIL import Image
import io

ROOT = Path("/home/z/my-project/CanvasDesk")
ICON_DIR = ROOT / "assets" / "icons"
OUT_RS = ROOT / "crates" / "canvas-render" / "src" / "icon_data.rs"

ICON_NAMES = [
    "close", "gear", "question", "search", "plus",
    "arrow_left", "arrow_right", "refresh",
    "tab_general", "tab_canvas", "tab_snap", "tab_edges", "tab_appearance",
    "more", "chevron_down", "chevron_right",
]
# FR-075 W2: роли шаблонных нод — отдельный SPARSE-набор: имена существуют
# только в наборе "roles", у остальных наборов эти ячейки атласа прозрачны.
# Ключи = match-arms template_icon_quads (cards.rs) + "custom" (фолбэк).
ROLE_NAMES = [
    "lb", "db", "cache", "http", "queue", "gateway", "worker",
    "storage", "auth", "grpc", "graphql", "money", "burn", "users",
    "retention", "churn", "funnel", "chart", "clock", "custom",
]
# Имена в атласе = объединение всех наборов (столбцы атласа).
ICON_NAMES_ALL = ICON_NAMES + ROLE_NAMES
# Наборы: 4 UI-стиля + роли. Sparse-наборы — SET_NAMES[set] задаёт свои имена.
SETS = ["lucide", "material", "feather", "bootstrap", "roles"]
SET_NAMES = {
    "lucide": ICON_NAMES,
    "material": ICON_NAMES,
    "feather": ICON_NAMES,
    "bootstrap": ICON_NAMES,
    "roles": ROLE_NAMES,
}
# Размер растеризации в px (для каждого набора).
SET_PX = {
    "lucide": 32,
    "material": 32,
    "feather": 32,
    "bootstrap": 24,
    "roles": 32,
}


def rasterize_svg(svg_path: Path, output_px: int) -> bytes:
    """Rasterize SVG → RGBA8 bytes, length = output_px*output_px*4.

    currentColor заменяется на белый (255,255,255,255); tint делает шейдер.
    cairosvg рендерит `currentColor` как чёрный по умолчанию — мы заменяем
    на `#ffffff` явно перед рендерингом, чтобы получить белый силуэт.
    """
    svg_text = svg_path.read_text(encoding="utf-8")
    # Заменяем currentColor → #ffffff (монохромный белый силуэт).
    svg_text = svg_text.replace("currentColor", "#ffffff")
    # Растеризуем через cairosvg → PNG → RGBA bytes.
    png_bytes = cairosvg.svg2png(
        bytestring=svg_text.encode("utf-8"),
        output_width=output_px,
        output_height=output_px,
    )
    img = Image.open(io.BytesIO(png_bytes)).convert("RGBA")
    if img.size != (output_px, output_px):
        img = img.resize((output_px, output_px), Image.LANCZOS)
    return img.tobytes()


def const_name(set_name: str, icon_name: str) -> str:
    return f"ICON_{set_name.upper()}_{icon_name.upper()}"


def rust_byte_array_literal(data: bytes, name: str, set_name: str, icon_name: str, px: int) -> str:
    """Emit `pub const NAME: &[u8; LEN] = [...];` with hex bytes."""
    lines = [f"/// {set_name}/{icon_name} — rasterized {px}×{px} RGBA8 ({len(data)} bytes)."]
    lines.append(f"pub const {name}: &[u8; {len(data)}] = &[")
    # 12 bytes per line: 0xAA, 0xBB, ...
    CHUNK = 16
    for i in range(0, len(data), CHUNK):
        chunk = data[i:i + CHUNK]
        hex_bytes = ", ".join(f"0x{b:02x}" for b in chunk)
        lines.append(f"    {hex_bytes},")
    lines.append("];")
    return "\n".join(lines)


def main() -> None:
    # Растеризация всех иконок (sparse: набор определяет свои имена).
    rasterized: dict[tuple[str, str], bytes] = {}
    for set_name in SETS:
        px = SET_PX[set_name]
        for icon_name in SET_NAMES[set_name]:
            svg_path = ICON_DIR / set_name / f"{icon_name}.svg"
            if not svg_path.exists():
                print(f"MISSING: {svg_path}", file=sys.stderr)
                sys.exit(1)
            data = rasterize_svg(svg_path, px)
            rasterized[(set_name, icon_name)] = data
            print(f"  rasterized {set_name}/{icon_name} → {len(data)} bytes ({px}×{px})")

    # Генерация Rust-файла.
    parts: list[str] = []
    parts.append("//! FR-ICONS: вшитые RGBA-данные растеризованных SVG-иконок.")
    parts.append("//!")
    parts.append("//! Генерируется скриптом `scripts/rasterize_icons.py` из")
    parts.append("//! `assets/icons/{lucide,material,feather,bootstrap,roles}/*.svg`.")
    parts.append("//!")
    parts.append("//! Все иконки монохромные (белый силуэт на прозрачном фоне) —")
    parts.append("//! tint делается в шейдере умножением на цвет слота `icon` темы.")
    parts.append("//!")
    parts.append("//! Размеры: lucide/material/feather/roles — 32×32, bootstrap — 24×24.")
    parts.append("//! Сетку атласа образует ОБЪЕДИНЕНИЕ имён всех наборов (столбцы =")
    parts.append("//! имена, строки = наборы); отсутствующие пары (set, name) —")
    parts.append("//! прозрачные ячейки (icon_rgba возвращает None — upload пропускает).")
    parts.append("//!")
    parts.append("//! wasm-gate (ADR-0011): байты вшиты в бинарник через `pub const` —")
    parts.append("//! никаких runtime FS-доступа, никаких новых runtime-зависимостей.")
    parts.append("//!")
    parts.append("//! Auto-generated: do not edit by hand.")
    parts.append("")
    parts.append("/// Идентификатор набора (строковый, используется как ключ атласа).")
    parts.append("/// `roles` — sparse-набор ролей шаблонных нод (FR-075 W2): только")
    parts.append("/// свои имена, ячейки других наборов в атласе прозрачны.")
    sets_lit = ", ".join(f'"{s}"' for s in SETS)
    parts.append(f"pub const ICON_SETS: &[&str] = &[{sets_lit}];")
    parts.append("")
    parts.append("/// Размер растеризации набора в px (32 для 24-viewbox наборов,")
    parts.append("/// 24 для bootstrap 16-viewbox).")
    parts.append("pub fn icon_set_px(set: &str) -> u32 {")
    parts.append("    match set {")
    for set_name in SETS:
        parts.append(f"        \"{set_name}\" => {SET_PX[set_name]},")
    parts.append("        _ => 32,")
    parts.append("    }")
    parts.append("}")
    parts.append("")

    # Идентификатор иконки (строковый ключ, используется в Painter API).
    parts.append("/// Список ВСЕХ идентификаторов иконок (объединение имён всех наборов;")
    parts.append("/// порядок = столбцы атласа = индекс в `icon_index`).")
    parts.append("pub const ICON_NAMES: &[&str] = &[")
    for name in ICON_NAMES_ALL:
        parts.append(f"    \"{name}\",")
    parts.append("];")
    parts.append("")

    # Имена конкретного набора (sparse: roles имеет собственные имена).
    parts.append("/// Имена, объявленные набором (sparse-наборы — только свои).")
    parts.append("pub fn set_names(set: &str) -> &'static [&'static str] {")
    parts.append("    match set {")
    for set_name in SETS:
        names_lit = ", ".join(f'"{n}"' for n in SET_NAMES[set_name])
        parts.append(f"        \"{set_name}\" => &[{names_lit}],")
    parts.append("        _ => &[],")
    parts.append("    }")
    parts.append("}")
    parts.append("")

    # Pub const байты для каждой иконки × набор (sparse: только объявленные).
    for set_name in SETS:
        px = SET_PX[set_name]
        for icon_name in SET_NAMES[set_name]:
            data = rasterized[(set_name, icon_name)]
            cname = const_name(set_name, icon_name)
            parts.append(rust_byte_array_literal(data, cname, set_name, icon_name, px))
            parts.append("")

    # Реестр: `(set, name) -> &'static [u8]`.
    parts.append("/// Доступ к растеризованным байтам по (set, name).")
    parts.append("/// Возвращает `None` для неизвестной пары — фолбэк на глиф.")
    parts.append("pub fn icon_rgba(set: &str, name: &str) -> Option<&'static [u8]> {")
    parts.append("    match (set, name) {")
    for set_name in SETS:
        for icon_name in SET_NAMES[set_name]:
            cname = const_name(set_name, icon_name)
            parts.append(f"        (\"{set_name}\", \"{icon_name}\") => Some({cname}),")
    parts.append("        _ => None,")
    parts.append("    }")
    parts.append("}")
    parts.append("")

    # Реестр: для каждого набора — массив всех иконок (для атласа;
    # sparse-набор — только свои имена).
    for set_name in SETS:
        parts.append(f"/// Все иконки набора `{set_name}` (порядок = `ICON_NAMES`).")
        parts.append(f"pub fn {set_name}_icons() -> Vec<(&'static str, &'static [u8])> {{")
        parts.append("    vec![")
        for icon_name in SET_NAMES[set_name]:
            cname = const_name(set_name, icon_name)
            parts.append(f"        (\"{icon_name}\", {cname}),")
        parts.append("    ]")
        parts.append("}")
        parts.append("")

    OUT_RS.parent.mkdir(parents=True, exist_ok=True)
    OUT_RS.write_text("\n".join(parts), encoding="utf-8")
    print(f"\nWrote {OUT_RS} ({OUT_RS.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
