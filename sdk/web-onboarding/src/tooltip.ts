/**
 * @file web-onboarding/src/tooltip.ts
 * @summary Default tooltip DOM renderer.
 *
 * The tooltip is a small fixed-position card containing:
 *   - title + body
 *   - a small caret (rotated square) pointing at the anchor
 *   - a footer with Back / Skip / Next buttons
 *
 * The renderer returns the root element; the engine positions it via
 * `positioning.computePlacement()` and writes CSS `transform`/`top`/`left`.
 *
 * Custom renderers can replace this whole thing by passing
 * `TourHooks.renderTooltip` — the engine still handles positioning.
 */

import type { TooltipContext } from "./types";

export function renderDefaultTooltip(ctx: TooltipContext): HTMLElement {
  const root = document.createElement("div");
  root.className = "cd-tour-tooltip";
  root.setAttribute("role", "dialog");
  root.setAttribute("aria-modal", "false");

  // Header
  if (ctx.step.title) {
    const h = document.createElement("div");
    h.className = "cd-tour-tooltip__title";
    h.innerHTML = escapeHtml(ctx.step.title);
    root.appendChild(h);
  }
  if (ctx.step.body) {
    const b = document.createElement("div");
    b.className = "cd-tour-tooltip__body";
    b.innerHTML = escapeHtml(ctx.step.body);
    root.appendChild(b);
  }

  // Progress
  const prog = document.createElement("div");
  prog.className = "cd-tour-tooltip__progress";
  prog.textContent = `${ctx.index + 1} / ${ctx.total}`;
  root.appendChild(prog);

  // Caret (rotated square) — actual position/rotation is written by
  // the engine after placement is computed. A data-attr lets CSS know
  // the active direction.
  const caret = document.createElement("div");
  caret.className = "cd-tour-caret";
  root.appendChild(caret);

  // Footer
  const footer = document.createElement("div");
  footer.className = "cd-tour-tooltip__footer";

  if (ctx.skippable && !ctx.step.hideSkip) {
    const skip = document.createElement("button");
    skip.type = "button";
    skip.className = "cd-tour-btn cd-tour-btn_skip";
    skip.textContent = ctx.skipLabel;
    skip.addEventListener("click", ctx.onSkip);
    footer.appendChild(skip);
  }

  if (!ctx.isFirst) {
    const back = document.createElement("button");
    back.type = "button";
    back.className = "cd-tour-btn cd-tour-btn_back";
    back.textContent = ctx.backLabel;
    back.addEventListener("click", ctx.onBack);
    footer.appendChild(back);
  }

  const next = document.createElement("button");
  next.type = "button";
  next.className = "cd-tour-btn cd-tour-btn_primary";
  next.textContent = ctx.isLast ? ctx.doneLabel : ctx.primaryLabel;
  next.addEventListener("click", ctx.onPrimary);
  // If the step is passive (no Next button) we still keep the element
  // for screen readers but visually hide it.
  if (ctx.step.passive) next.style.display = "none";
  footer.appendChild(next);

  root.appendChild(footer);
  return root;
}

function escapeHtml(s: string): string {
  // Tooltip texts are plain text by contract, but the API tolerates HTML
  // for richer scenarios. Escape angle brackets in plain-text path.
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}
