/**
 * @file web-onboarding/src/scenarios/ai-mode.ts
 * @summary AI-mode onboarding tour scenario.
 *
 * Walks the user through AI features after the AI-mode selection screen:
 *   1. Highlights the AI status panel (right, above minimap)
 *   2. Shows the Settings → AI and Models tab
 *   3. Explains privacy modes (Local / Cloud / Self-hosted)
 *   4. Points to user-docs/ai-features.md for full guide
 *
 * Run from the AI-mode onboarding screen "Продолжить" button,
 * or from URL fragment `#tour=ai-mode`.
 *
 * Host contract:
 *   - #aistatus must exist (AI status panel, right side above minimap).
 *   - Settings modal with 9th tab "AI and Models" must be openable.
 *   - The host app should expose `__canvasdesk.tour` = the Tour instance.
 */
import type { TourScenario } from "../types";

export const aiModeScenario: TourScenario = {
  id: "cd-ai-mode",
  name: "AI-функции CanvasDesk",
  primaryLabel: "Далее",
  skipLabel: "Пропустить",
  backLabel: "Назад",
  doneLabel: "Готово",
  skippable: true,

  steps: [
    {
      target: "#aistatus",
      title: "Статусная панель AI",
      text: [
        "Здесь видно, какая модель активна, сколько AI-функций включено и сколько потрачено за день.",
        "",
        "Клик по функции (Suggest ✓ / Graph ✓ / Agent ✓) — вкл/выкл.",
        "⚙ — открыть настройки AI. ⏸ — пауза всех AI-функций.",
        "",
        "На узких экранах панель скрывается.",
      ].join("\n"),
      attachTo: { position: "left" },
      waitFor: { kind: "click", target: "next" },
    },
    {
      target: "#aiGear",
      title: "Настройки AI and Models",
      text: [
        "9-й таб настроек — выбор провайдера для каждой функции отдельно:",
        "• Suggest — BYOK / Ollama / Laya / Off",
        "• Graph Builder — ChatGPT OAuth / BYOK / Ollama",
        "• Agent Panel — ChatGPT OAuth / BYOK / Ollama",
        "",
        "Здесь же: API-ключ, модель, cost-лимит, confidence-порог, data residency.",
      ].join("\n"),
      attachTo: { position: "bottom" },
      waitFor: { kind: "click", target: "next" },
    },
    {
      target: "#aiResid",
      title: "Privacy: где живут ваши данные",
      text: [
        "Local only — данные не покидают машину (Laya/Ollama).",
        "Cloud — контекст уходит провайдеру, значения маскируются: price=<redacted> руб.",
        "Self-hosted — данные в вашем контуре, нужен OpenAI-compatible endpoint.",
        "",
        "Ключи хранятся в OS keychain, не в файлах. Бэкенд не участвует.",
      ].join("\n"),
      attachTo: { position: "top" },
      waitFor: { kind: "click", target: "next" },
    },
    {
      target: "#aiCostLim",
      title: "Cost-лимит",
      text: [
        "Дневной лимит расходов на LLM (по умолчанию $1.00).",
        "",
        "При 80% — диалог расширения. При 100% — LLM-запросы отклоняются, Suggest работает на lex (без AI).",
        "",
        "Стоимость видна в статусной панели: Session и Day.",
      ].join("\n"),
      attachTo: { position: "top" },
      waitFor: { kind: "click", target: "next" },
    },
    {
      target: null,
      title: "Готово!",
      text: [
        "AI-функции настроены. Полный гайд — user-docs/ai-features.md.",
        "",
        "Suggest работает при редактировании нод.",
        "Graph Builder — из меню или .byok-файла.",
        "Agent Panel — Ctrl+I.",
        "",
        "Изменить настройки можно в любой момент: ⚙ в статусной панели.",
      ].join("\n"),
      attachTo: null,
      waitFor: { kind: "click", target: "done" },
    },
  ],
};
