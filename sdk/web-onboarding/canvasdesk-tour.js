/*! CanvasDesk inline onboarding engine — pre-built bundle.
 *  Source: sdk/web-onboarding/src/*.ts (TypeScript).
 *  Build:  npm run build  (esbuild)
 *  Rebuild after editing the source; this file is committed for the
 *  convenience of integrators who load via <script> tag without a
 *  bundler (CanvasDesk web index.html does this).
 *
 *  Exposes `window.CanvasDeskTour = { Tour, scenarios, ... }`.
 *
 *  Design influences:
 *    - Intro.js: overlay + tooltip + highlight cutout.
 *    - Guida.js: scenario declared as plain-data JSON.
 *    - flows.sh: step-by-step action runner with explicit waitFor gates.
 *
 *  Zero runtime dependencies.
 */
(function (global) {
  "use strict";

  // ── styles.ts ───────────────────────────────────────────────────
  var DEFAULT_STYLES = [
    ".cd-tour-tooltip{position:fixed;background:#1d2128;color:#d5d9e0;",
    "border:1px solid #333a44;border-radius:8px;",
    "box-shadow:0 8px 24px rgba(0,0,0,0.45);padding:14px 16px 10px;",
    "font:13px/1.5 system-ui,-apple-system,'Segoe UI',sans-serif;",
    "max-width:420px;min-width:280px;pointer-events:auto;",
    "transition:opacity 180ms ease,transform 180ms ease;}",
    ".cd-tour-tooltip__title{font-weight:600;font-size:14px;color:#f3f5f8;margin-bottom:4px;}",
    ".cd-tour-tooltip__body{color:#b9c0cb;white-space:normal;word-wrap:break-word;}",
    ".cd-tour-tooltip__progress{font-size:11px;color:#6b7480;margin-top:6px;}",
    ".cd-tour-tooltip__footer{display:flex;gap:8px;justify-content:flex-end;",
    "align-items:center;margin-top:12px;border-top:1px solid #2a3138;padding-top:8px;}",
    ".cd-tour-btn{appearance:none;font:inherit;padding:6px 12px;border-radius:6px;",
    "border:1px solid #333a44;background:#262c35;color:#d5d9e0;cursor:pointer;",
    "min-height:28px;user-select:none;-webkit-tap-highlight-color:transparent;}",
    ".cd-tour-btn:hover{background:#2e353f;}",
    ".cd-tour-btn:focus-visible{outline:2px solid #8ab4ff;outline-offset:1px;}",
    ".cd-tour-btn_primary{background:#2f6fda;border-color:#2f6fda;color:#fff;font-weight:600;}",
    ".cd-tour-btn_primary:hover{background:#3a78e0;}",
    ".cd-tour-btn_skip{background:transparent;border-color:transparent;color:#6b7480;}",
    ".cd-tour-btn_back{background:transparent;}",
    ".cd-tour-caret{position:absolute;width:12px;height:12px;background:#1d2128;",
    "border:1px solid #333a44;transform:translate(-50%,-50%) rotate(45deg);",
    "pointer-events:none;opacity:0;transition:opacity 180ms ease;}",
    ".cd-tour-tooltip[data-caret-dir=up] .cd-tour-caret,",
    ".cd-tour-tooltip[data-caret-dir=down] .cd-tour-caret,",
    ".cd-tour-tooltip[data-caret-dir=left] .cd-tour-caret,",
    ".cd-tour-tooltip[data-caret-dir=right] .cd-tour-caret{opacity:1;}",
    ".cd-tour-tooltip[data-caret-dir=up] .cd-tour-caret{border-bottom:none;border-right:none;}",
    ".cd-tour-tooltip[data-caret-dir=down] .cd-tour-caret{border-top:none;border-left:none;}",
    ".cd-tour-tooltip[data-caret-dir=left] .cd-tour-caret{border-top:none;border-right:none;}",
    ".cd-tour-tooltip[data-caret-dir=right] .cd-tour-caret{border-bottom:none;border-left:none;}",
    ".cd-tour-tooltip[data-caret-dir=none] .cd-tour-caret{display:none;}",
    ".cd-tour-dim{cursor:pointer;}",
    "@media (max-width:480px),(pointer:coarse){",
    ".cd-tour-tooltip{min-width:0;max-width:calc(100vw - 32px);padding:12px 14px 8px;}",
    ".cd-tour-btn{padding:10px 14px;min-height:36px;}}"
  ].join("");

  // ── positioning.ts ──────────────────────────────────────────────
  var GAP = 12;
  var VIEWPORT_PAD = 8;
  var TOOLTIP_MIN_W = 280;
  var TOOLTIP_MAX_W = 420;
  var TOOLTIP_MIN_H = 120;

  function measureTooltip(el, maxW) {
    maxW = maxW || TOOLTIP_MAX_W;
    var prev = el.style.cssText;
    el.style.cssText = "position:fixed;left:-9999px;top:-9999px;width:" + maxW +
      "px;visibility:hidden;opacity:0;";
    var rect = el.getBoundingClientRect();
    el.style.cssText = prev;
    return {
      width: Math.max(TOOLTIP_MIN_W, Math.min(maxW, Math.ceil(rect.width))),
      height: Math.max(TOOLTIP_MIN_H, Math.ceil(rect.height))
    };
  }

  function getElementRect(el) {
    var r = el.getBoundingClientRect();
    return { x: r.left, y: r.top, width: r.width, height: r.height };
  }

  var OPPOSITES = {
    "top": "bottom", "top-start": "bottom-start", "top-end": "bottom-end",
    "bottom": "top", "bottom-start": "top-start", "bottom-end": "top-end",
    "left": "right", "left-start": "right-start", "left-end": "right-end",
    "right": "left", "right-start": "left-start", "right-end": "left-end"
  };

  function orderCandidates(side) {
    return [side, OPPOSITES[side] || side, "bottom", "top", "right", "left", "center"];
  }

  function computePlacement(side, anchor, tip, vp) {
    vp = vp || { width: global.innerWidth, height: global.innerHeight };
    if (side === "center" || anchor === null) {
      return {
        side: "center",
        x: Math.round((vp.width - tip.width) / 2),
        y: Math.round((vp.height - tip.height) / 2),
        width: tip.width, height: tip.height, caret: null
      };
    }
    var candidates = orderCandidates(side);
    for (var i = 0; i < candidates.length; i++) {
      var p = tryPlacement(candidates[i], anchor, tip, vp);
      if (p) return p;
    }
    return {
      side: "center",
      x: Math.round((vp.width - tip.width) / 2),
      y: Math.round((vp.height - tip.height) / 2),
      width: tip.width, height: tip.height, caret: null
    };
  }

  function tryPlacement(side, a, tip, vp) {
    var x, y, caret;
    switch (side) {
      case "top":
        x = a.x + (a.width - tip.width) / 2;
        y = a.y - tip.height - GAP;
        caret = { x: a.x + a.width / 2, y: a.y - GAP, dir: "down" };
        break;
      case "top-start":
        x = a.x; y = a.y - tip.height - GAP;
        caret = { x: a.x + 24, y: a.y - GAP, dir: "down" }; break;
      case "top-end":
        x = a.x + a.width - tip.width; y = a.y - tip.height - GAP;
        caret = { x: a.x + a.width - 24, y: a.y - GAP, dir: "down" }; break;
      case "bottom":
        x = a.x + (a.width - tip.width) / 2;
        y = a.y + a.height + GAP;
        caret = { x: a.x + a.width / 2, y: a.y + a.height + GAP, dir: "up" }; break;
      case "bottom-start":
        x = a.x; y = a.y + a.height + GAP;
        caret = { x: a.x + 24, y: a.y + a.height + GAP, dir: "up" }; break;
      case "bottom-end":
        x = a.x + a.width - tip.width; y = a.y + a.height + GAP;
        caret = { x: a.x + a.width - 24, y: a.y + a.height + GAP, dir: "up" }; break;
      case "left":
        x = a.x - tip.width - GAP; y = a.y + (a.height - tip.height) / 2;
        caret = { x: a.x - GAP, y: a.y + a.height / 2, dir: "right" }; break;
      case "left-start":
        x = a.x - tip.width - GAP; y = a.y;
        caret = { x: a.x - GAP, y: a.y + 24, dir: "right" }; break;
      case "left-end":
        x = a.x - tip.width - GAP; y = a.y + a.height - tip.height;
        caret = { x: a.x - GAP, y: a.y + a.height - 24, dir: "right" }; break;
      case "right":
        x = a.x + a.width + GAP; y = a.y + (a.height - tip.height) / 2;
        caret = { x: a.x + a.width + GAP, y: a.y + a.height / 2, dir: "left" }; break;
      case "right-start":
        x = a.x + a.width + GAP; y = a.y;
        caret = { x: a.x + a.width + GAP, y: a.y + 24, dir: "left" }; break;
      case "right-end":
        x = a.x + a.width + GAP; y = a.y + a.height - tip.height;
        caret = { x: a.x + a.width + GAP, y: a.y + a.height - 24, dir: "left" }; break;
      default: return null;
    }
    if (x < VIEWPORT_PAD || x + tip.width > vp.width - VIEWPORT_PAD ||
        y < VIEWPORT_PAD || y + tip.height > vp.height - VIEWPORT_PAD) {
      return null;
    }
    return { side: side, x: Math.round(x), y: Math.round(y),
             width: tip.width, height: tip.height, caret: caret };
  }

  // ── highlight.ts ────────────────────────────────────────────────
  var DIM_DEFAULTS = { padding: 8, radius: 6, dim: true, dimColor: "rgba(0,0,0,0.55)", animate: true };

  function createHighlight(container, baseZIndex) {
    var overlay = document.createElement("div");
    overlay.className = "cd-tour-dim";
    overlay.style.cssText = "position:fixed;inset:0;z-index:" + (baseZIndex) +
      ";pointer-events:auto;background:transparent;transition:opacity 180ms ease;opacity:0;";
    container.appendChild(overlay);
    var current = null;
    var raf = null;

    function paint() {
      raf = null;
      if (!current) {
        overlay.style.opacity = "0";
        overlay.style.background = "transparent";
        overlay.style.pointerEvents = "none";
        return;
      }
      var rect = current.rect, opts = current.opts;
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
      var p = opts.padding;
      var x = rect.x - p, y = rect.y - p;
      var w = rect.width + 2 * p, h = rect.height + 2 * p;
      var r = opts.radius;
      var svg = '<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%">' +
        '<defs><mask id="m"><rect width="100%" height="100%" fill="white"/>' +
        '<rect x="' + x + '" y="' + y + '" width="' + w + '" height="' + h +
        '" rx="' + r + '" ry="' + r + '" fill="black"/></mask></defs>' +
        '<rect width="100%" height="100%" fill="' + escapeCss(opts.dimColor) + '" mask="url(#m)"/></svg>';
      overlay.style.background = "url(\"data:image/svg+xml;utf8," + encodeURIComponent(svg) + '") no-repeat';
      overlay.style.backgroundSize = "100% 100%";
    }

    function schedulePaint() {
      if (raf !== null) return;
      raf = requestAnimationFrame(paint);
    }

    return {
      setAnchor: function (rect, options) {
        var opts = Object.assign({}, DIM_DEFAULTS, options || {});
        current = { rect: rect, opts: opts };
        schedulePaint();
      },
      hide: function () { current = null; schedulePaint(); },
      destroy: function () {
        if (raf !== null) cancelAnimationFrame(raf);
        overlay.remove();
      }
    };
  }

  function escapeCss(c) {
    return c.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  }

  // ── tooltip.ts ──────────────────────────────────────────────────
  function escapeHtml(s) {
    return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }

  function renderDefaultTooltip(ctx) {
    var root = document.createElement("div");
    root.className = "cd-tour-tooltip";
    root.setAttribute("role", "dialog");
    root.setAttribute("aria-modal", "false");

    if (ctx.step.title) {
      var h = document.createElement("div");
      h.className = "cd-tour-tooltip__title";
      h.innerHTML = escapeHtml(ctx.step.title);
      root.appendChild(h);
    }
    if (ctx.step.body) {
      var b = document.createElement("div");
      b.className = "cd-tour-tooltip__body";
      b.innerHTML = escapeHtml(ctx.step.body);
      root.appendChild(b);
    }

    var prog = document.createElement("div");
    prog.className = "cd-tour-tooltip__progress";
    prog.textContent = (ctx.index + 1) + " / " + ctx.total;
    root.appendChild(prog);

    var caret = document.createElement("div");
    caret.className = "cd-tour-caret";
    root.appendChild(caret);

    var footer = document.createElement("div");
    footer.className = "cd-tour-tooltip__footer";

    if (ctx.skippable && !ctx.step.hideSkip) {
      var skip = document.createElement("button");
      skip.type = "button";
      skip.className = "cd-tour-btn cd-tour-btn_skip";
      skip.textContent = ctx.skipLabel;
      skip.addEventListener("click", ctx.onSkip);
      footer.appendChild(skip);
    }

    if (!ctx.isFirst) {
      var back = document.createElement("button");
      back.type = "button";
      back.className = "cd-tour-btn cd-tour-btn_back";
      back.textContent = ctx.backLabel;
      back.addEventListener("click", ctx.onBack);
      footer.appendChild(back);
    }

    var next = document.createElement("button");
    next.type = "button";
    next.className = "cd-tour-btn cd-tour-btn_primary";
    next.textContent = ctx.isLast ? ctx.doneLabel : ctx.primaryLabel;
    next.addEventListener("click", ctx.onPrimary);
    if (ctx.step.passive) next.style.display = "none";
    footer.appendChild(next);

    root.appendChild(footer);
    return root;
  }

  // ── actions.ts ──────────────────────────────────────────────────
  function WaitForError(spec, message) {
    var err = Error.call(this, message);
    this.spec = spec;
    this.message = message;
    this.name = "WaitForError";
    if (Error.captureStackTrace) Error.captureStackTrace(this, WaitForError);
    return this;
  }
  WaitForError.prototype = Object.create(Error.prototype);

  function resolveElement(a) {
    if (a.target === "element" && a.element) return a.element;
    if (a.selector) return document.querySelector(a.selector);
    return null;
  }

  function simulateClick(el) {
    try { el.focus && el.focus({ preventScroll: true }); } catch (e) {}
    var opts = { bubbles: true, cancelable: true, view: global, button: 0 };
    el.dispatchEvent(new MouseEvent("mousedown", opts));
    el.dispatchEvent(new MouseEvent("mouseup", opts));
    el.dispatchEvent(new MouseEvent("click", opts));
  }

  function performActions(actions, ctx) {
    return new Promise(function (resolve, reject) {
      var i = 0;
      function next() {
        if (i >= actions.length) return resolve();
        var a = actions[i++];
        try {
          if (a.kind === "click") {
            var el = resolveElement(a);
            if (!el) return reject(new Error("click: target not found"));
            simulateClick(el);
            ctx.log("info", "perform: click");
          } else if (a.kind === "focus") {
            var el2 = a.element || (a.selector ? document.querySelector(a.selector) : null);
            if (!el2) return reject(new Error("focus: target not found"));
            el2.focus({ preventScroll: false });
          } else if (a.kind === "signal") {
            ctx.signal(a.signal, a.payload);
          } else if (a.kind === "dispatch") {
            var el3 = resolveElement(a);
            if (!el3) return reject(new Error("dispatch: target not found"));
            el3.dispatchEvent(new Event(a.eventType, a.init || {}));
          }
          next();
        } catch (e) { reject(e); }
      }
      next();
    });
  }

  function awaitWaitFor(spec, ctx) {
    var cancelled = false;
    var timer = null, poll = null, cleanup = null;
    var timeout = spec.timeout || 10000;

    var promise = new Promise(function (resolve, reject) {
      function ok() { doCleanup(); resolve(); }
      function err(e) { doCleanup(); reject(e); }
      function doCleanup() {
        if (timer) clearTimeout(timer);
        if (poll) clearInterval(poll);
        if (cleanup) cleanup();
      }
      if (cancelled) { err(new Error("waitFor cancelled")); return; }

      timer = setTimeout(function () {
        err(new WaitForError(spec, "waitFor timed out after " + timeout + "ms"));
      }, timeout);

      if (spec.kind === "selector") {
        if (document.querySelector(spec.selector)) { ok(); return; }
        poll = setInterval(function () {
          if (cancelled) return err(new Error("waitFor cancelled"));
          if (document.querySelector(spec.selector)) ok();
        }, 60);
      } else if (spec.kind === "predicate") {
        if (spec.predicate()) { ok(); return; }
        poll = setInterval(function () {
          if (cancelled) return err(new Error("waitFor cancelled"));
          if (spec.predicate()) ok();
        }, 60);
      } else if (spec.kind === "event") {
        var target = spec.target === "document" ? document :
                     spec.target === "window" ? global : spec.target;
        var handler = function () { ok(); };
        target.addEventListener(spec.eventType, handler, { once: true });
        cleanup = function () { target.removeEventListener(spec.eventType, handler); };
      } else if (spec.kind === "signal") {
        if (!ctx.subscribe) { err(new Error("waitFor(signal): no bus")); return; }
        cleanup = ctx.subscribe(spec.signals[0], function () { ok(); });
        for (var i = 1; i < spec.signals.length; i++) {
          ctx.subscribe(spec.signals[i], function () { ok(); });
        }
      }
    });

    function cancel() {
      cancelled = true;
      if (timer) clearTimeout(timer);
      if (poll) clearInterval(poll);
      if (cleanup) cleanup();
    }

    return { promise: promise, cancel: cancel };
  }

  // ── tour.ts ─────────────────────────────────────────────────────
  var stylesInjected = false;
  function injectStyles(container, css) {
    if (stylesInjected) return;
    var style = document.createElement("style");
    style.dataset.cdTourStyles = "1";
    style.textContent = css;
    var root = container.getRootNode();
    (root === document ? document.head : root).appendChild(style);
    stylesInjected = true;
  }

  function resolveAnchor(spec, hooks) {
    if (!spec) return null;
    if (hooks && hooks.resolveAnchor) {
      var r = hooks.resolveAnchor(spec);
      if (r) return resolveAnchorRect(r);
    }
    return resolveAnchorRect(spec);
  }

  function resolveAnchorRect(spec) {
    if (spec.kind === "rect") return spec.rect;
    if (spec.kind === "element") return getElementRect(spec.element);
    if (spec.kind === "selector") {
      var el = document.querySelector(spec.selector);
      return el ? getElementRect(el) : (spec.fallback || null);
    }
    return null;
  }

  function Tour(opts) {
    opts = opts || {};
    this.container = opts.container || document.body;
    this.zIndex = opts.baseZIndex || 10000;
    this.log = opts.log || function (lvl, msg) {
      if (global.console && global.console[lvl]) global.console[lvl]("[tour] " + msg);
    };
    this.hooks = {};
    this.highlight = createHighlight(this.container, this.zIndex + 1);
    this.active = null;
    this.keyHandler = null;
    this.refreshRaf = null;
    this.signalBus = new Set();
    injectStyles(this.container, opts.styles || DEFAULT_STYLES);
    Tour.instance = this;
  }

  Tour.prototype.onSignal = function (listener) {
    var self = this;
    this.signalBus.add(listener);
    return function () { self.signalBus.delete(listener); };
  };

  Tour.prototype.signal = function (name, payload) {
    this.log("info", "signal: " + name);
    this.signalBus.forEach(function (l) { l(name, payload); });
  };

  Tour.prototype.run = function (scenario) {
    if (this.active) {
      this.log("warn", "scenario still active; cancelling");
      this.cancel();
    }
    var handle = this.start(scenario);
    if (scenario.onStart) scenario.onStart(handle);
    return handle;
  };

  Tour.prototype.start = function (scenario) {
    var self = this;
    var active = {
      scenario: scenario,
      step: scenario.steps[0],
      index: 0,
      tooltipEl: null,
      lastAnchorRect: null,
      wait: null,
      advanceUnsub: null,
      handle: null
    };
    var handle = {
      scenario: scenario,
      get step() { return active.step; },
      get index() { return active.index; },
      next: function () { self.next(); },
      prev: function () { self.prev(); },
      skip: function () { self.skip(); },
      resolve: function (sig) { self.resolveWait(sig); },
      signal: function (n, p) { self.signal(n, p); },
      cancel: function () { self.cancel(); }
    };
    active.handle = handle;
    this.active = active;

    this.keyHandler = function (e) { self.onKey(e); };
    document.addEventListener("keydown", this.keyHandler, true);

    this.startRefreshRaf();
    this.activateStep(0);
    return handle;
  };

  Tour.prototype.next = function () {
    if (!this.active) return;
    var a = this.active;
    if (a.wait && !a.wait.resolved) {
      this.log("info", "next() ignored — waitFor pending");
      return;
    }
    if (a.index >= a.scenario.steps.length - 1) { this.complete(); return; }
    this.activateStep(a.index + 1);
  };

  Tour.prototype.prev = function () {
    if (!this.active || this.active.index <= 0) return;
    this.activateStep(this.active.index - 1);
  };

  Tour.prototype.skip = function () {
    if (!this.active) return;
    var a = this.active;
    this.cleanup();
    if (a.scenario.onSkip) a.scenario.onSkip(a.handle);
  };

  Tour.prototype.complete = function () {
    if (!this.active) return;
    var a = this.active;
    this.cleanup();
    if (a.scenario.onComplete) a.scenario.onComplete(a.handle);
    this.signal("tour:" + a.scenario.id + ":complete");
  };

  Tour.prototype.cancel = function () {
    if (!this.active) return;
    this.cleanup();
  };

  Tour.prototype.resolveWait = function (sig) {
    if (!this.active || !this.active.wait) return;
    this.active.wait.resolved = true;
    if (this.active.wait.cancel) this.active.wait.cancel();
    this.log("info", "waitFor resolved via " + (sig ? ("signal " + sig) : "external"));
    if (this.active.scenario.autoAdvance) this.next();
  };

  Tour.prototype.onKey = function (e) {
    if (!this.active) return;
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    switch (e.key) {
      case "Escape":
        e.preventDefault(); e.stopPropagation();
        if (this.active.scenario.skippable !== false) this.skip();
        break;
      case "Enter":
      case "ArrowRight":
        e.preventDefault(); e.stopPropagation();
        this.next();
        break;
      case "ArrowLeft":
        e.preventDefault(); e.stopPropagation();
        this.prev();
        break;
    }
  };

  Tour.prototype.subscribeForWait = function (name, cb) {
    return this.onSignal(function (sigName) { if (sigName === name) cb(); });
  };

  Tour.prototype.activateStep = function (index) {
    if (!this.active) return;
    var a = this.active;
    if (a.tooltipEl) { a.tooltipEl.remove(); a.tooltipEl = null; }
    if (a.advanceUnsub) { a.advanceUnsub(); a.advanceUnsub = null; }
    if (a.wait) { if (a.wait.cancel) a.wait.cancel(); a.wait = null; }

    a.index = index;
    var step = a.scenario.steps[index];
    a.step = step;
    if (a.scenario.onStep) a.scenario.onStep(step, index, a.handle);

    var rect = resolveAnchor(step.anchor, this.hooks);
    a.lastAnchorRect = rect;
    // Passive steps (no Next button) — user must perform an action on
    // the host surface. Disable dim so clicks reach the canvas.
    var highlightOpts = step.highlight || {};
    if (step.passive) {
      if (highlightOpts.dim === undefined || highlightOpts.dim === null) {
        highlightOpts.dim = false;
      }
    }
    this.highlight.setAnchor(rect, highlightOpts);

    var ctx = {
      container: this.container, step: step, index: index, total: a.scenario.steps.length,
      side: step.side || "bottom", anchor: rect,
      primaryLabel: step.primaryLabel || a.scenario.primaryLabel || "Далее",
      skipLabel: a.scenario.skipLabel || "Пропустить",
      backLabel: a.scenario.backLabel || "Назад",
      doneLabel: a.scenario.doneLabel || "Готово",
      isLast: index === a.scenario.steps.length - 1,
      isFirst: index === 0,
      skippable: a.scenario.skippable !== false,
      onPrimary: () => this.next(),
      onBack: () => this.prev(),
      onSkip: () => this.skip()
    };
    var tooltipEl = (this.hooks.renderTooltip && this.hooks.renderTooltip(ctx)) || renderDefaultTooltip(ctx);
    tooltipEl.style.zIndex = String(this.zIndex + 2);
    a.tooltipEl = tooltipEl;
    this.container.appendChild(tooltipEl);

    this.placeTooltip(tooltipEl, ctx.side, rect);

    var actx = {
      signal: (n, p) => this.signal(n, p),
      log: this.log,
      subscribe: (n, cb) => this.subscribeForWait(n, cb)
    };

    if (step.perform && step.perform.length > 0) {
      performActions(step.perform, actx).catch((e) => {
        if (e instanceof WaitForError) this.log("warn", "perform WaitForError: " + e.message);
        else this.log("error", "perform crashed: " + e.message);
      });
    }

    if (step.waitFor) {
      var wait = awaitWaitFor(step.waitFor, actx);
      a.wait = { resolved: false, cancel: wait.cancel };
      var self = this;
      wait.promise.then(function () {
        if (!a.wait) return;
        a.wait.resolved = true;
        // Passive step (no Next button) — waitFor resolving is the
        // only path forward. Always advance, regardless of
        // scenario.autoAdvance. For non-passive steps, respect the
        // scenario.autoAdvance flag (default false — user clicks Next).
        if (a.scenario.autoAdvance || step.passive) self.next();
      }).catch(function (e) {
        if (e instanceof WaitForError) self.log("warn", "waitFor soft-failed: " + e.message);
        else self.log("error", "waitFor crashed: " + e.message);
      });
    }

    if (step.advanceOnClick) {
      var target = typeof step.advanceOnClick === "string"
        ? document.querySelector(step.advanceOnClick) : step.advanceOnClick;
      if (target) {
        var handler = function () { self.next(); };
        target.addEventListener("click", handler, { once: true });
        a.advanceUnsub = function () { target.removeEventListener("click", handler); };
      } else {
        this.log("warn", "advanceOnClick: target not found");
      }
    }
  };

  Tour.prototype.placeTooltip = function (el, side, rect) {
    var size = measureTooltip(el);
    var placement = computePlacement(side, rect, size);
    el.style.position = "fixed";
    el.style.left = placement.x + "px";
    el.style.top = placement.y + "px";
    el.style.width = placement.width + "px";
    el.dataset.caretDir = (placement.caret && placement.caret.dir) || "none";
    if (placement.caret) {
      var caret = el.querySelector(".cd-tour-caret");
      if (caret) {
        var lx = placement.caret.x - placement.x;
        var ly = placement.caret.y - placement.y;
        caret.style.left = lx + "px";
        caret.style.top = ly + "px";
      }
    }
  };

  Tour.prototype.refresh = function () {
    if (!this.active) return;
    var a = this.active;
    var rect = resolveAnchor(a.step.anchor, this.hooks);
    a.lastAnchorRect = rect;
    // Same passive-dim logic as activateStep — rAF refresh must not
    // re-enable dim (would re-block canvas clicks every frame).
    var rOpts = a.step.highlight ? Object.assign({}, a.step.highlight) : {};
    if (a.step.passive && (rOpts.dim === undefined || rOpts.dim === null)) {
      rOpts.dim = false;
    }
    this.highlight.setAnchor(rect, rOpts);
    if (a.tooltipEl) this.placeTooltip(a.tooltipEl, a.step.side || "bottom", rect);
  };

  Tour.prototype.startRefreshRaf = function () {
    if (this.refreshRaf !== null) return;
    var self = this;
    function tick() {
      if (!self.active) { self.refreshRaf = null; return; }
      self.refresh();
      self.refreshRaf = requestAnimationFrame(tick);
    }
    this.refreshRaf = requestAnimationFrame(tick);
  };

  Tour.prototype.cleanup = function () {
    if (!this.active) return;
    var a = this.active;
    if (a.tooltipEl) a.tooltipEl.remove();
    if (a.wait && a.wait.cancel) a.wait.cancel();
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
  };

  Tour.instance = null;

  // ── scenarios/*.ts (inlined as plain data) ─────────────────────
  var toolbarTourScenario = {
    id: "cd-toolbar-tour",
    name: "Тур по панели хранилища",
    primaryLabel: "Далее", skipLabel: "Пропустить",
    backLabel: "Назад", doneLabel: "Готово", skippable: true,
    steps: [
      {
        id: "intro", title: "Панель хранилища",
        body: "В верхнем правом углу — три кнопки для работы с .canvas-файлами. Пройдёмся по каждой.",
        side: "bottom", anchor: { kind: "selector", selector: "#w6-toolbar" }
      },
      {
        id: "open", title: "Открыть с диска",
        body: "Открывает системный диалог выбора .canvas-файла. После выбора файл становится активным канвасом и автосохраняется обратно на диск.",
        side: "bottom", anchor: { kind: "selector", selector: "#btn-open" },
        advanceOnClick: "#btn-open", primaryLabel: "Понятно"
      },
      {
        id: "recent", title: "Недавние",
        body: "Кнопка показывает последний открывавшийся канвас. Полное имя — в title-тултипе, ширина ограничена, чтобы панель никогда не уходила за край экрана.",
        side: "bottom", anchor: { kind: "selector", selector: "#btn-recent" }
      },
      {
        id: "export", title: "Экспорт .canvas",
        body: "Скачивает последнюю сохранённую версию канваса. Удобно для бэкапа или если автосейв в файл недоступен (например, в браузере без File System Access API).",
        side: "bottom", anchor: { kind: "selector", selector: "#btn-export" }
      },
      {
        id: "done", title: "Готово",
        body: "Канвас всегда сохраняется автоматически — эти кнопки для ручного контроля.",
        side: "center"
      }
    ]
  };

  var firstRunInlineScenario = {
    id: "cd-first-run-inline",
    name: "Первый запуск: интерактивный тур",
    primaryLabel: "Далее", skipLabel: "Пропустить",
    backLabel: "Назад", doneLabel: "Готово",
    skippable: true, autoAdvance: false,
    steps: [
      {
        id: "welcome", title: "Это интерактивный тур",
        body: "Карточки-карусель уже показали основы. Теперь попробуем по-настоящему — каждый шаг сопровождается действием, и кнопка «Далее» активируется только когда вы его выполните. Выход — Esc — всегда виден.",
        side: "center"
      },
      {
        id: "locate-toolbar", title: "Панель хранилища",
        body: "В правом верхнем углу — кнопки для работы с .canvas-файлами. Открыть, недавние, экспорт. Канвас автосохраняется в выбранный файл.",
        side: "bottom", anchor: { kind: "selector", selector: "#w6-toolbar" }
      },
      {
        id: "create-note", title: "Создайте первую заметку",
        body: "Сделайте двойной клик по пустому месту на канвасе. Появится новая заметка с курсором внутри — можно сразу писать текст, markdown-разметку или формулу Numi. «Далее» активируется, когда заметка создана.",
        side: "center", passive: true, primaryLabel: "Жду действия…",
        waitFor: { kind: "signal", signals: ["canvas:note-created", "canvas:note-activated"], timeout: 120000 }
      },
      {
        id: "write-formula", title: "Попробуйте формулу Numi",
        body: "В заметке напишите: rps = 1200 и на следующей строке daily = rps / 86400. Заметка подсветит результат вычисления. Numi понимает единицы (rps, ms, MB/s) и подсказки при вводе.",
        side: "top", anchor: { kind: "rect", rect: { x: 80, y: 80, width: 320, height: 80 } },
        primaryLabel: "Понятно"
      },
      {
        id: "connect-nodes", title: "Связи и поток значений",
        body: "Создайте вторую заметку. Перетащите от края первой ко второй — появится связь. В первой напишите $in = 10, во второй — $in * 2. Связь передаст значение и вторая заметка покажет 20.",
        side: "top", anchor: { kind: "rect", rect: { x: 80, y: 80, width: 320, height: 80 } },
        primaryLabel: "Понятно"
      },
      {
        id: "palette", title: "Палитра шаблонов",
        body: "Ctrl+P открывает палитру готовых нод: метрики unit-economics (ARPU, LTV, CAC), инфраструктура (DB, API-gateway, LB), потоки (funnel-conv, retention-d7). Shift+клик по колесу вставляет выбранный шаблон.",
        side: "center"
      },
      {
        id: "help", title: "Кнопка «?» и F1",
        body: "«?» — меню помощи: документация, повтор онбординга (карточки и этот интерактивный тур), UI-консоль. F1 — список горячих клавиш. Документация по всем функциям — в user-docs/.",
        side: "center"
      },
      {
        id: "done", title: "Готово!",
        body: "Базовый набор освоен. Остальное — по мере необходимости из документации. Канвас сохраняется автоматически.",
        side: "center"
      }
    ]
  };

  var paletteTourScenario = {
    id: "cd-palette-tour",
    name: "Палитра шаблонов",
    primaryLabel: "Далее", skipLabel: "Пропустить",
    backLabel: "Назад", doneLabel: "Готово", skippable: true,
    steps: [
      {
        id: "intro", title: "Палитра шаблонов",
        body: "В CanvasDesk есть встроенные шаблоны нод: метрики, инфраструктура, потоки. Их можно перетаскивать на канвас как готовые блоки.",
        side: "center"
      },
      {
        id: "open", title: "Откройте палитру",
        body: "Нажмите Ctrl+P (или ⌘+P на Mac). Слева появится колесо-навигатор с категориями.",
        side: "center", passive: true, primaryLabel: "Жду открытия палитры…",
        waitFor: { kind: "signal", signals: ["canvas:palette-opened"], timeout: 60000 }
      },
      {
        id: "categories", title: "Категории",
        body: "Колесо переключает категории: Unit Economics, Product Analytics, Infrastructure, Patterns. Shift+клик — выбор категории.",
        side: "right", anchor: { kind: "rect", rect: { x: 0, y: 0, width: 240, height: 800 } }
      },
      {
        id: "shift-click", title: "Shift+клик — вставить шаблон",
        body: "Shift+клик по шаблону вставляет его в центр канваса со стандартными параметрами. После вставки параметры редактируются через ПКМ.",
        side: "right", anchor: { kind: "rect", rect: { x: 0, y: 0, width: 240, height: 800 } }
      },
      {
        id: "done", title: "Готово",
        body: "Шаблоны — это строительные блоки для канвас-схем. Большинство моделей собирается за 5–10 минут.",
        side: "center"
      }
    ]
  };

  var calculationsTourScenario = {
    id: "cd-calculations-tour",
    name: "Расчёты в CanvasDesk",
    primaryLabel: "Далее", skipLabel: "Пропустить",
    backLabel: "Назад", doneLabel: "Готово", skippable: true,
    steps: [
      {
        id: "intro", title: "Формулы прямо в заметках",
        body: "Любая заметка — это Numi-формула: можно объявить переменную, использовать единицы (rps, ms, MB/s), получить результат. Связи между заметками передают значения — это и есть поток.",
        side: "center"
      },
      {
        id: "assignment", title: "Объявление переменной",
        body: "Формат: name = expression. Пример: rps = 1200. Заметка подсветит имя и значение, курсор сразу в редактировании правой части.",
        side: "top", anchor: { kind: "rect", rect: { x: 80, y: 120, width: 320, height: 60 } }
      },
      {
        id: "units", title: "Единицы измерения",
        body: "Numi понимает единицы: rps, ms, MB/s, €, users. Они проверяются на совместимость — нельзя сложить rps и ms. Подсказки появляются при вводе.",
        side: "top", anchor: { kind: "rect", rect: { x: 80, y: 180, width: 320, height: 60 } }
      },
      {
        id: "value-flow", title: "Поток значений по связям",
        body: "Связь между заметками передаёт значение. В подчинённой заметке $in — входящее значение, $N — массив всех входящих. Пересчёт — мгновенный, при изменении любой заметки.",
        side: "top", anchor: { kind: "rect", rect: { x: 80, y: 240, width: 320, height: 60 } }
      },
      {
        id: "whatif", title: "What-if сценарии",
        body: "Откройте What-if панель (FR-017) и задайте диапазоны значений — CanvasDesk посчитает чувствительность результата. Полезно для unit-economics: «что если конверсия упадёт на 10%».",
        side: "center"
      },
      {
        id: "monte-carlo", title: "Монте-Карло (FR-066)",
        body: "Для вероятностных моделей — задайте распределения параметров и CanvasDesk прогонит симуляцию. Результаты отображаются как гистограмма в панели результата.",
        side: "center"
      },
      {
        id: "done", title: "Готово",
        body: "Расчёты в CanvasDesk — это Numi + граф значений + симуляции. Никакого Excel.",
        side: "center"
      }
    ]
  };

  var scenarios = {
    toolbarTourScenario: toolbarTourScenario,
    firstRunInlineScenario: firstRunInlineScenario,
    paletteTourScenario: paletteTourScenario,
    calculationsTourScenario: calculationsTourScenario
  };

  // ── exports ─────────────────────────────────────────────────────
  global.CanvasDeskTour = {
    Tour: Tour,
    scenarios: scenarios,
    DEFAULT_STYLES: DEFAULT_STYLES,
    computePlacement: computePlacement,
    getElementRect: getElementRect,
    measureTooltip: measureTooltip,
    performActions: performActions,
    awaitWaitFor: awaitWaitFor,
    WaitForError: WaitForError,
    createHighlight: createHighlight,
    renderDefaultTooltip: renderDefaultTooltip
  };
})(typeof window !== "undefined" ? window : this);
