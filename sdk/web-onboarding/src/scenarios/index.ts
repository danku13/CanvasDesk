/**
 * @file web-onboarding/src/scenarios/index.ts
 * @summary Demo scenarios exported as a single namespace.
 *
 * Russian variants (default) are at top level; English variants are
 * under the `en` namespace. The init script in
 * `crates/canvas-web/index.html` picks based on
 * `window.__canvasdesk.getLanguage()` (canvas-web JS-bridge).
 */
export { toolbarTourScenario } from "./toolbar";
export { firstRunInlineScenario } from "./first-run-inline";
export { paletteTourScenario } from "./palette-tour";
export { calculationsTourScenario } from "./calculations-tour";
export { schemeGalleryTourScenario } from "./scheme-gallery";

export * as en from "./en";
