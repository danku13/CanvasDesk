// CanvasDesk inline onboarding engine — see sdk/web-onboarding/README.md
"use strict";
var CanvasDeskTour = (() => {
  var __defProp = Object.defineProperty;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  var __export = (target, all) => {
    for (var name in all)
      __defProp(target, name, { get: all[name], enumerable: true });
  };
  var __copyProps = (to, from, except, desc) => {
    if (from && typeof from === "object" || typeof from === "function") {
      for (let key of __getOwnPropNames(from))
        if (!__hasOwnProp.call(to, key) && key !== except)
          __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
    }
    return to;
  };
  var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

  // src/index.ts
  var src_exports = {};
  __export(src_exports, {
    DEFAULT_STYLES: () => DEFAULT_STYLES,
    Tour: () => Tour,
    WaitForError: () => WaitForError,
    awaitWaitFor: () => awaitWaitFor,
    computePlacement: () => computePlacement,
    createHighlight: () => createHighlight,
    getElementRect: () => getElementRect,
    measureTooltip: () => measureTooltip,
    performActions: () => performActions,
    renderDefaultTooltip: () => renderDefaultTooltip,
    scenarios: () => scenarios_exports
  });

  // src/positioning.ts
  var GAP = 12;
  var VIEWPORT_PAD = 8;
  var TOOLTIP_MIN_W = 280;
  var TOOLTIP_MAX_W = 420;
  var TOOLTIP_MIN_H = 120;
  function measureTooltip(el, maxW = TOOLTIP_MAX_W) {
    const prevStyle = el.style.cssText;
    el.style.cssText = `position:fixed;left:-9999px;top:-9999px;width:${maxW}px;visibility:hidden;opacity:0;`;
    const rect = el.getBoundingClientRect();
    el.style.cssText = prevStyle;
    return {
      width: Math.max(TOOLTIP_MIN_W, Math.min(maxW, Math.ceil(rect.width))),
      height: Math.max(TOOLTIP_MIN_H, Math.ceil(rect.height))
    };
  }
  function computePlacement(side, anchor, tipSize, viewport = {
    width: window.innerWidth,
    height: window.innerHeight
  }) {
    if (side === "center" || anchor === null) {
      return {
        side: "center",
        x: Math.round((viewport.width - tipSize.width) / 2),
        y: Math.round((viewport.height - tipSize.height) / 2),
        width: tipSize.width,
        height: tipSize.height,
        caret: null
      };
    }
    const candidates = orderCandidates(side);
    for (const cand of candidates) {
      const placement = tryPlacement(cand, anchor, tipSize, viewport);
      if (placement) return placement;
    }
    return {
      side: "center",
      x: Math.round((viewport.width - tipSize.width) / 2),
      y: Math.round((viewport.height - tipSize.height) / 2),
      width: tipSize.width,
      height: tipSize.height,
      caret: null
    };
  }
  function orderCandidates(side) {
    var _a;
    const opposites = {
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
      "right-end": "left-end"
    };
    return [side, (_a = opposites[side]) != null ? _a : side, "bottom", "top", "right", "left", "center"];
  }
  function tryPlacement(side, anchor, tip, vp) {
    const a = anchor;
    let x, y, caret;
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
    const overflowL = x < VIEWPORT_PAD;
    const overflowR = x + tip.width > vp.width - VIEWPORT_PAD;
    const overflowT = y < VIEWPORT_PAD;
    const overflowB = y + tip.height > vp.height - VIEWPORT_PAD;
    if (overflowL || overflowR || overflowT || overflowB) return null;
    return { side, x: Math.round(x), y: Math.round(y), width: tip.width, height: tip.height, caret };
  }
  function getElementRect(el) {
    const r = el.getBoundingClientRect();
    return { x: r.left, y: r.top, width: r.width, height: r.height };
  }

  // src/highlight.ts
  var DEFAULTS = {
    padding: 8,
    radius: 6,
    dim: true,
    dimColor: "rgba(0,0,0,0.55)",
    animate: true
  };
  function createHighlight(container, baseZIndex) {
    const overlay = document.createElement("div");
    overlay.className = "cd-tour-dim";
    overlay.style.cssText = `position:fixed;inset:0;z-index:${baseZIndex};pointer-events:none;background:transparent;transition:opacity 180ms ease;opacity:0;`;
    container.appendChild(overlay);
    let current = null;
    let raf = null;
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
      overlay.style.transition = opts.animate ? "background 180ms ease" : "none";
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
      const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%"><defs><mask id="m"><rect width="100%" height="100%" fill="white"/><rect x="${x}" y="${y}" width="${w}" height="${h}" rx="${r}" ry="${r}" fill="black"/></mask></defs><rect width="100%" height="100%" fill="${escapeCss(opts.dimColor)}" mask="url(#m)"/></svg>`;
      overlay.style.background = `url("data:image/svg+xml;utf8,${encodeURIComponent(svg)}") no-repeat`;
      overlay.style.backgroundSize = "100% 100%";
    }
    function schedulePaint() {
      if (raf !== null) return;
      raf = requestAnimationFrame(paint);
    }
    return {
      setAnchor(rect, options) {
        const opts = { ...DEFAULTS, ...options != null ? options : {} };
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
      }
    };
  }
  function escapeCss(c) {
    return c.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  }

  // src/tooltip.ts
  function renderDefaultTooltip(ctx) {
    const root = document.createElement("div");
    root.className = "cd-tour-tooltip";
    root.setAttribute("role", "dialog");
    root.setAttribute("aria-modal", "false");
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
    const prog = document.createElement("div");
    prog.className = "cd-tour-tooltip__progress";
    prog.textContent = `${ctx.index + 1} / ${ctx.total}`;
    root.appendChild(prog);
    const caret = document.createElement("div");
    caret.className = "cd-tour-caret";
    root.appendChild(caret);
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
    if (ctx.step.passive) next.style.display = "none";
    footer.appendChild(next);
    root.appendChild(footer);
    return root;
  }
  function escapeHtml(s) {
    return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }

  // src/actions.ts
  var WaitForError = class extends Error {
    constructor(spec, message) {
      super(message);
      this.spec = spec;
      this.name = "WaitForError";
    }
  };
  async function performActions(actions, ctx) {
    for (const a of actions) {
      try {
        await performOne(a, ctx);
      } catch (e) {
        ctx.log("error", `perform failed: ${e.message}`);
        throw e;
      }
    }
  }
  async function performOne(a, ctx) {
    var _a, _b;
    switch (a.kind) {
      case "click": {
        const el = resolveElement(a);
        if (!el) throw new Error(`click: target not found`);
        simulateClick(el);
        ctx.log("info", `perform: click ${describe(el)}`);
        return;
      }
      case "focus": {
        const el = (_a = a.element) != null ? _a : a.selector ? document.querySelector(a.selector) : null;
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
        const el = resolveElement(a);
        if (!el) throw new Error(`dispatch: target not found`);
        const init = (_b = a.init) != null ? _b : {};
        const event = new Event(a.eventType, init);
        el.dispatchEvent(event);
        ctx.log("info", `perform: dispatch ${a.eventType}`);
        return;
      }
    }
  }
  function resolveElement(a) {
    if (a.target === "element" && a.element) return a.element;
    if (a.selector) return document.querySelector(a.selector);
    return null;
  }
  function simulateClick(el) {
    var _a;
    try {
      (_a = el.focus) == null ? void 0 : _a.call(el, { preventScroll: true });
    } catch (e) {
    }
    const opts = { bubbles: true, cancelable: true, view: window, button: 0 };
    el.dispatchEvent(new MouseEvent("mousedown", opts));
    el.dispatchEvent(new MouseEvent("mouseup", opts));
    el.dispatchEvent(new MouseEvent("click", opts));
  }
  function describe(el) {
    if (el.id) return `#${el.id}`;
    if (el.className && typeof el.className === "string") {
      return `.${el.className.split(" ")[0]}`;
    }
    return el.tagName.toLowerCase();
  }
  function awaitWaitFor(spec, ctx) {
    var _a;
    let cancelled = false;
    let timer = null;
    let poll = null;
    let cleanup = null;
    const timeout = (_a = spec.timeout) != null ? _a : 1e4;
    const promise = new Promise((resolve, reject) => {
      function finishOk() {
        doCleanup();
        resolve();
      }
      function finishErr(e) {
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
          const target = spec.target === "document" ? document : spec.target === "window" ? window : spec.target;
          const handler = () => finishOk();
          target.addEventListener(spec.eventType, handler, { once: true });
          cleanup = () => target.removeEventListener(spec.eventType, handler);
          return;
        }
        case "signal": {
          const unsub = ctx.subscribe;
          if (!unsub) {
            finishErr(new Error("waitFor(signal): engine has no signal bus"));
            return;
          }
          const off = unsub(spec.signals[0], () => finishOk());
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
      if (timer) clearTimeout(timer);
      if (poll) clearInterval(poll);
      if (cleanup) cleanup();
    }
    return { promise, cancel };
  }

  // src/styles.ts
  var DEFAULT_STYLES = `
.cd-tour-tooltip {
  position: fixed;
  background: #1d2128;
  color: #d5d9e0;
  border: 1px solid #333a44;
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0,0,0,0.45);
  padding: 14px 16px 10px;
  font: 13px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif;
  max-width: 420px;
  min-width: 280px;
  pointer-events: auto;
  opacity: 1;
  transform: translateY(0);
  transition: opacity 180ms ease, transform 180ms ease;
}
.cd-tour-tooltip[hidden] { display: none; }

.cd-tour-tooltip__title {
  font-weight: 600;
  font-size: 14px;
  color: #f3f5f8;
  margin-bottom: 4px;
}
.cd-tour-tooltip__body {
  color: #b9c0cb;
  white-space: normal;
  word-wrap: break-word;
}
.cd-tour-tooltip__progress {
  font-size: 11px;
  color: #6b7480;
  margin-top: 6px;
}
.cd-tour-tooltip__footer {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  align-items: center;
  margin-top: 12px;
  border-top: 1px solid #2a3138;
  padding-top: 8px;
}

.cd-tour-btn {
  -webkit-appearance: none;
  appearance: none;
  font: inherit;
  padding: 6px 12px;
  border-radius: 6px;
  border: 1px solid #333a44;
  background: #262c35;
  color: #d5d9e0;
  cursor: pointer;
  /* W-e (\u0434\u0435\u0444\u0435\u043A\u0442 \u211614 \u0430\u0443\u0434\u0438\u0442\u0430 ui-kit): \u0442\u0430\u0447-\u0446\u0435\u043B\u044C \u226544\xD744 CSS px (WCAG 2.5.5 /
   * Apple HIG). border-box \u2014 min-height \u0432\u043C\u0435\u0441\u0442\u0435 \u0441 \u0440\u0430\u043C\u043A\u043E\u0439; min-width \u2014
   * \u0433\u043E\u0440\u0438\u0437\u043E\u043D\u0442\u0430\u043B\u044C \u0442\u043E\u0439 \u0436\u0435 \u0446\u0435\u043B\u0438 (\u043A\u043E\u0440\u043E\u0442\u043A\u0438\u0435 \xAB\u041D\u0430\u0437\u0430\u0434\xBB/\xABBack\xBB \u043D\u0435 \u0441\u0436\u0438\u043C\u0430\u044E\u0442\u0441\u044F). */
  box-sizing: border-box;
  min-height: 44px;
  min-width: 44px;
  -webkit-user-select: none;
  user-select: none;
  -webkit-tap-highlight-color: transparent;
}
.cd-tour-btn:hover { background: #2e353f; }
.cd-tour-btn:focus-visible {
  outline: 2px solid #8ab4ff;
  outline-offset: 1px;
}
.cd-tour-btn_primary {
  background: #2f6fda;
  border-color: #2f6fda;
  color: #ffffff;
  font-weight: 600;
}
.cd-tour-btn_primary:hover { background: #3a78e0; }
.cd-tour-btn_skip {
  background: transparent;
  border-color: transparent;
  color: #6b7480;
}
.cd-tour-btn_back {
  background: transparent;
}

.cd-tour-caret {
  position: absolute;
  width: 12px;
  height: 12px;
  background: #1d2128;
  border: 1px solid #333a44;
  transform: translate(-50%, -50%) rotate(45deg);
  pointer-events: none;
  opacity: 0;
  transition: opacity 180ms ease;
}
.cd-tour-tooltip[data-caret-dir="up"]    .cd-tour-caret,
.cd-tour-tooltip[data-caret-dir="down"]  .cd-tour-caret,
.cd-tour-tooltip[data-caret-dir="left"]  .cd-tour-caret,
.cd-tour-tooltip[data-caret-dir="right"]  .cd-tour-caret {
  opacity: 1;
}
.cd-tour-tooltip[data-caret-dir="up"]    .cd-tour-caret { border-bottom: none; border-right: none; }
.cd-tour-tooltip[data-caret-dir="down"]  .cd-tour-caret { border-top: none; border-left: none; }
.cd-tour-tooltip[data-caret-dir="left"]  .cd-tour-caret { border-top: none; border-right: none; }
.cd-tour-tooltip[data-caret-dir="right"] .cd-tour-caret { border-bottom: none; border-left: none; }
.cd-tour-tooltip[data-caret-dir="none"]  .cd-tour-caret { display: none; }

.cd-tour-dim {
  cursor: pointer;
}

/* Touch / small screens */
@media (max-width: 480px), (pointer: coarse) {
  .cd-tour-tooltip {
    min-width: 0;
    max-width: calc(100vw - 32px);
    padding: 12px 14px 8px;
  }
  .cd-tour-btn {
    padding: 10px 14px;
    min-height: 44px;
  }
}
`;

  // src/tour.ts
  var _Tour = class _Tour {
    constructor(opts = {}) {
      this.active = null;
      this.keyHandler = null;
      this.refreshRaf = null;
      this.signalBus = /* @__PURE__ */ new Set();
      var _a, _b, _c, _d, _e;
      this.container = (_a = opts.container) != null ? _a : document.body;
      this.zIndex = (_b = opts.baseZIndex) != null ? _b : 1e4;
      this.log = (_c = opts.log) != null ? _c : (lvl, msg) => {
        var _a2;
        return (_a2 = console[lvl]) == null ? void 0 : _a2.call(console, `[tour] ${msg}`);
      };
      this.hooks = (_d = opts.hooks) != null ? _d : {};
      this.storageKey = opts.storageKey === void 0 ? "cd-tour:" : opts.storageKey;
      this.highlight = createHighlight(this.container, this.zIndex + 1);
      injectStyles(this.container, (_e = opts.styles) != null ? _e : DEFAULT_STYLES);
      _Tour.instance = this;
    }
    // ── persistence ────────────────────────────────────────────────
    /** Check if scenario was previously completed. */
    isCompleted(id) {
      return this.getCompletedAt(id) !== null;
    }
    /** Get completion timestamp (ms epoch) or null. */
    getCompletedAt(id) {
      const raw = this.readStorage(`${this.storageKey}done:${id}`);
      if (raw === null) return null;
      const ts = Number(raw);
      return Number.isFinite(ts) ? ts : null;
    }
    /** Mark scenario as completed (manual override — also called
     *  automatically by the engine on normal completion). */
    markCompleted(id) {
      this.writeStorage(`${this.storageKey}done:${id}`, String(Date.now()));
    }
    /** Reset completion + resume state for a scenario (re-enable
     *  auto-show, fresh start). */
    resetCompleted(id) {
      this.removeStorage(`${this.storageKey}done:${id}`);
      this.clearResume(id);
    }
    /** Get the saved step index for a scenario (resume), or null. */
    getResumeIndex(id) {
      const raw = this.readStorage(`${this.storageKey}resume:${id}`);
      if (raw === null) return null;
      const idx = Number(raw);
      return Number.isFinite(idx) && idx >= 0 ? idx : null;
    }
    /** Clear resume state for a scenario (called on completion). */
    clearResume(id) {
      this.removeStorage(`${this.storageKey}resume:${id}`);
    }
    // ── run ─────────────────────────────────────────────────────────
    /** Run a scenario. Returns a handle for programmatic control, or
     *  null if `skipIfCompleted` was set and the scenario was already
     *  completed. */
    run(scenario, opts = {}) {
      var _a, _b;
      if (opts.skipIfCompleted && this.isCompleted(scenario.id)) {
        this.log("info", `scenario "${scenario.id}" already completed; skipping (use force to re-run)`);
        return null;
      }
      if (this.active) {
        this.log("warn", `scenario "${this.active.scenario.id}" still active; cancelling`);
        this.cancel();
      }
      const startIdx = opts.resume ? (_a = this.getResumeIndex(scenario.id)) != null ? _a : 0 : 0;
      if (opts.resume && startIdx > 0) {
        this.log("info", `resuming "${scenario.id}" from step ${startIdx + 1}`);
      }
      const handle = this.start(scenario, startIdx, opts);
      (_b = scenario.onStart) == null ? void 0 : _b.call(scenario, handle);
      return handle;
    }
    /** Subscribe to engine signals. Returns an unsubscribe fn. */
    onSignal(listener) {
      this.signalBus.add(listener);
      return () => this.signalBus.delete(listener);
    }
    /** Emit a signal into the bus. */
    signal(name, payload) {
      this.log("info", `signal: ${name}`);
      for (const l of this.signalBus) l(name, payload);
    }
    /** Refresh the current step's anchor (called from a rAF loop while
     *  the host app animates canvas-internal elements). */
    refresh() {
      var _a;
      if (!this.active) return;
      const a = this.active;
      const rect = resolveAnchor(a.step.anchor, this.hooks);
      a.lastAnchorRect = rect;
      const opts = a.step.highlight ? { ...a.step.highlight } : {};
      if (a.step.passive && (opts.dim === void 0 || opts.dim === null)) {
        opts.dim = false;
      }
      this.highlight.setAnchor(rect, opts);
      if (a.tooltipEl) {
        this.placeTooltip(a.tooltipEl, (_a = a.step.side) != null ? _a : "bottom", rect);
      }
    }
    start(scenario, startIdx = 0, runOpts = {}) {
      var _a;
      const safeIdx = Math.max(0, Math.min(startIdx, scenario.steps.length - 1));
      const handle = {
        scenario,
        get step() {
          return active.step;
        },
        get index() {
          return active.index;
        },
        next: () => this.next(),
        prev: () => this.prev(),
        skip: () => this.skip(),
        resolve: (sig) => this.resolveWait(sig),
        signal: (n, p) => this.signal(n, p),
        cancel: () => this.cancel()
      };
      const active = {
        scenario,
        step: scenario.steps[safeIdx],
        index: safeIdx,
        tooltipEl: null,
        lastAnchorRect: null,
        wait: null,
        advanceUnsub: null,
        handle,
        runOpts: {
          markCompletedOnSkip: (_a = runOpts.markCompletedOnSkip) != null ? _a : false
        }
      };
      this.active = active;
      this.keyHandler = (e) => this.onKey(e);
      document.addEventListener("keydown", this.keyHandler, true);
      this.startRefreshRaf();
      this.activateStep(safeIdx);
      return handle;
    }
    next() {
      if (!this.active) return;
      const a = this.active;
      if (a.wait && !a.wait.resolved) {
        this.log("info", `next() ignored \u2014 waitFor pending`);
        return;
      }
      if (a.index >= a.scenario.steps.length - 1) {
        this.complete();
        return;
      }
      this.activateStep(a.index + 1);
    }
    prev() {
      if (!this.active) return;
      if (this.active.index <= 0) return;
      this.activateStep(this.active.index - 1);
    }
    skip() {
      var _a, _b, _c;
      if (!this.active) return;
      const a = this.active;
      this.writeStorage(
        `${this.storageKey}resume:${a.scenario.id}`,
        String(a.index)
      );
      if ((_a = a.runOpts) == null ? void 0 : _a.markCompletedOnSkip) {
        this.markCompleted(a.scenario.id);
        this.clearResume(a.scenario.id);
      }
      this.cleanup();
      (_c = (_b = a.scenario).onSkip) == null ? void 0 : _c.call(_b, a.handle);
    }
    complete() {
      var _a, _b;
      if (!this.active) return;
      const a = this.active;
      if (this.storageKey) {
        this.markCompleted(a.scenario.id);
        this.clearResume(a.scenario.id);
      }
      this.cleanup();
      (_b = (_a = a.scenario).onComplete) == null ? void 0 : _b.call(_a, a.handle);
      this.signal(`tour:${a.scenario.id}:complete`);
    }
    cancel() {
      if (!this.active) return;
      this.cleanup();
    }
    resolveWait(sig) {
      var _a, _b;
      if (!this.active || !this.active.wait) return;
      this.active.wait.resolved = true;
      (_b = (_a = this.active.wait).cancel) == null ? void 0 : _b.call(_a);
      if (sig) this.log("info", `waitFor resolved via signal "${sig}"`);
      else this.log("info", `waitFor resolved externally`);
      if (this.active.scenario.autoAdvance) {
        this.next();
      }
    }
    onKey(e) {
      var _a;
      if (!this.active) return;
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      switch (e.key) {
        case "Escape":
          e.preventDefault();
          e.stopPropagation();
          if ((_a = this.active.scenario.skippable) != null ? _a : true) this.skip();
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
    activateStep(index) {
      var _a, _b, _c, _d, _e, _f, _g, _h, _i, _j, _k, _l, _m, _n;
      if (!this.active) return;
      const a = this.active;
      if (a.tooltipEl) {
        a.tooltipEl.remove();
        a.tooltipEl = null;
      }
      if (a.advanceUnsub) {
        a.advanceUnsub();
        a.advanceUnsub = null;
      }
      if (a.wait) {
        (_b = (_a = a.wait).cancel) == null ? void 0 : _b.call(_a);
        a.wait = null;
      }
      a.index = index;
      const step = a.scenario.steps[index];
      a.step = step;
      (_d = (_c = a.scenario).onStep) == null ? void 0 : _d.call(_c, step, index, a.handle);
      if (this.storageKey) {
        this.writeStorage(
          `${this.storageKey}resume:${a.scenario.id}`,
          String(index)
        );
      }
      const rect = resolveAnchor(step.anchor, this.hooks);
      a.lastAnchorRect = rect;
      const highlightOpts = (_e = step.highlight) != null ? _e : {};
      if (step.passive) {
        (_f = highlightOpts.dim) != null ? _f : highlightOpts.dim = false;
      }
      this.highlight.setAnchor(rect, highlightOpts);
      const ctx = {
        container: this.container,
        step,
        index,
        total: a.scenario.steps.length,
        side: (_g = step.side) != null ? _g : "bottom",
        anchor: rect,
        primaryLabel: (_i = (_h = step.primaryLabel) != null ? _h : a.scenario.primaryLabel) != null ? _i : "\u0414\u0430\u043B\u0435\u0435",
        skipLabel: (_j = a.scenario.skipLabel) != null ? _j : "\u041F\u0440\u043E\u043F\u0443\u0441\u0442\u0438\u0442\u044C",
        backLabel: (_k = a.scenario.backLabel) != null ? _k : "\u041D\u0430\u0437\u0430\u0434",
        doneLabel: (_l = a.scenario.doneLabel) != null ? _l : "\u0413\u043E\u0442\u043E\u0432\u043E",
        isLast: index === a.scenario.steps.length - 1,
        isFirst: index === 0,
        skippable: (_m = a.scenario.skippable) != null ? _m : true,
        onPrimary: () => this.next(),
        onBack: () => this.prev(),
        onSkip: () => this.skip()
      };
      const tooltipEl = this.hooks.renderTooltip ? (_n = this.hooks.renderTooltip(ctx)) != null ? _n : renderDefaultTooltip(ctx) : renderDefaultTooltip(ctx);
      tooltipEl.style.zIndex = String(this.zIndex + 2);
      a.tooltipEl = tooltipEl;
      this.container.appendChild(tooltipEl);
      this.placeTooltip(tooltipEl, ctx.side, rect);
      if (step.perform && step.perform.length > 0) {
        const actx = {
          signal: (n, p) => this.signal(n, p),
          log: this.log,
          // Provide a "subscribe" hook so waitFor(signal) can hook the bus.
          // Cast-escape because ActionContext's public shape doesn't expose
          // it; the action runner reads it via duck-typing.
          ...this,
          subscribe: (n, cb) => this.subscribeForWait(n, cb)
        };
        performActions(step.perform, actx).catch((e) => {
          if (e instanceof WaitForError) {
            this.log("warn", `perform WaitForError: ${e.message}`);
          } else {
            this.log("error", `perform crashed: ${e.message}`);
          }
        });
      }
      if (step.waitFor) {
        const wait = awaitWaitFor(step.waitFor, {
          signal: (n, p) => this.signal(n, p),
          log: this.log,
          // The action runner looks up `subscribe` via duck-typing.
          subscribe: (n, cb) => this.subscribeForWait(n, cb)
        });
        a.wait = {
          resolved: false,
          cancel: wait.cancel
        };
        wait.promise.then(() => {
          if (!a.wait) return;
          a.wait.resolved = true;
          if (a.scenario.autoAdvance || step.passive) this.next();
        }).catch((e) => {
          if (e instanceof WaitForError) {
            this.log("warn", `waitFor soft-failed: ${e.message} \u2014 Next still clickable`);
          } else {
            this.log("error", `waitFor crashed: ${e.message}`);
          }
        });
      }
      if (step.advanceOnClick) {
        const target = typeof step.advanceOnClick === "string" ? document.querySelector(step.advanceOnClick) : step.advanceOnClick;
        if (target) {
          const handler = () => this.next();
          target.addEventListener("click", handler, { once: true });
          a.advanceUnsub = () => target.removeEventListener("click", handler);
        } else {
          this.log("warn", `advanceOnClick: target not found`);
        }
      }
    }
    subscribeForWait(name, cb) {
      const off = this.onSignal((sigName) => {
        if (sigName === name) cb();
      });
      return off;
    }
    placeTooltip(el, side, rect) {
      var _a, _b;
      const size = measureTooltip(el);
      const placement = computePlacement(side, rect, size);
      el.style.position = "fixed";
      el.style.left = `${placement.x}px`;
      el.style.top = `${placement.y}px`;
      el.style.width = `${placement.width}px`;
      el.dataset.caretDir = (_b = (_a = placement.caret) == null ? void 0 : _a.dir) != null ? _b : "none";
      if (placement.caret) {
        const caret = el.querySelector(".cd-tour-caret");
        if (caret) {
          const localX = placement.caret.x - placement.x;
          const localY = placement.caret.y - placement.y;
          caret.style.left = `${localX}px`;
          caret.style.top = `${localY}px`;
        }
      }
    }
    startRefreshRaf() {
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
    cleanup() {
      var _a, _b;
      if (!this.active) return;
      const a = this.active;
      if (a.tooltipEl) a.tooltipEl.remove();
      if (a.wait) (_b = (_a = a.wait).cancel) == null ? void 0 : _b.call(_a);
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
    // ── storage helpers (graceful degradation when no localStorage) ──
    readStorage(key) {
      var _a, _b;
      if (!this.storageKey) return null;
      try {
        return (_b = (_a = globalThis.localStorage) == null ? void 0 : _a.getItem(key)) != null ? _b : null;
      } catch (e) {
        return null;
      }
    }
    writeStorage(key, value) {
      var _a;
      if (!this.storageKey) return;
      try {
        (_a = globalThis.localStorage) == null ? void 0 : _a.setItem(key, value);
      } catch (e) {
        this.log("warn", `localStorage write failed for key ${key}`);
      }
    }
    removeStorage(key) {
      var _a;
      if (!this.storageKey) return;
      try {
        (_a = globalThis.localStorage) == null ? void 0 : _a.removeItem(key);
      } catch (e) {
      }
    }
  };
  // Singleton bus for cross-engine use (rare; the engine is normally
  // a single instance per page).
  _Tour.instance = null;
  var Tour = _Tour;
  function resolveAnchor(spec, hooks) {
    if (!spec) return null;
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
        const el = document.querySelector(spec.selector);
        if (el) return getElementRect(el);
        if (spec.fallback) return spec.fallback;
        return null;
      }
    }
  }
  function resolveAnchorRect(spec) {
    var _a;
    switch (spec.kind) {
      case "rect":
        return spec.rect;
      case "element":
        return getElementRect(spec.element);
      case "selector": {
        const el = document.querySelector(spec.selector);
        return el ? getElementRect(el) : (_a = spec.fallback) != null ? _a : null;
      }
    }
  }
  var stylesInjected = false;
  function injectStyles(container, css) {
    if (stylesInjected) return;
    const style = document.createElement("style");
    style.dataset.cdTourStyles = "1";
    style.textContent = css;
    const root = container.getRootNode();
    (root === document ? document.head : root).appendChild(style);
    stylesInjected = true;
  }

  // src/scenarios/index.ts
  var scenarios_exports = {};
  __export(scenarios_exports, {
    calculationsTourScenario: () => calculationsTourScenario,
    en: () => en_exports,
    firstRunInlineScenario: () => firstRunInlineScenario,
    paletteTourScenario: () => paletteTourScenario,
    schemeGalleryTourScenario: () => schemeGalleryTourScenario
  });

  // src/scenarios/first-run-inline.ts
  var firstRunInlineScenario = {
    id: "cd-first-run-inline",
    name: "\u041F\u0435\u0440\u0432\u044B\u0439 \u0437\u0430\u043F\u0443\u0441\u043A: \u0438\u043D\u0442\u0435\u0440\u0430\u043A\u0442\u0438\u0432\u043D\u044B\u0439 \u0442\u0443\u0440",
    primaryLabel: "\u0414\u0430\u043B\u0435\u0435",
    skipLabel: "\u041F\u0440\u043E\u043F\u0443\u0441\u0442\u0438\u0442\u044C",
    backLabel: "\u041D\u0430\u0437\u0430\u0434",
    doneLabel: "\u0413\u043E\u0442\u043E\u0432\u043E",
    skippable: true,
    autoAdvance: false,
    steps: [
      {
        id: "welcome",
        title: "\u042D\u0442\u043E \u0438\u043D\u0442\u0435\u0440\u0430\u043A\u0442\u0438\u0432\u043D\u044B\u0439 \u0442\u0443\u0440",
        body: "\u041A\u0430\u0440\u0442\u043E\u0447\u043A\u0438-\u043A\u0430\u0440\u0443\u0441\u0435\u043B\u044C \u0443\u0436\u0435 \u043F\u043E\u043A\u0430\u0437\u0430\u043B\u0438 \u043E\u0441\u043D\u043E\u0432\u044B. \u0422\u0435\u043F\u0435\u0440\u044C \u043F\u043E\u043F\u0440\u043E\u0431\u0443\u0435\u043C \u043F\u043E-\u043D\u0430\u0441\u0442\u043E\u044F\u0449\u0435\u043C\u0443 \u2014 \u043A\u0430\u0436\u0434\u044B\u0439 \u0448\u0430\u0433 \u0441\u043E\u043F\u0440\u043E\u0432\u043E\u0436\u0434\u0430\u0435\u0442\u0441\u044F \u0434\u0435\u0439\u0441\u0442\u0432\u0438\u0435\u043C, \u0438 \u043A\u043D\u043E\u043F\u043A\u0430 \xAB\u0414\u0430\u043B\u0435\u0435\xBB \u0430\u043A\u0442\u0438\u0432\u0438\u0440\u0443\u0435\u0442\u0441\u044F \u0442\u043E\u043B\u044C\u043A\u043E \u043A\u043E\u0433\u0434\u0430 \u0432\u044B \u0435\u0433\u043E \u0432\u044B\u043F\u043E\u043B\u043D\u0438\u0442\u0435. \u0412\u044B\u0445\u043E\u0434 \u2014 Esc \u2014 \u0432\u0441\u0435\u0433\u0434\u0430 \u0432\u0438\u0434\u0435\u043D.",
        side: "center"
      },
      {
        id: "locate-toolbar",
        title: "\u041A\u0430\u043D\u0432\u0430\u0441\u044B \u0438 \u0445\u0440\u0430\u043D\u0438\u043B\u0438\u0449\u0435",
        body: "\u0412 \u043F\u0440\u0430\u0432\u043E\u043C \u0432\u0435\u0440\u0445\u043D\u0435\u043C \u0443\u0433\u043B\u0443, \u0441\u043B\u0435\u0432\u0430 \u043E\u0442 \u043A\u043D\u043E\u043F\u043A\u0438 \xAB?\xBB, \u2014 \u0447\u0438\u043F \u0430\u043A\u0442\u0438\u0432\u043D\u043E\u0433\u043E \u043A\u0430\u043D\u0432\u0430\u0441\u0430: \u043A\u043B\u0438\u043A \u043F\u043E \u0438\u043C\u0435\u043D\u0438 \u043F\u0435\u0440\u0435\u0438\u043C\u0435\u043D\u043E\u0432\u044B\u0432\u0430\u0435\u0442, \u043F\u043E \u0438\u043A\u043E\u043D\u043A\u0435 \u0441\u043F\u0438\u0441\u043A\u0430 \u2014 \u043E\u0442\u043A\u0440\u044B\u0432\u0430\u0435\u0442 \u043C\u0435\u043D\u0435\u0434\u0436\u0435\u0440 \u043A\u0430\u043D\u0432\u0430\u0441\u043E\u0432 (\u0441\u043E\u0437\u0434\u0430\u043D\u0438\u0435, \u0448\u0430\u0431\u043B\u043E\u043D\u044B, \u0438\u043C\u043F\u043E\u0440\u0442, \u044D\u043A\u0441\u043F\u043E\u0440\u0442). \u041E\u0442\u043A\u0440\u044B\u0442\u044C \u0441 \u0434\u0438\u0441\u043A\u0430 \u0438 \u044D\u043A\u0441\u043F\u043E\u0440\u0442 HTML \u2014 \u0432 \u043C\u0435\u043D\u044E \u043F\u0440\u0430\u0432\u043E\u0433\u043E \u043A\u043B\u0438\u043A\u0430 \u043F\u043E \u043A\u0430\u043D\u0432\u0430\u0441\u0443.",
        side: "bottom",
        anchor: { kind: "selector", selector: "#gpu-canvas-chip" }
      },
      {
        id: "create-note",
        title: "\u0421\u043E\u0437\u0434\u0430\u0439\u0442\u0435 \u043F\u0435\u0440\u0432\u0443\u044E \u0437\u0430\u043C\u0435\u0442\u043A\u0443",
        body: "\u0421\u0434\u0435\u043B\u0430\u0439\u0442\u0435 \u0434\u0432\u043E\u0439\u043D\u043E\u0439 \u043A\u043B\u0438\u043A \u043F\u043E \u043F\u0443\u0441\u0442\u043E\u043C\u0443 \u043C\u0435\u0441\u0442\u0443 \u043D\u0430 \u043A\u0430\u043D\u0432\u0430\u0441\u0435. \u041F\u043E\u044F\u0432\u0438\u0442\u0441\u044F \u043D\u043E\u0432\u0430\u044F \u0437\u0430\u043C\u0435\u0442\u043A\u0430 \u0441 \u043A\u0443\u0440\u0441\u043E\u0440\u043E\u043C \u0432\u043D\u0443\u0442\u0440\u0438 \u2014 \u043C\u043E\u0436\u043D\u043E \u0441\u0440\u0430\u0437\u0443 \u043F\u0438\u0441\u0430\u0442\u044C \u0442\u0435\u043A\u0441\u0442, markdown-\u0440\u0430\u0437\u043C\u0435\u0442\u043A\u0443 \u0438\u043B\u0438 \u0444\u043E\u0440\u043C\u0443\u043B\u0443 Numi. \xAB\u0414\u0430\u043B\u0435\u0435\xBB \u0430\u043A\u0442\u0438\u0432\u0438\u0440\u0443\u0435\u0442\u0441\u044F, \u043A\u043E\u0433\u0434\u0430 \u0437\u0430\u043C\u0435\u0442\u043A\u0430 \u0441\u043E\u0437\u0434\u0430\u043D\u0430.",
        side: "center",
        passive: true,
        primaryLabel: "\u0416\u0434\u0443 \u0434\u0435\u0439\u0441\u0442\u0432\u0438\u044F\u2026",
        waitFor: {
          kind: "signal",
          signals: ["canvas:note-created", "canvas:note-activated"],
          timeout: 12e4
        }
      },
      {
        id: "write-formula",
        title: "\u041F\u043E\u043F\u0440\u043E\u0431\u0443\u0439\u0442\u0435 \u0444\u043E\u0440\u043C\u0443\u043B\u0443 Numi",
        body: "\u0412 \u0437\u0430\u043C\u0435\u0442\u043A\u0435 \u043D\u0430\u043F\u0438\u0448\u0438\u0442\u0435: `rps = 1200` \u0438 \u043D\u0430 \u0441\u043B\u0435\u0434\u0443\u044E\u0449\u0435\u0439 \u0441\u0442\u0440\u043E\u043A\u0435 `daily = rps / 86400`. \u0417\u0430\u043C\u0435\u0442\u043A\u0430 \u043F\u043E\u0434\u0441\u0432\u0435\u0442\u0438\u0442 \u0440\u0435\u0437\u0443\u043B\u044C\u0442\u0430\u0442 \u0432\u044B\u0447\u0438\u0441\u043B\u0435\u043D\u0438\u044F. Numi \u043F\u043E\u043D\u0438\u043C\u0430\u0435\u0442 \u0435\u0434\u0438\u043D\u0438\u0446\u044B (rps, ms, MB/s) \u0438 \u043F\u043E\u0434\u0441\u043A\u0430\u0437\u043A\u0438 \u043F\u0440\u0438 \u0432\u0432\u043E\u0434\u0435. \u0417\u0430\u0432\u0435\u0440\u0448\u0438\u0442\u0435 \u0440\u0435\u0434\u0430\u043A\u0442\u0438\u0440\u043E\u0432\u0430\u043D\u0438\u0435 (Esc \u0438\u043B\u0438 \u043A\u043B\u0438\u043A \u043C\u0438\u043C\u043E) \u2014 \xAB\u0414\u0430\u043B\u0435\u0435\xBB \u0430\u043A\u0442\u0438\u0432\u0438\u0440\u0443\u0435\u0442\u0441\u044F, \u043A\u043E\u0433\u0434\u0430 \u0444\u043E\u0440\u043C\u0443\u043B\u0430 \u0432\u044B\u0447\u0438\u0441\u043B\u0435\u043D\u0430.",
        side: "top",
        anchor: { kind: "rect", rect: { x: 80, y: 80, width: 320, height: 80 } },
        passive: true,
        primaryLabel: "\u0416\u0434\u0443 \u0444\u043E\u0440\u043C\u0443\u043B\u0443\u2026",
        waitFor: {
          kind: "signal",
          signals: ["canvas:formula-written"],
          timeout: 18e4
        }
      },
      {
        id: "connect-nodes",
        title: "\u0421\u0432\u044F\u0437\u0438 \u0438 \u043F\u043E\u0442\u043E\u043A \u0437\u043D\u0430\u0447\u0435\u043D\u0438\u0439",
        body: "\u0421\u043E\u0437\u0434\u0430\u0439\u0442\u0435 \u0432\u0442\u043E\u0440\u0443\u044E \u0437\u0430\u043C\u0435\u0442\u043A\u0443. \u041F\u0435\u0440\u0435\u0442\u0430\u0449\u0438\u0442\u0435 \u043E\u0442 \u043A\u0440\u0430\u044F \u043F\u0435\u0440\u0432\u043E\u0439 \u043A\u043E \u0432\u0442\u043E\u0440\u043E\u0439 \u2014 \u043F\u043E\u044F\u0432\u0438\u0442\u0441\u044F \u0441\u0432\u044F\u0437\u044C. \u0412 \u043F\u0435\u0440\u0432\u043E\u0439 \u043D\u0430\u043F\u0438\u0448\u0438\u0442\u0435 `$in = 10`, \u0432\u043E \u0432\u0442\u043E\u0440\u043E\u0439 \u2014 `$in * 2`. \u0421\u0432\u044F\u0437\u044C \u043F\u0435\u0440\u0435\u0434\u0430\u0441\u0442 \u0437\u043D\u0430\u0447\u0435\u043D\u0438\u0435 \u0438 \u0432\u0442\u043E\u0440\u0430\u044F \u0437\u0430\u043C\u0435\u0442\u043A\u0430 \u043F\u043E\u043A\u0430\u0436\u0435\u0442 20.",
        side: "top",
        anchor: { kind: "rect", rect: { x: 80, y: 80, width: 320, height: 80 } },
        primaryLabel: "\u041F\u043E\u043D\u044F\u0442\u043D\u043E"
      },
      {
        id: "palette",
        title: "\u041F\u0430\u043B\u0438\u0442\u0440\u0430 \u0448\u0430\u0431\u043B\u043E\u043D\u043E\u0432",
        body: "Ctrl+P \u043E\u0442\u043A\u0440\u044B\u0432\u0430\u0435\u0442 \u043F\u0430\u043B\u0438\u0442\u0440\u0443 \u0433\u043E\u0442\u043E\u0432\u044B\u0445 \u043D\u043E\u0434: \u043C\u0435\u0442\u0440\u0438\u043A\u0438 unit-economics (ARPU, LTV, CAC), \u0438\u043D\u0444\u0440\u0430\u0441\u0442\u0440\u0443\u043A\u0442\u0443\u0440\u0430 (DB, API-gateway, LB), \u043F\u043E\u0442\u043E\u043A\u0438 (funnel-conv, retention-d7). Shift+\u043A\u043B\u0438\u043A \u043F\u043E \u043A\u043E\u043B\u0435\u0441\u0443 \u0432\u0441\u0442\u0430\u0432\u043B\u044F\u0435\u0442 \u0432\u044B\u0431\u0440\u0430\u043D\u043D\u044B\u0439 \u0448\u0430\u0431\u043B\u043E\u043D.",
        side: "center"
      },
      {
        id: "help",
        title: "\u041A\u043D\u043E\u043F\u043A\u0430 \xAB?\xBB \u0438 F1",
        body: "\xAB?\xBB \u2014 \u043C\u0435\u043D\u044E \u043F\u043E\u043C\u043E\u0449\u0438: \u0434\u043E\u043A\u0443\u043C\u0435\u043D\u0442\u0430\u0446\u0438\u044F, \u043F\u043E\u0432\u0442\u043E\u0440 \u043E\u043D\u0431\u043E\u0440\u0434\u0438\u043D\u0433\u0430 (\u043A\u0430\u0440\u0442\u043E\u0447\u043A\u0438 \u0438 \u044D\u0442\u043E\u0442 \u0438\u043D\u0442\u0435\u0440\u0430\u043A\u0442\u0438\u0432\u043D\u044B\u0439 \u0442\u0443\u0440), UI-\u043A\u043E\u043D\u0441\u043E\u043B\u044C. F1 \u2014 \u0441\u043F\u0438\u0441\u043E\u043A \u0433\u043E\u0440\u044F\u0447\u0438\u0445 \u043A\u043B\u0430\u0432\u0438\u0448. \u0414\u043E\u043A\u0443\u043C\u0435\u043D\u0442\u0430\u0446\u0438\u044F \u043F\u043E \u0432\u0441\u0435\u043C \u0444\u0443\u043D\u043A\u0446\u0438\u044F\u043C \u2014 \u0432 user-docs/.",
        side: "center"
      },
      {
        id: "done",
        title: "\u0413\u043E\u0442\u043E\u0432\u043E!",
        body: "\u0411\u0430\u0437\u043E\u0432\u044B\u0439 \u043D\u0430\u0431\u043E\u0440 \u043E\u0441\u0432\u043E\u0435\u043D. \u041E\u0441\u0442\u0430\u043B\u044C\u043D\u043E\u0435 \u2014 \u043F\u043E \u043C\u0435\u0440\u0435 \u043D\u0435\u043E\u0431\u0445\u043E\u0434\u0438\u043C\u043E\u0441\u0442\u0438 \u0438\u0437 \u0434\u043E\u043A\u0443\u043C\u0435\u043D\u0442\u0430\u0446\u0438\u0438. \u041A\u0430\u043D\u0432\u0430\u0441 \u0441\u043E\u0445\u0440\u0430\u043D\u044F\u0435\u0442\u0441\u044F \u0430\u0432\u0442\u043E\u043C\u0430\u0442\u0438\u0447\u0435\u0441\u043A\u0438.",
        side: "center"
      }
    ]
  };

  // src/scenarios/palette-tour.ts
  var paletteTourScenario = {
    id: "cd-palette-tour",
    name: "\u041F\u0430\u043B\u0438\u0442\u0440\u0430 \u0448\u0430\u0431\u043B\u043E\u043D\u043E\u0432",
    primaryLabel: "\u0414\u0430\u043B\u0435\u0435",
    skipLabel: "\u041F\u0440\u043E\u043F\u0443\u0441\u0442\u0438\u0442\u044C",
    backLabel: "\u041D\u0430\u0437\u0430\u0434",
    doneLabel: "\u0413\u043E\u0442\u043E\u0432\u043E",
    skippable: true,
    steps: [
      {
        id: "intro",
        title: "\u041F\u0430\u043B\u0438\u0442\u0440\u0430 \u0448\u0430\u0431\u043B\u043E\u043D\u043E\u0432",
        body: "\u0412 CanvasDesk \u0435\u0441\u0442\u044C \u0432\u0441\u0442\u0440\u043E\u0435\u043D\u043D\u044B\u0435 \u0448\u0430\u0431\u043B\u043E\u043D\u044B \u043D\u043E\u0434: \u043C\u0435\u0442\u0440\u0438\u043A\u0438, \u0438\u043D\u0444\u0440\u0430\u0441\u0442\u0440\u0443\u043A\u0442\u0443\u0440\u0430, \u043F\u043E\u0442\u043E\u043A\u0438. \u0418\u0445 \u043C\u043E\u0436\u043D\u043E \u043F\u0435\u0440\u0435\u0442\u0430\u0441\u043A\u0438\u0432\u0430\u0442\u044C \u043D\u0430 \u043A\u0430\u043D\u0432\u0430\u0441 \u043A\u0430\u043A \u0433\u043E\u0442\u043E\u0432\u044B\u0435 \u0431\u043B\u043E\u043A\u0438.",
        side: "center"
      },
      {
        id: "open",
        title: "\u041E\u0442\u043A\u0440\u043E\u0439\u0442\u0435 \u043F\u0430\u043B\u0438\u0442\u0440\u0443",
        body: "\u041D\u0430\u0436\u043C\u0438\u0442\u0435 Ctrl+P (\u0438\u043B\u0438 \u2318+P \u043D\u0430 Mac). \u0421\u043B\u0435\u0432\u0430 \u043F\u043E\u044F\u0432\u0438\u0442\u0441\u044F \u043A\u043E\u043B\u0435\u0441\u043E-\u043D\u0430\u0432\u0438\u0433\u0430\u0442\u043E\u0440 \u0441 \u043A\u0430\u0442\u0435\u0433\u043E\u0440\u0438\u044F\u043C\u0438.",
        side: "center",
        passive: true,
        primaryLabel: "\u0416\u0434\u0443 \u043E\u0442\u043A\u0440\u044B\u0442\u0438\u044F \u043F\u0430\u043B\u0438\u0442\u0440\u044B\u2026",
        waitFor: {
          kind: "signal",
          signals: ["canvas:palette-opened"],
          timeout: 6e4
        }
      },
      {
        id: "categories",
        title: "\u041A\u0430\u0442\u0435\u0433\u043E\u0440\u0438\u0438",
        body: "\u041A\u043E\u043B\u0435\u0441\u043E \u043F\u0435\u0440\u0435\u043A\u043B\u044E\u0447\u0430\u0435\u0442 \u043A\u0430\u0442\u0435\u0433\u043E\u0440\u0438\u0438: Unit Economics, Product Analytics, Infrastructure, Patterns. Shift+\u043A\u043B\u0438\u043A \u2014 \u0432\u044B\u0431\u043E\u0440 \u043A\u0430\u0442\u0435\u0433\u043E\u0440\u0438\u0438.",
        side: "right",
        anchor: { kind: "rect", rect: { x: 0, y: 0, width: 240, height: 800 } }
      },
      {
        id: "shift-click",
        title: "Shift+\u043A\u043B\u0438\u043A \u2014 \u0432\u0441\u0442\u0430\u0432\u0438\u0442\u044C \u0448\u0430\u0431\u043B\u043E\u043D",
        body: "Shift+\u043A\u043B\u0438\u043A \u043F\u043E \u0448\u0430\u0431\u043B\u043E\u043D\u0443 \u0432\u0441\u0442\u0430\u0432\u043B\u044F\u0435\u0442 \u0435\u0433\u043E \u0432 \u0446\u0435\u043D\u0442\u0440 \u043A\u0430\u043D\u0432\u0430\u0441\u0430 \u0441\u043E \u0441\u0442\u0430\u043D\u0434\u0430\u0440\u0442\u043D\u044B\u043C\u0438 \u043F\u0430\u0440\u0430\u043C\u0435\u0442\u0440\u0430\u043C\u0438. \u041F\u043E\u0441\u043B\u0435 \u0432\u0441\u0442\u0430\u0432\u043A\u0438 \u043F\u0430\u0440\u0430\u043C\u0435\u0442\u0440\u044B \u0440\u0435\u0434\u0430\u043A\u0442\u0438\u0440\u0443\u044E\u0442\u0441\u044F \u0447\u0435\u0440\u0435\u0437 \u041F\u041A\u041C.",
        side: "right",
        anchor: { kind: "rect", rect: { x: 0, y: 0, width: 240, height: 800 } }
      },
      {
        id: "done",
        title: "\u0413\u043E\u0442\u043E\u0432\u043E",
        body: "\u0428\u0430\u0431\u043B\u043E\u043D\u044B \u2014 \u044D\u0442\u043E \u0441\u0442\u0440\u043E\u0438\u0442\u0435\u043B\u044C\u043D\u044B\u0435 \u0431\u043B\u043E\u043A\u0438 \u0434\u043B\u044F \u043A\u0430\u043D\u0432\u0430\u0441-\u0441\u0445\u0435\u043C. \u0411\u043E\u043B\u044C\u0448\u0438\u043D\u0441\u0442\u0432\u043E \u043C\u043E\u0434\u0435\u043B\u0435\u0439 \u0441\u043E\u0431\u0438\u0440\u0430\u0435\u0442\u0441\u044F \u0437\u0430 5\u201310 \u043C\u0438\u043D\u0443\u0442.",
        side: "center"
      }
    ]
  };

  // src/scenarios/calculations-tour.ts
  var calculationsTourScenario = {
    id: "cd-calculations-tour",
    name: "\u0420\u0430\u0441\u0447\u0451\u0442\u044B \u0432 CanvasDesk",
    primaryLabel: "\u0414\u0430\u043B\u0435\u0435",
    skipLabel: "\u041F\u0440\u043E\u043F\u0443\u0441\u0442\u0438\u0442\u044C",
    backLabel: "\u041D\u0430\u0437\u0430\u0434",
    doneLabel: "\u0413\u043E\u0442\u043E\u0432\u043E",
    skippable: true,
    steps: [
      {
        id: "intro",
        title: "\u0424\u043E\u0440\u043C\u0443\u043B\u044B \u043F\u0440\u044F\u043C\u043E \u0432 \u0437\u0430\u043C\u0435\u0442\u043A\u0430\u0445",
        body: "\u041B\u044E\u0431\u0430\u044F \u0437\u0430\u043C\u0435\u0442\u043A\u0430 \u2014 \u044D\u0442\u043E Numi-\u0444\u043E\u0440\u043C\u0443\u043B\u0430: \u043C\u043E\u0436\u043D\u043E \u043E\u0431\u044A\u044F\u0432\u0438\u0442\u044C \u043F\u0435\u0440\u0435\u043C\u0435\u043D\u043D\u0443\u044E, \u0438\u0441\u043F\u043E\u043B\u044C\u0437\u043E\u0432\u0430\u0442\u044C \u0435\u0434\u0438\u043D\u0438\u0446\u044B (rps, ms, MB/s), \u043F\u043E\u043B\u0443\u0447\u0438\u0442\u044C \u0440\u0435\u0437\u0443\u043B\u044C\u0442\u0430\u0442. \u0421\u0432\u044F\u0437\u0438 \u043C\u0435\u0436\u0434\u0443 \u0437\u0430\u043C\u0435\u0442\u043A\u0430\u043C\u0438 \u043F\u0435\u0440\u0435\u0434\u0430\u044E\u0442 \u0437\u043D\u0430\u0447\u0435\u043D\u0438\u044F \u2014 \u044D\u0442\u043E \u0438 \u0435\u0441\u0442\u044C \u043F\u043E\u0442\u043E\u043A.",
        side: "center"
      },
      {
        id: "assignment",
        title: "\u041E\u0431\u044A\u044F\u0432\u043B\u0435\u043D\u0438\u0435 \u043F\u0435\u0440\u0435\u043C\u0435\u043D\u043D\u043E\u0439",
        body: "\u0424\u043E\u0440\u043C\u0430\u0442: `name = expression`. \u041F\u0440\u0438\u043C\u0435\u0440: `rps = 1200`. \u0417\u0430\u043C\u0435\u0442\u043A\u0430 \u043F\u043E\u0434\u0441\u0432\u0435\u0442\u0438\u0442 \u0438\u043C\u044F \u0438 \u0437\u043D\u0430\u0447\u0435\u043D\u0438\u0435, \u043A\u0443\u0440\u0441\u043E\u0440 \u0441\u0440\u0430\u0437\u0443 \u0432 \u0440\u0435\u0434\u0430\u043A\u0442\u0438\u0440\u043E\u0432\u0430\u043D\u0438\u0438 \u043F\u0440\u0430\u0432\u043E\u0439 \u0447\u0430\u0441\u0442\u0438.",
        side: "top",
        anchor: { kind: "rect", rect: { x: 80, y: 120, width: 320, height: 60 } }
      },
      {
        id: "units",
        title: "\u0415\u0434\u0438\u043D\u0438\u0446\u044B \u0438\u0437\u043C\u0435\u0440\u0435\u043D\u0438\u044F",
        body: "Numi \u043F\u043E\u043D\u0438\u043C\u0430\u0435\u0442 \u0435\u0434\u0438\u043D\u0438\u0446\u044B: `rps`, `ms`, `MB/s`, `\u20AC`, `users`. \u041E\u043D\u0438 \u043F\u0440\u043E\u0432\u0435\u0440\u044F\u044E\u0442\u0441\u044F \u043D\u0430 \u0441\u043E\u0432\u043C\u0435\u0441\u0442\u0438\u043C\u043E\u0441\u0442\u044C \u2014 \u043D\u0435\u043B\u044C\u0437\u044F \u0441\u043B\u043E\u0436\u0438\u0442\u044C rps \u0438 ms. \u041F\u043E\u0434\u0441\u043A\u0430\u0437\u043A\u0438 \u043F\u043E\u044F\u0432\u043B\u044F\u044E\u0442\u0441\u044F \u043F\u0440\u0438 \u0432\u0432\u043E\u0434\u0435.",
        side: "top",
        anchor: { kind: "rect", rect: { x: 80, y: 180, width: 320, height: 60 } }
      },
      {
        id: "value-flow",
        title: "\u041F\u043E\u0442\u043E\u043A \u0437\u043D\u0430\u0447\u0435\u043D\u0438\u0439 \u043F\u043E \u0441\u0432\u044F\u0437\u044F\u043C",
        body: "\u0421\u0432\u044F\u0437\u044C \u043C\u0435\u0436\u0434\u0443 \u0437\u0430\u043C\u0435\u0442\u043A\u0430\u043C\u0438 \u043F\u0435\u0440\u0435\u0434\u0430\u0451\u0442 \u0437\u043D\u0430\u0447\u0435\u043D\u0438\u0435. \u0412 \u043F\u043E\u0434\u0447\u0438\u043D\u0451\u043D\u043D\u043E\u0439 \u0437\u0430\u043C\u0435\u0442\u043A\u0435 `$in` \u2014 \u0432\u0445\u043E\u0434\u044F\u0449\u0435\u0435 \u0437\u043D\u0430\u0447\u0435\u043D\u0438\u0435, `$N` \u2014 \u043C\u0430\u0441\u0441\u0438\u0432 \u0432\u0441\u0435\u0445 \u0432\u0445\u043E\u0434\u044F\u0449\u0438\u0445. \u041F\u0435\u0440\u0435\u0441\u0447\u0451\u0442 \u2014 \u043C\u0433\u043D\u043E\u0432\u0435\u043D\u043D\u044B\u0439, \u043F\u0440\u0438 \u0438\u0437\u043C\u0435\u043D\u0435\u043D\u0438\u0438 \u043B\u044E\u0431\u043E\u0439 \u0437\u0430\u043C\u0435\u0442\u043A\u0438.",
        side: "top",
        anchor: { kind: "rect", rect: { x: 80, y: 240, width: 320, height: 60 } }
      },
      {
        id: "whatif",
        title: "What-if \u0441\u0446\u0435\u043D\u0430\u0440\u0438\u0438",
        body: "\u041E\u0442\u043A\u0440\u043E\u0439\u0442\u0435 What-if \u043F\u0430\u043D\u0435\u043B\u044C (FR-017) \u0438 \u0437\u0430\u0434\u0430\u0439\u0442\u0435 \u0434\u0438\u0430\u043F\u0430\u0437\u043E\u043D\u044B \u0437\u043D\u0430\u0447\u0435\u043D\u0438\u0439 \u2014 CanvasDesk \u043F\u043E\u0441\u0447\u0438\u0442\u0430\u0435\u0442 \u0447\u0443\u0432\u0441\u0442\u0432\u0438\u0442\u0435\u043B\u044C\u043D\u043E\u0441\u0442\u044C \u0440\u0435\u0437\u0443\u043B\u044C\u0442\u0430\u0442\u0430. \u041F\u043E\u043B\u0435\u0437\u043D\u043E \u0434\u043B\u044F unit-economics: \xAB\u0447\u0442\u043E \u0435\u0441\u043B\u0438 \u043A\u043E\u043D\u0432\u0435\u0440\u0441\u0438\u044F \u0443\u043F\u0430\u0434\u0451\u0442 \u043D\u0430 10%\xBB.",
        side: "center"
      },
      {
        id: "monte-carlo",
        title: "\u041C\u043E\u043D\u0442\u0435-\u041A\u0430\u0440\u043B\u043E (FR-066)",
        body: "\u0414\u043B\u044F \u0432\u0435\u0440\u043E\u044F\u0442\u043D\u043E\u0441\u0442\u043D\u044B\u0445 \u043C\u043E\u0434\u0435\u043B\u0435\u0439 \u2014 \u0437\u0430\u0434\u0430\u0439\u0442\u0435 \u0440\u0430\u0441\u043F\u0440\u0435\u0434\u0435\u043B\u0435\u043D\u0438\u044F \u043F\u0430\u0440\u0430\u043C\u0435\u0442\u0440\u043E\u0432 \u0438 CanvasDesk \u043F\u0440\u043E\u0433\u043E\u043D\u0438\u0442 \u0441\u0438\u043C\u0443\u043B\u044F\u0446\u0438\u044E. \u0420\u0435\u0437\u0443\u043B\u044C\u0442\u0430\u0442\u044B \u043E\u0442\u043E\u0431\u0440\u0430\u0436\u0430\u044E\u0442\u0441\u044F \u043A\u0430\u043A \u0433\u0438\u0441\u0442\u043E\u0433\u0440\u0430\u043C\u043C\u0430 \u0432 \u043F\u0430\u043D\u0435\u043B\u0438 \u0440\u0435\u0437\u0443\u043B\u044C\u0442\u0430\u0442\u0430.",
        side: "center"
      },
      {
        id: "done",
        title: "\u0413\u043E\u0442\u043E\u0432\u043E",
        body: "\u0420\u0430\u0441\u0447\u0451\u0442\u044B \u0432 CanvasDesk \u2014 \u044D\u0442\u043E Numi + \u0433\u0440\u0430\u0444 \u0437\u043D\u0430\u0447\u0435\u043D\u0438\u0439 + \u0441\u0438\u043C\u0443\u043B\u044F\u0446\u0438\u0438. \u041D\u0438\u043A\u0430\u043A\u043E\u0433\u043E Excel.",
        side: "center"
      }
    ]
  };

  // src/scenarios/scheme-gallery.ts
  var schemeGalleryTourScenario = {
    id: "cd-scheme-gallery-tour",
    name: "\u0413\u0430\u043B\u0435\u0440\u0435\u044F \u0441\u0445\u0435\u043C",
    primaryLabel: "\u0414\u0430\u043B\u0435\u0435",
    skipLabel: "\u041F\u0440\u043E\u043F\u0443\u0441\u0442\u0438\u0442\u044C",
    backLabel: "\u041D\u0430\u0437\u0430\u0434",
    doneLabel: "\u0413\u043E\u0442\u043E\u0432\u043E",
    skippable: true,
    steps: [
      {
        id: "intro",
        title: "\u0413\u0430\u043B\u0435\u0440\u0435\u044F \u0441\u0445\u0435\u043C",
        body: "\u0413\u043E\u0442\u043E\u0432\u044B\u0435 \u043C\u043E\u0434\u0435\u043B\u0438: cohort-launch, intro-whatif, investment-case, runway, support-staffing, capacity-service, unit-economics, project-budget, renovation-estimate. \u041A\u0430\u0436\u0434\u0430\u044F \u2014 \u0441\u0442\u0430\u0440\u0442\u043E\u0432\u0430\u044F \u0442\u043E\u0447\u043A\u0430 \u0434\u043B\u044F \u0441\u0432\u043E\u0435\u0439 \u0437\u0430\u0434\u0430\u0447\u0438, \u043D\u0435 \u043D\u0443\u0436\u043D\u043E \u0441\u043E\u0431\u0438\u0440\u0430\u0442\u044C \u0441 \u043D\u0443\u043B\u044F.",
        side: "center"
      },
      {
        id: "open",
        title: "\u041E\u0442\u043A\u0440\u043E\u0439\u0442\u0435 \u0433\u0430\u043B\u0435\u0440\u0435\u044E",
        body: "Ctrl+P (\u0438\u043B\u0438 \u2318+P \u043D\u0430 Mac) \u043E\u0442\u043A\u0440\u044B\u0432\u0430\u0435\u0442 \u0433\u0430\u043B\u0435\u0440\u0435\u044E. \u0410\u043B\u044C\u0442\u0435\u0440\u043D\u0430\u0442\u0438\u0432\u0430 \u2014 \u043A\u043D\u043E\u043F\u043A\u0430 \xAB\u041F\u043E\u043F\u0440\u043E\u0431\u043E\u0432\u0430\u0442\u044C\xBB \u0432 \u0444\u0438\u043D\u0430\u043B\u0435 v1 \u043A\u0430\u0440\u0443\u0441\u0435\u043B\u0438 \u043E\u043D\u0431\u043E\u0440\u0434\u0438\u043D\u0433\u0430. \u0428\u0430\u0433 \u0430\u043A\u0442\u0438\u0432\u0438\u0440\u0443\u0435\u0442\u0441\u044F, \u043A\u043E\u0433\u0434\u0430 \u0433\u0430\u043B\u0435\u0440\u0435\u044F \u0432\u0438\u0434\u043D\u0430.",
        side: "center",
        passive: true,
        primaryLabel: "\u0416\u0434\u0443 \u043E\u0442\u043A\u0440\u044B\u0442\u0438\u044F \u0433\u0430\u043B\u0435\u0440\u0435\u0438\u2026",
        waitFor: {
          kind: "signal",
          signals: ["canvas:scheme-gallery-opened", "canvas:palette-opened"],
          timeout: 6e4
        }
      },
      {
        id: "categories",
        title: "\u041A\u0430\u0442\u0435\u0433\u043E\u0440\u0438\u0438 \u0441\u0445\u0435\u043C",
        body: "\u0421\u043B\u0435\u0432\u0430 \u2014 \u0441\u043F\u0438\u0441\u043E\u043A \u043A\u0430\u0442\u0435\u0433\u043E\u0440\u0438\u0439: Unit Economics (ARPU, LTV, CAC, retention, funnel), Product Analytics (NPS, MAU, stickiness), Infrastructure (DB, API-gateway, LB, queue), Patterns (cohort-launch, intro-whatif, investment-case).",
        side: "right",
        anchor: { kind: "rect", rect: { x: 0, y: 0, width: 240, height: 800 } },
        primaryLabel: "\u041F\u043E\u043D\u044F\u0442\u043D\u043E"
      },
      {
        id: "preview",
        title: "\u041F\u0440\u0435\u0432\u044C\u044E \u0441\u0445\u0435\u043C\u044B",
        body: "\u041A\u043B\u0438\u043A \u043F\u043E \u043A\u0430\u0440\u0442\u043E\u0447\u043A\u0435 \u0441\u0445\u0435\u043C\u044B \u043E\u0442\u043A\u0440\u044B\u0432\u0430\u0435\u0442 \u043F\u0440\u0435\u0432\u044C\u044E: \u043C\u0438\u043D\u0438-\u0441\u0445\u0435\u043C\u0430 \u0441 \u0437\u0430\u043C\u0435\u0442\u043A\u0430\u043C\u0438, \u0444\u043E\u0440\u043C\u0443\u043B\u0430\u043C\u0438, \u0441\u0432\u044F\u0437\u044F\u043C\u0438. \u041C\u043E\u0436\u043D\u043E \u043F\u0440\u043E\u0447\u0438\u0442\u0430\u0442\u044C \u0441\u0442\u0440\u0443\u043A\u0442\u0443\u0440\u0443 \u0434\u043E \u043F\u0440\u0438\u043C\u0435\u043D\u0435\u043D\u0438\u044F. \u0428\u0430\u0433 \u0430\u043A\u0442\u0438\u0432\u0438\u0440\u0443\u0435\u0442\u0441\u044F, \u043A\u043E\u0433\u0434\u0430 \u043F\u0440\u0435\u0432\u044C\u044E \u043E\u0442\u043A\u0440\u044B\u0442\u043E.",
        side: "center",
        passive: true,
        primaryLabel: "\u041E\u0442\u043A\u0440\u043E\u0439\u0442\u0435 \u043F\u0440\u0435\u0432\u044C\u044E\u2026",
        waitFor: {
          kind: "signal",
          signals: ["canvas:scheme-preview-shown"],
          timeout: 12e4
        }
      },
      {
        id: "apply",
        title: "\u041F\u0440\u0438\u043C\u0435\u043D\u0438\u0442\u044C \u0441\u0445\u0435\u043C\u0443",
        body: "\u0412 \u043F\u0440\u0435\u0432\u044C\u044E \u2014 \u043A\u043D\u043E\u043F\u043A\u0430 \xAB\u041F\u0440\u0438\u043C\u0435\u043D\u0438\u0442\u044C\xBB. \u0421\u0445\u0435\u043C\u0430 \u0441\u0442\u0430\u043D\u043E\u0432\u0438\u0442\u0441\u044F \u0430\u043A\u0442\u0438\u0432\u043D\u044B\u043C \u043A\u0430\u043D\u0432\u0430\u0441\u043E\u043C (\u0441 \u0437\u0430\u043C\u0435\u043D\u043E\u0439 \u0442\u0435\u043A\u0443\u0449\u0435\u0439 \u0441\u0446\u0435\u043D\u044B \u2014 undo \u0440\u0430\u0431\u043E\u0442\u0430\u0435\u0442). \u0412\u0441\u0435 \u0444\u043E\u0440\u043C\u0443\u043B\u044B \u0438 \u0441\u0432\u044F\u0437\u0438 \u0441\u043E\u0445\u0440\u0430\u043D\u044F\u044E\u0442\u0441\u044F, \u043C\u043E\u0436\u043D\u043E \u0440\u0435\u0434\u0430\u043A\u0442\u0438\u0440\u043E\u0432\u0430\u0442\u044C \u043F\u043E\u0434 \u0441\u0432\u043E\u044E \u0437\u0430\u0434\u0430\u0447\u0443. \u0428\u0430\u0433 \u0430\u043A\u0442\u0438\u0432\u0438\u0440\u0443\u0435\u0442\u0441\u044F, \u043A\u043E\u0433\u0434\u0430 \u0441\u0445\u0435\u043C\u0430 \u043F\u0440\u0438\u043C\u0435\u043D\u0435\u043D\u0430.",
        side: "center",
        passive: true,
        primaryLabel: "\u0416\u0434\u0443 \u043F\u0440\u0438\u043C\u0435\u043D\u0435\u043D\u0438\u044F \u0441\u0445\u0435\u043C\u044B\u2026",
        waitFor: {
          kind: "signal",
          signals: ["canvas:scheme-applied"],
          timeout: 18e4
        }
      },
      {
        id: "edit",
        title: "\u0420\u0435\u0434\u0430\u043A\u0442\u0438\u0440\u0443\u0439\u0442\u0435 \u043F\u043E\u0434 \u0441\u0435\u0431\u044F",
        body: "\u041F\u043E\u0441\u043B\u0435 \u043F\u0440\u0438\u043C\u0435\u043D\u0435\u043D\u0438\u044F \u2014 \u043A\u0430\u043D\u0430\u043B \u0442\u043E\u0442 \u0436\u0435, \u0447\u0442\u043E \u0443 \u043E\u0431\u044B\u0447\u043D\u043E\u0433\u043E \u043A\u0430\u043D\u0432\u0430\u0441\u0430: \u0434\u0432\u043E\u0439\u043D\u043E\u0439 \u043A\u043B\u0438\u043A \u0441\u043E\u0437\u0434\u0430\u0451\u0442 \u0437\u0430\u043C\u0435\u0442\u043A\u0438, drag \u043E\u0442 \u043A\u0440\u0430\u044F \u2014 \u0441\u0432\u044F\u0437\u0438, \u041F\u041A\u041C \u2014 \u043F\u0430\u043B\u0438\u0442\u0440\u0430 \u0446\u0432\u0435\u0442\u0430 \u0438 \u043F\u0430\u0440\u0430\u043C\u0435\u0442\u0440\u043E\u0432. \u041C\u0435\u043D\u044F\u0442\u044C \u0437\u043D\u0430\u0447\u0435\u043D\u0438\u044F \u043F\u0435\u0440\u0435\u043C\u0435\u043D\u043D\u044B\u0445 \u2014 \u043F\u0440\u044F\u043C\u043E \u0432 \u0437\u0430\u043C\u0435\u0442\u043A\u0430\u0445 \u0441 \u0444\u043E\u0440\u043C\u0443\u043B\u0430\u043C\u0438.",
        side: "top",
        anchor: { kind: "rect", rect: { x: 80, y: 80, width: 400, height: 120 } },
        primaryLabel: "\u041F\u043E\u043D\u044F\u0442\u043D\u043E"
      },
      {
        id: "save",
        title: "\u0421\u043E\u0445\u0440\u0430\u043D\u0435\u043D\u0438\u0435",
        body: "\u041A\u0430\u043D\u0432\u0430\u0441 \u0430\u0432\u0442\u043E\u0441\u043E\u0445\u0440\u0430\u043D\u044F\u0435\u0442\u0441\u044F \u0432 \u0431\u0440\u0430\u0443\u0437\u0435\u0440\u043D\u043E\u0435 \u0445\u0440\u0430\u043D\u0438\u043B\u0438\u0449\u0435 (OPFS) \u0438\u043B\u0438 \u0432 \u0432\u044B\u0431\u0440\u0430\u043D\u043D\u0443\u044E \u043F\u0430\u043F\u043A\u0443 \u043D\u0430 \u0434\u0438\u0441\u043A\u0435 \u043F\u043E\u0441\u043B\u0435 \u043F\u0435\u0440\u0435\u0435\u0437\u0434\u0430 \u0432 \u043C\u0435\u043D\u0435\u0434\u0436\u0435\u0440\u0435. \u042D\u043A\u0441\u043F\u043E\u0440\u0442 .canvas \u0438 HTML \u2014 \u043A\u043D\u043E\u043F\u043A\u0438 \u043C\u0435\u043D\u0435\u0434\u0436\u0435\u0440\u0430 \u0438 \u043C\u0435\u043D\u044E \u043F\u0440\u0430\u0432\u043E\u0433\u043E \u043A\u043B\u0438\u043A\u0430.",
        side: "bottom",
        anchor: { kind: "selector", selector: "#gpu-canvas-chip" }
      },
      {
        id: "done",
        title: "\u0413\u043E\u0442\u043E\u0432\u043E",
        body: "\u0413\u0430\u043B\u0435\u0440\u0435\u044F \u0441\u0445\u0435\u043C \u2014 \u0431\u044B\u0441\u0442\u0440\u044B\u0439 \u0441\u0442\u0430\u0440\u0442 \u0434\u043B\u044F \u0442\u0438\u043F\u043E\u0432\u044B\u0445 \u043C\u043E\u0434\u0435\u043B\u0435\u0439. \u0411\u043E\u043B\u044C\u0448\u0438\u043D\u0441\u0442\u0432\u043E \u043F\u043E\u043B\u044C\u0437\u043E\u0432\u0430\u0442\u0435\u043B\u044C\u0441\u043A\u0438\u0445 \u0441\u0446\u0435\u043D\u0430\u0440\u0438\u0435\u0432 \u043F\u043E\u043A\u0440\u044B\u0432\u0430\u0435\u0442\u0441\u044F \u0433\u043E\u0442\u043E\u0432\u044B\u043C\u0438 \u0441\u0445\u0435\u043C\u0430\u043C\u0438; \u043A\u0430\u0441\u0442\u043E\u043C\u043D\u044B\u0435 \u2014 \u0441\u043E\u0431\u0438\u0440\u0430\u0439\u0442\u0435 \u0438\u0437 \u0448\u0430\u0431\u043B\u043E\u043D\u043E\u0432 \u043F\u0430\u043B\u0438\u0442\u0440\u044B (Ctrl+P).",
        side: "center"
      }
    ]
  };

  // src/scenarios/en/index.ts
  var en_exports = {};
  __export(en_exports, {
    calculationsTourScenario: () => calculationsTourScenario2,
    firstRunInlineScenario: () => firstRunInlineScenario2,
    paletteTourScenario: () => paletteTourScenario2,
    schemeGalleryTourScenario: () => schemeGalleryTourScenario2
  });
  var firstRunInlineScenario2 = {
    id: "cd-first-run-inline",
    name: "First Run: Interactive Tour",
    primaryLabel: "Next",
    skipLabel: "Skip",
    backLabel: "Back",
    doneLabel: "Done",
    skippable: true,
    autoAdvance: false,
    steps: [
      {
        id: "welcome",
        title: "This is an interactive tour",
        body: "The carousel cards explained the basics. Now let's try for real \u2014 each step has an action, and the Next button activates only after you perform it. Exit via Esc is always available.",
        side: "center"
      },
      {
        id: "locate-toolbar",
        title: "Canvases and Storage",
        body: "Top-right corner, to the left of the \xAB?\xBB button, \u2014 the active-canvas chip: click the name to rename, the list icon to open the canvas manager (create, templates, import, export). Open from disk and Export HTML live in the canvas right-click menu.",
        side: "bottom",
        anchor: { kind: "selector", selector: "#gpu-canvas-chip" }
      },
      {
        id: "create-note",
        title: "Create your first note",
        body: "Double-click an empty area on the canvas. A new note appears with the cursor inside \u2014 type text, markdown, or a Numi formula. Next activates when the note is created.",
        side: "center",
        passive: true,
        primaryLabel: "Waiting for action\u2026",
        waitFor: { kind: "signal", signals: ["canvas:note-created", "canvas:note-activated"], timeout: 12e4 }
      },
      {
        id: "write-formula",
        title: "Try a Numi formula",
        body: "In the note, write: rps = 1200, then on the next line daily = rps / 86400. The note shows the result. Numi understands units (rps, ms, MB/s) and shows hints while typing. Finish editing (Esc or click outside) \u2014 Next activates when the formula is computed.",
        side: "top",
        anchor: { kind: "rect", rect: { x: 80, y: 80, width: 320, height: 80 } },
        passive: true,
        primaryLabel: "Waiting for formula\u2026",
        waitFor: { kind: "signal", signals: ["canvas:formula-written"], timeout: 18e4 }
      },
      {
        id: "connect-nodes",
        title: "Edges and value flow",
        body: "Create a second note. Drag from the first note's edge to the second \u2014 an edge appears. In the first note write $in = 10, in the second $in * 2. The edge passes the value and the second note shows 20.",
        side: "top",
        anchor: { kind: "rect", rect: { x: 80, y: 80, width: 320, height: 80 } },
        primaryLabel: "Got it"
      },
      {
        id: "palette",
        title: "Template Palette",
        body: "Ctrl+P opens the palette of built-in nodes: unit-economics metrics (ARPU, LTV, CAC), infrastructure (DB, API-gateway, LB), flows (funnel-conv, retention-d7). Shift+click on the wheel inserts the selected template.",
        side: "center"
      },
      {
        id: "help",
        title: "Help button and F1",
        body: "? \u2014 help menu: docs, repeat onboarding (carousel and this interactive tour), UI console. F1 \u2014 hotkeys list. Documentation for all features is in user-docs/.",
        side: "center"
      },
      {
        id: "done",
        title: "Done!",
        body: "You've learned the basics. Everything else is available in the documentation as needed. Canvas saves automatically.",
        side: "center"
      }
    ]
  };
  var paletteTourScenario2 = {
    id: "cd-palette-tour",
    name: "Template Palette",
    primaryLabel: "Next",
    skipLabel: "Skip",
    backLabel: "Back",
    doneLabel: "Done",
    skippable: true,
    steps: [
      {
        id: "intro",
        title: "Template Palette",
        body: "CanvasDesk has built-in node templates: metrics, infrastructure, flows. You can drag them onto the canvas as ready-made blocks.",
        side: "center"
      },
      {
        id: "open",
        title: "Open the palette",
        body: "Press Ctrl+P (or \u2318+P on Mac). The category wheel appears on the left.",
        side: "center",
        passive: true,
        primaryLabel: "Waiting for palette to open\u2026",
        waitFor: { kind: "signal", signals: ["canvas:palette-opened"], timeout: 6e4 }
      },
      {
        id: "categories",
        title: "Categories",
        body: "The wheel switches categories: Unit Economics, Product Analytics, Infrastructure, Patterns. Shift+click \u2014 category selection.",
        side: "right",
        anchor: { kind: "rect", rect: { x: 0, y: 0, width: 240, height: 800 } }
      },
      {
        id: "shift-click",
        title: "Shift+click \u2014 insert template",
        body: "Shift+click on a template inserts it in the canvas center with default parameters. After insertion, parameters are editable via right-click.",
        side: "right",
        anchor: { kind: "rect", rect: { x: 0, y: 0, width: 240, height: 800 } }
      },
      {
        id: "done",
        title: "Done",
        body: "Templates are building blocks for canvas schemes. Most models can be assembled in 5\u201310 minutes.",
        side: "center"
      }
    ]
  };
  var calculationsTourScenario2 = {
    id: "cd-calculations-tour",
    name: "Calculations in CanvasDesk",
    primaryLabel: "Next",
    skipLabel: "Skip",
    backLabel: "Back",
    doneLabel: "Done",
    skippable: true,
    steps: [
      {
        id: "intro",
        title: "Formulas right in notes",
        body: "Any note is a Numi formula: declare a variable, use units (rps, ms, MB/s), get the result. Edges between notes pass values \u2014 that's the flow.",
        side: "center"
      },
      {
        id: "assignment",
        title: "Variable Declaration",
        body: "Format: name = expression. Example: rps = 1200. The note highlights the name and value; cursor is in the right-hand side editor.",
        side: "top",
        anchor: { kind: "rect", rect: { x: 80, y: 120, width: 320, height: 60 } }
      },
      {
        id: "units",
        title: "Units",
        body: "Numi understands units: rps, ms, MB/s, \u20AC, users. They are checked for compatibility \u2014 you can't add rps and ms. Hints appear while typing.",
        side: "top",
        anchor: { kind: "rect", rect: { x: 80, y: 180, width: 320, height: 60 } }
      },
      {
        id: "value-flow",
        title: "Value Flow via Edges",
        body: "An edge between notes passes a value. In the downstream note, $in \u2014 incoming value, $N \u2014 array of all incoming. Recomputation is instant on any note change.",
        side: "top",
        anchor: { kind: "rect", rect: { x: 80, y: 240, width: 320, height: 60 } }
      },
      {
        id: "whatif",
        title: "What-if Scenarios",
        body: `Open the What-if panel (FR-017) and set value ranges \u2014 CanvasDesk calculates the result's sensitivity. Useful for unit-economics: "what if conversion drops by 10%".`,
        side: "center"
      },
      {
        id: "monte-carlo",
        title: "Monte Carlo (FR-066)",
        body: "For probabilistic models \u2014 set parameter distributions and CanvasDesk runs the simulation. Results display as a histogram in the result panel.",
        side: "center"
      },
      {
        id: "done",
        title: "Done",
        body: "Calculations in CanvasDesk \u2014 Numi + value graph + simulations. No Excel needed.",
        side: "center"
      }
    ]
  };
  var schemeGalleryTourScenario2 = {
    id: "cd-scheme-gallery-tour",
    name: "Scheme Gallery",
    primaryLabel: "Next",
    skipLabel: "Skip",
    backLabel: "Back",
    doneLabel: "Done",
    skippable: true,
    steps: [
      {
        id: "intro",
        title: "Scheme Gallery",
        body: "Ready-made models: cohort-launch, intro-whatif, investment-case, runway, support-staffing, capacity-service, unit-economics, project-budget, renovation-estimate. Each is a starting point for a specific task \u2014 no need to build from scratch.",
        side: "center"
      },
      {
        id: "open",
        title: "Open the gallery",
        body: "Ctrl+P (or \u2318+P on Mac) opens the gallery. Alternatively, click the Try button at the end of the v1 onboarding carousel. The step activates when the gallery is visible.",
        side: "center",
        passive: true,
        primaryLabel: "Waiting for gallery to open\u2026",
        waitFor: { kind: "signal", signals: ["canvas:scheme-gallery-opened", "canvas:palette-opened"], timeout: 6e4 }
      },
      {
        id: "categories",
        title: "Scheme Categories",
        body: "Left \u2014 list of categories: Unit Economics (ARPU, LTV, CAC, retention, funnel), Product Analytics (NPS, MAU, stickiness), Infrastructure (DB, API-gateway, LB, queue), Patterns (cohort-launch, intro-whatif, investment-case).",
        side: "right",
        anchor: { kind: "rect", rect: { x: 0, y: 0, width: 240, height: 800 } },
        primaryLabel: "Got it"
      },
      {
        id: "preview",
        title: "Scheme Preview",
        body: "Click a scheme card to preview: a mini-scheme with notes, formulas, edges. You can read the structure before applying. The step activates when preview opens.",
        side: "center",
        passive: true,
        primaryLabel: "Open a preview\u2026",
        waitFor: { kind: "signal", signals: ["canvas:scheme-preview-shown"], timeout: 12e4 }
      },
      {
        id: "apply",
        title: "Apply the Scheme",
        body: "In the preview \u2014 Apply button. The scheme becomes the active canvas (replacing the current scene \u2014 undo works). All formulas and edges are preserved, you can edit for your task. The step activates when applied.",
        side: "center",
        passive: true,
        primaryLabel: "Waiting for scheme to apply\u2026",
        waitFor: { kind: "signal", signals: ["canvas:scheme-applied"], timeout: 18e4 }
      },
      {
        id: "edit",
        title: "Edit for your needs",
        body: "After applying \u2014 the same channel as a regular canvas: double-click creates notes, drag from edge creates edges, right-click \u2014 color and parameters palette. Change variable values right in formula notes.",
        side: "top",
        anchor: { kind: "rect", rect: { x: 80, y: 80, width: 400, height: 120 } },
        primaryLabel: "Got it"
      },
      {
        id: "save",
        title: "Saving",
        body: "Canvas auto-saves to the browser storage (OPFS) or to a chosen folder on disk after moving storage in the manager. Export to .canvas and HTML \u2014 manager buttons and the right-click menu.",
        side: "bottom",
        anchor: { kind: "selector", selector: "#gpu-canvas-chip" }
      },
      {
        id: "done",
        title: "Done",
        body: "Scheme gallery \u2014 quick start for typical models. Most user scenarios are covered by built-in schemes; custom ones \u2014 assemble from palette templates (Ctrl+P).",
        side: "center"
      }
    ]
  };
  return __toCommonJS(src_exports);
})();
