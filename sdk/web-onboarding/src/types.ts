/**
 * @file web-onboarding/src/types.ts
 * @summary Public types for the CanvasDesk inline onboarding engine.
 *
 * The engine is intentionally framework-agnostic and DOM-only: it overlays
 * HTML tooltips and dim/highlight layers on top of the host application
 * (which may itself render to a <canvas>, as CanvasDesk does via WebGPU).
 *
 * Design influences:
 *  - Intro.js   — overlay + tooltip + highlight cutout pattern.
 *  - Guida.js   — scenario declared as a serialisable JSON object.
 *  - flows.sh   — step-by-step action runner with explicit "waitFor"
 *                 contracts so a step can require a user gesture or
 *                 an observable state transition before advancing.
 *
 * All shapes are plain data (structurally typed) so scenarios can be
 * loaded from JSON without a parser.
 */

/** Compass directions for tooltip placement relative to the anchor. */
export type TooltipSide =
  | "top" | "top-start" | "top-end"
  | "bottom" | "bottom-start" | "bottom-end"
  | "left" | "left-start" | "left-end"
  | "right" | "right-start" | "right-end"
  | "center";

/**
 * Anchor spec — what the tooltip points at. Three families:
 *   - selector: a CSS selector resolved at step activation time.
 *   - element:  a direct DOM node (for programmatic scenarios).
 *   - rect:     a fixed viewport rect in CSS pixels (for canvas-internal
 *               regions that have no DOM representation — the engine
 *               synthesises an invisible anchor).
 */
export interface AnchorSelector {
  kind: "selector";
  selector: string;
  /** If the selector misses at runtime, fall back to this viewport rect
   *  (or null → skip the highlight and centre the tooltip). */
  fallback?: Rect | null;
}
export interface AnchorElement {
  kind: "element";
  element: HTMLElement;
}
export interface AnchorRect {
  kind: "rect";
  rect: Rect;
}
export type AnchorSpec = AnchorSelector | AnchorElement | AnchorRect;

/** Viewport rect in CSS pixels. */
export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Visual hint painted around the anchor (Intro.js-style). */
export interface HighlightOptions {
  /** Padding around the anchor rect (px). Default: 8. */
  padding?: number;
  /** Border-radius of the cutout (px). Default: 6. */
  radius?: number;
  /** Dim the rest of the screen. Default: true. */
  dim?: boolean;
  /** Dim colour (any CSS color). Default: rgba(0,0,0,0.55). */
  dimColor?: string;
  /** Animated transition between anchor rects. Default: true. */
  animate?: boolean;
}

/** "WaitFor" predicate — gate progression. Mirrors flows.sh semantics. */
export interface WaitForSelector {
  /** Wait until a CSS selector resolves to a node (polls). */
  kind: "selector";
  selector: string;
  /** Timeout (ms). Default: 10000. */
  timeout?: number;
}
export interface WaitForPredicate {
  /** Wait until a predicate returns true. */
  kind: "predicate";
  predicate: () => boolean;
  timeout?: number;
}
export interface WaitForEvent {
  /** Wait until an event fires on a target. */
  kind: "event";
  target: "document" | "window" | HTMLElement;
  eventType: string;
  timeout?: number;
}
export interface WaitForSignal {
  /** Wait until the engine's signal bus emits one of the listed signals. */
  kind: "signal";
  signals: string[];
  timeout?: number;
}
export type WaitForSpec =
  | WaitForSelector
  | WaitForPredicate
  | WaitForEvent
  | WaitForSignal;

/** "Perform" — a single atomic user action the engine executes. */
export interface PerformClick {
  kind: "click";
  target: "selector" | "element";
  selector?: string;
  element?: HTMLElement;
}
export interface PerformFocus {
  kind: "focus";
  selector?: string;
  element?: HTMLElement;
}
export interface PerformSignal {
  /** Emit a signal into the engine bus. Host app listens via onSignal. */
  kind: "signal";
  signal: string;
  payload?: unknown;
}
export interface PerformDispatch {
  /** Dispatch a synthetic event on a target. */
  kind: "dispatch";
  target: "selector" | "element";
  selector?: string;
  element?: HTMLElement;
  eventType: string;
  /** Optional event init payload. */
  init?: EventInit;
}
export type PerformSpec =
  | PerformClick
  | PerformFocus
  | PerformSignal
  | PerformDispatch;

/**
 * A single step of a scenario. All fields except `title` are optional:
 * a step may either explain, highlight, perform an action, or wait for
 * the user to satisfy a contract — or any combination.
 */
export interface TourStep {
  /** Stable id (used in telemetry / "resume from step" later). */
  id?: string;
  /** Tooltip title (plain text or HTML). */
  title: string;
  /** Tooltip body (plain text or HTML). */
  body?: string;
  /** Where the tooltip should attach. Default: { kind: "rect", rect: viewport-center }. */
  anchor?: AnchorSpec;
  /** Tooltip side. Default: "bottom". */
  side?: TooltipSide;
  /** Highlight options for this step (skip → no cutout). */
  highlight?: HighlightOptions | null;
  /** Optional "perform" actions executed when the step becomes active. */
  perform?: PerformSpec[];
  /** Optional "wait for" gate — "Next" is disabled until satisfied. */
  waitFor?: WaitForSpec;
  /** Custom label for the primary button on this step. */
  primaryLabel?: string;
  /** When true, no buttons are rendered; the engine only advances via
   *  waitFor or via Tour.next() from the host. */
  passive?: boolean;
  /** When true, the "Skip" button is hidden on this step. */
  hideSkip?: boolean;
  /** Optional element/selector whose click should advance to the next
   *  step automatically (a-la Intro.js `element` clicks). */
  advanceOnClick?: string | HTMLElement;
  /** Optional metadata bag — host app can read it via onStep lifecycle. */
  meta?: Record<string, unknown>;
}

/** A complete tour scenario. */
export interface TourScenario {
  /** Stable id — used as a localStorage key to remember "done". */
  id: string;
  /** Human-readable name. */
  name: string;
  /** Ordered steps. */
  steps: TourStep[];
  /** Default: show "Skip" button. */
  skippable?: boolean;
  /** Default "Next" label. */
  primaryLabel?: string;
  /** Default "Skip" label. */
  skipLabel?: string;
  /** Default "Done" label (last step). */
  doneLabel?: string;
  /** Default "Back" label. */
  backLabel?: string;
  /** Auto-advance when waitFor resolves. Default: false (user clicks Next). */
  autoAdvance?: boolean;
  /** Called when the scenario starts. */
  onStart?: (tour: TourHandle) => void;
  /** Called when the scenario completes normally. */
  onComplete?: (tour: TourHandle) => void;
  /** Called when the user skips. */
  onSkip?: (tour: TourHandle) => void;
  /** Called for each step transition. */
  onStep?: (step: TourStep, index: number, tour: TourHandle) => void;
}

/** Public lifecycle handle returned by Tour.run(). */
export interface TourHandle {
  readonly scenario: TourScenario;
  readonly step: TourStep | null;
  readonly index: number;
  next(): void;
  prev(): void;
  skip(): void;
  /** Resolve the current step's waitFor gate externally. */
  resolve(signal?: string): void;
  /** Emit a signal into the engine bus. */
  signal(name: string, payload?: unknown): void;
  /** Tear down without firing onSkip/onComplete. */
  cancel(): void;
}

/** Engine-level options. */
export interface TourOptions {
  /** Container for overlay nodes. Default: document.body. */
  container?: HTMLElement;
  /** Stylesheet URL or inline CSS string (already injected by default). */
  styles?: string;
  /** zIndex base for overlays. Default: 10000. */
  baseZIndex?: number;
  /** Telemetry sink. */
  log?: (level: "info" | "warn" | "error", message: string) => void;
}

/** Optional engine-level hooks. */
export interface TourHooks {
  /** Resolve an anchor that came in as a string (treated as a CSS selector). */
  resolveAnchor?: (anchor: unknown) => AnchorSpec | null;
  /** Custom dim renderer (replaces the default SVG-cutout). */
  renderDim?: (ctx: DimContext) => void;
  /** Custom tooltip renderer (replaces the default). */
  renderTooltip?: (ctx: TooltipContext) => HTMLElement | null;
}

export interface DimContext {
  container: HTMLElement;
  rect: Rect | null;
  options: HighlightOptions;
}
export interface TooltipContext {
  container: HTMLElement;
  step: TourStep;
  index: number;
  total: number;
  side: TooltipSide;
  anchor: Rect | null;
  primaryLabel: string;
  skipLabel: string;
  backLabel: string;
  doneLabel: string;
  isLast: boolean;
  isFirst: boolean;
  skippable: boolean;
  onPrimary: () => void;
  onBack: () => void;
  onSkip: () => void;
}
