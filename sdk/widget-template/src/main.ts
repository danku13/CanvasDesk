// Демо-виджет шаблона: счётчик в props. Показаны все базовые приёмы SDK:
// init/onProps/onTheme + setProps/toast + standalone-детект.
// Свой виджет начните с удаления этого файла и index.html-разметки.
import { CanvasDesk, type Props, type ThemeInfo } from "./canvasdesk";

interface State {
  count: number;
}

const elValue = document.getElementById("value") as HTMLDivElement;
const elHint = document.getElementById("hint") as HTMLDivElement;

let state: State = { count: 0 };

function applyTheme(theme: ThemeInfo): void {
  const root = document.documentElement.style;
  root.setProperty("--bg", theme.dark ? "#1e2430" : "#f5f7fb");
  root.setProperty("--fg", theme.dark ? "#e8ecf4" : "#1a2233");
  root.setProperty("--muted", theme.dark ? "#8a93a6" : "#5b6575");
  root.setProperty("--cell", theme.dark ? "#262e3d" : "#e8ecf4");
  root.setProperty("--cell-hover", theme.dark ? "#313b4e" : "#dce4f0");
  root.setProperty("--accent", theme.accent || "#3B82F6");
}

function render(): void {
  elValue.textContent = String(state.count);
}

// props — единственный канал персистентности, переживают перезапуск и undo.
function toState(props: Props): State {
  const count = typeof props.count === "number" ? Math.trunc(props.count) : 0;
  return { count };
}

function persist(): void {
  CanvasDesk.setProps({ count: state.count });
}

document.getElementById("inc")!.addEventListener("click", () => {
  state.count++;
  render();
  persist();
});

document.getElementById("dec")!.addEventListener("click", () => {
  state.count--;
  render();
  persist();
});

document.getElementById("say")!.addEventListener("click", () => {
  CanvasDesk.toast(`Счётчик: ${state.count}`);
});

CanvasDesk.onTheme(applyTheme);
CanvasDesk.onProps((props) => {
  state = toState(props);
  render();
});
CanvasDesk.init((data) => {
  applyTheme(data.theme);
  state = toState(data.props);
  render();
});

elHint.textContent = CanvasDesk.standalone
  ? "standalone: страница открыта без хоста, setProps — no-op"
  : "виджет: props сохраняются в .canvas";

render();
