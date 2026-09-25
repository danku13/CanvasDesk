/**
 * @file web-onboarding/src/styles.ts
 * @summary Default CSS for tooltips + dim overlay.
 *
 * Exported as a JS string so the engine can inject it without a CSS
 * import (works in vanilla script-tag scenarios, no Vite needed).
 *
 * The palette intentionally echoes CanvasDesk's existing dark UI
 * (#14161a canvas, #1d2128 panels) so the tour blends in.
 */
export const DEFAULT_STYLES = `
.cd-tour-tooltip {
  position: fixed;
  background: #1d2128;
  color: #d5d9e0;
  border: 1px solid #333a44;
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0,0,0,0.45);
  padding: 14px 16px 10px;
  font: 13px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif;
  max-width: 420px;
  min-width: 280px;
  pointer-events: auto;
  opacity: 1;
  transform: translateY(0);
  transition: opacity 180ms ease, transform 180ms ease;
}
.cd-tour-tooltip[hidden] { display: none; }

.cd-tour-tooltip__title {
  font-weight: 600;
  font-size: 14px;
  color: #f3f5f8;
  margin-bottom: 4px;
}
.cd-tour-tooltip__body {
  color: #b9c0cb;
  white-space: normal;
  word-wrap: break-word;
}
.cd-tour-tooltip__progress {
  font-size: 11px;
  color: #6b7480;
  margin-top: 6px;
}
.cd-tour-tooltip__footer {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  align-items: center;
  margin-top: 12px;
  border-top: 1px solid #2a3138;
  padding-top: 8px;
}

.cd-tour-btn {
  -webkit-appearance: none;
  appearance: none;
  font: inherit;
  padding: 6px 12px;
  border-radius: 6px;
  border: 1px solid #333a44;
  background: #262c35;
  color: #d5d9e0;
  cursor: pointer;
  min-height: 28px;
  -webkit-user-select: none;
  user-select: none;
  -webkit-tap-highlight-color: transparent;
}
.cd-tour-btn:hover { background: #2e353f; }
.cd-tour-btn:focus-visible {
  outline: 2px solid #8ab4ff;
  outline-offset: 1px;
}
.cd-tour-btn_primary {
  background: #2f6fda;
  border-color: #2f6fda;
  color: #ffffff;
  font-weight: 600;
}
.cd-tour-btn_primary:hover { background: #3a78e0; }
.cd-tour-btn_skip {
  background: transparent;
  border-color: transparent;
  color: #6b7480;
}
.cd-tour-btn_back {
  background: transparent;
}

.cd-tour-caret {
  position: absolute;
  width: 12px;
  height: 12px;
  background: #1d2128;
  border: 1px solid #333a44;
  transform: translate(-50%, -50%) rotate(45deg);
  pointer-events: none;
  opacity: 0;
  transition: opacity 180ms ease;
}
.cd-tour-tooltip[data-caret-dir="up"]    .cd-tour-caret,
.cd-tour-tooltip[data-caret-dir="down"]  .cd-tour-caret,
.cd-tour-tooltip[data-caret-dir="left"]  .cd-tour-caret,
.cd-tour-tooltip[data-caret-dir="right"]  .cd-tour-caret {
  opacity: 1;
}
.cd-tour-tooltip[data-caret-dir="up"]    .cd-tour-caret { border-bottom: none; border-right: none; }
.cd-tour-tooltip[data-caret-dir="down"]  .cd-tour-caret { border-top: none; border-left: none; }
.cd-tour-tooltip[data-caret-dir="left"]  .cd-tour-caret { border-top: none; border-right: none; }
.cd-tour-tooltip[data-caret-dir="right"] .cd-tour-caret { border-bottom: none; border-left: none; }
.cd-tour-tooltip[data-caret-dir="none"]  .cd-tour-caret { display: none; }

.cd-tour-dim {
  cursor: pointer;
}

/* Touch / small screens */
@media (max-width: 480px), (pointer: coarse) {
  .cd-tour-tooltip {
    min-width: 0;
    max-width: calc(100vw - 32px);
    padding: 12px 14px 8px;
  }
  .cd-tour-btn {
    padding: 10px 14px;
    min-height: 36px;
  }
}
`;
