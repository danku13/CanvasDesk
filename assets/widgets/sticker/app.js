// Виджет «Стикер» (T21-D): contenteditable-текст → props.text,
// debounce 400 мс (меньше setProps-потока; undo-коалесценция на хосте).
(function () {
  "use strict";
  var elText = document.getElementById("text");
  var props = { text: "" };
  var timer = null;
  var applying = false; // propsChanged из хоста не эхом в setProps

  function applyTheme(theme) {
    if (!theme) return;
    var root = document.documentElement.style;
    root.setProperty("--bg", theme.dark ? "#2a2f1e" : "#f7f4dd");
    root.setProperty("--fg", theme.dark ? "#f2f0e0" : "#26291a");
    root.setProperty("--muted", theme.dark ? "#9aa07f" : "#7a7f5c");
    root.setProperty("--accent", theme.accent || "#3B82F6");
  }

  function commit() {
    timer = null;
    if (applying) return;
    var text = elText.innerText;
    if (text === (props.text || "")) return;
    props.text = text;
    CanvasDesk.setProps({ text: props.text });
  }

  elText.addEventListener("input", function () {
    if (applying) return;
    if (timer !== null) clearTimeout(timer);
    timer = setTimeout(commit, 400);
  });

  function applyText(text) {
    if (typeof text === "string" && text !== elText.innerText) {
      applying = true;
      elText.innerText = text;
      applying = false;
      props.text = text;
    }
  }

  CanvasDesk.onTheme(applyTheme);
  CanvasDesk.onProps(function (next) {
    if (next && typeof next.text === "string") applyText(next.text);
  });
  CanvasDesk.onVisibility(function () {
    // потеря фокуса при уходе в снапшот — фиксируем черновик
    if (timer !== null) { clearTimeout(timer); commit(); }
  });
  CanvasDesk.init(function (data) {
    applyTheme(data.theme);
    if (data.props && typeof data.props.text === "string") applyText(data.props.text);
  });

  // Standalone: поле редактируемо, setProps уходит в no-op (SDK без моста)
  if (CanvasDesk.standalone) elText.focus();
})();
