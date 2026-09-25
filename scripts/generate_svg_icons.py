#!/usr/bin/env python3
"""
FR-ICONS: генератор монохромных SVG-иконок для CanvasDesk.

Создаёт 13 иконок × 4 набора (Lucide, Material, Feather, Bootstrap) = 52 файла.
Все SVG монохромные (stroke=currentColor или fill=currentColor), 24×24 (Bootstrap — 16×16).

Иконки покрывают:
  - 8 из kit::Icon enum: close, gear, question, search, plus, arrow_left, arrow_right, refresh
  - 5 иконок табов настроек: tab_general, tab_canvas, tab_snap, tab_edges, tab_appearance

Стили:
  - lucide   — stroke=currentColor, stroke-width=2, linecap=round, linejoin=round, 24×24
  - material — fill=currentColor (filled paths), 24×24
  - feather  — stroke=currentColor, stroke-width=2, linecap=round, linejoin=round, 24×24
               (упрощённые геометрии — более «лёгкие» формы)
  - bootstrap — смешанный стиль (fill + stroke), 16×16 (родной размер Bootstrap Icons)

Источники стилей: Lucide (MIT), Feather (MIT), Bootstrap Icons (MIT),
Material Symbols (Apache 2.0) — все допускают коммерческое использование.
Конкретные path-данные воссозданы по визуальным образцам наборов; полные
наборы этих проектов доступны по URL:
  - https://lucide.dev
  - https://feathericons.com
  - https://icons.getbootstrap.com
  - https://fonts.google.com/icons
"""

from pathlib import Path

ROOT = Path("/home/z/my-project/CanvasDesk/assets/icons")

# --- Иконки в 4 стилях ---------------------------------------------------

# Каждая иконка = функция, возвращающая (svg_inner_xml, viewbox_size).
# viewbox_size: 24 (lucide/material/feather) или 16 (bootstrap).

# Lucide-style: тонкие линии, stroke-width=2
LUCIDE = {
    "close": '<path d="M18 6 6 18"/><path d="m6 6 12 12"/>',
    "gear": (
        '<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08'
        'a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74'
        'l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1'
        ' 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08'
        'a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74'
        'l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0'
        ' 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>'
        '<circle cx="12" cy="12" r="3"/>'
    ),
    "question": '<circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><path d="M12 17h.01"/>',
    "search": '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>',
    "plus": '<path d="M5 12h14"/><path d="M12 5v14"/>',
    "arrow_left": '<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>',
    "arrow_right": '<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>',
    "refresh": '<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>',
    "tab_general": '<circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="2"/>',
    "tab_canvas": '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M3 9h18"/><path d="M9 21V9"/>',
    "tab_snap": '<path d="M4 6h16"/><path d="M4 12h16"/><path d="M4 18h16"/>',
    "tab_edges": '<path d="M8 3 4 7l4 4"/><path d="M4 7h16"/><path d="m16 21 4-4-4-4"/><path d="M20 17H4"/>',
    "tab_appearance": '<path d="M12 22a10 10 0 1 1 0-20 10 10 0 0 1 0 20z"/><path d="M12 2v20"/>',
}

# Material-style: заполненные пути (filled), 24×24
MATERIAL = {
    "close": '<path d="M18.3 5.71 12 12.01l-6.3-6.3-1.42 1.41L10.59 13.4 4.3 19.7l1.41 1.42 6.3-6.3 6.29 6.3 1.42-1.42-6.3-6.29 6.3-6.3z"/>',
    "gear": (
        '<path d="M19.14 12.94c.04-.3.06-.61.06-.94 0-.32-.02-.64-.07-.94l2.03-1.58a.49.49 0 0 0 .12-.61l-1.92-3.32a.488.488 0 0 0-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54a.484.484 0 0 0-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.05.3-.09.63-.09.94s.02.64.07.94l-2.03 1.58a.49.49 0 0 0-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.21.07-.47-.12-.61l-2.01-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z"/>'
    ),
    "question": '<path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 17h-2v-2h2v2zm2.07-7.75-.9.92C13.45 12.9 13 13.5 13 15h-2v-.5c0-1.1.45-2.1 1.17-2.83l1.24-1.26c.37-.36.59-.86.59-1.41 0-1.1-.9-2-2-2s-2 .9-2 2H8c0-2.21 1.79-4 4-4s4 1.79 4 4c0 .88-.36 1.68-.93 2.25z"/>',
    "search": '<path d="M15.5 14h-.79l-.28-.27a6.5 6.5 0 1 0-.7.7l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z"/>',
    "plus": '<path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"/>',
    "arrow_left": '<path d="M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z"/>',
    "arrow_right": '<path d="M4 11h12.17l-5.59-5.59L12 4l8 8-8 8-1.41-1.41L16.17 13H4v-2z"/>',
    "refresh": '<path d="M17.65 6.35A7.958 7.958 0 0 0 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08A5.99 5.99 0 0 1 12 18c-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"/>',
    "tab_general": '<path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 15c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5zm0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3z"/>',
    "tab_canvas": '<path d="M3 3h18v18H3V3zm2 2v14h14V5H5zm2 2h10v3H7V7zm0 5h4v7H7v-7z"/>',
    "tab_snap": '<path d="M3 18h18v2H3v-2zm0-7h18v3H3v-3zm0-7h18v4H3V4z"/>',
    "tab_edges": '<path d="M9 3 5 7l4 4V8h11V6H9V3zm6 18 4-4-4-4v3H4v2h11v3z"/>',
    "tab_appearance": '<path d="M12 2a10 10 0 1 0 0 20V2z"/><path d="M12 2v20" fill="none" stroke="currentColor" stroke-width="0"/>',
}

# Feather-style: тонкие линии, как Lucide, но с упрощённой геометрией
FEATHER = {
    "close": '<line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>',
    "gear": (
        '<circle cx="12" cy="12" r="3"/>'
        '<path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>'
    ),
    "question": '<circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><line x1="12" y1="17" x2="12.01" y2="17"/>',
    "search": '<circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>',
    "plus": '<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>',
    "arrow_left": '<line x1="19" y1="12" x2="5" y2="12"/><polyline points="12 19 5 12 12 5"/>',
    "arrow_right": '<line x1="5" y1="12" x2="19" y2="12"/><polyline points="12 5 19 12 12 19"/>',
    "refresh": (
        '<polyline points="23 4 23 10 17 10"/>'
        '<polyline points="1 20 1 14 7 14"/>'
        '<path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>'
    ),
    "tab_general": '<circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="3"/>',
    "tab_canvas": '<rect x="3" y="3" width="18" height="18" rx="2" ry="2"/><line x1="3" y1="9" x2="21" y2="9"/><line x1="9" y1="21" x2="9" y2="9"/>',
    "tab_snap": '<line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/>',
    "tab_edges": '<polyline points="7 17 3 13 7 9"/><polyline points="17 7 21 11 17 15"/><line x1="3" y1="13" x2="21" y2="13"/><line x1="3" y1="11" x2="21" y2="11"/>',
    "tab_appearance": '<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>',
}

# Bootstrap-style: 16×16 viewbox, более компактные пути (mix fill/stroke)
BOOTSTRAP = {
    "close": '<path d="M4.646 4.646a.5.5 0 0 1 .708 0L8 7.293l2.646-2.647a.5.5 0 0 1 .708.708L8.707 8l2.647 2.646a.5.5 0 0 1-.708.708L8 8.707l-2.646 2.647a.5.5 0 0 1-.708-.708L7.293 8 4.646 5.354a.5.5 0 0 1 0-.708"/>',
    "gear": (
        '<path d="M8 4.754a3.246 3.246 0 1 0 0 6.492 3.246 3.246 0 0 0 0-6.492M5.754 8a2.246 2.246 0 1 1 4.492 0 2.246 2.246 0 0 1-4.492 0"/>'
        '<path d="M9.5 2.34a.5.5 0 0 1 .47.516l-.19 1.508a5.5 5.5 0 0 1 1.016.587l1.275-.812a.5.5 0 0 1 .65.122l.97 1.68a.5.5 0 0 1-.18.681l-1.276.81c.06.336.09.68.09 1.029 0 .348-.03.692-.09 1.028l1.275.81a.5.5 0 0 1 .18.682l-.97 1.68a.5.5 0 0 1-.65.122l-1.275-.812a5.5 5.5 0 0 1-1.016.587l.19 1.508a.5.5 0 0 1-.47.516h-1.94a.5.5 0 0 1-.47-.516l.19-1.508a5.5 5.5 0 0 1-1.016-.587l-1.275.812a.5.5 0 0 1-.65-.122l-.97-1.68a.5.5 0 0 1 .18-.681l1.276-.81a5.5 5.5 0 0 1-.09-1.029c0-.348.03-.692.09-1.028l-1.275-.81a.5.5 0 0 1-.18-.682l.97-1.68a.5.5 0 0 1 .65-.122l1.275.812a5.5 5.5 0 0 1 1.016-.587l-.19-1.508a.5.5 0 0 1 .47-.516h1.94Z"/>'
    ),
    "question": '<path d="M8 15A7 7 0 1 1 8 1a7 7 0 0 1 0 14m0 1A8 8 0 1 0 8 0a8 8 0 0 0 0 16"/><path d="M5.255 5.786a.237.237 0 0 0 .241.247h.825c.138 0 .248-.113.266-.25.09-.656.54-1.134 1.342-1.134.686 0 1.314.343 1.314 1.168 0 .635-.374.927-.965 1.371-.673.489-1.206 1.06-1.168 1.987l.003.217a.25.25 0 0 0 .25.246h.811a.25.25 0 0 0 .25-.25v-.105c0-.718.273-.927 1.01-1.486.609-.463 1.244-.977 1.244-2.056 0-1.511-1.276-2.241-2.673-2.241-1.267 0-2.655.59-2.75 2.286m1.557 5.763c0 .433.36.78.78.78.78 0 .78-.347.78-.78 0-.433-.36-.78-.78-.78-.42 0-.78.347-.78.78"/>',
    "search": '<path d="M11.742 10.344a6.5 6.5 0 1 0-1.397 1.398h-.001c.03.04.062.078.098.115l3.85 3.85a1 1 0 0 0 1.415-1.414l-3.85-3.85a1.007 1.007 0 0 0-.115-.1zM12 6.5a5.5 5.5 0 1 1-11 0 5.5 5.5 0 0 1 11 0"/>',
    "plus": '<path d="M8 4a.5.5 0 0 1 .5.5v3h3a.5.5 0 0 1 0 1h-3v3a.5.5 0 0 1-1 0v-3h-3a.5.5 0 0 1 0-1h3v-3A.5.5 0 0 1 8 4"/>',
    "arrow_left": '<path fill-rule="evenodd" d="M15 8a.5.5 0 0 0-.5-.5H2.707l3.147-3.146a.5.5 0 1 0-.708-.708l-4 4a.5.5 0 0 0 0 .708l4 4a.5.5 0 0 0 .708-.708L2.707 8.5H14.5A.5.5 0 0 0 15 8"/>',
    "arrow_right": '<path fill-rule="evenodd" d="M1 8a.5.5 0 0 1 .5-.5h11.793L10.146 4.354a.5.5 0 1 1 .708-.708l4 4a.5.5 0 0 1 0 .708l-4 4a.5.5 0 0 1-.708-.708L13.293 8.5H1.5A.5.5 0 0 1 1 8"/>',
    "refresh": '<path d="M8 3a5 5 0 1 0 4.546 2.914.5.5 0 0 1 .908-.417A6 6 0 1 1 8 2v1z"/><path d="M8 4.466V.534a.25.25 0 0 1 .41-.192l2.36 1.966c.12.1.12.284 0 .384L8.41 4.658A.25.25 0 0 1 8 4.466"/>',
    "tab_general": '<path d="M8 15A7 7 0 1 1 8 1a7 7 0 0 1 0 14m0 1A8 8 0 1 0 8 0a8 8 0 0 0 0 16"/><path d="M8 6a2 2 0 1 0 0 4 2 2 0 0 0 0-4"/>',
    "tab_canvas": '<path d="M2.5 2A1.5 1.5 0 0 0 1 3.5v9A1.5 1.5 0 0 0 2.5 14h11a1.5 1.5 0 0 0 1.5-1.5v-9A1.5 1.5 0 0 0 13.5 2zM2 3.5a.5.5 0 0 1 .5-.5h11a.5.5 0 0 1 .5.5v9a.5.5 0 0 1-.5.5h-11a.5.5 0 0 1-.5-.5zM5 5h6v1H5zM5 7h6v1H5zM5 9h3v1H5z"/>',
    "tab_snap": '<path d="M2 4h12v1H2zm0 3h12v1H2zm0 3h12v1H2zm0 3h12v1H2z"/>',
    "tab_edges": '<path d="M1 4.5 4 7v-2h8v2l3-2.5L12 2v2H4V2zM1 11.5 4 9v2h8V9l3 2.5L12 14v-2H4v2z"/>',
    "tab_appearance": '<path d="M8 15A7 7 0 1 1 8 1a7 7 0 0 1 0 14m0 1A8 8 0 1 0 8 0a8 8 0 0 0 0 16"/><path d="M8 1v14" fill="none" stroke="currentColor" stroke-width="0"/>',
}

# Какой тип атрибута использовать (stroke / fill / mixed)
SET_ATTRS = {
    "lucide":    ('stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" fill="none"', 24),
    "material":  ('fill="currentColor"', 24),
    "feather":   ('stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" fill="none"', 24),
    "bootstrap": ('fill="currentColor"', 16),
}

SETS = {
    "lucide":    LUCIDE,
    "material":  MATERIAL,
    "feather":   FEATHER,
    "bootstrap": BOOTSTRAP,
}

ICON_NAMES = [
    "close", "gear", "question", "search", "plus",
    "arrow_left", "arrow_right", "refresh",
    "tab_general", "tab_canvas", "tab_snap", "tab_edges", "tab_appearance",
]


def write_svg(set_name: str, icon_name: str, body: str, attrs: str, viewbox: int) -> Path:
    """Write a monochrome SVG file. Returns path."""
    out_dir = ROOT / set_name
    out_dir.mkdir(parents=True, exist_ok=True)
    path = out_dir / f"{icon_name}.svg"
    svg = (
        f'<svg xmlns="http://www.w3.org/2000/svg" '
        f'width="{viewbox}" height="{viewbox}" viewBox="0 0 {viewbox} {viewbox}" '
        f'{attrs}>{body}</svg>\n'
    )
    path.write_text(svg, encoding="utf-8")
    return path


def main() -> None:
    written = 0
    for set_name, icons in SETS.items():
        attrs, viewbox = SET_ATTRS[set_name]
        for name in ICON_NAMES:
            body = icons[name]
            write_svg(set_name, name, body, attrs, viewbox)
            written += 1
    print(f"Wrote {written} SVG files to {ROOT}")
    # Sanity: list per set
    for set_name in SETS:
        d = ROOT / set_name
        files = sorted(d.glob("*.svg"))
        print(f"  {set_name}: {len(files)} files")


if __name__ == "__main__":
    main()
