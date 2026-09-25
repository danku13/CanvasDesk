/**
 * @file web-onboarding/src/scenarios/palette-tour.ts
 * @summary Tour of the templates palette (FR-018).
 *
 * Demonstrates: anchor rect (canvas-internal region, no DOM node),
 * advanceOnClick on a synthetic button the host app surfaces, and
 * waitFor(signal) for palette-open.
 */
import type { TourScenario } from "../types";

export const paletteTourScenario: TourScenario = {
  id: "cd-palette-tour",
  name: "Палитра шаблонов",
  primaryLabel: "Далее",
  skipLabel: "Пропустить",
  backLabel: "Назад",
  doneLabel: "Готово",
  skippable: true,
  steps: [
    {
      id: "intro",
      title: "Палитра шаблонов",
      body:
        "В CanvasDesk есть встроенные шаблоны нод: метрики, инфраструктура, " +
        "потоки. Их можно перетаскивать на канвас как готовые блоки.",
      side: "center",
    },
    {
      id: "open",
      title: "Откройте палитру",
      body:
        "Нажмите Ctrl+P (или ⌘+P на Mac). Слева появится колесо-навигатор " +
        "с категориями.",
      side: "center",
      passive: true,
      primaryLabel: "Жду открытия палитры…",
      waitFor: {
        kind: "signal",
        signals: ["canvas:palette-opened"],
        timeout: 60000,
      },
    },
    {
      id: "categories",
      title: "Категории",
      body:
        "Колесо переключает категории: Unit Economics, Product Analytics, " +
        "Infrastructure, Patterns. Shift+клик — выбор категории.",
      side: "right",
      anchor: { kind: "rect", rect: { x: 0, y: 0, width: 240, height: 800 } },
    },
    {
      id: "shift-click",
      title: "Shift+клик — вставить шаблон",
      body:
        "Shift+клик по шаблону вставляет его в центр канваса со стандартными " +
        "параметрами. После вставки параметры редактируются через ПКМ.",
      side: "right",
      anchor: { kind: "rect", rect: { x: 0, y: 0, width: 240, height: 800 } },
    },
    {
      id: "done",
      title: "Готово",
      body: "Шаблоны — это строительные блоки для канвас-схем. Большинство моделей собирается за 5–10 минут.",
      side: "center",
    },
  ],
};
