/**
 * @file web-onboarding/src/scenarios/toolbar.ts
 * @summary Walkthrough of the W6 storage toolbar (#btn-open / #btn-recent /
 * #btn-export). Smallest scenario — used as a smoke test for the engine
 * and a quick "what is this button?" overlay for end users.
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
        "В верхнем правом углу — три кнопки для работы с .canvas-файлами. " +
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
      id: "recent",
      title: "Недавние",
      body:
        "Кнопка показывает последний открывавшийся канвас. " +
        "Полное имя — в title-тултипе, ширина ограничена, чтобы панель " +
        "никогда не уходила за край экрана.",
      side: "bottom",
      anchor: { kind: "selector", selector: "#btn-recent" },
    },
    {
      id: "export",
      title: "Экспорт .canvas",
      body:
        "Скачивает последнюю сохранённую версию канваса. Удобно для бэкапа " +
        "или если автосейв в файл недоступен (например, в браузере без " +
        "File System Access API).",
      side: "bottom",
      anchor: { kind: "selector", selector: "#btn-export" },
    },
    {
      id: "done",
      title: "Готово",
      body: "Канвас всегда сохраняется автоматически — эти кнопки для ручного контроля.",
      side: "center",
    },
  ],
};
