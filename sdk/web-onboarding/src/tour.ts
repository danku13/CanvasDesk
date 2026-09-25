/**
 * @file web-onboarding/src/tour.ts
 * @summary The Tour engine.
 *
 * State machine:
 *
 *      ┌───────────────────────┐
 *      │  idle (no scenario)   │
 *      └──────────┬────────────┘
 *                 │ run(scenario)
 *                 ▼
 *      ┌───────────────────────┐
 *   ┌──│ active: step i        │◀──┐
 *   │  └─────┬─────────┬────────┘   │
 *   │        │next     │skip/esc    │
 *   │        ▼         ▼            │
 *   │  ┌──────────┐  ┌──────────┐   │
 *   │  │active:i+1│  │ done     │   │
 *   │  └─────┬────┘  └──────────┘   │
 *   │        │last                   │
 *   │        ▼                       │
 *   │  ┌──────────┐                  │
 *   └──│prev/back │                  │
 *      └──────────┘                  │
 *                                     │
 *                                     │
 *      cancel() ─────────────────────┘ (no callbacks)
 *
 * Per-step activation:
 *   1. Tear down the previous step's DOM (tooltip, advance-listeners).
 *   2. Resolve the anchor (selector/element/rect/none).
 *   3. Paint the highlight (or hide it for passive/no-anchor steps).
 *   4. Mount the tooltip off-DOM, measure it, compute placement,
 *      then move it on-screen with a transition.
 *   5. Run `perform[]` actions.
 *   6. If `waitFor` is set: register a gate (disable the Next button
 *      until it resolves; auto-advance if scenario.autoAdvance=true).
 *   7. Wire keyboard: Esc → skip, →/Enter → next, ← → back.
 *   8. If `advanceOnClick` is set, listen on that element for click →
 *      call next().
 */

import type {
  AnchorSpec, TourHandle, TourHooks, TourOptions, TourScenario, TourStep,
} from "./types";
import {
  computePlacement, getElementRect, measureTooltip,
} from "./positioning";
import { createHighlight, type HighlightController } from "./highlight";
import { renderDefaultTooltip } from "./tooltip";
import {
  awaitWaitFor, performActions, type ActionContext, WaitForError,
} from "./actions";
import { DEFAULT_STYLES } from "./styles";

type SignalListener = (name: string, payload: unknown) => void;

export class Tour {
  private container: HTMLElement;
  private zIndex: number;
  private log: NonNullable<TourOptions["log"]>;
  private hooks: TourHooks;
  private highlight: HighlightController;
  private active: ActiveState | null = null;
  private keyHandler: ((e: KeyboardEvent) => void) | null = null;
  private refreshRaf: number | null = null;
  private signalBus: Set<SignalListener> = new Set();

  // Singleton bus for cross-engine use (rare; the engine is normally
  // a single instance per page).
  static instance: Tour | null = null;

  constructor(opts: TourOptions = {}) {
    this.container = opts.container ?? document.body;
    this.zIndex = opts.baseZIndex ?? 10000;
    this.log = opts.log ?? ((lvl, msg) => console[lvl]?.(`[tour] ${msg}`));
    this.hooks = {};
    this.highlight = createHighlight(this.container, this.zIndex + 1);
    injectStyles(this.container, opts.styles ?? DEFAULT_STYLES);
    Tour.instance = this;
  }

  /** Run a scenario. Returns a handle for programmatic control. */
  run(scenario: TourScenario): TourHandle {
    if (this.active) {
      this.log("warn", `scenario "${this.active.scenario.id}" still active; cancelling`);
      this.cancel();
    }
    const handle = this.start(scenario);
    scenario.onStart?.(handle);
    return handle;
  }

  /** Subscribe to engine signals. Returns an unsubscribe fn. */
  onSignal(listener: SignalListener): () => void {
    this.signalBus.add(listener);
    return () => this.signalBus.delete(listener);
  }

  /** Emit a signal into the bus. */
  signal(name: string, payload?: unknown): void {
    this.log("info", `signal: ${name}`);
    for (const l of this.signalBus) l(name, payload);
  }

  /** Refresh the current step's anchor (called from a rAF loop while
   *  the host app animates canvas-internal elements). */
  refresh(): void {
    if (!this.active) return;
    const a = this.active;
    const rect = resolveAnchor(a.step.anchor, this.hooks);
    a.lastAnchorRect = rect;
    this.highlight.setAnchor(rect, a.step.highlight ?? undefined);
    if (a.tooltipEl) {
      this.placeTooltip(a.tooltipEl, a.step.side ?? "bottom", rect);
    }
  }

  private start(scenario: TourScenario): TourHandle {
    const handle: TourHandle = {
      scenario,
      get step() { return active.step; },
      get index() { return active.index; },
      next: () => this.next(),
      prev: () => this.prev(),
      skip: () => this.skip(),
      resolve: (sig) => this.resolveWait(sig),
      signal: (n, p) => this.signal(n, p),
      cancel: () => this.cancel(),
    };

    const active: ActiveState = {
      scenario,
      step: scenario.steps[0],
      index: 0,
      tooltipEl: null,
      lastAnchorRect: null,
      wait: null,
      advanceUnsub: null,
      handle,
    };
    this.active = active;

    // Wire keyboard.
    this.keyHandler = (e: KeyboardEvent) => this.onKey(e);
    document.addEventListener("keydown", this.keyHandler, true);

    // Start a refresh rAF (recomputes tooltip position when viewport
    // resizes or the host app scrolls/animates).
    this.startRefreshRaf();

    // Activate the first step.
    this.activateStep(0);

    return handle;
  }

  private next(): void {
    if (!this.active) return;
    const a = this.active;
    if (a.wait && !a.wait.resolved) {
      // Pending gate: ignore the click (the gate must complete first).
      this.log("info", `next() ignored — waitFor pending`);
      return;
    }
    if (a.index >= a.scenario.steps.length - 1) {
      this.complete();
      return;
    }
    this.activateStep(a.index + 1);
  }

  private prev(): void {
    if (!this.active) return;
    if (this.active.index <= 0) return;
    this.activateStep(this.active.index - 1);
  }

  private skip(): void {
    if (!this.active) return;
    const a = this.active;
    this.cleanup();
    a.scenario.onSkip?.(a.handle);
  }

  private complete(): void {
    if (!this.active) return;
    const a = this.active;
    this.cleanup();
    a.scenario.onComplete?.(a.handle);
    this.signal(`tour:${a.scenario.id}:complete`);
  }

  private cancel(): void {
    if (!this.active) return;
    this.cleanup();
  }

  private resolveWait(sig?: string): void {
    if (!this.active || !this.active.wait) return;
    this.active.wait.resolved = true;
    this.active.wait.cancel?.();
    if (sig) this.log("info", `waitFor resolved via signal "${sig}"`);
    else this.log("info", `waitFor resolved externally`);
    if (this.active.scenario.autoAdvance) {
      this.next();
    }
  }

  private onKey(e: KeyboardEvent): void {
    if (!this.active) return;
    // Allow host to handle key combos with modifiers.
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    switch (e.key) {
      case "Escape":
        e.preventDefault();
        e.stopPropagation();
        if (this.active.scenario.skippable ?? true) this.skip();
        break;
      case "Enter":
      case "ArrowRight":
        e.preventDefault();
        e.stopPropagation();
        this.next();
        break;
      case "ArrowLeft":
        e.preventDefault();
        e.stopPropagation();
        this.prev();
        break;
    }
  }

  private activateStep(index: number): void {
    if (!this.active) return;
    const a = this.active;
    // Tear down previous step's per-step DOM.
    if (a.tooltipEl) {
      a.tooltipEl.remove();
      a.tooltipEl = null;
    }
    if (a.advanceUnsub) {
      a.advanceUnsub();
      a.advanceUnsub = null;
    }
    if (a.wait) {
      a.wait.cancel?.();
      a.wait = null;
    }

    a.index = index;
    const step = a.scenario.steps[index];
    a.step = step;
    a.scenario.onStep?.(step, index, a.handle);

    // Resolve anchor.
    const rect = resolveAnchor(step.anchor, this.hooks);
    a.lastAnchorRect = rect;
    this.highlight.setAnchor(rect, step.highlight ?? undefined);

    // Build the tooltip.
    const ctx = {
      container: this.container,
      step, index, total: a.scenario.steps.length,
      side: step.side ?? "bottom",
      anchor: rect,
      primaryLabel: step.primaryLabel ?? a.scenario.primaryLabel ?? "Далее",
      skipLabel: a.scenario.skipLabel ?? "Пропустить",
      backLabel: a.scenario.backLabel ?? "Назад",
      doneLabel: a.scenario.doneLabel ?? "Готово",
      isLast: index === a.scenario.steps.length - 1,
      isFirst: index === 0,
      skippable: a.scenario.skippable ?? true,
      onPrimary: () => this.next(),
      onBack: () => this.prev(),
      onSkip: () => this.skip(),
    };
    const tooltipEl = this.hooks.renderTooltip
      ? this.hooks.renderTooltip(ctx) ?? renderDefaultTooltip(ctx)
      : renderDefaultTooltip(ctx);
    tooltipEl.style.zIndex = String(this.zIndex + 2);
    a.tooltipEl = tooltipEl;
    this.container.appendChild(tooltipEl);

    // Position it.
    this.placeTooltip(tooltipEl, ctx.side, rect);

    // perform[] actions.
    if (step.perform && step.perform.length > 0) {
      const actx: ActionContext = {
        signal: (n, p) => this.signal(n, p),
        log: this.log,
        // Provide a "subscribe" hook so waitFor(signal) can hook the bus.
        // Cast-escape because ActionContext's public shape doesn't expose
        // it; the action runner reads it via duck-typing.
        ...(this as unknown as { subscribe: unknown }),
        subscribe: (n: string, cb: () => void) => this.subscribeForWait(n, cb),
      };
      performActions(step.perform, actx).catch((e) => {
        if (e instanceof WaitForError) {
          this.log("warn", `perform WaitForError: ${e.message}`);
        } else {
          this.log("error", `perform crashed: ${(e as Error).message}`);
        }
      });
    }

    // waitFor gate.
    if (step.waitFor) {
      const wait = awaitWaitFor(step.waitFor, {
        signal: (n, p) => this.signal(n, p),
        log: this.log,
        // The action runner looks up `subscribe` via duck-typing.
        subscribe: (n: string, cb: () => void) => this.subscribeForWait(n, cb),
      } as unknown as ActionContext);
      a.wait = {
        resolved: false,
        cancel: wait.cancel,
      };
      wait.promise
        .then(() => {
          if (!a.wait) return;
          a.wait.resolved = true;
          if (a.scenario.autoAdvance) this.next();
        })
        .catch((e) => {
          if (e instanceof WaitForError) {
            this.log("warn", `waitFor soft-failed: ${e.message} — Next still clickable`);
          } else {
            this.log("error", `waitFor crashed: ${(e as Error).message}`);
          }
        });
    }

    // advanceOnClick.
    if (step.advanceOnClick) {
      const target = typeof step.advanceOnClick === "string"
        ? document.querySelector<HTMLElement>(step.advanceOnClick)
        : step.advanceOnClick;
      if (target) {
        const handler = () => this.next();
        target.addEventListener("click", handler, { once: true });
        a.advanceUnsub = () => target.removeEventListener("click", handler);
      } else {
        this.log("warn", `advanceOnClick: target not found`);
      }
    }
  }

  private subscribeForWait(name: string, cb: () => void): () => void {
    const off = this.onSignal((sigName) => {
      if (sigName === name) cb();
    });
    return off;
  }

  private placeTooltip(
    el: HTMLElement, side: NonNullable<TourStep["side"]>, rect: AnchorRectish,
  ): void {
    const size = measureTooltip(el);
    const placement = computePlacement(side, rect, size);
    el.style.position = "fixed";
    el.style.left = `${placement.x}px`;
    el.style.top = `${placement.y}px`;
    el.style.width = `${placement.width}px`;
    el.dataset.caretDir = placement.caret?.dir ?? "none";
    if (placement.caret) {
      const caret = el.querySelector<HTMLElement>(".cd-tour-caret");
      if (caret) {
        // Caret is a rotated square placed at the anchor-edge of the tooltip.
        const localX = placement.caret.x - placement.x;
        const localY = placement.caret.y - placement.y;
        caret.style.left = `${localX}px`;
        caret.style.top = `${localY}px`;
      }
    }
  }

  private startRefreshRaf(): void {
    if (this.refreshRaf !== null) return;
    const tick = () => {
      if (!this.active) {
        this.refreshRaf = null;
        return;
      }
      this.refresh();
      this.refreshRaf = requestAnimationFrame(tick);
    };
    this.refreshRaf = requestAnimationFrame(tick);
  }

  private cleanup(): void {
    if (!this.active) return;
    const a = this.active;
    if (a.tooltipEl) a.tooltipEl.remove();
    if (a.wait) a.wait.cancel?.();
    if (a.advanceUnsub) a.advanceUnsub();
    this.highlight.hide();
    if (this.keyHandler) {
      document.removeEventListener("keydown", this.keyHandler, true);
      this.keyHandler = null;
    }
    if (this.refreshRaf !== null) {
      cancelAnimationFrame(this.refreshRaf);
      this.refreshRaf = null;
    }
    this.active = null;
  }
}

interface ActiveState {
  scenario: TourScenario;
  step: TourStep;
  index: number;
  tooltipEl: HTMLElement | null;
  lastAnchorRect: AnchorRectish;
  wait: { resolved: boolean; cancel?: () => void } | null;
  advanceUnsub: (() => void) | null;
  handle: TourHandle;
}

type AnchorRectish = import("./types").Rect | null;

function resolveAnchor(
  spec: AnchorSpec | undefined,
  hooks: TourHooks,
): AnchorRectish {
  if (!spec) return null;
  // Hosts may register a custom resolver for non-DOM anchors (e.g.
  // canvas-internal element bounding boxes exposed via a hook).
  if (hooks.resolveAnchor) {
    const r = hooks.resolveAnchor(spec);
    if (r) return resolveAnchorRect(r);
  }
  switch (spec.kind) {
    case "rect":
      return spec.rect;
    case "element":
      return getElementRect(spec.element);
    case "selector": {
      const el = document.querySelector<HTMLElement>(spec.selector);
      if (el) return getElementRect(el);
      if (spec.fallback) return spec.fallback;
      return null;
    }
  }
}

function resolveAnchorRect(spec: AnchorSpec): AnchorRectish {
  switch (spec.kind) {
    case "rect": return spec.rect;
    case "element": return getElementRect(spec.element);
    case "selector": {
      const el = document.querySelector<HTMLElement>(spec.selector);
      return el ? getElementRect(el) : (spec.fallback ?? null);
    }
  }
}

let stylesInjected = false;
function injectStyles(container: HTMLElement, css: string): void {
  if (stylesInjected) return;
  const style = document.createElement("style");
  style.dataset.cdTourStyles = "1";
  style.textContent = css;
  (container.getRootNode() as Document | ShadowRoot).appendChild(style);
  stylesInjected = true;
}
