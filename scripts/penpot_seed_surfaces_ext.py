#!/usr/bin/env python3
"""
CanvasDesk → Penpot: 18 недостающих поверхностей (после первого прогона).

Создаёт новую страницу «Surfaces — Extended» со всеми поверхностями, не
попавшими в первый seeder (penpot_seed_all.py). Источник: полный список
SurfaceDecl в crates/canvas-app/src/app/ui_registry.rs.

Полный список поверхностей CanvasDesk (26):
Уже в Penpot (8): wheel-menu, minimap (Canvas Elements), context-menu,
search-panel, template-palette (collapsed+expanded в одном board),
scheme-gallery, whatif-bar, HUD (Surfaces).

В ЭТОМ СИДЕРЕ (18):
  1. WORLD (main canvas) — sample scene с 5 нодами unit-economics + edges
  2. STAGE — modal bundle view (main stage)
  3. SETTINGS modal — FR-039: nav 180 + sections + switches + dropdowns
  4. DOCS viewer — FR-031: in-app docs, scrollbar 6, close 26
  5. HELP_MENU — секции: Quick start / Templates / Hotkeys
  6. HOTKEYS panel — cheat sheet с хоткеями
  7. CORNER_BUTTONS — ⚙ / theme / ? / chevron
  8. FLOW_MAP — FR-050 Н9-4: панель «Карта проливаний»
  9. EDITOR — editing session: text node в edit + hints popup (FR-021)
 10. EXPLAIN — modal explain tree (PRD-0007 X2)
 11. AUTOLINK — modal review autolinks (PRD-0007 X4)
 12. ONBOARDING — onboarding card (FR-028)
 13. KIT_GALLERY — FR-055 витрина кита
 14. ADMIN — FR-070 UI-админпанель
 15. EMPTY — empty state card
 16. TEMPLATE_STRIP — collapsed strip (separate from expanded)
 17. PALETTE — selection toolbar + open column
 18. CHOICE_MENU — sub-menu (color/align)
"""
from __future__ import annotations

import json
from typing import Any

from penpot_mcp import PenpotMCP


# ───────────────── Цвета (те же что в penpot_seed_all.py) ─────────────────
C = {
    "accent":      "#65A0F7",
    "accent_18":   "#65A0F72E",
    "accent_50":   "#65A0F780",
    "primary":     "#285299",
    "secondary":   "#333B4A",
    "danger":      "#E55C5C",
    "ink":         "#1f2937",
    "ink_muted":   "#6b7280",
    "ink_disabled":"#9ca3af",
    "text_dark":   "#E8ECF4",
    "text_muted":  "#B6BECE",
    "white":       "#ffffff",
    "panel_fill":  "#1a1f29F2",
    "border":      "#3a445866",
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
    "explain_leaf":"#9FD6FF",
    "card_fill":   "#2b3340F2",
    "card_fill_18":"#65A0F72E",
    "group_fill":  "#65A0F714",
    "group_border":"#65A0F766",
    "stage_dim":   "#00000099",
}


# ───────────────── JS-генератор (переиспользует хелперы) ─────────────────

SEED_JS = r"""
const __page = __PAGE_JSON__;
if (!penpot.currentFile) throw new Error("No active Penpot file");

function normFill(color, fillOpacity) {
  if (!color) return null;
  let hex = color.startsWith("#") ? color.slice(1) : color;
  let opacity = fillOpacity === undefined ? 1 : fillOpacity;
  if (hex.length === 8) {
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

function makeRect(parent, lx, ly, w, h, fill, opts) {
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
  r.x = parent.x + lx; r.y = parent.y + ly;
  return r;
}
function makeCircle(parent, lx, ly, w, h, fill) {
  const e = penpot.createEllipse();
  e.resize(w, h);
  const f = normFill(fill, 1);
  e.fills = f ? [f] : [];
  parent.appendChild(e);
  e.x = parent.x + lx; e.y = parent.y + ly;
  return e;
}
function makeText(parent, lx, ly, w, h, text, opts) {
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
  t.x = parent.x + lx; t.y = parent.y + ly;
  return t;
}
function makeLine(parent, lx, ly, w, h, color) {
  const r = penpot.createRectangle();
  r.resize(w, h);
  const f = normFill(color, 1);
  r.fills = f ? [f] : [];
  parent.appendChild(r);
  r.x = parent.x + lx; r.y = parent.y + ly;
  return r;
}

// Найти или создать страницу
let page = penpot.currentFile.pages.find(p => p.name === __page.name);
if (!page) {
  page = penpot.createPage();
  page.name = __page.name;
}
await penpot.openPage(page);

// Idempotent: удалить старые boards с теми же именами
const expectedNames = __page.boards.map(b => b.name);
const root = page.root;
if (root && root.children) {
  for (const child of [...root.children]) {
    if (expectedNames.includes(child.name)) {
      try { child.remove(); } catch (e) {}
    }
  }
}

const boardResults = [];

for (const spec of __page.boards) {
  const board = penpot.createBoard();
  board.name = spec.name;
  board.x = spec.x;
  board.y = spec.y;
  board.resize(spec.w, spec.h);
  board.fills = [{ fillColor: "#ffffff", fillOpacity: 1 }];

  // Title
  makeText(board, 20, 18, spec.w - 40, 22, spec.name, {
    family: "Noto Sans Display", size: 16, weight: "700", line: 22, color: "#1f2937",
  });

  // Subtitle (use-case ref)
  makeText(board, 20, 40, spec.w - 40, 16, spec.subtitle || "", {
    family: "Noto Sans Mono", size: 10, weight: "400", line: 14, color: "#6b7280",
  });

  // Shapes
  let shapeCount = 0;
  for (const s of (spec.shapes || [])) {
    const opts = { stroke: s.stroke, strokeOpacity: s.strokeOpacity,
                   strokeWidth: s.stroke_w, radius: s.radius,
                   fillOpacity: s.fillOpacity };
    if (s.kind === "rect") {
      makeRect(board, s.x, s.y, s.w, s.h, s.fill, opts);
    } else if (s.kind === "circle") {
      makeCircle(board, s.x, s.y, s.w, s.h, s.fill);
    } else if (s.kind === "text") {
      makeText(board, s.x, s.y, s.w, s.h, s.text, {
        family: s.font, size: s.size, weight: s.weight,
        line: s.line, color: s.color, align: s.align,
      });
    } else if (s.kind === "line") {
      makeLine(board, s.x, s.y, s.w, s.h, s.stroke);
    }
    shapeCount++;
  }
  boardResults.push({ name: spec.name, shapes: shapeCount });
}

return { page: __page.name, boards: boardResults };
"""

# ───────────────── Спецификация 18 поверхностей ─────────────────

def s(x, y, w, h, fill, **kw):
    """Shortcut для shape-спецификации (rect)."""
    d = {"kind":"rect","x":x,"y":y,"w":w,"h":h,"fill":fill}
    d.update(kw)
    return d

def t(x, y, w, h, text, **kw):
    """Shortcut для text-спецификации."""
    d = {"kind":"text","x":x,"y":y,"w":w,"h":h,"text":text}
    d.update(kw)
    return d

def l(x, y, w, h, color):
    return {"kind":"line","x":x,"y":y,"w":w,"h":h,"stroke":color}

def c(x, y, d, fill):
    return {"kind":"circle","x":x,"y":y,"w":d,"h":d,"fill":fill}


EXT_PAGE = {
    "name": "Surfaces — Extended",
    "boards": [
        # ─── 1. WORLD (main canvas) — sample scene: 5 nodes unit-economics ───
        {
            "name": "01. WORLD — Main canvas",
            "subtitle": "L0, PassThrough. Main canvas viewport с sample scene (unit-economics: Unit → Margin → LTV, CAC → Ratio)",
            "x": 0, "y": 0, "w": 1100, "h": 460,
            "shapes": [
                # Canvas background
                s(20, 70, 1060, 360, "#0f141b"),
                # Hint card (top-left)
                s(40, 90, 320, 100, "#2b3340F2", radius=10),
                t(50, 100, 300, 16, "Юнит-экономика", font="Noto Sans Display", size=14, weight="700", line=20, color="#E8ECF4"),
                t(50, 122, 300, 60, "Маржа из цены и себестоимости, ценность — маржа на срок жизни.", font="Noto Sans Display", size=11, weight="400", line=14, color="#B6BECE"),
                # Node: Unit (text node)
                s(400, 110, 260, 110, "#2b3340F2", radius=10, stroke="#65A0F780", stroke_w=1),
                t(410, 118, 240, 22, "Единица", font="Noto Sans Display", size=14, weight="500", line=18, color="#E8ECF4"),
                t(410, 145, 240, 60, "price = 10 руб\ncogs = 4 руб", font="Noto Sans Mono", size=12, weight="400", line=18, color="#9FD6FF"),
                # Node: Margin (template node — accent header)
                s(720, 110, 280, 130, "#ffffff", radius=10, stroke="#65A0F780", stroke_w=1),
                s(720, 110, 280, 34, "#65A0F72E", radius=10),  # header band
                t(730, 118, 260, 22, "Маржа", font="Noto Sans Display", size=14, weight="500", line=18, color="#1f2937"),
                t(730, 148, 260, 60, "margin = Единица.price - Единица.cogs", font="Noto Sans Mono", size=12, weight="400", line=18, color="#1f2937"),
                s(720, 218, 280, 22, "#21A88C2E"),  # result strip
                t(730, 222, 260, 16, "margin = 6 руб", font="Noto Sans Mono", size=12, weight="700", line=16, color="#0d5b4f"),
                # Node: Lifetime
                s(400, 250, 260, 80, "#2b3340F2", radius=10),
                t(410, 258, 240, 22, "Срок жизни", font="Noto Sans Display", size=14, weight="500", line=18, color="#E8ECF4"),
                t(410, 285, 240, 40, "months = 36", font="Noto Sans Mono", size=12, weight="400", line=18, color="#9FD6FF"),
                # Node: LTV
                s(720, 270, 280, 100, "#ffffff", radius=10, stroke="#65A0F780", stroke_w=1),
                s(720, 270, 280, 34, "#65A0F72E", radius=10),
                t(730, 278, 260, 22, "LTV", font="Noto Sans Display", size=14, weight="500", line=18, color="#1f2937"),
                t(730, 308, 260, 30, "ltv = Маржа.margin * Срок.months", font="Noto Sans Mono", size=12, weight="400", line=18, color="#1f2937"),
                s(720, 348, 280, 22, "#21A88C2E"),
                t(730, 352, 260, 16, "ltv = 216 руб", font="Noto Sans Mono", size=12, weight="700", line=16, color="#0d5b4f"),
                # Edges (value-flow, teal)
                l(660, 165, 60, 2, "#21A88C"),  # Unit → Margin
                l(660, 290, 60, 2, "#21A88C"),  # (visual; actual flows from Margin/Lifetime)
                # Edge ports
                c(655, 162, 6, "#65A0F7"),
                c(655, 287, 6, "#65A0F7"),
                c(720, 175, 6, "#65A0F7"),
                # HUD overlay (bottom-left of canvas)
                t(40, 410, 500, 18, "60 fps · p95 12ms · 4 nodes · 2 edges", font="Noto Sans Mono", size=12, weight="500", line=18, color="#659CF8"),
                # Minimap top-right
                s(900, 90, 160, 100, "#1a1f29F2", radius=8, stroke="#3a445866", stroke_w=1),
                c(930, 120, 4, "#65A0F7"),
                c(960, 140, 4, "#65A0F7"),
                c(990, 110, 4, "#65A0F7"),
                s(945, 125, 50, 30, "#00000000", stroke="#E8ECF4", stroke_w=2),
            ],
        },
        # ─── 2. STAGE (modal bundle view) ───
        {
            "name": "02. STAGE — Main stage (bundle view)",
            "subtitle": "L5, Block. Modal срез пучка рёбер (FR-042): zoom in на агрегированную связь ×N",
            "x": 1120, "y": 0, "w": 700, "h": 460,
            "shapes": [
                # Dim overlay
                s(20, 70, 660, 360, "#00000099"),
                # Stage panel (centered)
                s(80, 130, 540, 240, "#1a1f29F2", radius=10, stroke="#3B82F5", stroke_w=1),
                t(100, 145, 500, 22, "Main stage · bundle ×3", font="Noto Sans Display", size=16, weight="700", line=22, color="#E8ECF4"),
                t(100, 175, 500, 16, "Агрегированный поток 3 value-рёбер между Unit → Margin", font="Noto Sans Display", size=11, weight="400", line=14, color="#B6BECE"),
                # Source node card
                s(100, 210, 180, 80, "#2b3340F2", radius=10),
                t(110, 218, 160, 18, "Unit", font="Noto Sans Display", size=13, weight="500", line=18, color="#E8ECF4"),
                t(110, 245, 160, 40, "price = 10\ncogs = 4", font="Noto Sans Mono", size=11, weight="400", line=18, color="#9FD6FF"),
                # Big bundle visualization
                s(290, 240, 220, 20, "#21A88C80", radius=0),  # thick teal line
                c(285, 247, 14, "#21A88C"),  # source dot
                c(510, 247, 14, "#21A88C"),  # target dot
                # ×3 badge
                s(390, 230, 28, 18, "#21A88C", radius=9),
                t(390, 231, 28, 18, "×3", font="Noto Sans Mono", size=10, weight="700", line=12, color="#FFFFFF", align="center"),
                # Target node card
                s(520, 210, 100, 80, "#ffffff", radius=10),
                t(530, 218, 80, 18, "Margin", font="Noto Sans Display", size=13, weight="500", line=18, color="#1f2937"),
                t(530, 245, 80, 40, "6 руб", font="Noto Sans Mono", size=11, weight="700", line=18, color="#0d5b4f"),
                # Close button
                s(580, 140, 26, 26, "#2b3340F2", radius=6),
                t(580, 145, 26, 18, "✕", font="Noto Sans Display", size=14, weight="500", line=18, color="#E8ECF4", align="center"),
                # Footer note
                t(100, 320, 500, 16, "Esc — закрыть · клик мимо — закрыть · scroll — zoom", font="Noto Sans Mono", size=10, weight="400", line=14, color="#6b7280"),
            ],
        },
        # ─── 3. SETTINGS modal (FR-039) ───
        {
            "name": "03. SETTINGS modal",
            "subtitle": "L5, Block. FR-039: nav 180 + sections. MIN_W 560 / MAX_W 880 / MIN_H 400 / MAX_H 640",
            "x": 0, "y": 480, "w": 1000, "h": 580,
            "shapes": [
                # Dim
                s(20, 70, 960, 480, "#00000099"),
                # Modal panel 800×460
                s(100, 100, 800, 460, "#1a1f29F2", radius=10, stroke="#3B82F5", stroke_w=1),
                # Header
                t(120, 115, 600, 22, "Settings", font="Noto Sans Display", size=16, weight="700", line=22, color="#E8ECF4"),
                s(840, 110, 26, 26, "#2b3340F2", radius=6),
                t(840, 115, 26, 18, "✕", font="Noto Sans Display", size=14, weight="500", line=18, color="#E8ECF4", align="center"),
                # Left nav (180 wide)
                s(120, 150, 180, 400, "#0d1117", radius=0),
                t(135, 160, 160, 22, "Appearance", font="Noto Sans Display", size=13, weight="500", line=18, color="#FFFFFF"),
                s(120, 156, 3, 22, "#65A0F7"),  # selected indicator
                t(135, 190, 160, 22, "Hotkeys", font="Noto Sans Display", size=13, weight="400", line=18, color="#B6BECE"),
                t(135, 220, 160, 22, "Behaviors", font="Noto Sans Display", size=13, weight="400", line=18, color="#B6BECE"),
                t(135, 250, 160, 22, "Templates", font="Noto Sans Display", size=13, weight="400", line=18, color="#B6BECE"),
                t(135, 280, 160, 22, "About", font="Noto Sans Display", size=13, weight="400", line=18, color="#B6BECE"),
                # Right content (620 wide)
                # Section title
                t(320, 160, 560, 22, "Appearance", font="Noto Sans Display", size=14, weight="700", line=20, color="#E8ECF4"),
                # Row: Theme
                t(320, 200, 200, 22, "Theme", font="Noto Sans Display", size=13, weight="400", line=18, color="#B6BECE"),
                s(620, 196, 200, 30, "#2b3340F2", radius=6, stroke="#3a445866", stroke_w=1),
                t(630, 202, 160, 18, "Nord", font="Noto Sans Display", size=13, weight="400", line=18, color="#E8ECF4"),
                t(800, 202, 20, 18, "▾", font="Noto Sans Display", size=12, weight="400", line=18, color="#B6BECE", align="center"),
                # Row: Switch — Show grid
                t(320, 240, 300, 22, "Show grid", font="Noto Sans Display", size=13, weight="400", line=18, color="#B6BECE"),
                # Switch ON
                s(800, 240, 36, 20, "#285299", radius=12),
                s(818, 242, 16, 16, "#E8ECF4", radius=8),
                # Row: Switch — Auto-save
                t(320, 270, 300, 22, "Auto-save (2s)", font="Noto Sans Display", size=13, weight="400", line=18, color="#B6BECE"),
                s(800, 270, 36, 20, "#285299", radius=12),
                s(818, 272, 16, 16, "#E8ECF4", radius=8),
                # Row: Switch — Show edge labels
                t(320, 300, 300, 22, "Show edge labels", font="Noto Sans Display", size=13, weight="400", line=18, color="#B6BECE"),
                s(800, 300, 36, 20, "#2b334080", radius=12),
                s(802, 302, 16, 16, "#E8ECF4", radius=8),
                # Row: Font size (text-field)
                t(320, 340, 200, 22, "Body font size", font="Noto Sans Display", size=13, weight="400", line=18, color="#B6BECE"),
                s(620, 336, 60, 30, "#0d1117", radius=6, stroke="#3a445866", stroke_w=1),
                t(630, 342, 40, 18, "14", font="Noto Sans Mono", size=13, weight="400", line=18, color="#E8ECF4"),
                # Footer note
                t(320, 530, 560, 16, "Изменения применяются немедленно · persistence: ~/.canvasdesk/settings.json", font="Noto Sans Mono", size=10, weight="400", line=14, color="#6b7280"),
            ],
        },
        # ─── 4. DOCS viewer (FR-031) ───
        {
            "name": "04. DOCS viewer",
            "subtitle": "L4, Block. FR-031: in-app docs viewer (include_str!). Scrollbar 6, close 26",
            "x": 1020, "y": 480, "w": 700, "h": 580,
            "shapes": [
                # Panel
                s(20, 70, 660, 480, "#1a1f29F2", radius=10, stroke="#3a445866", stroke_w=1),
                # Header
                t(40, 85, 500, 22, "Quick start", font="Noto Sans Display", size=14, weight="700", line=20, color="#E8ECF4"),
                s(640, 80, 26, 26, "#2b3340F2", radius=6),
                t(640, 85, 26, 18, "✕", font="Noto Sans Display", size=14, weight="500", line=18, color="#E8ECF4", align="center"),
                # Divider
                s(40, 115, 600, 1, "#3a445866"),
                # Content (text blocks)
                t(40, 125, 580, 22, "Создание первой модели", font="Noto Sans Display", size=13, weight="700", line=20, color="#E8ECF4"),
                t(40, 150, 580, 60, "1. Откройте галерею схем Ctrl+T\n2. Выберите «Unit economics»\n3. Кликните по любой цифре — пересчёт cascades по графу", font="Noto Sans Display", size=12, weight="400", line=18, color="#B6BECE"),
                t(40, 220, 580, 22, "Hotkeys", font="Noto Sans Display", size=13, weight="700", line=20, color="#E8ECF4"),
                t(40, 245, 580, 80, "Ctrl+T — галерея схем\nCtrl+F — поиск по канвасу\nF3 — HUD производительности\nF9 — debug overlay\nEsc — закрыть верхний диалог", font="Noto Sans Mono", size=11, weight="400", line=18, color="#9FD6FF"),
                # Scrollbar (right side, 6px wide)
                s(640, 125, 6, 400, "#2b334080", radius=3),
                s(640, 125, 6, 100, "#3D5070E6", radius=3),  # knob
            ],
        },
        # ─── 5. HELP_MENU ───
        {
            "name": "05. HELP_MENU",
            "subtitle": "L4, Block. Menu из кнопки «?»: Quick start / Templates / Hotkeys / Docs",
            "x": 0, "y": 1080, "w": 400, "h": 360,
            "shapes": [
                # Dim
                s(20, 70, 360, 260, "#00000033"),
                # Menu panel 220×~180
                s(140, 90, 220, 200, "#1a1f29F2", radius=6, stroke="#3a445866", stroke_w=1),
                # Items
                t(155, 100, 200, 26, "Quick start", font="Noto Sans Display", size=13, weight="400", line=18, color="#E8ECF4"),
                s(146, 130, 208, 26, "#3D5070E6", radius=4),
                t(155, 132, 200, 26, "Templates", font="Noto Sans Display", size=13, weight="400", line=18, color="#FFFFFF"),
                t(155, 162, 200, 26, "Hotkeys", font="Noto Sans Display", size=13, weight="400", line=18, color="#E8ECF4"),
                t(155, 194, 200, 26, "Docs", font="Noto Sans Display", size=13, weight="400", line=18, color="#E8ECF4"),
                s(146, 224, 208, 1, "#3a445866"),
                t(155, 230, 200, 26, "About CanvasDesk", font="Noto Sans Display", size=13, weight="400", line=18, color="#B6BECE"),
                t(155, 262, 200, 26, "Report issue", font="Noto Sans Display", size=13, weight="400", line=18, color="#E8ECF4"),
                # Anchor ? button (top-right of menu)
                s(340, 60, 26, 26, "#2b3340F2", radius=6),
                t(340, 65, 26, 18, "?", font="Noto Sans Display", size=14, weight="700", line=18, color="#E8ECF4", align="center"),
            ],
        },
        # ─── 6. HOTKEYS panel ───
        {
            "name": "06. HOTKEYS panel",
            "subtitle": "L3, Capture. Cheat-sheet с хоткеями (toggleable overlay)",
            "x": 420, "y": 1080, "w": 700, "h": 360,
            "shapes": [
                # Panel 580×260
                s(40, 80, 620, 260, "#1a1f29F2", radius=10, stroke="#3a445866", stroke_w=1),
                t(60, 95, 580, 22, "Hotkeys", font="Noto Sans Display", size=14, weight="700", line=20, color="#E8ECF4"),
                s(620, 90, 26, 26, "#2b3340F2", radius=6),
                t(620, 95, 26, 18, "✕", font="Noto Sans Display", size=14, weight="500", line=18, color="#E8ECF4", align="center"),
                # 2-column grid
                t(60, 130, 280, 18, "Ctrl+T", font="Noto Sans Mono", size=12, weight="700", line=18, color="#9FD6FF"),
                t(160, 130, 200, 18, "Scheme gallery", font="Noto Sans Display", size=12, weight="400", line=18, color="#B6BECE"),
                t(60, 152, 280, 18, "Ctrl+F", font="Noto Sans Mono", size=12, weight="700", line=18, color="#9FD6FF"),
                t(160, 152, 200, 18, "Search canvas", font="Noto Sans Display", size=12, weight="400", line=18, color="#B6BECE"),
                t(60, 174, 280, 18, "F3", font="Noto Sans Mono", size=12, weight="700", line=18, color="#9FD6FF"),
                t(160, 174, 200, 18, "Toggle HUD", font="Noto Sans Display", size=12, weight="400", line=18, color="#B6BECE"),
                t(60, 196, 280, 18, "F9", font="Noto Sans Mono", size=12, weight="700", line=18, color="#9FD6FF"),
                t(160, 196, 200, 18, "Debug overlay", font="Noto Sans Display", size=12, weight="400", line=18, color="#B6BECE"),
                # Right column
                t(360, 130, 280, 18, "Ctrl+P", font="Noto Sans Mono", size=12, weight="700", line=18, color="#9FD6FF"),
                t(460, 130, 180, 18, "Template palette", font="Noto Sans Display", size=12, weight="400", line=18, color="#B6BECE"),
                t(360, 152, 280, 18, "Ctrl+,", font="Noto Sans Mono", size=12, weight="700", line=18, color="#9FD6FF"),
                t(460, 152, 180, 18, "Settings", font="Noto Sans Display", size=12, weight="400", line=18, color="#B6BECE"),
                t(360, 174, 280, 18, "Esc", font="Noto Sans Mono", size=12, weight="700", line=18, color="#9FD6FF"),
                t(460, 174, 180, 18, "Close dialog", font="Noto Sans Display", size=12, weight="400", line=18, color="#B6BECE"),
                t(360, 196, 280, 18, "Space", font="Noto Sans Mono", size=12, weight="700", line=18, color="#9FD6FF"),
                t(460, 196, 180, 18, "Toggle bundle view", font="Noto Sans Display", size=12, weight="400", line=18, color="#B6BECE"),
            ],
        },
        # ─── 7. CORNER_BUTTONS ───
        {
            "name": "07. CORNER_BUTTONS",
            "subtitle": "L3, Capture. Угловой кластер ⚙/theme/?/chevron (top-right)",
            "x": 1140, "y": 1080, "w": 400, "h": 200,
            "shapes": [
                # Mini viewport (light gray)
                s(20, 70, 360, 100, "#0f141b"),
                t(40, 100, 320, 16, "CanvasDesk — main viewport", font="Noto Sans Mono", size=10, weight="400", line=14, color="#6b7280"),
                # Corner buttons top-right (gap 8)
                # Settings ⚙
                s(280, 80, 26, 26, "#2b3340F2", radius=6),
                t(280, 85, 26, 18, "⚙", font="Noto Sans Display", size=14, weight="500", line=18, color="#E8ECF4", align="center"),
                # Theme
                s(314, 80, 26, 26, "#2b3340F2", radius=6),
                t(314, 85, 26, 18, "◐", font="Noto Sans Display", size=14, weight="500", line=18, color="#E8ECF4", align="center"),
                # Help ?
                s(348, 80, 26, 26, "#2b3340F2", radius=6),
                t(348, 85, 26, 18, "?", font="Noto Sans Display", size=14, weight="700", line=18, color="#E8ECF4", align="center"),
                # Caption
                t(20, 180, 360, 16, "SETTINGS_BUTTON 36 · margin 12 · gap 8", font="Noto Sans Mono", size=10, weight="400", line=14, color="#6b7280"),
            ],
        },
        # ─── 8. FLOW_MAP ───
        {
            "name": "08. FLOW_MAP (FR-050 Н9-4)",
            "subtitle": "L3, Capture. «Карта проливаний»: панель показывает каскад value-flows по DAG",
            "x": 0, "y": 1300, "w": 700, "h": 360,
            "shapes": [
                s(20, 70, 660, 260, "#1a1f29F2", radius=10, stroke="#3a445866", stroke_w=1),
                t(40, 85, 600, 22, "Flow map", font="Noto Sans Display", size=14, weight="700", line=20, color="#E8ECF4"),
                s(640, 80, 26, 26, "#2b3340F2", radius=6),
                t(640, 85, 26, 18, "✕", font="Noto Sans Display", size=14, weight="500", line=18, color="#E8ECF4", align="center"),
                # Mini graph showing cascade
                # Source
                c(60, 160, 16, "#65A0F7"),
                t(80, 154, 100, 18, "Unit.price", font="Noto Sans Mono", size=11, weight="500", line=18, color="#9FD6FF"),
                # Flow arrow (animated, teal)
                l(140, 167, 60, 2, "#21A88C"),
                # Intermediate node
                c(200, 160, 16, "#21A88C"),
                t(220, 154, 100, 18, "Margin", font="Noto Sans Mono", size=11, weight="500", line=18, color="#9FD6FF"),
                # Flow arrow
                l(280, 167, 60, 2, "#21A88C"),
                # Target
                c(340, 160, 16, "#21A88C"),
                t(360, 154, 100, 18, "LTV", font="Noto Sans Mono", size=11, weight="500", line=18, color="#9FD6FF"),
                # Step indicator
                t(40, 200, 600, 18, "Step 3/5: cascade wave reached LTV (600ms · 200ms step)", font="Noto Sans Mono", size=10, weight="400", line=14, color="#6b7280"),
                # Timeline bar
                s(40, 230, 600, 6, "#2b334080", radius=3),
                s(40, 230, 360, 6, "#21A88C", radius=3),
                t(40, 245, 200, 16, "t=1800ms · 60% complete", font="Noto Sans Mono", size=10, weight="400", line=14, color="#B6BECE"),
            ],
        },
        # ─── 9. EDITOR (editing session + hints popup FR-021) ───
        {
            "name": "09. EDITOR (FR-021 hints popup)",
            "subtitle": "L2, scope. EditingSession: text node в edit mode + FR-021 autocomplete popup",
            "x": 720, "y": 1300, "w": 700, "h": 360,
            "shapes": [
                # Canvas dim
                s(20, 70, 660, 260, "#0f141b"),
                # Edited card (highlighted)
                s(60, 100, 320, 130, "#2b3340F2", radius=10, stroke="#65A0F7", stroke_w=2),
                s(60, 100, 320, 34, "#65A0F72E", radius=10),
                t(70, 108, 300, 22, "CAC", font="Noto Sans Display", size=14, weight="500", line=18, color="#E8ECF4"),
                # Edit text with caret
                t(70, 145, 280, 60, "spend = 350000 RUB/mo\nclients = 200\ncac = spend / cli|", font="Noto Sans Mono", size=12, weight="400", line=18, color="#E8ECF4"),
                # Caret indicator (small accent rect)
                s(280, 195, 2, 18, "#65A0F7"),
                # FR-021 hints popup (autocomplete)
                s(280, 215, 180, 90, "#1a1f29F2", radius=6, stroke="#65A0F780", stroke_w=1),
                # Hint items
                s(282, 217, 176, 22, "#3D5070E6", radius=4),  # selected
                t(288, 220, 160, 18, "clients", font="Noto Sans Mono", size=12, weight="500", line=18, color="#FFFFFF"),
                t(288, 244, 160, 18, "clients_count", font="Noto Sans Mono", size=12, weight="400", line=18, color="#B6BECE"),
                t(288, 265, 160, 18, "$1 ($in ref)", font="Noto Sans Mono", size=11, weight="400", line=18, color="#6b7280"),
                t(288, 286, 160, 18, "mm1(rps, …)", font="Noto Sans Mono", size=11, weight="400", line=18, color="#6b7280"),
                # Source icon (right side of popup)
                c(440, 220, 6, "#65A0F7"),
                # Caption
                t(20, 250, 280, 16, "Tab — принять · Esc — закрыть", font="Noto Sans Mono", size=10, weight="400", line=14, color="#6b7280"),
                t(20, 270, 280, 16, "8 items (vars+functions) · лимит 8", font="Noto Sans Mono", size=10, weight="400", line=14, color="#6b7280"),
            ],
        },
        # ─── 10. EXPLAIN tree (PRD-0007 X2) ───
        {
            "name": "10. EXPLAIN tree (PRD-0007 X2)",
            "subtitle": "L5, Block. Окно проверки цепочки расчёта: дерево происхождения значения",
            "x": 0, "y": 1680, "w": 700, "h": 460,
            "shapes": [
                # Dim
                s(20, 70, 660, 360, "#00000099"),
                # Panel
                s(60, 100, 580, 320, "#1a1f29F2", radius=10, stroke="#3B82F5", stroke_w=1),
                t(80, 115, 540, 22, "Explain · LTV = 216 руб", font="Noto Sans Display", size=14, weight="700", line=20, color="#E8ECF4"),
                s(600, 110, 26, 26, "#2b3340F2", radius=6),
                t(600, 115, 26, 18, "✕", font="Noto Sans Display", size=14, weight="500", line=18, color="#E8ECF4", align="center"),
                # Tree structure
                # Root: ltv = Маржа.margin × Срок.months
                t(80, 150, 540, 18, "ltv = 216  ←  Маржа.margin × Срок.months", font="Noto Sans Mono", size=12, weight="500", line=18, color="#9FD6FF"),
                # Children level 1
                l(120, 180, 1, 30, "#3a445866"),
                t(120, 210, 540, 18, "├ Маржа.margin = 6  ←  Единица.price − Единица.cogs", font="Noto Sans Mono", size=11, weight="400", line=18, color="#B6BECE"),
                t(120, 235, 540, 18, "└ Срок.months = 36  ←  const", font="Noto Sans Mono", size=11, weight="400", line=18, color="#B6BECE"),
                # Children level 2
                l(160, 255, 1, 30, "#3a445866"),
                t(160, 285, 540, 18, "├ Единица.price = 10  ←  const", font="Noto Sans Mono", size=11, weight="400", line=18, color="#6b7280"),
                t(160, 310, 540, 18, "└ Единица.cogs = 4  ←  const", font="Noto Sans Mono", size=11, weight="400", line=18, color="#6b7280"),
                # Leaf badges
                s(560, 285, 50, 18, "#9FD6FF80", radius=9),
                t(560, 286, 50, 18, "leaf", font="Noto Sans Mono", size=10, weight="400", line=12, color="#1f2937", align="center"),
                s(560, 310, 50, 18, "#9FD6FF80", radius=9),
                t(560, 311, 50, 18, "leaf", font="Noto Sans Mono", size=10, weight="400", line=12, color="#1f2937", align="center"),
                # Footer
                t(80, 380, 540, 16, "n42 · 来源: Unit-Economics scheme · recomputed 2ms ago", font="Noto Sans Mono", size=10, weight="400", line=14, color="#6b7280"),
            ],
        },
        # ─── 11. AUTOLINK review dialog ───
        {
            "name": "11. AUTOLINK review (PRD-0007 X4)",
            "subtitle": "L5, Block. Диалог ревью автосвязей: чекбоксы + «apply» / «cancel»",
            "x": 720, "y": 1680, "w": 700, "h": 460,
            "shapes": [
                # Dim
                s(20, 70, 660, 360, "#00000099"),
                # Panel
                s(60, 130, 580, 240, "#1a1f29F2", radius=10, stroke="#3B82F5", stroke_w=1),
                t(80, 145, 540, 22, "Review autolinks", font="Noto Sans Display", size=14, weight="700", line=20, color="#E8ECF4"),
                # Items (checkboxes)
                s(80, 180, 14, 14, "#285299", radius=3),
                t(102, 178, 480, 18, "Маржа.margin → LTV.ltv  (1.0 match)", font="Noto Sans Mono", size=11, weight="400", line=18, color="#E8ECF4"),
                s(80, 205, 14, 14, "#285299", radius=3),
                t(102, 203, 480, 18, "Единица.price → Маржа.margin  (0.92)", font="Noto Sans Mono", size=11, weight="400", line=18, color="#E8ECF4"),
                s(80, 230, 14, 14, "#2b334080", radius=3, stroke="#3a445866", stroke_w=1),
                t(102, 228, 480, 18, "CAC.cac → Ratio.ratio  (0.71)", font="Noto Sans Mono", size=11, weight="400", line=18, color="#B6BECE"),
                s(80, 255, 14, 14, "#2b334080", radius=3, stroke="#3a445866", stroke_w=1),
                t(102, 253, 480, 18, "Срок.months → LTV.ltv  (0.68, below threshold)", font="Noto Sans Mono", size=11, weight="400", line=18, color="#6b7280"),
                # Buttons (apply primary, cancel secondary)
                s(440, 320, 80, 30, "#285299", radius=6),
                t(440, 327, 80, 18, "Apply 2/4", font="Noto Sans Display", size=13, weight="500", line=18, color="#FFFFFF", align="center"),
                s(360, 320, 70, 30, "#2b334080", radius=6, stroke="#596680", stroke_w=1),
                t(360, 327, 70, 18, "Cancel", font="Noto Sans Display", size=13, weight="500", line=18, color="#E8ECF4", align="center"),
                # Note
                t(80, 360, 540, 16, "Undo одним шагом · тег autolink_batch", font="Noto Sans Mono", size=10, weight="400", line=14, color="#6b7280"),
            ],
        },
        # ─── 12. ONBOARDING (FR-028) ───
        {
            "name": "12. ONBOARDING (FR-028)",
            "subtitle": "L5, Block. Карточка онбординга: 8±2 шагов, «один шаг = одна мысль»",
            "x": 0, "y": 2160, "w": 700, "h": 360,
            "shapes": [
                # Dim
                s(20, 70, 660, 260, "#00000099"),
                # Card centered
                s(100, 110, 500, 200, "#1a1f29F2", radius=10, stroke="#3B82F5", stroke_w=1),
                t(120, 125, 460, 22, "Step 3/8 — Add your first node", font="Noto Sans Display", size=14, weight="700", line=20, color="#E8ECF4"),
                # Illustration box
                s(120, 155, 460, 80, "#2b3340F2", radius=6),
                t(140, 175, 420, 40, "📁  Open palette Ctrl+P → drag template to canvas", font="Noto Sans Display", size=12, weight="400", line=18, color="#9FD6FF"),
                # Dots (3 of 8 filled)
                c(280, 250, 8, "#65A0F7"),
                c(295, 250, 8, "#65A0F7"),
                c(310, 250, 8, "#65A0F7"),
                c(325, 250, 8, "#2b334080"),
                c(340, 250, 8, "#2b334080"),
                c(355, 250, 8, "#2b334080"),
                c(370, 250, 8, "#2b334080"),
                c(385, 250, 8, "#2b334080"),
                # Buttons
                s(440, 275, 80, 26, "#285299", radius=6),
                t(440, 281, 80, 18, "Next →", font="Noto Sans Display", size=13, weight="500", line=18, color="#FFFFFF", align="center"),
                t(120, 281, 200, 18, "Skip tour", font="Noto Sans Display", size=12, weight="400", line=18, color="#B6BECE"),
            ],
        },
        # ─── 13. KIT_GALLERY (FR-055) ───
        {
            "name": "13. KIT_GALLERY (FR-055)",
            "subtitle": "L5, Block. Витрина кита: компоненты canvas-ui::kit для интерактивного превью",
            "x": 720, "y": 2160, "w": 700, "h": 360,
            "shapes": [
                s(20, 70, 660, 260, "#1a1f29F2", radius=10, stroke="#3a445866", stroke_w=1),
                t(40, 85, 600, 22, "Kit gallery — interactive components", font="Noto Sans Display", size=14, weight="700", line=20, color="#E8ECF4"),
                s(640, 80, 26, 26, "#2b3340F2", radius=6),
                t(640, 85, 26, 18, "✕", font="Noto Sans Display", size=14, weight="500", line=18, color="#E8ECF4", align="center"),
                # 2x2 grid of component previews
                # Cell 1: button
                s(40, 120, 300, 80, "#2b3340F2", radius=6),
                t(50, 128, 280, 16, "Button", font="Noto Sans Mono", size=10, weight="700", line=14, color="#6b7280"),
                s(50, 155, 80, 30, "#285299", radius=6),
                t(50, 162, 80, 18, "Save", font="Noto Sans Display", size=13, weight="500", line=18, color="#FFFFFF", align="center"),
                s(140, 155, 80, 30, "#333B4A", radius=6),
                t(140, 162, 80, 18, "Cancel", font="Noto Sans Display", size=13, weight="500", line=18, color="#FFFFFF", align="center"),
                # Cell 2: chip
                s(350, 120, 300, 80, "#2b3340F2", radius=6),
                t(360, 128, 280, 16, "Chip", font="Noto Sans Mono", size=10, weight="700", line=14, color="#6b7280"),
                s(360, 155, 80, 24, "#2E4A7AF2", radius=6),
                t(360, 160, 80, 18, "selected", font="Noto Sans Display", size=12, weight="500", line=18, color="#FFFFFF", align="center"),
                s(450, 155, 80, 24, "#2b334080", radius=6),
                t(450, 160, 80, 18, "normal", font="Noto Sans Display", size=12, weight="500", line=18, color="#E8ECF4", align="center"),
                # Cell 3: switch
                s(40, 210, 300, 80, "#2b3340F2", radius=6),
                t(50, 218, 280, 16, "Switch", font="Noto Sans Mono", size=10, weight="700", line=14, color="#6b7280"),
                s(50, 248, 36, 20, "#285299", radius=12),
                s(68, 250, 16, 16, "#E8ECF4", radius=8),
                t(95, 248, 200, 18, "on", font="Noto Sans Display", size=12, weight="400", line=18, color="#B6BECE"),
                s(50, 275, 36, 20, "#2b334080", radius=12),
                s(52, 277, 16, 16, "#E8ECF4", radius=8),
                t(95, 275, 200, 18, "off", font="Noto Sans Display", size=12, weight="400", line=18, color="#B6BECE"),
                # Cell 4: text-field
                s(350, 210, 300, 80, "#2b3340F2", radius=6),
                t(360, 218, 280, 16, "Text-field", font="Noto Sans Mono", size=10, weight="700", line=14, color="#6b7280"),
                s(360, 245, 280, 32, "#0d1117", radius=6, stroke="#65A0F7", stroke_w=2),
                t(370, 252, 260, 18, "Search…|", font="Noto Sans Mono", size=13, weight="400", line=18, color="#E8ECF4"),
            ],
        },
        # ─── 14. ADMIN (FR-070) ───
        {
            "name": "14. ADMIN (FR-070)",
            "subtitle": "L5, Block. UI-админпанель: env vars, feature flags, paths",
            "x": 0, "y": 2540, "w": 700, "h": 360,
            "shapes": [
                s(20, 70, 660, 260, "#1a1f29F2", radius=10, stroke="#3a445866", stroke_w=1),
                t(40, 85, 600, 22, "Admin panel", font="Noto Sans Display", size=14, weight="700", line=20, color="#E8ECF4"),
                s(640, 80, 26, 26, "#2b3340F2", radius=6),
                t(640, 85, 26, 18, "✕", font="Noto Sans Display", size=14, weight="500", line=18, color="#E8ECF4", align="center"),
                # Section: Env vars
                t(40, 120, 600, 18, "Environment", font="Noto Sans Display", size=12, weight="700", line=18, color="#9FD6FF"),
                t(40, 145, 200, 18, "CANVASDESK_DATA_DIR", font="Noto Sans Mono", size=11, weight="400", line=18, color="#B6BECE"),
                t(240, 145, 400, 18, "~/.canvasdesk", font="Noto Sans Mono", size=11, weight="400", line=18, color="#E8ECF4"),
                t(40, 165, 200, 18, "CANVASDESK_LOG_LEVEL", font="Noto Sans Mono", size=11, weight="400", line=18, color="#B6BECE"),
                t(240, 165, 400, 18, "info", font="Noto Sans Mono", size=11, weight="400", line=18, color="#E8ECF4"),
                # Section: Feature flags
                t(40, 200, 600, 18, "Feature flags", font="Noto Sans Display", size=12, weight="700", line=18, color="#9FD6FF"),
                t(40, 225, 300, 18, "ai_autocomplete", font="Noto Sans Mono", size=11, weight="400", line=18, color="#B6BECE"),
                s(420, 222, 36, 20, "#285299", radius=12),
                s(438, 224, 16, 16, "#E8ECF4", radius=8),
                t(40, 248, 300, 18, "scheme_layout_v3", font="Noto Sans Mono", size=11, weight="400", line=18, color="#B6BECE"),
                s(420, 245, 36, 20, "#285299", radius=12),
                s(438, 247, 16, 16, "#E8ECF4", radius=8),
                t(40, 271, 300, 18, "experimental_webgl", font="Noto Sans Mono", size=11, weight="400", line=18, color="#B6BECE"),
                s(420, 268, 36, 20, "#2b334080", radius=12),
                s(422, 270, 16, 16, "#E8ECF4", radius=8),
                t(40, 295, 600, 16, "Только для dev-режима. Изменения требуют перезапуска.", font="Noto Sans Mono", size=10, weight="400", line=14, color="#6b7280"),
            ],
        },
        # ─── 15. EMPTY state ───
        {
            "name": "15. EMPTY state",
            "subtitle": "L3, Capture. Empty canvas card: «Open scheme gallery» CTA",
            "x": 720, "y": 2540, "w": 700, "h": 360,
            "shapes": [
                # Canvas dim
                s(20, 70, 660, 260, "#0f141b"),
                # Empty card centered
                s(160, 130, 380, 160, "#1a1f29F2", radius=10, stroke="#3a445866", stroke_w=1),
                # Icon
                c(340, 150, 32, "#65A0F7"),
                # Title
                t(160, 195, 380, 22, "Start your first model", font="Noto Sans Display", size=16, weight="700", line=22, color="#E8ECF4", align="center"),
                # Body
                t(180, 222, 340, 32, "Open scheme gallery to load a template", font="Noto Sans Display", size=12, weight="400", line=18, color="#B6BECE", align="center"),
                # CTA button
                s(280, 260, 140, 30, "#285299", radius=6),
                t(280, 267, 140, 18, "Open gallery", font="Noto Sans Display", size=13, weight="500", line=18, color="#FFFFFF", align="center"),
                # Hotkey hint
                t(160, 300, 380, 16, "Ctrl+T", font="Noto Sans Mono", size=10, weight="400", line=14, color="#6b7280", align="center"),
            ],
        },
        # ─── 16. TEMPLATE_STRIP (collapsed) ───
        {
            "name": "16. TEMPLATE_STRIP (collapsed)",
            "subtitle": "L3, Capture. Свёрнутая полоса категорий + flyout. PAL_BUTTON 30, PAL_ROW_H 26",
            "x": 0, "y": 2920, "w": 700, "h": 280,
            "shapes": [
                # Canvas
                s(20, 70, 660, 180, "#0f141b"),
                # Strip (left edge)
                s(20, 70, 64, 180, "#1a1f29F2", radius=0, stroke="#3a445866", stroke_w=1),
                # 5 category buttons (PAL_BUTTON 30, gap 6)
                s(26, 80, 36, 30, "#3D5070E6", radius=4),  # active (selected)
                t(26, 86, 36, 18, "infra", font="Noto Sans Display", size=10, weight="500", line=14, color="#FFFFFF", align="center"),
                s(26, 116, 36, 30, "#2b334080", radius=4),
                t(26, 122, 36, 18, "ue", font="Noto Sans Display", size=10, weight="500", line=14, color="#E8ECF4", align="center"),
                s(26, 152, 36, 30, "#2b334080", radius=4),
                t(26, 158, 36, 18, "pa", font="Noto Sans Display", size=10, weight="500", line=14, color="#E8ECF4", align="center"),
                s(26, 188, 36, 30, "#2b334080", radius=4),
                t(26, 194, 36, 18, "plan", font="Noto Sans Display", size=10, weight="500", line=14, color="#E8ECF4", align="center"),
                # Flyout (expanded list of templates for 'infra')
                s(90, 80, 240, 200, "#1a1f29F2", radius=6, stroke="#3a445866", stroke_w=1),
                t(100, 88, 220, 18, "infra · 8 templates", font="Noto Sans Mono", size=10, weight="700", line=14, color="#9FD6FF"),
                s(96, 108, 228, 26, "#3D5070E6", radius=4),  # hover
                t(100, 113, 220, 18, "Load Balancer", font="Noto Sans Display", size=12, weight="500", line=18, color="#FFFFFF"),
                t(100, 140, 220, 18, "API Gateway", font="Noto Sans Display", size=12, weight="400", line=18, color="#E8ECF4"),
                t(100, 165, 220, 18, "Cache (Redis)", font="Noto Sans Display", size=12, weight="400", line=18, color="#E8ECF4"),
                t(100, 190, 220, 18, "DB (Postgres)", font="Noto Sans Display", size=12, weight="400", line=18, color="#E8ECF4"),
                t(100, 215, 220, 18, "DB (NoSQL)", font="Noto Sans Display", size=12, weight="400", line=18, color="#E8ECF4"),
                t(100, 240, 220, 18, "Auth service", font="Noto Sans Display", size=12, weight="400", line=18, color="#E8ECF4"),
                # Caption
                t(20, 250, 660, 16, "flyout open: 150ms hover · 300ms close · drag threshold 4px", font="Noto Sans Mono", size=10, weight="400", line=14, color="#6b7280"),
            ],
        },
        # ─── 17. PALETTE (selection toolbar) ───
        {
            "name": "17. PALETTE (selection toolbar)",
            "subtitle": "L2, Capture. Selection toolbar + open column для действий над выделенным",
            "x": 720, "y": 2920, "w": 700, "h": 280,
            "shapes": [
                # Canvas with 2 selected nodes
                s(20, 70, 660, 180, "#0f141b"),
                # Selected node 1
                s(60, 110, 200, 80, "#2b3340F2", radius=10, stroke="#65A0F7", stroke_w=2),
                t(70, 118, 180, 18, "Unit", font="Noto Sans Display", size=13, weight="500", line=18, color="#E8ECF4"),
                t(70, 145, 180, 40, "price = 10\ncogs = 4", font="Noto Sans Mono", size=11, weight="400", line=18, color="#9FD6FF"),
                # Selected node 2
                s(280, 110, 200, 80, "#2b3340F2", radius=10, stroke="#65A0F7", stroke_w=2),
                t(290, 118, 180, 18, "Margin", font="Noto Sans Display", size=13, weight="500", line=18, color="#E8ECF4"),
                # Selection toolbar (floating, top of selection)
                s(60, 80, 220, 26, "#1a1f29F2", radius=6, stroke="#3a445866", stroke_w=1),
                # Toolbar items
                t(70, 84, 60, 18, "2 sel", font="Noto Sans Mono", size=10, weight="700", line=14, color="#9FD6FF"),
                s(140, 84, 1, 18, "#3a445866"),  # divider
                t(150, 84, 50, 18, "Group", font="Noto Sans Display", size=11, weight="500", line=14, color="#E8ECF4"),
                t(200, 84, 50, 18, "Align", font="Noto Sans Display", size=11, weight="500", line=14, color="#E8ECF4"),
                # Open column (right side)
                s(500, 80, 160, 170, "#1a1f29F2", radius=6, stroke="#3a445866", stroke_w=1),
                t(510, 88, 140, 18, "Actions", font="Noto Sans Display", size=12, weight="700", line=18, color="#E8ECF4"),
                t(510, 110, 140, 18, "• Duplicate", font="Noto Sans Display", size=11, weight="400", line=18, color="#B6BECE"),
                t(510, 130, 140, 18, "• Copy as PNG", font="Noto Sans Display", size=11, weight="400", line=18, color="#B6BECE"),
                t(510, 150, 140, 18, "• Copy as JSON", font="Noto Sans Display", size=11, weight="400", line=18, color="#B6BECE"),
                t(510, 170, 140, 18, "• Group (Ctrl+G)", font="Noto Sans Display", size=11, weight="400", line=18, color="#B6BECE"),
                t(510, 190, 140, 18, "• Align…", font="Noto Sans Display", size=11, weight="400", line=18, color="#B6BECE"),
                t(510, 210, 140, 18, "• Bring to front", font="Noto Sans Display", size=11, weight="400", line=18, color="#B6BECE"),
                t(510, 230, 140, 18, "• Delete (Del)", font="Noto Sans Display", size=11, weight="400", line=18, color="#E55C5C"),
            ],
        },
        # ─── 18. CHOICE_MENU (sub-menu) ───
        {
            "name": "18. CHOICE_MENU (sub-menu)",
            "subtitle": "L4, Block. Подменю для выбора цвета/выравнивания. CHOICE_MENU_TITLE_H 24",
            "x": 0, "y": 3220, "w": 700, "h": 280,
            "shapes": [
                # Canvas dim
                s(20, 70, 660, 180, "#0f141b"),
                # Parent context menu (faded)
                s(40, 80, 170, 130, "#1a1f2999", radius=6, stroke="#3a445866", stroke_w=1),
                t(50, 88, 150, 22, "Align", font="Noto Sans Display", size=13, weight="400", line=18, color="#6b7280"),
                t(50, 110, 150, 22, "Group", font="Noto Sans Display", size=13, weight="400", line=18, color="#6b7280"),
                s(46, 132, 158, 22, "#1a1f29F2", radius=4),  # highlighted (hover)
                t(50, 134, 150, 22, "Color ▸", font="Noto Sans Display", size=13, weight="500", line=18, color="#FFFFFF"),
                t(50, 156, 150, 22, "Bring to front", font="Noto Sans Display", size=13, weight="400", line=18, color="#6b7280"),
                # Sub-menu (choice) — appears to the right
                s(220, 130, 180, 100, "#1a1f29F2", radius=6, stroke="#3a445866", stroke_w=1),
                # Title bar (24)
                s(220, 130, 180, 24, "#2b3340F2", radius=6),
                t(230, 134, 160, 18, "Choose color", font="Noto Sans Display", size=11, weight="500", line=18, color="#9FD6FF"),
                # Color swatches
                c(230, 165, 16, "#65A0F7"),
                c(252, 165, 16, "#21A88C"),
                c(274, 165, 16, "#F5A623"),
                c(296, 165, 16, "#E55C5C"),
                c(318, 165, 16, "#9FD6FF"),
                c(340, 165, 16, "#FFD959"),
                c(362, 165, 16, "#737373"),
                # Bottom options
                t(230, 190, 160, 18, "Auto (by content)", font="Noto Sans Display", size=11, weight="400", line=18, color="#B6BECE"),
                t(230, 208, 160, 18, "Custom…", font="Noto Sans Display", size=11, weight="400", line=18, color="#B6BECE"),
                # Caption
                t(20, 230, 660, 16, "Открывается как подменю/панель с заголовком 24px · глубже одного уровня не вкладывается", font="Noto Sans Mono", size=10, weight="400", line=14, color="#6b7280"),
            ],
        },
    ]
}


# ───────────────── Запуск ─────────────────

def seed_extended(client: PenpotMCP) -> dict[str, Any]:
    js = SEED_JS.replace("__PAGE_JSON__", json.dumps(EXT_PAGE, ensure_ascii=False))
    raw = client.execute_code(js)
    if isinstance(raw, str):
        try:
            data = json.loads(raw)
            if isinstance(data, dict) and "result" in data:
                return data["result"]
            return data
        except json.JSONDecodeError:
            return {"raw_output": raw}
    return raw


if __name__ == "__main__":
    js = SEED_JS.replace("__PAGE_JSON__", json.dumps(EXT_PAGE, ensure_ascii=False))
    total_shapes = sum(len(b.get("shapes", [])) for b in EXT_PAGE["boards"])
    print(f"JS payload size: {len(js)} chars")
    print(f"Page: {EXT_PAGE['name']}")
    print(f"Boards: {len(EXT_PAGE['boards'])}")
    print(f"Total shapes: ~{total_shapes}")
