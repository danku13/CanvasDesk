// Сгенерировано из canvasdesk.ts: npx esbuild canvasdesk.ts --format=iife --target=es2018 --outfile=canvasdesk.js (см. sdk/README.md). Не редактировать руками.
(() => {
  const webview = () => {
    const w = window;
    return w.chrome && w.chrome.webview || null;
  };
  const listeners = { init: null, props: null, theme: null, visibility: null };
  const pending = /* @__PURE__ */ new Map();
  let nextId = 1;
  function send(method, params, id) {
    const shim = webview();
    if (!shim) return;
    const message = { jsonrpc: "2.0", method, params };
    if (id !== void 0) message.id = id;
    shim.postMessage(message);
  }
  function request(method, params) {
    return new Promise((resolve, reject) => {
      if (!webview()) {
        reject(new Error(`CanvasDesk: \u043C\u043E\u0441\u0442 \u043D\u0435\u0434\u043E\u0441\u0442\u0443\u043F\u0435\u043D (standalone-\u0440\u0435\u0436\u0438\u043C) \u2014 ${method} \u043E\u0442\u0432\u0435\u0440\u0433\u043D\u0443\u0442`));
        return;
      }
      const id = nextId++;
      pending.set(id, { resolve, reject });
      send(method, params, id);
    });
  }
  window.addEventListener("message", (event) => {
    var _a, _b, _c, _d, _e;
    const msg = event.data;
    if (!msg || msg.jsonrpc !== "2.0") return;
    if (msg.id !== void 0) {
      const handler = pending.get(msg.id);
      if (!handler) return;
      pending.delete(msg.id);
      const error = (_a = msg.result) == null ? void 0 : _a.error;
      if (typeof error === "string") handler.reject(new Error(error));
      else handler.resolve(msg.result);
      return;
    }
    const p = msg.params || {};
    switch (msg.method) {
      case "init":
        (_b = listeners.init) == null ? void 0 : _b.call(listeners, { nodeId: p.nodeId || "", props: p.props || {}, theme: p.theme, zoom: p.zoom || 1 });
        break;
      case "propsChanged":
        (_c = listeners.props) == null ? void 0 : _c.call(listeners, p.props || {});
        break;
      case "themeChanged":
        (_d = listeners.theme) == null ? void 0 : _d.call(listeners, p.theme);
        break;
      case "visibility":
        (_e = listeners.visibility) == null ? void 0 : _e.call(listeners, !!p.visible);
        break;
    }
  });
  const CanvasDesk = {
    /** Моста нет — страница открыта как обычный сайт (режим микрофронта). */
    standalone: !webview(),
    /** Регистрирует init-колбэк и сообщает хосту о готовности. Вызывать один раз. */
    init(cb) {
      listeners.init = cb;
      send("ready", {});
    },
    /** props изменены хостом: undo/redo, повторная инициализация. */
    onProps(cb) {
      listeners.props = cb;
    },
    /** Смена темы канваса. */
    onTheme(cb) {
      listeners.theme = cb;
    },
    /** Переход live ↔ snapshot: невидимому виджету стоит гасить таймеры. */
    onVisibility(cb) {
      listeners.visibility = cb;
    },
    /** Сохранить props в .canvas (на стороне хоста — undo-шаг и автосейв). */
    setProps(props) {
      send("setProps", { props });
    },
    /** Запросить размер ноды в мировых px (хост клампит 160..2000). */
    resize(w, h) {
      send("resize", { w, h });
    },
    /** Уведомление в HUD канваса (строка внизу, ~3 с) и в лог хоста. */
    toast(text) {
      send("toast", { text });
    },
    /** Открыть файл в системном приложении (permission `shell:open`). */
    openFile(path) {
      send("openFile", { path });
    },
    /** Листинг allowlist-директории (permission `fs:read`): имена + isDir. */
    readDir(path) {
      return request("readDir", { path }).then((r) => r.entries || []);
    },
    /** Объёмное состояние из SQLite `widget_state` — переживает перезапуск. */
    stateGet(key) {
      return request("stateGet", { key }).then((r) => {
        var _a;
        return (_a = r.value) != null ? _a : null;
      });
    },
    /** Записать объёмное состояние (значение — строка, JSON для структур). */
    stateSet(key, value) {
      return request("stateSet", { key, value }).then(() => void 0);
    }
  };
  globalThis.CanvasDesk = CanvasDesk;
})();
