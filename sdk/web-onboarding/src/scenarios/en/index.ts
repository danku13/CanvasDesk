/**
 * @file web-onboarding/src/scenarios/en/index.ts
 * @summary English variants of the inline-onboarding scenarios.
 *
 * Mirrors the Russian scenarios in `../` with translated text. IDs
 * are identical — only title/body/labels change. The bundle exposes
 * them as `scenarios.en.<scenarioName>`; the init script picks based
 * on `window.__canvasdesk.getLanguage()`.
 *
 * Anchors (selectors / rects) are identical to the Russian variants
 * — they reference DOM IDs and viewport regions that don't depend on
 * the language. Only the visible text changes.
 */
import type { TourScenario } from "../../types";

export const toolbarTourScenario: TourScenario = {
  id: "cd-toolbar-tour",
  name: "Storage Toolbar Tour",
  primaryLabel: "Next",
  skipLabel: "Skip",
  backLabel: "Back",
  doneLabel: "Done",
  skippable: true,
  steps: [
    {
      id: "intro", title: "Storage Toolbar",
      body:
        "Top-right corner — three buttons for managing .canvas files. " +
        "We'll walk through each.",
      side: "bottom", anchor: { kind: "selector", selector: "#w6-toolbar" },
    },
    {
      id: "open", title: "Open from Disk",
      body:
        "Opens the system file picker for .canvas files. Once selected, " +
        "the file becomes the active canvas and auto-saves back to disk.",
      side: "bottom", anchor: { kind: "selector", selector: "#btn-open" },
      advanceOnClick: "#btn-open", primaryLabel: "Got it",
    },
    {
      id: "recent", title: "Recent",
      body:
        "Button shows the most recently opened canvas. Full name is in " +
        "the title tooltip; width is bounded so the panel never extends " +
        "off-screen.",
      side: "bottom", anchor: { kind: "selector", selector: "#btn-recent" },
    },
    {
      id: "export", title: "Export .canvas",
      body:
        "Downloads the last saved version of the canvas. Useful for " +
        "backup or if file-system access is unavailable (e.g., browser " +
        "without File System Access API).",
      side: "bottom", anchor: { kind: "selector", selector: "#btn-export" },
    },
    {
      id: "done", title: "Done",
      body: "The canvas always saves automatically — these buttons are for manual control.",
      side: "center",
    },
  ],
};

export const firstRunInlineScenario: TourScenario = {
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
      id: "welcome", title: "This is an interactive tour",
      body:
        "The carousel cards explained the basics. Now let's try for real — " +
        "each step has an action, and the Next button activates only " +
        "after you perform it. Exit via Esc is always available.",
      side: "center",
    },
    {
      id: "locate-toolbar", title: "Storage Toolbar",
      body:
        "Top-right corner — buttons for managing .canvas files. Open, " +
        "recent, export. Canvas auto-saves to the selected file.",
      side: "bottom", anchor: { kind: "selector", selector: "#w6-toolbar" },
    },
    {
      id: "create-note", title: "Create your first note",
      body:
        "Double-click an empty area on the canvas. A new note appears " +
        "with the cursor inside — type text, markdown, or a Numi formula. " +
        "Next activates when the note is created.",
      side: "center", passive: true, primaryLabel: "Waiting for action…",
      waitFor: { kind: "signal", signals: ["canvas:note-created", "canvas:note-activated"], timeout: 120000 },
    },
    {
      id: "write-formula", title: "Try a Numi formula",
      body:
        "In the note, write: rps = 1200, then on the next line daily = rps / 86400. " +
        "The note shows the result. Numi understands units (rps, ms, MB/s) " +
        "and shows hints while typing. Finish editing (Esc or click outside) — " +
        "Next activates when the formula is computed.",
      side: "top", anchor: { kind: "rect", rect: { x: 80, y: 80, width: 320, height: 80 } },
      passive: true, primaryLabel: "Waiting for formula…",
      waitFor: { kind: "signal", signals: ["canvas:formula-written"], timeout: 180000 },
    },
    {
      id: "connect-nodes", title: "Edges and value flow",
      body:
        "Create a second note. Drag from the first note's edge to the second — " +
        "an edge appears. In the first note write $in = 10, in the second $in * 2. " +
        "The edge passes the value and the second note shows 20.",
      side: "top", anchor: { kind: "rect", rect: { x: 80, y: 80, width: 320, height: 80 } },
      primaryLabel: "Got it",
    },
    {
      id: "palette", title: "Template Palette",
      body:
        "Ctrl+P opens the palette of built-in nodes: unit-economics metrics " +
        "(ARPU, LTV, CAC), infrastructure (DB, API-gateway, LB), flows " +
        "(funnel-conv, retention-d7). Shift+click on the wheel inserts " +
        "the selected template.",
      side: "center",
    },
    {
      id: "help", title: "Help button and F1",
      body:
        "? — help menu: docs, repeat onboarding (carousel and this " +
        "interactive tour), UI console. F1 — hotkeys list. " +
        "Documentation for all features is in user-docs/.",
      side: "center",
    },
    {
      id: "done", title: "Done!",
      body:
        "You've learned the basics. Everything else is available in the " +
        "documentation as needed. Canvas saves automatically.",
      side: "center",
    },
  ],
};

export const paletteTourScenario: TourScenario = {
  id: "cd-palette-tour",
  name: "Template Palette",
  primaryLabel: "Next",
  skipLabel: "Skip",
  backLabel: "Back",
  doneLabel: "Done",
  skippable: true,
  steps: [
    {
      id: "intro", title: "Template Palette",
      body:
        "CanvasDesk has built-in node templates: metrics, infrastructure, flows. " +
        "You can drag them onto the canvas as ready-made blocks.",
      side: "center",
    },
    {
      id: "open", title: "Open the palette",
      body:
        "Press Ctrl+P (or ⌘+P on Mac). The category wheel appears on the left.",
      side: "center", passive: true, primaryLabel: "Waiting for palette to open…",
      waitFor: { kind: "signal", signals: ["canvas:palette-opened"], timeout: 60000 },
    },
    {
      id: "categories", title: "Categories",
      body:
        "The wheel switches categories: Unit Economics, Product Analytics, " +
        "Infrastructure, Patterns. Shift+click — category selection.",
      side: "right", anchor: { kind: "rect", rect: { x: 0, y: 0, width: 240, height: 800 } },
    },
    {
      id: "shift-click", title: "Shift+click — insert template",
      body:
        "Shift+click on a template inserts it in the canvas center with " +
        "default parameters. After insertion, parameters are editable via right-click.",
      side: "right", anchor: { kind: "rect", rect: { x: 0, y: 0, width: 240, height: 800 } },
    },
    {
      id: "done", title: "Done",
      body:
        "Templates are building blocks for canvas schemes. Most models " +
        "can be assembled in 5–10 minutes.",
      side: "center",
    },
  ],
};

export const calculationsTourScenario: TourScenario = {
  id: "cd-calculations-tour",
  name: "Calculations in CanvasDesk",
  primaryLabel: "Next",
  skipLabel: "Skip",
  backLabel: "Back",
  doneLabel: "Done",
  skippable: true,
  steps: [
    {
      id: "intro", title: "Formulas right in notes",
      body:
        "Any note is a Numi formula: declare a variable, use units " +
        "(rps, ms, MB/s), get the result. Edges between notes pass values — " +
        "that's the flow.",
      side: "center",
    },
    {
      id: "assignment", title: "Variable Declaration",
      body:
        "Format: name = expression. Example: rps = 1200. The note highlights " +
        "the name and value; cursor is in the right-hand side editor.",
      side: "top", anchor: { kind: "rect", rect: { x: 80, y: 120, width: 320, height: 60 } },
    },
    {
      id: "units", title: "Units",
      body:
        "Numi understands units: rps, ms, MB/s, €, users. They are checked " +
        "for compatibility — you can't add rps and ms. Hints appear while typing.",
      side: "top", anchor: { kind: "rect", rect: { x: 80, y: 180, width: 320, height: 60 } },
    },
    {
      id: "value-flow", title: "Value Flow via Edges",
      body:
        "An edge between notes passes a value. In the downstream note, " +
        "$in — incoming value, $N — array of all incoming. Recomputation " +
        "is instant on any note change.",
      side: "top", anchor: { kind: "rect", rect: { x: 80, y: 240, width: 320, height: 60 } },
    },
    {
      id: "whatif", title: "What-if Scenarios",
      body:
        "Open the What-if panel (FR-017) and set value ranges — CanvasDesk " +
        "calculates the result's sensitivity. Useful for unit-economics: " +
        "\"what if conversion drops by 10%\".",
      side: "center",
    },
    {
      id: "monte-carlo", title: "Monte Carlo (FR-066)",
      body:
        "For probabilistic models — set parameter distributions and " +
        "CanvasDesk runs the simulation. Results display as a histogram " +
        "in the result panel.",
      side: "center",
    },
    {
      id: "done", title: "Done",
      body: "Calculations in CanvasDesk — Numi + value graph + simulations. No Excel needed.",
      side: "center",
    },
  ],
};

export const schemeGalleryTourScenario: TourScenario = {
  id: "cd-scheme-gallery-tour",
  name: "Scheme Gallery",
  primaryLabel: "Next",
  skipLabel: "Skip",
  backLabel: "Back",
  doneLabel: "Done",
  skippable: true,
  steps: [
    {
      id: "intro", title: "Scheme Gallery",
      body:
        "Ready-made models: cohort-launch, intro-whatif, investment-case, " +
        "runway, support-staffing, capacity-service, unit-economics, " +
        "project-budget, renovation-estimate. Each is a starting point " +
        "for a specific task — no need to build from scratch.",
      side: "center",
    },
    {
      id: "open", title: "Open the gallery",
      body:
        "Ctrl+P (or ⌘+P on Mac) opens the gallery. Alternatively, click " +
        "the Try button at the end of the v1 onboarding carousel. The step " +
        "activates when the gallery is visible.",
      side: "center", passive: true, primaryLabel: "Waiting for gallery to open…",
      waitFor: { kind: "signal", signals: ["canvas:scheme-gallery-opened", "canvas:palette-opened"], timeout: 60000 },
    },
    {
      id: "categories", title: "Scheme Categories",
      body:
        "Left — list of categories: Unit Economics (ARPU, LTV, CAC, retention, funnel), " +
        "Product Analytics (NPS, MAU, stickiness), Infrastructure (DB, API-gateway, LB, queue), " +
        "Patterns (cohort-launch, intro-whatif, investment-case).",
      side: "right", anchor: { kind: "rect", rect: { x: 0, y: 0, width: 240, height: 800 } },
      primaryLabel: "Got it",
    },
    {
      id: "preview", title: "Scheme Preview",
      body:
        "Click a scheme card to preview: a mini-scheme with notes, formulas, edges. " +
        "You can read the structure before applying. The step activates when preview opens.",
      side: "center", passive: true, primaryLabel: "Open a preview…",
      waitFor: { kind: "signal", signals: ["canvas:scheme-preview-shown"], timeout: 120000 },
    },
    {
      id: "apply", title: "Apply the Scheme",
      body:
        "In the preview — Apply button. The scheme becomes the active canvas " +
        "(replacing the current scene — undo works). All formulas and edges " +
        "are preserved, you can edit for your task. The step activates when applied.",
      side: "center", passive: true, primaryLabel: "Waiting for scheme to apply…",
      waitFor: { kind: "signal", signals: ["canvas:scheme-applied"], timeout: 180000 },
    },
    {
      id: "edit", title: "Edit for your needs",
      body:
        "After applying — the same channel as a regular canvas: double-click " +
        "creates notes, drag from edge creates edges, right-click — color " +
        "and parameters palette. Change variable values right in formula notes.",
      side: "top", anchor: { kind: "rect", rect: { x: 80, y: 80, width: 400, height: 120 } },
      primaryLabel: "Got it",
    },
    {
      id: "save", title: "Saving",
      body:
        "Canvas auto-saves to the selected file (W6: Open from disk → File System Access) " +
        "or to OPFS (default). Export to .canvas — for backup or sharing.",
      side: "bottom", anchor: { kind: "selector", selector: "#w6-toolbar" },
    },
    {
      id: "done", title: "Done",
      body:
        "Scheme gallery — quick start for typical models. Most user scenarios " +
        "are covered by built-in schemes; custom ones — assemble from palette templates (Ctrl+P).",
      side: "center",
    },
  ],
};
