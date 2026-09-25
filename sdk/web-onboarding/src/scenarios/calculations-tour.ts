/**
 * @file web-onboarding/src/scenarios/calculations-tour.ts
 * @summary Tour of the Numi-style formula system (FR-013, FR-014, FR-015).
 *
 * Demonstrates a "passive" step whose anchor rect is canvas-internal
 * (no DOM node) — the engine places the tooltip on top of where the
 * formula text would be.
 */
import type { TourScenario } from "../types";

export const calculationsTourScenario: TourScenario = {
  id: "cd-calculations-tour",
  name: "Расчёты в CanvasDesk",
  primaryLabel: "Далее",
  skipLabel: "Пропустить",
  backLabel: "Назад",
  doneLabel: "Готово",
  skippable: true,
  steps: [
    {
      id: "intro",
      title: "Формулы прямо в заметках",
      body:
        "Любая заметка — это Numi-формула: можно объявить переменную, " +
        "использовать единицы (rps, ms, MB/s), получить результат. " +
        "Связи между заметками передают значения — это и есть поток.",
      side: "center",
    },
    {
      id: "assignment",
      title: "Объявление переменной",
      body:
        "Формат: `name = expression`. Пример: `rps = 1200`. Заметка " +
        "подсветит имя и значение, курсор сразу в редактировании " +
        "правой части.",
      side: "top",
      anchor: { kind: "rect", rect: { x: 80, y: 120, width: 320, height: 60 } },
    },
    {
      id: "units",
      title: "Единицы измерения",
      body:
        "Numi понимает единицы: `rps`, `ms`, `MB/s`, `€`, `users`. Они " +
        "проверяются на совместимость — нельзя сложить rps и ms. " +
        "Подсказки появляются при вводе.",
      side: "top",
      anchor: { kind: "rect", rect: { x: 80, y: 180, width: 320, height: 60 } },
    },
    {
      id: "value-flow",
      title: "Поток значений по связям",
      body:
        "Связь между заметками передаёт значение. В подчинённой заметке " +
        "`$in` — входящее значение, `$N` — массив всех входящих. " +
        "Пересчёт — мгновенный, при изменении любой заметки.",
      side: "top",
      anchor: { kind: "rect", rect: { x: 80, y: 240, width: 320, height: 60 } },
    },
    {
      id: "whatif",
      title: "What-if сценарии",
      body:
        "Откройте What-if панель (FR-017) и задайте диапазоны значений — " +
        "CanvasDesk посчитает чувствительность результата. " +
        "Полезно для unit-economics: «что если конверсия упадёт на 10%».",
      side: "center",
    },
    {
      id: "monte-carlo",
      title: "Монте-Карло (FR-066)",
      body:
        "Для вероятностных моделей — задайте распределения параметров " +
        "и CanvasDesk прогонит симуляцию. Результаты отображаются " +
        "как гистограмма в панели результата.",
      side: "center",
    },
    {
      id: "done",
      title: "Готово",
      body: "Расчёты в CanvasDesk — это Numi + граф значений + симуляции. Никакого Excel.",
      side: "center",
    },
  ],
};
