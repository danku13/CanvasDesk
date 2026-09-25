/**
 * @file web-onboarding/src/positioning.ts
 * @summary Tooltip positioning engine — 13 placement modes + auto-flip.
 *
 * Algorithm (Intro.js-like):
 *   1. Compute the anchor's viewport rect (or use the supplied one).
 *   2. Try the requested side; if it would overflow the viewport,
 *      try fallbacks in this order: opposite, top/bottom, left/right,
 *      center.
 *   3. Clamp the resulting position so the tooltip stays inside the
 *      viewport (and below the W6 toolbar if it would overlap).
 *
 * Returned coordinates are CSS px in viewport space (translate to
 * document coordinates by adding window.scrollX/Y — but CanvasDesk
 * uses position:fixed overlays, so viewport coords are correct).
 */

import type { Rect, TooltipSide } from "./types";

const GAP = 12;          // px between anchor edge and tooltip
const VIEWPORT_PAD = 8;  // px margin from viewport edge
const TOOLTIP_MIN_W = 280;
const TOOLTIP_MAX_W = 420;
const TOOLTIP_MIN_H = 120;

export interface Placement {
  side: TooltipSide;
  /** Tooltip top-left in viewport px. */
  x: number;
  y: number;
  /** Recommended width (the engine may override via CSS min/max). */
  width: number;
  /** Measured tooltip height (after a layout probe). */
  height: number;
  /** Where the caret points, in viewport px (null for "center"). */
  caret?: { x: number; y: number; dir: "up" | "down" | "left" | "right" } | null;
}

/**
 * Probe the tooltip element for its natural size. The element should be
 * already mounted and visible (engine does it inside an off-DOM measurement
 * pass: `position: fixed; left: -9999px; opacity: 0`).
 */
export function measureTooltip(
  el: HTMLElement,
  maxW: number = TOOLTIP_MAX_W,
): { width: number; height: number } {
  // Lock width to the max, let height grow with content.
  const prevStyle = el.style.cssText;
  el.style.cssText =
    `position:fixed;left:-9999px;top:-9999px;` +
    `width:${maxW}px;visibility:hidden;opacity:0;`;
  const rect = el.getBoundingClientRect();
  el.style.cssText = prevStyle;
  return {
    width: Math.max(TOOLTIP_MIN_W, Math.min(maxW, Math.ceil(rect.width))),
    height: Math.max(TOOLTIP_MIN_H, Math.ceil(rect.height)),
  };
}

/**
 * Compute the optimal placement for a tooltip.
 *
 * @param side        Requested side.
 * @param anchor      Anchor rect in viewport px (or null = center on screen).
 * @param tipSize     Measured {width, height} from `measureTooltip`.
 * @param viewport    Override viewport size (testing).
 */
export function computePlacement(
  side: TooltipSide,
  anchor: Rect | null,
  tipSize: { width: number; height: number },
  viewport: { width: number; height: number } = {
    width: window.innerWidth,
    height: window.innerHeight,
  },
): Placement {
  if (side === "center" || anchor === null) {
    return {
      side: "center",
      x: Math.round((viewport.width - tipSize.width) / 2),
      y: Math.round((viewport.height - tipSize.height) / 2),
      width: tipSize.width,
      height: tipSize.height,
      caret: null,
    };
  }

  const candidates = orderCandidates(side);
  for (const cand of candidates) {
    const placement = tryPlacement(cand, anchor, tipSize, viewport);
    if (placement) return placement;
  }
  // Last-resort: centre.
  return {
    side: "center",
    x: Math.round((viewport.width - tipSize.width) / 2),
    y: Math.round((viewport.height - tipSize.height) / 2),
    width: tipSize.width,
    height: tipSize.height,
    caret: null,
  };
}

function orderCandidates(side: TooltipSide): TooltipSide[] {
  const opposites: Record<string, TooltipSide> = {
    top: "bottom",
    "top-start": "bottom-start",
    "top-end": "bottom-end",
    bottom: "top",
    "bottom-start": "top-start",
    "bottom-end": "top-end",
    left: "right",
    "left-start": "right-start",
    "left-end": "right-end",
    right: "left",
    "right-start": "left-start",
    "right-end": "left-end",
  };
  return [side, opposites[side] ?? side, "bottom", "top", "right", "left", "center"];
}

function tryPlacement(
  side: TooltipSide,
  anchor: Rect,
  tip: { width: number; height: number },
  vp: { width: number; height: number },
): Placement | null {
  const a = anchor;
  let x: number, y: number, caret: Placement["caret"];

  switch (side) {
    case "top":
      x = a.x + (a.width - tip.width) / 2;
      y = a.y - tip.height - GAP;
      caret = { x: a.x + a.width / 2, y: a.y - GAP, dir: "down" };
      break;
    case "top-start":
      x = a.x;
      y = a.y - tip.height - GAP;
      caret = { x: a.x + 24, y: a.y - GAP, dir: "down" };
      break;
    case "top-end":
      x = a.x + a.width - tip.width;
      y = a.y - tip.height - GAP;
      caret = { x: a.x + a.width - 24, y: a.y - GAP, dir: "down" };
      break;
    case "bottom":
      x = a.x + (a.width - tip.width) / 2;
      y = a.y + a.height + GAP;
      caret = { x: a.x + a.width / 2, y: a.y + a.height + GAP, dir: "up" };
      break;
    case "bottom-start":
      x = a.x;
      y = a.y + a.height + GAP;
      caret = { x: a.x + 24, y: a.y + a.height + GAP, dir: "up" };
      break;
    case "bottom-end":
      x = a.x + a.width - tip.width;
      y = a.y + a.height + GAP;
      caret = { x: a.x + a.width - 24, y: a.y + a.height + GAP, dir: "up" };
      break;
    case "left":
      x = a.x - tip.width - GAP;
      y = a.y + (a.height - tip.height) / 2;
      caret = { x: a.x - GAP, y: a.y + a.height / 2, dir: "right" };
      break;
    case "left-start":
      x = a.x - tip.width - GAP;
      y = a.y;
      caret = { x: a.x - GAP, y: a.y + 24, dir: "right" };
      break;
    case "left-end":
      x = a.x - tip.width - GAP;
      y = a.y + a.height - tip.height;
      caret = { x: a.x - GAP, y: a.y + a.height - 24, dir: "right" };
      break;
    case "right":
      x = a.x + a.width + GAP;
      y = a.y + (a.height - tip.height) / 2;
      caret = { x: a.x + a.width + GAP, y: a.y + a.height / 2, dir: "left" };
      break;
    case "right-start":
      x = a.x + a.width + GAP;
      y = a.y;
      caret = { x: a.x + a.width + GAP, y: a.y + 24, dir: "left" };
      break;
    case "right-end":
      x = a.x + a.width + GAP;
      y = a.y + a.height - tip.height;
      caret = { x: a.x + a.width + GAP, y: a.y + a.height - 24, dir: "left" };
      break;
    default:
      return null;
  }

  // Viewport clamp — if either side overflows, reject this placement.
  const overflowL = x < VIEWPORT_PAD;
  const overflowR = x + tip.width > vp.width - VIEWPORT_PAD;
  const overflowT = y < VIEWPORT_PAD;
  const overflowB = y + tip.height > vp.height - VIEWPORT_PAD;
  if (overflowL || overflowR || overflowT || overflowB) return null;

  return { side, x: Math.round(x), y: Math.round(y), width: tip.width, height: tip.height, caret };
}

/** Helper: get a node's viewport rect (CSS px). */
export function getElementRect(el: HTMLElement): Rect {
  const r = el.getBoundingClientRect();
  return { x: r.left, y: r.top, width: r.width, height: r.height };
}
