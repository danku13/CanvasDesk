#!/usr/bin/env python3
"""
CanvasDesk → Penpot: сидер дизайн-системы.

Заливает в активный файл Penpot:
- Library colors (accent, edge.*, severity.*, dialog, wheel, toast, hud, error)
- Library typographies (5 семейств CanvasDesk)
- Board «Color palette» — образцы всех цветов с подписями
- Board «Typography scale» — title/body/edge_label/result/zone_label
- Board «Spacing & radius» — scale образцы
- Board «Card anatomy» — header (34px) + body (400×N) + result strip

Требует:
1. Активный файл в Penpot (https://design.penpot.app)
2. File → MCP Server → Connect
3. Запуск: PENPOT_MCP_URL=... python scripts/penpot_mcp.py seed-design-system

Дизайн-токены — зеркало design/tokens/colors.json и dimensions.json.
"""
from __future__ import annotations

import json
from typing import Any

from penpot_mcp import PenpotMCP


# ───────────────── Токены CanvasDesk (зеркало design/tokens/*.json) ─────────────────

# Цвета: name → (rgba_float_0_1, hex_display, semantic_role)
COLORS: list[dict[str, Any]] = [
    {"name": "accent",         "rgba": [0.396, 0.612, 0.969, 1.0], "hex": "#65A0F7", "role": "Акцент — выделение/фокус/черновая связь/группы"},
    {"name": "edge.default",   "rgba": [0.52,  0.58,  0.66,  1.0], "hex": "#8594A8", "role": "Нейтральная связь"},
    {"name": "edge.flow",      "rgba": [0.13,  0.66,  0.55,  1.0], "hex": "#21A88C", "role": "Value-ребро (поток значений)"},
    {"name": "edge.draft",     "rgba": [0.396, 0.612, 0.969, 0.7], "hex": "#65A0F7B3", "role": "Резиновая линия (drag новой связи)"},
    {"name": "state.broken",   "rgba": [0.45,  0.45,  0.45,  1.0], "hex": "#737373", "role": "Битая ссылка"},
    {"name": "state.highlight","rgba": [0.85,  0.75,  0.30,  0.30],"hex": "#D9BF4D4D", "role": "Подсветка ==текст=="},
    {"name": "whatif_fill",    "rgba": [0.30,  0.55,  0.95,  0.22],"hex": "#4C8CF2 22%", "role": "What-if строка (FR-017)"},
    {"name": "whatif_chip",    "rgba": [0.30,  0.55,  0.95,  1.0], "hex": "#4C8CF2", "role": "What-if чип активный"},
    {"name": "whatif_badge",   "rgba": [0.875, 0.651, 0.243, 1.0], "hex": "#DFA63E", "role": "Дельта-бейдж what-if"},
    {"name": "error",          "rgba": [0.898, 0.361, 0.361, 1.0], "hex": "#E55C5C", "role": "Строка результата с ошибкой"},
    {"name": "hud",            "rgba": [0.396, 0.612, 0.969, 1.0], "hex": "#659CF8", "role": "F3-оверлей (HUD)"},
    {"name": "pulse_result",   "rgba": [1.0,   0.85,  0.35,  1.0], "hex": "#FFD959", "role": "Рамка пульса свежего результата"},
    {"name": "explain_leaf",   "rgba": [0.624, 0.839, 1.0,   1.0], "hex": "#9FD6FF", "role": "Explain-лист (dark)"},
    {"name": "severity.warning", "rgba": [0.961, 0.651, 0.137, 1.0], "hex": "#F5A623", "role": "Severity warning (dark)"},
    {"name": "severity.danger",  "rgba": [0.898, 0.282, 0.302, 1.0], "hex": "#E5484D", "role": "Severity danger (dark)"},
    {"name": "severity.critical","rgba": [1.0,   0.271, 0.188, 1.0], "hex": "#FF4530", "role": "Severity critical (dark)"},
    {"name": "dialog.fill",         "rgba": [0.09, 0.11, 0.15, 0.97], "hex": "#171C26F2", "role": "Панель диалога (T21)"},
    {"name": "dialog.border",       "rgba": [0.23, 0.51, 0.96, 1.0], "hex": "#3B82F5", "role": "Рамка диалога"},
    {"name": "dialog.btn_primary",  "rgba": [0.16, 0.32, 0.60, 1.0], "hex": "#285299", "role": "Кнопка primary"},
    {"name": "dialog.btn_secondary","rgba": [0.20, 0.23, 0.29, 1.0], "hex": "#333B4A", "role": "Кнопка secondary"},
    {"name": "dialog.btn_border",   "rgba": [0.35, 0.40, 0.50, 1.0], "hex": "#596680", "role": "Рамка кнопок/чипов"},
    {"name": "dialog.text",         "rgba": [0.910, 0.929, 0.957, 1.0], "hex": "#E8ECF4", "role": "Текст диалога"},
    {"name": "dialog.text_muted",   "rgba": [0.714, 0.745, 0.808, 1.0], "hex": "#B6BECE", "role": "Приглушённый текст диалога"},
    {"name": "toast.text",          "rgba": [0.941, 0.902, 0.761, 1.0], "hex": "#F0E6C2", "role": "Текст тоста (тёплая бумага)"},
    {"name": "wheel.dim",        "rgba": [0.0, 0.0, 0.0, 0.35], "hex": "#00000059", "role": "Wheel: дим-диск"},
    {"name": "wheel.category",   "rgba": [0.17, 0.18, 0.22, 0.92], "hex": "#2B2E38EB", "role": "Wheel: сектор категории"},
    {"name": "wheel.template",   "rgba": [0.20, 0.22, 0.27, 0.92], "hex": "#333844EB", "role": "Wheel: сектор шаблона"},
    {"name": "wheel.hover",      "rgba": [0.18, 0.29, 0.48, 0.95], "hex": "#2E4A7AF2", "role": "Wheel: hover/активный хаб"},
    {"name": "wheel.border",     "rgba": [0.22, 0.24, 0.30, 0.90], "hex": "#383D4DE6", "role": "Wheel: рамка сектора"},
]

# Типографика: name → (family, weight, size_px, line_height_px, role)
TYPOGRAPHIES: list[dict[str, Any]] = [
    {"name": "Display Medium",  "family": "Noto Sans Display", "weight": "500", "size": 14, "line": 20, "role": "Базовый UI и канвас-текст (body)"},
    {"name": "Display Bold",    "family": "Noto Sans Display", "weight": "700", "size": 14, "line": 20, "role": "Акцентные слова, заголовки блоков"},
    {"name": "Mono Regular",    "family": "Noto Sans Mono",   "weight": "400", "size": 14, "line": 20, "role": "Numi-строки, значения таблиц, код"},
    {"name": "Mono Bold",       "family": "Noto Sans Mono",   "weight": "700", "size": 14, "line": 20, "role": "Жирные моно-спаны"},
    {"name": "CanvasDesk Oblique", "family": "CanvasDesk Mono Oblique", "weight": "400", "size": 14, "line": 20, "role": "Пролитые значения (синтетический курсив 11°)"},
]

# Типо-шкала CanvasDesk (роль → размер/интерлиньяж)
TYPE_SCALE: list[dict[str, Any]] = [
    {"role": "title",      "size": 16,   "line": 22, "weight": "500", "sample": "Заголовок карточки-ноды"},
    {"role": "body",       "size": 14,   "line": 20, "weight": "500", "sample": "Тело ноды, проза"},
    {"role": "edge_label", "size": 12,   "line": 16, "weight": "500", "sample": "Подписи рёбер"},
    {"role": "result",     "size": 12,   "line": 16, "weight": "400", "sample": "12 345.6 (моно, результат)"},
    {"role": "zone_label", "size": 10.5, "line": 16, "weight": "500", "sample": "ПАРАМЕТРЫ · 3"},
    {"role": "hud",        "size": 14,   "line": 18, "weight": "500", "sample": "60fps · p95 12ms"},
    {"role": "badge",      "size": 10,   "line": 12, "weight": "500", "sample": "+5.2%"},
]

# Spacing scale
SPACING: list[dict[str, Any]] = [
    {"token": "s",  "value": 6,  "role": "Базовый зазор чипов/строк"},
    {"token": "sm", "value": 8,  "role": "Поля таблицы сравнения"},
    {"token": "md", "value": 10, "role": "BAR_PADDING, поля empty-кнопок"},
    {"token": "lg", "value": 12, "role": "BAR_MARGIN, CHIP_PAD_X, PANEL_PAD"},
    {"token": "xl", "value": 24, "role": "Поля панели галереи/empty-карточки"},
]

# Radius scale
RADII: list[dict[str, Any]] = [
    {"token": "chip",  "value": 6,  "role": "Квад чипа/кнопки what-if бара"},
    {"token": "card",  "value": 10, "role": "CORNER_RADIUS cards (FR-075: 8→10)"},
    {"token": "panel", "value": 10, "role": "Панель галереи"},
    {"token": "pill",  "value": 12, "role": "Чипы категорий галереи — пилюли"},
]

# Карта анатомии
CARD_ANATOMY = {
    "corner_radius": 10,
    "header_height": 34,
    "body_width": 400,   # правка 2 от 27.09.2026 (было 300)
    "body_padding": 10,
    "result_strip_h": 22,
}


# ───────────────── JS-код, исполняемый в Penpot plugin context ─────────────────
# API (выявлено через penpot_api_info):
#   penpot.currentFile / penpot.currentPage
#   penpot.createPage() / penpot.openPage(page) (async!)
#   penpot.createBoard() / penpot.createRectangle() / penpot.createText(text?)
#   penpot.library.local.createColor() / createTypography()
#   shape.resize(w,h) / shape.fills = [{fillColor, fillOpacity}]
#   shape.strokes = [{strokeColor, strokeOpacity, strokeStyle, strokeWidth, strokeAlignment}]
#   board.appendChild(child)
#   text.fontFamily/fontSize(STRING)/fontWeight(STRING)/lineHeight(STRING)/growType

SEED_JS_TEMPLATE = r"""
// ===== CanvasDesk design-system seeder (Penpot plugin context) =====
// Все значения — зеркало design/tokens/{colors,dimensions}.json.

const __colors = __COLORS_JSON__;
const __typographies = __TYPOGRAPHIES_JSON__;
const __type_scale = __TYPE_SCALE_JSON__;
const __spacing = __SPACING_JSON__;
const __radii = __RADII_JSON__;
const __card_anatomy = __CARD_ANATOMY_JSON__;

if (!penpot.currentFile) throw new Error("No active Penpot file — File → MCP Server → Connect required");

function sRGBtoHex(arr) {
  const h = (x) => Math.round(x*255).toString(16).padStart(2, '0');
  return `#${h(arr[0])}${h(arr[1])}${h(arr[2])}`;
}
function solidFill(rgba) {
  return { fillColor: sRGBtoHex(rgba), fillOpacity: rgba[3] };
}

// ─── 0. Найти или создать страницу «CanvasDesk Design System» ───
let page = penpot.currentFile.pages.find(p => p.name === "CanvasDesk Design System");
if (!page) {
  page = penpot.createPage();
  page.name = "CanvasDesk Design System";
}
await penpot.openPage(page);

// ─── 0.5. Удалить старые top-level boards с теми же именами (idempotent retry) ───
const boardNames = ["Color palette", "Typography scale", "Spacing & radius", "Card anatomy"];
const root = page.root;
if (root && root.children) {
  for (const child of [...root.children]) {
    if (boardNames.includes(child.name)) {
      try { child.remove(); } catch (e) { /* ignore */ }
    }
  }
}

// ─── 1. Library colors (idempotent: skip existing by name) ───
const lib = penpot.library.local;
const existingColorNames = new Set(lib.colors.map(c => c.name));
const colorRefs = [];
for (const c of __colors) {
  try {
    if (existingColorNames.has(c.name)) {
      colorRefs.push({ name: c.name, status: "exists" });
      continue;
    }
    const lc = lib.createColor();
    lc.name = c.name;
    lc.color = sRGBtoHex(c.rgba);
    lc.opacity = c.rgba[3];
    colorRefs.push({ name: c.name, status: "created", id: lc.id });
  } catch (e) {
    colorRefs.push({ name: c.name, status: "error", error: String(e).slice(0, 120) });
  }
}

// ─── 2. Library typographies (idempotent) ───
const existingTypoNames = new Set(lib.typographies.map(t => t.name));
const typoRefs = [];
for (const t of __typographies) {
  try {
    if (existingTypoNames.has(t.name)) {
      typoRefs.push({ name: t.name, status: "exists" });
      continue;
    }
    const lt = lib.createTypography();
    lt.name = t.name;
    lt.fontFamily = t.family;
    lt.fontWeight = String(t.weight);
    lt.fontSize = String(t.size);
    lt.lineHeight = String(t.line);
    lt.fontStyle = "normal";
    typoRefs.push({ name: t.name, status: "created", id: lt.id });
  } catch (e) {
    typoRefs.push({ name: t.name, status: "error", error: String(e).slice(0, 120) });
  }
}

// ─── 3. Board «Color palette» ───
const colorBoard = penpot.createBoard();
colorBoard.name = "Color palette";
colorBoard.x = 0; colorBoard.y = 0;
colorBoard.resize(900, 100 + __colors.length * 50);
colorBoard.fills = [{ fillColor: "#ffffff", fillOpacity: 1 }];

let cy = 50;
for (const c of __colors) {
  const swatch = penpot.createRectangle();
  swatch.x = 50; swatch.y = cy;
  swatch.resize(200, 36);
  swatch.fills = [solidFill(c.rgba)];
  swatch.borderRadius = 6;
  colorBoard.appendChild(swatch);

  const label = penpot.createText(`${c.name}   ${c.hex}`);
  label.x = 270; label.y = cy;
  label.resize(580, 36);
  label.fontFamily = "Noto Sans Mono";
  label.fontSize = "12"; label.fontWeight = "400"; label.lineHeight = "16";
  label.growType = "auto-height";
  label.fills = [{ fillColor: "#1f2937", fillOpacity: 1 }];
  colorBoard.appendChild(label);

  cy += 50;
}

// ─── 4. Board «Typography scale» ───
const typoBoard = penpot.createBoard();
typoBoard.name = "Typography scale";
typoBoard.x = 1000; typoBoard.y = 0;
typoBoard.resize(700, 100 + __type_scale.length * 80);
typoBoard.fills = [{ fillColor: "#ffffff", fillOpacity: 1 }];

let ty = 50;
for (const t of __type_scale) {
  const sample = penpot.createText(t.sample);
  sample.x = 50; sample.y = ty;
  sample.resize(600, t.line + 4);
  sample.fontFamily = t.role === "result" ? "Noto Sans Mono" : "Noto Sans Display";
  sample.fontSize = String(t.size);
  sample.fontWeight = String(t.weight);
  sample.lineHeight = String(t.line);
  sample.growType = "auto-height";
  sample.fills = [{ fillColor: "#1f2937", fillOpacity: 1 }];
  typoBoard.appendChild(sample);

  const meta = penpot.createText(`${t.role} · ${t.size}px / ${t.line}px · ${t.weight}`);
  meta.x = 50; meta.y = ty + t.line + 6;
  meta.resize(600, 16);
  meta.fontFamily = "Noto Sans Mono";
  meta.fontSize = "10"; meta.fontWeight = "400"; meta.lineHeight = "14";
  meta.growType = "auto-height";
  meta.fills = [{ fillColor: "#6b7280", fillOpacity: 1 }];
  typoBoard.appendChild(meta);

  ty += 80;
}

// ─── 5. Board «Spacing & radius» ───
const spacBoard = penpot.createBoard();
spacBoard.name = "Spacing & radius";
spacBoard.x = 1800; spacBoard.y = 0;
spacBoard.resize(600, 100 + (__spacing.length + __radii.length) * 70);
spacBoard.fills = [{ fillColor: "#ffffff", fillOpacity: 1 }];

let sy = 50;
for (const s of __spacing) {
  const swatch = penpot.createRectangle();
  swatch.x = 50; swatch.y = sy;
  swatch.resize(s.value, 24);
  swatch.fills = [{ fillColor: "#65A0F7", fillOpacity: 0.5 }];
  spacBoard.appendChild(swatch);

  const label = penpot.createText(`${s.token} = ${s.value}px   ${s.role}`);
  label.x = 100; label.y = sy - 4;
  label.resize(450, 32);
  label.fontFamily = "Noto Sans Mono";
  label.fontSize = "12"; label.fontWeight = "400"; label.lineHeight = "16";
  label.growType = "auto-height";
  label.fills = [{ fillColor: "#1f2937", fillOpacity: 1 }];
  spacBoard.appendChild(label);

  sy += 70;
}
for (const r of __radii) {
  const swatch = penpot.createRectangle();
  swatch.x = 50; swatch.y = sy;
  swatch.resize(80, 40);
  swatch.fills = [{ fillColor: "#65A0F7", fillOpacity: 0.15 }];
  swatch.borderRadius = r.value;
  spacBoard.appendChild(swatch);

  const label = penpot.createText(`${r.token} = ${r.value}px   ${r.role}`);
  label.x = 150; label.y = sy + 12;
  label.resize(400, 16);
  label.fontFamily = "Noto Sans Mono";
  label.fontSize = "12"; label.fontWeight = "400"; label.lineHeight = "16";
  label.growType = "auto-height";
  label.fills = [{ fillColor: "#1f2937", fillOpacity: 1 }];
  spacBoard.appendChild(label);

  sy += 70;
}

// ─── 6. Board «Card anatomy» ───
const cardBoard = penpot.createBoard();
cardBoard.name = "Card anatomy";
cardBoard.x = 2500; cardBoard.y = 0;
cardBoard.resize(
  __card_anatomy.body_width + 100,
  __card_anatomy.header_height + 200 + __card_anatomy.result_strip_h
);
cardBoard.fills = [{ fillColor: "#ffffff", fillOpacity: 1 }];

// Header (34px, accent fill α0.18)
const header = penpot.createRectangle();
header.x = 50; header.y = 50;
header.resize(__card_anatomy.body_width, __card_anatomy.header_height);
header.fills = [{ fillColor: "#65A0F7", fillOpacity: 0.18 }];
header.borderRadius = __card_anatomy.corner_radius;
cardBoard.appendChild(header);

// Body (white with accent stroke)
const bodyY = 50 + __card_anatomy.header_height;
const body = penpot.createRectangle();
body.x = 50; body.y = bodyY;
body.resize(__card_anatomy.body_width, 120);
body.fills = [{ fillColor: "#ffffff", fillOpacity: 1 }];
body.strokes = [{ strokeColor: "#65A0F7", strokeOpacity: 0.40, strokeStyle: "solid", strokeWidth: 1, strokeAlignment: "center" }];
cardBoard.appendChild(body);

// Sample Numi-list текст
const sampleText = penpot.createText("rps = 1000 rps\nservice_rate = 1200 rps\nservers = 2");
sampleText.x = 60; sampleText.y = bodyY + __card_anatomy.body_padding;
sampleText.resize(__card_anatomy.body_width - 2 * __card_anatomy.body_padding, 80);
sampleText.fontFamily = "Noto Sans Mono";
sampleText.fontSize = "14"; sampleText.fontWeight = "400"; sampleText.lineHeight = "20";
sampleText.growType = "auto-height";
sampleText.fills = [{ fillColor: "#1f2937", fillOpacity: 1 }];
cardBoard.appendChild(sampleText);

// Result strip
const resultStripY = bodyY + 120 + 8;
const resultStrip = penpot.createRectangle();
resultStrip.x = 50; resultStrip.y = resultStripY;
resultStrip.resize(__card_anatomy.body_width, __card_anatomy.result_strip_h);
resultStrip.fills = [{ fillColor: "#21A88C", fillOpacity: 0.18 }];
cardBoard.appendChild(resultStrip);

const resultText = penpot.createText("mm1(rps, service_rate, servers) = 0.833");
resultText.x = 60; resultText.y = resultStripY + 3;
resultText.resize(__card_anatomy.body_width - 20, __card_anatomy.result_strip_h - 6);
resultText.fontFamily = "Noto Sans Mono";
resultText.fontSize = "12"; resultText.fontWeight = "700"; resultText.lineHeight = "16";
resultText.growType = "auto-height";
resultText.fills = [{ fillColor: "#0d5b4f", fillOpacity: 1 }];
cardBoard.appendChild(resultText);

return {
  pageId: page.id,
  pageName: page.name,
  colors: colorRefs,
  typographies: typoRefs,
  boards: boardNames,
  cardAnatomy: __card_anatomy,
};
"""


def _inject(client_js: str, replacements: dict[str, str]) -> str:
    out = client_js
    for placeholder, json_value in replacements.items():
        out = out.replace(placeholder, json_value)
    return out


def seed_design_system(client: PenpotMCP) -> dict[str, Any]:
    """Залить дизайн-систему CanvasDesk в активный файл Penpot."""
    js = _inject(
        SEED_JS_TEMPLATE,
        {
            "__COLORS_JSON__": json.dumps(COLORS, ensure_ascii=False),
            "__TYPOGRAPHIES_JSON__": json.dumps(TYPOGRAPHIES, ensure_ascii=False),
            "__TYPE_SCALE_JSON__": json.dumps(TYPE_SCALE, ensure_ascii=False),
            "__SPACING_JSON__": json.dumps(SPACING, ensure_ascii=False),
            "__RADII_JSON__": json.dumps(RADII, ensure_ascii=False),
            "__CARD_ANATOMY_JSON__": json.dumps(CARD_ANATOMY, ensure_ascii=False),
        },
    )
    raw = client.execute_code(js)
    if isinstance(raw, str):
        try:
            return json.loads(raw)
        except json.JSONDecodeError:
            return {"raw_output": raw}
    return raw


if __name__ == "__main__":
    js = _inject(
        SEED_JS_TEMPLATE,
        {
            "__COLORS_JSON__": json.dumps(COLORS, ensure_ascii=False),
            "__TYPOGRAPHIES_JSON__": json.dumps(TYPOGRAPHIES, ensure_ascii=False),
            "__TYPE_SCALE_JSON__": json.dumps(TYPE_SCALE, ensure_ascii=False),
            "__SPACING_JSON__": json.dumps(SPACING, ensure_ascii=False),
            "__RADII_JSON__": json.dumps(RADII, ensure_ascii=False),
            "__CARD_ANATOMY_JSON__": json.dumps(CARD_ANATOMY, ensure_ascii=False),
        },
    )
    print(f"JS payload size: {len(js)} chars")
    print(f"Colors: {len(COLORS)}, Typographies: {len(TYPOGRAPHIES)}, Type scale: {len(TYPE_SCALE)}")
    print(f"Spacing tokens: {len(SPACING)}, Radius tokens: {len(RADII)}")
