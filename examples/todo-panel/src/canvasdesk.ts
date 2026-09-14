/**
 * CanvasDesk Widget SDK — типизированная обёртка над widget-bridge
 * (SPEC §7.6, план M5 §7 T22-A). Без зависимостей; бюджет <150 строк —
 * его проверяет тест `sdk_line_budget` в canvas-widgets (CI).
 *
 * Один и тот же bundle работает в двух режимах («микрофронтенд»):
 *  - виджет: есть window.chrome.webview → JSON-RPC-мост к хосту;
 *  - standalone: моста нет → уведомления теряются молча, запросы
 *    (readDir/stateGet/stateSet) отвергаются с внятной ошибкой —
 *    страницу можно открывать как обычный сайт.
 */

/** Тема канваса (приходит в init и themeChanged). */
export interface ThemeInfo { dark: boolean; accent: string; }

/** props ноды: произвольный JSON-объект, персистится в .canvas вместе со сценой. */
export type Props = Record<string, unknown>;

/** Стартовые данные виджета (ответ хоста на ready). */
export interface InitData { nodeId: string; props: Props; theme: ThemeInfo; zoom: number; }

/** Запись листинга readDir. */
export interface DirEntry { name: string; isDir: boolean; }

type Listener<T> = (value: T) => void;
interface WebviewShim { postMessage(message: unknown): void; }

const webview = (): WebviewShim | null => {
  const w = window as { chrome?: { webview?: WebviewShim } };
  return (w.chrome && w.chrome.webview) || null;
};

const listeners: {
  init: Listener<InitData> | null;
  props: Listener<Props> | null;
  theme: Listener<ThemeInfo> | null;
  visibility: Listener<boolean> | null;
} = { init: null, props: null, theme: null, visibility: null };

const pending = new Map<number, { resolve(v: unknown): void; reject(e: Error): void }>();
let nextId = 1;

function send(method: string, params: unknown, id?: number): void {
  const shim = webview();
  if (!shim) return; // standalone: уведомления уходят в никуда — это норма
  const message: Record<string, unknown> = { jsonrpc: "2.0", method, params };
  if (id !== undefined) message.id = id;
  shim.postMessage(message);
}

function request<T>(method: string, params: unknown): Promise<T> {
  return new Promise((resolve, reject) => {
    if (!webview()) {
      reject(new Error(`CanvasDesk: мост недоступен (standalone-режим) — ${method} отвергнут`));
      return;
    }
    const id = nextId++;
    pending.set(id, { resolve: resolve as (v: unknown) => void, reject });
    send(method, params, id);
  });
}

// Ответы на запросы и события хоста приходят одним window-"message".
window.addEventListener("message", (event: MessageEvent) => {
  const msg = event.data as {
    jsonrpc?: string; id?: number; result?: unknown; method?: string; params?: {
      nodeId?: string; props?: Props; theme?: ThemeInfo; visible?: boolean; zoom?: number;
    };
  };
  if (!msg || msg.jsonrpc !== "2.0") return;
  if (msg.id !== undefined) { // ответ хоста: result, ошибки — {error: "текст"}
    const handler = pending.get(msg.id);
    if (!handler) return;
    pending.delete(msg.id);
    const error = (msg.result as { error?: unknown } | undefined)?.error;
    if (typeof error === "string") handler.reject(new Error(error));
    else handler.resolve(msg.result);
    return;
  }
  const p = msg.params || {};
  switch (msg.method) {
    case "init":
      listeners.init?.({ nodeId: p.nodeId || "", props: p.props || {}, theme: p.theme!, zoom: p.zoom || 1 });
      break;
    case "propsChanged": listeners.props?.(p.props || {}); break;
    case "themeChanged": listeners.theme?.(p.theme!); break;
    case "visibility": listeners.visibility?.(!!p.visible); break;
  }
});

export const CanvasDesk = {
  /** Моста нет — страница открыта как обычный сайт (режим микрофронта). */
  standalone: !webview(),

  /** Регистрирует init-колбэк и сообщает хосту о готовности. Вызывать один раз. */
  init(cb: Listener<InitData>): void {
    listeners.init = cb;
    send("ready", {});
  },
  /** props изменены хостом: undo/redo, повторная инициализация. */
  onProps(cb: Listener<Props>): void { listeners.props = cb; },
  /** Смена темы канваса. */
  onTheme(cb: Listener<ThemeInfo>): void { listeners.theme = cb; },
  /** Переход live ↔ snapshot: невидимому виджету стоит гасить таймеры. */
  onVisibility(cb: Listener<boolean>): void { listeners.visibility = cb; },

  /** Сохранить props в .canvas (на стороне хоста — undo-шаг и автосейв). */
  setProps(props: Props): void { send("setProps", { props }); },
  /** Запросить размер ноды в мировых px (хост клампит 160..2000). */
  resize(w: number, h: number): void { send("resize", { w, h }); },
  /** Уведомление в HUD канваса (строка внизу, ~3 с) и в лог хоста. */
  toast(text: string): void { send("toast", { text }); },
  /** Открыть файл в системном приложении (permission `shell:open`). */
  openFile(path: string): void { send("openFile", { path }); },

  /** Листинг allowlist-директории (permission `fs:read`): имена + isDir. */
  readDir(path: string): Promise<DirEntry[]> {
    return request<{ entries?: DirEntry[] }>("readDir", { path }).then((r) => r.entries || []);
  },
  /** Объёмное состояние из SQLite `widget_state` — переживает перезапуск. */
  stateGet(key: string): Promise<string | null> {
    return request<{ value?: string | null }>("stateGet", { key }).then((r) => r.value ?? null);
  },
  /** Записать объёмное состояние (значение — строка, JSON для структур). */
  stateSet(key: string, value: string): Promise<void> {
    return request("stateSet", { key, value }).then(() => undefined);
  },
};

// Глобал для script-тега (сборка sdk/canvasdesk.js, формат iife) и для консоли.
(globalThis as { CanvasDesk?: typeof CanvasDesk }).CanvasDesk = CanvasDesk;
