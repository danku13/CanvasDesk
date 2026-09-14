// Todo Panel (T22-C): items в props — «микрофронтенд».
// В виджете: props → setProps (undo-шаг и автосейв на стороне хоста).
// Standalone: тот же bundle, фолбэк на localStorage (SDK без моста — no-op).
import { CanvasDesk, type Props, type ThemeInfo } from "./canvasdesk";

interface Item {
  text: string;
  done: boolean;
}

const LS_KEY = "canvasdesk-todo-items";
const elMode = document.getElementById("mode") as HTMLDivElement;
const elForm = document.getElementById("form") as HTMLFormElement;
const elNew = document.getElementById("new") as HTMLInputElement;
const elList = document.getElementById("list") as HTMLDivElement;
const elEmpty = document.getElementById("empty") as HTMLDivElement;

let items: Item[] = [];

function applyTheme(theme: ThemeInfo): void {
  const root = document.documentElement.style;
  root.setProperty("--bg", theme.dark ? "#1e2430" : "#f5f7fb");
  root.setProperty("--fg", theme.dark ? "#e8ecf4" : "#1a2233");
  root.setProperty("--muted", theme.dark ? "#8a93a6" : "#5b6575");
  root.setProperty("--cell", theme.dark ? "#262e3d" : "#e8ecf4");
  root.setProperty("--cell-hover", theme.dark ? "#313b4e" : "#dce4f0");
  root.setProperty("--accent", theme.accent || "#3B82F6");
}

function fromProps(props: Props): Item[] {
  if (!Array.isArray(props.items)) return [];
  return props.items.flatMap((raw) => {
    if (typeof raw !== "object" || raw === null) return [];
    const { text, done } = raw as { text?: unknown; done?: unknown };
    return typeof text === "string" && text ? [{ text, done: !!done }] : [];
  });
}

// Standalone-хранилище: SDK в этом режиме молча игнорирует setProps.
function persist(): void {
  if (CanvasDesk.standalone) {
    localStorage.setItem(LS_KEY, JSON.stringify(items));
    return;
  }
  CanvasDesk.setProps({ items });
}

function standaloneLoad(): Item[] {
  try {
    return fromProps({ items: JSON.parse(localStorage.getItem(LS_KEY) || "[]") });
  } catch {
    return [];
  }
}

function render(): void {
  elList.textContent = "";
  elEmpty.style.display = items.length ? "none" : "block";
  items.forEach((item, index) => {
    const row = document.createElement("div");
    row.className = "item" + (item.done ? " done" : "");

    const check = document.createElement("input");
    check.type = "checkbox";
    check.checked = item.done;
    check.addEventListener("change", () => {
      items[index].done = check.checked;
      render();
      persist();
    });

    const label = document.createElement("span");
    label.textContent = item.text;

    const remove = document.createElement("button");
    remove.title = "Удалить";
    remove.textContent = "✕";
    remove.addEventListener("click", () => {
      items.splice(index, 1);
      render();
      persist();
    });

    row.append(check, label, remove);
    elList.appendChild(row);
  });
}

elForm.addEventListener("submit", (event) => {
  event.preventDefault();
  const text = elNew.value.trim();
  if (!text) return;
  elNew.value = "";
  items.push({ text, done: false });
  render();
  persist();
  elNew.focus();
});

CanvasDesk.onTheme(applyTheme);
CanvasDesk.onProps((props) => {
  // Полная перерисовка DOM — эхо-цикл невозможен: input-события не генерируются
  items = fromProps(props);
  render();
});
CanvasDesk.init((data) => {
  applyTheme(data.theme);
  items = fromProps(data.props);
  render();
});

if (CanvasDesk.standalone) {
  items = standaloneLoad();
  elMode.textContent = "standalone: задачи в localStorage (в виджете — в props канваса)";
} else {
  elMode.textContent = "виджет: задачи в props ноды (.canvas)";
}

render();
