// Виджет «Календарь» (T21-D): месяц × дни, клик дня — заметка.
// props.notes: {"YYYY-MM-DD": "текст"} — персистентность через SDK
// (setProps → undo-шаг и автосейв на стороне хоста).
(function () {
  "use strict";
  var elMonth = document.getElementById("month");
  var elGrid = document.getElementById("grid");
  var elNote = document.getElementById("note");
  var props = { notes: {} };
  var cursor = new Date(); cursor.setDate(1); // текущий месяц
  var selected = null; // "YYYY-MM-DD"

  function applyTheme(theme) {
    if (!theme) return;
    var root = document.documentElement.style;
    root.setProperty("--bg", theme.dark ? "#1e2430" : "#f5f7fb");
    root.setProperty("--fg", theme.dark ? "#e8ecf4" : "#1a2233");
    root.setProperty("--muted", theme.dark ? "#8a93a6" : "#5b6575");
    root.setProperty("--cell", theme.dark ? "#262e3d" : "#e8ecf4");
    root.setProperty("--cell-hover", theme.dark ? "#313b4e" : "#dce4f0");
    root.setProperty("--accent", theme.accent || "#3B82F6");
  }

  function iso(d) {
    return d.getFullYear() + "-" +
      String(d.getMonth() + 1).padStart(2, "0") + "-" +
      String(d.getDate()).padStart(2, "0");
  }

  function render() {
    elMonth.textContent = cursor.toLocaleDateString("ru-RU", {
      month: "long", year: "numeric"
    });
    elGrid.textContent = "";
    ["пн", "вт", "ср", "чт", "пт", "сб", "вс"].forEach(function (d) {
      var cell = document.createElement("div");
      cell.className = "dow"; cell.textContent = d;
      elGrid.appendChild(cell);
    });
    var first = new Date(cursor);
    var shift = (first.getDay() + 6) % 7; // пн=0
    var today = iso(new Date());
    var days = new Date(cursor.getFullYear(), cursor.getMonth() + 1, 0).getDate();
    for (var i = 0; i < shift; i++) elGrid.appendChild(document.createElement("div"));
    for (var d = 1; d <= days; d++) {
      (function (d) {
        var date = new Date(cursor.getFullYear(), cursor.getMonth(), d);
        var key = iso(date);
        var cell = document.createElement("div");
        cell.className = "day";
        cell.textContent = String(d);
        if (key === today) cell.classList.add("today");
        if (props.notes && props.notes[key]) cell.classList.add("hasnote");
        if (key === selected) cell.classList.add("selected");
        cell.addEventListener("click", function () {
          selected = key;
          elNote.style.display = "block";
          elNote.value = (props.notes && props.notes[key]) || "";
          render();
        });
        elGrid.appendChild(cell);
      })(d);
    }
  }

  function saveNote() {
    if (!selected) return;
    props.notes = props.notes || {};
    if (elNote.value.trim()) props.notes[selected] = elNote.value;
    else delete props.notes[selected];
    CanvasDesk.setProps({ notes: props.notes });
    render();
  }
  elNote.addEventListener("keydown", function (e) {
    if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); saveNote(); }
  });
  elNote.addEventListener("blur", saveNote);

  document.getElementById("prev").addEventListener("click", function () {
    cursor.setMonth(cursor.getMonth() - 1); render();
  });
  document.getElementById("next").addEventListener("click", function () {
    cursor.setMonth(cursor.getMonth() + 1); render();
  });

  CanvasDesk.onTheme(applyTheme);
  CanvasDesk.onProps(function (next) {
    props.notes = (next && next.notes) || {};
    render();
  });
  CanvasDesk.init(function (data) {
    applyTheme(data.theme);
    if (data.props && data.props.notes) props.notes = data.props.notes;
    render();
  });

  // Standalone (страница без хоста): рендерим пустой месяц — «микрофронтенд»
  if (CanvasDesk.standalone) render();
})();
