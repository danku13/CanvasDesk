/**
 * @file web-onboarding/src/scenarios/first-run-inline.ts
 * @summary "v2" inline first-run scenario.
 *
 * Complements (does not replace) the existing Rust-side carousel (v1):
 *   - v1 carousel explains concepts statically via 9 cards.
 *   - v2 inline tour runs ON TOP of the live canvas and walks the user
 *     through real interactions with `waitFor` gates — Intro.js-style.
 *
 * Run from a custom menu item "Пройти интерактивный тур" (FR-031 menu
 * extension) or from a URL fragment `#tour=first-run-inline`.
 *
 * Host contract:
 *   - #w6-toolbar must exist (W6 storage panel).
 *   - The host app must emit a "canvas:note-created" signal into the tour
 *     bus when the user double-clicks the canvas to create a note. The
 *     engine listens for this signal via `waitFor.kind: "signal"`.
 *   - The host app should expose `__canvasdesk.tour` = the Tour instance
 *     so it can `tour.signal("canvas:note-created")` from its WASM-bridge.
 */
import type { TourScenario } from "../types";

export const firstRunInlineScenario: TourScenario = {
  id: "cd-first-run-inline",
  name: "Первый запуск: интерактивный тур",
  primaryLabel: "Далее",
  skipLabel: "Пропустить",
  backLabel: "Назад",
  doneLabel: "Готово",
  skippable: true,
  autoAdvance: false,
  steps: [
    {
      id: "welcome",
      title: "Это интерактивный тур",
      body:
        "Карточки-карусель уже показали основы. Теперь попробуем по-настоящему — " +
        "каждый шаг сопровождается действием, и кнопка «Далее» активируется " +
        "только когда вы его выполните. Выход — Esc — всегда виден.",
      side: "center",
    },
    {
      id: "locate-toolbar",
      title: "Панель хранилища",
      body:
        "В правом верхнем углу — кнопки для работы с .canvas-файлами. " +
        "Открыть, недавние, экспорт. Канвас автосохраняется в выбранный файл.",
      side: "bottom",
      anchor: { kind: "selector", selector: "#w6-toolbar" },
    },
    {
      id: "create-note",
      title: "Создайте первую заметку",
      body:
        "Сделайте двойной клик по пустому месту на канвасе. " +
        "Появится новая заметка с курсором внутри — можно сразу писать текст, " +
        "markdown-разметку или формулу Numi. «Далее» активируется, когда " +
        "заметка создана.",
      side: "center",
      passive: true,
      primaryLabel: "Жду действия…",
      waitFor: {
        kind: "signal",
        signals: ["canvas:note-created", "canvas:note-activated"],
        timeout: 120000,
      },
    },
    {
      id: "write-formula",
      title: "Попробуйте формулу Numi",
      body:
        "В заметке напишите: `rps = 1200` и на следующей строке `daily = rps / 86400`. " +
        "Заметка подсветит результат вычисления. Numi понимает единицы " +
        "(rps, ms, MB/s) и подсказки при вводе.",
      side: "top",
      anchor: { kind: "rect", rect: { x: 80, y: 80, width: 320, height: 80 } },
      primaryLabel: "Понятно",
    },
    {
      id: "connect-nodes",
      title: "Связи и поток значений",
      body:
        "Создайте вторую заметку. Перетащите от края первой ко второй — " +
        "появится связь. В первой напишите `$in = 10`, во второй — " +
        "`$in * 2`. Связь передаст значение и вторая заметка покажет 20.",
      side: "top",
      anchor: { kind: "rect", rect: { x: 80, y: 80, width: 320, height: 80 } },
      primaryLabel: "Понятно",
    },
    {
      id: "palette",
      title: "Палитра шаблонов",
      body:
        "Ctrl+P открывает палитру готовых нод: метрики unit-economics " +
        "(ARPU, LTV, CAC), инфраструктура (DB, API-gateway, LB), потоки " +
        "(funnel-conv, retention-d7). Shift+клик по колесу вставляет " +
        "выбранный шаблон.",
      side: "center",
    },
    {
      id: "help",
      title: "Кнопка «?» и F1",
      body:
        "«?» — меню помощи: документация, повтор онбординга (карточки и " +
        "этот интерактивный тур), UI-консоль. F1 — список горячих клавиш. " +
        "Документация по всем функциям — в user-docs/.",
      side: "center",
    },
    {
      id: "done",
      title: "Готово!",
      body:
        "Базовый набор освоен. Остальное — по мере необходимости из " +
        "документации. Канвас сохраняется автоматически.",
      side: "center",
    },
  ],
};
