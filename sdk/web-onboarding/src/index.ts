/**
 * @file web-onboarding/src/index.ts
 * @summary Public API entry.
 *
 * Usage:
 *   import { Tour } from "./web-onboarding";
 *   const tour = new Tour();
 *   tour.run(scenario);
 *
 * Or the bundled vanilla JS:
 *   <script src="canvasdesk-tour.js"></script>
 *   const tour = new CanvasDeskTour.Tour();
 *
 * The library has zero runtime dependencies.
 */

export { Tour } from "./tour";
export type {
  TourHandle, TourOptions, RunOptions, TourScenario, TourStep,
  AnchorSpec, AnchorSelector, AnchorElement, AnchorRect,
  Rect, TooltipSide, HighlightOptions,
  WaitForSpec, WaitForSelector, WaitForPredicate, WaitForEvent, WaitForSignal,
  PerformSpec, PerformClick, PerformFocus, PerformSignal, PerformDispatch,
  TourHooks, DimContext, TooltipContext,
} from "./types";
export { renderDefaultTooltip } from "./tooltip";
export { computePlacement, getElementRect, measureTooltip } from "./positioning";
export {
  performActions, awaitWaitFor, WaitForError, type ActionContext,
} from "./actions";
export { createHighlight, type HighlightController } from "./highlight";
export { DEFAULT_STYLES } from "./styles";

// Demo scenarios — ready to use, editable.
export * as scenarios from "./scenarios/index";
