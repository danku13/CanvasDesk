/**
 * @file web-onboarding/src/scenarios/toolbar.ts
 * @summary Walkthrough of the W6 storage toolbar (#btn-open /
 * #btn-export-html). Smallest scenario — used as a smoke test for the
 * engine and a quick "what is this button?" overlay for end users.
 * FR-107 (C4, №37b): #btn-recent / #btn-export left the toolbar (covered
 * by the canvas manager — entry via the active-canvas chip №21c).
 */
import type { TourScenario } from "../types";

export const toolbarTourScenario: TourScenario = {
  id: "cd-toolbar-tour",
  name: "Тур по панели хранилища",
  primaryLabel: "Далее",
  skipLabel: "Пропустить",
  backLabel: "Назад",
  doneLabel: "Готово",
  skippable: true,
  steps: [
    {
      id: "intro",
      title: "Панель хранилища",
      body:
        "В верхнем правом углу — две кнопки для работы с .canvas-файлами. " +
        "Пройдёмся по каждой.",
      side: "bottom",
      anchor: { kind: "selector", selector: "#w6-toolbar" },
    },
    {
      id: "open",
      title: "Открыть с диска",
      body:
        "Открывает системный диалог выбора .canvas-файла. После выбора " +
        "файл становится активным канвасом и автосохраняется обратно на диск.",
      side: "bottom",
      anchor: { kind: "selector", selector: "#btn-open" },
      advanceOnClick: "#btn-open",
      primaryLabel: "Понятно",
    },
    {
      id: "done",
      title: "Готово",
      body:
        "Канвас всегда сохраняется автоматически. Список канвасов, создание " +
        "и переименование — чип активного канваса в левом верхнем углу.",
      side: "center",
    },
  ],
};
