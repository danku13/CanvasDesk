/**
 * @file web-onboarding/src/scenarios/index.ts
 * @summary Demo scenarios exported as a single namespace.
 *
 * Each scenario is plain data — no functions except lifecycle callbacks.
 * They serve as:
 *   1. Ready-to-run examples for first-time integrators.
 *   2. A test surface for the engine (smoke coverage of anchor / waitFor /
 *      perform / passive / advanceOnClick).
 *   3. The "v2" layer of CanvasDesk onboarding — interactive checkpoints
 *      that complement the existing Rust-side carousel (v1).
 */
export { toolbarTourScenario } from "./toolbar";
export { firstRunInlineScenario } from "./first-run-inline";
export { paletteTourScenario } from "./palette-tour";
export { calculationsTourScenario } from "./calculations-tour";
export { schemeGalleryTourScenario } from "./scheme-gallery";
