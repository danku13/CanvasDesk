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
    {"role": "body",       "size": 14,   "line": 20, "weight": "500", "sample": "Тело ноты, проза"},
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
# ВАЖНО: строка должна быть валидным JS, исполняемым через execute_code.
# execute_code возвращает результат последнего выражения.

SEED_JS_TEMPLATE = r"""
// ===== CanvasDesk design-system seeder (Penpot plugin context) =====
// Все значения — зеркало design/tokens/{colors,dimensions}.json репозитория CanvasDesk.

const __colors = __COLORS_JSON__;
const __typographies = __TYPOGRAPHIES_JSON__;
const __type_scale = __TYPE_SCALE_JSON__;
const __spacing = __SPACING_JSON__;
const __radii = __RADII_JSON__;
const __card_anatomy = __CARD_ANATOMY_JSON__;

const file = penpot.file;
if (!file) throw new Error("No active Penpot file — File → MCP Server → Connect required");

// ─── Helpers ───
function rgbaStr(c) { return `rgba(${Math.round(c[0]*255)}, ${Math.round(c[1]*255)}, ${Math.round(c[2]*255)}, ${c[3]})`; }
function sRGBtoHex(c) {
  const h = (x) => Math.round(x*255).toString(16).padStart(2, '0');
  return `#${h(c[0])}${h(c[1])}${h(c[2])}${c[3] < 1 ? h(c[3]) : ''}`;
}

// ─── 1. Library colors ───
const colorRefs = [];
for (const c of __colors) {
  try {
    const lib = penpot.library.colors.create(c.name);
    const hex = sRGBtoHex(c.rgba);
    // library.colors.create возвращает LibraryColor; add colorStop для opacity<1
    if (c.rgba[3] < 1) {
      lib.addColorStop({ color: `#${sRGBtoHex([c.rgba[0],c.rgba[1],c.rgba[2],1]).slice(1)}`, opacity: c.rgba[3], name: c.name });
    } else {
      lib.addColorStop({ color: hex, opacity: 1.0, name: c.name });
    }
    colorRefs.push({ name: c.name, id: lib.id });
  } catch (e) {
    // Если уже есть — переиспользуем
    colorRefs.push({ name: c.name, id: null, error: String(e) });
  }
}

// ─── 2. Library typographies ───
const typoRefs = [];
for (const t of __typographies) {
  try {
    const lib = penpot.library.typographies.create(t.name);
    lib.fontFamily = t.family;
    lib.fontStyle = "normal";
    lib.fontWeight = parseInt(t.weight, 10);
    lib.fontSize = t.size;
    lib.lineHeight = t.line;
    typoRefs.push({ name: t.name, id: lib.id });
  } catch (e) {
    typoRefs.push({ name: t.name, id: null, error: String(e) });
  }
}

// ─── 3. Создать страницу «CanvasDesk Design System» ───
const page = penpot.pages.create({ name: "CanvasDesk Design System" });
// Активировать страницу, чтобы можно было рисовать
penpot.page = page;

// ─── 4. Board «Color palette» ───
const colorBoard = penpot.boards.create({
  x: 0, y: 0, width: 900, height: 100 + __colors.length * 50,
  name: "Color palette", fill: penpot.colors.white(),
});
let y = 50;
for (const c of __colors) {
  const swatch = penpot.shapes.rect.create({
    x: 50, y, width: 200, height: 36,
    fill: penpot.colors.create({ color: sRGBtoHex(c.rgba.slice(0,3).concat([1])), opacity: c.rgba[3] }),
    radius: 6,
  });
  const label = penpot.shapes.text.create({
    x: 270, y, width: 580, height: 36,
    text: `${c.name}   ${c.hex}`,
    font: "Noto Sans Mono",
    fontSize: 12,
    fontWeight: "400",
    lineHeight: 16,
  });
  y += 50;
}

// ─── 5. Board «Typography scale» ───
const typoBoard = penpot.boards.create({
  x: 1000, y: 0, width: 700, height: 100 + __type_scale.length * 80,
  name: "Typography scale", fill: penpot.colors.white(),
});
let ty = 50;
for (const t of __type_scale) {
  const sample = penpot.shapes.text.create({
    x: 50, y: ty, width: 600, height: t.line + 4,
    text: `${t.sample}`,
    font: t.role === "result" ? "Noto Sans Mono" : "Noto Sans Display",
    fontSize: t.size,
    fontWeight: t.weight,
    lineHeight: t.line,
  });
  const meta = penpot.shapes.text.create({
    x: 50, y: ty + t.line + 4, width: 600, height: 16,
    text: `${t.role} · ${t.size}px / ${t.line}px · ${t.weight}`,
    font: "Noto Sans Mono", fontSize: 10, fontWeight: "400", lineHeight: 14,
  });
  ty += 80;
}

// ─── 6. Board «Spacing & radius» ───
const spacBoard = penpot.boards.create({
  x: 1800, y: 0, width: 600, height: 100 + (__spacing.length + __radii.length) * 70,
  name: "Spacing & radius", fill: penpot.colors.white(),
});
let sy = 50;
for (const s of __spacing) {
  const swatch = penpot.shapes.rect.create({
    x: 50, y: sy, width: s.value, height: 24,
    fill: penpot.colors.create({ color: "#65A0F7", opacity: 0.5 }),
  });
  const label = penpot.shapes.text.create({
    x: 100, y: sy - 4, width: 400, height: 32,
    text: `${s.token} = ${s.value}px   ${s.role}`,
    font: "Noto Sans Mono", fontSize: 12, fontWeight: "400", lineHeight: 16,
  });
  sy += 70;
}
for (const r of __radii) {
  const swatch = penpot.shapes.rect.create({
    x: 50, y: sy, width: 80, height: 40,
    fill: penpot.colors.create({ color: "#65A0F7", opacity: 0.15 }),
    radius: r.value,
  });
  const label = penpot.shapes.text.create({
    x: 150, y: sy + 12, width: 400, height: 16,
    text: `${r.token} = ${r.value}px   ${r.role}`,
    font: "Noto Sans Mono", fontSize: 12, fontWeight: "400", lineHeight: 16,
  });
  sy += 70;
}

// ─── 7. Board «Card anatomy» ───
const cardBoard = penpot.boards.create({
  x: 2500, y: 0,
  width: __card_anatomy.body_width + 100,
  height: __card_anatomy.header_height + 200 + __card_anatomy.result_strip_h,
  name: "Card anatomy", fill: penpot.colors.white(),
});
// Header (34px, accent fill)
const header = penpot.shapes.rect.create({
  x: 50, y: 50,
  width: __card_anatomy.body_width, height: __card_anatomy.header_height,
  fill: penpot.colors.create({ color: "#65A0F7", opacity: 0.18 }),
  radius: __card_anatomy.corner_radius,
});
// Body (padding 10px)
const bodyY = 50 + __card_anatomy.header_height;
const body = penpot.shapes.rect.create({
  x: 50, y: bodyY,
  width: __card_anatomy.body_width, height: 120,
  fill: penpot.colors.white(),
  stroke: penpot.colors.create({ color: "#65A0F7", opacity: 0.40 }),
  strokeSize: 1,
});
// Sample Numi-list текст
const sampleText = penpot.shapes.text.create({
  x: 60, y: bodyY + __card_anatomy.body_padding,
  width: __card_anatomy.body_width - 2 * __card_anatomy.body_padding,
  height: 80,
  text: "rps = 1000 rps\nservice_rate = 1200 rps\nservers = 2",
  font: "Noto Sans Mono", fontSize: 14, fontWeight: "400", lineHeight: 20,
});
// Result strip
const resultStripY = bodyY + 120 + 8;
const resultStrip = penpot.shapes.rect.create({
  x: 50, y: resultStripY,
  width: __card_anatomy.body_width, height: __card_anatomy.result_strip_h,
  fill: penpot.colors.create({ color: "#21A88C", opacity: 0.18 }),
});
const resultText = penpot.shapes.text.create({
  x: 60, y: resultStripY + 3,
  width: __card_anatomy.body_width - 20, height: __card_anatomy.result_strip_h - 6,
  text: "mm1(rps, service_rate, servers) = 0.833",
  font: "Noto Sans Mono", fontSize: 12, fontWeight: "700", lineHeight: 16,
});

return {
  pageId: page.id,
  colors: colorRefs.length,
  typographies: typoRefs.length,
  boards: ["Color palette", "Typography scale", "Spacing & radius", "Card anatomy"],
  cardAnatomy: __card_anatomy,
};
"""


def _inject(client_js: str, replacements: dict[str, str]) -> str:
    """Заменить __PLACEHOLDER__ на JSON-литералы."""
    out = client_js
    for placeholder, json_value in replacements.items():
        out = out.replace(placeholder, json_value)
    return out


def seed_design_system(client: PenpotMCP) -> dict[str, Any]:
    """Залить дизайн-систему CanvasDesk в активный файл Penpot.

    Возвращает результат выполнения JS (созданные ids и счётчики).
    Бросает PenpotMCPError при ошибке (нет подключения и т.п.).
    """
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
    # execute_code возвращает текст; пытаемся распарсить JSON
    if isinstance(raw, str):
        # На случай если вернулся plain text
        try:
            return json.loads(raw)
        except json.JSONDecodeError:
            return {"raw_output": raw}
    return raw


if __name__ == "__main__":
    # Quick sanity test: вывести JS без отправки
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
