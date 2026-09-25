/**
 * @file web-onboarding/src/actions.ts
 * @summary Action runner — perform + waitFor.
 *
 * Two halves, both async, both cancelable:
 *
 *   performActions(steps): runs a list of PerformSpec atomically. If any
 *   throws, the engine aborts the step transition and surfaces a warning
 *   via TourOptions.log. performClick / performFocus simulate real user
 *   input (focus + mousedown/up + click) so the host app sees a normal
 *   event sequence — important for code that listens to "mousedown"
 *   rather than "click" (e.g. canvas drag start).
 *
 *   awaitWaitFor(spec): resolves when the gate is satisfied. The engine
 *   polls every 60ms for predicate/selector variants; for events and
 *   signals, it listens once. Timeout → reject with a typed error; the
 *   Tour engine treats rejection as "user can still click Next manually
 *   after a 5s soft-fail" — but only if autoAdvance=false.
 *
 * flows.sh influence: the engine never _assumes_ an action succeeded. It
 * always pairs perform+waitFor: "click X" then "wait for X to emit signal"
 * — so a buggy selector doesn't desync the tour.
 */

import type {
  PerformSpec, WaitForSpec,
} from "./types";

export interface ActionContext {
  /** Emit a signal into the engine's bus (subscribe via onSignal). */
  signal: (name: string, payload?: unknown) => void;
  log: (level: "info" | "warn" | "error", message: string) => void;
}

export class WaitForError extends Error {
  constructor(public spec: WaitForSpec, message: string) {
    super(message);
    this.name = "WaitForError";
  }
}

export async function performActions(
  actions: PerformSpec[],
  ctx: ActionContext,
): Promise<void> {
  for (const a of actions) {
    try {
      await performOne(a, ctx);
    } catch (e) {
      ctx.log("error", `perform failed: ${(e as Error).message}`);
      throw e;
    }
  }
}

async function performOne(a: PerformSpec, ctx: ActionContext): Promise<void> {
  switch (a.kind) {
    case "click": {
      const el = resolveElement(a);
      if (!el) throw new Error(`click: target not found`);
      simulateClick(el);
      ctx.log("info", `perform: click ${describe(el)}`);
      return;
    }
    case "focus": {
      const el = a.element ?? (a.selector ? document.querySelector<HTMLElement>(a.selector) : null);
      if (!el) throw new Error(`focus: target not found`);
      el.focus({ preventScroll: false });
      ctx.log("info", `perform: focus ${describe(el)}`);
      return;
    }
    case "signal": {
      ctx.signal(a.signal, a.payload);
      ctx.log("info", `perform: signal ${a.signal}`);
      return;
    }
    case "dispatch": {
      const el = resolveElement(a as PerformSpec & { kind: "dispatch" });
      if (!el) throw new Error(`dispatch: target not found`);
      const init = a.init ?? {};
      const event = new Event(a.eventType, init);
      el.dispatchEvent(event);
      ctx.log("info", `perform: dispatch ${a.eventType}`);
      return;
    }
  }
}

function resolveElement(a: PerformSpec & { target?: string; selector?: string; element?: HTMLElement }): HTMLElement | null {
  if (a.target === "element" && a.element) return a.element;
  if (a.selector) return document.querySelector<HTMLElement>(a.selector);
  return null;
}

function simulateClick(el: HTMLElement): void {
  // Mirror real input: focus → mousedown → mouseup → click.
  // Some hosts (canvas-drag libraries) listen to "mousedown" to start
  // a drag; a synthetic "click" alone is invisible to them.
  try { (el as HTMLElement).focus?.({ preventScroll: true }); } catch { /* noop */ }
  const opts: MouseEventInit = { bubbles: true, cancelable: true, view: window, button: 0 };
  el.dispatchEvent(new MouseEvent("mousedown", opts));
  el.dispatchEvent(new MouseEvent("mouseup", opts));
  el.dispatchEvent(new MouseEvent("click", opts));
}

function describe(el: HTMLElement): string {
  if (el.id) return `#${el.id}`;
  if (el.className && typeof el.className === "string") {
    return `.${el.className.split(" ")[0]}`;
  }
  return el.tagName.toLowerCase();
}

/**
 * Wait for a contract to be satisfied.
 *
 * Returns a promise + a cancel function. The engine uses the cancel
 * function when the user clicks Next / Skip while the gate is still
 * pending.
 */
export function awaitWaitFor(
  spec: WaitForSpec,
  ctx: ActionContext,
): { promise: Promise<void>; cancel: () => void } {
  let cancelled = false;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let poll: ReturnType<typeof setInterval> | null = null;
  let cleanup: (() => void) | null = null;
  const timeout = spec.timeout ?? 10000;

  const promise = new Promise<void>((resolve, reject) => {
    function finishOk() {
      doCleanup();
      resolve();
    }
    function finishErr(e: Error) {
      doCleanup();
      reject(e);
    }
    function doCleanup() {
      if (timer) clearTimeout(timer);
      if (poll) clearInterval(poll);
      if (cleanup) cleanup();
    }

    if (cancelled) {
      finishErr(new Error("waitFor cancelled"));
      return;
    }

    timer = setTimeout(() => {
      finishErr(new WaitForError(spec, `waitFor timed out after ${timeout}ms`));
    }, timeout);

    switch (spec.kind) {
      case "selector": {
        // Initial check (selector may already exist).
        if (document.querySelector(spec.selector)) {
          finishOk();
          return;
        }
        poll = setInterval(() => {
          if (cancelled) return finishErr(new Error("waitFor cancelled"));
          if (document.querySelector(spec.selector)) finishOk();
        }, 60);
        return;
      }
      case "predicate": {
        if (spec.predicate()) {
          finishOk();
          return;
        }
        poll = setInterval(() => {
          if (cancelled) return finishErr(new Error("waitFor cancelled"));
          if (spec.predicate()) finishOk();
        }, 60);
        return;
      }
      case "event": {
        const target: EventTarget =
          spec.target === "document" ? document :
          spec.target === "window" ? window :
          spec.target;
        const handler = () => finishOk();
        target.addEventListener(spec.eventType, handler, { once: true });
        cleanup = () => target.removeEventListener(spec.eventType, handler);
        return;
      }
      case "signal": {
        // Subscribe to the engine's signal bus. The bus is on `ctx`,
        // exposed via a closure; the engine injects a "subscribe" function
        // in the action context (see Tour.run).
        const unsub = (ctx as unknown as {
          subscribe?: (name: string, cb: () => void) => () => void;
        }).subscribe;
        if (!unsub) {
          finishErr(new Error("waitFor(signal): engine has no signal bus"));
          return;
        }
        const off = unsub(spec.signals[0], () => finishOk());
        // If multiple signals are listed, subscribe to each.
        for (let i = 1; i < spec.signals.length; i++) {
          unsub(spec.signals[i], () => finishOk());
        }
        cleanup = off;
        return;
      }
    }
  });

  function cancel() {
    cancelled = true;
    // Cleanup happens via the promise's doCleanup() — but we trigger
    // it explicitly here too in case the promise hasn't been awaited.
    if (timer) clearTimeout(timer);
    if (poll) clearInterval(poll);
    if (cleanup) cleanup();
  }

  return { promise, cancel };
}
