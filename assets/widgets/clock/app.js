// Виджет «Часы» (T20): цифровые часы + дата. SDK-мост (T22): тема из
// init/themeChanged, гашение таймера в снапшоте через onVisibility.
// У часов нет ни permissions, ни props — только ready и события хоста.
(function () {
  "use strict";
  var elTime = document.getElementById("time");
  var elDate = document.getElementById("date");
  var timer = null;

  function applyTheme(theme) {
    if (!theme) return;
    var root = document.documentElement.style;
    root.setProperty("--bg", theme.dark ? "#1e2430" : "#f5f7fb");
    root.setProperty("--fg", theme.dark ? "#e8ecf4" : "#1a2233");
    root.setProperty("--muted", theme.dark ? "#8a93a6" : "#5b6575");
    root.setProperty("--accent", theme.accent || "#3B82F6");
  }

  function render() {
    var now = new Date();
    var h = String(now.getHours()).padStart(2, "0");
    var m = String(now.getMinutes()).padStart(2, "0");
    var s = String(now.getSeconds()).padStart(2, "0");
    elTime.innerHTML = h + "<span>:</span>" + m + "<span>:</span>" + s;
    elDate.textContent = now.toLocaleDateString("ru-RU", {
      weekday: "long", day: "numeric", month: "long", year: "numeric"
    });
  }

  function tickLoop() {
    if (timer !== null) return;
    render();
    timer = setInterval(render, 1000);
  }

  function tickStop() {
    if (timer === null) return;
    clearInterval(timer);
    timer = null;
  }

  CanvasDesk.onTheme(applyTheme);
  CanvasDesk.onVisibility(function (visible) {
    if (visible) { tickLoop(); } else { tickStop(); }
  });
  CanvasDesk.init(function (data) {
    applyTheme(data.theme);
  });

  tickLoop();
})();
