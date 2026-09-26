/**
 * @file web-onboarding/src/highlight.ts
 * @summary Dim overlay + anchor cutout (Intro.js-like).
 *
 * Implementation: a single fixed-position <div> with an SVG background
 * that paints the dim colour everywhere except the anchor rect (a
 * "donut" via the SVG even-odd fill rule). Updating is cheap: one DOM
 * style write per step transition; per-frame reflow only happens when
 * the host app notifies the engine that the anchor moved (via
 * `refresh()` called from a rAF loop while a step is active).
 *
 * Alternative considered: 4 dim divs around the anchor. Rejected
 * because they break under browser zoom and require 4 layout passes.
 */

import type { HighlightOptions, Rect } from "./types";

const DEFAULTS: Required<HighlightOptions> = {
  padding: 8,
  radius: 6,
  dim: true,
  dimColor: "rgba(0,0,0,0.55)",
  animate: true,
};

export interface HighlightController {
  /** Update the anchor rect (or null = no cutout). */
  setAnchor(rect: Rect | null, options?: HighlightOptions): void;
  /** Hide everything (step passive / scenario end). */
  hide(): void;
  /** Dispose DOM. */
  destroy(): void;
}

export function createHighlight(
  container: HTMLElement,
  baseZIndex: number,
): HighlightController {
  const overlay = document.createElement("div");
  overlay.className = "cd-tour-dim";
  overlay.style.cssText =
    `position:fixed;inset:0;z-index:${baseZIndex};` +
    // FR-075 (дефект найден верификацией витрины 2026-09-26): оверлей создаётся
    // при `new Tour()` — до первого шага сценария paint() ещё не исполнялся,
    // и стартовый pointer-events:auto делал НЕВИДИМЫЙ оверлей на весь экран
    // щитом, глотавшим все клики страницы (канвас мёртв до запуска тура).
    // Неактивный оверлей не перехватывает указатель; paint() вернёт auto,
    // когда шаг сценария затемнит экран.
    `pointer-events:none;background:transparent;` +
    `transition:opacity 180ms ease;opacity:0;`;
  container.appendChild(overlay);

  let current: { rect: Rect | null; opts: Required<HighlightOptions> } | null = null;
  let raf: number | null = null;

  function paint() {
    raf = null;
    if (!current) {
      overlay.style.opacity = "0";
      overlay.style.background = "transparent";
      overlay.style.pointerEvents = "none";
      return;
    }
    const { rect, opts } = current;
    if (!opts.dim) {
      overlay.style.opacity = "0";
      overlay.style.pointerEvents = "none";
      overlay.style.background = "transparent";
      return;
    }
    overlay.style.opacity = "1";
    overlay.style.pointerEvents = "auto";
    overlay.style.transition = opts.animate
      ? "background 180ms ease"
      : "none";
    if (rect === null) {
      overlay.style.background = opts.dimColor;
      return;
    }
    const p = opts.padding;
    const x = rect.x - p;
    const y = rect.y - p;
    const w = rect.width + 2 * p;
    const h = rect.height + 2 * p;
    const r = opts.radius;
    // CSS-only rounded cutout: dim around a transparent rounded rect via
    // a single radial-gradient + box-shadow trick is fragile, so we use
    // an SVG data-URI with even-odd fill — well-supported and crisp.
    const svg =
      `<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%">` +
      `<defs><mask id="m"><rect width="100%" height="100%" fill="white"/>` +
      `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="${r}" ry="${r}" fill="black"/></mask></defs>` +
      `<rect width="100%" height="100%" fill="${escapeCss(opts.dimColor)}" mask="url(#m)"/></svg>`;
    overlay.style.background = `url("data:image/svg+xml;utf8,${encodeURIComponent(svg)}") no-repeat`;
    overlay.style.backgroundSize = "100% 100%";
  }

  function schedulePaint() {
    if (raf !== null) return;
    raf = requestAnimationFrame(paint);
  }

  return {
    setAnchor(rect, options) {
      const opts = { ...DEFAULTS, ...(options ?? {}) } as Required<HighlightOptions>;
      current = { rect, opts };
      schedulePaint();
    },
    hide() {
      current = null;
      schedulePaint();
    },
    destroy() {
      if (raf !== null) cancelAnimationFrame(raf);
      overlay.remove();
    },
  };
}

function escapeCss(c: string): string {
  // The colour ends up inside an SVG attribute; only worry about quotes/backslashes.
  return c.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}
