/**
 * @file web-onboarding/src/scenarios/scheme-gallery.ts
 * @summary Tour of the scheme gallery (FR-049: gallery.try action).
 *
 * The scheme gallery is CanvasDesk's template library — pre-built
 * canvas scenes for common unit-economics / capacity / what-if models.
 * Catalog: see `assets/canvas-schemes/com.canvasdesk.scheme.*`.
 *
 * This scenario walks the user through opening the gallery, browsing
 * categories, previewing a scheme, and applying one. It complements
 * the v1 carousel step 7 «Шаблоны» (which explains the concept
 * statically) with an interactive walkthrough.
 *
 * Host contract (signals):
 *   - `canvas:scheme-gallery-opened` — emitted by WASM-bridge when
 *     the gallery panel becomes visible (Ctrl+P or "Попробовать" CTA).
 *   - `canvas:scheme-preview-shown` — emitted when the user clicks a
 *     scheme card and the preview opens.
 *   - `canvas:scheme-applied` — emitted when the user clicks "Apply"
 *     in the preview and the scheme becomes the active canvas.
 */
import type { TourScenario } from "../types";

export const schemeGalleryTourScenario: TourScenario = {
  id: "cd-scheme-gallery-tour",
  name: "Галерея схем",
  primaryLabel: "Далее",
  skipLabel: "Пропустить",
  backLabel: "Назад",
  doneLabel: "Готово",
  skippable: true,
  steps: [
    {
      id: "intro",
      title: "Галерея схем",
      body:
        "Готовые модели: cohort-launch, intro-whatif, investment-case, " +
        "runway, support-staffing, capacity-service, unit-economics, " +
        "project-budget, renovation-estimate. Каждая — стартовая точка " +
        "для своей задачи, не нужно собирать с нуля.",
      side: "center",
    },
    {
      id: "open",
      title: "Откройте галерею",
      body:
        "Ctrl+P (или ⌘+P на Mac) открывает галерею. Альтернатива — " +
        "кнопка «Попробовать» в финале v1 карусели онбординга. " +
        "Шаг активируется, когда галерея видна.",
      side: "center",
      passive: true,
      primaryLabel: "Жду открытия галереи…",
      waitFor: {
        kind: "signal",
        signals: ["canvas:scheme-gallery-opened", "canvas:palette-opened"],
        timeout: 60000,
      },
    },
    {
      id: "categories",
      title: "Категории схем",
      body:
        "Слева — список категорий: Unit Economics (ARPU, LTV, CAC, " +
        "retention, funnel), Product Analytics (NPS, MAU, stickiness), " +
        "Infrastructure (DB, API-gateway, LB, queue), Patterns " +
        "(cohort-launch, intro-whatif, investment-case).",
      side: "right",
      anchor: { kind: "rect", rect: { x: 0, y: 0, width: 240, height: 800 } },
      primaryLabel: "Понятно",
    },
    {
      id: "preview",
      title: "Превью схемы",
      body:
        "Клик по карточке схемы открывает превью: мини-схема с " +
        "заметками, формулами, связями. Можно прочитать структуру " +
        "до применения. Шаг активируется, когда превью открыто.",
      side: "center",
      passive: true,
      primaryLabel: "Откройте превью…",
      waitFor: {
        kind: "signal",
        signals: ["canvas:scheme-preview-shown"],
        timeout: 120000,
      },
    },
    {
      id: "apply",
      title: "Применить схему",
      body:
        "В превью — кнопка «Применить». Схема становится активным " +
        "канвасом (с заменой текущей сцены — undo работает). " +
        "Все формулы и связи сохраняются, можно редактировать под свою " +
        "задачу. Шаг активируется, когда схема применена.",
      side: "center",
      passive: true,
      primaryLabel: "Жду применения схемы…",
      waitFor: {
        kind: "signal",
        signals: ["canvas:scheme-applied"],
        timeout: 180000,
      },
    },
    {
      id: "edit",
      title: "Редактируйте под себя",
      body:
        "После применения — канал тот же, что у обычного канваса: " +
        "двойной клик создаёт заметки, drag от края — связи, ПКМ — " +
        "палитра цвета и параметров. Менять значения переменных — " +
        "прямо в заметках с формулами.",
      side: "top",
      anchor: { kind: "rect", rect: { x: 80, y: 80, width: 400, height: 120 } },
      primaryLabel: "Понятно",
    },
    {
      id: "save",
      title: "Сохранение",
      body:
        "Канвас автосохраняется в выбранный файл (W6: «Открыть с диска» " +
        "→ File System Access) или в OPFS (по умолчанию). " +
        "Экспорт в .canvas — для бэкапа или шеринга.",
      side: "bottom",
      anchor: { kind: "selector", selector: "#w6-toolbar" },
    },
    {
      id: "done",
      title: "Готово",
      body:
        "Галерея схем — быстрый старт для типовых моделей. " +
        "Большинство пользовательских сценариев покрывается готовыми " +
        "схемами; кастомные — собирайте из шаблонов палитры (Ctrl+P).",
      side: "center",
    },
  ],
};
