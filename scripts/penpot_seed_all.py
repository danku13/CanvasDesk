#!/usr/bin/env python3
"""
CanvasDesk → Penpot: сидер всех поверхностей и элементов UI.

Создаёт 3 новые страницы в файле CanvasDesk-DesignSystem:
1. "UI Kit — Atoms" — 10 атомарных компонентов
2. "Canvas Elements" — 4 канвас-элемента
3. "Surfaces" — 6 поверхностей приложения

Каждый компонент — demo board с default + 2-3 ключевыми состояниями.
Источник спецификаций: design/use-cases/*.md (exact sizes/colors/states).
Idempotent: повторный прогон удаляет старые boards с теми же именами
и пересоздаёт. Library colors/typographies skip'аются по имени.

Запуск:
  PENPOT_MCP_TOKEN=... python scripts/penpot_mcp.py seed-all
"""
from __future__ import annotations

import json
from typing import Any

from penpot_mcp import PenpotMCP


# ───────────────── Цвета (для использования в спецификациях) ─────────────────
C = {
    "accent":      "#65A0F7",
    "accent_18":   "#65A0F72E",  # α 0.18
    "accent_50":   "#65A0F780",
    "primary":     "#285299",
    "primary_h":   "#3F75D6",
    "secondary":   "#333B4A",
    "danger":      "#E55C5C",
    "error":       "#E55C5C",
    "ink":         "#1f2937",
    "ink_muted":   "#6b7280",
    "ink_disabled":"#9ca3af",
    "text_dark":   "#E8ECF4",
    "text_muted":  "#B6BECE",
    "white":       "#ffffff",
    "panel_fill":  "#1a1f29F2",  # 0.97 alpha
    "border":      "#3a445866",  # 0.40 alpha
    "control_fill":"#2b334080",
    "hover_fill":  "#3D5070E6",
    "selected":    "#2E4A7AF2",
    "search_input":"#0d1117",
    "edge_default": "#8594A8",
    "edge_flow":   "#21A88C",
    "broken":      "#737373",
    "warning":     "#F5A623",
    "critical":    "#FF4530",
    "wheel_dim":   "#00000059",
    "wheel_cat":   "#2B2E38EB",
    "wheel_hover": "#2E4A7AF2",
    "chip_dim":    "#242A33",
    "whatif_badge":"#DFA63E",
    "teal_18":     "#21A88C2E",
    "amber_18":    "#F5A6232E",
    "red_18":      "#E55C5C2E",
}

# ───────────────── Динамические позиции (по доске) ─────────────────
# Каждая board имеет: title (rect+text в шапке), затем state-варианты в строках.
# Layout-хелперы ниже.

def state_row(local_x: int, local_y: int, w: int, h: int, label: str,
              shapes: list[dict]) -> list[dict]:
    """Группа форм, представляющая одно состояние компонента с подписью."""
    out = [
        {"kind": "text", "x": local_x, "y": local_y, "w": w, "h": 16,
         "text": label, "font": "Noto Sans Mono", "size": 10, "weight": "400",
         "line": 14, "color": C["ink_muted"]},
    ]
    for s in shapes:
        s2 = dict(s)
        s2["x"] = local_x + s.get("dx", 0)
        s2["y"] = local_y + 18 + s.get("dy", 0)
        out.append(s2)
    return out


# ───────────────── 1. UI KIT — 10 атомарных компонентов ─────────────────

UI_KIT_PAGE = {
    "name": "UI Kit — Atoms",
    "boards": [
        # ─── 1. Button ───
        {
            "name": "01. Button",
            "x": 0, "y": 0, "w": 700, "h": 280,
            "states": [
                state_row(20, 50, 660, 60, "primary",
                    [{"kind":"rect","dx":0,"dy":0,"w":120,"h":30,"fill":C["primary"],"radius":6},
                     {"kind":"text","dx":0,"dy":7,"w":120,"h":30,"text":"Save","font":"Noto Sans Display","size":14,"weight":"500","line":18,"color":"#FFFFFF","align":"center"}]),
                state_row(20, 130, 660, 60, "secondary",
                    [{"kind":"rect","dx":0,"dy":0,"w":120,"h":30,"fill":C["secondary"],"radius":6},
                     {"kind":"text","dx":0,"dy":7,"w":120,"h":30,"text":"Cancel","font":"Noto Sans Display","size":14,"weight":"500","line":18,"color":"#FFFFFF","align":"center"}]),
                state_row(20, 210, 660, 60, "danger",
                    [{"kind":"rect","dx":0,"dy":0,"w":120,"h":30,"fill":C["danger"],"radius":6},
                     {"kind":"text","dx":0,"dy":7,"w":120,"h":30,"text":"Delete","font":"Noto Sans Display","size":14,"weight":"500","line":18,"color":"#FFFFFF","align":"center"}]),
            ],
        },
        # ─── 2. Chip & Badge ───
        {
            "name": "02. Chip & Badge",
            "x": 750, "y": 0, "w": 700, "h": 280,
            "states": [
                state_row(20, 50, 660, 60, "chip normal/hover/selected",
                    [{"kind":"rect","dx":0,"dy":4,"w":80,"h":24,"fill":C["control_fill"],"radius":6},
                     {"kind":"text","dx":0,"dy":9,"w":80,"h":24,"text":"filter","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":C["ink"],"align":"center"},
                     {"kind":"rect","dx":100,"dy":4,"w":80,"h":24,"fill":C["hover_fill"],"radius":6},
                     {"kind":"text","dx":100,"dy":9,"w":80,"h":24,"text":"hover","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":C["ink"],"align":"center"},
                     {"kind":"rect","dx":200,"dy":4,"w":90,"h":24,"fill":C["selected"],"radius":6},
                     {"kind":"text","dx":200,"dy":9,"w":90,"h":24,"text":"selected","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#FFFFFF","align":"center"}]),
                state_row(20, 130, 660, 60, "whatif chip (active/dim) + delta badge",
                    [{"kind":"rect","dx":0,"dy":4,"w":100,"h":26,"fill":"#4C8CF2","radius":6},
                     {"kind":"text","dx":0,"dy":9,"w":100,"h":26,"text":"scenario A","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#FFFFFF","align":"center"},
                     {"kind":"rect","dx":120,"dy":4,"w":100,"h":26,"fill":C["chip_dim"],"radius":6},
                     {"kind":"text","dx":120,"dy":9,"w":100,"h":26,"text":"scenario B","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":C["text_muted"],"align":"center"},
                     {"kind":"rect","dx":240,"dy":4,"w":60,"h":18,"fill":C["whatif_badge"],"radius":9},
                     {"kind":"text","dx":240,"dy":5,"w":60,"h":18,"text":"+5.2%","font":"Noto Sans Mono","size":10,"weight":"700","line":12,"color":"#1f2937","align":"center"}]),
                state_row(20, 210, 660, 60, "severity badges (warning/danger/critical)",
                    [{"kind":"rect","dx":0,"dy":4,"w":80,"h":18,"fill":C["warning"],"radius":9},
                     {"kind":"text","dx":0,"dy":5,"w":80,"h":18,"text":"warning","font":"Noto Sans Mono","size":10,"weight":"700","line":12,"color":"#1f2937","align":"center"},
                     {"kind":"rect","dx":100,"dy":4,"w":80,"h":18,"fill":C["danger"],"radius":9},
                     {"kind":"text","dx":100,"dy":5,"w":80,"h":18,"text":"danger","font":"Noto Sans Mono","size":10,"weight":"700","line":12,"color":"#FFFFFF","align":"center"},
                     {"kind":"rect","dx":200,"dy":4,"w":80,"h":18,"fill":C["critical"],"radius":9},
                     {"kind":"text","dx":200,"dy":5,"w":80,"h":18,"text":"critical","font":"Noto Sans Mono","size":10,"weight":"700","line":12,"color":"#FFFFFF","align":"center"}]),
            ],
        },
        # ─── 3. Text-field ───
        {
            "name": "03. Text-field",
            "x": 0, "y": 320, "w": 700, "h": 220,
            "states": [
                state_row(20, 50, 660, 60, "default (search_input_fill + palette_border)",
                    [{"kind":"rect","dx":0,"dy":0,"w":280,"h":36,"fill":C["search_input"],"radius":6,"stroke":C["border"],"stroke_w":1},
                     {"kind":"text","dx":10,"dy":10,"w":200,"h":18,"text":"Search nodes...","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":C["ink_muted"]}]),
                state_row(20, 130, 660, 60, "focus (accent border)",
                    [{"kind":"rect","dx":0,"dy":0,"w":280,"h":36,"fill":C["search_input"],"radius":6,"stroke":C["accent"],"stroke_w":2},
                     {"kind":"text","dx":10,"dy":10,"w":200,"h":18,"text":"que|","font":"Noto Sans Mono","size":13,"weight":"400","line":18,"color":C["ink"]}]),
            ],
        },
        # ─── 4. Switch ───
        {
            "name": "04. Switch",
            "x": 750, "y": 320, "w": 700, "h": 220,
            "states": [
                state_row(20, 50, 660, 60, "off (control_fill, knob left)",
                    [{"kind":"rect","dx":0,"dy":0,"w":36,"h":20,"fill":C["control_fill"],"radius":12,"stroke":C["border"],"stroke_w":1},
                     {"kind":"rect","dx":2,"dy":2,"w":16,"h":16,"fill":"#E8ECF4","radius":8}]),
                state_row(20, 130, 660, 60, "on (control_primary, knob right)",
                    [{"kind":"rect","dx":0,"dy":0,"w":36,"h":20,"fill":C["primary"],"radius":12},
                     {"kind":"rect","dx":18,"dy":2,"w":16,"h":16,"fill":"#E8ECF4","radius":8}]),
            ],
        },
        # ─── 5. Dropdown ───
        {
            "name": "05. Dropdown",
            "x": 0, "y": 560, "w": 700, "h": 280,
            "states": [
                state_row(20, 50, 660, 60, "anchor (closed) + open menu",
                    [{"kind":"rect","dx":0,"dy":0,"w":170,"h":24,"fill":C["control_fill"],"radius":6,"stroke":C["border"],"stroke_w":1},
                     {"kind":"text","dx":8,"dy":4,"w":150,"h":18,"text":"Choose…","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":C["ink_muted"]},
                     {"kind":"text","dx":150,"dy":4,"w":20,"h":18,"text":"▾","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":C["ink_muted"],"align":"center"}]),
                state_row(20, 130, 660, 90, "open menu (items: normal/hover/selected/disabled)",
                    [{"kind":"rect","dx":0,"dy":0,"w":200,"h":120,"fill":C["panel_fill"],"radius":10,"stroke":C["border"],"stroke_w":1},
                     {"kind":"text","dx":10,"dy":8,"w":180,"h":22,"text":"Option A","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":C["text_dark"]},
                     {"kind":"rect","dx":4,"dy":30,"w":192,"h":22,"fill":C["hover_fill"],"radius":4},
                     {"kind":"text","dx":10,"dy":32,"w":180,"h":22,"text":"Option B (hover)","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":"#FFFFFF"},
                     {"kind":"rect","dx":4,"dy":54,"w":192,"h":22,"fill":C["selected"],"radius":4},
                     {"kind":"text","dx":10,"dy":56,"w":180,"h":22,"text":"Option C (selected)","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":"#FFFFFF"},
                     {"kind":"text","dx":10,"dy":80,"w":180,"h":22,"text":"Option D (disabled)","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":C["ink_disabled"]}]),
            ],
        },
        # ─── 6. Tooltip ───
        {
            "name": "06. Tooltip",
            "x": 750, "y": 560, "w": 700, "h": 280,
            "states": [
                state_row(20, 50, 660, 60, "tooltip over button",
                    [{"kind":"rect","dx":0,"dy":20,"w":120,"h":30,"fill":C["secondary"],"radius":6},
                     {"kind":"text","dx":0,"dy":27,"w":120,"h":30,"text":"Save","font":"Noto Sans Display","size":14,"weight":"500","line":18,"color":"#FFFFFF","align":"center"},
                     {"kind":"rect","dx":20,"dy":0,"w":160,"h":24,"fill":"#000000E6","radius":6},
                     {"kind":"text","dx":24,"dy":4,"w":152,"h":18,"text":"Saves the canvas","font":"Noto Sans Display","size":11,"weight":"400","line":16,"color":"#FFFFFF"}]),
            ],
        },
        # ─── 7. Toast ───
        {
            "name": "07. Toast",
            "x": 0, "y": 860, "w": 700, "h": 280,
            "states": [
                state_row(20, 50, 660, 60, "info",
                    [{"kind":"rect","dx":0,"dy":0,"w":320,"h":40,"fill":C["secondary"],"radius":8,"stroke":C["border"],"stroke_w":1},
                     {"kind":"rect","dx":0,"dy":0,"w":4,"h":40,"fill":C["accent"],"radius":0},
                     {"kind":"text","dx":14,"dy":10,"w":280,"h":18,"text":"Connection restored","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#FFFFFF"}]),
                state_row(20, 110, 660, 60, "success (teal)",
                    [{"kind":"rect","dx":0,"dy":0,"w":320,"h":40,"fill":C["secondary"],"radius":8,"stroke":C["border"],"stroke_w":1},
                     {"kind":"rect","dx":0,"dy":0,"w":4,"h":40,"fill":C["edge_flow"],"radius":0},
                     {"kind":"text","dx":14,"dy":10,"w":280,"h":18,"text":"Saved successfully","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#FFFFFF"}]),
                state_row(20, 170, 660, 60, "error (red)",
                    [{"kind":"rect","dx":0,"dy":0,"w":320,"h":40,"fill":C["secondary"],"radius":8,"stroke":C["border"],"stroke_w":1},
                     {"kind":"rect","dx":0,"dy":0,"w":4,"h":40,"fill":C["danger"],"radius":0},
                     {"kind":"text","dx":14,"dy":10,"w":280,"h":18,"text":"Cannot save file","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#FFFFFF"}]),
            ],
        },
        # ─── 8. Modal dialog (confirm) ───
        {
            "name": "08. Modal dialog (confirm)",
            "x": 750, "y": 860, "w": 700, "h": 280,
            "states": [
                state_row(20, 50, 660, 180, "confirm dialog (FR-060)",
                    [{"kind":"rect","dx":0,"dy":0,"w":340,"h":130,"fill":C["panel_fill"],"radius":10,"stroke":"#3B82F5","stroke_w":1},
                     # Header
                     {"kind":"text","dx":20,"dy":16,"w":300,"h":22,"text":"Delete scenario?","font":"Noto Sans Display","size":16,"weight":"700","line":22,"color":"#E8ECF4"},
                     # Body
                     {"kind":"text","dx":20,"dy":46,"w":300,"h":48,"text":"This will permanently delete scenario «A» and all overrides.","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":"#B6BECE"},
                     # Buttons (primary right, secondary left)
                     {"kind":"rect","dx":180,"dy":84,"w":60,"h":30,"fill":C["primary"],"radius":6},
                     {"kind":"text","dx":180,"dy":91,"w":60,"h":18,"text":"OK","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#FFFFFF","align":"center"},
                     {"kind":"rect","dx":100,"dy":84,"w":70,"h":30,"fill":C["secondary"],"radius":6,"stroke":"#596680","stroke_w":1},
                     {"kind":"text","dx":100,"dy":91,"w":70,"h":18,"text":"Cancel","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#E8ECF4","align":"center"}]),
            ],
        },
        # ─── 9. Scrollbar ───
        {
            "name": "09. Scrollbar",
            "x": 0, "y": 1160, "w": 700, "h": 160,
            "states": [
                state_row(20, 50, 660, 60, "track + knob (normal/hover/drag)",
                    [{"kind":"rect","dx":0,"dy":0,"w":4,"h":100,"fill":C["control_fill"],"radius":2},
                     {"kind":"rect","dx":0,"dy":0,"w":4,"h":40,"fill":C["control_fill"],"radius":2},
                     {"kind":"text","dx":14,"dy":0,"w":100,"h":16,"text":"normal","font":"Noto Sans Mono","size":10,"weight":"400","line":14,"color":C["ink_muted"]},
                     {"kind":"rect","dx":120,"dy":0,"w":4,"h":100,"fill":C["control_fill"],"radius":2},
                     {"kind":"rect","dx":120,"dy":20,"w":4,"h":40,"fill":C["hover_fill"],"radius":2},
                     {"kind":"text","dx":134,"dy":20,"w":100,"h":16,"text":"hover","font":"Noto Sans Mono","size":10,"weight":"400","line":14,"color":C["ink_muted"]},
                     {"kind":"rect","dx":260,"dy":0,"w":4,"h":100,"fill":C["control_fill"],"radius":2},
                     {"kind":"rect","dx":260,"dy":40,"w":4,"h":40,"fill":C["accent"],"radius":2},
                     {"kind":"text","dx":274,"dy":40,"w":100,"h":16,"text":"drag (accent)","font":"Noto Sans Mono","size":10,"weight":"400","line":14,"color":C["ink_muted"]}]),
            ],
        },
        # ─── 10. Icon glyphs ───
        {
            "name": "10. Icon glyphs",
            "x": 750, "y": 1160, "w": 700, "h": 160,
            "states": [
                state_row(20, 50, 660, 60, "kit glyphs (Unicode text)",
                    [{"kind":"rect","dx":0,"dy":0,"w":26,"h":26,"fill":C["control_fill"],"radius":6},
                     {"kind":"text","dx":0,"dy":6,"w":26,"h":18,"text":"✕","font":"Noto Sans Display","size":14,"weight":"500","line":18,"color":C["ink"],"align":"center"},
                     {"kind":"rect","dx":36,"dy":0,"w":26,"h":26,"fill":C["control_fill"],"radius":6},
                     {"kind":"text","dx":36,"dy":6,"w":26,"h":18,"text":"⚙","font":"Noto Sans Display","size":14,"weight":"500","line":18,"color":C["ink"],"align":"center"},
                     {"kind":"rect","dx":72,"dy":0,"w":26,"h":26,"fill":C["control_fill"],"radius":6},
                     {"kind":"text","dx":72,"dy":6,"w":26,"h":18,"text":"?","font":"Noto Sans Display","size":14,"weight":"500","line":18,"color":C["ink"],"align":"center"},
                     {"kind":"rect","dx":108,"dy":0,"w":26,"h":26,"fill":C["control_fill"],"radius":6},
                     {"kind":"text","dx":108,"dy":6,"w":26,"h":18,"text":"⌕","font":"Noto Sans Display","size":14,"weight":"500","line":18,"color":C["ink"],"align":"center"},
                     {"kind":"rect","dx":144,"dy":0,"w":26,"h":26,"fill":C["control_fill"],"radius":6},
                     {"kind":"text","dx":144,"dy":6,"w":26,"h":18,"text":"+","font":"Noto Sans Display","size":14,"weight":"500","line":18,"color":C["ink"],"align":"center"},
                     {"kind":"rect","dx":180,"dy":0,"w":26,"h":26,"fill":C["control_fill"],"radius":6},
                     {"kind":"text","dx":180,"dy":6,"w":26,"h":18,"text":"↻","font":"Noto Sans Display","size":14,"weight":"500","line":18,"color":C["ink"],"align":"center"}]),
            ],
        },
    ]
}

# ───────────────── 2. CANVAS ELEMENTS — 4 элемента ─────────────────

CANVAS_ELEMENTS_PAGE = {
    "name": "Canvas Elements",
    "boards": [
        # ─── 1. Card node anatomy ───
        {
            "name": "01. Card node",
            "x": 0, "y": 0, "w": 600, "h": 360,
            "states": [
                state_row(20, 50, 560, 280, "default card (header + body + result strip)",
                    [{"kind":"rect","dx":0,"dy":0,"w":400,"h":34,"fill":C["accent_18"],"radius":10},
                     {"kind":"text","dx":10,"dy":8,"w":380,"h":22,"text":"CAC","font":"Noto Sans Display","size":16,"weight":"500","line":22,"color":C["ink"]},
                     # Body
                     {"kind":"rect","dx":0,"dy":34,"w":400,"h":140,"fill":C["white"],"radius":0,"stroke":C["accent_50"],"stroke_w":1},
                     {"kind":"text","dx":10,"dy":44,"w":380,"h":80,"text":"spend = 350000 RUB/mo\nclients = 200","font":"Noto Sans Mono","size":14,"weight":"400","line":20,"color":C["ink"]},
                     # Ports (left=in, right=out)
                     {"kind":"circle","dx":-5,"dy":80,"w":10,"h":10,"fill":C["accent"]},
                     {"kind":"circle","dx":395,"dy":60,"w":10,"h":10,"fill":C["accent"]},
                     # Result strip
                     {"kind":"rect","dx":0,"dy":174,"w":400,"h":22,"fill":C["teal_18"],"radius":0},
                     {"kind":"text","dx":10,"dy":178,"w":380,"h":16,"text":"cac = 1750 RUB","font":"Noto Sans Mono","size":12,"weight":"700","line":16,"color":"#0d5b4f"}]),
            ],
        },
        # ─── 2. Edge types ───
        {
            "name": "02. Edge types",
            "x": 660, "y": 0, "w": 700, "h": 360,
            "states": [
                state_row(20, 50, 660, 60, "default (gray) / flow (teal) / draft (accent α0.70)",
                    [{"kind":"line","dx":0,"dy":10,"w":120,"h":2,"stroke":C["edge_default"]},
                     {"kind":"text","dx":130,"dy":0,"w":120,"h":16,"text":"default","font":"Noto Sans Mono","size":10,"weight":"400","line":14,"color":C["ink_muted"]},
                     {"kind":"line","dx":260,"dy":10,"w":120,"h":2,"stroke":C["edge_flow"]},
                     {"kind":"text","dx":390,"dy":0,"w":120,"h":16,"text":"flow","font":"Noto Sans Mono","size":10,"weight":"400","line":14,"color":C["ink_muted"]},
                     {"kind":"line","dx":0,"dy":40,"w":120,"h":2,"stroke":C["accent_50"]},
                     {"kind":"text","dx":130,"dy":30,"w":200,"h":16,"text":"draft (drag new)","font":"Noto Sans Mono","size":10,"weight":"400","line":14,"color":C["ink_muted"]}]),
                state_row(20, 130, 660, 60, "broken (#737373) / focus (accent + breath)",
                    [{"kind":"line","dx":0,"dy":10,"w":120,"h":2,"stroke":C["broken"]},
                     {"kind":"text","dx":130,"dy":0,"w":120,"h":16,"text":"broken","font":"Noto Sans Mono","size":10,"weight":"400","line":14,"color":C["ink_muted"]},
                     {"kind":"line","dx":260,"dy":10,"w":120,"h":3,"stroke":C["accent"]},
                     {"kind":"text","dx":390,"dy":0,"w":160,"h":16,"text":"focus (3.5px, breath)","font":"Noto Sans Mono","size":10,"weight":"400","line":14,"color":C["ink_muted"]}]),
                state_row(20, 210, 660, 60, "bundle ×N (aggregation, FR-042)",
                    [{"kind":"line","dx":0,"dy":10,"w":120,"h":4,"stroke":C["edge_flow"]},
                     {"kind":"rect","dx":110,"dy":0,"w":24,"h":18,"fill":C["edge_flow"],"radius":9},
                     {"kind":"text","dx":110,"dy":1,"w":24,"h":18,"text":"×3","font":"Noto Sans Mono","size":10,"weight":"700","line":12,"color":"#FFFFFF","align":"center"},
                     {"kind":"text","dx":150,"dy":0,"w":300,"h":16,"text":"bundle of 3 value-edges","font":"Noto Sans Mono","size":10,"weight":"400","line":14,"color":C["ink_muted"]}]),
            ],
        },
        # ─── 3. Minimap ───
        {
            "name": "03. Minimap",
            "x": 0, "y": 400, "w": 600, "h": 220,
            "states": [
                state_row(20, 50, 560, 150, "minimap 220×140 with viewport frame",
                    [{"kind":"rect","dx":0,"dy":0,"w":220,"h":140,"fill":C["panel_fill"],"radius":8,"stroke":C["border"],"stroke_w":1},
                     # Nodes (dots)
                     {"kind":"circle","dx":30,"dy":30,"w":4,"h":4,"fill":C["accent"]},
                     {"kind":"circle","dx":50,"dy":50,"w":4,"h":4,"fill":C["accent"]},
                     {"kind":"circle","dx":80,"dy":40,"w":4,"h":4,"fill":C["accent"]},
                     {"kind":"circle","dx":100,"dy":80,"w":4,"h":4,"fill":C["accent"]},
                     {"kind":"circle","dx":150,"dy":60,"w":4,"h":4,"fill":C["accent"]},
                     # Viewport frame
                     {"kind":"rect","dx":40,"dy":40,"w":80,"h":50,"fill":"#00000000","radius":0,"stroke":"#E8ECF4","stroke_w":2}]),
            ],
        },
        # ─── 4. Wheel-menu (radial) ───
        {
            "name": "04. Wheel-menu",
            "x": 660, "y": 400, "w": 700, "h": 220,
            "states": [
                state_row(20, 50, 560, 150, "radial menu (FR-022)",
                    [{"kind":"circle","dx":0,"dy":0,"w":140,"h":140,"fill":C["wheel_dim"],"radius":70},
                     # Hub (center)
                     {"kind":"circle","dx":50,"dy":50,"w":40,"h":40,"fill":C["wheel_hover"],"radius":20},
                     {"kind":"text","dx":50,"dy":60,"w":40,"h":18,"text":"+","font":"Noto Sans Display","size":16,"weight":"700","line":18,"color":"#FFFFFF","align":"center"},
                     # 4 sector labels
                     {"kind":"text","dx":50,"dy":0,"w":40,"h":18,"text":"infra","font":"Noto Sans Display","size":10,"weight":"500","line":14,"color":"#FFFFFF","align":"center"},
                     {"kind":"text","dx":100,"dy":60,"w":40,"h":18,"text":"ue","font":"Noto Sans Display","size":10,"weight":"500","line":14,"color":"#FFFFFF","align":"center"},
                     {"kind":"text","dx":50,"dy":120,"w":40,"h":18,"text":"pa","font":"Noto Sans Display","size":10,"weight":"500","line":14,"color":"#FFFFFF","align":"center"},
                     {"kind":"text","dx":0,"dy":60,"w":40,"h":18,"text":"custom","font":"Noto Sans Display","size":10,"weight":"500","line":14,"color":"#FFFFFF","align":"center"}]),
            ],
        },
    ]
}

# ───────────────── 3. SURFACES — 6 поверхностей ─────────────────

SURFACES_PAGE = {
    "name": "Surfaces",
    "boards": [
        # ─── 1. Context menu ───
        {
            "name": "01. Context-menu",
            "x": 0, "y": 0, "w": 400, "h": 360,
            "states": [
                state_row(20, 50, 360, 280, "context menu (170×~150, FR-context-menu)",
                    [{"kind":"rect","dx":0,"dy":0,"w":170,"h":180,"fill":C["panel_fill"],"radius":6,"stroke":C["border"],"stroke_w":1},
                     # Items: Edit, Duplicate, Delete, Align…
                     {"kind":"text","dx":10,"dy":10,"w":150,"h":22,"text":"Edit","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":"#E8ECF4"},
                     {"kind":"text","dx":10,"dy":32,"w":150,"h":22,"text":"Duplicate","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":"#E8ECF4"},
                     {"kind":"rect","dx":6,"dy":54,"w":158,"h":22,"fill":C["hover_fill"],"radius":4},
                     {"kind":"text","dx":10,"dy":56,"w":150,"h":22,"text":"Delete (hover)","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":"#FFFFFF"},
                     {"kind":"text","dx":10,"dy":78,"w":150,"h":22,"text":"Align…","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":"#B6BECE"},
                     {"kind":"text","dx":10,"dy":100,"w":150,"h":22,"text":"Group","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":"#E8ECF4"},
                     {"kind":"rect","dx":6,"dy":124,"w":158,"h":1,"fill":C["border"]},
                     {"kind":"text","dx":10,"dy":130,"w":150,"h":22,"text":"Bring to front","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":"#E8ECF4"}]),
            ],
        },
        # ─── 2. Search panel (Ctrl+F) ───
        {
            "name": "02. Search-panel (Ctrl+F)",
            "x": 460, "y": 0, "w": 600, "h": 360,
            "states": [
                state_row(20, 50, 560, 280, "search panel 460×~280 with results",
                    [{"kind":"rect","dx":0,"dy":0,"w":460,"h":280,"fill":C["panel_fill"],"radius":10,"stroke":C["border"],"stroke_w":1},
                     # Input field
                     {"kind":"rect","dx":12,"dy":12,"w":436,"h":36,"fill":C["search_input"],"radius":6,"stroke":C["accent"],"stroke_w":2},
                     {"kind":"text","dx":22,"dy":22,"w":400,"h":18,"text":"cac|","font":"Noto Sans Mono","size":13,"weight":"400","line":18,"color":"#E8ECF4"},
                     # Result rows (ROW_HEIGHT=28)
                     {"kind":"rect","dx":12,"dy":60,"w":436,"h":28,"fill":C["hover_fill"],"radius":4},
                     {"kind":"text","dx":22,"dy":66,"w":400,"h":18,"text":"CAC — cost of acquisition","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":"#FFFFFF"},
                     {"kind":"text","dx":22,"dy":94,"w":400,"h":18,"text":"CAC ratio (LTV/CAC)","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":"#B6BECE"},
                     {"kind":"text","dx":22,"dy":122,"w":400,"h":18,"text":"ARPU → CAC","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":"#B6BECE"},
                     # Footer
                     {"kind":"rect","dx":12,"dy":244,"w":436,"h":1,"fill":C["border"]},
                     {"kind":"text","dx":22,"dy":248,"w":200,"h":18,"text":"3 of 12 results","font":"Noto Sans Mono","size":10,"weight":"400","line":14,"color":"#6b7280"}]),
            ],
        },
        # ─── 3. Template palette (collapsed + expanded) ───
        {
            "name": "03. Template-palette",
            "x": 0, "y": 400, "w": 800, "h": 460,
            "states": [
                state_row(20, 50, 760, 60, "collapsed strip (PAL_BUTTON 30)",
                    [{"kind":"rect","dx":0,"dy":0,"w":200,"h":30,"fill":C["panel_fill"],"radius":6,"stroke":C["border"],"stroke_w":1},
                     {"kind":"rect","dx":6,"dy":4,"w":22,"h":22,"fill":C["control_fill"],"radius":4},
                     {"kind":"text","dx":34,"dy":8,"w":160,"h":18,"text":"infra","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#E8ECF4"}]),
                state_row(20, 130, 760, 300, "expanded dock (340×~400)",
                    [{"kind":"rect","dx":0,"dy":0,"w":340,"h":300,"fill":C["panel_fill"],"radius":10,"stroke":C["border"],"stroke_w":1},
                     # Header
                     {"kind":"text","dx":12,"dy":12,"w":316,"h":22,"text":"Templates","font":"Noto Sans Display","size":14,"weight":"700","line":20,"color":"#E8ECF4"},
                     # Search field
                     {"kind":"rect","dx":12,"dy":44,"w":316,"h":32,"fill":C["search_input"],"radius":6,"stroke":C["border"],"stroke_w":1},
                     {"kind":"text","dx":22,"dy":52,"w":300,"h":18,"text":"Search templates…","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":"#B6BECE"},
                     # Category chips
                     {"kind":"rect","dx":12,"dy":86,"w":60,"h":26,"fill":C["selected"],"radius":12},
                     {"kind":"text","dx":12,"dy":91,"w":60,"h":18,"text":"infra","font":"Noto Sans Display","size":12,"weight":"500","line":16,"color":"#FFFFFF","align":"center"},
                     {"kind":"rect","dx":80,"dy":86,"w":60,"h":26,"fill":C["control_fill"],"radius":12},
                     {"kind":"text","dx":80,"dy":91,"w":60,"h":18,"text":"ue","font":"Noto Sans Display","size":12,"weight":"500","line":16,"color":"#E8ECF4","align":"center"},
                     # Template rows (46px each)
                     {"kind":"rect","dx":12,"dy":124,"w":316,"h":46,"fill":C["hover_fill"],"radius":4},
                     {"kind":"rect","dx":20,"dy":134,"w":28,"h":28,"fill":C["accent_18"],"radius":4},
                     {"kind":"text","dx":56,"dy":138,"w":260,"h":18,"text":"Load Balancer","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#FFFFFF"},
                     {"kind":"text","dx":56,"dy":156,"w":260,"h":14,"text":"rps, latency, hit-rate","font":"Noto Sans Mono","size":10,"weight":"400","line":14,"color":"#B6BECE"},
                     {"kind":"text","dx":20,"dy":180,"w":300,"h":18,"text":"API Gateway","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#E8ECF4"},
                     {"kind":"text","dx":20,"dy":226,"w":300,"h":18,"text":"Cache (Redis)","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#E8ECF4"},
                     # Footer
                     {"kind":"rect","dx":12,"dy":274,"w":316,"h":1,"fill":C["border"]},
                     {"kind":"text","dx":12,"dy":278,"w":200,"h":14,"text":"62 templates","font":"Noto Sans Mono","size":10,"weight":"400","line":14,"color":"#6b7280"}]),
            ],
        },
        # ─── 4. Scheme gallery ───
        {
            "name": "04. Scheme-gallery",
            "x": 820, "y": 400, "w": 700, "h": 460,
            "states": [
                state_row(20, 50, 660, 380, "gallery (PANEL_W 560)",
                    [{"kind":"rect","dx":0,"dy":0,"w":560,"h":380,"fill":C["panel_fill"],"radius":10,"stroke":C["border"],"stroke_w":1},
                     # Header
                     {"kind":"text","dx":12,"dy":12,"w":536,"h":22,"text":"Scheme gallery","font":"Noto Sans Display","size":14,"weight":"700","line":20,"color":"#E8ECF4"},
                     # Search input
                     {"kind":"rect","dx":12,"dy":44,"w":536,"h":34,"fill":C["search_input"],"radius":6,"stroke":C["border"],"stroke_w":1},
                     {"kind":"text","dx":22,"dy":52,"w":510,"h":18,"text":"Search schemes…","font":"Noto Sans Display","size":13,"weight":"400","line":18,"color":"#B6BECE"},
                     # Category chips
                     {"kind":"rect","dx":12,"dy":86,"w":56,"h":28,"fill":C["selected"],"radius":14},
                     {"kind":"text","dx":12,"dy":92,"w":56,"h":18,"text":"All","font":"Noto Sans Display","size":12,"weight":"500","line":16,"color":"#FFFFFF","align":"center"},
                     {"kind":"rect","dx":76,"dy":86,"w":108,"h":28,"fill":C["control_fill"],"radius":14},
                     {"kind":"text","dx":76,"dy":92,"w":108,"h":18,"text":"Business","font":"Noto Sans Display","size":12,"weight":"500","line":16,"color":"#E8ECF4","align":"center"},
                     {"kind":"rect","dx":192,"dy":86,"w":108,"h":28,"fill":C["control_fill"],"radius":14},
                     {"kind":"text","dx":192,"dy":92,"w":108,"h":18,"text":"Architecture","font":"Noto Sans Display","size":12,"weight":"500","line":16,"color":"#E8ECF4","align":"center"},
                     # Scheme rows (ROW_H 56)
                     {"kind":"rect","dx":12,"dy":124,"w":536,"h":50,"fill":C["hover_fill"],"radius":4},
                     {"kind":"text","dx":24,"dy":138,"w":400,"h":18,"text":"Unit economics","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#FFFFFF"},
                     {"kind":"text","dx":24,"dy":156,"w":400,"h":14,"text":"Margin, LTV, CAC, runway","font":"Noto Sans Mono","size":11,"weight":"400","line":14,"color":"#B6BECE"},
                     {"kind":"text","dx":24,"dy":194,"w":400,"h":18,"text":"Runway","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#E8ECF4"},
                     {"kind":"text","dx":24,"dy":212,"w":400,"h":14,"text":"Cash burn, runway, monthly","font":"Noto Sans Mono","size":11,"weight":"400","line":14,"color":"#B6BECE"},
                     {"kind":"text","dx":24,"dy":250,"w":400,"h":18,"text":"CJM: SaaS B2B","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#E8ECF4"},
                     {"kind":"text","dx":24,"dy":268,"w":400,"h":14,"text":"Stages, touchpoints, pains","font":"Noto Sans Mono","size":11,"weight":"400","line":14,"color":"#B6BECE"},
                     # Footer
                     {"kind":"rect","dx":12,"dy":344,"w":536,"h":1,"fill":C["border"]},
                     {"kind":"text","dx":24,"dy":348,"w":300,"h":14,"text":"14 schemes","font":"Noto Sans Mono","size":10,"weight":"400","line":14,"color":"#6b7280"}]),
            ],
        },
        # ─── 5. Whatif bar ───
        {
            "name": "05. Whatif-bar",
            "x": 0, "y": 880, "w": 800, "h": 280,
            "states": [
                state_row(20, 50, 760, 200, "whatif bar (chips + comparison table)",
                    [{"kind":"rect","dx":0,"dy":0,"w":760,"h":200,"fill":C["panel_fill"],"radius":10,"stroke":C["border"],"stroke_w":1},
                     # Chips row (BAR_HEIGHT ~44)
                     {"kind":"rect","dx":12,"dy":12,"w":100,"h":26,"fill":"#4C8CF2","radius":6},
                     {"kind":"text","dx":12,"dy":17,"w":100,"h":18,"text":"base","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#FFFFFF","align":"center"},
                     {"kind":"rect","dx":120,"dy":12,"w":100,"h":26,"fill":C["selected"],"radius":6},
                     {"kind":"text","dx":120,"dy":17,"w":100,"h":18,"text":"scenario A","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#FFFFFF","align":"center"},
                     {"kind":"rect","dx":228,"dy":12,"w":100,"h":26,"fill":C["chip_dim"],"radius":6},
                     {"kind":"text","dx":228,"dy":17,"w":100,"h":18,"text":"scenario B","font":"Noto Sans Display","size":13,"weight":"500","line":18,"color":"#B6BECE","align":"center"},
                     # Add scenario button
                     {"kind":"rect","dx":336,"dy":12,"w":26,"h":26,"fill":C["control_fill"],"radius":6},
                     {"kind":"text","dx":336,"dy":17,"w":26,"h":18,"text":"+","font":"Noto Sans Display","size":14,"weight":"700","line":18,"color":"#E8ECF4","align":"center"},
                     # Comparison table header
                     {"kind":"rect","dx":12,"dy":50,"w":736,"h":1,"fill":C["border"]},
                     {"kind":"text","dx":12,"dy":56,"w":200,"h":18,"text":"Metric","font":"Noto Sans Mono","size":11,"weight":"700","line":14,"color":"#B6BECE"},
                     {"kind":"text","dx":220,"dy":56,"w":100,"h":18,"text":"base","font":"Noto Sans Mono","size":11,"weight":"700","line":14,"color":"#B6BECE","align":"right"},
                     {"kind":"text","dx":330,"dy":56,"w":100,"h":18,"text":"A","font":"Noto Sans Mono","size":11,"weight":"700","line":14,"color":"#FFFFFF","align":"right"},
                     {"kind":"text","dx":440,"dy":56,"w":100,"h":18,"text":"B","font":"Noto Sans Mono","size":11,"weight":"700","line":14,"color":"#B6BECE","align":"right"},
                     # Rows
                     {"kind":"text","dx":12,"dy":80,"w":200,"h":18,"text":"CAC","font":"Noto Sans Mono","size":12,"weight":"400","line":18,"color":"#E8ECF4"},
                     {"kind":"text","dx":220,"dy":80,"w":100,"h":18,"text":"1750","font":"Noto Sans Mono","size":12,"weight":"400","line":18,"color":"#B6BECE","align":"right"},
                     {"kind":"text","dx":330,"dy":80,"w":100,"h":18,"text":"1500","font":"Noto Sans Mono","size":12,"weight":"400","line":18,"color":"#FFFFFF","align":"right"},
                     {"kind":"rect","dx":430,"dy":80,"w":40,"h":18,"fill":C["whatif_badge"],"radius":9},
                     {"kind":"text","dx":430,"dy":81,"w":40,"h":18,"text":"-14%","font":"Noto Sans Mono","size":10,"weight":"700","line":12,"color":"#1f2937","align":"center"},
                     {"kind":"text","dx":12,"dy":110,"w":200,"h":18,"text":"LTV","font":"Noto Sans Mono","size":12,"weight":"400","line":18,"color":"#E8ECF4"},
                     {"kind":"text","dx":220,"dy":110,"w":100,"h":18,"text":"36000","font":"Noto Sans Mono","size":12,"weight":"400","line":18,"color":"#B6BECE","align":"right"},
                     {"kind":"text","dx":330,"dy":110,"w":100,"h":18,"text":"42000","font":"Noto Sans Mono","size":12,"weight":"400","line":18,"color":"#FFFFFF","align":"right"},
                     {"kind":"rect","dx":430,"dy":110,"w":40,"h":18,"fill":C["whatif_badge"],"radius":9},
                     {"kind":"text","dx":430,"dy":111,"w":40,"h":18,"text":"+17%","font":"Noto Sans Mono","size":10,"weight":"700","line":12,"color":"#1f2937","align":"center"}]),
            ],
        },
        # ─── 6. HUD (F3) ───
        {
            "name": "06. HUD (F3)",
            "x": 820, "y": 880, "w": 700, "h": 280,
            "states": [
                state_row(20, 50, 660, 60, "HUD overlay (single line, screen px)",
                    [{"kind":"rect","dx":0,"dy":0,"w":480,"h":22,"fill":"#00000000"},
                     {"kind":"text","dx":0,"dy":2,"w":480,"h":18,"text":"60 fps · p95 12ms · 14 nodes · 8 edges · 0 thumbs","font":"Noto Sans Mono","size":14,"weight":"500","line":18,"color":"#659CF8"}]),
                state_row(20, 110, 660, 60, "whatif indicator (when bar hidden)",
                    [{"kind":"rect","dx":0,"dy":4,"w":50,"h":18,"fill":"#4C8CF2","radius":9},
                     {"kind":"text","dx":0,"dy":5,"w":50,"h":18,"text":"A","font":"Noto Sans Mono","size":10,"weight":"700","line":12,"color":"#FFFFFF","align":"center"},
                     {"kind":"text","dx":60,"dy":0,"w":200,"h":18,"text":"scenario active","font":"Noto Sans Mono","size":12,"weight":"400","line":18,"color":"#B6BECE"}]),
            ],
        },
    ]
}

ALL_PAGES = [UI_KIT_PAGE, CANVAS_ELEMENTS_PAGE, SURFACES_PAGE]


# ───────────────── JS-генератор ─────────────────

SEED_JS = r"""
const __pages = __PAGES_JSON__;

if (!penpot.currentFile) throw new Error("No active Penpot file");

function sRGBtoHex(arr) { return arr; }  // colors already hex strings

// Penpot требует fillColor = "#RRGGBB" (6 символов) + отдельный fillOpacity.
// Принимаем "#RRGGBB" или "#RRGGBBAA" и нормализуем.
function normFill(color, fillOpacity) {
  if (!color) return null;
  // Trim leading #
  let hex = color.startsWith("#") ? color.slice(1) : color;
  let opacity = fillOpacity === undefined ? 1 : fillOpacity;
  if (hex.length === 8) {
    // #RRGGBBAA — извлечь alpha
    const a = parseInt(hex.slice(6, 8), 16) / 255;
    opacity = opacity === 1 ? a : opacity * a;
    hex = hex.slice(0, 6);
  }
  return { fillColor: "#" + hex, fillOpacity: opacity };
}
function normStroke(color, opacity) {
  if (!color) return null;
  let hex = color.startsWith("#") ? color.slice(1) : color;
  let op = opacity === undefined ? 1 : opacity;
  if (hex.length === 8) {
    const a = parseInt(hex.slice(6, 8), 16) / 255;
    op = op === 1 ? a : op * a;
    hex = hex.slice(0, 6);
  }
  return { strokeColor: "#" + hex, strokeOpacity: op };
}

function makeRect(parent, localX, localY, w, h, fill, opts) {
  opts = opts || {};
  const r = penpot.createRectangle();
  r.resize(w, h);
  const f = normFill(fill, opts.fillOpacity);
  r.fills = f ? [f] : [];
  if (opts.stroke) {
    const s = normStroke(opts.stroke, opts.strokeOpacity);
    r.strokes = [{ strokeColor: s.strokeColor, strokeOpacity: s.strokeOpacity, strokeStyle: "solid", strokeWidth: opts.strokeWidth || 1, strokeAlignment: "center" }];
  }
  if (opts.radius) r.borderRadius = opts.radius;
  parent.appendChild(r);
  r.x = parent.x + localX;
  r.y = parent.y + localY;
  return r;
}
function makeCircle(parent, localX, localY, w, h, fill) {
  const e = penpot.createEllipse();
  e.resize(w, h);
  const f = normFill(fill, 1);
  e.fills = f ? [f] : [];
  parent.appendChild(e);
  e.x = parent.x + localX;
  e.y = parent.y + localY;
  return e;
}
function makeText(parent, localX, localY, w, h, text, opts) {
  opts = opts || {};
  const t = penpot.createText(text);
  t.resize(w, h);
  t.fontFamily = opts.family || "Noto Sans Display";
  t.fontSize = String(opts.size || 13);
  t.fontWeight = String(opts.weight || "400");
  const size = opts.size || 13;
  const linePx = opts.line || 18;
  t.lineHeight = String(Math.round((linePx / size) * 100) / 100);
  t.growType = "auto-height";
  const f = normFill(opts.color || "#1f2937", 1);
  t.fills = [f];
  if (opts.align) t.align = opts.align;
  parent.appendChild(t);
  t.x = parent.x + localX;
  t.y = parent.y + localY;
  return t;
}
function makeLine(parent, localX, localY, w, h, color, strokeW) {
  // Тонкая линия — это Rectangle с маленькой высотой
  const r = penpot.createRectangle();
  r.resize(w, h);
  const f = normFill(color, 1);
  r.fills = f ? [f] : [];
  parent.appendChild(r);
  r.x = parent.x + localX;
  r.y = parent.y + localY;
  return r;
}

const pageResults = [];

for (const pageSpec of __pages) {
  // Найти или создать страницу
  let page = penpot.currentFile.pages.find(p => p.name === pageSpec.name);
  if (!page) {
    page = penpot.createPage();
    page.name = pageSpec.name;
  }
  await penpot.openPage(page);

  // Idempotent: удалить старые boards с теми же именами
  const expectedNames = pageSpec.boards.map(b => b.name);
  const root = page.root;
  if (root && root.children) {
    for (const child of [...root.children]) {
      if (expectedNames.includes(child.name)) {
        try { child.remove(); } catch (e) {}
      }
    }
  }

  // Создать boards со всеми shape'ами
  const boardResults = [];
  for (const boardSpec of pageSpec.boards) {
    const board = penpot.createBoard();
    board.name = boardSpec.name;
    board.x = boardSpec.x;
    board.y = boardSpec.y;
    board.resize(boardSpec.w, boardSpec.h);
    board.fills = [{ fillColor: "#ffffff", fillOpacity: 1 }];

    // Title (header text на самом верху)
    makeText(board, 20, 18, boardSpec.w - 40, 22, boardSpec.name, {
      family: "Noto Sans Display", size: 16, weight: "700", line: 22, color: "#1f2937",
    });

    // Состояния (state rows) — каждый это массив shape-спецификаций
    let shapeCount = 0;
    for (const stateGroup of (boardSpec.states || [])) {
      // stateGroup — массив shape-спецификаций
      for (const spec of stateGroup) {
        const opts = { stroke: spec.stroke, strokeOpacity: spec.strokeOpacity,
                       strokeWidth: spec.stroke_w, radius: spec.radius,
                       fillOpacity: spec.fillOpacity };
        if (spec.kind === "rect") {
          makeRect(board, spec.x, spec.y, spec.w, spec.h, spec.fill, opts);
        } else if (spec.kind === "circle") {
          makeCircle(board, spec.x, spec.y, spec.w, spec.h, spec.fill);
        } else if (spec.kind === "text") {
          makeText(board, spec.x, spec.y, spec.w, spec.h, spec.text, {
            family: spec.font, size: spec.size, weight: spec.weight,
            line: spec.line, color: spec.color, align: spec.align,
          });
        } else if (spec.kind === "line") {
          makeLine(board, spec.x, spec.y, spec.w, spec.h, spec.stroke, spec.stroke_w);
        }
        shapeCount++;
      }
    }
    boardResults.push({ name: boardSpec.name, shapes: shapeCount });
  }

  pageResults.push({ page: pageSpec.name, boards: boardResults });
}

return { pages: pageResults };
"""


def seed_all(client: PenpotMCP) -> dict[str, Any]:
    """Создать все 3 страницы (UI Kit, Canvas Elements, Surfaces)."""
    js = SEED_JS.replace("__PAGES_JSON__", json.dumps(ALL_PAGES, ensure_ascii=False))
    raw = client.execute_code(js)
    if isinstance(raw, str):
        try:
            return json.loads(raw)
        except json.JSONDecodeError:
            return {"raw_output": raw}
    return raw


if __name__ == "__main__":
    js = SEED_JS.replace("__PAGES_JSON__", json.dumps(ALL_PAGES, ensure_ascii=False))
    total_boards = sum(len(p["boards"]) for p in ALL_PAGES)
    total_shapes = sum(
        sum(len(s) for s in b.get("states", []))
        for p in ALL_PAGES for b in p["boards"]
    )
    print(f"JS payload size: {len(js)} chars")
    print(f"Pages: {len(ALL_PAGES)}")
    for p in ALL_PAGES:
        print(f"  {p['name']}: {len(p['boards'])} boards")
    print(f"Total boards: {total_boards}")
    print(f"Total shapes: ~{total_shapes}")
